use crate::config::get_backend_url;
use crate::utils::seo::{use_seo, SeoMeta};
use crate::Route;
use gloo_net::http::Request;
use serde::Deserialize;
use wasm_bindgen::prelude::*;
use yew::prelude::*;
use yew_router::prelude::*;

// -- Data types matching backend --

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct TrustChainData {
    commit_sha: Option<String>,
    workflow_run_id: Option<String>,
    image_ref: Option<String>,
    eif_sha256: Option<String>,
    pcr0: Option<String>,
    pcr1: Option<String>,
    pcr2: Option<String>,
    image_id: Option<String>,
    kms_contract_address: Option<String>,
    built_at: Option<String>,
    build_metadata_url: Option<String>,
    blockchain: Option<BlockchainInfo>,
    attestation: Option<AttestationInfo>,
    history: Vec<HistoricalBuild>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct AttestationInfo {
    available: bool,
    pcr0: Option<String>,
    pcr1: Option<String>,
    pcr2: Option<String>,
    doc_byte_size: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct BlockchainInfo {
    approved: bool,
    propose_tx: Option<String>,
    propose_timestamp: Option<String>,
    activate_tx: Option<String>,
    activate_timestamp: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct HistoricalBuild {
    image_id: String,
    commit_hash: String,
    propose_tx: String,
    propose_timestamp: Option<String>,
    activate_tx: Option<String>,
    activate_timestamp: Option<String>,
    is_current: bool,
}

// -- Helpers --

fn short_hash(hash: &str, len: usize) -> String {
    let clean = hash.strip_prefix("0x").unwrap_or(hash);
    if clean.len() > len {
        format!("{}...", &clean[..len])
    } else {
        clean.to_string()
    }
}

fn relative_time(iso: &str) -> String {
    let date = js_sys::Date::new(&JsValue::from_str(iso));
    let now = js_sys::Date::new_0();
    let diff_ms = now.get_time() - date.get_time();
    if diff_ms < 0.0 {
        return "just now".to_string();
    }
    let secs = (diff_ms / 1000.0) as u64;
    let mins = secs / 60;
    let hours = mins / 60;
    let days = hours / 24;
    if secs < 60 {
        "just now".to_string()
    } else if mins < 60 {
        format!("{} min ago", mins)
    } else if hours < 24 {
        format!("{}h ago", hours)
    } else {
        format!("{}d ago", days)
    }
}

fn format_ts(iso: &str) -> String {
    let date = js_sys::Date::new(&JsValue::from_str(iso));
    format!(
        "{}-{:02}-{:02} {:02}:{:02} UTC",
        date.get_utc_full_year(),
        date.get_utc_month() + 1,
        date.get_utc_date(),
        date.get_utc_hours(),
        date.get_utc_minutes()
    )
}

// -- Main page --

#[function_component(TrustChainPage)]
pub fn trust_chain_page() -> Html {
    use_seo(SeoMeta {
        title: "Trust Chain - Lightfriend",
        description: "Visual map of how Lightfriend keeps your data private. Follow the chain from source code to running enclave.",
        canonical: "https://lightfriend.ai/trust-chain",
        og_type: "website",
    });

    let data = use_state(|| None::<TrustChainData>);
    let loading = use_state(|| true);

    {
        use_effect_with_deps(
            move |_| {
                if let Some(window) = web_sys::window() {
                    window.scroll_to_with_x_and_y(0.0, 0.0);
                }
                || ()
            },
            (),
        );
    }

    {
        let data = data.clone();
        let loading = loading.clone();
        use_effect_with_deps(
            move |_| {
                wasm_bindgen_futures::spawn_local(async move {
                    let url = format!("{}/api/trust-chain", get_backend_url());
                    if let Ok(resp) = Request::get(&url).send().await {
                        if let Ok(d) = resp.json::<TrustChainData>().await {
                            data.set(Some(d));
                        }
                    }
                    loading.set(false);
                });
                || ()
            },
            (),
        );
    }

    let d = (*data).clone();

    html! {
        <>
        <style>{STYLES}</style>
        <div class="tc-page">
            <div class="tc-header">
                <h1>{"Trust Chain"}</h1>
                <p class="tc-subtitle">{"Visual map of the running system. Every link points to something you can verify yourself."}</p>
            </div>

            if *loading {
                <div class="tc-loading">
                    <i class="fa-solid fa-spinner fa-spin"></i>{" Loading..."}
                </div>
            } else if let Some(ref d) = d {
                {render_diagram(d)}

                {render_verify_section(d)}

                if !d.history.is_empty() {
                    {render_history(&d.history)}
                }
            } else {
                <div class="tc-loading">
                    {"Could not load trust chain data. This page works best when connected to a live Lightfriend instance."}
                </div>
            }

            <div class="tc-footer-links">
                <Link<Route> to={Route::Trustless}>
                    {"How does this all work? Read the full explanation"}
                    {" "}<i class="fa-solid fa-arrow-right"></i>
                </Link<Route>>
            </div>
            <div class="legal-links">
                <Link<Route> to={Route::Home}>{"Home"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Terms}>{"Terms"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Privacy}>{"Privacy"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Trustless}>{"Verifiably Private"}</Link<Route>>
            </div>
        </div>
        </>
    }
}

// =============================================
// DIAGRAM
// =============================================

fn render_diagram(d: &TrustChainData) -> Html {
    let past_builds: Vec<&HistoricalBuild> = d.history.iter()
        .filter(|b| !b.is_current)
        .take(1)
        .collect();

    html! {
        <div class="tc-diagram">
            // Left column: Sealed computers
            <div class="tc-col-enclaves">
                <div class="col-header">
                    <i class="fa-solid fa-cloud"></i>
                    {" Sealed Computers"}
                </div>
                {render_live_enclave(d)}
                {render_data_flow_between()}
                if let Some(build) = past_builds.first() {
                    {render_past_enclave(build)}
                } else {
                    {render_placeholder_enclave()}
                }
            </div>

            // Right column: Verification + key management
            <div class="tc-col-governance">
                <div class="col-header">
                    <i class="fa-solid fa-diagram-project"></i>
                    {" Verification & Key Management"}
                </div>

                {render_dev_node()}
                {render_gov_arrow("commits code")}
                {render_github_node(d)}
                {render_gov_arrow("image ID")}
                {render_arbitrum_node(d)}
                {render_gov_arrow_up("Marlin reads contract")}
                {render_marlin_sealed(d)}
            </div>
        </div>
    }
}

// ---- Live enclave ----

fn render_live_enclave(d: &TrustChainData) -> Html {
    let commit = d.commit_sha.as_deref().unwrap_or("unknown");
    let commit_short = short_hash(commit, 8);
    let pcr0 = d.pcr0.as_deref().unwrap_or("unavailable");
    let pcr1 = d.pcr1.as_deref().unwrap_or("unavailable");
    let pcr2 = d.pcr2.as_deref().unwrap_or("unavailable");

    html! {
        <div class="enclave-wrapper">
            <span class="cloud-tag-outer live-tag">{"LIVE"}</span>
            <div class="enclave-cloud live">
                // Encrypted data bar - very first thing, hugs top wall
                <div class="edge-bar edge-top-bar">
                    <i class="fa-solid fa-lock ld-icon"></i>
                    <span>{"Encrypted data"}</span>
                </div>

                <div class="cloud-inner">
                    // Export: encrypted data(top) -> ↑ -> key from Marlin -> + -> unlocked data
                    <div class="vert-flow">
                        <div class="stack-arrow"><i class="fa-solid fa-arrow-up"></i></div>
                        <div class="eq-item key-source-inline">
                            <i class="fa-solid fa-key ks-icon"></i>
                            <span>{"from Marlin"}</span>
                        </div>
                        <span class="eq-op">{"+"}</span>
                        <div class="eq-item">
                            <i class="fa-solid fa-lock-open eq-icon eq-green"></i>
                            <span class="eq-text">{"data"}</span>
                        </div>
                    </div>
                    <div class="pathway-label">{"export"}</div>

                <div class="cloud-code-label">
                    <i class="fa-solid fa-code"></i>
                    <span>{"lightfriend's code"}</span>
                </div>

                <div class="cloud-attestation">
                    <div class="att-row">
                        <span class="att-label">{"PCR0"}</span>
                        <code class="att-val">{short_hash(pcr0, 16)}</code>
                    </div>
                    <div class="att-row">
                        <span class="att-label">{"PCR1"}</span>
                        <code class="att-val">{short_hash(pcr1, 16)}</code>
                    </div>
                    <div class="att-row">
                        <span class="att-label">{"PCR2"}</span>
                        <code class="att-val">{short_hash(pcr2, 16)}</code>
                    </div>
                    <div class="att-row">
                        <span class="att-label">{"Commit"}</span>
                        <code class="att-val">{&commit_short}</code>
                    </div>
                </div>

                if let Some(ref ts) = d.built_at {
                    <div class="cloud-time">{"Built "}{relative_time(ts)}</div>
                }

                <div class="cloud-links">
                    <a href="/.well-known/lightfriend/attestation" target="_blank" rel="noopener noreferrer">
                        <i class="fa-solid fa-certificate"></i>{" Live attestation"}
                    </a>
                    <a href={format!("https://github.com/ahtavarasmus/lightfriend/tree/{}/tools/attestation-verifier", commit)} target="_blank" rel="noopener noreferrer">
                        <i class="fa-solid fa-magnifying-glass"></i>{" Verification tool"}
                    </a>
                </div>

                // Import: unlocked data -> ↑ -> key from Marlin -> + -> encrypted data(bottom)
                <div class="pathway-label">{"import"}</div>
                <div class="vert-flow">
                    <div class="eq-item">
                        <i class="fa-solid fa-lock-open eq-icon eq-green"></i>
                        <span class="eq-text">{"data"}</span>
                    </div>
                    <div class="stack-arrow"><i class="fa-solid fa-arrow-up"></i></div>
                    <div class="eq-item key-source-inline">
                        <i class="fa-solid fa-key ks-icon"></i>
                        <span>{"from Marlin"}</span>
                    </div>
                    <span class="eq-op">{"+"}</span>
                </div>

            </div>

            // Encrypted data bar - very last thing, hugs bottom wall
            <div class="edge-bar edge-bottom-bar">
                <i class="fa-solid fa-lock ld-icon"></i>
                <span>{"Encrypted data"}</span>
            </div>
        </div>
        </div>
    }
}

// ---- Data flow between enclaves (locked data only, key comes from Marlin) ----

fn render_data_flow_between() -> Html {
    html! {
        <div class="inter-flow">
            <div class="inter-flow-cols">
                <div class="flow-data-col">
                    <div class="flow-vline orange"></div>
                    <div class="flow-badge data-badge">
                        <i class="fa-solid fa-lock"></i>
                        {" Encrypted data"}
                    </div>
                    <div class="flow-note">{"old export → new import"}</div>
                    <div class="flow-vline orange"></div>
                </div>
            </div>
        </div>
    }
}

// ---- Past enclave ----

fn render_past_enclave(build: &HistoricalBuild) -> Html {
    let commit_short = if build.commit_hash.len() > 8 {
        &build.commit_hash[..8]
    } else {
        &build.commit_hash
    };

    html! {
        <div class="enclave-cloud past">
            <div class="edge-bar edge-top-bar">
                <i class="fa-solid fa-lock ld-icon"></i>
                <span>{"Encrypted data"}</span>
            </div>
            <div class="cloud-inner">
                <div class="vert-flow">
                    <div class="stack-arrow"><i class="fa-solid fa-arrow-up"></i></div>
                    <div class="eq-item key-source-inline">
                        <i class="fa-solid fa-key ks-icon"></i>
                        <span>{"from Marlin"}</span>
                    </div>
                    <span class="eq-op">{"+"}</span>
                    <div class="eq-item">
                        <i class="fa-solid fa-lock-open eq-icon eq-green"></i>
                        <span class="eq-text">{"data"}</span>
                    </div>
                </div>
                <div class="pathway-label">{"export"}</div>
                <div class="cloud-code-label">
                    <i class="fa-solid fa-code"></i>
                    <span>{"lightfriend's code"}</span>
                </div>
                if let Some(ref ts) = build.propose_timestamp {
                    <div class="cloud-time">{relative_time(ts)}</div>
                }
                <div class="pathway-label">{"import"}</div>
                <div class="vert-flow">
                    <div class="eq-item">
                        <i class="fa-solid fa-lock-open eq-icon eq-green"></i>
                        <span class="eq-text">{"data"}</span>
                    </div>
                    <div class="stack-arrow"><i class="fa-solid fa-arrow-up"></i></div>
                    <div class="eq-item key-source-inline">
                        <i class="fa-solid fa-key ks-icon"></i>
                        <span>{"from Marlin"}</span>
                    </div>
                    <span class="eq-op">{"+"}</span>
                </div>
            </div>
            <div class="edge-bar edge-bottom-bar">
                <i class="fa-solid fa-lock ld-icon"></i>
                <span>{"Encrypted data"}</span>
            </div>
        </div>
    }
}

fn render_placeholder_enclave() -> Html {
    html! {
        <div class="enclave-cloud past placeholder">
            <div class="edge-bar edge-top-bar">
                <i class="fa-solid fa-lock ld-icon"></i>
                <span>{"Encrypted data"}</span>
            </div>
            <div class="cloud-inner">
                <div class="vert-flow">
                    <div class="stack-arrow"><i class="fa-solid fa-arrow-up"></i></div>
                    <div class="eq-item key-source-inline">
                        <i class="fa-solid fa-key ks-icon"></i>
                        <span>{"from Marlin"}</span>
                    </div>
                    <span class="eq-op">{"+"}</span>
                    <div class="eq-item">
                        <i class="fa-solid fa-lock-open eq-icon eq-green"></i>
                        <span class="eq-text">{"data"}</span>
                    </div>
                </div>
                <div class="pathway-label">{"export"}</div>
                <div class="cloud-code-label">
                    <i class="fa-solid fa-code"></i>
                    <span>{"lightfriend's code"}</span>
                </div>
                <div class="pathway-label">{"import"}</div>
                <div class="vert-flow">
                    <div class="eq-item">
                        <i class="fa-solid fa-lock-open eq-icon eq-green"></i>
                        <span class="eq-text">{"data"}</span>
                    </div>
                    <div class="stack-arrow"><i class="fa-solid fa-arrow-up"></i></div>
                    <div class="eq-item key-source-inline">
                        <i class="fa-solid fa-key ks-icon"></i>
                        <span>{"from Marlin"}</span>
                    </div>
                    <span class="eq-op">{"+"}</span>
                </div>
            </div>
            <div class="edge-bar edge-bottom-bar">
                <i class="fa-solid fa-lock ld-icon"></i>
                <span>{"Encrypted data"}</span>
            </div>
        </div>
    }
}

// ---- Governance nodes ----

fn render_dev_node() -> Html {
    html! {
        <div class="gov-node dev-node">
            <i class="fa-solid fa-user-gear node-icon"></i>
            <div class="node-content">
                <span class="node-title">{"Developer"}</span>
                <span class="node-detail">{"writes and pushes code"}</span>
            </div>
        </div>
    }
}

fn render_github_node(d: &TrustChainData) -> Html {
    let commit = d.commit_sha.as_deref().unwrap_or("unknown");
    let commit_short = short_hash(commit, 8);
    let pcr0 = d.pcr0.as_deref().unwrap_or("unavailable");
    let image_id = d.image_id.as_deref().unwrap_or("unavailable");
    let commit_url = format!("https://github.com/ahtavarasmus/lightfriend/commit/{}", commit);
    let actions_url = d.workflow_run_id.as_ref().map(|id|
        format!("https://github.com/ahtavarasmus/lightfriend/actions/runs/{}", id)
    );

    html! {
        <div class="gov-node github-node">
            <div class="node-header">
                <i class="fa-brands fa-github node-icon"></i>
                <span class="node-title">{"GitHub"}</span>
            </div>
            <div class="node-links">
                <a href={commit_url} target="_blank" rel="noopener noreferrer">
                    <i class="fa-solid fa-code-commit"></i>
                    {format!(" Commit {}", commit_short)}
                </a>
                if let Some(ref url) = actions_url {
                    <a href={url.clone()} target="_blank" rel="noopener noreferrer">
                        <i class="fa-solid fa-gears"></i>
                        {" Build job (GitHub Actions)"}
                    </a>
                }
                if let Some(ref url) = d.build_metadata_url {
                    <a href={url.clone()} target="_blank" rel="noopener noreferrer">
                        <i class="fa-solid fa-file-code"></i>
                        {" Published metadata"}
                    </a>
                }
            </div>
            <div class="node-values">
                <div class="nv-row">
                    <span class="nv-label">{"PCR0"}</span>
                    <code class="nv-val">{short_hash(pcr0, 16)}</code>
                </div>
                <div class="nv-row">
                    <span class="nv-label">{"PCR1"}</span>
                    <code class="nv-val">{short_hash(d.pcr1.as_deref().unwrap_or("unavailable"), 16)}</code>
                </div>
                <div class="nv-row">
                    <span class="nv-label">{"PCR2"}</span>
                    <code class="nv-val">{short_hash(d.pcr2.as_deref().unwrap_or("unavailable"), 16)}</code>
                </div>
                <div class="nv-row">
                    <span class="nv-label">{"Image ID"}</span>
                    <code class="nv-val">{short_hash(image_id, 16)}</code>
                </div>
            </div>
            if let Some(ref ts) = d.built_at {
                <div class="cloud-time">{"Built "}{relative_time(ts)}</div>
            }
        </div>
    }
}

fn render_arbitrum_node(d: &TrustChainData) -> Html {
    let contract_addr = d.kms_contract_address.as_deref()
        .unwrap_or("0x2e51F48F7440b415D9De30b4D73a18C8E9428982");
    let image_id = d.image_id.as_deref().unwrap_or("unavailable");
    let bc = d.blockchain.as_ref();
    let approved = bc.map_or(false, |b| b.approved);

    html! {
        <div class="gov-node arbitrum-node">
            <div class="node-header">
                <i class="fa-solid fa-link-slash node-icon"></i>
                <span class="node-title">{"Arbitrum"}</span>
                if approved {
                    <span class="node-status approved">
                        <i class="fa-solid fa-circle-check"></i>{" approved"}
                    </span>
                }
            </div>
            <div class="node-values">
                <div class="nv-row">
                    <span class="nv-label">{"Image ID"}</span>
                    <code class="nv-val">{short_hash(image_id, 16)}</code>
                </div>
                <div class="nv-row">
                    <span class="nv-label">{"Contract"}</span>
                    <code class="nv-val">{short_hash(contract_addr, 16)}</code>
                </div>
                <div class="nv-row">
                    <span class="nv-label">{"Approved"}</span>
                    if approved {
                        <span class="nv-approved"><i class="fa-solid fa-circle-check"></i>{" true"}</span>
                    } else {
                        <code class="nv-val">{"checking..."}</code>
                    }
                </div>
            </div>
            if let Some(ref b) = bc {
                if let Some(ref tx) = b.propose_tx {
                    <div class="node-txs">
                        <a href={format!("https://arbiscan.io/tx/{}", tx)} target="_blank" rel="noopener noreferrer" class="tx-link">
                            {"Proposed: "}{short_hash(tx, 8)}
                        </a>
                        if let Some(ref ts) = b.propose_timestamp {
                            <span class="tx-time" title={format_ts(ts)}>{relative_time(ts)}</span>
                        }
                    </div>
                }
                if let Some(ref tx) = b.activate_tx {
                    <div class="node-txs">
                        <a href={format!("https://arbiscan.io/tx/{}", tx)} target="_blank" rel="noopener noreferrer" class="tx-link">
                            {"Activated: "}{short_hash(tx, 8)}
                        </a>
                        if let Some(ref ts) = b.activate_timestamp {
                            <span class="tx-time" title={format_ts(ts)}>{relative_time(ts)}</span>
                        }
                    </div>
                }
            }
            <div class="node-links">
                <a href={format!("https://arbiscan.io/address/{}#readContract", contract_addr)} target="_blank" rel="noopener noreferrer">
                    <i class="fa-solid fa-magnifying-glass"></i>
                    {format!(" Check approvedImages[{}]", short_hash(image_id, 8))}
                </a>
                <a href={format!("https://arbiscan.io/address/{}#code", contract_addr)} target="_blank" rel="noopener noreferrer">
                    <i class="fa-solid fa-file-contract"></i>
                    {format!(" View contract source ({})", short_hash(contract_addr, 8))}
                </a>
            </div>
        </div>
    }
}

// ---- Marlin as its own sealed computer ----

fn render_marlin_sealed(d: &TrustChainData) -> Html {
    let pcr0 = d.pcr0.as_deref().unwrap_or("unavailable");
    let image_id = d.image_id.as_deref().unwrap_or("unavailable");
    let approved = d.blockchain.as_ref().map_or(false, |b| b.approved);
    let has_data = pcr0 != "unavailable";

    html! {
        <div class="enclave-cloud marlin-cloud">
            <div class="marlin-header">
                <span class="cloud-tag marlin-tag">{"MARLIN"}</span>
                <span class="marlin-badge">{"independent"}</span>
            </div>

            <p class="marlin-explain">
                {"Key guardian. Runs in its own sealed computer. Verifies before releasing the encryption key:"}
            </p>

            // Verification arrow 1: reads enclave attestation
            <div class="verify-check">
                <div class="verify-label">
                    <i class="fa-solid fa-arrow-right vl-arrow"></i>
                    <span>{"Reads enclave attestation"}</span>
                </div>
                <div class="verify-value-row">
                    <code>{"PCR0: "}{short_hash(pcr0, 12)}</code>
                    if has_data {
                        <i class="fa-solid fa-circle-check v-match"></i>
                    }
                </div>
            </div>

            // Verification arrow 2: reads Arbitrum contract
            <div class="verify-check">
                <div class="verify-label">
                    <i class="fa-solid fa-arrow-right vl-arrow"></i>
                    <span>{"Reads Arbitrum contract"}</span>
                </div>
                <div class="verify-value-row">
                    <code>{"approvedImages["}{short_hash(image_id, 8)}{"]"}</code>
                    if approved {
                        <span class="v-approved">{"= true"}</span>
                        <i class="fa-solid fa-circle-check v-match"></i>
                    } else {
                        <span class="v-pending">{"= ?"}</span>
                    }
                </div>
            </div>

            // Match result
            if has_data || approved {
                <div class="verify-result-line">
                    <i class="fa-solid fa-circle-check v-match"></i>
                    {" Values match → key released"}
                </div>
            }

            // KEY EXPORT PATHWAY (from Marlin to Lightfriend)
            <div class="pathway key-export-path">
                <div class="path-gate">
                    <i class="fa-solid fa-key gate-icon gate-key"></i>
                </div>
                <span class="path-label">{"export"}</span>
                <i class="fa-solid fa-arrow-right path-arrow"></i>
                <span class="key-dest">{"into Lightfriend sealed computer"}</span>
            </div>

            <div class="marlin-links">
                <a href="https://github.com/marlinprotocol/oyster-monorepo" target="_blank" rel="noopener noreferrer">
                    <i class="fa-brands fa-github"></i>
                    {" Open source code"}
                </a>
            </div>
        </div>
    }
}

fn render_gov_arrow(label: &str) -> Html {
    html! {
        <div class="gov-arrow">
            <div class="gov-arrow-line"></div>
            <span class="gov-arrow-label">{label}</span>
            <div class="gov-arrow-head"><i class="fa-solid fa-arrow-down"></i></div>
        </div>
    }
}

fn render_gov_arrow_up(label: &str) -> Html {
    html! {
        <div class="gov-arrow">
            <div class="gov-arrow-head"><i class="fa-solid fa-arrow-up"></i></div>
            <span class="gov-arrow-label">{label}</span>
            <div class="gov-arrow-line"></div>
        </div>
    }
}

// =============================================
// VERIFY SECTION
// =============================================

fn render_verify_section(d: &TrustChainData) -> Html {
    let commit = d.commit_sha.as_deref().unwrap_or("unknown");

    html! {
        <div class="tc-verify">
            <h2><i class="fa-solid fa-shield-halved"></i>{" Verify It Yourself"}</h2>
            <p class="verify-desc">
                {"Run the open-source verification tool on your own machine. It checks Amazon's signature, compares PCR values, and queries the blockchain - all independently."}
            </p>
            <pre class="verify-cmd">{format!("git clone https://github.com/ahtavarasmus/lightfriend\ncd lightfriend\ncargo run --manifest-path tools/attestation-verifier/Cargo.toml -- \\\n  https://lightfriend.ai --rpc-url https://arb1.arbitrum.io/rpc")}</pre>
            <a href={format!("https://github.com/ahtavarasmus/lightfriend/tree/{}/tools/attestation-verifier", commit)} target="_blank" rel="noopener noreferrer" class="verify-source-link">
                {"Read the tool's source code "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
            </a>
        </div>
    }
}

// =============================================
// HISTORY
// =============================================

fn render_history(history: &[HistoricalBuild]) -> Html {
    html! {
        <div class="tc-history">
            <h2>{"All Approved Builds"}</h2>
            <div class="history-list">
                {for history.iter().map(|build| {
                    let commit_url = if !build.commit_hash.is_empty() {
                        Some(format!("https://github.com/ahtavarasmus/lightfriend/commit/{}", build.commit_hash))
                    } else { None };
                    let propose_url = if !build.propose_tx.is_empty() {
                        Some(format!("https://arbiscan.io/tx/{}", build.propose_tx))
                    } else { None };

                    html! {
                        <div class={classes!("history-row", if build.is_current { "history-current" } else { "" })}>
                            <div class="history-left">
                                if build.is_current {
                                    <span class="history-badge">{"LIVE"}</span>
                                }
                                if !build.commit_hash.is_empty() {
                                    <code class="history-commit">{short_hash(&build.commit_hash, 8)}</code>
                                }
                                if let Some(ref ts) = build.propose_timestamp {
                                    <span class="history-time" title={format_ts(ts)}>{relative_time(ts)}</span>
                                }
                            </div>
                            <div class="history-right">
                                if let Some(ref url) = commit_url {
                                    <a href={url.clone()} target="_blank" rel="noopener noreferrer">{"Source"}</a>
                                }
                                if let Some(ref url) = propose_url {
                                    <a href={url.clone()} target="_blank" rel="noopener noreferrer">{"Tx"}</a>
                                }
                            </div>
                        </div>
                    }
                })}
            </div>
        </div>
    }
}

// =============================================
// STYLES
// =============================================

const STYLES: &str = r#"
/* ========================================
   Trust Chain - single accent: warm amber
   ======================================== */
.tc-page {
    max-width: 1000px;
    margin: 0 auto;
    padding: 5rem 1.5rem 3rem;
    color: #d4d4d4;
}
.tc-header { text-align: center; margin-bottom: 2.5rem; }
.tc-header h1 { font-size: 1.8rem; font-weight: 600; margin: 0 0 0.5rem; color: #f0f0f0; }
.tc-subtitle { color: rgba(255,255,255,0.5); font-size: 0.9rem; line-height: 1.5; margin: 0; }
.tc-loading { text-align: center; padding: 3rem; color: rgba(255,255,255,0.3); }

/* ---- Diagram grid ---- */
.tc-diagram {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 2rem;
    align-items: start;
}
.col-header {
    font-size: 0.7rem; font-weight: 500; text-transform: uppercase;
    letter-spacing: 0.1em; color: rgba(255,255,255,0.25);
    margin-bottom: 1rem;
}
.col-header i { margin-right: 0.3rem; }

.tc-col-enclaves { display: flex; flex-direction: column; align-items: stretch; }

/* ---- Sealed computer ---- */
.enclave-cloud {
    position: relative;
    background: rgba(255,255,255,0.02);
    border: 5px solid transparent;
    border-radius: 12px;
    padding: 0; margin: 8px 0;
    overflow: hidden;
    background-clip: padding-box;
}
.enclave-cloud::before {
    content: '';
    position: absolute; inset: -5px;
    border-radius: 12px;
    background: repeating-linear-gradient(
        45deg,
        rgba(210, 180, 80, 0.1),
        rgba(210, 180, 80, 0.1) 3px,
        transparent 3px,
        transparent 7px
    );
    z-index: -1; pointer-events: none;
}
.cloud-inner { padding: 0.75rem 1.15rem; }
.enclave-wrapper { position: relative; }
.cloud-tag-outer {
    display: block; text-align: center;
    font-size: 0.6rem; font-weight: 600; text-transform: uppercase;
    letter-spacing: 0.1em; color: rgba(255,255,255,0.35);
    margin-bottom: 0.3rem;
}
.enclave-cloud.live { background: rgba(255,255,255,0.025); }
.enclave-cloud.live::before {
    background: repeating-linear-gradient(
        45deg,
        rgba(210, 180, 80, 0.12),
        rgba(210, 180, 80, 0.12) 3px,
        transparent 3px,
        transparent 7px
    );
}
.marlin-cloud {
    background: rgba(255,255,255,0.025);
    background-clip: padding-box;
    padding: 1rem 1.25rem;
}
.marlin-cloud::before {
    background: repeating-linear-gradient(
        45deg,
        rgba(210, 180, 80, 0.1),
        rgba(210, 180, 80, 0.1) 3px,
        transparent 3px,
        transparent 7px
    );
}
.enclave-cloud.past { opacity: 0.6; }
.enclave-cloud.placeholder { opacity: 0.35; }

/* Tags */
.cloud-tag {
    font-size: 0.6rem; font-weight: 600; text-transform: uppercase;
    letter-spacing: 0.08em; padding: 0.1rem 0.5rem; border-radius: 4px;
}
.live-tag { color: rgba(255,255,255,0.4); }
.past-tag { color: rgba(255,255,255,0.3); font-family: monospace; letter-spacing: 0; font-weight: 400; }
.marlin-tag { color: rgba(255,255,255,0.5); font-weight: 600; }
.marlin-badge {
    font-size: 0.55rem; color: rgba(255,255,255,0.3);
    padding: 0.1rem 0.4rem; margin-left: auto;
}

/* Code label */
.cloud-code-label {
    display: flex; align-items: center; gap: 0.4rem;
    font-size: 0.85rem; color: rgba(255,255,255,0.7); margin-bottom: 0.5rem;
}
.cloud-code-label i { color: rgba(255,255,255,0.25); font-size: 0.7rem; }

/* Values */
.cloud-attestation {
    background: rgba(0,0,0,0.15); border: 1px solid rgba(255,255,255,0.05);
    border-radius: 6px; padding: 0.45rem 0.55rem; margin-bottom: 0.5rem;
    display: flex; flex-direction: column; gap: 0.12rem;
}
.att-row { display: flex; align-items: center; gap: 0.4rem; }
.att-label { font-size: 0.6rem; color: rgba(255,255,255,0.25); min-width: 42px; text-align: right; }
.att-val { font-family: monospace; font-size: 0.68rem; color: rgba(255,255,255,0.55); word-break: break-all; }
.cloud-time { font-size: 0.68rem; color: rgba(255,255,255,0.2); margin-bottom: 0.4rem; }
.cloud-links {
    display: flex; flex-direction: column; gap: 0.15rem; margin-bottom: 0.6rem;
}
.cloud-links a {
    font-size: 0.72rem; color: rgba(80, 160, 245, 0.85); text-decoration: none;
}
.cloud-links a:hover { text-decoration: underline; color: rgba(80, 160, 245, 1); }

/* ---- Pathways ---- */
.pathway {
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.55rem 0.75rem; border-radius: 8px;
    background: rgba(0,0,0,0.1); border: 1px solid rgba(255,255,255,0.04);
    margin: 0.35rem 0;
}
.key-export-path { border-left: 2px solid rgba(215, 185, 75, 0.3); }

.eq-item { display: inline-flex; align-items: center; gap: 0.25rem; }
.eq-icon { font-size: 0.75rem; }
.eq-green { color: rgba(80, 200, 80, 0.85); }
.eq-text { font-size: 0.75rem; color: rgba(255,255,255,0.4); }
.eq-op {
    font-size: 0.8rem; font-weight: 500; color: rgba(255,255,255,0.15);
    padding: 0 0.1rem;
}
.pathway-label {
    font-size: 0.6rem; font-weight: 500; text-transform: uppercase;
    letter-spacing: 0.1em; color: rgba(255,255,255,0.18);
    margin: 0.3rem 0 0.1rem 0; text-align: center;
}
.vert-flow {
    display: flex; flex-direction: column; align-items: center;
    gap: 0.15rem; padding: 0.2rem 0;
}
.stack-arrow { color: rgba(255,255,255,0.15); font-size: 0.65rem; }

/* Edge bar */
.edge-bar {
    display: flex; align-items: center; justify-content: center; gap: 0.3rem;
    width: 100%;
    background: rgba(215, 185, 75, 0.06);
    padding: 0.4rem 1rem;
    font-size: 0.7rem; color: rgba(215, 185, 75, 0.7);
}
.edge-bar .ld-icon { font-size: 0.6rem; color: rgba(215, 185, 75, 0.6); }
.edge-top-bar { border-bottom: 1px solid rgba(215, 185, 75, 0.1); }
.edge-bottom-bar { border-top: 1px solid rgba(215, 185, 75, 0.1); }

.locked-data-inline {
    display: inline-flex; align-items: center; gap: 0.25rem;
    background: rgba(215, 185, 75, 0.06); border: 1px solid rgba(215, 185, 75, 0.12);
    border-radius: 6px; padding: 0.15rem 0.5rem;
    font-size: 0.68rem; color: rgba(215, 185, 75, 0.7);
}
.ld-icon { font-size: 0.55rem; color: rgba(215, 185, 75, 0.6); }
.key-source-inline {
    display: inline-flex; align-items: center; gap: 0.25rem;
    background: rgba(215, 185, 75, 0.06); border: 1px solid rgba(215, 185, 75, 0.12);
    border-radius: 6px; padding: 0.15rem 0.5rem;
    font-size: 0.68rem; color: rgba(215, 185, 75, 0.65);
}
.ks-icon { font-size: 0.55rem; color: rgba(215, 185, 75, 0.6); }
.key-dest { font-size: 0.72rem; color: rgba(215, 185, 75, 0.6); }

/* ---- Inter-enclave flow ---- */
.inter-flow { padding: 0.25rem 0; }
.inter-flow-cols { display: flex; justify-content: center; }
.flow-data-col { display: flex; flex-direction: column; align-items: center; gap: 0.1rem; }
.flow-vline { width: 1px; height: 10px; }
.flow-vline.orange { background: rgba(215, 185, 75, 0.15); }
.flow-badge {
    font-size: 0.65rem; display: flex; align-items: center; gap: 0.25rem;
    padding: 0.15rem 0.5rem; border-radius: 6px;
    color: rgba(215, 185, 75, 0.6); border: 1px solid rgba(215, 185, 75, 0.1);
}
.flow-badge i { font-size: 0.55rem; }
.data-badge { color: rgba(215, 185, 75, 0.6); }
.flow-note { font-size: 0.55rem; color: rgba(255,255,255,0.18); }

/* ---- Governance column ---- */
.tc-col-governance { display: flex; flex-direction: column; align-items: stretch; }
.gov-node {
    border-radius: 10px; padding: 0.8rem 1rem;
    background: rgba(255,255,255,0.02); border: 1px solid rgba(255,255,255,0.07);
}
.node-header {
    display: flex; align-items: center; gap: 0.4rem; margin-bottom: 0.3rem;
}
.node-icon { font-size: 0.85rem; color: rgba(255,255,255,0.3); }
.node-title { font-size: 0.9rem; font-weight: 600; color: rgba(255,255,255,0.85); }
.node-status { font-size: 0.65rem; margin-left: auto; color: rgba(255,255,255,0.3); }
.node-status.approved { color: rgba(80, 200, 80, 0.85); }

.dev-node {
    display: flex; align-items: center; gap: 0.6rem;
    padding: 0.5rem 1rem; border-color: rgba(255,255,255,0.05);
}
.node-content { display: flex; flex-direction: column; }
.node-detail { font-size: 0.72rem; color: rgba(255,255,255,0.25); }

.github-node { border-color: rgba(255,255,255,0.08); }
.arbitrum-node { border-color: rgba(255,255,255,0.08); }

.node-links { display: flex; flex-direction: column; gap: 0.12rem; margin-top: 0.3rem; }
.node-links a {
    font-size: 0.72rem; color: rgba(80, 160, 245, 0.85); text-decoration: none;
}
.node-links a:hover { text-decoration: underline; color: rgba(80, 160, 245, 1); }
.node-links a i { font-size: 0.6rem; margin-right: 0.2rem; }

.node-values {
    margin-top: 0.35rem;
    background: rgba(0,0,0,0.12); border: 1px solid rgba(255,255,255,0.04);
    border-radius: 5px; padding: 0.35rem 0.5rem;
    display: flex; flex-direction: column; gap: 0.1rem;
}
.nv-row { display: flex; align-items: center; gap: 0.4rem; }
.nv-label { font-size: 0.6rem; color: rgba(255,255,255,0.25); min-width: 50px; text-align: right; }
.nv-val { font-family: monospace; font-size: 0.65rem; color: rgba(255,255,255,0.55); word-break: break-all; }
.nv-approved { font-size: 0.65rem; color: rgba(80, 200, 80, 0.85); display: flex; align-items: center; gap: 0.2rem; }
.nv-approved i { font-size: 0.6rem; }

.node-txs { margin-top: 0.25rem; display: flex; align-items: center; gap: 0.4rem; }
.tx-link { font-size: 0.65rem; color: rgba(80, 160, 245, 0.75); text-decoration: none; font-family: monospace; }
.tx-link:hover { text-decoration: underline; }
.tx-time { font-size: 0.6rem; color: rgba(255,255,255,0.2); }

/* Gov arrows */
.gov-arrow { display: flex; flex-direction: column; align-items: center; padding: 0.15rem 0; }
.gov-arrow-line { width: 1px; height: 6px; background: rgba(255,255,255,0.08); }
.gov-arrow-label { font-size: 0.6rem; color: rgba(255,255,255,0.2); padding: 0.1rem 0; }
.gov-arrow-head { color: rgba(255,255,255,0.15); font-size: 0.55rem; }

/* ---- Marlin ---- */
.marlin-header {
    display: flex; align-items: center; gap: 0.5rem; margin-bottom: 0.35rem;
}
.marlin-explain {
    font-size: 0.75rem; color: rgba(255,255,255,0.3);
    line-height: 1.4; margin: 0 0 0.5rem;
}
.verify-check {
    background: rgba(0,0,0,0.12); border: 1px solid rgba(255,255,255,0.04);
    border-radius: 6px; padding: 0.45rem 0.6rem; margin-bottom: 0.3rem;
}
.verify-label {
    display: flex; align-items: center; gap: 0.3rem;
    font-size: 0.7rem; color: rgba(255,255,255,0.35); margin-bottom: 0.2rem;
}
.vl-arrow { font-size: 0.55rem; color: rgba(255,255,255,0.2); }
.verify-value-row {
    display: flex; align-items: center; gap: 0.35rem; padding-left: 0.4rem;
}
.verify-value-row code {
    font-size: 0.65rem; color: rgba(255,255,255,0.45);
    background: rgba(255,255,255,0.04); padding: 0.1rem 0.25rem; border-radius: 3px;
}
.v-match { color: rgba(80, 200, 80, 0.85); font-size: 0.7rem; }
.v-approved { color: rgba(80, 200, 80, 0.85); font-size: 0.65rem; }
.v-pending { color: rgba(255,255,255,0.2); font-size: 0.65rem; }
.verify-result-line {
    display: flex; align-items: center; gap: 0.35rem;
    font-size: 0.72rem; color: rgba(100, 200, 100, 0.6);
    padding: 0.3rem 0 0.15rem;
}
.marlin-links { margin-top: 0.35rem; }
.marlin-links a {
    font-size: 0.72rem; color: rgba(80, 160, 245, 0.85); text-decoration: none;
}
.marlin-links a:hover { text-decoration: underline; }

/* ---- Verify section ---- */
.tc-verify {
    margin-top: 3rem; padding-top: 2rem;
    border-top: 1px solid rgba(255,255,255,0.06);
}
.tc-verify h2 { font-size: 1.1rem; margin: 0 0 0.5rem; color: #d4d4d4; }
.tc-verify h2 i { color: rgba(255,255,255,0.3); font-size: 0.9rem; }
.verify-desc { color: rgba(255,255,255,0.35); font-size: 0.8rem; line-height: 1.5; margin: 0 0 1rem; }
.verify-cmd {
    background: rgba(0,0,0,0.2); border: 1px solid rgba(255,255,255,0.06);
    padding: 0.75rem 1rem; border-radius: 6px; font-size: 0.72rem;
    color: rgba(255,255,255,0.45); overflow-x: auto; white-space: pre;
    font-family: monospace; margin: 0 0 0.5rem;
}
.verify-source-link { font-size: 0.75rem; color: rgba(80, 160, 245, 0.85); text-decoration: none; }
.verify-source-link:hover { text-decoration: underline; }

/* ---- History ---- */
.tc-history {
    margin-top: 2.5rem; padding-top: 2rem;
    border-top: 1px solid rgba(255,255,255,0.06);
}
.tc-history h2 { font-size: 1.1rem; margin: 0 0 0.6rem; color: #d4d4d4; }
.history-list { display: flex; flex-direction: column; gap: 0.3rem; }
.history-row {
    display: flex; justify-content: space-between; align-items: center;
    padding: 0.4rem 0.6rem; background: rgba(255,255,255,0.02);
    border: 1px solid rgba(255,255,255,0.04); border-radius: 6px;
    flex-wrap: wrap; gap: 0.3rem;
}
.history-current { border-color: rgba(100, 200, 100, 0.15); }
.history-left { display: flex; align-items: center; gap: 0.4rem; }
.history-badge {
    font-size: 0.5rem; font-weight: 600; letter-spacing: 0.05em;
    padding: 0.08rem 0.3rem; border-radius: 3px;
    background: rgba(100, 200, 100, 0.12); color: rgba(80, 200, 80, 0.85);
}
.history-commit { font-family: monospace; font-size: 0.72rem; color: rgba(255,255,255,0.45); }
.history-time { font-size: 0.65rem; color: rgba(255,255,255,0.2); }
.history-right { display: flex; gap: 0.5rem; }
.history-right a { font-size: 0.7rem; color: rgba(80, 160, 245, 0.75); text-decoration: none; }
.history-right a:hover { text-decoration: underline; }

/* ---- Footer ---- */
.tc-footer-links { margin-top: 2.5rem; text-align: center; }
.tc-footer-links a {
    color: rgba(80, 160, 245, 0.85); text-decoration: none; font-size: 0.85rem;
}
.tc-footer-links a:hover { text-decoration: underline; }
.tc-page .legal-links {
    margin-top: 1.5rem; text-align: center; font-size: 0.75rem; color: rgba(255,255,255,0.25);
}
.tc-page .legal-links a { color: rgba(255,255,255,0.3); text-decoration: none; }
.tc-page .legal-links a:hover { color: rgba(80, 160, 245, 0.85); }

@media (max-width: 768px) {
    .tc-page { padding: 3rem 1rem 2rem; }
    .tc-header h1 { font-size: 1.4rem; }
    .tc-diagram { grid-template-columns: 1fr; gap: 2rem; }
    .att-row { flex-direction: column; gap: 0.05rem; align-items: flex-start; }
    .att-label { min-width: auto; text-align: left; }
    .inter-flow-cols { flex-direction: column; gap: 0.3rem; }
}
"#;
