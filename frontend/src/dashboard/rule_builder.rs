use super::rules_section::RuleData;
use crate::utils::api::Api;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;

#[derive(Clone, PartialEq, Deserialize)]
struct RuleSourceOption {
    source_type: String,
    label: String,
    available: bool,
    meta: serde_json::Value,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
enum SourceConfig {
    Email,
    Chat {
        platform: String,
        limit: u32,
    },
    Weather {
        #[serde(default)]
        location: String,
    },
    Internet {
        query: String,
    },
    Tesla,
    Mcp {
        server: String,
        tool: String,
        args: String,
    },
    Events,
}

// ---------------------------------------------------------------------------
// FlowNode: frontend representation of the evaluation tree
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FlowNode {
    LlmCondition {
        prompt: String,
        #[serde(default)]
        fetch: Vec<SourceConfig>,
        true_branch: Box<Option<FlowNode>>,
        false_branch: Box<Option<FlowNode>>,
    },
    KeywordCondition {
        keyword: String,
        true_branch: Box<Option<FlowNode>>,
        false_branch: Box<Option<FlowNode>>,
    },
    Action {
        action_type: String,
        config: serde_json::Value,
    },
}

impl FlowNode {
    fn condition_depth(&self) -> usize {
        match self {
            FlowNode::LlmCondition {
                true_branch,
                false_branch,
                ..
            }
            | FlowNode::KeywordCondition {
                true_branch,
                false_branch,
                ..
            } => {
                let t = true_branch
                    .as_ref()
                    .as_ref()
                    .map(|n| n.condition_depth())
                    .unwrap_or(0);
                let f = false_branch
                    .as_ref()
                    .as_ref()
                    .map(|n| n.condition_depth())
                    .unwrap_or(0);
                1 + t.max(f)
            }
            FlowNode::Action { .. } => 0,
        }
    }
}

impl SourceConfig {
    fn type_key(&self) -> &str {
        match self {
            SourceConfig::Email => "email",
            SourceConfig::Chat { .. } => "chat",
            SourceConfig::Weather { .. } => "weather",
            SourceConfig::Internet { .. } => "internet",
            SourceConfig::Tesla => "tesla",
            SourceConfig::Mcp { server, .. } => server.as_str(),
            SourceConfig::Events => "events",
        }
    }

    fn is_type(&self, t: &str) -> bool {
        match self {
            SourceConfig::Email => t == "email",
            SourceConfig::Chat { .. } => t == "chat",
            SourceConfig::Weather { .. } => t == "weather",
            SourceConfig::Internet { .. } => t == "internet",
            SourceConfig::Tesla => t == "tesla",
            SourceConfig::Mcp { server, .. } => t == format!("mcp:{}", server),
            SourceConfig::Events => t == "events",
        }
    }
}

#[derive(Clone, PartialEq, Deserialize)]
struct McpToolOption {
    name: String,
    #[allow(dead_code)]
    description: Option<String>,
}

const BUILDER_STYLES: &str = r#"
.rule-builder-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.7);
    z-index: 1100;
    display: flex;
    justify-content: flex-end;
}
.rule-builder-panel {
    width: 100%;
    max-width: 480px;
    height: 100%;
    background: #1a1a1a;
    overflow-y: auto;
    animation: rbSlideIn 0.3s ease;
    padding: 0 0 2rem 0;
}
@keyframes rbSlideIn {
    from { transform: translateX(100%); }
    to { transform: translateX(0); }
}
.rb-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1.25rem 1.5rem 0.75rem;
    position: sticky;
    top: 0;
    background: #1a1a1a;
    z-index: 10;
}
.rb-header h2 {
    color: #fff;
    font-size: 1.1rem;
    font-weight: 600;
    margin: 0;
}
.rb-close {
    background: transparent;
    border: none;
    color: #888;
    font-size: 1.5rem;
    cursor: pointer;
    padding: 0.25rem 0.5rem;
    line-height: 1;
}
.rb-close:hover { color: #fff; }
.rb-body {
    padding: 0 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 0;
}
.rb-name-input {
    width: 100%;
    background: #12121f;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 8px;
    color: #ddd;
    padding: 0.6rem 0.75rem;
    font-size: 0.9rem;
    margin-bottom: 1rem;
    box-sizing: border-box;
}
.rb-name-input:focus {
    outline: none;
    border-color: rgba(126, 178, 255, 0.4);
}
/* Pipeline connector */
.rb-connector {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 0.25rem 0;
    color: rgba(255, 255, 255, 0.15);
    font-size: 0.7rem;
    line-height: 1;
}
.rb-connector-line {
    width: 1px;
    height: 8px;
    background: rgba(255, 255, 255, 0.15);
}
/* Card shared */
.rb-card {
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 10px;
    overflow: hidden;
    transition: background 0.2s;
}
.rb-card.collapsed {
    background: rgba(255, 255, 255, 0.04);
    cursor: pointer;
}
.rb-card.expanded {
    background: rgba(126, 178, 255, 0.08);
}
.rb-card-header {
    display: flex;
    align-items: center;
    padding: 0.65rem 0.85rem;
    gap: 0.5rem;
}
.rb-card-label {
    font-size: 0.7rem;
    font-weight: 700;
    color: #7EB2FF;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    width: 3rem;
    flex-shrink: 0;
}
.rb-card-summary {
    flex: 1;
    font-size: 0.85rem;
    color: #ccc;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}
.rb-card-chevron {
    color: #555;
    font-size: 0.7rem;
    transition: transform 0.2s;
    flex-shrink: 0;
}
.rb-card.expanded .rb-card-chevron {
    transform: rotate(180deg);
}
.rb-card-content {
    padding: 0 0.85rem 0.85rem;
}
/* Toggle buttons */
.rb-toggle-group {
    display: flex;
    gap: 0.3rem;
    margin-bottom: 0.6rem;
}
.rb-toggle-btn {
    background: transparent;
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #666;
    font-size: 0.8rem;
    padding: 0.35rem 0.75rem;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.15s;
}
.rb-toggle-btn.active {
    color: #7EB2FF;
    background: rgba(126, 178, 255, 0.15);
    border-color: rgba(126, 178, 255, 0.3);
}
.rb-toggle-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
    font-size: 0.7rem;
}
/* Form elements */
.rb-field {
    margin-bottom: 0.5rem;
}
.rb-field-label {
    font-size: 0.7rem;
    color: #888;
    margin-bottom: 0.2rem;
    text-transform: uppercase;
    letter-spacing: 0.03em;
}
.rb-input {
    width: 100%;
    background: #12121f;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 6px;
    color: #ddd;
    padding: 0.45rem 0.6rem;
    font-size: 0.85rem;
    box-sizing: border-box;
}
.rb-input:focus {
    outline: none;
    border-color: rgba(126, 178, 255, 0.4);
}
.rb-select {
    width: 100%;
    background: #12121f;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 6px;
    color: #ddd;
    padding: 0.45rem 0.6rem;
    font-size: 0.85rem;
    box-sizing: border-box;
    appearance: none;
}
.rb-select:focus {
    outline: none;
    border-color: rgba(126, 178, 255, 0.4);
}
.rb-row {
    display: flex;
    gap: 0.5rem;
}
.rb-row > * { flex: 1; }
.rb-textarea {
    width: 100%;
    background: #12121f;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 6px;
    color: #ddd;
    padding: 0.45rem 0.6rem;
    font-size: 0.85rem;
    min-height: 60px;
    resize: vertical;
    font-family: inherit;
    box-sizing: border-box;
}
.rb-textarea:focus {
    outline: none;
    border-color: rgba(126, 178, 255, 0.4);
}
.rb-radio-group {
    display: flex;
    gap: 1rem;
    align-items: center;
}
.rb-radio-label {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.85rem;
    color: #ccc;
    cursor: pointer;
}
.rb-radio-label input[type="radio"] {
    accent-color: #7EB2FF;
}
/* Submit button */
.rb-submit {
    width: 100%;
    background: #7EB2FF;
    border: none;
    color: #1a1a1a;
    font-size: 0.9rem;
    font-weight: 600;
    padding: 0.7rem;
    border-radius: 8px;
    cursor: pointer;
    margin-top: 1rem;
    transition: opacity 0.15s;
}
.rb-submit:hover { opacity: 0.9; }
.rb-submit:disabled {
    opacity: 0.4;
    cursor: not-allowed;
}
.rb-error {
    color: #ff6b6b;
    font-size: 0.8rem;
    margin-top: 0.5rem;
}
.rb-fetch-checks {
    display: flex;
    gap: 0.75rem;
    margin-top: 0.3rem;
}
.rb-check-label {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    font-size: 0.8rem;
    color: #aaa;
    cursor: pointer;
}
.rb-check-label input[type="checkbox"] {
    accent-color: #7EB2FF;
}
.rb-autocomplete {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    background: #1e1e2f;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 6px;
    max-height: 180px;
    overflow-y: auto;
    z-index: 20;
    margin-top: 2px;
}
.rb-autocomplete-item {
    padding: 0.4rem 0.6rem;
    font-size: 0.8rem;
    color: #ccc;
    cursor: pointer;
}
.rb-autocomplete-item:hover {
    background: rgba(126, 178, 255, 0.15);
    color: #fff;
}
.rb-template-group {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    margin-bottom: 0.6rem;
}
.rb-template-btn {
    background: transparent;
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: #666;
    font-size: 0.75rem;
    padding: 0.25rem 0.6rem;
    border-radius: 12px;
    cursor: pointer;
    transition: all 0.15s;
}
.rb-template-btn:hover {
    border-color: rgba(255, 255, 255, 0.2);
    color: #aaa;
}
.rb-template-btn.active {
    color: #7EB2FF;
    background: rgba(126, 178, 255, 0.12);
    border-color: rgba(126, 178, 255, 0.25);
}
.rb-template-desc {
    background: rgba(126, 178, 255, 0.06);
    border: 1px solid rgba(126, 178, 255, 0.12);
    border-radius: 6px;
    padding: 0.5rem 0.65rem;
    font-size: 0.8rem;
    color: #aaa;
    line-height: 1.4;
    margin-bottom: 0.5rem;
}
.rb-template-edit-link {
    display: inline-block;
    font-size: 0.75rem;
    color: #7EB2FF;
    cursor: pointer;
    margin-top: 0.2rem;
    background: none;
    border: none;
    padding: 0;
    text-decoration: underline;
    text-decoration-style: dotted;
}
.rb-template-edit-link:hover {
    color: #a8cfff;
}
.rb-field-hint {
    font-size: 0.75rem;
    color: #888;
    margin-top: 0.2rem;
    font-style: italic;
}
.rb-context-hint {
    font-size: 0.75rem;
    color: #888;
    margin-bottom: 0.5rem;
    line-height: 1.4;
}
.rb-source-pills {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    margin-bottom: 0.4rem;
}
.rb-source-pill {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    background: transparent;
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #666;
    font-size: 0.78rem;
    padding: 0.25rem 0.6rem;
    border-radius: 12px;
    cursor: pointer;
    transition: all 0.15s;
}
.rb-source-pill:hover {
    border-color: rgba(255, 255, 255, 0.2);
    color: #aaa;
}
.rb-source-pill.active {
    color: #7EB2FF;
    background: rgba(126, 178, 255, 0.12);
    border-color: rgba(126, 178, 255, 0.25);
}
.rb-source-pill.disabled {
    opacity: 0.35;
    cursor: not-allowed;
}
.rb-source-pill .rb-pill-x {
    font-size: 0.65rem;
    color: rgba(126, 178, 255, 0.6);
    cursor: pointer;
    margin-left: 0.15rem;
}
.rb-source-pill .rb-pill-x:hover {
    color: #ff6b6b;
}
.rb-source-options {
    background: rgba(126, 178, 255, 0.06);
    border: 1px solid rgba(126, 178, 255, 0.12);
    border-radius: 6px;
    padding: 0.5rem 0.65rem;
    margin-bottom: 0.4rem;
}
/* ---- ELSE / Nested flow ---- */
.rb-else-divider {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.6rem 0 0.3rem;
}
.rb-else-divider span {
    font-size: 0.7rem;
    font-weight: 700;
    color: #888;
    text-transform: uppercase;
    letter-spacing: 0.05em;
}
.rb-else-divider-line {
    flex: 1;
    height: 1px;
    background: rgba(255, 255, 255, 0.08);
}
.rb-else-content {
    padding-left: 0.75rem;
    border-left: 2px solid rgba(126, 178, 255, 0.15);
    margin-left: 0.25rem;
}
.rb-add-condition-btn {
    background: transparent;
    border: 1px dashed rgba(255, 255, 255, 0.12);
    color: #666;
    font-size: 0.78rem;
    padding: 0.45rem 0.75rem;
    border-radius: 8px;
    cursor: pointer;
    transition: all 0.15s;
}
.rb-add-condition-btn:hover {
    border-color: rgba(126, 178, 255, 0.3);
    color: #7EB2FF;
}
.rb-do-nothing {
    font-size: 0.78rem;
    color: #555;
    font-style: italic;
}
.rb-nested-card {
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 10px;
    padding: 0.75rem;
    background: rgba(255, 255, 255, 0.02);
    margin-bottom: 0.5rem;
}
.rb-nested-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
}
.rb-nested-label {
    font-size: 0.7rem;
    font-weight: 700;
    color: #7EB2FF;
    text-transform: uppercase;
    letter-spacing: 0.05em;
}
.rb-remove-btn {
    background: transparent;
    border: none;
    color: #666;
    cursor: pointer;
    font-size: 0.75rem;
    padding: 0.15rem 0.4rem;
}
.rb-remove-btn:hover { color: #ff6b6b; }
.rb-branch-label {
    font-size: 0.65rem;
    color: #888;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    margin: 0.5rem 0 0.25rem;
}
.rb-nested-action-summary {
    font-size: 0.8rem;
    color: #aaa;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 6px;
    padding: 0.4rem 0.6rem;
    display: flex;
    align-items: center;
    gap: 0.4rem;
}
.rb-nested-action-icon { color: #7EB2FF; font-size: 0.75rem; }
/* Rule summary bar */
.rb-rule-summary {
    background: rgba(126, 178, 255, 0.06);
    border: 1px solid rgba(126, 178, 255, 0.12);
    border-radius: 8px;
    padding: 0.5rem 0.75rem;
    font-size: 0.85rem;
    color: #bbb;
    line-height: 1.5;
    margin-bottom: 0.75rem;
}
.rb-summary-incomplete {
    color: #666;
    font-style: italic;
}
/* Sentence starters */
.rb-starter-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    margin-bottom: 0.4rem;
}
/* Review card */
.rb-review-card {
    border: 1px solid rgba(74, 222, 128, 0.15);
    border-radius: 10px;
    background: rgba(74, 222, 128, 0.04);
    padding: 0.75rem;
    margin-top: 0.75rem;
}
.rb-review-title {
    font-size: 0.7rem;
    font-weight: 700;
    color: rgba(74, 222, 128, 0.7);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin-bottom: 0.4rem;
}
.rb-review-flow {
    font-size: 0.82rem;
    color: #bbb;
    line-height: 1.7;
}
.rb-review-step {
    padding-left: 0.75rem;
    color: #aaa;
}
.rb-review-step::before {
    content: "-> ";
    color: rgba(74, 222, 128, 0.5);
}
.rb-review-missing {
    color: #f5a623;
    font-size: 0.8rem;
    font-style: italic;
    margin-top: 0.3rem;
}
/* Test panel */
.rb-test-toggle {
    width: 100%;
    background: transparent;
    border: 1px dashed rgba(255, 255, 255, 0.12);
    color: #888;
    font-size: 0.8rem;
    padding: 0.5rem;
    border-radius: 8px;
    cursor: pointer;
    margin-top: 0.75rem;
    transition: all 0.15s;
}
.rb-test-toggle:hover {
    border-color: rgba(126, 178, 255, 0.3);
    color: #7EB2FF;
}
.rb-test-toggle.open {
    border-color: rgba(126, 178, 255, 0.2);
    color: #7EB2FF;
    border-style: solid;
}
.rb-test-panel {
    border: 1px solid rgba(126, 178, 255, 0.12);
    border-radius: 10px;
    padding: 0.75rem;
    margin-top: 0.5rem;
    background: rgba(126, 178, 255, 0.03);
}
.rb-test-presets {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    margin-bottom: 0.5rem;
}
.rb-test-preset {
    background: transparent;
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: #777;
    font-size: 0.72rem;
    padding: 0.2rem 0.5rem;
    border-radius: 10px;
    cursor: pointer;
    transition: all 0.15s;
}
.rb-test-preset:hover {
    border-color: rgba(255, 255, 255, 0.2);
    color: #aaa;
}
.rb-test-preset.active {
    color: #7EB2FF;
    background: rgba(126, 178, 255, 0.1);
    border-color: rgba(126, 178, 255, 0.25);
}
.rb-test-run {
    width: 100%;
    background: rgba(126, 178, 255, 0.15);
    border: 1px solid rgba(126, 178, 255, 0.25);
    color: #7EB2FF;
    font-size: 0.8rem;
    font-weight: 600;
    padding: 0.45rem;
    border-radius: 6px;
    cursor: pointer;
    margin-top: 0.4rem;
    transition: all 0.15s;
}
.rb-test-run:hover { background: rgba(126, 178, 255, 0.25); }
.rb-test-run:disabled { opacity: 0.4; cursor: not-allowed; }
.rb-test-steps {
    margin-top: 0.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
}
.rb-test-step {
    display: flex;
    align-items: flex-start;
    gap: 0.4rem;
    font-size: 0.78rem;
    line-height: 1.4;
    padding: 0.3rem 0;
    border-left: 2px solid rgba(255, 255, 255, 0.08);
    padding-left: 0.6rem;
}
.rb-test-step.deciding {
    border-left-color: rgba(126, 178, 255, 0.4);
    color: #aaa;
}
.rb-test-step.yes {
    border-left-color: rgba(74, 222, 128, 0.5);
    color: #8fd8a8;
}
.rb-test-step.no {
    border-left-color: rgba(255, 165, 0, 0.5);
    color: #d4a06a;
}
.rb-test-step.action {
    border-left-color: rgba(126, 178, 255, 0.5);
    color: #7EB2FF;
}
.rb-test-step.inactive {
    border-left-color: rgba(255, 255, 255, 0.06);
    color: #555;
}
.rb-test-step.fail {
    border-left-color: rgba(255, 107, 107, 0.5);
    color: #ff6b6b;
}
.rb-test-step-icon {
    flex-shrink: 0;
    width: 1rem;
    text-align: center;
}
.rb-test-step-msg {
    font-style: italic;
    color: #888;
    font-size: 0.75rem;
    margin-top: 0.1rem;
}
.rb-test-cost-hint {
    font-size: 0.7rem;
    color: #666;
    margin-top: 0.3rem;
    text-align: center;
}
"#;

// ---------------------------------------------------------------------------
// Rule templates
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq, Debug)]
pub enum RuleTemplate {
    CriticalMessages,
    DailyDigest { time: String },
    TrackItems,
    Custom,
}

impl RuleTemplate {
    fn label(&self) -> &'static str {
        match self {
            RuleTemplate::CriticalMessages => "Critical Messages",
            RuleTemplate::DailyDigest { .. } => "Daily Digest",
            RuleTemplate::TrackItems => "Track Items",
            RuleTemplate::Custom => "Something else",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            RuleTemplate::CriticalMessages => "Get notified about urgent or important messages",
            RuleTemplate::DailyDigest { .. } => "Daily summary of messages and emails",
            RuleTemplate::TrackItems => "Auto-track deliveries, invoices, and deadlines",
            RuleTemplate::Custom => "Start blank and tell AI what to do",
        }
    }

    fn example(&self) -> &'static str {
        match self {
            RuleTemplate::CriticalMessages => "e.g., 'Urgent: server is down' -> SMS alert",
            RuleTemplate::DailyDigest { .. } => "e.g., summary of 12 messages -> morning SMS",
            RuleTemplate::TrackItems => "e.g., 'Package shipped' -> tracked on dashboard",
            RuleTemplate::Custom => "",
        }
    }

    fn is_popular(&self) -> bool {
        matches!(self, RuleTemplate::CriticalMessages)
    }

    fn icon(&self) -> &'static str {
        match self {
            RuleTemplate::CriticalMessages => "fa-solid fa-bell",
            RuleTemplate::DailyDigest { .. } => "fa-solid fa-newspaper",
            RuleTemplate::TrackItems => "fa-solid fa-thumbtack",
            RuleTemplate::Custom => "fa-solid fa-wrench",
        }
    }
}

// ---------------------------------------------------------------------------
// Template picker component
// ---------------------------------------------------------------------------

const PICKER_STYLES: &str = r#"
.rule-template-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.7);
    z-index: 1200;
    display: flex;
    justify-content: center;
    align-items: center;
}
.rule-template-modal {
    background: #1a1a1a;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 14px;
    padding: 1.5rem;
    width: 100%;
    max-width: 400px;
    animation: rtmFadeIn 0.2s ease;
}
@keyframes rtmFadeIn {
    from { opacity: 0; transform: scale(0.95); }
    to { opacity: 1; transform: scale(1); }
}
.rule-template-modal h3 {
    color: #fff;
    font-size: 1rem;
    font-weight: 600;
    margin: 0 0 0.25rem;
}
.rule-template-modal .rtm-subtitle {
    color: #888;
    font-size: 0.8rem;
    margin-bottom: 1rem;
}
.rule-template-card {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 10px;
    cursor: pointer;
    transition: all 0.15s;
    margin-bottom: 0.5rem;
}
.rule-template-card:hover {
    background: rgba(126, 178, 255, 0.08);
    border-color: rgba(126, 178, 255, 0.2);
}
.rule-template-card .rtc-icon {
    flex-shrink: 0;
    width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 8px;
    background: rgba(126, 178, 255, 0.1);
    color: #7EB2FF;
    font-size: 0.85rem;
}
.rule-template-card .rtc-body {
    flex: 1;
    min-width: 0;
}
.rule-template-card .rtc-label {
    font-size: 0.85rem;
    font-weight: 600;
    color: #ddd;
}
.rule-template-card .rtc-desc {
    font-size: 0.75rem;
    color: #888;
    margin-top: 0.1rem;
}
.rule-template-card .rtc-arrow {
    color: #555;
    font-size: 0.75rem;
}
.rule-template-card .rtc-example {
    font-size: 0.7rem;
    color: #666;
    margin-top: 0.2rem;
    font-style: italic;
}
.rtc-popular {
    display: inline-block;
    font-size: 0.6rem;
    font-weight: 600;
    color: #7EB2FF;
    background: rgba(126, 178, 255, 0.12);
    border: 1px solid rgba(126, 178, 255, 0.2);
    border-radius: 8px;
    padding: 0.1rem 0.4rem;
    margin-left: 0.4rem;
    vertical-align: middle;
    text-transform: uppercase;
    letter-spacing: 0.03em;
}
@media (prefers-color-scheme: light) {
    .rule-template-modal { background: #fff; border-color: rgba(0,0,0,0.1); }
    .rule-template-modal h3 { color: #222; }
    .rule-template-card { border-color: rgba(0,0,0,0.08); }
    .rule-template-card:hover { background: rgba(126,178,255,0.06); }
    .rule-template-card .rtc-label { color: #333; }
    .rule-template-card .rtc-desc { color: #666; }
}
"#;

#[derive(Properties, PartialEq)]
pub struct TemplatePickerProps {
    pub is_open: bool,
    pub on_close: Callback<()>,
    pub on_select: Callback<RuleTemplate>,
}

#[function_component(RuleTemplatePicker)]
pub fn rule_template_picker(props: &TemplatePickerProps) -> Html {
    let existing_rules = use_state(|| Vec::<RuleData>::new());

    // Fetch existing rules when picker opens
    {
        let existing_rules = existing_rules.clone();
        use_effect_with_deps(
            move |open| {
                if *open {
                    let existing_rules = existing_rules.clone();
                    spawn_local(async move {
                        if let Ok(r) = Api::get("/api/rules").send().await {
                            if let Ok(rules) = r.json::<Vec<RuleData>>().await {
                                existing_rules.set(rules);
                            }
                        }
                    });
                }
                || ()
            },
            props.is_open,
        );
    }

    if !props.is_open {
        return html! {};
    }

    let on_overlay_click = {
        let cb = props.on_close.clone();
        Callback::from(move |e: MouseEvent| {
            if let Some(target) = e.target_dyn_into::<web_sys::Element>() {
                if let Some(class) = target.get_attribute("class") {
                    if class.contains("rule-template-overlay") {
                        cb.emit(());
                    }
                }
            }
        })
    };

    // Check what templates are already covered by existing rules
    let has_critical = existing_rules.iter().any(|r| {
        r.trigger_type == "ontology_change"
            && r.logic_type == "llm"
            && r.action_type == "notify"
            && r.status == "active"
            && r.logic_prompt
                .as_deref()
                .map(|p| {
                    let p = p.to_lowercase();
                    p.contains("important") || p.contains("urgent") || p.contains("critical")
                })
                .unwrap_or(false)
    });
    // For digest, find existing schedule times to suggest alternatives
    let existing_digest_times: Vec<String> = existing_rules
        .iter()
        .filter(|r| {
            r.trigger_type == "schedule"
                && r.logic_type == "llm"
                && r.status == "active"
                && r.logic_prompt
                    .as_deref()
                    .map(|p| {
                        let p = p.to_lowercase();
                        p.contains("summar") || p.contains("digest") || p.contains("review")
                    })
                    .unwrap_or(false)
        })
        .filter_map(|r| {
            // Extract time from trigger_config pattern like "daily 09:00"
            let tc: serde_json::Value = serde_json::from_str(&r.trigger_config).ok()?;
            let pattern = tc.get("pattern")?.as_str()?;
            let parts: Vec<&str> = pattern.split_whitespace().collect();
            parts.get(1).map(|t| t.to_string())
        })
        .collect();

    let has_any_digest = !existing_digest_times.is_empty();

    // Pick a digest time that doesn't conflict
    let digest_time = if existing_digest_times.contains(&"09:00".to_string()) {
        if existing_digest_times.contains(&"18:00".to_string()) {
            if existing_digest_times.contains(&"12:00".to_string()) {
                None // they have morning, evening, and noon - skip
            } else {
                Some("12:00")
            }
        } else {
            Some("18:00")
        }
    } else {
        Some("09:00")
    };
    let digest_label = match digest_time {
        Some("09:00") => "Morning Digest",
        Some("12:00") => "Midday Digest",
        Some("18:00") => "Evening Digest",
        _ => "Daily Digest",
    };
    let digest_desc = match digest_time {
        Some("09:00") => "Daily summary of messages and emails at 9am",
        Some("12:00") => "Midday summary of messages and emails at noon",
        Some("18:00") => "Evening summary of messages and emails at 6pm",
        _ => "Daily summary of messages and emails",
    };

    let mut templates: Vec<(RuleTemplate, String, String)> = Vec::new();
    // CriticalMessages removed - importance notifications are now system-level
    if let Some(time) = digest_time {
        templates.push((
            RuleTemplate::DailyDigest {
                time: time.to_string(),
            },
            digest_label.to_string(),
            digest_desc.to_string(),
        ));
    }
    templates.push((
        RuleTemplate::Custom,
        RuleTemplate::Custom.label().to_string(),
        RuleTemplate::Custom.description().to_string(),
    ));

    html! {
        <>
            <style>{PICKER_STYLES}</style>
            <div class="rule-template-overlay" onclick={on_overlay_click}>
                <div class="rule-template-modal">
                    <h3>{"New Rule"}</h3>
                    <div class="rtm-subtitle">{"What kind of rule?"}</div>
                    { for templates.iter().map(|(tmpl, label, desc)| {
                        let tmpl_clone = tmpl.clone();
                        let on_select = props.on_select.clone();
                        let example = tmpl.example();
                        let is_popular = tmpl.is_popular();
                        html! {
                            <div class="rule-template-card" onclick={Callback::from(move |_: MouseEvent| {
                                on_select.emit(tmpl_clone.clone());
                            })}>
                                <div class="rtc-icon">
                                    <i class={tmpl.icon()}></i>
                                </div>
                                <div class="rtc-body" style="position: relative;">
                                    <div class="rtc-label">
                                        {label.clone()}
                                        if is_popular {
                                            <span class="rtc-popular">{"Popular"}</span>
                                        }
                                    </div>
                                    <div class="rtc-desc">{desc.clone()}</div>
                                    if !example.is_empty() {
                                        <div class="rtc-example">{example}</div>
                                    }
                                </div>
                                <div class="rtc-arrow">
                                    <i class="fa-solid fa-chevron-right"></i>
                                </div>
                            </div>
                        }
                    })}
                </div>
            </div>
        </>
    }
}

// ---------------------------------------------------------------------------
// Rule builder props
// ---------------------------------------------------------------------------

#[derive(Properties, PartialEq)]
pub struct RuleBuilderProps {
    pub is_open: bool,
    pub on_close: Callback<()>,
    pub on_saved: Callback<()>,
    pub editing_rule: Option<RuleData>,
    #[prop_or_default]
    pub initial_template: Option<RuleTemplate>,
    #[prop_or_default]
    pub plan_type: Option<String>,
}

#[derive(Clone, PartialEq)]
enum WhenMode {
    Schedule,
    Event,
}

#[derive(Clone, PartialEq)]
enum ScheduleMode {
    Once,
    Recurring,
}

#[derive(Clone, PartialEq)]
enum RecurringFreq {
    Daily,
    Weekdays,
    Weekly,
    Hourly,
}

#[derive(Clone, PartialEq)]
enum LogicMode {
    Always,
    Keyword,
    Llm,
}

#[derive(Clone, PartialEq)]
enum PromptTemplate {
    Summarize,
    FilterImportant,
    CheckCondition,
    TrackItemsUpdate,
    TrackItemsCreate,
    Custom,
}

#[derive(Clone, PartialEq)]
enum ActionMode {
    Notify,
    ToolCall,
}

#[derive(Clone, PartialEq)]
enum NotifyMethod {
    Sms,
    Call,
}

#[derive(Clone, PartialEq, Copy)]
enum Card {
    When,
    If,
    Then,
}

#[function_component(RuleBuilder)]
pub fn rule_builder(props: &RuleBuilderProps) -> Html {
    let is_editing = props.editing_rule.is_some();
    let editing_rule_id = props.editing_rule.as_ref().map(|r| r.id);
    let is_autopilot = props.plan_type.as_deref() == Some("autopilot");

    // Track whether user has actually interacted with the form (for edit mode close confirmation)
    let user_touched_form = use_state(|| false);

    // Form state
    let name = use_state(|| String::new());
    let expanded_card = use_state(|| None::<Card>);

    // WHEN state
    let when_mode = use_state(|| WhenMode::Schedule);
    let schedule_mode = use_state(|| ScheduleMode::Recurring);
    let once_date = use_state(|| String::new());
    let once_time = use_state(|| String::new());
    let recurring_freq = use_state(|| RecurringFreq::Daily);
    let recurring_time = use_state(|| "09:00".to_string());
    let recurring_day = use_state(|| "monday".to_string());
    let event_entity = use_state(|| "Message".to_string());
    let event_change = use_state(|| "created".to_string());
    let event_filter_key = use_state(|| "sender".to_string());
    let event_filter_value = use_state(|| String::new());
    let event_fire_once = use_state(|| true); // default: one-shot
    let event_delay = use_state(|| 600i32); // default: 10 min delay before rule fires
                                            // (display_name, platform, is_group, group_mode)
                                            // group_mode: None for non-groups, Some("all") or Some("mention_only") for groups
    let sender_suggestions =
        use_state(|| Vec::<(String, Option<String>, bool, Option<String>)>::new());
    let sender_dropdown_open = use_state(|| false);
    let selected_group_mode = use_state(|| None::<String>); // None = not a group, Some("all") or Some("mention_only")

    // IF state
    let logic_mode = use_state(|| LogicMode::Always);
    let logic_prompt = use_state(|| String::new());
    let available_sources = use_state(|| Vec::<RuleSourceOption>::new());
    let active_sources = use_state(|| Vec::<SourceConfig>::new());
    let expanded_source = use_state(|| None::<String>);
    let mcp_source_tools = use_state(|| HashMap::<String, Vec<McpToolOption>>::new());
    let selected_template = use_state(|| PromptTemplate::Summarize);
    let condition_input = use_state(|| String::new());
    let keyword_input = use_state(|| String::new());

    // THEN state
    let action_mode = use_state(|| ActionMode::Notify);
    let notify_method = use_state(|| NotifyMethod::Sms);
    let notify_message = use_state(|| String::new());
    let tool_name = use_state(|| "send_chat_message".to_string());
    // Per-tool params
    let tc_platform = use_state(|| "whatsapp".to_string());
    let tc_chat_name = use_state(|| String::new());
    let tc_chat_room_id = use_state(|| None::<String>);
    let tc_chat_search_results = use_state(|| Vec::<(String, String, Option<String>)>::new()); // (display_name, room_id, person_name)
    let tc_chat_search_open = use_state(|| false);
    let tc_chat_searching = use_state(|| false);
    let tc_message = use_state(|| String::new());
    let tc_email_to = use_state(|| String::new());
    let tc_email_subject = use_state(|| String::new());
    let tc_email_body = use_state(|| String::new());
    let tc_reply_text = use_state(|| String::new());
    let tc_tesla_cmd = use_state(|| "lock".to_string());
    // MCP tool params (generic key-value map)
    let tc_mcp_params = use_state(|| HashMap::<String, String>::new());
    // MCP tools fetched from servers: (value, display, schema_fields)
    // schema_fields = Vec of required param names from input_schema
    let mcp_tools = use_state(|| Vec::<(String, String, Vec<String>)>::new());

    // ELSE branch state (nested conditions)
    let else_flow = use_state(|| None::<FlowNode>);

    let saving = use_state(|| false);
    let error_msg = use_state(|| None::<String>);

    // Test panel state
    let test_open = use_state(|| false);
    let test_message = use_state(|| String::new());
    let test_sender = use_state(|| "Test Sender".to_string());
    let test_running = use_state(|| false);
    let test_steps = use_state(|| Vec::<(String, String, String)>::new()); // (css_class, icon, text)
    let test_es_ref = use_mut_ref(|| None::<web_sys::EventSource>);

    // Fetch senders for autocomplete (persons + chat room names + group chats)
    {
        let sender_suggestions = sender_suggestions.clone();
        use_effect_with_deps(
            move |open| {
                if *open {
                    let sender_suggestions = sender_suggestions.clone();
                    spawn_local(async move {
                        if let Ok(r) = Api::get("/api/dashboard/senders").send().await {
                            if let Ok(senders) = r.json::<Vec<serde_json::Value>>().await {
                                let mut suggestions: Vec<(
                                    String,
                                    Option<String>,
                                    bool,
                                    Option<String>,
                                )> = Vec::new();
                                for s in &senders {
                                    let name = match s.get("name").and_then(|n| n.as_str()) {
                                        Some(n) => n.to_string(),
                                        None => continue,
                                    };
                                    let platform = s
                                        .get("platform")
                                        .and_then(|p| p.as_str())
                                        .map(|p| p.to_string());
                                    let is_group = s
                                        .get("is_group")
                                        .and_then(|g| g.as_bool())
                                        .unwrap_or(false);
                                    if is_group {
                                        // Add two entries for groups: (all) and (mention only)
                                        suggestions.push((
                                            name.clone(),
                                            platform.clone(),
                                            true,
                                            Some("all".to_string()),
                                        ));
                                        suggestions.push((
                                            name,
                                            platform,
                                            true,
                                            Some("mention_only".to_string()),
                                        ));
                                    } else {
                                        suggestions.push((name, platform, false, None));
                                    }
                                }
                                sender_suggestions.set(suggestions);
                            }
                        }
                    });
                }
                || ()
            },
            props.is_open,
        );
    }

    // Fetch available rule sources
    {
        let available_sources = available_sources.clone();
        use_effect_with_deps(
            move |open| {
                if *open {
                    let available_sources = available_sources.clone();
                    spawn_local(async move {
                        if let Ok(r) = Api::get("/api/dashboard/rule-sources").send().await {
                            if let Ok(sources) = r.json::<Vec<RuleSourceOption>>().await {
                                available_sources.set(sources);
                            }
                        }
                    });
                }
                || ()
            },
            props.is_open,
        );
    }

    // Fetch MCP tools from enabled servers
    {
        let mcp_tools = mcp_tools.clone();
        use_effect_with_deps(
            move |open| {
                if *open {
                    let mcp_tools = mcp_tools.clone();
                    spawn_local(async move {
                        // Get all MCP servers
                        let servers: Vec<serde_json::Value> =
                            match Api::get("/api/mcp/servers").send().await {
                                Ok(r) if r.ok() => r.json().await.unwrap_or_default(),
                                _ => vec![],
                            };
                        let mut all_tools = Vec::new();
                        for srv in &servers {
                            let enabled = srv
                                .get("is_enabled")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            if !enabled {
                                continue;
                            }
                            let srv_id = match srv.get("id").and_then(|v| v.as_i64()) {
                                Some(id) => id,
                                None => continue,
                            };
                            let srv_name = srv
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                            let url = format!("/api/mcp/servers/{}/tools", srv_id);
                            if let Ok(r) = Api::get(&url).send().await {
                                if let Ok(resp) = r.json::<serde_json::Value>().await {
                                    if let Some(tools) =
                                        resp.get("tools").and_then(|v| v.as_array())
                                    {
                                        for tool in tools {
                                            let tool_name = tool
                                                .get("name")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("");
                                            if tool_name.is_empty() {
                                                continue;
                                            }
                                            let value = format!("mcp:{}:{}", srv_name, tool_name);
                                            let display =
                                                format!("MCP: {} - {}", srv_name, tool_name);
                                            // Extract required fields from input_schema
                                            let fields =
                                                extract_schema_fields(tool.get("input_schema"));
                                            all_tools.push((value, display, fields));
                                        }
                                    }
                                }
                            }
                        }
                        mcp_tools.set(all_tools);
                    });
                }
                || ()
            },
            props.is_open,
        );
    }

    // Initialize from editing_rule
    {
        let editing = props.editing_rule.clone();
        let name = name.clone();
        let when_mode = when_mode.clone();
        let schedule_mode = schedule_mode.clone();
        let once_date = once_date.clone();
        let once_time = once_time.clone();
        let recurring_freq = recurring_freq.clone();
        let recurring_time = recurring_time.clone();
        let recurring_day = recurring_day.clone();
        let event_entity = event_entity.clone();
        let event_change = event_change.clone();
        let event_filter_key = event_filter_key.clone();
        let event_filter_value = event_filter_value.clone();
        let event_fire_once = event_fire_once.clone();
        let event_delay = event_delay.clone();
        let logic_mode = logic_mode.clone();
        let logic_prompt = logic_prompt.clone();
        let active_sources_init = active_sources.clone();
        let action_mode = action_mode.clone();
        let notify_method = notify_method.clone();
        let notify_message = notify_message.clone();
        let tool_name = tool_name.clone();
        let tc_platform = tc_platform.clone();
        let tc_chat_name = tc_chat_name.clone();
        let tc_chat_room_id = tc_chat_room_id.clone();
        let tc_message = tc_message.clone();
        let tc_email_to = tc_email_to.clone();
        let tc_email_subject = tc_email_subject.clone();
        let tc_email_body = tc_email_body.clone();
        let tc_reply_text = tc_reply_text.clone();
        let tc_tesla_cmd = tc_tesla_cmd.clone();
        let tc_mcp_params = tc_mcp_params.clone();
        let expanded_card = expanded_card.clone();
        let error_msg_init = error_msg.clone();
        let selected_template = selected_template.clone();
        let condition_input = condition_input.clone();
        let keyword_input = keyword_input.clone();
        let else_flow_init = else_flow.clone();
        let user_touched_form = user_touched_form.clone();
        let selected_group_mode = selected_group_mode.clone();

        use_effect_with_deps(
            move |editing: &Option<RuleData>| {
                user_touched_form.set(false);
                if let Some(rule) = editing {
                    name.set(rule.name.clone());
                    expanded_card.set(None); // all collapsed in view mode

                    // Parse else branch from flow_config if present
                    if let Some(ref fc) = rule.flow_config {
                        if let Ok(node) = serde_json::from_str::<FlowNode>(fc) {
                            match &node {
                                FlowNode::LlmCondition { false_branch, .. }
                                | FlowNode::KeywordCondition { false_branch, .. } => {
                                    else_flow_init.set(false_branch.as_ref().clone());
                                }
                                _ => {
                                    else_flow_init.set(None);
                                }
                            }
                        } else {
                            else_flow_init.set(None);
                        }
                    } else {
                        else_flow_init.set(None);
                    }

                    // Parse trigger
                    if rule.trigger_type == "schedule" {
                        when_mode.set(WhenMode::Schedule);
                        let tc: serde_json::Value =
                            serde_json::from_str(&rule.trigger_config).unwrap_or_default();
                        match tc.get("schedule").and_then(|v| v.as_str()) {
                            Some("once") => {
                                schedule_mode.set(ScheduleMode::Once);
                                if let Some(at) = tc.get("at").and_then(|v| v.as_str()) {
                                    if at.len() >= 16 {
                                        once_date.set(at[..10].to_string());
                                        once_time.set(at[11..16].to_string());
                                    }
                                }
                            }
                            Some("recurring") => {
                                schedule_mode.set(ScheduleMode::Recurring);
                                if let Some(pattern) = tc.get("pattern").and_then(|v| v.as_str()) {
                                    parse_pattern_into(
                                        pattern,
                                        &recurring_freq,
                                        &recurring_time,
                                        &recurring_day,
                                    );
                                }
                            }
                            _ => {}
                        }
                    } else {
                        when_mode.set(WhenMode::Event);
                        let tc: serde_json::Value =
                            serde_json::from_str(&rule.trigger_config).unwrap_or_default();
                        if let Some(et) = tc.get("entity_type").and_then(|v| v.as_str()) {
                            event_entity.set(et.to_string());
                        }
                        if let Some(ch) = tc.get("change").and_then(|v| v.as_str()) {
                            event_change.set(ch.to_string());
                        }
                        if let Some(filters) = tc.get("filters").and_then(|v| v.as_object()) {
                            if let Some((k, v)) = filters.iter().next() {
                                event_filter_key.set(k.clone());
                                event_filter_value.set(v.as_str().unwrap_or("").to_string());
                            }
                        }
                        // fire_once defaults to true if not explicitly false
                        let fo = tc
                            .get("fire_once")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true);
                        event_fire_once.set(fo);
                        // delay_seconds defaults to 600 if not set
                        let ds = tc
                            .get("delay_seconds")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(600) as i32;
                        event_delay.set(ds);
                        // Restore group_mode if present
                        if let Some(gm) = tc.get("group_mode").and_then(|v| v.as_str()) {
                            selected_group_mode.set(Some(gm.to_string()));
                        } else {
                            selected_group_mode.set(None);
                        }
                    }

                    // Parse logic
                    if rule.logic_type == "llm" {
                        logic_mode.set(LogicMode::Llm);
                        // Check flow_config prompt for template IDs
                        let fc_prompt = rule
                            .flow_config
                            .as_ref()
                            .and_then(|fc| serde_json::from_str::<serde_json::Value>(fc).ok())
                            .and_then(|v| {
                                v.get("prompt")
                                    .and_then(|p| p.as_str())
                                    .map(|s| s.to_string())
                            })
                            .unwrap_or_default();
                        if fc_prompt.starts_with("template:") {
                            match fc_prompt.as_str() {
                                "template:summarize" => {
                                    selected_template.set(PromptTemplate::Summarize);
                                    logic_prompt.set(String::new());
                                    condition_input.set(String::new());
                                }
                                "template:filter_important" => {
                                    selected_template.set(PromptTemplate::FilterImportant);
                                    logic_prompt.set(String::new());
                                    condition_input.set(String::new());
                                }
                                "template:track_items_update" => {
                                    selected_template.set(PromptTemplate::TrackItemsUpdate);
                                    logic_prompt.set(String::new());
                                    condition_input.set(String::new());
                                }
                                "template:track_items_create" => {
                                    selected_template.set(PromptTemplate::TrackItemsCreate);
                                    logic_prompt.set(String::new());
                                    condition_input.set(String::new());
                                }
                                s if s.starts_with("template:check_condition:") => {
                                    selected_template.set(PromptTemplate::CheckCondition);
                                    let cond =
                                        s.strip_prefix("template:check_condition:").unwrap_or("");
                                    condition_input.set(cond.to_string());
                                    logic_prompt.set(String::new());
                                }
                                _ => {
                                    selected_template.set(PromptTemplate::Custom);
                                    logic_prompt.set(fc_prompt);
                                    condition_input.set(String::new());
                                }
                            }
                        } else {
                            selected_template.set(PromptTemplate::Custom);
                            logic_prompt
                                .set(rule.logic_prompt.clone().unwrap_or_else(|| fc_prompt));
                            condition_input.set(String::new());
                        }
                        if let Some(ref fetch_raw) = rule.logic_fetch {
                            let trimmed = fetch_raw.trim();
                            if trimmed.starts_with('[') {
                                // JSON array format
                                if let Ok(sources) =
                                    serde_json::from_str::<Vec<SourceConfig>>(trimmed)
                                {
                                    active_sources_init.set(sources);
                                }
                            } else {
                                // Legacy comma-separated fallback
                                let mut sources = Vec::new();
                                for s in trimmed.split(',').map(|s| s.trim()) {
                                    match s {
                                        "email" => sources.push(SourceConfig::Email),
                                        "chat" => sources.push(SourceConfig::Chat {
                                            platform: "all".to_string(),
                                            limit: 50,
                                        }),
                                        _ => {}
                                    }
                                }
                                active_sources_init.set(sources);
                            }
                        }
                    } else if rule.logic_type == "keyword" {
                        logic_mode.set(LogicMode::Keyword);
                        keyword_input.set(rule.logic_prompt.clone().unwrap_or_default());
                    } else {
                        logic_mode.set(LogicMode::Always);
                    }

                    // Parse action
                    let ac: serde_json::Value =
                        serde_json::from_str(&rule.action_config).unwrap_or_default();
                    if rule.action_type == "notify" {
                        action_mode.set(ActionMode::Notify);
                        match ac.get("method").and_then(|v| v.as_str()) {
                            Some("call") => notify_method.set(NotifyMethod::Call),
                            _ => notify_method.set(NotifyMethod::Sms),
                        }
                        if let Some(msg) = ac.get("message").and_then(|v| v.as_str()) {
                            notify_message.set(msg.to_string());
                        }
                    } else {
                        action_mode.set(ActionMode::ToolCall);
                        if let Some(t) = ac.get("tool").and_then(|v| v.as_str()) {
                            tool_name.set(t.to_string());
                            // Populate per-tool fields from params
                            if let Some(p) = ac.get("params") {
                                match t {
                                    "send_chat_message" => {
                                        if let Some(v) = p.get("platform").and_then(|v| v.as_str())
                                        {
                                            tc_platform.set(v.to_string());
                                        }
                                        if let Some(v) = p.get("chat_name").and_then(|v| v.as_str())
                                        {
                                            tc_chat_name.set(v.to_string());
                                        }
                                        if let Some(v) = p.get("room_id").and_then(|v| v.as_str()) {
                                            tc_chat_room_id.set(Some(v.to_string()));
                                        }
                                        if let Some(v) = p.get("message").and_then(|v| v.as_str()) {
                                            tc_message.set(v.to_string());
                                        }
                                    }
                                    "send_email" => {
                                        if let Some(v) = p.get("to").and_then(|v| v.as_str()) {
                                            tc_email_to.set(v.to_string());
                                        }
                                        if let Some(v) = p.get("subject").and_then(|v| v.as_str()) {
                                            tc_email_subject.set(v.to_string());
                                        }
                                        if let Some(v) = p.get("body").and_then(|v| v.as_str()) {
                                            tc_email_body.set(v.to_string());
                                        }
                                    }
                                    "respond_to_email" => {
                                        if let Some(v) =
                                            p.get("response_text").and_then(|v| v.as_str())
                                        {
                                            tc_reply_text.set(v.to_string());
                                        }
                                    }
                                    "control_tesla" => {
                                        if let Some(v) = p.get("command").and_then(|v| v.as_str()) {
                                            tc_tesla_cmd.set(v.to_string());
                                        }
                                    }
                                    _ if t.starts_with("mcp:") => {
                                        // Populate MCP params from JSON object
                                        if let Some(obj) = p.as_object() {
                                            let map: HashMap<String, String> = obj
                                                .iter()
                                                .filter_map(|(k, v)| {
                                                    v.as_str().map(|s| (k.clone(), s.to_string()))
                                                })
                                                .collect();
                                            tc_mcp_params.set(map);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                } else {
                    // Reset for create mode - expand all cards
                    name.set(String::new());
                    expanded_card.set(Some(Card::When));
                    when_mode.set(WhenMode::Schedule);
                    schedule_mode.set(ScheduleMode::Recurring);
                    recurring_freq.set(RecurringFreq::Daily);
                    recurring_time.set("09:00".to_string());
                    logic_mode.set(LogicMode::Always);
                    action_mode.set(ActionMode::Notify);
                    notify_method.set(NotifyMethod::Sms);
                    notify_message.set(String::new());
                    logic_prompt.set(String::new());
                    active_sources_init.set(Vec::new());
                    selected_template.set(PromptTemplate::Summarize);
                    condition_input.set(String::new());
                    keyword_input.set(String::new());
                    once_date.set(String::new());
                    once_time.set(String::new());
                    event_filter_value.set(String::new());
                    tool_name.set("send_chat_message".to_string());
                    tc_platform.set("whatsapp".to_string());
                    tc_chat_name.set(String::new());
                    tc_chat_room_id.set(None);
                    tc_message.set(String::new());
                    tc_email_to.set(String::new());
                    tc_email_subject.set(String::new());
                    tc_email_body.set(String::new());
                    tc_reply_text.set(String::new());
                    tc_tesla_cmd.set("lock".to_string());
                    tc_mcp_params.set(HashMap::new());
                    else_flow_init.set(None);
                    error_msg_init.set(None);
                }
                || ()
            },
            editing,
        );
    }

    // Initialize from template
    {
        let template = props.initial_template.clone();
        let is_editing = props.editing_rule.is_some();
        let name = name.clone();
        let when_mode = when_mode.clone();
        let schedule_mode = schedule_mode.clone();
        let recurring_freq = recurring_freq.clone();
        let recurring_time = recurring_time.clone();
        let event_entity = event_entity.clone();
        let event_change = event_change.clone();
        let event_fire_once = event_fire_once.clone();
        let event_filter_key = event_filter_key.clone();
        let logic_mode = logic_mode.clone();
        let logic_prompt = logic_prompt.clone();
        let active_sources_tmpl = active_sources.clone();
        let selected_template = selected_template.clone();
        let action_mode = action_mode.clone();
        let notify_method = notify_method.clone();
        let tool_name = tool_name.clone();
        let expanded_card = expanded_card.clone();
        let else_flow = else_flow.clone();
        let event_delay = event_delay.clone();

        use_effect_with_deps(
            move |tmpl: &Option<RuleTemplate>| {
                if !is_editing {
                    if let Some(template) = tmpl {
                        match template {
                            RuleTemplate::CriticalMessages => {
                                name.set("Critical messages".to_string());
                                when_mode.set(WhenMode::Event);
                                event_entity.set("Message".to_string());
                                event_change.set("created".to_string());
                                event_fire_once.set(false);
                                event_filter_key.set("none".to_string());
                                logic_mode.set(LogicMode::Llm);
                                selected_template.set(PromptTemplate::FilterImportant);
                                logic_prompt.set(String::new());
                                active_sources_tmpl.set(vec![]);
                                action_mode.set(ActionMode::Notify);
                                notify_method.set(NotifyMethod::Sms);
                                else_flow.set(None);
                                expanded_card.set(Some(Card::If));
                            }
                            RuleTemplate::DailyDigest { ref time } => {
                                let label = match time.as_str() {
                                    "18:00" => "Evening digest",
                                    "12:00" => "Midday digest",
                                    _ => "Morning digest",
                                };
                                name.set(label.to_string());
                                when_mode.set(WhenMode::Schedule);
                                schedule_mode.set(ScheduleMode::Recurring);
                                recurring_freq.set(RecurringFreq::Daily);
                                recurring_time.set(time.clone());
                                logic_mode.set(LogicMode::Llm);
                                selected_template.set(PromptTemplate::Summarize);
                                logic_prompt.set(String::new());
                                active_sources_tmpl.set(vec![
                                    SourceConfig::Email,
                                    SourceConfig::Chat {
                                        platform: "all".to_string(),
                                        limit: 50,
                                    },
                                    SourceConfig::Events,
                                ]);
                                action_mode.set(ActionMode::Notify);
                                notify_method.set(NotifyMethod::Sms);
                                else_flow.set(None);
                                expanded_card.set(Some(Card::When));
                            }
                            RuleTemplate::TrackItems => {
                                name.set("Track items".to_string());
                                when_mode.set(WhenMode::Event);
                                event_entity.set("Message".to_string());
                                event_change.set("created".to_string());
                                event_fire_once.set(false);
                                event_delay.set(0);
                                event_filter_key.set("none".to_string());
                                logic_mode.set(LogicMode::Llm);
                                selected_template.set(PromptTemplate::TrackItemsUpdate);
                                logic_prompt.set(String::new());
                                active_sources_tmpl.set(vec![SourceConfig::Events]);
                                action_mode.set(ActionMode::ToolCall);
                                tool_name.set("update_event".to_string());
                                // ELSE branch: nested condition to create new tracked obligations
                                else_flow.set(Some(FlowNode::LlmCondition {
                                    prompt: "template:track_items_create".to_string(),
                                    fetch: vec![],
                                    true_branch: Box::new(Some(FlowNode::Action {
                                        action_type: "tool_call".to_string(),
                                        config: serde_json::json!({"tool": "create_event"}),
                                    })),
                                    false_branch: Box::new(None),
                                }));
                                expanded_card.set(Some(Card::If));
                            }
                            RuleTemplate::Custom => {
                                else_flow.set(None);
                                expanded_card.set(Some(Card::When));
                            }
                        }
                    }
                }
                || ()
            },
            template,
        );
    }

    // Summary generators
    let when_summary = {
        let when_mode = (*when_mode).clone();
        let schedule_mode = (*schedule_mode).clone();
        let once_date = (*once_date).clone();
        let once_time = (*once_time).clone();
        let recurring_freq = (*recurring_freq).clone();
        let recurring_time = (*recurring_time).clone();
        let recurring_day = (*recurring_day).clone();
        let event_entity = (*event_entity).clone();
        let event_filter_key = (*event_filter_key).clone();
        let event_filter_value = (*event_filter_value).clone();
        match when_mode {
            WhenMode::Schedule => match schedule_mode {
                ScheduleMode::Once => {
                    if !once_date.is_empty() && !once_time.is_empty() {
                        let at = format!("{}T{}", once_date, once_time);
                        format!("once {}", format_datetime_short_local(&at))
                    } else {
                        "once (set date/time)".to_string()
                    }
                }
                ScheduleMode::Recurring => match recurring_freq {
                    RecurringFreq::Hourly => "every hour".to_string(),
                    RecurringFreq::Daily => {
                        format!("daily at {}", format_time_display(&recurring_time))
                    }
                    RecurringFreq::Weekdays => {
                        format!("weekdays at {}", format_time_display(&recurring_time))
                    }
                    RecurringFreq::Weekly => format!(
                        "{}s at {}",
                        capitalize_first(&recurring_day),
                        format_time_display(&recurring_time)
                    ),
                },
            },
            WhenMode::Event => {
                if !event_filter_value.is_empty() {
                    match event_filter_key.as_str() {
                        "sender" => format!("when message from {}", event_filter_value),
                        "content" => format!("when message contains '{}'", event_filter_value),
                        _ => "when message received".to_string(),
                    }
                } else {
                    "when message received".to_string()
                }
            }
        }
    };

    let if_summary = match *logic_mode {
        LogicMode::Always => "always run".to_string(),
        LogicMode::Keyword => {
            let k = (*keyword_input).clone();
            if k.is_empty() {
                "keyword match".to_string()
            } else if k.len() > 20 {
                format!("contains '{}...'", &k[..20])
            } else {
                format!("contains '{}'", k)
            }
        }
        LogicMode::Llm => match *selected_template {
            PromptTemplate::Summarize => "AI summarizes updates".to_string(),
            PromptTemplate::FilterImportant => "AI filters important".to_string(),
            PromptTemplate::TrackItemsUpdate => "AI tracks item updates".to_string(),
            PromptTemplate::TrackItemsCreate => "AI creates tracked items".to_string(),
            PromptTemplate::CheckCondition => {
                let c = (*condition_input).clone();
                if c.is_empty() {
                    "AI checks condition".to_string()
                } else if c.len() > 25 {
                    format!("AI: if {}...", &c[..25])
                } else {
                    format!("AI: if {}", c)
                }
            }
            PromptTemplate::Custom => {
                let p = (*logic_prompt).clone();
                if p.is_empty() {
                    "AI evaluates".to_string()
                } else if p.len() > 30 {
                    format!("AI: {}...", &p[..30])
                } else {
                    format!("AI: {}", p)
                }
            }
        },
    };

    let then_summary = match *action_mode {
        ActionMode::Notify => match *notify_method {
            NotifyMethod::Sms => "notify via SMS".to_string(),
            NotifyMethod::Call => "notify via call".to_string(),
        },
        ActionMode::ToolCall => {
            let t = (*tool_name).clone();
            match t.as_str() {
                "send_chat_message" => {
                    let cn = (*tc_chat_name).clone();
                    let plat = capitalize_first(&*tc_platform);
                    if cn.is_empty() {
                        format!("{} message", plat)
                    } else {
                        format!("{} msg {}", plat, cn)
                    }
                }
                "send_email" => "send email".to_string(),
                "respond_to_email" => "reply to email".to_string(),
                "control_tesla" => humanize_tesla_cmd_short(&*tc_tesla_cmd).to_string(),
                "create_event" => "create event".to_string(),
                "update_event" => "update event".to_string(),
                _ if t.starts_with("mcp:") => {
                    // "mcp:server:tool" -> "MCP: tool"
                    let parts: Vec<&str> = t.splitn(3, ':').collect();
                    let tool_short = parts.get(2).unwrap_or(&"tool");
                    format!("MCP: {}", tool_short)
                }
                _ => {
                    if t.is_empty() {
                        "run tool".to_string()
                    } else {
                        format!("run: {}", t)
                    }
                }
            }
        }
    };

    // Card toggle
    let toggle_card = {
        let expanded_card = expanded_card.clone();
        let user_touched_form = user_touched_form.clone();
        move |card: Card| {
            let expanded_card = expanded_card.clone();
            let user_touched_form = user_touched_form.clone();
            Callback::from(move |_: MouseEvent| {
                user_touched_form.set(true);
                if *expanded_card == Some(card) {
                    expanded_card.set(None);
                } else {
                    expanded_card.set(Some(card));
                }
            })
        }
    };

    // Compute whether form has user work that would be lost on close
    let has_unsaved_work = {
        let has_typed_text = !name.is_empty()
            || !logic_prompt.is_empty()
            || !keyword_input.is_empty()
            || !condition_input.is_empty()
            || !notify_message.is_empty()
            || !tc_message.is_empty()
            || !tc_chat_name.is_empty()
            || !tc_email_to.is_empty()
            || !tc_email_subject.is_empty()
            || !tc_email_body.is_empty()
            || !tc_reply_text.is_empty()
            || !active_sources.is_empty()
            || else_flow.is_some();
        if is_editing {
            *user_touched_form
        } else {
            has_typed_text
        }
    };

    let on_close_click = {
        let cb = props.on_close.clone();
        let has_work = has_unsaved_work;
        Callback::from(move |_: MouseEvent| {
            if has_work {
                let window = web_sys::window().unwrap();
                if !window
                    .confirm_with_message("You have unsaved changes. Discard?")
                    .unwrap_or(true)
                {
                    return;
                }
            }
            cb.emit(());
        })
    };

    // Use mousedown so text-selection drags that end over the overlay don't trigger close
    let on_overlay_mousedown = {
        let cb = props.on_close.clone();
        let has_work = has_unsaved_work;
        Callback::from(move |e: MouseEvent| {
            if let Some(target) = e.target_dyn_into::<web_sys::Element>() {
                if let Some(class) = target.get_attribute("class") {
                    if class.contains("rule-builder-overlay") {
                        if has_work {
                            let window = web_sys::window().unwrap();
                            if !window
                                .confirm_with_message("You have unsaved changes. Discard?")
                                .unwrap_or(true)
                            {
                                return;
                            }
                        }
                        cb.emit(());
                    }
                }
            }
        })
    };

    // Submit handler
    let on_submit = {
        let name = name.clone();
        let when_mode = when_mode.clone();
        let schedule_mode = schedule_mode.clone();
        let once_date = once_date.clone();
        let once_time = once_time.clone();
        let recurring_freq = recurring_freq.clone();
        let recurring_time = recurring_time.clone();
        let recurring_day = recurring_day.clone();
        let event_entity = event_entity.clone();
        let event_change = event_change.clone();
        let event_filter_key = event_filter_key.clone();
        let event_filter_value = event_filter_value.clone();
        let event_fire_once = event_fire_once.clone();
        let event_delay = event_delay.clone();
        let logic_mode = logic_mode.clone();
        let logic_prompt = logic_prompt.clone();
        let active_sources_submit = active_sources.clone();
        let selected_template = selected_template.clone();
        let condition_input = condition_input.clone();
        let keyword_input = keyword_input.clone();
        let action_mode = action_mode.clone();
        let notify_method = notify_method.clone();
        let notify_message = notify_message.clone();
        let tool_name = tool_name.clone();
        let tc_platform = tc_platform.clone();
        let tc_chat_name = tc_chat_name.clone();
        let tc_chat_room_id = tc_chat_room_id.clone();
        let tc_message = tc_message.clone();
        let tc_email_to = tc_email_to.clone();
        let tc_email_subject = tc_email_subject.clone();
        let tc_email_body = tc_email_body.clone();
        let tc_reply_text = tc_reply_text.clone();
        let tc_tesla_cmd = tc_tesla_cmd.clone();
        let tc_mcp_params = tc_mcp_params.clone();
        let else_flow_submit = else_flow.clone();
        let selected_group_mode = selected_group_mode.clone();
        let saving = saving.clone();
        let error_msg = error_msg.clone();
        let on_saved = props.on_saved.clone();

        Callback::from(move |_: MouseEvent| {
            // Build trigger first (name may be auto-generated from it)
            let (trigger_type, trigger_config) = match *when_mode {
                WhenMode::Schedule => {
                    let config = match *schedule_mode {
                        ScheduleMode::Once => {
                            let at = format!("{}T{}", *once_date, *once_time);
                            serde_json::json!({ "schedule": "once", "at": at })
                        }
                        ScheduleMode::Recurring => {
                            let pattern = match *recurring_freq {
                                RecurringFreq::Hourly => "hourly".to_string(),
                                RecurringFreq::Daily => format!("daily {}", *recurring_time),
                                RecurringFreq::Weekdays => {
                                    format!("weekdays {}", *recurring_time)
                                }
                                RecurringFreq::Weekly => {
                                    format!("weekly {} {}", *recurring_day, *recurring_time)
                                }
                            };
                            serde_json::json!({ "schedule": "recurring", "pattern": pattern })
                        }
                    };
                    ("schedule".to_string(), config.to_string())
                }
                WhenMode::Event => {
                    let mut config = serde_json::json!({
                        "entity_type": *event_entity,
                        "change": *event_change,
                        "fire_once": *event_fire_once,
                        "delay_seconds": *event_delay,
                    });
                    let fv = (*event_filter_value).clone();
                    let fk = (*event_filter_key).clone();
                    if fk != "none" && !fv.is_empty() {
                        config["filters"] = serde_json::json!({ fk: fv });
                    }
                    // Include group_mode if a group chat sender is selected
                    if let Some(ref gm) = *selected_group_mode {
                        config["group_mode"] = serde_json::json!(gm);
                    }
                    ("ontology_change".to_string(), config.to_string())
                }
            };

            // Build logic
            let logic_type = match *logic_mode {
                LogicMode::Always => "passthrough",
                LogicMode::Keyword => "keyword",
                LogicMode::Llm => "llm",
            };
            let lp = match *logic_mode {
                LogicMode::Keyword => {
                    let k = (*keyword_input).clone();
                    if k.is_empty() {
                        None
                    } else {
                        Some(k)
                    }
                }
                LogicMode::Llm => match *selected_template {
                    PromptTemplate::Custom => {
                        let v = (*logic_prompt).clone();
                        if v.is_empty() {
                            None
                        } else {
                            Some(v)
                        }
                    }
                    PromptTemplate::CheckCondition => {
                        let ci = (*condition_input).clone();
                        if ci.is_empty() {
                            None
                        } else {
                            Some(format!("template:check_condition:{}", ci))
                        }
                    }
                    PromptTemplate::Summarize => Some("template:summarize".to_string()),
                    PromptTemplate::FilterImportant => {
                        Some("template:filter_important".to_string())
                    }
                    PromptTemplate::TrackItemsUpdate => Some("template:track_items_update".to_string()),
                    PromptTemplate::TrackItemsCreate => Some("template:track_items_create".to_string()),
                },
                _ => None,
            };
            let lf = if *logic_mode == LogicMode::Llm && !active_sources_submit.is_empty() {
                Some(serde_json::to_string(&*active_sources_submit).unwrap_or_default())
            } else {
                None
            };

            // Build action
            let (action_type, action_config) = match *action_mode {
                ActionMode::Notify => {
                    let method = match *notify_method {
                        NotifyMethod::Sms => "sms",
                        NotifyMethod::Call => "call",
                    };
                    let mut config = serde_json::json!({ "method": method });
                    let msg = (*notify_message).clone();
                    if !msg.is_empty() {
                        config["message"] = serde_json::json!(msg);
                    }
                    ("notify".to_string(), config.to_string())
                }
                ActionMode::ToolCall => {
                    let tn = (*tool_name).clone();
                    let params = match tn.as_str() {
                        "send_chat_message" => {
                            let mut p = serde_json::json!({
                                "platform": *tc_platform,
                                "chat_name": *tc_chat_name,
                                "message": *tc_message,
                            });
                            if let Some(ref rid) = *tc_chat_room_id {
                                p["room_id"] = serde_json::json!(rid);
                            }
                            p
                        }
                        "send_email" => serde_json::json!({
                            "to": *tc_email_to,
                            "subject": *tc_email_subject,
                            "body": *tc_email_body,
                        }),
                        "respond_to_email" => serde_json::json!({
                            "response_text": *tc_reply_text,
                        }),
                        "control_tesla" => serde_json::json!({
                            "command": *tc_tesla_cmd,
                        }),
                        "create_event" => serde_json::json!({}),
                        "update_event" => serde_json::json!({}),
                        _ => {
                            // MCP or unknown - serialize from mcp_params map
                            let map = &*tc_mcp_params;
                            let obj: serde_json::Map<String, serde_json::Value> = map
                                .iter()
                                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                                .collect();
                            serde_json::Value::Object(obj)
                        }
                    };
                    let config = serde_json::json!({ "tool": tn, "params": params });
                    ("tool_call".to_string(), config.to_string())
                }
            };

            // Auto-generate name if user didn't provide one
            let name_val = {
                let n = (*name).clone();
                if n.trim().is_empty() {
                    auto_generate_name(&trigger_type, &trigger_config, &action_type, &action_config)
                } else {
                    n
                }
            };

            // Build flow_config from logic + action + else branch
            let action_config_json: serde_json::Value =
                serde_json::from_str(&action_config).unwrap_or_default();
            let action_node = serde_json::json!({
                "type": "action",
                "action_type": action_type,
                "config": action_config_json
            });
            let else_branch_json: serde_json::Value = match &*else_flow_submit {
                Some(node) => serde_json::to_value(node).unwrap_or(serde_json::Value::Null),
                None => serde_json::Value::Null,
            };
            let flow_config = match logic_type {
                "llm" => {
                    let fetch_sources: serde_json::Value = lf
                        .as_deref()
                        .and_then(|f| serde_json::from_str(f).ok())
                        .unwrap_or(serde_json::json!([]));
                    serde_json::json!({
                        "type": "llm_condition",
                        "prompt": lp.as_deref().unwrap_or("Evaluate and decide"),
                        "fetch": fetch_sources,
                        "true_branch": action_node,
                        "false_branch": else_branch_json
                    })
                }
                "keyword" => {
                    serde_json::json!({
                        "type": "keyword_condition",
                        "keyword": lp.as_deref().unwrap_or(""),
                        "true_branch": action_node,
                        "false_branch": else_branch_json
                    })
                }
                _ => action_node, // passthrough
            };

            let body = serde_json::json!({
                "name": name_val,
                "trigger_type": trigger_type,
                "trigger_config": trigger_config,
                "logic_type": logic_type,
                "logic_prompt": lp,
                "logic_fetch": lf,
                "action_type": action_type,
                "action_config": action_config,
                "flow_config": flow_config.to_string(),
            });

            saving.set(true);
            error_msg.set(None);
            let saving = saving.clone();
            let error_msg = error_msg.clone();
            let on_saved = on_saved.clone();
            let rule_id = editing_rule_id;

            spawn_local(async move {
                let request = if let Some(id) = rule_id {
                    let url = format!("/api/rules/{}", id);
                    Api::put(&url).json(&body)
                } else {
                    Api::post("/api/rules").json(&body)
                };
                let request = match request {
                    Ok(r) => r,
                    Err(e) => {
                        error_msg.set(Some(format!("Failed to build request: {}", e)));
                        saving.set(false);
                        return;
                    }
                };
                match request.send().await {
                    Ok(response) => {
                        if response.ok() {
                            on_saved.emit(());
                        } else {
                            let msg = response
                                .text()
                                .await
                                .unwrap_or_else(|_| "Failed to save rule".to_string());
                            error_msg.set(Some(msg));
                        }
                    }
                    Err(e) => {
                        error_msg.set(Some(format!("Network error: {}", e)));
                    }
                }
                saving.set(false);
            });
        })
    };

    // Pre-compute MCP tool param fields (can't do let bindings inside html! if blocks)
    let mcp_fields_html = if tool_name.starts_with("mcp:") {
        let fields = mcp_tools
            .iter()
            .find(|(v, _, _)| *v == *tool_name)
            .map(|(_, _, f)| f.clone())
            .unwrap_or_default();
        html! {
            <>
                {for fields.into_iter().map(|field| {
                    let field_label = field.clone();
                    let field_key = field.clone();
                    let current_val = tc_mcp_params.get(&field).cloned().unwrap_or_default();
                    let mcp_p = tc_mcp_params.clone();
                    html! {
                        <div class="rb-field">
                            <div class="rb-field-label">{field_label}</div>
                            <input
                                class="rb-input"
                                type="text"
                                value={current_val}
                                oninput={{
                                    let mcp_p = mcp_p.clone();
                                    let fk = field_key.clone();
                                    Callback::from(move |e: InputEvent| {
                                        if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                            let mut map = (*mcp_p).clone();
                                            map.insert(fk.clone(), input.value());
                                            mcp_p.set(map);
                                        }
                                    })
                                }}
                            />
                        </div>
                    }
                })}
            </>
        }
    } else {
        html! {}
    };

    // Tool visibility based on WHEN/IF selections
    let is_event_trigger = *when_mode == WhenMode::Event;
    let sender_is_email = *event_filter_key == "sender" && event_filter_value.contains('@');
    let no_sender_filter = *event_filter_key == "none" || *event_filter_key == "content";
    let show_respond_email = is_event_trigger && (sender_is_email || no_sender_filter);
    let show_create_event = is_event_trigger;
    let show_update_tracked = *logic_mode == LogicMode::Llm;

    // Pre-compute validation for review section
    let (rule_complete, rule_missing) = is_rule_complete(
        &*when_mode,
        &*schedule_mode,
        &*once_date,
        &*once_time,
        &*logic_mode,
        &*selected_template,
        &*logic_prompt,
        &*condition_input,
        &*keyword_input,
    );

    if !props.is_open {
        return html! {};
    }

    html! {
        <>
            <style>{BUILDER_STYLES}</style>
            <div class="rule-builder-overlay" onmousedown={on_overlay_mousedown}>
                <div class="rule-builder-panel">
                    <div class="rb-header">
                        <h2>{if is_editing { "Edit Rule" } else { "New Rule" }}</h2>
                        <button class="rb-close" onclick={on_close_click}>{"x"}</button>
                    </div>

                    <div class="rb-body">
                        // Name input
                        <input
                            class="rb-name-input"
                            type="text"
                            placeholder="Rule name..."
                            value={(*name).clone()}
                            oninput={{
                                let name = name.clone();
                                Callback::from(move |e: InputEvent| {
                                    if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                        name.set(input.value());
                                    }
                                })
                            }}
                            readonly={false}
                        />

                        // Rule summary bar
                        {build_rule_summary(
                            &when_summary,
                            &*logic_mode,
                            &*selected_template,
                            &*logic_prompt,
                            &*condition_input,
                            &*keyword_input,
                            &*action_mode,
                            &*notify_method,
                            &*tool_name,
                            &*tc_platform,
                            &*tc_chat_name,
                            &*tc_tesla_cmd,
                            &*else_flow,
                        )}

                        // WHEN card
                        {render_card(
                            Card::When,
                            "WHEN",
                            &when_summary,
                            *expanded_card == Some(Card::When),
                            toggle_card(Card::When),
                            { Some(html! {
                                <>
                                    <div class="rb-toggle-group">
                                        <button
                                            class={classes!("rb-toggle-btn", (*when_mode == WhenMode::Schedule).then(|| "active"))}
                                            onclick={{
                                                let wm = when_mode.clone();
                                                Callback::from(move |_: MouseEvent| wm.set(WhenMode::Schedule))
                                            }}
                                        >{"Schedule"}</button>
                                        <button
                                            class={classes!("rb-toggle-btn", (*when_mode == WhenMode::Event).then(|| "active"))}
                                            disabled={!is_autopilot}
                                            onclick={{
                                                let wm = when_mode.clone();
                                                Callback::from(move |_: MouseEvent| wm.set(WhenMode::Event))
                                            }}
                                        >{if is_autopilot { "Event" } else { "Event (Autopilot)" }}</button>
                                    </div>

                                    if *when_mode == WhenMode::Schedule {
                                        <div class="rb-toggle-group">
                                            <button
                                                class={classes!("rb-toggle-btn", (*schedule_mode == ScheduleMode::Once).then(|| "active"))}
                                                onclick={{
                                                    let sm = schedule_mode.clone();
                                                    Callback::from(move |_: MouseEvent| sm.set(ScheduleMode::Once))
                                                }}
                                            >{"Once"}</button>
                                            <button
                                                class={classes!("rb-toggle-btn", (*schedule_mode == ScheduleMode::Recurring).then(|| "active"))}
                                                onclick={{
                                                    let sm = schedule_mode.clone();
                                                    Callback::from(move |_: MouseEvent| sm.set(ScheduleMode::Recurring))
                                                }}
                                            >{"Recurring"}</button>
                                        </div>

                                        if *schedule_mode == ScheduleMode::Once {
                                            <div class="rb-row">
                                                <div class="rb-field">
                                                    <div class="rb-field-label">{"Date"}</div>
                                                    <input
                                                        class="rb-input"
                                                        type="date"
                                                        value={(*once_date).clone()}
                                                        oninput={{
                                                            let s = once_date.clone();
                                                            Callback::from(move |e: InputEvent| {
                                                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                                                    s.set(input.value());
                                                                }
                                                            })
                                                        }}
                                                    />
                                                </div>
                                                <div class="rb-field">
                                                    <div class="rb-field-label">{"Time"}</div>
                                                    <input
                                                        class="rb-input"
                                                        type="time"
                                                        value={(*once_time).clone()}
                                                        oninput={{
                                                            let s = once_time.clone();
                                                            Callback::from(move |e: InputEvent| {
                                                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                                                    s.set(input.value());
                                                                }
                                                            })
                                                        }}
                                                    />
                                                </div>
                                            </div>
                                        }

                                        if *schedule_mode == ScheduleMode::Recurring {
                                            <div class="rb-field">
                                                <div class="rb-field-label">{"Frequency"}</div>
                                                <div class="rb-toggle-group">
                                                    {for [
                                                        (RecurringFreq::Hourly, "Hourly"),
                                                        (RecurringFreq::Daily, "Daily"),
                                                        (RecurringFreq::Weekdays, "Weekdays"),
                                                        (RecurringFreq::Weekly, "Weekly"),
                                                    ].iter().map(|(freq, label)| {
                                                        let is_active = *recurring_freq == *freq;
                                                        let freq_clone = freq.clone();
                                                        let rf = recurring_freq.clone();
                                                        html! {
                                                            <button
                                                                class={classes!("rb-toggle-btn", is_active.then(|| "active"))}
                                                                onclick={Callback::from(move |_: MouseEvent| rf.set(freq_clone.clone()))}
                                                            >{label}</button>
                                                        }
                                                    })}
                                                </div>
                                            </div>

                                            if *recurring_freq != RecurringFreq::Hourly {
                                                <div class="rb-row">
                                                    if *recurring_freq == RecurringFreq::Weekly {
                                                        <div class="rb-field">
                                                            <div class="rb-field-label">{"Day"}</div>
                                                            <select
                                                                class="rb-select"
                                                                onchange={{
                                                                    let rd = recurring_day.clone();
                                                                    Callback::from(move |e: Event| {
                                                                        if let Some(sel) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                                                                            rd.set(sel.value());
                                                                        }
                                                                    })
                                                                }}
                                                            >
                                                                {for ["monday","tuesday","wednesday","thursday","friday","saturday","sunday"].iter().map(|d| {
                                                                    html! { <option value={*d} selected={*recurring_day == *d}>{capitalize_first(d)}</option> }
                                                                })}
                                                            </select>
                                                        </div>
                                                    }
                                                    <div class="rb-field">
                                                        <div class="rb-field-label">{"Time"}</div>
                                                        <input
                                                            class="rb-input"
                                                            type="time"
                                                            value={(*recurring_time).clone()}
                                                            oninput={{
                                                                let s = recurring_time.clone();
                                                                Callback::from(move |e: InputEvent| {
                                                                    if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                                                        s.set(input.value());
                                                                    }
                                                                })
                                                            }}
                                                        />
                                                    </div>
                                                </div>
                                            }
                                        }
                                    }

                                    if *when_mode == WhenMode::Event {
                                        <div class="rb-field">
                                            <div class="rb-field-label">{"Trigger on"}</div>
                                            <select
                                                class="rb-select"
                                                onchange={{
                                                    let ee = event_entity.clone();
                                                    Callback::from(move |e: Event| {
                                                        if let Some(sel) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                                                            ee.set(sel.value());
                                                        }
                                                    })
                                                }}
                                            >
                                                <option value="Message" selected={*event_entity == "Message"}>{"Message"}</option>
                                            </select>
                                        </div>
                                        <div class="rb-row">
                                            <div class="rb-field">
                                                <div class="rb-field-label">{"Filter"}</div>
                                                <select
                                                    class="rb-select"
                                                    onchange={{
                                                        let fk = event_filter_key.clone();
                                                        let fv = event_filter_value.clone();
                                                        let sgm = selected_group_mode.clone();
                                                        Callback::from(move |e: Event| {
                                                            if let Some(sel) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                                                                if sel.value() == "none" {
                                                                    fv.set(String::new());
                                                                }
                                                                fk.set(sel.value());
                                                                // Clear group mode when filter type changes
                                                                sgm.set(None);
                                                            }
                                                        })
                                                    }}
                                                >
                                                    <option value="none" selected={*event_filter_key == "none"}>{"No filter"}</option>
                                                    <option value="sender" selected={*event_filter_key == "sender"}>{"Sender"}</option>
                                                    <option value="content" selected={*event_filter_key == "content"}>{"Content"}</option>
                                                </select>
                                            </div>
                                            if *event_filter_key == "sender" {
                                                <div class="rb-field" style="position: relative;">
                                                    <div class="rb-field-label">{"Person / Group"}</div>
                                                    <input
                                                        class="rb-input"
                                                        type="text"
                                                        placeholder="Type to search..."
                                                        value={(*event_filter_value).clone()}
                                                        oninput={{
                                                            let fv = event_filter_value.clone();
                                                            let sd = sender_dropdown_open.clone();
                                                            let sgm = selected_group_mode.clone();
                                                            Callback::from(move |e: InputEvent| {
                                                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                                                    fv.set(input.value());
                                                                    sd.set(true);
                                                                    // Clear group mode when user types (deselects group)
                                                                    sgm.set(None);
                                                                }
                                                            })
                                                        }}
                                                        onfocus={{
                                                            let sd = sender_dropdown_open.clone();
                                                            Callback::from(move |_: FocusEvent| sd.set(true))
                                                        }}
                                                    />
                                                    if *sender_dropdown_open {
                                                        <div class="rb-autocomplete">
                                                            { for sender_suggestions.iter()
                                                                .filter(|(name, _, _, _)| {
                                                                    let q = event_filter_value.to_lowercase();
                                                                    q.is_empty() || name.to_lowercase().contains(&q)
                                                                })
                                                                .map(|(name, platform, is_group, group_mode)| {
                                                                    let display = if *is_group {
                                                                        let mode_label = match group_mode.as_deref() {
                                                                            Some("mention_only") => "(mention only)",
                                                                            _ => "(all)",
                                                                        };
                                                                        match platform {
                                                                            Some(p) => format!("{} ({}) (group){}", name, p, mode_label),
                                                                            None => format!("{} (group){}", name, mode_label),
                                                                        }
                                                                    } else {
                                                                        match platform {
                                                                            Some(p) => format!("{} ({})", name, p),
                                                                            None => name.clone(),
                                                                        }
                                                                    };
                                                                    let set_val = name.clone();
                                                                    let fv = event_filter_value.clone();
                                                                    let sd = sender_dropdown_open.clone();
                                                                    let sgm = selected_group_mode.clone();
                                                                    let gm = group_mode.clone();
                                                                    let is_grp = *is_group;
                                                                    let lm = logic_mode.clone();
                                                                    html! {
                                                                        <div class="rb-autocomplete-item"
                                                                            onmousedown={{
                                                                                Callback::from(move |e: MouseEvent| {
                                                                                    e.prevent_default();
                                                                                    fv.set(set_val.clone());
                                                                                    sd.set(false);
                                                                                    if is_grp {
                                                                                        sgm.set(gm.clone());
                                                                                        // Force logic mode to non-LLM when group is selected
                                                                                        lm.set(LogicMode::Always);
                                                                                    } else {
                                                                                        sgm.set(None);
                                                                                    }
                                                                                })
                                                                            }}
                                                                        >
                                                                            {display}
                                                                        </div>
                                                                    }
                                                                })
                                                            }
                                                        </div>
                                                    }
                                                </div>
                                            } else if *event_filter_key == "content" {
                                                <div class="rb-field">
                                                    <div class="rb-field-label">{"Contains"}</div>
                                                    <input
                                                        class="rb-input"
                                                        type="text"
                                                        placeholder="e.g. urgent"
                                                        value={(*event_filter_value).clone()}
                                                        oninput={{
                                                            let fv = event_filter_value.clone();
                                                            Callback::from(move |e: InputEvent| {
                                                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                                                    fv.set(input.value());
                                                                }
                                                            })
                                                        }}
                                                    />
                                                </div>
                                            }
                                        </div>
                                        <div class="rb-field" style="margin-top: 0.5rem;">
                                            <div class="rb-field-label">{"Frequency"}</div>
                                            <div style="display: flex; gap: 0.5rem;">
                                                <button
                                                    class={classes!("rb-toggle-btn", (*event_fire_once).then(|| "active"))}
                                                    onclick={{
                                                        let fo = event_fire_once.clone();
                                                        Callback::from(move |_: MouseEvent| fo.set(true))
                                                    }}
                                                >
                                                    {"Once"}
                                                </button>
                                                <button
                                                    class={classes!("rb-toggle-btn", (!*event_fire_once).then(|| "active"))}
                                                    onclick={{
                                                        let fo = event_fire_once.clone();
                                                        Callback::from(move |_: MouseEvent| fo.set(false))
                                                    }}
                                                >
                                                    {"Recurring"}
                                                </button>
                                            </div>
                                        </div>
                                        <div class="rb-field" style="margin-top: 0.5rem;">
                                            <div class="rb-field-label">{"Wait before acting"}</div>
                                            <div class="rb-field-hint">{
                                                if *event_delay == 0 {
                                                    "Fires instantly, even if you already saw the message"
                                                } else {
                                                    "Waits this long, then skips if you already saw the message"
                                                }
                                            }</div>
                                            <div style="display: flex; flex-wrap: wrap; gap: 0.3rem; margin-top: 0.3rem;">
                                                {for [(0, "Immediate"), (120, "2 min"), (300, "5 min"), (600, "10 min")].iter().map(|(secs, label)| {
                                                    let is_active = *event_delay == *secs;
                                                    let delay = event_delay.clone();
                                                    let val = *secs;
                                                    html! {
                                                        <button
                                                            class={if is_active { "rb-toggle-btn active" } else { "rb-toggle-btn" }}
                                                            onclick={Callback::from(move |_: MouseEvent| delay.set(val))}
                                                        >
                                                            {label}
                                                        </button>
                                                    }
                                                })}
                                            </div>
                                        </div>
                                    }
                                </>
                            })}
                        )}

                        <div class="rb-connector">
                            <div class="rb-connector-line"></div>
                            <span>{"v"}</span>
                        </div>

                        // IF card
                        {render_card(
                            Card::If,
                            "IF",
                            &if_summary,
                            *expanded_card == Some(Card::If),
                            toggle_card(Card::If),
                            { Some(html! {
                                <>
                                    <div class="rb-toggle-group">
                                        <button
                                            class={classes!("rb-toggle-btn", (*logic_mode == LogicMode::Always).then(|| "active"))}
                                            onclick={{
                                                let lm = logic_mode.clone();
                                                Callback::from(move |_: MouseEvent| lm.set(LogicMode::Always))
                                            }}
                                        >{"Always"}</button>
                                        <button
                                            class={classes!("rb-toggle-btn", (*logic_mode == LogicMode::Keyword).then(|| "active"))}
                                            onclick={{
                                                let lm = logic_mode.clone();
                                                Callback::from(move |_: MouseEvent| lm.set(LogicMode::Keyword))
                                            }}
                                        >{"Keyword"}</button>
                                        <button
                                            class={classes!("rb-toggle-btn", (*logic_mode == LogicMode::Llm).then(|| "active"))}
                                            disabled={!is_autopilot || selected_group_mode.is_some()}
                                            onclick={{
                                                let lm = logic_mode.clone();
                                                Callback::from(move |_: MouseEvent| lm.set(LogicMode::Llm))
                                            }}
                                        >{if selected_group_mode.is_some() {
                                            "AI decides (not for groups)"
                                        } else if is_autopilot {
                                            "AI decides"
                                        } else {
                                            "AI decides (Autopilot)"
                                        }}</button>
                                    </div>
                                    if *logic_mode == LogicMode::Keyword {
                                        <div class="rb-field">
                                            <div class="rb-field-label">{"Keyword"}</div>
                                            <input
                                                class="rb-input"
                                                type="text"
                                                placeholder="e.g. ?, urgent, meeting"
                                                value={(*keyword_input).clone()}
                                                oninput={{
                                                    let ki = keyword_input.clone();
                                                    Callback::from(move |e: InputEvent| {
                                                        if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                                            ki.set(input.value());
                                                        }
                                                    })
                                                }}
                                            />
                                        </div>
                                        <div class="rb-field-hint">
                                            {"Triggers when message content contains this text (case-insensitive)"}
                                        </div>
                                    }
                                    if *logic_mode == LogicMode::Llm {
                                        <div class="rb-template-group">
                                            {for [
                                                (PromptTemplate::Summarize, "Summarize"),
                                                (PromptTemplate::CheckCondition, "Check condition"),
                                                (PromptTemplate::Custom, "Custom"),
                                            ].iter().map(|(tmpl, label)| {
                                                let is_active = *selected_template == *tmpl;
                                                let tmpl_clone = tmpl.clone();
                                                let st = selected_template.clone();
                                                let as_tmpl = active_sources.clone();
                                                let wm = when_mode.clone();
                                                html! {
                                                    <button
                                                        class={classes!("rb-template-btn", is_active.then(|| "active"))}
                                                        onclick={Callback::from(move |_: MouseEvent| {
                                                            st.set(tmpl_clone.clone());
                                                            match &tmpl_clone {
                                                                PromptTemplate::Summarize => match *wm {
                                                                    WhenMode::Schedule => {
                                                                        as_tmpl.set(vec![SourceConfig::Email, SourceConfig::Chat { platform: "all".to_string(), limit: 50 }, SourceConfig::Events]);
                                                                    }
                                                                    WhenMode::Event => {
                                                                        as_tmpl.set(vec![SourceConfig::Chat { platform: "all".to_string(), limit: 50 }]);
                                                                    }
                                                                },
                                                                PromptTemplate::FilterImportant => match *wm {
                                                                    WhenMode::Schedule => {
                                                                        as_tmpl.set(vec![SourceConfig::Email, SourceConfig::Chat { platform: "all".to_string(), limit: 50 }]);
                                                                    }
                                                                    WhenMode::Event => {
                                                                        as_tmpl.set(vec![]);
                                                                    }
                                                                },
                                                                PromptTemplate::TrackItemsUpdate | PromptTemplate::TrackItemsCreate => {
                                                                    as_tmpl.set(vec![SourceConfig::Events]);
                                                                },
                                                                PromptTemplate::CheckCondition | PromptTemplate::Custom => {}
                                                            }
                                                        })}
                                                    >{label}</button>
                                                }
                                            })}
                                        </div>

                                        if *selected_template == PromptTemplate::Summarize || *selected_template == PromptTemplate::FilterImportant || *selected_template == PromptTemplate::TrackItemsUpdate || *selected_template == PromptTemplate::TrackItemsCreate {
                                            <div class="rb-template-desc">
                                                {get_template_description(&*selected_template, &*when_mode)}
                                            </div>
                                            <button class="rb-template-edit-link"
                                                onclick={{
                                                    let st = selected_template.clone();
                                                    let lp = logic_prompt.clone();
                                                    let wm = when_mode.clone();
                                                    let tmpl = (*selected_template).clone();
                                                    Callback::from(move |_: MouseEvent| {
                                                        let prompt = get_template_prompt(&tmpl, &*wm, "");
                                                        lp.set(prompt);
                                                        st.set(PromptTemplate::Custom);
                                                    })
                                                }}
                                            >{"Customize..."}</button>
                                        }

                                        if *selected_template == PromptTemplate::CheckCondition {
                                            <div class="rb-field">
                                                <div class="rb-field-label">{"Condition"}</div>
                                                <input
                                                    class="rb-input"
                                                    type="text"
                                                    placeholder="e.g. mentions a meeting or deadline"
                                                    value={(*condition_input).clone()}
                                                    oninput={{
                                                        let ci = condition_input.clone();
                                                        Callback::from(move |e: InputEvent| {
                                                            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                                                ci.set(input.value());
                                                            }
                                                        })
                                                    }}
                                                />
                                            </div>
                                        }

                                        if *selected_template == PromptTemplate::Custom {
                                            // Sentence starters when prompt is empty/short
                                            if logic_prompt.len() < 20 {
                                                <div class="rb-starter-chips">
                                                    {for get_starter_chips(&*when_mode).iter().map(|starter| {
                                                        let text = starter.to_string();
                                                        let lp = logic_prompt.clone();
                                                        html! {
                                                            <button
                                                                class="rb-template-btn"
                                                                onclick={Callback::from(move |_: MouseEvent| {
                                                                    lp.set(text.clone());
                                                                })}
                                                            >{*starter}</button>
                                                        }
                                                    })}
                                                </div>
                                            }
                                            <div class="rb-field">
                                                <div class="rb-field-label">{"Prompt"}</div>
                                                <textarea
                                                    class="rb-textarea"
                                                    placeholder="Describe what the AI should evaluate..."
                                                    value={(*logic_prompt).clone()}
                                                    oninput={{
                                                        let lp = logic_prompt.clone();
                                                        let as_auto = active_sources.clone();
                                                        let avail = available_sources.clone();
                                                        Callback::from(move |e: InputEvent| {
                                                            if let Some(input) = e.target_dyn_into::<web_sys::HtmlTextAreaElement>() {
                                                                let val = input.value();
                                                                lp.set(val.clone());
                                                                // Auto-detect context sources from prompt
                                                                let detected = auto_detect_sources(&val, &*avail);
                                                                if !detected.is_empty() {
                                                                    let mut current = (*as_auto).clone();
                                                                    let mut changed = false;
                                                                    for src in detected {
                                                                        if !current.iter().any(|s| s.type_key() == src.type_key()) {
                                                                            current.push(src);
                                                                            changed = true;
                                                                        }
                                                                    }
                                                                    if changed {
                                                                        as_auto.set(current);
                                                                    }
                                                                }
                                                            }
                                                        })
                                                    }}
                                                ></textarea>
                                                <div class="rb-field-hint">
                                                    {if *when_mode == WhenMode::Event {
                                                        "e.g., is urgent, mentions a deadline, is from my boss"
                                                    } else {
                                                        "e.g., there's something important, I have unread emails"
                                                    }}
                                                </div>
                                            </div>
                                        }

                                        <div class="rb-field">
                                            <div class="rb-field-label">{"Extra context for AI"}</div>
                                            <div class="rb-context-hint">
                                                {"AI already sees the trigger event. Add more context:"}
                                            </div>
                                            <div class="rb-source-pills">
                                                {for available_sources.iter().map(|src| {
                                                    let st = src.source_type.clone();
                                                    let label = src.label.clone();
                                                    let avail = src.available;
                                                    let meta = src.meta.clone();

                                                    // For MCP, render one pill per server
                                                    if st == "mcp" {
                                                        let servers = meta.get("servers").and_then(|s| s.as_array()).cloned().unwrap_or_default();
                                                        return html! {
                                                            <>
                                                                {for servers.into_iter().map(|srv| {
                                                                    let srv_name = srv.get("name").and_then(|n| n.as_str()).unwrap_or("mcp").to_string();
                                                                    let pill_key = format!("mcp:{}", srv_name);
                                                                    let is_active = active_sources.iter().any(|s| matches!(s, SourceConfig::Mcp { server, .. } if *server == srv_name));
                                                                    let pill_class = if !avail {
                                                                        "rb-source-pill disabled"
                                                                    } else if is_active {
                                                                        "rb-source-pill active"
                                                                    } else {
                                                                        "rb-source-pill"
                                                                    };
                                                                    let as_pill = active_sources.clone();
                                                                    let es = expanded_source.clone();
                                                                    let srv_n = srv_name.clone();
                                                                    let pk = pill_key.clone();
                                                                    let mst = mcp_source_tools.clone();
                                                                    let srv_id = srv.get("id").and_then(|i| i.as_i64()).unwrap_or(0);
                                                                    html! {
                                                                        <span class={pill_class}
                                                                            title={if !avail { "Connect in Settings" } else { "" }}
                                                                            onclick={Callback::from(move |_: MouseEvent| {
                                                                                if !avail { return; }
                                                                                let mut current = (*as_pill).clone();
                                                                                if is_active {
                                                                                    current.retain(|s| !matches!(s, SourceConfig::Mcp { server, .. } if *server == srv_n));
                                                                                    as_pill.set(current);
                                                                                    if *es == Some(pk.clone()) { es.set(None); }
                                                                                } else {
                                                                                    // Expand to pick tool
                                                                                    es.set(Some(pk.clone()));
                                                                                    // Fetch tools if not cached
                                                                                    if !mst.contains_key(&srv_n) {
                                                                                        let mst2 = mst.clone();
                                                                                        let sn = srv_n.clone();
                                                                                        let sid = srv_id;
                                                                                        spawn_local(async move {
                                                                                            let url = format!("/api/mcp/servers/{}/tools", sid);
                                                                                            if let Ok(r) = Api::get(&url).send().await {
                                                                                                if let Ok(resp) = r.json::<serde_json::Value>().await {
                                                                                                    if let Some(tools) = resp.get("tools").and_then(|v| v.as_array()) {
                                                                                                        let tool_opts: Vec<McpToolOption> = tools.iter().filter_map(|t| {
                                                                                                            let name = t.get("name")?.as_str()?.to_string();
                                                                                                            let desc = t.get("description").and_then(|d| d.as_str()).map(|s| s.to_string());
                                                                                                            Some(McpToolOption { name, description: desc })
                                                                                                        }).collect();
                                                                                                        let mut map = (*mst2).clone();
                                                                                                        map.insert(sn, tool_opts);
                                                                                                        mst2.set(map);
                                                                                                    }
                                                                                                }
                                                                                            }
                                                                                        });
                                                                                    }
                                                                                }
                                                                            })}
                                                                        >
                                                                            {&srv_name}
                                                                            if is_active {
                                                                                <span class="rb-pill-x"
                                                                                    onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}
                                                                                >{"x"}</span>
                                                                            }
                                                                        </span>
                                                                    }
                                                                })}
                                                            </>
                                                        };
                                                    }

                                                    let is_active = active_sources.iter().any(|s| s.is_type(&st));
                                                    let pill_class = if !avail {
                                                        "rb-source-pill disabled"
                                                    } else if is_active {
                                                        "rb-source-pill active"
                                                    } else {
                                                        "rb-source-pill"
                                                    };
                                                    let has_config = st == "chat" || st == "internet" || st == "weather";
                                                    let as_pill = active_sources.clone();
                                                    let es = expanded_source.clone();
                                                    let st_c = st.clone();
                                                    let st_c2 = st.clone();
                                                    // Pre-compute profile location for weather default
                                                    let profile_location = meta.get("location")
                                                        .and_then(|l| l.as_str())
                                                        .unwrap_or("")
                                                        .to_string();
                                                    html! {
                                                        <span class={pill_class}
                                                            title={if !avail { "Connect in Settings" } else { "" }}
                                                            onclick={Callback::from(move |_: MouseEvent| {
                                                                if !avail { return; }
                                                                let mut current = (*as_pill).clone();
                                                                if is_active {
                                                                    // Remove
                                                                    current.retain(|s| !s.is_type(&st_c));
                                                                    as_pill.set(current);
                                                                    if *es == Some(st_c.clone()) { es.set(None); }
                                                                } else {
                                                                    // Add default config
                                                                    let new_source = match st_c.as_str() {
                                                                        "email" => SourceConfig::Email,
                                                                        "chat" => SourceConfig::Chat { platform: "all".to_string(), limit: 50 },
                                                                        "weather" => SourceConfig::Weather { location: profile_location.clone() },
                                                                        "internet" => SourceConfig::Internet { query: String::new() },
                                                                        "tesla" => SourceConfig::Tesla,
                                                                        "events" => SourceConfig::Events,
                                                                        _ => return,
                                                                    };
                                                                    current.push(new_source);
                                                                    as_pill.set(current);
                                                                    if has_config {
                                                                        es.set(Some(st_c.clone()));
                                                                    }
                                                                }
                                                            })}
                                                        >
                                                            {&label}
                                                            if is_active {
                                                                <span class="rb-pill-x"
                                                                    onclick={{
                                                                        let as_x = active_sources.clone();
                                                                        let es_x = expanded_source.clone();
                                                                        let st_x = st_c2.clone();
                                                                        Callback::from(move |e: MouseEvent| {
                                                                            e.stop_propagation();
                                                                            let mut current = (*as_x).clone();
                                                                            current.retain(|s| !s.is_type(&st_x));
                                                                            as_x.set(current);
                                                                            if *es_x == Some(st_x.clone()) { es_x.set(None); }
                                                                        })
                                                                    }}
                                                                >{"x"}</span>
                                                            }
                                                        </span>
                                                    }
                                                })}
                                            </div>

                                            // Inline config panels for configurable sources
                                            if *expanded_source == Some("chat".to_string()) {
                                                <div class="rb-source-options">
                                                    <div class="rb-row">
                                                        <div class="rb-field">
                                                            <div class="rb-field-label">{"Platform"}</div>
                                                            <select
                                                                class="rb-select"
                                                                onchange={{
                                                                    let as_c = active_sources.clone();
                                                                    Callback::from(move |e: Event| {
                                                                        if let Some(sel) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                                                                            let mut current = (*as_c).clone();
                                                                            if let Some(chat) = current.iter_mut().find(|s| matches!(s, SourceConfig::Chat { .. })) {
                                                                                if let SourceConfig::Chat { platform, .. } = chat {
                                                                                    *platform = sel.value();
                                                                                }
                                                                            }
                                                                            as_c.set(current);
                                                                        }
                                                                    })
                                                                }}
                                                            >
                                                                <option value="all" selected={active_sources.iter().any(|s| matches!(s, SourceConfig::Chat { platform, .. } if platform == "all"))}>{"All platforms"}</option>
                                                                {for available_sources.iter()
                                                                    .find(|s| s.source_type == "chat")
                                                                    .and_then(|s| s.meta.get("platforms"))
                                                                    .and_then(|p| p.as_array())
                                                                    .cloned()
                                                                    .unwrap_or_default()
                                                                    .iter()
                                                                    .filter_map(|p| p.as_str())
                                                                    .map(|p| {
                                                                        let sel = active_sources.iter().any(|s| matches!(s, SourceConfig::Chat { platform, .. } if platform == p));
                                                                        html! { <option value={p.to_string()} selected={sel}>{capitalize_first(p)}</option> }
                                                                    })
                                                                }
                                                            </select>
                                                        </div>
                                                        <div class="rb-field">
                                                            <div class="rb-field-label">{"Messages"}</div>
                                                            <select
                                                                class="rb-select"
                                                                onchange={{
                                                                    let as_c = active_sources.clone();
                                                                    Callback::from(move |e: Event| {
                                                                        if let Some(sel) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                                                                            let lim: u32 = sel.value().parse().unwrap_or(50);
                                                                            let mut current = (*as_c).clone();
                                                                            if let Some(chat) = current.iter_mut().find(|s| matches!(s, SourceConfig::Chat { .. })) {
                                                                                if let SourceConfig::Chat { limit, .. } = chat {
                                                                                    *limit = lim;
                                                                                }
                                                                            }
                                                                            as_c.set(current);
                                                                        }
                                                                    })
                                                                }}
                                                            >
                                                                {for [20u32, 50, 100].iter().map(|n| {
                                                                    let sel = active_sources.iter().any(|s| matches!(s, SourceConfig::Chat { limit, .. } if *limit == *n));
                                                                    html! { <option value={n.to_string()} selected={sel}>{n.to_string()}</option> }
                                                                })}
                                                            </select>
                                                        </div>
                                                    </div>
                                                </div>
                                            }

                                            if *expanded_source == Some("weather".to_string()) {
                                                <div class="rb-source-options">
                                                    <div class="rb-field">
                                                        <div class="rb-field-label">{"Location"}</div>
                                                        <input
                                                            class="rb-input"
                                                            type="text"
                                                            placeholder="e.g. Helsinki, Finland"
                                                            value={active_sources.iter().find_map(|s| match s {
                                                                SourceConfig::Weather { location } => Some(location.clone()),
                                                                _ => None,
                                                            }).unwrap_or_default()}
                                                            oninput={{
                                                                let as_c = active_sources.clone();
                                                                Callback::from(move |e: InputEvent| {
                                                                    if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                                                        let mut current = (*as_c).clone();
                                                                        if let Some(w) = current.iter_mut().find(|s| matches!(s, SourceConfig::Weather { .. })) {
                                                                            if let SourceConfig::Weather { location } = w {
                                                                                *location = input.value();
                                                                            }
                                                                        }
                                                                        as_c.set(current);
                                                                    }
                                                                })
                                                            }}
                                                        />
                                                    </div>
                                                </div>
                                            }

                                            if *expanded_source == Some("internet".to_string()) {
                                                <div class="rb-source-options">
                                                    <div class="rb-field">
                                                        <div class="rb-field-label">{"Search query"}</div>
                                                        <input
                                                            class="rb-input"
                                                            type="text"
                                                            placeholder="e.g. latest news about AI regulation"
                                                            value={active_sources.iter().find_map(|s| match s {
                                                                SourceConfig::Internet { query } => Some(query.clone()),
                                                                _ => None,
                                                            }).unwrap_or_default()}
                                                            oninput={{
                                                                let as_c = active_sources.clone();
                                                                Callback::from(move |e: InputEvent| {
                                                                    if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                                                        let mut current = (*as_c).clone();
                                                                        if let Some(inet) = current.iter_mut().find(|s| matches!(s, SourceConfig::Internet { .. })) {
                                                                            if let SourceConfig::Internet { query } = inet {
                                                                                *query = input.value();
                                                                            }
                                                                        }
                                                                        as_c.set(current);
                                                                    }
                                                                })
                                                            }}
                                                        />
                                                    </div>
                                                </div>
                                            }

                                            // MCP server tool picker
                                            {for available_sources.iter()
                                                .find(|s| s.source_type == "mcp")
                                                .and_then(|s| s.meta.get("servers"))
                                                .and_then(|s| s.as_array())
                                                .cloned()
                                                .unwrap_or_default()
                                                .iter()
                                                .filter_map(|srv| {
                                                    let srv_name = srv.get("name")?.as_str()?.to_string();
                                                    let pill_key = format!("mcp:{}", srv_name);
                                                    if *expanded_source != Some(pill_key) { return None; }
                                                    let tools = mcp_source_tools.get(&srv_name).cloned().unwrap_or_default();
                                                    let as_mcp = active_sources.clone();
                                                    let sn = srv_name.clone();
                                                    Some(html! {
                                                        <div class="rb-source-options">
                                                            <div class="rb-field">
                                                                <div class="rb-field-label">{"Tool"}</div>
                                                                <select
                                                                    class="rb-select"
                                                                    onchange={{
                                                                        let as_c = as_mcp.clone();
                                                                        let sn_c = sn.clone();
                                                                        Callback::from(move |e: Event| {
                                                                            if let Some(sel) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                                                                                let mut current = (*as_c).clone();
                                                                                // Remove old MCP entry for this server
                                                                                current.retain(|s| !matches!(s, SourceConfig::Mcp { server, .. } if *server == sn_c));
                                                                                if !sel.value().is_empty() {
                                                                                    current.push(SourceConfig::Mcp {
                                                                                        server: sn_c.clone(),
                                                                                        tool: sel.value(),
                                                                                        args: "{}".to_string(),
                                                                                    });
                                                                                }
                                                                                as_c.set(current);
                                                                            }
                                                                        })
                                                                    }}
                                                                >
                                                                    <option value="">{"Select tool..."}</option>
                                                                    {for tools.iter().map(|t| {
                                                                        let sel = active_sources.iter().any(|s| matches!(s, SourceConfig::Mcp { server, tool, .. } if *server == sn && *tool == t.name));
                                                                        html! { <option value={t.name.clone()} selected={sel}>{&t.name}</option> }
                                                                    })}
                                                                </select>
                                                            </div>
                                                            // Args field for selected MCP tool
                                                            if active_sources.iter().any(|s| matches!(s, SourceConfig::Mcp { server, .. } if *server == sn)) {
                                                                <div class="rb-field">
                                                                    <div class="rb-field-label">{"Args (JSON)"}</div>
                                                                    <input
                                                                        class="rb-input"
                                                                        type="text"
                                                                        placeholder="{}"
                                                                        value={active_sources.iter().find_map(|s| match s {
                                                                            SourceConfig::Mcp { server, args, .. } if *server == sn => Some(args.clone()),
                                                                            _ => None,
                                                                        }).unwrap_or_else(|| "{}".to_string())}
                                                                        oninput={{
                                                                            let as_c = active_sources.clone();
                                                                            let sn_c = sn.clone();
                                                                            Callback::from(move |e: InputEvent| {
                                                                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                                                                    let mut current = (*as_c).clone();
                                                                                    if let Some(mcp) = current.iter_mut().find(|s| matches!(s, SourceConfig::Mcp { server, .. } if *server == sn_c)) {
                                                                                        if let SourceConfig::Mcp { args, .. } = mcp {
                                                                                            *args = input.value();
                                                                                        }
                                                                                    }
                                                                                    as_c.set(current);
                                                                                }
                                                                            })
                                                                        }}
                                                                    />
                                                                </div>
                                                            }
                                                        </div>
                                                    })
                                                })
                                            }
                                        </div>
                                    }
                                </>
                            })}
                        )}

                        <div class="rb-connector">
                            <div class="rb-connector-line"></div>
                            <span>{"v"}</span>
                        </div>

                        // THEN card
                        {render_card(
                            Card::Then,
                            "THEN",
                            &then_summary,
                            *expanded_card == Some(Card::Then),
                            toggle_card(Card::Then),
                            { Some(html! {
                                <>
                                    <div class="rb-toggle-group">
                                        <button
                                            class={classes!("rb-toggle-btn", (*action_mode == ActionMode::Notify).then(|| "active"))}
                                            onclick={{
                                                let am = action_mode.clone();
                                                Callback::from(move |_: MouseEvent| am.set(ActionMode::Notify))
                                            }}
                                        >{"Notify me"}</button>
                                        <button
                                            class={classes!("rb-toggle-btn", (*action_mode == ActionMode::ToolCall).then(|| "active"))}
                                            onclick={{
                                                let am = action_mode.clone();
                                                Callback::from(move |_: MouseEvent| am.set(ActionMode::ToolCall))
                                            }}
                                        >{"Run tool"}</button>
                                    </div>

                                    if *action_mode == ActionMode::Notify {
                                        <div class="rb-radio-group" style="margin-bottom: 0.5rem;">
                                            <label class="rb-radio-label">
                                                <input
                                                    type="radio"
                                                    name="notify-method"
                                                    checked={*notify_method == NotifyMethod::Sms}
                                                    onchange={{
                                                        let nm = notify_method.clone();
                                                        Callback::from(move |_: Event| nm.set(NotifyMethod::Sms))
                                                    }}
                                                />
                                                {"SMS"}
                                            </label>
                                            <label class="rb-radio-label">
                                                <input
                                                    type="radio"
                                                    name="notify-method"
                                                    checked={*notify_method == NotifyMethod::Call}
                                                    onchange={{
                                                        let nm = notify_method.clone();
                                                        Callback::from(move |_: Event| nm.set(NotifyMethod::Call))
                                                    }}
                                                />
                                                {"Call"}
                                            </label>
                                        </div>
                                        <div class="rb-field">
                                            <div class="rb-field-label">{"Message (optional)"}</div>
                                            <textarea
                                                class="rb-textarea"
                                                placeholder={if *logic_mode == LogicMode::Llm {
                                                    "Leave empty - AI will generate the message"
                                                } else {
                                                    "Fixed message text, or leave empty for default"
                                                }}
                                                value={(*notify_message).clone()}
                                                oninput={{
                                                    let nm = notify_message.clone();
                                                    Callback::from(move |e: InputEvent| {
                                                        if let Some(input) = e.target_dyn_into::<web_sys::HtmlTextAreaElement>() {
                                                            nm.set(input.value());
                                                        }
                                                    })
                                                }}
                                            ></textarea>
                                            if *logic_mode == LogicMode::Llm {
                                                <div class="rb-field-hint">
                                                    {"AI's evaluation result will be used as the notification text"}
                                                </div>
                                            }
                                        </div>
                                    }

                                    if *action_mode == ActionMode::ToolCall {
                                        <div class="rb-field">
                                            <div class="rb-field-label">{"Tool"}</div>
                                            <select
                                                class="rb-select"
                                                onchange={{
                                                    let tn = tool_name.clone();
                                                    let mcp_p = tc_mcp_params.clone();
                                                    Callback::from(move |e: Event| {
                                                        if let Some(sel) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                                                            tn.set(sel.value());
                                                            // Clear MCP params when switching tools
                                                            mcp_p.set(HashMap::new());
                                                        }
                                                    })
                                                }}
                                            >
                                                <optgroup label="Built-in">
                                                    <option value="send_chat_message" selected={*tool_name == "send_chat_message"}>{"Send chat message"}</option>
                                                    <option value="send_email" selected={*tool_name == "send_email"}>{"Send email"}</option>
                                                    if show_respond_email {
                                                        <option value="respond_to_email" selected={*tool_name == "respond_to_email"}>{"Reply to email"}</option>
                                                    }
                                                    <option value="control_tesla" selected={*tool_name == "control_tesla"}>{"Tesla command"}</option>
                                                    if show_create_event {
                                                        <option value="create_event" selected={*tool_name == "create_event"}>{"Create event"}</option>
                                                    }
                                                    if show_update_tracked {
                                                        <option value="update_event" selected={*tool_name == "update_event"}>{"Update event"}</option>
                                                    }
                                                </optgroup>
                                                if !mcp_tools.is_empty() {
                                                    <optgroup label="MCP Tools">
                                                        {for mcp_tools.iter().map(|(value, display, _)| {
                                                            let sel = *tool_name == *value;
                                                            html! { <option value={value.clone()} selected={sel}>{display.clone()}</option> }
                                                        })}
                                                    </optgroup>
                                                }
                                            </select>
                                        </div>

                                        // Show LLM-fillable params when in AI mode
                                        if *logic_mode == LogicMode::Llm {
                                            {render_llm_params_hint(&tool_name)}
                                        }

                                        // Per-tool parameter fields
                                        if *tool_name == "send_chat_message" {
                                            <div class="rb-field">
                                                <div class="rb-field-label">{"Platform"}</div>
                                                <select
                                                    class="rb-select"
                                                    onchange={{
                                                        let s = tc_platform.clone();
                                                        let sr = tc_chat_search_results.clone();
                                                        let cn = tc_chat_name.clone();
                                                        let rid = tc_chat_room_id.clone();
                                                        Callback::from(move |e: Event| {
                                                            if let Some(sel) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                                                                s.set(sel.value());
                                                                // Clear chat selection when platform changes
                                                                cn.set(String::new());
                                                                rid.set(None);
                                                                sr.set(Vec::new());
                                                            }
                                                        })
                                                    }}
                                                >
                                                    <option value="whatsapp" selected={*tc_platform == "whatsapp"}>{"WhatsApp"}</option>
                                                    <option value="telegram" selected={*tc_platform == "telegram"}>{"Telegram"}</option>
                                                    <option value="signal" selected={*tc_platform == "signal"}>{"Signal"}</option>
                                                </select>
                                            </div>
                                            <div class="rb-field" style="position: relative;">
                                                <div class="rb-field-label">{"Chat"}</div>
                                                <input
                                                    class="rb-input"
                                                    type="text"
                                                    placeholder="Search chats..."
                                                    value={(*tc_chat_name).clone()}
                                                    oninput={{
                                                        let cn = tc_chat_name.clone();
                                                        let rid = tc_chat_room_id.clone();
                                                        let sr = tc_chat_search_results.clone();
                                                        let so = tc_chat_search_open.clone();
                                                        let sg = tc_chat_searching.clone();
                                                        let plat = (*tc_platform).clone();
                                                        Callback::from(move |e: InputEvent| {
                                                            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                                                let v = input.value();
                                                                cn.set(v.clone());
                                                                rid.set(None);
                                                                if v.len() >= 2 {
                                                                    so.set(true);
                                                                    sg.set(true);
                                                                    let sr = sr.clone();
                                                                    let sg = sg.clone();
                                                                    let plat = plat.clone();
                                                                    let query = v;
                                                                    spawn_local(async move {
                                                                        let url = format!(
                                                                            "/api/persons/search/{}?q={}",
                                                                            plat,
                                                                            js_sys::encode_uri_component(&query)
                                                                        );
                                                                        if let Ok(r) = Api::get(&url).send().await {
                                                                            if let Ok(data) = r.json::<serde_json::Value>().await {
                                                                                if let Some(results) = data.get("results").and_then(|r| r.as_array()) {
                                                                                    let rooms: Vec<(String, String, Option<String>)> = results.iter()
                                                                                        .filter_map(|r| {
                                                                                            let name = r.get("display_name")?.as_str()?.to_string();
                                                                                            let room_id = r.get("room_id")?.as_str()?.to_string();
                                                                                            let person = r.get("person_name").and_then(|v| v.as_str()).map(|s| s.to_string());
                                                                                            Some((name, room_id, person))
                                                                                        })
                                                                                        .collect();
                                                                                    sr.set(rooms);
                                                                                }
                                                                            }
                                                                        }
                                                                        sg.set(false);
                                                                    });
                                                                } else {
                                                                    so.set(false);
                                                                    sr.set(Vec::new());
                                                                }
                                                            }
                                                        })
                                                    }}
                                                    onfocus={{
                                                        let so = tc_chat_search_open.clone();
                                                        let cn = tc_chat_name.clone();
                                                        Callback::from(move |_: FocusEvent| {
                                                            if cn.len() >= 2 {
                                                                so.set(true);
                                                            }
                                                        })
                                                    }}
                                                    onblur={{
                                                        let so = tc_chat_search_open.clone();
                                                        Callback::from(move |_: FocusEvent| {
                                                            let so = so.clone();
                                                            gloo_timers::callback::Timeout::new(200, move || {
                                                                so.set(false);
                                                            }).forget();
                                                        })
                                                    }}
                                                />
                                                if *tc_chat_search_open {
                                                    <div class="rb-autocomplete">
                                                        if *tc_chat_searching {
                                                            <div class="rb-autocomplete-item" style="color: #666;">{"Searching..."}</div>
                                                        } else if tc_chat_search_results.is_empty() {
                                                            <div class="rb-autocomplete-item" style="color: #666;">{"No chats found"}</div>
                                                        } else {
                                                            {for tc_chat_search_results.iter().map(|(name, room_id, _person)| {
                                                                let display_name = name.clone();
                                                                let set_name = name.clone();
                                                                let set_rid = room_id.clone();
                                                                let cn = tc_chat_name.clone();
                                                                let rid = tc_chat_room_id.clone();
                                                                let so = tc_chat_search_open.clone();
                                                                html! {
                                                                    <div class="rb-autocomplete-item"
                                                                        onmousedown={Callback::from(move |e: MouseEvent| {
                                                                            e.prevent_default();
                                                                            cn.set(set_name.clone());
                                                                            rid.set(Some(set_rid.clone()));
                                                                            so.set(false);
                                                                        })}
                                                                    >
                                                                        {display_name}
                                                                    </div>
                                                                }
                                                            })}
                                                        }
                                                    </div>
                                                }
                                            </div>
                                            <div class="rb-field">
                                                <div class="rb-field-label">{"Message"}</div>
                                                <textarea
                                                    class="rb-textarea"
                                                    placeholder="Message to send..."
                                                    value={(*tc_message).clone()}
                                                    oninput={{
                                                        let s = tc_message.clone();
                                                        Callback::from(move |e: InputEvent| {
                                                            if let Some(input) = e.target_dyn_into::<web_sys::HtmlTextAreaElement>() {
                                                                s.set(input.value());
                                                            }
                                                        })
                                                    }}
                                                ></textarea>
                                            </div>
                                        }

                                        if *tool_name == "send_email" {
                                            <div class="rb-field">
                                                <div class="rb-field-label">{"To"}</div>
                                                <input
                                                    class="rb-input"
                                                    type="text"
                                                    placeholder="recipient@example.com"
                                                    value={(*tc_email_to).clone()}
                                                    oninput={{
                                                        let s = tc_email_to.clone();
                                                        Callback::from(move |e: InputEvent| {
                                                            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                                                s.set(input.value());
                                                            }
                                                        })
                                                    }}
                                                />
                                            </div>
                                            <div class="rb-field">
                                                <div class="rb-field-label">{"Subject"}</div>
                                                <input
                                                    class="rb-input"
                                                    type="text"
                                                    placeholder="Email subject..."
                                                    value={(*tc_email_subject).clone()}
                                                    oninput={{
                                                        let s = tc_email_subject.clone();
                                                        Callback::from(move |e: InputEvent| {
                                                            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                                                s.set(input.value());
                                                            }
                                                        })
                                                    }}
                                                />
                                            </div>
                                            <div class="rb-field">
                                                <div class="rb-field-label">{"Body"}</div>
                                                <textarea
                                                    class="rb-textarea"
                                                    placeholder="Email body..."
                                                    value={(*tc_email_body).clone()}
                                                    oninput={{
                                                        let s = tc_email_body.clone();
                                                        Callback::from(move |e: InputEvent| {
                                                            if let Some(input) = e.target_dyn_into::<web_sys::HtmlTextAreaElement>() {
                                                                s.set(input.value());
                                                            }
                                                        })
                                                    }}
                                                ></textarea>
                                            </div>
                                        }

                                        if *tool_name == "respond_to_email" {
                                            <div class="rb-field-hint" style="margin-bottom: 0.5rem;">
                                                {"Replies to the email that triggered this rule. Use with an Event trigger filtered to emails."}
                                            </div>
                                            <div class="rb-field">
                                                <div class="rb-field-label">{"Response"}</div>
                                                <textarea
                                                    class="rb-textarea"
                                                    placeholder="Reply text, or leave empty for AI-generated response..."
                                                    value={(*tc_reply_text).clone()}
                                                    oninput={{
                                                        let s = tc_reply_text.clone();
                                                        Callback::from(move |e: InputEvent| {
                                                            if let Some(input) = e.target_dyn_into::<web_sys::HtmlTextAreaElement>() {
                                                                s.set(input.value());
                                                            }
                                                        })
                                                    }}
                                                ></textarea>
                                            </div>
                                        }

                                        if *tool_name == "control_tesla" {
                                            <div class="rb-field">
                                                <div class="rb-field-label">{"Command"}</div>
                                                <select
                                                    class="rb-select"
                                                    onchange={{
                                                        let s = tc_tesla_cmd.clone();
                                                        Callback::from(move |e: Event| {
                                                            if let Some(sel) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                                                                s.set(sel.value());
                                                            }
                                                        })
                                                    }}
                                                >
                                                    {for [
                                                        ("lock", "Lock"),
                                                        ("unlock", "Unlock"),
                                                        ("climate_on", "Climate on"),
                                                        ("climate_off", "Climate off"),
                                                        ("defrost", "Defrost"),
                                                        ("remote_start", "Remote start"),
                                                        ("charge_status", "Charge status"),
                                                        ("precondition_battery", "Precondition battery"),
                                                    ].iter().map(|(val, label)| {
                                                        html! { <option value={*val} selected={*tc_tesla_cmd == *val}>{label}</option> }
                                                    })}
                                                </select>
                                            </div>
                                        }

                                        if *tool_name == "create_event" {
                                            <div class="rb-field-hint" style="margin-bottom: 0.5rem;">
                                                {"Creates one tracked obligation with a real due time and reminder time. Use for a concrete commitment, not a whole situation."}
                                            </div>
                                        }

                                        if *tool_name == "update_event" {
                                            <div class="rb-field-hint" style="margin-bottom: 0.5rem;">
                                                {"Updates a tracked obligation by appending new context and adjusting status, reminder time, or due time."}
                                            </div>
                                        }

                                        // MCP tool params
                                        if tool_name.starts_with("mcp:") {
                                            {mcp_fields_html.clone()}
                                        }
                                    }
                                </>
                            })}
                        )}

                        // ELSE section (only when there IS a condition)
                        if *logic_mode != LogicMode::Always {
                            <div class="rb-else-divider">
                                <span>{"OTHERWISE"}</span>
                                <div class="rb-else-divider-line"></div>
                            </div>
                            if (*else_flow).is_some() {
                                <div class="rb-else-content">
                                    <NestedConditionEditor
                                        node={(*else_flow).as_ref().unwrap().clone()}
                                        on_update={{
                                            let ef = else_flow.clone();
                                            Callback::from(move |new_node: Option<FlowNode>| {
                                                ef.set(new_node);
                                            })
                                        }}
                                        depth={1_usize}
                                        available_sources={(*available_sources).clone()}
                                        when_mode={(*when_mode).clone()}
                                    />
                                </div>
                            } else {
                                <div style="display: flex; gap: 0.5rem; align-items: center;">
                                    <span class="rb-do-nothing">{"Skip - no action"}</span>
                                    <button class="rb-add-condition-btn"
                                        onclick={{
                                            let ef = else_flow.clone();
                                            Callback::from(move |_: MouseEvent| {
                                                ef.set(Some(FlowNode::LlmCondition {
                                                    prompt: String::new(),
                                                    fetch: vec![],
                                                    true_branch: Box::new(Some(FlowNode::Action {
                                                        action_type: "notify".to_string(),
                                                        config: serde_json::json!({"method": "sms"}),
                                                    })),
                                                    false_branch: Box::new(None),
                                                }));
                                            })
                                        }}
                                    >
                                        {"+ Add a check"}
                                    </button>
                                </div>
                            }
                        }

                        // Review section
                        {render_review(
                            &when_summary,
                            &*logic_mode,
                            &*selected_template,
                            &*logic_prompt,
                            &*condition_input,
                            &*keyword_input,
                            &*action_mode,
                            &*notify_method,
                            &*tool_name,
                            &*tc_platform,
                            &*tc_chat_name,
                            &*tc_tesla_cmd,
                            &*else_flow,
                            &rule_missing,
                        )}

                        // Test panel
                        <button
                            class={if *test_open { "rb-test-toggle open" } else { "rb-test-toggle" }}
                            onclick={{
                                let test_open = test_open.clone();
                                Callback::from(move |_: MouseEvent| {
                                    test_open.set(!*test_open);
                                })
                            }}
                        >
                            {if *test_open { "Hide test panel" } else { "Test this rule" }}
                        </button>

                        if *test_open {
                            <div class="rb-test-panel">
                                <div class="rb-field-label">{"Sample message"}</div>
                                <div class="rb-test-presets">
                                    {{
                                        let presets: Vec<(&str, &str)> = vec![
                                            ("Hey, can we meet tomorrow at 3pm?", "Alex"),
                                            ("URGENT: Server is down!", "DevOps Alert"),
                                            ("Click here for free iPhone!", "Unknown"),
                                            ("Flight AA1234 delayed 2hrs", "Airline"),
                                        ];
                                        presets.into_iter().map(|(msg, sender)| {
                                            let test_message = test_message.clone();
                                            let test_sender = test_sender.clone();
                                            let msg_str = msg.to_string();
                                            let sender_str = sender.to_string();
                                            let is_active = *test_message == msg_str;
                                            html! {
                                                <button
                                                    class={if is_active { "rb-test-preset active" } else { "rb-test-preset" }}
                                                    onclick={{
                                                        let test_message = test_message.clone();
                                                        let test_sender = test_sender.clone();
                                                        let m = msg_str.clone();
                                                        let s = sender_str.clone();
                                                        Callback::from(move |_: MouseEvent| {
                                                            test_message.set(m.clone());
                                                            test_sender.set(s.clone());
                                                        })
                                                    }}
                                                >
                                                    {format!("{}: {}", sender, if msg.len() > 28 { &msg[..28] } else { msg })}
                                                </button>
                                            }
                                        }).collect::<Html>()
                                    }}
                                </div>
                                <div class="rb-field">
                                    <div class="rb-field-label">{"Sender"}</div>
                                    <input
                                        class="rb-input"
                                        type="text"
                                        value={(*test_sender).clone()}
                                        oninput={{
                                            let test_sender = test_sender.clone();
                                            Callback::from(move |e: InputEvent| {
                                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                                    test_sender.set(input.value());
                                                }
                                            })
                                        }}
                                    />
                                </div>
                                <div class="rb-field">
                                    <div class="rb-field-label">{"Message"}</div>
                                    <textarea
                                        class="rb-textarea"
                                        value={(*test_message).clone()}
                                        oninput={{
                                            let test_message = test_message.clone();
                                            Callback::from(move |e: InputEvent| {
                                                if let Some(input) = e.target_dyn_into::<web_sys::HtmlTextAreaElement>() {
                                                    test_message.set(input.value());
                                                }
                                            })
                                        }}
                                        placeholder="Type a test message or pick a preset above..."
                                    />
                                </div>
                                <button
                                    class="rb-test-run"
                                    disabled={*test_running || (*test_message).is_empty() || !rule_complete}
                                    onclick={{
                                        let test_running = test_running.clone();
                                        let test_steps = test_steps.clone();
                                        let test_message = test_message.clone();
                                        let test_sender = test_sender.clone();
                                        let name = name.clone();
                                        let logic_mode = logic_mode.clone();
                                        let selected_template = selected_template.clone();
                                        let logic_prompt = logic_prompt.clone();
                                        let condition_input = condition_input.clone();
                                        let keyword_input = keyword_input.clone();
                                        let active_sources = active_sources.clone();
                                        let action_mode = action_mode.clone();
                                        let notify_method = notify_method.clone();
                                        let notify_message = notify_message.clone();
                                        let tool_name = tool_name.clone();
                                        let tc_platform = tc_platform.clone();
                                        let tc_chat_name = tc_chat_name.clone();
                                        let tc_chat_room_id = tc_chat_room_id.clone();
                                        let tc_message = tc_message.clone();
                                        let tc_email_to = tc_email_to.clone();
                                        let tc_email_subject = tc_email_subject.clone();
                                        let tc_email_body = tc_email_body.clone();
                                        let tc_reply_text = tc_reply_text.clone();
                                        let tc_tesla_cmd = tc_tesla_cmd.clone();
                                        let tc_mcp_params = tc_mcp_params.clone();
                                        let else_flow = else_flow.clone();
                                        let when_mode = when_mode.clone();
                                        let test_es_ref = test_es_ref.clone();
                                        Callback::from(move |_: MouseEvent| {
                                            // Close any previous EventSource
                                            if let Some(old_es) = test_es_ref.borrow_mut().take() {
                                                old_es.close();
                                            }
                                            test_running.set(true);
                                            test_steps.set(vec![]);

                                            // Build flow_config (same logic as on_submit)
                                            let action_config_json = match *action_mode {
                                                ActionMode::Notify => {
                                                    let method = match *notify_method {
                                                        NotifyMethod::Sms => "sms",
                                                        NotifyMethod::Call => "call",
                                                    };
                                                    let mut c = serde_json::json!({ "method": method });
                                                    let msg = (*notify_message).clone();
                                                    if !msg.is_empty() { c["message"] = serde_json::json!(msg); }
                                                    c
                                                }
                                                ActionMode::ToolCall => {
                                                    let tn = (*tool_name).clone();
                                                    let params = match tn.as_str() {
                                                        "send_chat_message" => {
                                                            let mut p = serde_json::json!({
                                                                "platform": *tc_platform,
                                                                "chat_name": *tc_chat_name,
                                                                "message": *tc_message,
                                                            });
                                                            if let Some(ref rid) = *tc_chat_room_id {
                                                                p["room_id"] = serde_json::json!(rid);
                                                            }
                                                            p
                                                        },
                                                        "send_email" => serde_json::json!({
                                                            "to": *tc_email_to,
                                                            "subject": *tc_email_subject,
                                                            "body": *tc_email_body,
                                                        }),
                                                        "respond_to_email" => serde_json::json!({
                                                            "response_text": *tc_reply_text,
                                                        }),
                                                        "control_tesla" => serde_json::json!({
                                                            "command": *tc_tesla_cmd,
                                                        }),
                                                        "create_event" => serde_json::json!({}),
                                                        "update_event" => serde_json::json!({}),
                                                        _ => {
                                                            let map = &*tc_mcp_params;
                                                            let obj: serde_json::Map<String, serde_json::Value> = map.iter()
                                                                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                                                                .collect();
                                                            serde_json::Value::Object(obj)
                                                        }
                                                    };
                                                    serde_json::json!({ "tool": tn, "params": params })
                                                }
                                            };
                                            let action_type = match *action_mode {
                                                ActionMode::Notify => "notify",
                                                ActionMode::ToolCall => "tool_call",
                                            };
                                            let action_node = serde_json::json!({
                                                "type": "action",
                                                "action_type": action_type,
                                                "config": action_config_json
                                            });
                                            let else_branch_json: serde_json::Value = match &*else_flow {
                                                Some(node) => serde_json::to_value(node).unwrap_or(serde_json::Value::Null),
                                                None => serde_json::Value::Null,
                                            };

                                            let lp = match *logic_mode {
                                                LogicMode::Keyword => {
                                                    let k = (*keyword_input).clone();
                                                    if k.is_empty() { None } else { Some(k) }
                                                }
                                                LogicMode::Llm => {
                                                    match *selected_template {
                                                        PromptTemplate::Custom => {
                                                            let v = (*logic_prompt).clone();
                                                            if v.is_empty() { None } else { Some(v) }
                                                        }
                                                        PromptTemplate::CheckCondition => {
                                                            let ci = (*condition_input).clone();
                                                            if ci.is_empty() { None } else { Some(format!("template:check_condition:{}", ci)) }
                                                        }
                                                        PromptTemplate::Summarize => Some("template:summarize".to_string()),
                                                        PromptTemplate::FilterImportant => Some("template:filter_important".to_string()),
                                                        PromptTemplate::TrackItemsUpdate => Some("template:track_items_update".to_string()),
                                                        PromptTemplate::TrackItemsCreate => Some("template:track_items_create".to_string()),
                                                    }
                                                }
                                                _ => None,
                                            };
                                            let lf: Option<String> = if *logic_mode == LogicMode::Llm && !active_sources.is_empty() {
                                                Some(serde_json::to_string(&*active_sources).unwrap_or_default())
                                            } else {
                                                None
                                            };

                                            let flow_config = match *logic_mode {
                                                LogicMode::Llm => {
                                                    let fetch_sources: serde_json::Value = lf.as_deref()
                                                        .and_then(|f| serde_json::from_str(f).ok())
                                                        .unwrap_or(serde_json::json!([]));
                                                    serde_json::json!({
                                                        "type": "llm_condition",
                                                        "prompt": lp.as_deref().unwrap_or("Evaluate and decide"),
                                                        "fetch": fetch_sources,
                                                        "true_branch": action_node,
                                                        "false_branch": else_branch_json
                                                    })
                                                }
                                                LogicMode::Keyword => {
                                                    serde_json::json!({
                                                        "type": "keyword_condition",
                                                        "keyword": lp.as_deref().unwrap_or(""),
                                                        "true_branch": action_node,
                                                        "false_branch": else_branch_json
                                                    })
                                                }
                                                _ => action_node,
                                            };

                                            let body = serde_json::json!({
                                                "flow_config": flow_config.to_string(),
                                                "message": *test_message,
                                                "sender": *test_sender,
                                                "rule_name": *name,
                                            });

                                            let test_running = test_running.clone();
                                            let test_steps = test_steps.clone();
                                            let test_es_ref = test_es_ref.clone();

                                            spawn_local(async move {
                                                // Pre-flight auth refresh
                                                let _ = Api::get("/api/auth/status").send().await;

                                                // POST to get test_id
                                                let request = match Api::post("/api/rules/test").json(&body) {
                                                    Ok(r) => r,
                                                    Err(_) => {
                                                        test_steps.set(vec![("fail".into(), "x".into(), "Failed to build request".into())]);
                                                        test_running.set(false);
                                                        return;
                                                    }
                                                };
                                                let response = match request.send().await {
                                                    Ok(r) => r,
                                                    Err(_) => {
                                                        test_steps.set(vec![("fail".into(), "x".into(), "Network error".into())]);
                                                        test_running.set(false);
                                                        return;
                                                    }
                                                };
                                                if !response.ok() {
                                                    let msg = response.text().await.unwrap_or_else(|_| "Request failed".into());
                                                    test_steps.set(vec![("fail".into(), "x".into(), msg)]);
                                                    test_running.set(false);
                                                    return;
                                                }
                                                let data: serde_json::Value = match response.json().await {
                                                    Ok(d) => d,
                                                    Err(_) => {
                                                        test_steps.set(vec![("fail".into(), "x".into(), "Invalid response".into())]);
                                                        test_running.set(false);
                                                        return;
                                                    }
                                                };
                                                let test_id = data["test_id"].as_str().unwrap_or("").to_string();
                                                if test_id.is_empty() {
                                                    test_steps.set(vec![("fail".into(), "x".into(), "No test_id returned".into())]);
                                                    test_running.set(false);
                                                    return;
                                                }

                                                // Open SSE stream
                                                let url = format!("{}/api/rules/test-stream?test_id={}", crate::config::get_backend_url(), test_id);
                                                use wasm_bindgen::JsCast;
                                                use wasm_bindgen::closure::Closure;

                                                let mut init = web_sys::EventSourceInit::new();
                                                init.set_with_credentials(true);
                                                let es = match web_sys::EventSource::new_with_event_source_init_dict(&url, &init) {
                                                    Ok(es) => es,
                                                    Err(_) => {
                                                        test_steps.set(vec![("fail".into(), "x".into(), "Failed to open stream".into())]);
                                                        test_running.set(false);
                                                        return;
                                                    }
                                                };

                                                // Store ref so next run can close it
                                                *test_es_ref.borrow_mut() = Some(es.clone());

                                                let steps_handle = test_steps.clone();
                                                let running_handle = test_running.clone();
                                                let es_ref = es.clone();

                                                // Fresh accumulator per run - avoids stale Yew state reads
                                                let acc = std::rc::Rc::new(std::cell::RefCell::new(Vec::<(String, String, String)>::new()));
                                                let acc_msg = acc.clone();

                                                let onmessage = Closure::wrap(Box::new(move |event: web_sys::MessageEvent| {
                                                    if let Some(data_str) = event.data().as_string() {
                                                        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&data_str) {
                                                            let step = data["step"].as_str().unwrap_or("");
                                                            let (css, icon, text) = match step {
                                                                "prefetching" => {
                                                                    let sources = data["sources"].as_array()
                                                                        .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", "))
                                                                        .unwrap_or_default();
                                                                    ("deciding".to_string(), "...".to_string(), format!("Fetching {}...", sources))
                                                                }
                                                                "evaluating_llm" => {
                                                                    let preview = data["prompt_preview"].as_str().unwrap_or("").to_string();
                                                                    ("deciding".to_string(), "...".to_string(), format!("Evaluating AI condition: {}", preview))
                                                                }
                                                                "llm_result" => {
                                                                    let decided = data["decided"].as_bool().unwrap_or(false);
                                                                    if decided {
                                                                        let msg = data["message"].as_str().unwrap_or("").to_string();
                                                                        let text = if msg.is_empty() {
                                                                            "AI decided: Yes".to_string()
                                                                        } else {
                                                                            format!("AI decided: Yes - \"{}\"", msg)
                                                                        };
                                                                        ("yes".to_string(), "Y".to_string(), text)
                                                                    } else {
                                                                        ("no".to_string(), "-".to_string(), "AI decided: No".to_string())
                                                                    }
                                                                }
                                                                "checking_keyword" => {
                                                                    let kw = data["keyword"].as_str().unwrap_or("").to_string();
                                                                    ("deciding".to_string(), "...".to_string(), format!("Checking for keyword '{}'...", kw))
                                                                }
                                                                "keyword_result" => {
                                                                    let matched = data["matched"].as_bool().unwrap_or(false);
                                                                    if matched {
                                                                        ("yes".to_string(), "Y".to_string(), "Keyword matched".to_string())
                                                                    } else {
                                                                        ("no".to_string(), "-".to_string(), "No match".to_string())
                                                                    }
                                                                }
                                                                "would_execute" => {
                                                                    let desc = data["description"].as_str().unwrap_or("").to_string();
                                                                    ("action".to_string(), ">".to_string(), format!("Would {}", desc))
                                                                }
                                                                "no_action" => {
                                                                    let reason = data["reason"].as_str().unwrap_or("").to_string();
                                                                    ("inactive".to_string(), "-".to_string(), format!("No action: {}", reason))
                                                                }
                                                                "error" => {
                                                                    let msg = data["message"].as_str().unwrap_or("Unknown error").to_string();
                                                                    ("fail".to_string(), "x".to_string(), msg)
                                                                }
                                                                "complete" => {
                                                                    running_handle.set(false);
                                                                    es_ref.close();
                                                                    return;
                                                                }
                                                                _ => return,
                                                            };
                                                            acc_msg.borrow_mut().push((css, icon, text));
                                                            steps_handle.set(acc_msg.borrow().clone());
                                                        }
                                                    }
                                                }) as Box<dyn FnMut(web_sys::MessageEvent)>);

                                                es.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
                                                onmessage.forget();

                                                // onerror
                                                let running_err = test_running.clone();
                                                let steps_err = test_steps.clone();
                                                let es_err = es.clone();
                                                let acc_err = acc.clone();
                                                let onerror = Closure::wrap(Box::new(move |_: web_sys::Event| {
                                                    acc_err.borrow_mut().push(("fail".into(), "x".into(), "Stream connection error".into()));
                                                    steps_err.set(acc_err.borrow().clone());
                                                    running_err.set(false);
                                                    es_err.close();
                                                }) as Box<dyn FnMut(web_sys::Event)>);

                                                es.set_onerror(Some(onerror.as_ref().unchecked_ref()));
                                                onerror.forget();
                                            });
                                        })
                                    }}
                                >
                                    {if *test_running { "Running..." } else { "Run Test" }}
                                </button>

                                if !test_steps.is_empty() {
                                    <div class="rb-test-steps">
                                        {for (*test_steps).iter().map(|(css, icon, text)| {
                                            html! {
                                                <div class={format!("rb-test-step {}", css)}>
                                                    <span class="rb-test-step-icon">{icon}</span>
                                                    <span>{text}</span>
                                                </div>
                                            }
                                        })}
                                    </div>
                                }

                                <div class="rb-test-cost-hint">{"Uses 1 credit (same as web chat)"}</div>
                            </div>
                        }

                        if let Some(ref err) = *error_msg {
                            <div class="rb-error">{err}</div>
                        }

                        <button
                            class="rb-submit"
                            onclick={on_submit}
                            disabled={*saving || !rule_complete}
                        >
                            {if *saving {
                                "Saving..."
                            } else if is_editing {
                                "Save Changes"
                            } else {
                                "Create Rule"
                            }}
                        </button>
                    </div>
                </div>
            </div>
        </>
    }
}

// ---------------------------------------------------------------------------
// NestedConditionEditor: a proper Yew function_component for nested IF+THEN
// ---------------------------------------------------------------------------

#[derive(Properties, PartialEq)]
struct NestedConditionEditorProps {
    node: FlowNode,
    on_update: Callback<Option<FlowNode>>,
    depth: usize,
    available_sources: Vec<RuleSourceOption>,
    when_mode: WhenMode,
}

#[function_component(NestedConditionEditor)]
fn nested_condition_editor(props: &NestedConditionEditorProps) -> Html {
    let depth = props.depth;
    let on_update = &props.on_update;

    // --- Derive all data state from the FlowNode prop ---
    let mode = match &props.node {
        FlowNode::LlmCondition { .. } => "llm",
        FlowNode::KeywordCondition { .. } => "keyword",
        FlowNode::Action { .. } => "always",
    };

    let (prompt, keyword, fetch, true_branch, false_branch) = match &props.node {
        FlowNode::LlmCondition {
            prompt,
            fetch,
            true_branch,
            false_branch,
        } => (
            prompt.clone(),
            String::new(),
            fetch.clone(),
            true_branch.clone(),
            false_branch.clone(),
        ),
        FlowNode::KeywordCondition {
            keyword,
            true_branch,
            false_branch,
        } => (
            String::new(),
            keyword.clone(),
            vec![],
            true_branch.clone(),
            false_branch.clone(),
        ),
        FlowNode::Action {
            action_type,
            config,
        } => {
            let tb = Box::new(Some(FlowNode::Action {
                action_type: action_type.clone(),
                config: config.clone(),
            }));
            (String::new(), String::new(), vec![], tb, Box::new(None))
        }
    };

    let (cur_action_type, cur_action_config) = match true_branch.as_ref() {
        Some(FlowNode::Action {
            action_type,
            config,
        }) => (action_type.clone(), config.clone()),
        _ => ("notify".to_string(), serde_json::json!({"method": "sms"})),
    };
    let cur_method = cur_action_config
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("sms")
        .to_string();
    let cur_tool = cur_action_config
        .get("tool")
        .and_then(|v| v.as_str())
        .unwrap_or("create_event")
        .to_string();
    let cur_notify_msg = cur_action_config
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Derive per-tool params from action config
    let cur_params = cur_action_config
        .get("params")
        .cloned()
        .unwrap_or(serde_json::json!({}));
    let cur_tc_platform = cur_params
        .get("platform")
        .and_then(|v| v.as_str())
        .unwrap_or("whatsapp")
        .to_string();
    let cur_tc_chat_name = cur_params
        .get("chat_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let cur_tc_message = cur_params
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let cur_tc_email_to = cur_params
        .get("to")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let cur_tc_email_subject = cur_params
        .get("subject")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let cur_tc_email_body = cur_params
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let cur_tc_reply_text = cur_params
        .get("response_text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let cur_tc_tesla_cmd = cur_params
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("lock")
        .to_string();
    // Derive template/condition from prompt
    let (selected_template, condition_input_val) = if prompt.is_empty() {
        (PromptTemplate::Summarize, String::new())
    } else if prompt == "template:summarize" {
        (PromptTemplate::Summarize, String::new())
    } else if prompt == "template:filter_important" {
        (PromptTemplate::FilterImportant, String::new())
    } else if prompt == "template:track_items_update" {
        (PromptTemplate::TrackItemsUpdate, String::new())
    } else if prompt == "template:track_items_create" {
        (PromptTemplate::TrackItemsCreate, String::new())
    } else if prompt.starts_with("template:check_condition:") {
        let cond = prompt.strip_prefix("template:check_condition:").unwrap_or("").to_string();
        (PromptTemplate::CheckCondition, cond)
    } else {
        (PromptTemplate::Custom, String::new())
    };

    // --- UI-only state hooks ---
    let expanded_card = use_state(|| None::<Card>);
    // --- Summaries for render_card ---
    let if_summary = match mode {
        "always" => "always run".to_string(),
        "keyword" => {
            if keyword.is_empty() {
                "keyword match".to_string()
            } else if keyword.len() > 20 {
                format!("contains '{}...'", &keyword[..20])
            } else {
                format!("contains '{}'", keyword)
            }
        }
        "llm" => {
            if prompt.is_empty() {
                "AI evaluates".to_string()
            } else if prompt.len() > 30 {
                format!("AI: {}...", &prompt[..30])
            } else {
                format!("AI: {}", prompt)
            }
        }
        _ => "condition".to_string(),
    };

    let then_summary = match cur_action_type.as_str() {
        "notify" => match cur_method.as_str() {
            "call" => "notify via call".to_string(),
            _ => "notify via SMS".to_string(),
        },
        "tool_call" => match cur_tool.as_str() {
            "send_chat_message" => {
                let plat = capitalize_first(&cur_tc_platform);
                if cur_tc_chat_name.is_empty() {
                    format!("{} message", plat)
                } else {
                    format!("{} msg {}", plat, cur_tc_chat_name)
                }
            }
            "send_email" => "send email".to_string(),
            "respond_to_email" => "reply to email".to_string(),
            "control_tesla" => humanize_tesla_cmd_short(&cur_tc_tesla_cmd).to_string(),
            "create_event" => "create event".to_string(),
            "update_event" => "update event".to_string(),
            t if t.starts_with("mcp:") => {
                let parts: Vec<&str> = t.splitn(3, ':').collect();
                let tool_short = parts.get(2).unwrap_or(&"tool");
                format!("MCP: {}", tool_short)
            }
            t => {
                if t.is_empty() {
                    "run tool".to_string()
                } else {
                    format!("run: {}", t)
                }
            }
        },
        _ => "action".to_string(),
    };

    // --- Card toggle ---
    let toggle_card = {
        let expanded_card = expanded_card.clone();
        move |card: Card| {
            let expanded_card = expanded_card.clone();
            Callback::from(move |_: MouseEvent| {
                if *expanded_card == Some(card) {
                    expanded_card.set(None);
                } else {
                    expanded_card.set(Some(card));
                }
            })
        }
    };

    // --- Remove button ---
    let on_remove = {
        let ou = on_update.clone();
        Callback::from(move |_: MouseEvent| {
            ou.emit(None);
        })
    };

    // --- IF card content ---
    let if_content = {
        // Mode switch callbacks
        let make_mode_switch = |new_mode: &'static str| {
            let ou = on_update.clone();
            let tb = true_branch.clone();
            Callback::from(move |_: MouseEvent| {
                let new_node = match new_mode {
                    "always" => match tb.as_ref() {
                        Some(action) => action.clone(),
                        None => FlowNode::Action {
                            action_type: "notify".to_string(),
                            config: serde_json::json!({"method": "sms"}),
                        },
                    },
                    "keyword" => FlowNode::KeywordCondition {
                        keyword: String::new(),
                        true_branch: tb.clone(),
                        false_branch: Box::new(None),
                    },
                    "llm" => FlowNode::LlmCondition {
                        prompt: String::new(),
                        fetch: vec![],
                        true_branch: tb.clone(),
                        false_branch: Box::new(None),
                    },
                    _ => return,
                };
                ou.emit(Some(new_node));
            })
        };
        let on_mode_always = make_mode_switch("always");
        let on_mode_keyword = make_mode_switch("keyword");
        let on_mode_llm = make_mode_switch("llm");

        // Keyword change
        let on_keyword_change = {
            let ou = on_update.clone();
            let tb = true_branch.clone();
            let fb = false_branch.clone();
            Callback::from(move |e: InputEvent| {
                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                    ou.emit(Some(FlowNode::KeywordCondition {
                        keyword: input.value(),
                        true_branch: tb.clone(),
                        false_branch: fb.clone(),
                    }));
                }
            })
        };

        // Template buttons
        let template_buttons = {
            let templates = [
                (PromptTemplate::Summarize, "Summarize"),
                (PromptTemplate::CheckCondition, "Check condition"),
                (PromptTemplate::Custom, "Custom"),
            ];
            templates.iter().map(|(tmpl, label)| {
                let is_active = selected_template == *tmpl;
                let tmpl_clone = tmpl.clone();
                let ou = on_update.clone();
                let f = fetch.clone();
                let tb = true_branch.clone();
                let fb = false_branch.clone();
                let wm = props.when_mode.clone();
                html! {
                    <button
                        class={classes!("rb-template-btn", is_active.then(|| "active"))}
                        onclick={Callback::from(move |_: MouseEvent| {
                            let new_prompt = match &tmpl_clone {
                                PromptTemplate::Custom => String::new(),
                                PromptTemplate::Summarize => "template:summarize".to_string(),
                                PromptTemplate::FilterImportant => "template:filter_important".to_string(),
                                PromptTemplate::TrackItemsUpdate => "template:track_items_update".to_string(),
                                PromptTemplate::TrackItemsCreate => "template:track_items_create".to_string(),
                                PromptTemplate::CheckCondition => "template:check_condition:".to_string(),
                            };
                            let new_fetch = match &tmpl_clone {
                                PromptTemplate::Summarize => match wm {
                                    WhenMode::Schedule => vec![SourceConfig::Email, SourceConfig::Chat { platform: "all".to_string(), limit: 50 }, SourceConfig::Events],
                                    WhenMode::Event => vec![SourceConfig::Chat { platform: "all".to_string(), limit: 50 }],
                                },
                                PromptTemplate::FilterImportant => match wm {
                                    WhenMode::Schedule => vec![SourceConfig::Email, SourceConfig::Chat { platform: "all".to_string(), limit: 50 }],
                                    WhenMode::Event => vec![],
                                },
                                PromptTemplate::TrackItemsCreate | PromptTemplate::TrackItemsUpdate => vec![SourceConfig::Events],
                                _ => f.clone(),
                            };
                            ou.emit(Some(FlowNode::LlmCondition {
                                prompt: new_prompt,
                                fetch: new_fetch,
                                true_branch: tb.clone(),
                                false_branch: fb.clone(),
                            }));
                        })}
                    >{label}</button>
                }
            }).collect::<Vec<Html>>()
        };

        // Prompt change
        let on_prompt_change = {
            let ou = on_update.clone();
            let f = fetch.clone();
            let tb = true_branch.clone();
            let fb = false_branch.clone();
            Callback::from(move |e: InputEvent| {
                if let Some(input) = e.target_dyn_into::<web_sys::HtmlTextAreaElement>() {
                    ou.emit(Some(FlowNode::LlmCondition {
                        prompt: input.value(),
                        fetch: f.clone(),
                        true_branch: tb.clone(),
                        false_branch: fb.clone(),
                    }));
                }
            })
        };

        // Condition input change (for CheckCondition template)
        let on_condition_change = {
            let ou = on_update.clone();
            let f = fetch.clone();
            let tb = true_branch.clone();
            let fb = false_branch.clone();
            Callback::from(move |e: InputEvent| {
                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                    let new_prompt = format!("template:check_condition:{}", input.value());
                    ou.emit(Some(FlowNode::LlmCondition {
                        prompt: new_prompt,
                        fetch: f.clone(),
                        true_branch: tb.clone(),
                        false_branch: fb.clone(),
                    }));
                }
            })
        };

        // Edit prompt link
        let on_edit_prompt = {
            let ou = on_update.clone();
            let f = fetch.clone();
            let tb = true_branch.clone();
            let fb = false_branch.clone();
            let wm = props.when_mode.clone();
            let current_template = selected_template.clone();
            Callback::from(move |_: MouseEvent| {
                let generated = get_template_prompt(&current_template, &wm, "");
                ou.emit(Some(FlowNode::LlmCondition {
                    prompt: generated,
                    fetch: f.clone(),
                    true_branch: tb.clone(),
                    false_branch: fb.clone(),
                }));
            })
        };

        // Source pills
        let source_pills_html = if mode == "llm" {
            let pills: Vec<Html> = props.available_sources.iter().filter(|s| s.source_type != "mcp").map(|src| {
                let st = src.source_type.clone();
                let label = src.label.clone();
                let avail = src.available;
                let is_active = fetch.iter().any(|s| s.is_type(&st));
                let pill_class = if !avail { "rb-source-pill disabled" } else if is_active { "rb-source-pill active" } else { "rb-source-pill" };

                let ou = on_update.clone();
                let pv = prompt.clone();
                let fetch_c = fetch.clone();
                let tb = true_branch.clone();
                let fb = false_branch.clone();
                let st_c = st.clone();
                let onclick = Callback::from(move |_: MouseEvent| {
                    if !avail { return; }
                    let mut new_fetch = fetch_c.clone();
                    if is_active {
                        new_fetch.retain(|s| !s.is_type(&st_c));
                    } else {
                        let new_source = match st_c.as_str() {
                            "email" => SourceConfig::Email,
                            "chat" => SourceConfig::Chat { platform: "all".to_string(), limit: 50 },
                            "weather" => SourceConfig::Weather { location: String::new() },
                            "internet" => SourceConfig::Internet { query: String::new() },
                            "tesla" => SourceConfig::Tesla,
                            "events" => SourceConfig::Events,
                            _ => return,
                        };
                        new_fetch.push(new_source);
                    }
                    ou.emit(Some(FlowNode::LlmCondition {
                        prompt: pv.clone(),
                        fetch: new_fetch,
                        true_branch: tb.clone(),
                        false_branch: fb.clone(),
                    }));
                });

                html! {
                    <span class={pill_class} title={if !avail { "Connect in Settings" } else { "" }} onclick={onclick}>
                        {&label}
                    </span>
                }
            }).collect();
            html! {
                <div class="rb-field">
                    <div class="rb-field-label">{"Extra context for AI"}</div>
                    <div class="rb-context-hint">
                        {"AI already sees the trigger event. Add more context:"}
                    </div>
                    <div class="rb-source-pills">{for pills}</div>
                </div>
            }
        } else {
            html! {}
        };

        // Detect which template is active based on prompt content
        let is_summarize = selected_template == PromptTemplate::Summarize;
        let is_filter = selected_template == PromptTemplate::FilterImportant;
        let is_track_create = selected_template == PromptTemplate::TrackItemsCreate;
        let is_check = selected_template == PromptTemplate::CheckCondition;
        let is_custom = selected_template == PromptTemplate::Custom;

        html! {
            <>
                <div class="rb-toggle-group">
                    <button
                        class={classes!("rb-toggle-btn", (mode == "always").then(|| "active"))}
                        onclick={on_mode_always}
                    >{"Always"}</button>
                    <button
                        class={classes!("rb-toggle-btn", (mode == "keyword").then(|| "active"))}
                        onclick={on_mode_keyword}
                    >{"Keyword"}</button>
                    <button
                        class={classes!("rb-toggle-btn", (mode == "llm").then(|| "active"))}
                        onclick={on_mode_llm}
                    >{"AI decides"}</button>
                </div>
                if mode == "keyword" {
                    <div class="rb-field">
                        <div class="rb-field-label">{"Keyword"}</div>
                        <input
                            class="rb-input"
                            type="text"
                            placeholder="e.g. ?, urgent, meeting"
                            value={keyword.clone()}
                            oninput={on_keyword_change}
                        />
                    </div>
                    <div class="rb-field-hint">
                        {"Triggers when message content contains this text (case-insensitive)"}
                    </div>
                }
                if mode == "llm" {
                    <div class="rb-template-group">
                        {for template_buttons}
                    </div>

                    if is_summarize || is_filter || is_track_create {
                        <div class="rb-template-desc">
                            {get_template_description(&selected_template, &props.when_mode)}
                        </div>
                        <button class="rb-template-edit-link" onclick={on_edit_prompt}>
                            {"Customize..."}
                        </button>
                    }

                    if is_check {
                        <div class="rb-field">
                            <div class="rb-field-label">{"Condition"}</div>
                            <input
                                class="rb-input"
                                type="text"
                                placeholder="e.g. mentions a meeting or deadline"
                                value={condition_input_val.clone()}
                                oninput={on_condition_change}
                            />
                        </div>
                    }

                    if is_custom {
                        <div class="rb-field">
                            <div class="rb-field-label">{"Prompt"}</div>
                            <textarea
                                class="rb-textarea"
                                placeholder="Describe what the AI should evaluate..."
                                value={prompt.clone()}
                                oninput={on_prompt_change}
                            ></textarea>
                        </div>
                    }

                    {source_pills_html}
                }
            </>
        }
    };

    // --- THEN card content ---
    let then_content = {
        // Notify me / Run tool toggle
        let make_action_mode_cb = |atype: &'static str| {
            let ou = on_update.clone();
            let p = prompt.clone();
            let f = fetch.clone();
            let fb = false_branch.clone();
            let m = mode.to_string();
            let kw = keyword.clone();
            Callback::from(move |_: MouseEvent| {
                let action = if atype == "notify" {
                    FlowNode::Action {
                        action_type: "notify".into(),
                        config: serde_json::json!({"method":"sms"}),
                    }
                } else {
                    FlowNode::Action {
                        action_type: "tool_call".into(),
                        config: serde_json::json!({"tool":"create_event"}),
                    }
                };
                let new_node = match m.as_str() {
                    "llm" => FlowNode::LlmCondition {
                        prompt: p.clone(),
                        fetch: f.clone(),
                        true_branch: Box::new(Some(action)),
                        false_branch: fb.clone(),
                    },
                    "keyword" => FlowNode::KeywordCondition {
                        keyword: kw.clone(),
                        true_branch: Box::new(Some(action)),
                        false_branch: fb.clone(),
                    },
                    _ => action,
                };
                ou.emit(Some(new_node));
            })
        };
        let on_mode_notify = make_action_mode_cb("notify");
        let on_mode_tool = make_action_mode_cb("tool_call");

        // SMS / Call radio
        let radio_name = format!("nested-notify-method-{}", depth);
        let make_method_cb = |method: &'static str| {
            let ou = on_update.clone();
            let p = prompt.clone();
            let f = fetch.clone();
            let fb = false_branch.clone();
            let m = mode.to_string();
            let kw = keyword.clone();
            let msg = cur_notify_msg.clone();
            Callback::from(move |_: Event| {
                let mut cfg = serde_json::json!({"method": method});
                if !msg.is_empty() {
                    cfg["message"] = serde_json::json!(msg);
                }
                let action = FlowNode::Action {
                    action_type: "notify".into(),
                    config: cfg,
                };
                let new_node = match m.as_str() {
                    "llm" => FlowNode::LlmCondition {
                        prompt: p.clone(),
                        fetch: f.clone(),
                        true_branch: Box::new(Some(action)),
                        false_branch: fb.clone(),
                    },
                    "keyword" => FlowNode::KeywordCondition {
                        keyword: kw.clone(),
                        true_branch: Box::new(Some(action)),
                        false_branch: fb.clone(),
                    },
                    _ => action,
                };
                ou.emit(Some(new_node));
            })
        };
        let on_method_sms = make_method_cb("sms");
        let on_method_call = make_method_cb("call");

        // Message textarea
        let on_msg_change = {
            let ou = on_update.clone();
            let p = prompt.clone();
            let f = fetch.clone();
            let fb = false_branch.clone();
            let m = mode.to_string();
            let kw = keyword.clone();
            let meth = cur_method.clone();
            Callback::from(move |e: InputEvent| {
                if let Some(input) = e.target_dyn_into::<web_sys::HtmlTextAreaElement>() {
                    let mut cfg = serde_json::json!({"method": meth});
                    let v = input.value();
                    if !v.is_empty() {
                        cfg["message"] = serde_json::json!(v);
                    }
                    let action = FlowNode::Action {
                        action_type: "notify".into(),
                        config: cfg,
                    };
                    let new_node = match m.as_str() {
                        "llm" => FlowNode::LlmCondition {
                            prompt: p.clone(),
                            fetch: f.clone(),
                            true_branch: Box::new(Some(action)),
                            false_branch: fb.clone(),
                        },
                        "keyword" => FlowNode::KeywordCondition {
                            keyword: kw.clone(),
                            true_branch: Box::new(Some(action)),
                            false_branch: fb.clone(),
                        },
                        _ => action,
                    };
                    ou.emit(Some(new_node));
                }
            })
        };

        // Tool selector
        let on_tool_change = {
            let ou = on_update.clone();
            let p = prompt.clone();
            let f = fetch.clone();
            let fb = false_branch.clone();
            let m = mode.to_string();
            let kw = keyword.clone();
            Callback::from(move |e: Event| {
                if let Some(sel) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                    let action = FlowNode::Action {
                        action_type: "tool_call".into(),
                        config: serde_json::json!({"tool": sel.value(), "params": {}}),
                    };
                    let new_node = match m.as_str() {
                        "llm" => FlowNode::LlmCondition {
                            prompt: p.clone(),
                            fetch: f.clone(),
                            true_branch: Box::new(Some(action)),
                            false_branch: fb.clone(),
                        },
                        "keyword" => FlowNode::KeywordCondition {
                            keyword: kw.clone(),
                            true_branch: Box::new(Some(action)),
                            false_branch: fb.clone(),
                        },
                        _ => action,
                    };
                    ou.emit(Some(new_node));
                }
            })
        };

        // --- Per-tool param change helpers ---

        // send_chat_message: platform change
        let on_chat_platform_change = {
            let ou = on_update.clone();
            let p = prompt.clone();
            let f = fetch.clone();
            let fb = false_branch.clone();
            let m = mode.to_string();
            let kw = keyword.clone();
            Callback::from(move |e: Event| {
                if let Some(sel) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                    let action = FlowNode::Action {
                        action_type: "tool_call".into(),
                        config: serde_json::json!({"tool": "send_chat_message", "params": {"platform": sel.value(), "chat_name": "", "message": ""}}),
                    };
                    let new_node = match m.as_str() {
                        "llm" => FlowNode::LlmCondition {
                            prompt: p.clone(),
                            fetch: f.clone(),
                            true_branch: Box::new(Some(action)),
                            false_branch: fb.clone(),
                        },
                        "keyword" => FlowNode::KeywordCondition {
                            keyword: kw.clone(),
                            true_branch: Box::new(Some(action)),
                            false_branch: fb.clone(),
                        },
                        _ => action,
                    };
                    ou.emit(Some(new_node));
                }
            })
        };

        // send_chat_message: chat name change
        let on_chat_name_change = {
            let ou = on_update.clone();
            let p = prompt.clone();
            let f = fetch.clone();
            let fb = false_branch.clone();
            let m = mode.to_string();
            let kw = keyword.clone();
            let plat = cur_tc_platform.clone();
            let msg = cur_tc_message.clone();
            Callback::from(move |e: InputEvent| {
                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                    let action = FlowNode::Action {
                        action_type: "tool_call".into(),
                        config: serde_json::json!({"tool": "send_chat_message", "params": {"platform": plat, "chat_name": input.value(), "message": msg}}),
                    };
                    let new_node = match m.as_str() {
                        "llm" => FlowNode::LlmCondition {
                            prompt: p.clone(),
                            fetch: f.clone(),
                            true_branch: Box::new(Some(action)),
                            false_branch: fb.clone(),
                        },
                        "keyword" => FlowNode::KeywordCondition {
                            keyword: kw.clone(),
                            true_branch: Box::new(Some(action)),
                            false_branch: fb.clone(),
                        },
                        _ => action,
                    };
                    ou.emit(Some(new_node));
                }
            })
        };

        // send_chat_message: message change
        let on_chat_msg_change = {
            let ou = on_update.clone();
            let p = prompt.clone();
            let f = fetch.clone();
            let fb = false_branch.clone();
            let m = mode.to_string();
            let kw = keyword.clone();
            let plat = cur_tc_platform.clone();
            let cn = cur_tc_chat_name.clone();
            Callback::from(move |e: InputEvent| {
                if let Some(input) = e.target_dyn_into::<web_sys::HtmlTextAreaElement>() {
                    let action = FlowNode::Action {
                        action_type: "tool_call".into(),
                        config: serde_json::json!({"tool": "send_chat_message", "params": {"platform": plat, "chat_name": cn, "message": input.value()}}),
                    };
                    let new_node = match m.as_str() {
                        "llm" => FlowNode::LlmCondition {
                            prompt: p.clone(),
                            fetch: f.clone(),
                            true_branch: Box::new(Some(action)),
                            false_branch: fb.clone(),
                        },
                        "keyword" => FlowNode::KeywordCondition {
                            keyword: kw.clone(),
                            true_branch: Box::new(Some(action)),
                            false_branch: fb.clone(),
                        },
                        _ => action,
                    };
                    ou.emit(Some(new_node));
                }
            })
        };

        // send_email: to change
        let on_email_to_change = {
            let ou = on_update.clone();
            let p = prompt.clone();
            let f = fetch.clone();
            let fb = false_branch.clone();
            let m = mode.to_string();
            let kw = keyword.clone();
            let subj = cur_tc_email_subject.clone();
            let body = cur_tc_email_body.clone();
            Callback::from(move |e: InputEvent| {
                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                    let action = FlowNode::Action {
                        action_type: "tool_call".into(),
                        config: serde_json::json!({"tool": "send_email", "params": {"to": input.value(), "subject": subj, "body": body}}),
                    };
                    let new_node = match m.as_str() {
                        "llm" => FlowNode::LlmCondition {
                            prompt: p.clone(),
                            fetch: f.clone(),
                            true_branch: Box::new(Some(action)),
                            false_branch: fb.clone(),
                        },
                        "keyword" => FlowNode::KeywordCondition {
                            keyword: kw.clone(),
                            true_branch: Box::new(Some(action)),
                            false_branch: fb.clone(),
                        },
                        _ => action,
                    };
                    ou.emit(Some(new_node));
                }
            })
        };

        // send_email: subject change
        let on_email_subj_change = {
            let ou = on_update.clone();
            let p = prompt.clone();
            let f = fetch.clone();
            let fb = false_branch.clone();
            let m = mode.to_string();
            let kw = keyword.clone();
            let to = cur_tc_email_to.clone();
            let body = cur_tc_email_body.clone();
            Callback::from(move |e: InputEvent| {
                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                    let action = FlowNode::Action {
                        action_type: "tool_call".into(),
                        config: serde_json::json!({"tool": "send_email", "params": {"to": to, "subject": input.value(), "body": body}}),
                    };
                    let new_node = match m.as_str() {
                        "llm" => FlowNode::LlmCondition {
                            prompt: p.clone(),
                            fetch: f.clone(),
                            true_branch: Box::new(Some(action)),
                            false_branch: fb.clone(),
                        },
                        "keyword" => FlowNode::KeywordCondition {
                            keyword: kw.clone(),
                            true_branch: Box::new(Some(action)),
                            false_branch: fb.clone(),
                        },
                        _ => action,
                    };
                    ou.emit(Some(new_node));
                }
            })
        };

        // send_email: body change
        let on_email_body_change = {
            let ou = on_update.clone();
            let p = prompt.clone();
            let f = fetch.clone();
            let fb = false_branch.clone();
            let m = mode.to_string();
            let kw = keyword.clone();
            let to = cur_tc_email_to.clone();
            let subj = cur_tc_email_subject.clone();
            Callback::from(move |e: InputEvent| {
                if let Some(input) = e.target_dyn_into::<web_sys::HtmlTextAreaElement>() {
                    let action = FlowNode::Action {
                        action_type: "tool_call".into(),
                        config: serde_json::json!({"tool": "send_email", "params": {"to": to, "subject": subj, "body": input.value()}}),
                    };
                    let new_node = match m.as_str() {
                        "llm" => FlowNode::LlmCondition {
                            prompt: p.clone(),
                            fetch: f.clone(),
                            true_branch: Box::new(Some(action)),
                            false_branch: fb.clone(),
                        },
                        "keyword" => FlowNode::KeywordCondition {
                            keyword: kw.clone(),
                            true_branch: Box::new(Some(action)),
                            false_branch: fb.clone(),
                        },
                        _ => action,
                    };
                    ou.emit(Some(new_node));
                }
            })
        };

        // respond_to_email: reply text change
        let on_reply_text_change = {
            let ou = on_update.clone();
            let p = prompt.clone();
            let f = fetch.clone();
            let fb = false_branch.clone();
            let m = mode.to_string();
            let kw = keyword.clone();
            Callback::from(move |e: InputEvent| {
                if let Some(input) = e.target_dyn_into::<web_sys::HtmlTextAreaElement>() {
                    let action = FlowNode::Action {
                        action_type: "tool_call".into(),
                        config: serde_json::json!({"tool": "respond_to_email", "params": {"response_text": input.value()}}),
                    };
                    let new_node = match m.as_str() {
                        "llm" => FlowNode::LlmCondition {
                            prompt: p.clone(),
                            fetch: f.clone(),
                            true_branch: Box::new(Some(action)),
                            false_branch: fb.clone(),
                        },
                        "keyword" => FlowNode::KeywordCondition {
                            keyword: kw.clone(),
                            true_branch: Box::new(Some(action)),
                            false_branch: fb.clone(),
                        },
                        _ => action,
                    };
                    ou.emit(Some(new_node));
                }
            })
        };

        // control_tesla: command change
        let on_tesla_cmd_change = {
            let ou = on_update.clone();
            let p = prompt.clone();
            let f = fetch.clone();
            let fb = false_branch.clone();
            let m = mode.to_string();
            let kw = keyword.clone();
            Callback::from(move |e: Event| {
                if let Some(sel) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                    let action = FlowNode::Action {
                        action_type: "tool_call".into(),
                        config: serde_json::json!({"tool": "control_tesla", "params": {"command": sel.value()}}),
                    };
                    let new_node = match m.as_str() {
                        "llm" => FlowNode::LlmCondition {
                            prompt: p.clone(),
                            fetch: f.clone(),
                            true_branch: Box::new(Some(action)),
                            false_branch: fb.clone(),
                        },
                        "keyword" => FlowNode::KeywordCondition {
                            keyword: kw.clone(),
                            true_branch: Box::new(Some(action)),
                            false_branch: fb.clone(),
                        },
                        _ => action,
                    };
                    ou.emit(Some(new_node));
                }
            })
        };

        html! {
            <>
                <div class="rb-toggle-group">
                    <button
                        class={classes!("rb-toggle-btn", (cur_action_type == "notify").then(|| "active"))}
                        onclick={on_mode_notify}
                    >{"Notify me"}</button>
                    <button
                        class={classes!("rb-toggle-btn", (cur_action_type == "tool_call").then(|| "active"))}
                        onclick={on_mode_tool}
                    >{"Run tool"}</button>
                </div>

                if cur_action_type == "notify" {
                    <div class="rb-radio-group" style="margin-bottom: 0.5rem;">
                        <label class="rb-radio-label">
                            <input
                                type="radio"
                                name={radio_name.clone()}
                                checked={cur_method == "sms"}
                                onchange={on_method_sms}
                            />
                            {"SMS"}
                        </label>
                        <label class="rb-radio-label">
                            <input
                                type="radio"
                                name={radio_name}
                                checked={cur_method == "call"}
                                onchange={on_method_call}
                            />
                            {"Call"}
                        </label>
                    </div>
                    <div class="rb-field">
                        <div class="rb-field-label">{"Message (optional)"}</div>
                        <textarea
                            class="rb-textarea"
                            placeholder={if mode == "llm" {
                                "Leave empty - AI will generate the message"
                            } else {
                                "Fixed message text, or leave empty for default"
                            }}
                            value={cur_notify_msg.clone()}
                            oninput={on_msg_change}
                        ></textarea>
                        if mode == "llm" {
                            <div class="rb-field-hint">
                                {"AI's evaluation result will be used as the notification text"}
                            </div>
                        }
                    </div>
                }

                if cur_action_type == "tool_call" {
                    <div class="rb-field">
                        <div class="rb-field-label">{"Tool"}</div>
                        <select
                            class="rb-select"
                            onchange={on_tool_change}
                        >
                            <optgroup label="Built-in">
                                <option value="send_chat_message" selected={cur_tool == "send_chat_message"}>{"Send chat message"}</option>
                                <option value="send_email" selected={cur_tool == "send_email"}>{"Send email"}</option>
                                <option value="respond_to_email" selected={cur_tool == "respond_to_email"}>{"Reply to email"}</option>
                                <option value="control_tesla" selected={cur_tool == "control_tesla"}>{"Tesla command"}</option>
                                <option value="create_event" selected={cur_tool == "create_event"}>{"Create event"}</option>
                                <option value="update_event" selected={cur_tool == "update_event"}>{"Update event"}</option>
                            </optgroup>
                        </select>
                    </div>

                    // Show LLM-fillable params when in AI mode
                    if mode == "llm" {
                        {render_llm_params_hint(&cur_tool)}
                    }

                    // Per-tool parameter fields
                    if cur_tool == "send_chat_message" {
                        <div class="rb-field">
                            <div class="rb-field-label">{"Platform"}</div>
                            <select
                                class="rb-select"
                                onchange={on_chat_platform_change}
                            >
                                <option value="whatsapp" selected={cur_tc_platform == "whatsapp"}>{"WhatsApp"}</option>
                                <option value="telegram" selected={cur_tc_platform == "telegram"}>{"Telegram"}</option>
                                <option value="signal" selected={cur_tc_platform == "signal"}>{"Signal"}</option>
                            </select>
                        </div>
                        <div class="rb-field">
                            <div class="rb-field-label">{"Chat"}</div>
                            <input
                                class="rb-input"
                                type="text"
                                placeholder="Chat name..."
                                value={cur_tc_chat_name.clone()}
                                oninput={on_chat_name_change}
                            />
                        </div>
                        <div class="rb-field">
                            <div class="rb-field-label">{"Message"}</div>
                            <textarea
                                class="rb-textarea"
                                placeholder="Message to send..."
                                value={cur_tc_message.clone()}
                                oninput={on_chat_msg_change}
                            ></textarea>
                        </div>
                    }

                    if cur_tool == "send_email" {
                        <div class="rb-field">
                            <div class="rb-field-label">{"To"}</div>
                            <input
                                class="rb-input"
                                type="text"
                                placeholder="recipient@example.com"
                                value={cur_tc_email_to.clone()}
                                oninput={on_email_to_change}
                            />
                        </div>
                        <div class="rb-field">
                            <div class="rb-field-label">{"Subject"}</div>
                            <input
                                class="rb-input"
                                type="text"
                                placeholder="Email subject..."
                                value={cur_tc_email_subject.clone()}
                                oninput={on_email_subj_change}
                            />
                        </div>
                        <div class="rb-field">
                            <div class="rb-field-label">{"Body"}</div>
                            <textarea
                                class="rb-textarea"
                                placeholder="Email body..."
                                value={cur_tc_email_body.clone()}
                                oninput={on_email_body_change}
                            ></textarea>
                        </div>
                    }

                    if cur_tool == "respond_to_email" {
                        <div class="rb-field-hint" style="margin-bottom: 0.5rem;">
                            {"Replies to the email that triggered this rule. Use with an Event trigger filtered to emails."}
                        </div>
                        <div class="rb-field">
                            <div class="rb-field-label">{"Response"}</div>
                            <textarea
                                class="rb-textarea"
                                placeholder="Reply text, or leave empty for AI-generated response..."
                                value={cur_tc_reply_text.clone()}
                                oninput={on_reply_text_change}
                            ></textarea>
                        </div>
                    }

                    if cur_tool == "control_tesla" {
                        <div class="rb-field">
                            <div class="rb-field-label">{"Command"}</div>
                            <select
                                class="rb-select"
                                onchange={on_tesla_cmd_change}
                            >
                                {for [
                                    ("lock", "Lock"),
                                    ("unlock", "Unlock"),
                                    ("climate_on", "Climate on"),
                                    ("climate_off", "Climate off"),
                                    ("defrost", "Defrost"),
                                    ("remote_start", "Remote start"),
                                    ("charge_status", "Charge status"),
                                    ("precondition_battery", "Precondition battery"),
                                ].iter().map(|(val, label)| {
                                    html! { <option value={*val} selected={cur_tc_tesla_cmd == *val}>{label}</option> }
                                })}
                            </select>
                        </div>
                    }

                    if cur_tool == "create_event" {
                        <div class="rb-field-hint" style="margin-bottom: 0.5rem;">
                            {"Creates one tracked obligation with a real due time and reminder time. Use for a concrete commitment, not a whole situation."}
                        </div>
                    }

                    if cur_tool == "update_event" {
                        <div class="rb-field-hint" style="margin-bottom: 0.5rem;">
                            {"Updates a tracked obligation by appending new context and adjusting status, reminder time, or due time."}
                        </div>
                    }
                }
            </>
        }
    };

    // --- ELSE section ---
    let can_add_else = depth < 3 && mode != "always";
    let on_add_else = {
        let ou = on_update.clone();
        let p = prompt.clone();
        let f = fetch.clone();
        let tb = true_branch.clone();
        let m = mode.to_string();
        let kw = keyword.clone();
        Callback::from(move |_: MouseEvent| {
            let new_else = FlowNode::LlmCondition {
                prompt: String::new(),
                fetch: vec![],
                true_branch: Box::new(Some(FlowNode::Action {
                    action_type: "notify".to_string(),
                    config: serde_json::json!({"method":"sms"}),
                })),
                false_branch: Box::new(None),
            };
            let new_node = match m.as_str() {
                "llm" => FlowNode::LlmCondition {
                    prompt: p.clone(),
                    fetch: f.clone(),
                    true_branch: tb.clone(),
                    false_branch: Box::new(Some(new_else)),
                },
                "keyword" => FlowNode::KeywordCondition {
                    keyword: kw.clone(),
                    true_branch: tb.clone(),
                    false_branch: Box::new(Some(new_else)),
                },
                _ => return,
            };
            ou.emit(Some(new_node));
        })
    };

    let on_else_update = {
        let ou = on_update.clone();
        let p = prompt.clone();
        let f = fetch.clone();
        let tb = true_branch.clone();
        let m = mode.to_string();
        let kw = keyword.clone();
        Callback::from(move |new_fb: Option<FlowNode>| {
            let new_node = match m.as_str() {
                "llm" => FlowNode::LlmCondition {
                    prompt: p.clone(),
                    fetch: f.clone(),
                    true_branch: tb.clone(),
                    false_branch: Box::new(new_fb),
                },
                "keyword" => FlowNode::KeywordCondition {
                    keyword: kw.clone(),
                    true_branch: tb.clone(),
                    false_branch: Box::new(new_fb),
                },
                _ => return,
            };
            ou.emit(Some(new_node));
        })
    };

    html! {
        <>
            // Remove button row
            <div style="display: flex; justify-content: flex-end; margin-bottom: 0.25rem;">
                <button class="rb-remove-btn" onclick={on_remove} title="Remove condition">
                    <i class="fa-solid fa-xmark"></i>{" remove"}
                </button>
            </div>

            // IF card using render_card
            {render_card(
                Card::If,
                "IF",
                &if_summary,
                *expanded_card == Some(Card::If),
                toggle_card(Card::If),
                Some(if_content),
            )}

            if mode != "always" {
                <div class="rb-connector">
                    <div class="rb-connector-line"></div>
                    <span>{"v"}</span>
                </div>
            }

            // THEN card using render_card
            {render_card(
                Card::Then,
                "THEN",
                &then_summary,
                *expanded_card == Some(Card::Then),
                toggle_card(Card::Then),
                Some(then_content),
            )}

            // ELSE section (only for condition modes)
            if mode != "always" {
                <div class="rb-else-divider" style="margin-top: 0.5rem;">
                    <span>{"OTHERWISE"}</span>
                    <div class="rb-else-divider-line"></div>
                </div>
                if false_branch.as_ref().is_some() {
                    <div class="rb-else-content">
                        <NestedConditionEditor
                            node={false_branch.as_ref().as_ref().unwrap().clone()}
                            on_update={on_else_update}
                            depth={depth + 1}
                            available_sources={props.available_sources.clone()}
                            when_mode={props.when_mode.clone()}
                        />
                    </div>
                } else if can_add_else {
                    <div style="display: flex; gap: 0.5rem; align-items: center;">
                        <span class="rb-do-nothing">{"Skip - no action"}</span>
                        <button class="rb-add-condition-btn" onclick={on_add_else}>
                            {"+ Add a check"}
                        </button>
                    </div>
                } else {
                    <span class="rb-do-nothing">{"Skip - no action (max depth reached)"}</span>
                }
            }
        </>
    }
}

fn build_rule_summary(
    when_summary: &str,
    logic_mode: &LogicMode,
    selected_template: &PromptTemplate,
    logic_prompt: &str,
    condition_input: &str,
    keyword_input: &str,
    action_mode: &ActionMode,
    notify_method: &NotifyMethod,
    tool_name: &str,
    tc_platform: &str,
    tc_chat_name: &str,
    tc_tesla_cmd: &str,
    else_flow: &Option<FlowNode>,
) -> Html {
    // WHEN part
    let when_part = when_summary.to_string();

    // IF part
    let if_part: Option<String> = match logic_mode {
        LogicMode::Always => None,
        LogicMode::Keyword => {
            if keyword_input.is_empty() {
                Some("checks for a keyword".to_string())
            } else {
                Some(format!("checks for '{}'", keyword_input))
            }
        }
        LogicMode::Llm => match selected_template {
            PromptTemplate::Summarize => Some("AI summarizes your updates".to_string()),
            PromptTemplate::FilterImportant => Some("AI checks if it's important".to_string()),
            PromptTemplate::TrackItemsUpdate => {
                Some("AI checks if it updates a tracked obligation".to_string())
            }
            PromptTemplate::TrackItemsCreate => {
                Some("AI checks if it should create a new tracked item".to_string())
            }
            PromptTemplate::CheckCondition => {
                if condition_input.is_empty() {
                    Some("AI checks a condition".to_string())
                } else {
                    Some(format!("AI checks if it {}", condition_input))
                }
            }
            PromptTemplate::Custom => {
                if logic_prompt.is_empty() {
                    Some("AI evaluates ... ".to_string())
                } else if logic_prompt.len() > 40 {
                    Some(format!("AI checks: {}...", &logic_prompt[..40]))
                } else {
                    Some(format!("AI checks: {}", logic_prompt))
                }
            }
        },
    };

    // THEN part
    let then_part = match action_mode {
        ActionMode::Notify => match notify_method {
            NotifyMethod::Sms => "texts you".to_string(),
            NotifyMethod::Call => "calls you".to_string(),
        },
        ActionMode::ToolCall => match tool_name {
            "send_chat_message" => {
                let plat = capitalize_first(tc_platform);
                if tc_chat_name.is_empty() {
                    format!("sends a {} message", plat)
                } else {
                    format!("{} messages {}", plat, tc_chat_name)
                }
            }
            "send_email" => "sends an email".to_string(),
            "respond_to_email" => "replies to the email".to_string(),
            "control_tesla" => humanize_tesla_cmd(tc_tesla_cmd).to_string(),
            "create_event" => "creates a tracked obligation".to_string(),
            "update_event" => "updates the tracked obligation".to_string(),
            t if t.starts_with("mcp:") => {
                let parts: Vec<&str> = t.splitn(3, ':').collect();
                let tool_short = parts.get(2).unwrap_or(&"tool");
                format!("runs {}", tool_short)
            }
            _ => "runs an action".to_string(),
        },
    };

    // ELSE part - recursively describe all nesting levels
    fn describe_node_summary(n: &FlowNode) -> Vec<String> {
        match n {
            FlowNode::LlmCondition {
                prompt,
                true_branch,
                false_branch,
                ..
            } => {
                let check = if prompt.is_empty() {
                    "AI evaluates".to_string()
                } else if prompt.len() > 30 {
                    format!("AI checks: {}...", &prompt[..30])
                } else {
                    format!("AI checks: {}", prompt)
                };
                let action = match true_branch.as_ref() {
                    Some(FlowNode::Action {
                        action_type,
                        config,
                    }) => describe_action_summary(action_type, config),
                    Some(nested) => {
                        let nested_parts = describe_node_summary(nested);
                        nested_parts.join(", then ")
                    }
                    None => "runs an action".to_string(),
                };
                let mut parts = vec![format!("{}, then {}", check, action)];
                if let Some(fb) = false_branch.as_ref() {
                    let fb_parts = describe_node_summary(fb);
                    for p in fb_parts {
                        parts.push(format!("otherwise, {}", p));
                    }
                }
                parts
            }
            FlowNode::KeywordCondition {
                keyword,
                true_branch,
                false_branch,
                ..
            } => {
                let action = match true_branch.as_ref() {
                    Some(FlowNode::Action {
                        action_type,
                        config,
                    }) => describe_action_summary(action_type, config),
                    Some(nested) => {
                        let nested_parts = describe_node_summary(nested);
                        nested_parts.join(", then ")
                    }
                    None => "runs an action".to_string(),
                };
                let mut parts = vec![format!("checks for '{}', then {}", keyword, action)];
                if let Some(fb) = false_branch.as_ref() {
                    let fb_parts = describe_node_summary(fb);
                    for p in fb_parts {
                        parts.push(format!("otherwise, {}", p));
                    }
                }
                parts
            }
            FlowNode::Action {
                action_type,
                config,
            } => vec![describe_action_summary(action_type, config)],
        }
    }
    fn describe_action_summary(action_type: &str, config: &serde_json::Value) -> String {
        match action_type {
            "notify" => match config.get("method").and_then(|v| v.as_str()) {
                Some("call") => "calls you".to_string(),
                _ => "texts you".to_string(),
            },
            "tool_call" => match config.get("tool").and_then(|v| v.as_str()) {
                Some("create_event") => "creates an event".to_string(),
                Some("update_event") => "updates the event".to_string(),
                Some("send_email") => "sends an email".to_string(),
                Some("send_chat_message") => "sends a message".to_string(),
                Some("control_tesla") => {
                    let cmd = config
                        .get("params")
                        .and_then(|p| p.get("command"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("command");
                    humanize_tesla_cmd(cmd).to_string()
                }
                _ => "runs a tool".to_string(),
            },
            _ => "runs an action".to_string(),
        }
    }

    let else_lines: Vec<String> = else_flow
        .as_ref()
        .map(|node| describe_node_summary(node))
        .unwrap_or_default();

    html! {
        <div class="rb-rule-summary">
            {capitalize_first(&when_part)}
            if let Some(ref check) = if_part {
                {format!(", {}", check)}
            }
            {format!(", then {}.", then_part)}
            {for else_lines.iter().map(|line| html! {
                <>
                    <br/>
                    {format!("{}.", capitalize_first(line))}
                </>
            })}
        </div>
    }
}

fn render_card(
    _card: Card,
    label: &str,
    summary: &str,
    is_expanded: bool,
    on_toggle: Callback<MouseEvent>,
    content: Option<Html>,
) -> Html {
    let class = if is_expanded {
        "rb-card expanded"
    } else {
        "rb-card collapsed"
    };

    html! {
        <div class={class}>
            <div class="rb-card-header" onclick={on_toggle}>
                <span class="rb-card-label">{label}</span>
                <span class="rb-card-summary">{summary}</span>
                <i class="rb-card-chevron fa-solid fa-chevron-down"></i>
            </div>
            if is_expanded {
                if let Some(inner) = content {
                    <div class="rb-card-content">
                        {inner}
                    </div>
                }
            }
        </div>
    }
}

fn parse_pattern_into(
    pattern: &str,
    freq: &UseStateHandle<RecurringFreq>,
    time: &UseStateHandle<String>,
    day: &UseStateHandle<String>,
) {
    let parts: Vec<&str> = pattern.splitn(2, ' ').collect();
    if parts.is_empty() {
        return;
    }
    match parts[0] {
        "hourly" => freq.set(RecurringFreq::Hourly),
        "daily" => {
            freq.set(RecurringFreq::Daily);
            if let Some(t) = parts.get(1) {
                time.set(t.to_string());
            }
        }
        "weekdays" => {
            freq.set(RecurringFreq::Weekdays);
            if let Some(t) = parts.get(1) {
                time.set(t.to_string());
            }
        }
        "weekly" => {
            freq.set(RecurringFreq::Weekly);
            if let Some(rest) = parts.get(1) {
                let sub: Vec<&str> = rest.splitn(2, ' ').collect();
                if sub.len() >= 2 {
                    day.set(sub[0].to_lowercase());
                    time.set(sub[1].to_string());
                }
            }
        }
        _ => {}
    }
}

fn format_time_display(time: &str) -> String {
    let parts: Vec<&str> = time.split(':').collect();
    if parts.len() >= 2 {
        let hour: u32 = parts[0].parse().unwrap_or(0);
        let minute: u32 = parts[1].parse().unwrap_or(0);
        let (h12, ampm) = if hour == 0 {
            (12, "am")
        } else if hour < 12 {
            (hour, "am")
        } else if hour == 12 {
            (12, "pm")
        } else {
            (hour - 12, "pm")
        };
        if minute == 0 {
            format!("{}{}", h12, ampm)
        } else {
            format!("{}:{:02}{}", h12, minute, ampm)
        }
    } else {
        time.to_string()
    }
}

fn auto_generate_name(
    trigger_type: &str,
    trigger_config: &str,
    action_type: &str,
    action_config: &str,
) -> String {
    let tc: serde_json::Value = serde_json::from_str(trigger_config).unwrap_or_default();
    let ac: serde_json::Value = serde_json::from_str(action_config).unwrap_or_default();

    let trigger_part = if trigger_type == "schedule" {
        match tc.get("schedule").and_then(|v| v.as_str()) {
            Some("recurring") => {
                let pattern = tc
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .unwrap_or("daily");
                let freq = pattern.split_whitespace().next().unwrap_or("daily");
                capitalize_first(freq)
            }
            Some("once") => "One-time".to_string(),
            _ => "Scheduled".to_string(),
        }
    } else {
        // ontology_change
        let filters = tc.get("filters").and_then(|v| v.as_object());
        if let Some(f) = filters {
            if let Some((key, val)) = f.iter().next() {
                let val_str = val.as_str().unwrap_or("");
                let group_mode = tc.get("group_mode").and_then(|v| v.as_str());
                if key == "sender" && !val_str.is_empty() {
                    match group_mode {
                        Some("mention_only") => format!("Group {} (mentions)", val_str),
                        Some("all") => format!("Group {} (all)", val_str),
                        _ => format!("From {}", val_str),
                    }
                } else if !val_str.is_empty() {
                    format!("{} {}", capitalize_first(key), val_str)
                } else {
                    "Message monitor".to_string()
                }
            } else {
                "Message monitor".to_string()
            }
        } else {
            "Message monitor".to_string()
        }
    };

    let action_part = match action_type {
        "notify" => match ac.get("method").and_then(|v| v.as_str()) {
            Some("call") => "call".to_string(),
            _ => "SMS".to_string(),
        },
        "tool_call" => {
            let tool = ac.get("tool").and_then(|v| v.as_str()).unwrap_or("action");
            match tool {
                "send_chat_message" => {
                    let chat = ac
                        .get("params")
                        .and_then(|p| p.get("chat_name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if chat.is_empty() {
                        "message".to_string()
                    } else {
                        format!("msg {}", chat)
                    }
                }
                "send_email" => "email".to_string(),
                "respond_to_email" => "reply email".to_string(),
                "control_tesla" => {
                    let cmd = ac
                        .get("params")
                        .and_then(|p| p.get("command"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("cmd");
                    format!("Tesla {}", cmd)
                }
                "create_event" => "create event".to_string(),
                "update_event" => "update event".to_string(),
                _ if tool.starts_with("mcp:") => {
                    let parts: Vec<&str> = tool.splitn(3, ':').collect();
                    let t = parts.get(2).unwrap_or(&"tool");
                    format!("MCP: {}", t)
                }
                _ => tool.to_string(),
            }
        }
        _ => "notify".to_string(),
    };

    format!("{} - {}", trigger_part, action_part)
}

fn capitalize_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

/// Render a human-readable hint about what extra info the AI will figure out for a tool.
fn render_llm_params_hint(tool: &str) -> Html {
    let hint = match tool {
        "update_event" => {
            Some("AI will pick which tracked obligation to update and append the new concrete change")
        }
        _ => None,
    };
    match hint {
        Some(text) => html! {
            <div class="rb-field-hint">{text}</div>
        },
        None => html! {},
    }
}

fn get_track_update_prompt() -> String {
    "Does this message update an already-tracked obligation with a concrete next step or deadline? Compare it against existing tracked obligations and their linked message context. Create or update events for specific commitments like paying, booking, confirming, or sending something. Do not use one umbrella event for an entire trip or situation. Routine updates should be tracked silently in the background. If this message changes a tracked obligation's status, due date, or best reminder time, act on it. Otherwise skip.".to_string()
}

fn get_template_prompt(template: &PromptTemplate, when_mode: &WhenMode, condition: &str) -> String {
    match template {
        PromptTemplate::Summarize => match when_mode {
            WhenMode::Schedule => "Summarize recent messages and emails into a brief digest. Focus on key points, action items, and anything that needs attention. Also mention any tracked obligations with approaching due times. Format as a numbered list, one item per line.".to_string(),
            WhenMode::Event => "Summarize this message along with recent conversation context. Highlight key points and any action needed.".to_string(),
        },
        PromptTemplate::FilterImportant => match when_mode {
            WhenMode::Schedule => "Review recent messages and emails. Only notify if delaying over 2 hours could cause harm, financial loss, or miss a time-sensitive opportunity. Examples: emergencies, someone asking to meet now, immediate decisions needed. Routine updates and vague requests are NOT critical. If nothing critical, respond with just 'skip'.".to_string(),
            WhenMode::Event => "Only notify if delaying this message over 2 hours could cause harm, financial loss, or miss a time-sensitive opportunity. Examples: emergencies, someone asking to meet now, immediate decisions needed. Routine updates, casual messages, and vague requests are NOT critical. If not critical, respond with just 'skip'.".to_string(),
        },
        PromptTemplate::CheckCondition => match when_mode {
            WhenMode::Schedule => format!("Check if the following condition is met based on recent messages: {}. If the condition is not met, respond with just 'skip'.", condition),
            WhenMode::Event => format!("Check if this message matches the following condition: {}. If it doesn't match, respond with just 'skip'.", condition),
        },
        PromptTemplate::TrackItemsUpdate => get_track_update_prompt(),
        PromptTemplate::TrackItemsCreate => "Should this message create a new tracked obligation? Only create one for a concrete commitment the user could forget and would benefit from being reminded about at the right time, such as paying, booking, confirming, sending, or following up by a certain date. Do not create umbrella events for whole situations like trip planning when the message is really about a smaller obligation inside it. If nothing specific should be tracked, respond with just 'skip'.".to_string(),
        PromptTemplate::Custom => String::new(),
    }
}

fn get_template_description(template: &PromptTemplate, when_mode: &WhenMode) -> &'static str {
    match template {
        PromptTemplate::Summarize => match when_mode {
            WhenMode::Schedule => "AI will review your recent messages and emails, then send you a concise summary with key points and action items.",
            WhenMode::Event => "AI will summarize this message with recent conversation context, highlighting what's important.",
        },
        PromptTemplate::FilterImportant => match when_mode {
            WhenMode::Schedule => "AI will review your recent messages and only notify you if something urgent or important needs your attention.",
            WhenMode::Event => "AI will evaluate this message and only notify you if it seems important or urgent.",
        },
        PromptTemplate::TrackItemsUpdate => "AI will check if this message updates an item you're already tracking (delivery status, payment, deadline) and update it automatically.",
        PromptTemplate::TrackItemsCreate => "AI will check if this message contains a concrete commitment worth tracking (payment, booking, follow-up by a date) and create a new tracked item for it.",
        _ => "",
    }
}

/// Extract required field names from a JSON Schema input_schema.
/// Returns all property names (required first, then optional).
fn extract_schema_fields(schema: Option<&serde_json::Value>) -> Vec<String> {
    let schema = match schema {
        Some(s) => s,
        None => return vec![],
    };
    let props = match schema.get("properties").and_then(|p| p.as_object()) {
        Some(p) => p,
        None => return vec![],
    };
    let required: Vec<String> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    // Required fields first, then remaining properties
    let mut fields: Vec<String> = required.clone();
    for key in props.keys() {
        if !fields.contains(key) {
            fields.push(key.clone());
        }
    }
    fields
}

fn is_rule_complete(
    when_mode: &WhenMode,
    schedule_mode: &ScheduleMode,
    once_date: &str,
    once_time: &str,
    logic_mode: &LogicMode,
    selected_template: &PromptTemplate,
    logic_prompt: &str,
    condition_input: &str,
    keyword_input: &str,
) -> (bool, Vec<&'static str>) {
    let mut missing = Vec::new();

    // Check WHEN
    if *when_mode == WhenMode::Schedule && *schedule_mode == ScheduleMode::Once {
        if once_date.is_empty() || once_time.is_empty() {
            missing.push("Set a date and time");
        }
    }

    // Check IF
    match logic_mode {
        LogicMode::Keyword => {
            if keyword_input.is_empty() {
                missing.push("Enter a keyword to match");
            }
        }
        LogicMode::Llm => match selected_template {
            PromptTemplate::Custom => {
                if logic_prompt.is_empty() {
                    missing.push("Describe what AI should check");
                }
            }
            PromptTemplate::CheckCondition => {
                if condition_input.is_empty() {
                    missing.push("Enter the condition to check");
                }
            }
            _ => {} // Built-in templates are always complete
        },
        LogicMode::Always => {}
    }

    let complete = missing.is_empty();
    (complete, missing)
}

fn render_review(
    when_summary: &str,
    logic_mode: &LogicMode,
    selected_template: &PromptTemplate,
    logic_prompt: &str,
    condition_input: &str,
    keyword_input: &str,
    action_mode: &ActionMode,
    notify_method: &NotifyMethod,
    tool_name: &str,
    tc_platform: &str,
    tc_chat_name: &str,
    tc_tesla_cmd: &str,
    else_flow: &Option<FlowNode>,
    missing: &[&str],
) -> Html {
    // Build summary parts
    let when_text = capitalize_first(when_summary);

    let if_text: Option<String> = match logic_mode {
        LogicMode::Always => None,
        LogicMode::Keyword => {
            if keyword_input.is_empty() {
                None
            } else {
                Some(format!("AI checks for '{}'", keyword_input))
            }
        }
        LogicMode::Llm => match selected_template {
            PromptTemplate::Summarize => Some("AI summarizes your updates".to_string()),
            PromptTemplate::FilterImportant => Some("AI checks if it's important".to_string()),
            PromptTemplate::TrackItemsUpdate => {
                Some("AI checks if it updates a tracked obligation".to_string())
            }
            PromptTemplate::TrackItemsCreate => {
                Some("AI checks if it should create a new tracked item".to_string())
            }
            PromptTemplate::CheckCondition => {
                if condition_input.is_empty() {
                    None
                } else {
                    Some(format!("AI checks if it {}", condition_input))
                }
            }
            PromptTemplate::Custom => {
                if logic_prompt.is_empty() {
                    None
                } else if logic_prompt.len() > 50 {
                    Some(format!("AI checks: {}...", &logic_prompt[..50]))
                } else {
                    Some(format!("AI checks: {}", logic_prompt))
                }
            }
        },
    };

    let then_text = match action_mode {
        ActionMode::Notify => match notify_method {
            NotifyMethod::Sms => "Texts you".to_string(),
            NotifyMethod::Call => "Calls you".to_string(),
        },
        ActionMode::ToolCall => match tool_name {
            "send_chat_message" => {
                let plat = capitalize_first(tc_platform);
                if tc_chat_name.is_empty() {
                    format!("Sends a {} message", plat)
                } else {
                    format!("{} messages {}", plat, tc_chat_name)
                }
            }
            "send_email" => "Sends an email".to_string(),
            "respond_to_email" => "Replies to the email".to_string(),
            "control_tesla" => capitalize_first(humanize_tesla_cmd(tc_tesla_cmd)),
            "create_event" => "Creates a tracked obligation".to_string(),
            "update_event" => "Updates the tracked obligation".to_string(),
            t if t.starts_with("mcp:") => {
                let parts: Vec<&str> = t.splitn(3, ':').collect();
                format!("Runs {}", parts.get(2).unwrap_or(&"tool"))
            }
            _ => "Runs an action".to_string(),
        },
    };

    html! {
        <div class="rb-review-card">
            <div class="rb-review-title">{"Review"}</div>
            <div class="rb-review-flow">
                {&when_text}
                if let Some(ref check) = if_text {
                    <div class="rb-review-step">{check}</div>
                }
                <div class="rb-review-step">{&then_text}</div>
                if let Some(node) = else_flow {
                    <br/>
                    {"Otherwise"}
                    {render_else_review(node)}
                }
            </div>
            if !missing.is_empty() {
                <div class="rb-review-missing">
                    {for missing.iter().map(|m| html! {
                        <div>{*m}</div>
                    })}
                </div>
            }
        </div>
    }
}

fn render_else_review(node: &FlowNode) -> Html {
    match node {
        FlowNode::LlmCondition {
            prompt,
            true_branch,
            false_branch,
            ..
        } => {
            let check = if prompt.is_empty() {
                "AI evaluates".to_string()
            } else if prompt.len() > 40 {
                format!("AI checks: {}...", &prompt[..40])
            } else {
                format!("AI checks: {}", prompt)
            };
            let action = match true_branch.as_ref() {
                Some(FlowNode::Action {
                    action_type,
                    config,
                }) => review_action_text(action_type, config),
                Some(nested) => {
                    return html! {
                        <>
                            <div class="rb-review-step">{check}</div>
                            {render_else_review(nested)}
                        </>
                    }
                }
                None => "runs an action".to_string(),
            };
            html! {
                <>
                    <div class="rb-review-step">{check}</div>
                    <div class="rb-review-step">{action}</div>
                    if let Some(fb) = false_branch.as_ref() {
                        <br/>
                        {"Otherwise"}
                        {render_else_review(fb)}
                    }
                </>
            }
        }
        FlowNode::KeywordCondition {
            keyword,
            true_branch,
            false_branch,
            ..
        } => {
            let action = match true_branch.as_ref() {
                Some(FlowNode::Action {
                    action_type,
                    config,
                }) => review_action_text(action_type, config),
                Some(nested) => {
                    return html! {
                        <>
                            <div class="rb-review-step">{format!("Checks for '{}'", keyword)}</div>
                            {render_else_review(nested)}
                        </>
                    }
                }
                None => "runs an action".to_string(),
            };
            html! {
                <>
                    <div class="rb-review-step">{format!("Checks for '{}'", keyword)}</div>
                    <div class="rb-review-step">{action}</div>
                    if let Some(fb) = false_branch.as_ref() {
                        <br/>
                        {"Otherwise"}
                        {render_else_review(fb)}
                    }
                </>
            }
        }
        FlowNode::Action {
            action_type,
            config,
        } => {
            html! { <div class="rb-review-step">{review_action_text(action_type, config)}</div> }
        }
    }
}

fn review_action_text(action_type: &str, config: &serde_json::Value) -> String {
    match action_type {
        "notify" => match config.get("method").and_then(|v| v.as_str()) {
            Some("call") => "Calls you".to_string(),
            _ => "Texts you".to_string(),
        },
        "tool_call" => match config.get("tool").and_then(|v| v.as_str()) {
            Some("create_event") => "Pins it to your dashboard".to_string(),
            Some("update_event") => "Updates the tracked obligation".to_string(),
            Some("send_email") => "Sends an email".to_string(),
            Some("send_chat_message") => {
                let plat = config
                    .get("params")
                    .and_then(|p| p.get("platform"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("chat");
                let chat = config
                    .get("params")
                    .and_then(|p| p.get("chat_name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if chat.is_empty() {
                    format!("Sends a {} message", capitalize_first(plat))
                } else {
                    format!("{} messages {}", capitalize_first(plat), chat)
                }
            }
            Some("control_tesla") => {
                let cmd = config
                    .get("params")
                    .and_then(|p| p.get("command"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("command");
                capitalize_first(humanize_tesla_cmd(cmd))
            }
            _ => "Runs a tool".to_string(),
        },
        _ => "Runs an action".to_string(),
    }
}

fn humanize_tesla_cmd(cmd: &str) -> &'static str {
    match cmd {
        "lock" => "locks Tesla doors",
        "unlock" => "unlocks Tesla doors",
        "climate_on" => "turns on Tesla climate",
        "climate_off" => "turns off Tesla climate",
        "defrost" => "defrosts Tesla",
        "remote_start" => "remote starts Tesla",
        "charge_status" => "checks Tesla charge",
        "precondition_battery" => "preconditions Tesla battery",
        _ => "runs Tesla command",
    }
}

fn humanize_tesla_cmd_short(cmd: &str) -> &'static str {
    match cmd {
        "lock" => "Tesla lock doors",
        "unlock" => "Tesla unlock doors",
        "climate_on" => "Tesla climate on",
        "climate_off" => "Tesla climate off",
        "defrost" => "Tesla defrost",
        "remote_start" => "Tesla remote start",
        "charge_status" => "Tesla charge status",
        "precondition_battery" => "Tesla precondition",
        _ => "Tesla command",
    }
}

fn get_starter_chips(when_mode: &WhenMode) -> Vec<&'static str> {
    match when_mode {
        WhenMode::Event => vec![
            "Notify me if ",
            "Only when it's about ",
            "Ignore unless ",
            "Check if this ",
        ],
        WhenMode::Schedule => vec![
            "Summarize anything about ",
            "Only tell me if ",
            "Check whether ",
        ],
    }
}

fn auto_detect_sources(prompt: &str, available: &[RuleSourceOption]) -> Vec<SourceConfig> {
    let lower = prompt.to_lowercase();
    let mut detected = Vec::new();

    let mappings: Vec<(&[&str], &str, SourceConfig)> = vec![
        (
            &["weather", "temperature", "rain", "forecast", "cold", "hot"],
            "weather",
            SourceConfig::Weather {
                location: String::new(),
            },
        ),
        (
            &["email", "inbox", "mail", "sent"],
            "email",
            SourceConfig::Email,
        ),
        (
            &[
                "chat",
                "message",
                "whatsapp",
                "telegram",
                "signal",
                "conversation",
            ],
            "chat",
            SourceConfig::Chat {
                platform: "all".to_string(),
                limit: 50,
            },
        ),
        (
            &["tesla", "car", "charge", "drive", "vehicle", "battery"],
            "tesla",
            SourceConfig::Tesla,
        ),
        (
            &["tracked", "event", "delivery", "package", "invoice"],
            "events",
            SourceConfig::Events,
        ),
        (
            &["search", "look up", "find online", "news"],
            "internet",
            SourceConfig::Internet {
                query: String::new(),
            },
        ),
    ];

    for (keywords, source_type, source_config) in mappings {
        // Check if this source is available
        let is_available = available
            .iter()
            .any(|s| s.source_type == source_type && s.available);
        if !is_available {
            continue;
        }
        for kw in keywords {
            if lower.contains(kw) {
                detected.push(source_config);
                break;
            }
        }
    }

    detected
}

fn format_datetime_short_local(at: &str) -> String {
    if at.len() >= 16 {
        let month_day = &at[5..10];
        let time = &at[11..16];
        let parts: Vec<&str> = month_day.split('-').collect();
        if parts.len() == 2 {
            let month = match parts[0] {
                "01" => "Jan",
                "02" => "Feb",
                "03" => "Mar",
                "04" => "Apr",
                "05" => "May",
                "06" => "Jun",
                "07" => "Jul",
                "08" => "Aug",
                "09" => "Sep",
                "10" => "Oct",
                "11" => "Nov",
                "12" => "Dec",
                _ => parts[0],
            };
            let day: u32 = parts[1].parse().unwrap_or(0);
            return format!("{} {}, {}", month, day, format_time_display(time));
        }
    }
    at.to_string()
}
