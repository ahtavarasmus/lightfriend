use yew::prelude::*;

const TRIAGE_STYLES: &str = r#"
.triage-indicator {
    display: flex;
    flex-direction: column;
    background: rgba(30, 144, 255, 0.08);
    border: 1px solid rgba(30, 144, 255, 0.15);
    border-radius: 12px;
    overflow: hidden;
}
.triage-indicator.all-clear {
    background: rgba(76, 175, 80, 0.08);
    border-color: rgba(76, 175, 80, 0.15);
}
.triage-indicator.has-attention {
    background: rgba(255, 193, 7, 0.08);
    border-color: rgba(255, 193, 7, 0.2);
}
.triage-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 1rem 1.25rem;
    cursor: pointer;
    user-select: none;
}
.triage-content {
    display: flex;
    align-items: center;
    gap: 0.75rem;
}
.triage-icon {
    font-size: 1.25rem;
    color: #4CAF50;
}
.triage-icon.attention {
    color: #FFC107;
}
.triage-text {
    color: #ddd;
    font-size: 0.95rem;
}
.triage-chevron {
    color: #888;
    font-size: 0.8rem;
    transition: transform 0.2s;
}
.triage-chevron.expanded {
    transform: rotate(180deg);
}
.triage-items {
    max-height: 0;
    overflow: hidden;
    transition: max-height 0.25s ease;
}
.triage-items.expanded {
    max-height: 500px;
}
.triage-item-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.6rem 1.25rem;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
}
.triage-item-badge {
    font-size: 0.75rem;
    font-weight: 600;
    padding: 0.15rem 0.5rem;
    border-radius: 4px;
    white-space: nowrap;
    flex-shrink: 0;
}
.triage-item-badge.bridge {
    background: rgba(255, 152, 0, 0.15);
    color: #ffb74d;
}
.triage-item-summary {
    flex: 1;
    color: #ccc;
    font-size: 0.9rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}
.triage-dismiss-btn {
    background: none;
    border: none;
    color: #666;
    font-size: 1rem;
    cursor: pointer;
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    line-height: 1;
    flex-shrink: 0;
    transition: color 0.15s, background 0.15s;
}
.triage-dismiss-btn:hover {
    color: #e57373;
    background: rgba(229, 115, 115, 0.1);
}
"#;

#[derive(Clone, PartialEq)]
pub struct AttentionItem {
    pub id: i32,
    pub item_type: String,
    pub summary: String,
    pub timestamp: i32,
    pub source: Option<String>,
}

#[derive(Properties, PartialEq, Clone)]
pub struct TriageIndicatorProps {
    pub attention_count: i32,
    pub attention_items: Vec<AttentionItem>,
    pub on_dismiss: Callback<AttentionItem>,
}

#[function_component(TriageIndicator)]
pub fn triage_indicator(props: &TriageIndicatorProps) -> Html {
    let expanded = use_state(|| false);

    if props.attention_count == 0 {
        return html! {
            <>
                <style>{TRIAGE_STYLES}</style>
                <div class="triage-indicator all-clear">
                    <div class="triage-header">
                        <div class="triage-content">
                            <span class="triage-icon">{"*"}</span>
                            <span class="triage-text">{"No urgent items"}</span>
                        </div>
                    </div>
                </div>
            </>
        };
    }

    let count = props.attention_count;
    let things_text = if count == 1 { "thing needs" } else { "things need" };

    let toggle = {
        let expanded = expanded.clone();
        Callback::from(move |_: MouseEvent| {
            expanded.set(!*expanded);
        })
    };

    let chevron_class = if *expanded {
        "triage-chevron expanded"
    } else {
        "triage-chevron"
    };

    let items_class = if *expanded {
        "triage-items expanded"
    } else {
        "triage-items"
    };

    html! {
        <>
            <style>{TRIAGE_STYLES}</style>
            <div class="triage-indicator has-attention">
                <div class="triage-header" onclick={toggle}>
                    <div class="triage-content">
                        <span class="triage-icon attention">{"!"}</span>
                        <span class="triage-text">
                            {format!("{} {} attention", count, things_text)}
                        </span>
                    </div>
                    <span class={chevron_class}>
                        <i class="fa-solid fa-chevron-down"></i>
                    </span>
                </div>
                <div class={items_class}>
                    { for props.attention_items.iter().map(|item| {
                        let (badge_class, badge_label) = match item.item_type.as_str() {
                            "bridge_disconnected" => ("triage-item-badge bridge", "Bridge"),
                            _ => ("triage-item-badge", "Other"),
                        };
                        let dismiss_item = item.clone();
                        let on_dismiss = props.on_dismiss.clone();
                        html! {
                            <div class="triage-item-row">
                                <span class={badge_class}>{badge_label}</span>
                                <span class="triage-item-summary">{&item.summary}</span>
                                <button
                                    class="triage-dismiss-btn"
                                    onclick={Callback::from(move |e: MouseEvent| {
                                        e.stop_propagation();
                                        on_dismiss.emit(dismiss_item.clone());
                                    })}
                                    title="Dismiss"
                                >
                                    {"x"}
                                </button>
                            </div>
                        }
                    })}
                </div>
            </div>
        </>
    }
}
