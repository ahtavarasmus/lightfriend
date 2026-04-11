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

fn format_date(iso: &str) -> String {
    let date = js_sys::Date::new(&JsValue::from_str(iso));
    let months = ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"];
    let m = date.get_utc_month() as usize;
    let month = months.get(m).unwrap_or(&"???");
    format!("{} {} {}", date.get_utc_date(), month, date.get_utc_full_year())
}

// -- Main page --

#[function_component(TrustChainPage)]
pub fn trust_chain_page() -> Html {
    use_seo(SeoMeta {
        title: "Trust Chain - Lightfriend",
        description: "Follow the cryptographic proof chain from open source code to running enclave to blockchain attestation. Verify Lightfriend's privacy yourself - no trust required.",
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
            </div>

            <div class="tc-intro-box">
                <p class="tc-intro-text">
                    {"Your messages are processed inside a sealed hardware enclave. Not even Lightfriend can access your data."}
                </p>
                <p class="tc-intro-hw">
                    {"Hardware attested by "}
                    <strong>{"AWS Nitro"}</strong>
                </p>
                <a href="https://github.com/ahtavarasmus/lightfriend" target="_blank" rel="noopener noreferrer" class="tc-intro-link">
                    <i class="fa-brands fa-github"></i>{" github.com"}
                </a>
            </div>

            if *loading {
                <div class="tc-loading">
                    <i class="fa-solid fa-spinner fa-spin"></i>{" Loading..."}
                </div>
            } else if let Some(ref d) = d {
                {render_diagram()}
                <PillarsSection data={d.clone()} />
                {render_history_chain(d)}
                {render_verify_section(d)}
            } else {
                <div class="tc-loading">
                    {"Could not load trust chain data."}
                </div>
            }

            <div class="tc-footer-links">
                <Link<Route> to={Route::Trustless}>
                    {"Full explanation: how it all works"}
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

fn render_diagram() -> Html {
    html! {
        <div class="tc-diagram">
            <div class="tc-proof-boundary">
                <div class="tc-proof-badge">
                    <i class="fa-solid fa-shield-halved"></i>
                    {" Attestation Proof"}
                </div>

                <div class="tc-diagram-layout">
                    <div class="tc-node tc-kms-node">
                        <div class="tc-node-title">
                            <i class="fa-solid fa-key"></i>
                            {" Marlin KMS"}
                        </div>
                        <ul class="tc-node-points">
                            <li>{"Verifies attestation"}</li>
                            <li>{"Manages encryption keys"}</li>
                        </ul>
                    </div>

                    <div class="tc-node tc-server-node">
                        <div class="tc-node-title">
                            <i class="fa-solid fa-server"></i>
                            {" Lightfriend Server"}
                        </div>
                        <ul class="tc-node-points">
                            <li>{"Cannot access user data"}</li>
                            <li>{"Cannot see inside the enclave"}</li>
                        </ul>

                        <div class="tc-node tc-enclave-node">
                            <div class="tc-node-title">
                                <i class="fa-solid fa-lock"></i>
                                {" Nitro Enclave"}
                            </div>
                            <p class="tc-node-desc">
                                {"Messages processed in sealed hardware, isolated from the server."}
                            </p>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}

// =============================================
// PILLARS (interactive verification boxes)
// =============================================

#[derive(Properties, PartialEq)]
struct PillarsProps {
    pub data: TrustChainData,
}

#[function_component(PillarsSection)]
fn pillars_section(props: &PillarsProps) -> Html {
    let selected = use_state(|| Option::<usize>::None);
    let show_extra = use_state(|| false);
    let d = &props.data;

    let make_cb = {
        let selected = selected.clone();
        let show_extra = show_extra.clone();
        move |idx: usize| {
            let selected = selected.clone();
            let show_extra = show_extra.clone();
            Callback::from(move |_: MouseEvent| {
                if *selected == Some(idx) {
                    selected.set(None);
                } else {
                    selected.set(Some(idx));
                    show_extra.set(false);
                }
            })
        }
    };

    let toggle_extra = {
        let show_extra = show_extra.clone();
        Callback::from(move |_: MouseEvent| show_extra.set(!*show_extra))
    };

    html! {
        <div class="tc-pillars">
            <div class="tc-pillar-boxes">
                <button class={classes!("tc-pillar-box", (*selected == Some(0)).then(|| "selected"))}
                        onclick={make_cb(0)}>
                    <span class="tc-check"><i class="fa-solid fa-circle-check"></i></span>
                    <span class="tc-pillar-label">{"Code is"}<br/>{"Auditable"}</span>
                    <i class="fa-solid fa-terminal tc-pillar-icon"></i>
                </button>
                <button class={classes!("tc-pillar-box", (*selected == Some(1)).then(|| "selected"))}
                        onclick={make_cb(1)}>
                    <span class="tc-check"><i class="fa-solid fa-circle-check"></i></span>
                    <span class="tc-pillar-label">{"Runtime is"}<br/>{"Isolated"}</span>
                    <i class="fa-solid fa-shield-halved tc-pillar-icon"></i>
                </button>
                <button class={classes!("tc-pillar-box", (*selected == Some(2)).then(|| "selected"))}
                        onclick={make_cb(2)}>
                    <span class="tc-check"><i class="fa-solid fa-circle-check"></i></span>
                    <span class="tc-pillar-label">{"Data is"}<br/>{"Encrypted"}</span>
                    <i class="fa-solid fa-lock tc-pillar-icon"></i>
                </button>
            </div>

            if let Some(idx) = *selected {
                <div class="tc-pillar-detail">
                    {match idx {
                        0 => render_pillar_auditable(d, *show_extra, toggle_extra.clone()),
                        1 => render_pillar_isolated(d, *show_extra, toggle_extra.clone()),
                        _ => render_pillar_encrypted(d, *show_extra, toggle_extra.clone()),
                    }}
                </div>
            }
        </div>
    }
}

fn render_pillar_encrypted(d: &TrustChainData, show_extra: bool, toggle: Callback<MouseEvent>) -> Html {
    let image_id = d.image_id.as_deref().unwrap_or("unavailable");
    let approved = d.blockchain.as_ref().map_or(false, |b| b.approved);
    let contract_addr = d.kms_contract_address.as_deref()
        .unwrap_or("0x2e51F48F7440b415D9De30b4D73a18C8E9428982");

    html! {
        <>
            <h3 class="tc-detail-title">{"Data is encrypted"}</h3>
            <p class="tc-detail-desc">
                {"Your data is encrypted using a key managed by Marlin KMS. It only releases the encryption key after verifying this exact Image ID is approved on-chain."}
            </p>

            <div class="tc-fingerprint">
                <i class="fa-solid fa-fingerprint tc-fp-icon"></i>
                <div class="tc-fp-info">
                    <span class="tc-fp-label">{"Image ID - verified on-chain"}</span>
                    <code class="tc-fp-value">{short_hash(image_id, 20)}</code>
                </div>
                if approved {
                    <span class="tc-fp-status">{"Approved "}<i class="fa-solid fa-check"></i></span>
                }
            </div>

            <div class="tc-fp-source">
                <a href={format!("https://arbiscan.io/address/{}#readContract", contract_addr)} target="_blank" rel="noopener noreferrer">
                    {"Check approvedImages on Arbiscan "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                </a>
            </div>

            <button class="tc-extra-toggle" onclick={toggle}>
                {if show_extra { "Hide additional info" } else { "Show additional info" }}
            </button>

            if show_extra {
                <div class="tc-extra-content">
                    <div class="tc-extra-card">
                        <span class="tc-extra-title">{"Marlin KMS"}</span>
                        <p>{"An independent key guardian running in its own sealed computer. Verifies enclave attestation and checks the Arbitrum blockchain before releasing the encryption key."}</p>
                        <a href="https://github.com/marlinprotocol/oyster-monorepo" target="_blank" rel="noopener noreferrer">
                            {"Marlin source code "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                        </a>
                    </div>
                    <div class="tc-extra-card">
                        <span class="tc-extra-title">{"Arbitrum Contract"}</span>
                        <code>{short_hash(contract_addr, 20)}</code>
                        <div class="tc-extra-links">
                            <a href={format!("https://arbiscan.io/address/{}#events", contract_addr)} target="_blank" rel="noopener noreferrer">
                                {"View on Arbiscan "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                            </a>
                        </div>
                    </div>
                </div>
            }
        </>
    }
}

fn render_pillar_auditable(d: &TrustChainData, show_extra: bool, toggle: Callback<MouseEvent>) -> Html {
    let image_id = d.image_id.as_deref().unwrap_or("unavailable");
    let commit = d.commit_sha.as_deref().unwrap_or("unknown");

    html! {
        <>
            <h3 class="tc-detail-title">{"Code is auditable"}</h3>
            <p class="tc-detail-desc">
                {"All code processing your data is open source. GitHub Actions builds it deterministically and publishes this Image ID as the build fingerprint."}
            </p>

            <div class="tc-fingerprint">
                <i class="fa-solid fa-fingerprint tc-fp-icon"></i>
                <div class="tc-fp-info">
                    <span class="tc-fp-label">{"Image ID - from build pipeline"}</span>
                    <code class="tc-fp-value">{short_hash(image_id, 20)}</code>
                </div>
                <span class="tc-fp-status">{"Verified "}<i class="fa-solid fa-check"></i></span>
            </div>

            <div class="tc-fp-source">
                if let Some(ref run_id) = d.workflow_run_id {
                    <a href={format!("https://github.com/ahtavarasmus/lightfriend/actions/runs/{}", run_id)} target="_blank" rel="noopener noreferrer">
                        {"View build that produced this fingerprint "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                    </a>
                }
            </div>

            <button class="tc-extra-toggle" onclick={toggle}>
                {if show_extra { "Hide additional info" } else { "Show additional info" }}
            </button>

            if show_extra {
                <div class="tc-extra-content">
                    <div class="tc-extra-card">
                        <span class="tc-extra-title">{"Source Commit"}</span>
                        <p>{"The build was produced from this exact commit. Anyone can audit the code and reproduce the build."}</p>
                        <code>{short_hash(commit, 12)}</code>
                        <div class="tc-extra-links">
                            <a href={format!("https://github.com/ahtavarasmus/lightfriend/commit/{}", commit)} target="_blank" rel="noopener noreferrer">
                                {"View commit on GitHub "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                            </a>
                        </div>
                    </div>
                    if let Some(ref url) = d.build_metadata_url {
                        <div class="tc-extra-card">
                            <span class="tc-extra-title">{"Published Metadata"}</span>
                            <div class="tc-extra-links">
                                <a href={url.clone()} target="_blank" rel="noopener noreferrer">
                                    {"View build metadata "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                                </a>
                            </div>
                        </div>
                    }
                </div>
            }
        </>
    }
}

fn render_pillar_isolated(d: &TrustChainData, show_extra: bool, toggle: Callback<MouseEvent>) -> Html {
    let image_id = d.image_id.as_deref().unwrap_or("unavailable");
    let pcr0 = d.pcr0.as_deref().unwrap_or("unavailable");
    let pcr1 = d.pcr1.as_deref().unwrap_or("unavailable");
    let pcr2 = d.pcr2.as_deref().unwrap_or("unavailable");
    let has_attestation = d.attestation.as_ref().map_or(false, |a| a.available);
    let commit = d.commit_sha.as_deref().unwrap_or("unknown");

    html! {
        <>
            <h3 class="tc-detail-title">{"Runtime is isolated"}</h3>
            <p class="tc-detail-desc">
                {"The secure hardware enclave that processes your data has been attested by AWS. The live attestation confirms this exact Image ID is running."}
            </p>

            <div class="tc-fingerprint">
                <i class="fa-solid fa-fingerprint tc-fp-icon"></i>
                <div class="tc-fp-info">
                    <span class="tc-fp-label">{"Image ID - from live attestation"}</span>
                    <code class="tc-fp-value">{short_hash(image_id, 20)}</code>
                </div>
                if has_attestation {
                    <span class="tc-fp-status">{"Attested "}<i class="fa-solid fa-check"></i></span>
                }
            </div>

            <div class="tc-fp-source">
                <a href="/.well-known/lightfriend/attestation" target="_blank" rel="noopener noreferrer">
                    {"View live attestation endpoint "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                </a>
            </div>

            <button class="tc-extra-toggle" onclick={toggle}>
                {if show_extra { "Hide additional info" } else { "Show additional info" }}
            </button>

            if show_extra {
                <div class="tc-extra-content">
                    <div class="tc-extra-card">
                        <span class="tc-extra-title">{"Hardware Attestation"}</span>
                        <p>{"The enclave produces a cryptographic attestation document signed by AWS, certifying the hardware environment and the exact code running inside."}</p>
                        <div class="tc-extra-links">
                            <a href={format!("https://github.com/ahtavarasmus/lightfriend/tree/{}/tools/attestation-verifier", commit)} target="_blank" rel="noopener noreferrer">
                                {"Verification tool source "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                            </a>
                        </div>
                    </div>
                    <div class="tc-extra-card">
                        <span class="tc-extra-title">{"Platform Configuration Registers"}</span>
                        <p>{"Low-level enclave measurements. PCR0 is derived from the Image ID."}</p>
                        <div class="tc-pcr-list">
                            <div class="tc-pcr-row">
                                <span>{"PCR0"}</span>
                                <code>{short_hash(pcr0, 16)}</code>
                            </div>
                            <div class="tc-pcr-row">
                                <span>{"PCR1"}</span>
                                <code>{short_hash(pcr1, 16)}</code>
                            </div>
                            <div class="tc-pcr-row">
                                <span>{"PCR2"}</span>
                                <code>{short_hash(pcr2, 16)}</code>
                            </div>
                        </div>
                    </div>
                </div>
            }
        </>
    }
}

// =============================================
// HISTORY CHAIN
// =============================================

fn render_history_chain(d: &TrustChainData) -> Html {
    let mut builds: Vec<&HistoricalBuild> = d.history.iter().collect();
    if builds.is_empty() {
        return html! {};
    }

    // Sort by propose_timestamp ascending (oldest first)
    builds.sort_by(|a, b| {
        a.propose_timestamp.as_deref().unwrap_or("")
            .cmp(&b.propose_timestamp.as_deref().unwrap_or(""))
    });

    let contract_addr = d.kms_contract_address.as_deref()
        .unwrap_or("0x2e51F48F7440b415D9De30b4D73a18C8E9428982");

    html! {
        <div class="tc-history">
            <h2>{"On-chain build history"}</h2>
            <p class="tc-history-desc">
                {"Every version of Lightfriend is permanently recorded on the Arbitrum blockchain. Each entry links to its source code and on-chain proof."}
            </p>

            <div class="tc-history-chain">
                {for builds.iter().enumerate().map(|(i, build)| {
                    let commit_short = short_hash(&build.commit_hash, 7);
                    let image_id_short = short_hash(&build.image_id, 12);
                    let is_last = i == builds.len() - 1;
                    let is_current = build.is_current;
                    let commit_url = format!("https://github.com/ahtavarasmus/lightfriend/commit/{}", build.commit_hash);

                    html! {
                        <div class="tc-history-entry">
                            <div class={classes!("tc-history-node", is_current.then(|| "current"))}>
                                <div class="tc-history-dot">
                                    if is_current {
                                        <i class="fa-solid fa-circle-check"></i>
                                    } else {
                                        <i class="fa-solid fa-circle"></i>
                                    }
                                </div>

                                <div class="tc-history-card">
                                    if is_current {
                                        <span class="tc-history-badge">{"running"}</span>
                                    }

                                    <div class="tc-history-fp">
                                        <i class="fa-solid fa-fingerprint"></i>
                                        <code>{&image_id_short}</code>
                                    </div>

                                    <div class="tc-history-links">
                                        <a href={commit_url} target="_blank" rel="noopener noreferrer" title="View source code">
                                            <i class="fa-solid fa-code-commit"></i>
                                            {format!(" {}", commit_short)}
                                        </a>
                                        if !build.propose_tx.is_empty() {
                                            <a href={format!("https://arbiscan.io/tx/{}", build.propose_tx)} target="_blank" rel="noopener noreferrer" title="View on-chain proof">
                                                <i class="fa-solid fa-link"></i>
                                                {" arbiscan"}
                                            </a>
                                        }
                                    </div>

                                    if let Some(ref ts) = build.propose_timestamp {
                                        <span class="tc-history-date">{format_date(ts)}</span>
                                    }
                                </div>
                            </div>
                            if !is_last {
                                <div class="tc-history-connector">
                                    <i class="fa-solid fa-arrow-right"></i>
                                </div>
                            }
                        </div>
                    }
                })}
            </div>

            <div class="tc-history-footer">
                <a href={format!("https://arbiscan.io/address/{}#events", contract_addr)} target="_blank" rel="noopener noreferrer">
                    {"View all events on Arbiscan "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                </a>
            </div>
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
            <h2>{"Verify it yourself"}</h2>

            <div class="tc-verify-block">
                <h3>{"Run the verification tool"}</h3>
                <p class="tc-verify-desc">
                    {"Check Amazon's signature, compare fingerprints, and query the blockchain - all on your own machine."}
                </p>
                <pre class="tc-verify-cmd">{format!("git clone https://github.com/ahtavarasmus/lightfriend\ncd lightfriend\ncargo run --manifest-path tools/attestation-verifier/Cargo.toml -- \\\n  https://lightfriend.ai --rpc-url https://arb1.arbitrum.io/rpc")}</pre>
                <a href={format!("https://github.com/ahtavarasmus/lightfriend/tree/{}/tools/attestation-verifier", commit)} target="_blank" rel="noopener noreferrer" class="tc-verify-link">
                    {"Read the tool's source code "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                </a>
            </div>

            <div class="tc-verify-block">
                <h3>{"Audit the code with AI"}</h3>
                <p class="tc-verify-desc">
                    {"Pull the codebase and use an AI assistant to review the security. Ask it anything - how data is encrypted, what the enclave can access, where credentials are stored."}
                </p>
                <pre class="tc-verify-cmd">{"git clone https://github.com/ahtavarasmus/lightfriend\ncd lightfriend\nclaude"}</pre>
                <p class="tc-verify-hint">
                    {"Works with "}<a href="https://claude.ai/download" target="_blank" rel="noopener noreferrer">{"Claude Code"}</a>
                    {", Cursor, or any AI coding tool."}
                </p>
            </div>
        </div>
    }
}

// =============================================
// STYLES
// =============================================

const STYLES: &str = r#"
.tc-page {
    max-width: 700px;
    margin: 0 auto;
    padding: 5rem 1.5rem 3rem;
    color: #e0e0e0;
}

/* Header */
.tc-header { text-align: center; margin-bottom: 1.5rem; }
.tc-header h1 { font-size: 1.6rem; font-weight: 600; margin: 0; color: #f0f0f0; }

/* Intro box */
.tc-intro-box {
    border: 1px solid rgba(76, 175, 80, 0.2);
    border-radius: 12px;
    padding: 1.25rem 1.5rem;
    margin-bottom: 2rem;
    background: rgba(76, 175, 80, 0.03);
}
.tc-intro-text {
    color: rgba(76, 175, 80, 0.85);
    font-size: 0.9rem;
    line-height: 1.6;
    margin: 0 0 0.5rem;
}
.tc-intro-hw {
    color: rgba(255,255,255,0.6);
    font-size: 0.85rem;
    margin: 0 0 0.4rem;
}
.tc-intro-hw strong { color: rgba(255,255,255,0.8); }
.tc-intro-link {
    color: rgba(76, 175, 80, 0.7);
    font-size: 0.8rem;
    text-decoration: none;
}
.tc-intro-link:hover { text-decoration: underline; }

.tc-loading { text-align: center; padding: 3rem; color: rgba(255,255,255,0.3); }

/* ---- Diagram ---- */
.tc-diagram { margin-bottom: 2rem; }

.tc-proof-boundary {
    position: relative;
    border: 2px dashed rgba(255,255,255,0.12);
    border-radius: 16px;
    padding: 2.5rem 1.5rem 1.5rem;
}
.tc-proof-badge {
    position: absolute;
    top: -0.65rem;
    left: 50%;
    transform: translateX(-50%);
    background: #1a1a1a;
    padding: 0 0.75rem;
    font-size: 0.85rem;
    font-weight: 500;
    color: rgba(76, 175, 80, 0.8);
    white-space: nowrap;
}
.tc-proof-badge i { font-size: 0.75rem; }

.tc-diagram-layout {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 1.5rem;
    align-items: start;
}

.tc-node {
    border: 1px solid rgba(255,255,255,0.1);
    border-radius: 10px;
    padding: 1rem 1.25rem;
    background: rgba(255,255,255,0.025);
}
.tc-node-title {
    font-size: 0.95rem;
    font-weight: 500;
    color: #e0e0e0;
    margin-bottom: 0.4rem;
}
.tc-node-title i { font-size: 0.8rem; color: rgba(255,255,255,0.35); }
.tc-node-points {
    list-style: disc;
    padding-left: 1.2rem;
    margin: 0;
    color: rgba(255,255,255,0.45);
    font-size: 0.82rem;
    line-height: 1.6;
}
.tc-node-desc {
    color: rgba(255,255,255,0.45);
    font-size: 0.82rem;
    line-height: 1.5;
    margin: 0;
}

.tc-server-node { grid-column: 2; }
.tc-enclave-node {
    margin-top: 0.75rem;
    border-color: rgba(76, 175, 80, 0.15);
    background: rgba(76, 175, 80, 0.02);
}
.tc-enclave-node .tc-node-title i { color: rgba(76, 175, 80, 0.5); }

.tc-kms-node {
    grid-column: 1;
    grid-row: 1;
    align-self: end;
}
.tc-kms-node .tc-node-title i { color: rgba(255, 215, 0, 0.5); }

/* ---- Pillars ---- */
.tc-pillars { margin-bottom: 2.5rem; }

.tc-pillar-boxes {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 0.75rem;
    margin-bottom: 1rem;
}
.tc-pillar-box {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.4rem;
    padding: 1rem 0.5rem;
    border: 1px solid rgba(255,255,255,0.1);
    border-radius: 10px;
    background: rgba(255,255,255,0.025);
    cursor: pointer;
    transition: border-color 0.2s, background 0.2s;
    color: #e0e0e0;
    font-family: inherit;
}
.tc-pillar-box:hover {
    border-color: rgba(255,255,255,0.18);
    background: rgba(255,255,255,0.04);
}
.tc-pillar-box.selected {
    border-color: rgba(76, 175, 80, 0.3);
    background: rgba(76, 175, 80, 0.03);
}

.tc-check {
    position: absolute;
    top: -0.4rem;
    right: -0.4rem;
    color: #4CAF50;
    font-size: 0.85rem;
    background: #1a1a1a;
    border-radius: 50%;
}
.tc-pillar-label {
    font-size: 0.82rem;
    text-align: center;
    line-height: 1.3;
    color: rgba(255,255,255,0.7);
}
.tc-pillar-icon {
    font-size: 1rem;
    color: rgba(255,255,255,0.2);
}

/* Pillar detail card */
.tc-pillar-detail {
    border: 1px solid rgba(255,255,255,0.1);
    border-radius: 12px;
    padding: 1.5rem;
    background: rgba(255,255,255,0.02);
}
.tc-detail-title {
    font-size: 1.1rem;
    font-weight: 600;
    margin: 0 0 0.5rem;
    color: #e0e0e0;
}
.tc-detail-desc {
    color: rgba(255,255,255,0.5);
    font-size: 0.85rem;
    line-height: 1.6;
    margin: 0 0 1rem;
}

/* Fingerprint card */
.tc-fingerprint {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    border: 1px solid rgba(255,255,255,0.08);
    border-radius: 10px;
    padding: 0.75rem 1rem;
    background: rgba(255,255,255,0.02);
    margin-bottom: 1rem;
}
.tc-fp-icon { font-size: 1.2rem; color: rgba(255,255,255,0.2); }
.tc-fp-info { display: flex; flex-direction: column; gap: 0.1rem; flex: 1; }
.tc-fp-label { font-size: 0.72rem; color: rgba(255,255,255,0.35); }
.tc-fp-value {
    font-family: monospace; font-size: 0.78rem;
    color: rgba(255,255,255,0.6);
}
.tc-fp-status {
    font-size: 0.78rem;
    color: rgba(76, 175, 80, 0.8);
    white-space: nowrap;
}
.tc-fp-status i { font-size: 0.7rem; }

/* Source link under fingerprint */
.tc-fp-source {
    margin-bottom: 0.75rem;
}
.tc-fp-source a {
    font-size: 0.78rem;
    color: rgba(76, 175, 80, 0.7);
    text-decoration: none;
}
.tc-fp-source a:hover { text-decoration: underline; }

/* Extra toggle button */
.tc-extra-toggle {
    display: block;
    width: 100%;
    padding: 0.6rem;
    border: 1px solid rgba(255,255,255,0.08);
    border-radius: 8px;
    background: rgba(255,255,255,0.02);
    color: rgba(255,255,255,0.5);
    font-size: 0.82rem;
    cursor: pointer;
    font-family: inherit;
    transition: background 0.2s;
}
.tc-extra-toggle:hover { background: rgba(255,255,255,0.04); }

/* Extra content */
.tc-extra-content {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    margin-top: 0.75rem;
}
.tc-extra-card {
    border: 1px solid rgba(255,255,255,0.06);
    border-radius: 8px;
    padding: 0.75rem 1rem;
    background: rgba(255,255,255,0.015);
}
.tc-extra-title {
    display: block;
    font-size: 0.78rem;
    font-weight: 500;
    color: rgba(76, 175, 80, 0.7);
    margin-bottom: 0.3rem;
}
.tc-extra-card p {
    font-size: 0.8rem;
    color: rgba(255,255,255,0.4);
    line-height: 1.5;
    margin: 0 0 0.4rem;
}
.tc-extra-card code {
    font-family: monospace;
    font-size: 0.72rem;
    color: rgba(255,255,255,0.55);
    display: block;
    margin-bottom: 0.3rem;
    word-break: break-all;
}
.tc-extra-links {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
}
.tc-extra-card a, .tc-extra-links a {
    font-size: 0.78rem;
    color: rgba(76, 175, 80, 0.7);
    text-decoration: none;
}
.tc-extra-card a:hover, .tc-extra-links a:hover {
    text-decoration: underline;
}

/* PCR values */
.tc-pcr-list {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    margin-top: 0.3rem;
}
.tc-pcr-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.75rem;
}
.tc-pcr-row span { color: rgba(255,255,255,0.3); min-width: 35px; }
.tc-pcr-row code {
    font-family: monospace;
    font-size: 0.7rem;
    color: rgba(255,255,255,0.55);
    margin: 0;
}

/* ---- History chain ---- */
.tc-history {
    margin-bottom: 2rem;
    padding-top: 1.5rem;
    border-top: 1px solid rgba(255,255,255,0.06);
}
.tc-history h2 { font-size: 1rem; margin: 0 0 0.3rem; color: #e0e0e0; font-weight: 500; }
.tc-history-desc {
    color: rgba(255,255,255,0.4); font-size: 0.82rem; line-height: 1.5; margin: 0 0 1rem;
}

.tc-history-chain {
    display: flex;
    align-items: flex-start;
    overflow-x: auto;
    padding-bottom: 0.5rem;
    gap: 0;
}
.tc-history-entry {
    display: flex;
    align-items: center;
    flex-shrink: 0;
}
.tc-history-node {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.35rem;
    min-width: 120px;
}
.tc-history-dot {
    font-size: 0.65rem;
    color: rgba(255,255,255,0.2);
}
.tc-history-node.current .tc-history-dot {
    color: #4CAF50;
    font-size: 0.75rem;
}

.tc-history-card {
    border: 1px solid rgba(255,255,255,0.08);
    border-radius: 8px;
    padding: 0.6rem 0.75rem;
    background: rgba(255,255,255,0.02);
    text-align: center;
    min-width: 110px;
}
.tc-history-node.current .tc-history-card {
    border-color: rgba(76, 175, 80, 0.25);
    background: rgba(76, 175, 80, 0.03);
}

.tc-history-badge {
    display: inline-block;
    font-size: 0.55rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: #4CAF50;
    margin-bottom: 0.25rem;
}

.tc-history-fp {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.3rem;
    margin-bottom: 0.3rem;
}
.tc-history-fp i { font-size: 0.6rem; color: rgba(255,255,255,0.2); }
.tc-history-fp code {
    font-family: monospace;
    font-size: 0.65rem;
    color: rgba(255,255,255,0.55);
}

.tc-history-links {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    margin-bottom: 0.2rem;
}
.tc-history-links a {
    font-size: 0.68rem;
    color: rgba(76, 175, 80, 0.7);
    text-decoration: none;
}
.tc-history-links a:hover { text-decoration: underline; }

.tc-history-date {
    font-size: 0.6rem;
    color: rgba(255,255,255,0.25);
}

.tc-history-connector {
    color: rgba(255,255,255,0.12);
    font-size: 0.55rem;
    padding: 0 0.25rem;
    margin-top: 0.5rem;
}

.tc-history-footer {
    margin-top: 0.75rem;
}
.tc-history-footer a {
    font-size: 0.78rem;
    color: rgba(76, 175, 80, 0.7);
    text-decoration: none;
}
.tc-history-footer a:hover { text-decoration: underline; }

/* ---- Verify section ---- */
.tc-verify {
    margin-top: 2rem;
    padding-top: 1.5rem;
    border-top: 1px solid rgba(255,255,255,0.06);
}
.tc-verify h2 { font-size: 1rem; margin: 0 0 1rem; color: #e0e0e0; font-weight: 500; }
.tc-verify-block { margin-bottom: 1.25rem; }
.tc-verify-block h3 { font-size: 0.88rem; margin: 0 0 0.3rem; color: rgba(255,255,255,0.75); font-weight: 500; }
.tc-verify-desc { color: rgba(255,255,255,0.4); font-size: 0.82rem; line-height: 1.5; margin: 0 0 0.75rem; }
.tc-verify-hint { font-size: 0.75rem; color: rgba(255,255,255,0.3); margin: 0.4rem 0 0; }
.tc-verify-hint a { color: rgba(76, 175, 80, 0.7); text-decoration: none; }
.tc-verify-hint a:hover { text-decoration: underline; }
.tc-verify-cmd {
    background: rgba(0,0,0,0.2); border: 1px solid rgba(255,255,255,0.06);
    padding: 0.75rem 1rem; border-radius: 8px; font-size: 0.72rem;
    color: rgba(255,255,255,0.55); overflow-x: auto; white-space: pre;
    font-family: monospace; margin: 0 0 0.5rem;
}
.tc-verify-link { font-size: 0.75rem; color: rgba(76, 175, 80, 0.7); text-decoration: none; }
.tc-verify-link:hover { text-decoration: underline; }

/* ---- Footer ---- */
.tc-footer-links { margin-top: 2.5rem; text-align: center; }
.tc-footer-links a {
    color: rgba(76, 175, 80, 0.7); text-decoration: none; font-size: 0.85rem;
}
.tc-footer-links a:hover { text-decoration: underline; }
.tc-page .legal-links {
    margin-top: 1.5rem; text-align: center; font-size: 0.75rem; color: rgba(255,255,255,0.25);
}
.tc-page .legal-links a { color: rgba(255,255,255,0.35); text-decoration: none; }
.tc-page .legal-links a:hover { color: rgba(76, 175, 80, 0.7); }

@media (max-width: 640px) {
    .tc-page { padding: 5rem 1rem 2rem; }
    .tc-header h1 { font-size: 1.3rem; }
    .tc-diagram-layout { grid-template-columns: 1fr; }
    .tc-kms-node { grid-column: 1; grid-row: auto; }
    .tc-server-node { grid-column: 1; }
    .tc-pillar-boxes { gap: 0.5rem; }
    .tc-pillar-label { font-size: 0.75rem; }
    .tc-fingerprint { flex-direction: column; align-items: flex-start; text-align: left; }
    .tc-fp-status { align-self: flex-end; }
}
"#;
