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

// Collapsible details component
#[derive(Properties, PartialEq)]
struct DetailsProps {
    pub summary: String,
    pub children: Children,
}

#[function_component(Details)]
fn details(props: &DetailsProps) -> Html {
    let open = use_state(|| false);
    let toggle = {
        let open = open.clone();
        Callback::from(move |_: MouseEvent| open.set(!*open))
    };
    html! {
        <div class={classes!("tc-details", (*open).then(|| "open"))}>
            <button class="tc-details-toggle" onclick={toggle}>
                <i class={classes!("fa-solid", if *open {"fa-chevron-down"} else {"fa-chevron-right"})}></i>
                <span>{&props.summary}</span>
            </button>
            if *open {
                <div class="tc-details-content">
                    {for props.children.iter()}
                </div>
            }
        </div>
    }
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
                <p class="tc-intro">
                    {"Your data is encrypted inside a sealed computer that nobody can peek inside - not even us. "}
                    {"This page shows the live proof. Every link opens a third-party site you can check yourself."}
                </p>
            </div>

            if *loading {
                <div class="tc-loading">
                    <i class="fa-solid fa-spinner fa-spin"></i>{" Loading..."}
                </div>
            } else if let Some(ref d) = d {
                {render_diagram(d)}
                {render_verify_section(d)}
            } else {
                <div class="tc-loading">
                    {"Could not load trust chain data. This page works best when connected to a live Lightfriend instance."}
                </div>
            }

            <div class="tc-footer-links">
                <Link<Route> to={Route::Trustless}>
                    {"Want the full explanation? Read how it all works"}
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
                <p class="col-desc">{"Your data lives inside sealed rooms nobody can peek inside. Data leaves only encrypted."}</p>

                {render_live_enclave(d)}
                {render_data_flow_between()}
                if let Some(build) = past_builds.first() {
                    {render_past_enclave(build, d)}
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
                <p class="col-desc">{"How anyone can verify the code running inside, and how the encryption key is protected."}</p>

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
            <div class="live-indicator">
                <span class="live-dot"></span>
                <span class="live-text">{"Running now"}</span>
            </div>
            <div class="enclave-cloud live">
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
                        if let Some(ref ts) = d.built_at {
                            <span class="cloud-time">{" - built "}{relative_time(ts)}</span>
                        }
                    </div>

                    <Details summary="Attestation values (fingerprints)">
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
                        <div class="cloud-links">
                            <a href="/.well-known/lightfriend/attestation" target="_blank" rel="noopener noreferrer">
                                <i class="fa-solid fa-certificate"></i>{" Live attestation endpoint"}
                            </a>
                            <a href={format!("https://github.com/ahtavarasmus/lightfriend/tree/{}/tools/attestation-verifier", commit)} target="_blank" rel="noopener noreferrer">
                                <i class="fa-solid fa-magnifying-glass"></i>{" Verification tool source code"}
                            </a>
                        </div>
                    </Details>

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
        </div>
    }
}

// ---- Data flow between enclaves ----

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
                    <div class="flow-note">{"old export \u{2192} new import"}</div>
                    <div class="flow-vline orange"></div>
                </div>
            </div>
        </div>
    }
}

// ---- Past enclave with expandable details ----

fn render_past_enclave(build: &HistoricalBuild, d: &TrustChainData) -> Html {
    let commit_short = short_hash(&build.commit_hash, 8);
    let commit_url = format!("https://github.com/ahtavarasmus/lightfriend/commit/{}", build.commit_hash);
    let image_id_short = short_hash(&build.image_id, 12);
    let contract_addr = d.kms_contract_address.as_deref()
        .unwrap_or("0x2e51F48F7440b415D9De30b4D73a18C8E9428982");

    html! {
        <div class="enclave-wrapper">
            <div class="past-indicator">
                <span class="past-text">{"Previous version"}</span>
                if let Some(ref ts) = build.propose_timestamp {
                    <span class="past-time">{" - "}{relative_time(ts)}</span>
                }
            </div>
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

                    <Details summary={format!("Details - commit {}", commit_short)}>
                        <div class="past-details-links">
                            <a href={commit_url} target="_blank" rel="noopener noreferrer">
                                <i class="fa-solid fa-code-commit"></i>{format!(" View commit {} on GitHub", commit_short)}
                            </a>
                            if !build.propose_tx.is_empty() {
                                <a href={format!("https://arbiscan.io/tx/{}", build.propose_tx)} target="_blank" rel="noopener noreferrer">
                                    <i class="fa-solid fa-link-slash"></i>{" Blockchain approval transaction"}
                                </a>
                            }
                            if let Some(ref tx) = build.activate_tx {
                                <a href={format!("https://arbiscan.io/tx/{}", tx)} target="_blank" rel="noopener noreferrer">
                                    <i class="fa-solid fa-circle-check"></i>{" Activation transaction"}
                                </a>
                            }
                            <a href={format!("https://arbiscan.io/address/{}#events", contract_addr)} target="_blank" rel="noopener noreferrer">
                                <i class="fa-solid fa-list"></i>{" All approved builds on Arbiscan"}
                            </a>
                        </div>
                        <div class="cloud-attestation">
                            <div class="att-row">
                                <span class="att-label">{"Image ID"}</span>
                                <code class="att-val">{&image_id_short}</code>
                            </div>
                            if let Some(ref ts) = build.propose_timestamp {
                                <div class="att-row">
                                    <span class="att-label">{"Proposed"}</span>
                                    <code class="att-val">{format_ts(ts)}</code>
                                </div>
                            }
                        </div>
                    </Details>

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
        </div>
    }
}

fn render_placeholder_enclave() -> Html {
    html! {
        <div class="enclave-wrapper">
            <div class="past-indicator">
                <span class="past-text">{"Previous version"}</span>
            </div>
            <div class="enclave-cloud past placeholder">
                <div class="edge-bar edge-top-bar">
                    <i class="fa-solid fa-lock ld-icon"></i>
                    <span>{"Encrypted data"}</span>
                </div>
                <div class="cloud-inner">
                    <div class="cloud-code-label">
                        <i class="fa-solid fa-code"></i>
                        <span>{"lightfriend's code"}</span>
                    </div>
                </div>
                <div class="edge-bar edge-bottom-bar">
                    <i class="fa-solid fa-lock ld-icon"></i>
                    <span>{"Encrypted data"}</span>
                </div>
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
            <p class="node-desc">{"All code is public. A robot built it and published the fingerprint."}</p>
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
            <Details summary="Fingerprints from this build">
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
            </Details>
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
            <p class="node-desc">{"Public blockchain record of which builds are approved. Anyone can check."}</p>
            <Details summary="Contract values and links">
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
                    <a href={format!("https://arbiscan.io/address/{}#events", contract_addr)} target="_blank" rel="noopener noreferrer">
                        <i class="fa-solid fa-magnifying-glass"></i>
                        {format!(" Check approvedImages[{}]", short_hash(image_id, 8))}
                    </a>
                    <a href={format!("https://arbiscan.io/address/{}#code", contract_addr)} target="_blank" rel="noopener noreferrer">
                        <i class="fa-solid fa-file-contract"></i>
                        {format!(" View contract source ({})", short_hash(contract_addr, 8))}
                    </a>
                </div>
            </Details>
        </div>
    }
}

// ---- Marlin sealed computer ----

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
                {"Key guardian. Runs in its own sealed computer. Verifies the enclave's identity before releasing the encryption key."}
            </p>

            <Details summary="How Marlin verifies">
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

                if has_data || approved {
                    <div class="verify-result-line">
                        <i class="fa-solid fa-circle-check v-match"></i>
                        {" Values match - key released into sealed computer"}
                    </div>
                }
            </Details>

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
                {"Run the open-source verification tool on your own machine. It checks Amazon's signature, compares fingerprints, and queries the blockchain independently."}
            </p>
            <pre class="verify-cmd">{format!("git clone https://github.com/ahtavarasmus/lightfriend\ncd lightfriend\ncargo run --manifest-path tools/attestation-verifier/Cargo.toml -- \\\n  https://lightfriend.ai --rpc-url https://arb1.arbitrum.io/rpc")}</pre>
            <a href={format!("https://github.com/ahtavarasmus/lightfriend/tree/{}/tools/attestation-verifier", commit)} target="_blank" rel="noopener noreferrer" class="verify-source-link">
                {"Read the tool's source code "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
            </a>
        </div>
    }
}

// =============================================
// STYLES
// =============================================

const STYLES: &str = r#"
.tc-page {
    max-width: 1000px;
    margin: 0 auto;
    padding: 5rem 1.5rem 3rem;
    color: #e0e0e0;
}
.tc-header { text-align: center; margin-bottom: 2.5rem; }
.tc-header h1 { font-size: 1.8rem; font-weight: 600; margin: 0 0 0.75rem; color: #f0f0f0; }
.tc-intro { color: rgba(255,255,255,0.55); font-size: 0.9rem; line-height: 1.6; margin: 0 auto; max-width: 600px; }
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
    letter-spacing: 0.1em; color: rgba(255,255,255,0.3);
    margin-bottom: 0.3rem;
}
.col-header i { margin-right: 0.3rem; }
.col-desc {
    font-size: 0.8rem; color: rgba(255,255,255,0.4); line-height: 1.4;
    margin: 0 0 1rem;
}

.tc-col-enclaves { display: flex; flex-direction: column; align-items: stretch; }

/* ---- Collapsible details ---- */
.tc-details { margin: 0.3rem 0; }
.tc-details-toggle {
    display: flex; align-items: center; gap: 0.35rem;
    background: none; border: none; color: #1E90FF;
    font-size: 0.78rem; cursor: pointer; padding: 0.2rem 0;
}
.tc-details-toggle:hover { color: #4da6ff; }
.tc-details-toggle i { font-size: 0.55rem; width: 0.7rem; }
.tc-details-content {
    padding: 0.4rem 0 0.2rem 1rem;
    border-left: 1px solid rgba(255,255,255,0.06);
    margin-top: 0.2rem;
}

/* ---- Sealed computer ---- */
.enclave-cloud {
    position: relative;
    background: rgba(255,255,255,0.02);
    border: 2px solid rgba(255,255,255,0.12);
    border-radius: 12px;
    padding: 0; margin: 0;
    overflow: hidden;
    /* Double border via outline */
    outline: 2px solid rgba(255,255,255,0.06);
    outline-offset: 3px;
    /* Recessed inner shadow */
    box-shadow: inset 0 0 20px rgba(0,0,0,0.3);
}
/* Corner lock icons */
.enclave-cloud::before,
.enclave-cloud::after {
    font-family: 'Font Awesome 6 Free'; font-weight: 900;
    content: '\f023';
    position: absolute;
    font-size: 0.5rem;
    color: rgba(255, 215, 0, 0.3);
    z-index: 2;
    pointer-events: none;
}
.enclave-cloud::before { top: 6px; left: 8px; }
.enclave-cloud::after { top: 6px; right: 8px; }
.cloud-inner { padding: 0.75rem 1.15rem; }
.enclave-wrapper { margin: 8px 0; }

/* Live indicator */
.live-indicator {
    display: flex; align-items: center; gap: 0.4rem;
    justify-content: center; margin-bottom: 0.4rem;
}
.live-dot {
    width: 8px; height: 8px; border-radius: 50%;
    background: #4CAF50;
    box-shadow: 0 0 6px #4CAF50;
    animation: pulse-glow 2s ease-in-out infinite;
}
@keyframes pulse-glow {
    0%, 100% { box-shadow: 0 0 4px #4CAF50; }
    50% { box-shadow: 0 0 12px #4CAF50, 0 0 20px rgba(76,175,80,0.3); }
}
.live-text { font-size: 0.75rem; font-weight: 600; color: #4CAF50; }

/* Past indicator */
.past-indicator {
    display: flex; align-items: center; gap: 0.2rem;
    justify-content: center; margin-bottom: 0.3rem;
}
.past-text { font-size: 0.7rem; color: rgba(255,255,255,0.35); }
.past-time { font-size: 0.65rem; color: rgba(255,255,255,0.25); }

.enclave-cloud.live {
    background: rgba(255,255,255,0.03);
    border-color: rgba(76, 175, 80, 0.25);
    outline-color: rgba(76, 175, 80, 0.1);
}
.enclave-cloud.live::before,
.enclave-cloud.live::after { color: rgba(76, 175, 80, 0.35); }
.marlin-cloud {
    background: rgba(255,255,255,0.025);
    padding: 1rem 1.25rem;
    border-color: rgba(156, 39, 176, 0.2);
    outline-color: rgba(156, 39, 176, 0.08);
}
.marlin-cloud::before,
.marlin-cloud::after { color: rgba(156, 39, 176, 0.3); }
.enclave-cloud.past { opacity: 0.6; }
.enclave-cloud.placeholder { opacity: 0.35; }

/* Tags */
.cloud-tag {
    font-size: 0.6rem; font-weight: 600; text-transform: uppercase;
    letter-spacing: 0.08em; padding: 0.1rem 0.5rem; border-radius: 4px;
}
.cloud-tag-outer {
    display: block; text-align: center;
    font-size: 0.6rem; font-weight: 600; text-transform: uppercase;
    letter-spacing: 0.1em; color: rgba(255,255,255,0.35);
    margin-bottom: 0.3rem;
}
.live-tag { color: rgba(255,255,255,0.4); }
.marlin-tag { color: rgba(255,255,255,0.5); font-weight: 600; }
.marlin-badge {
    font-size: 0.55rem; color: rgba(255,255,255,0.3);
    padding: 0.1rem 0.4rem; margin-left: auto;
}

/* Code label */
.cloud-code-label {
    display: flex; align-items: center; gap: 0.4rem;
    font-size: 0.88rem; color: rgba(255,255,255,0.8); margin-bottom: 0.3rem;
}
.cloud-code-label i { color: #1E90FF; font-size: 0.75rem; }

/* Values */
.cloud-attestation {
    background: rgba(0,0,0,0.15); border: 1px solid rgba(255,255,255,0.05);
    border-radius: 6px; padding: 0.45rem 0.55rem; margin-bottom: 0.4rem;
    display: flex; flex-direction: column; gap: 0.12rem;
}
.att-row { display: flex; align-items: center; gap: 0.4rem; }
.att-label { font-size: 0.6rem; color: rgba(255,255,255,0.3); min-width: 50px; text-align: right; }
.att-val { font-family: monospace; font-size: 0.72rem; color: rgba(255,255,255,0.7); word-break: break-all; }
.cloud-time { font-size: 0.72rem; color: rgba(255,255,255,0.3); }
.cloud-links {
    display: flex; flex-direction: column; gap: 0.15rem; margin-top: 0.3rem;
}
.cloud-links a {
    font-size: 0.78rem; color: #1E90FF; text-decoration: none;
}
.cloud-links a:hover { text-decoration: underline; }

/* Past enclave details */
.past-details-links {
    display: flex; flex-direction: column; gap: 0.15rem; margin-bottom: 0.4rem;
}
.past-details-links a {
    font-size: 0.78rem; color: #1E90FF; text-decoration: none;
    display: inline-flex; align-items: center; gap: 0.3rem;
}
.past-details-links a:hover { text-decoration: underline; }

/* ---- Pathways ---- */
.pathway {
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.55rem 0.75rem; border-radius: 8px;
    background: rgba(0,0,0,0.1); border: 1px solid rgba(255,255,255,0.04);
    margin: 0.35rem 0;
}
.key-export-path { border-left: 2px solid rgba(255, 215, 0, 0.5); }
.path-gate { flex-shrink: 0; }
.gate-icon { font-size: 0.85rem; }
.gate-key { color: rgba(255, 215, 0, 0.7); }
.path-label { font-size: 0.75rem; color: rgba(255,255,255,0.4); }
.path-arrow { font-size: 0.65rem; color: rgba(255,255,255,0.2); }
.key-dest { font-size: 0.72rem; color: rgba(255, 215, 0, 0.5); }

.eq-item { display: inline-flex; align-items: center; gap: 0.25rem; }
.eq-icon { font-size: 0.75rem; }
.eq-green { color: #4CAF50; }
.eq-text { font-size: 0.75rem; color: rgba(255,255,255,0.5); }
.eq-op {
    font-size: 0.8rem; font-weight: 500; color: rgba(255,255,255,0.2);
    padding: 0 0.1rem;
}
.pathway-label {
    font-size: 0.6rem; font-weight: 500; text-transform: uppercase;
    letter-spacing: 0.1em; color: rgba(255,255,255,0.2);
    margin: 0.3rem 0 0.1rem 0; text-align: center;
}
.vert-flow {
    display: flex; flex-direction: column; align-items: center;
    gap: 0.15rem; padding: 0.2rem 0;
}
.stack-arrow { color: rgba(255,255,255,0.2); font-size: 0.65rem; }

/* Edge bar */
.edge-bar {
    display: flex; align-items: center; justify-content: center; gap: 0.3rem;
    width: 100%;
    background: rgba(255, 152, 0, 0.06);
    padding: 0.4rem 1rem;
    font-size: 0.7rem; color: rgba(255, 152, 0, 0.6);
}
.edge-bar .ld-icon { font-size: 0.6rem; color: rgba(255, 152, 0, 0.5); }
.edge-top-bar { border-bottom: 1px solid rgba(255, 152, 0, 0.1); }
.edge-bottom-bar { border-top: 1px solid rgba(255, 152, 0, 0.1); }

.locked-data-inline {
    display: inline-flex; align-items: center; gap: 0.25rem;
    background: rgba(255, 152, 0, 0.06); border: 1px solid rgba(255, 152, 0, 0.12);
    border-radius: 6px; padding: 0.15rem 0.5rem;
    font-size: 0.68rem; color: rgba(255, 152, 0, 0.6);
}
.ld-icon { font-size: 0.55rem; color: rgba(255, 152, 0, 0.5); }
.key-source-inline {
    display: inline-flex; align-items: center; gap: 0.25rem;
    background: rgba(255, 215, 0, 0.06); border: 1px solid rgba(255, 215, 0, 0.12);
    border-radius: 6px; padding: 0.15rem 0.5rem;
    font-size: 0.68rem; color: rgba(255, 215, 0, 0.6);
}
.ks-icon { font-size: 0.55rem; color: rgba(255, 215, 0, 0.5); }

/* ---- Inter-enclave flow ---- */
.inter-flow { padding: 0.25rem 0; }
.inter-flow-cols { display: flex; justify-content: center; }
.flow-data-col { display: flex; flex-direction: column; align-items: center; gap: 0.1rem; }
.flow-vline { width: 1px; height: 10px; }
.flow-vline.orange { background: rgba(255, 152, 0, 0.2); }
.flow-badge {
    font-size: 0.65rem; display: flex; align-items: center; gap: 0.25rem;
    padding: 0.15rem 0.5rem; border-radius: 6px;
    color: rgba(255, 152, 0, 0.5); border: 1px solid rgba(255, 152, 0, 0.12);
}
.flow-badge i { font-size: 0.55rem; }
.data-badge { color: rgba(255, 152, 0, 0.5); }
.flow-note { font-size: 0.55rem; color: rgba(255,255,255,0.2); }

/* ---- Governance column ---- */
.tc-col-governance { display: flex; flex-direction: column; align-items: stretch; }
.gov-node {
    border-radius: 10px; padding: 0.8rem 1rem;
    background: rgba(255,255,255,0.04); border: 1px solid rgba(255,255,255,0.1);
}
.node-header {
    display: flex; align-items: center; gap: 0.4rem; margin-bottom: 0.2rem;
}
.node-icon { font-size: 0.85rem; color: rgba(255,255,255,0.5); }
.node-title { font-size: 0.95rem; font-weight: 600; color: #fff; }
.node-desc { font-size: 0.78rem; color: rgba(255,255,255,0.4); line-height: 1.4; margin: 0 0 0.4rem; }
.node-status { font-size: 0.65rem; margin-left: auto; color: rgba(255,255,255,0.3); }
.node-status.approved { color: #4CAF50; }

.dev-node {
    display: flex; align-items: center; gap: 0.6rem;
    padding: 0.5rem 1rem; border-color: rgba(255,255,255,0.05);
}
.node-content { display: flex; flex-direction: column; }
.node-detail { font-size: 0.72rem; color: rgba(255,255,255,0.3); }

.node-links { display: flex; flex-direction: column; gap: 0.12rem; margin-top: 0.3rem; }
.node-links a {
    font-size: 0.78rem; color: #1E90FF; text-decoration: none;
    display: inline-flex; align-items: center; gap: 0.3rem;
}
.node-links a:hover { text-decoration: underline; }

.node-values {
    margin-top: 0.2rem;
    background: rgba(0,0,0,0.12); border: 1px solid rgba(255,255,255,0.04);
    border-radius: 5px; padding: 0.35rem 0.5rem;
    display: flex; flex-direction: column; gap: 0.1rem;
}
.nv-row { display: flex; align-items: center; gap: 0.4rem; }
.nv-label { font-size: 0.6rem; color: rgba(255,255,255,0.3); min-width: 50px; text-align: right; }
.nv-val { font-family: monospace; font-size: 0.7rem; color: rgba(255,255,255,0.7); word-break: break-all; }
.nv-approved { font-size: 0.65rem; color: #4CAF50; display: flex; align-items: center; gap: 0.2rem; }

.node-txs { margin-top: 0.25rem; display: flex; align-items: center; gap: 0.4rem; }
.tx-link { font-size: 0.65rem; color: rgba(30, 144, 255, 0.8); text-decoration: none; font-family: monospace; }
.tx-link:hover { text-decoration: underline; }
.tx-time { font-size: 0.6rem; color: rgba(255,255,255,0.25); }

/* Gov arrows */
.gov-arrow { display: flex; flex-direction: column; align-items: center; padding: 0.15rem 0; }
.gov-arrow-line { width: 1px; height: 6px; background: rgba(255,255,255,0.1); }
.gov-arrow-label { font-size: 0.6rem; color: rgba(255,255,255,0.25); padding: 0.1rem 0; }
.gov-arrow-head { color: rgba(255,255,255,0.18); font-size: 0.55rem; }

/* ---- Marlin ---- */
.marlin-header {
    display: flex; align-items: center; gap: 0.5rem; margin-bottom: 0.35rem;
}
.marlin-explain {
    font-size: 0.78rem; color: rgba(255,255,255,0.4);
    line-height: 1.4; margin: 0 0 0.4rem;
}
.verify-check {
    background: rgba(0,0,0,0.12); border: 1px solid rgba(255,255,255,0.04);
    border-radius: 6px; padding: 0.45rem 0.6rem; margin-bottom: 0.3rem;
}
.verify-label {
    display: flex; align-items: center; gap: 0.3rem;
    font-size: 0.7rem; color: rgba(255,255,255,0.4); margin-bottom: 0.2rem;
}
.vl-arrow { font-size: 0.55rem; color: rgba(255,255,255,0.25); }
.verify-value-row {
    display: flex; align-items: center; gap: 0.35rem; padding-left: 0.4rem;
}
.verify-value-row code {
    font-size: 0.65rem; color: rgba(255,255,255,0.6);
    background: rgba(255,255,255,0.04); padding: 0.1rem 0.25rem; border-radius: 3px;
}
.v-match { color: #4CAF50; font-size: 0.7rem; }
.v-approved { color: #4CAF50; font-size: 0.65rem; }
.v-pending { color: rgba(255,255,255,0.25); font-size: 0.65rem; }
.verify-result-line {
    display: flex; align-items: center; gap: 0.35rem;
    font-size: 0.72rem; color: rgba(76, 175, 80, 0.7);
    padding: 0.3rem 0 0.15rem;
}
.marlin-links { margin-top: 0.35rem; }
.marlin-links a {
    font-size: 0.78rem; color: #1E90FF; text-decoration: none;
}
.marlin-links a:hover { text-decoration: underline; }

/* ---- Verify section ---- */
.tc-verify {
    margin-top: 3rem; padding-top: 2rem;
    border-top: 1px solid rgba(255,255,255,0.06);
}
.tc-verify h2 { font-size: 1.1rem; margin: 0 0 0.5rem; color: #e0e0e0; }
.tc-verify h2 i { color: rgba(255,255,255,0.4); font-size: 0.9rem; }
.verify-desc { color: rgba(255,255,255,0.4); font-size: 0.8rem; line-height: 1.5; margin: 0 0 1rem; }
.verify-cmd {
    background: rgba(0,0,0,0.2); border: 1px solid rgba(255,255,255,0.06);
    padding: 0.75rem 1rem; border-radius: 6px; font-size: 0.72rem;
    color: rgba(255,255,255,0.55); overflow-x: auto; white-space: pre;
    font-family: monospace; margin: 0 0 0.5rem;
}
.verify-source-link { font-size: 0.75rem; color: #1E90FF; text-decoration: none; }
.verify-source-link:hover { text-decoration: underline; }

/* ---- Footer ---- */
.tc-footer-links { margin-top: 2.5rem; text-align: center; }
.tc-footer-links a {
    color: #1E90FF; text-decoration: none; font-size: 0.85rem;
}
.tc-footer-links a:hover { text-decoration: underline; }
.tc-page .legal-links {
    margin-top: 1.5rem; text-align: center; font-size: 0.75rem; color: rgba(255,255,255,0.25);
}
.tc-page .legal-links a { color: rgba(255,255,255,0.35); text-decoration: none; }
.tc-page .legal-links a:hover { color: #1E90FF; }

@media (max-width: 768px) {
    .tc-page { padding: 3rem 1rem 2rem; }
    .tc-header h1 { font-size: 1.4rem; }
    .tc-diagram { grid-template-columns: 1fr; gap: 2rem; }
    .att-row { flex-direction: column; gap: 0.05rem; align-items: flex-start; }
    .att-label { min-width: auto; text-align: left; }
    .inter-flow-cols { flex-direction: column; gap: 0.3rem; }
}
"#;
