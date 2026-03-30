use crate::config::get_backend_url;
use crate::utils::seo::{use_seo, SeoMeta};
use crate::Route;
use gloo_net::http::Request;
use serde::Deserialize;
use wasm_bindgen::prelude::*;
use yew::prelude::*;
use yew_router::prelude::*;

// -- Data types matching backend response --

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
    history: Vec<HistoricalBuild>,
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
        format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
    } else if days < 30 {
        format!("{} day{} ago", days, if days == 1 { "" } else { "s" })
    } else {
        let months = days / 30;
        format!("{} month{} ago", months, if months == 1 { "" } else { "s" })
    }
}

fn format_timestamp(iso: &str) -> String {
    let date = js_sys::Date::new(&JsValue::from_str(iso));
    let year = date.get_utc_full_year();
    let month = date.get_utc_month() + 1;
    let day = date.get_utc_date();
    let hour = date.get_utc_hours();
    let min = date.get_utc_minutes();
    format!(
        "{}-{:02}-{:02} {:02}:{:02} UTC",
        year, month, day, hour, min
    )
}

// -- Components --

#[derive(Properties, PartialEq)]
struct ChainStepProps {
    pub step_num: u32,
    pub icon: &'static str,
    pub title: &'static str,
    pub simple: String,
    pub status: StepStatus,
    pub timestamp: Option<String>,
    pub children: Children,
    #[prop_or_default]
    pub is_last: bool,
}

#[derive(Clone, PartialEq)]
enum StepStatus {
    Verified,
    Unverified,
    Loading,
    Info,
}

#[function_component(ChainStep)]
fn chain_step(props: &ChainStepProps) -> Html {
    let expanded = use_state(|| false);
    let toggle = {
        let expanded = expanded.clone();
        Callback::from(move |_: MouseEvent| {
            expanded.set(!*expanded);
        })
    };

    let status_class = match props.status {
        StepStatus::Verified => "step-verified",
        StepStatus::Unverified => "step-unverified",
        StepStatus::Loading => "step-loading",
        StepStatus::Info => "step-info",
    };

    let status_icon = match props.status {
        StepStatus::Verified => html! { <i class="fa-solid fa-circle-check"></i> },
        StepStatus::Unverified => html! { <i class="fa-solid fa-circle-question"></i> },
        StepStatus::Loading => html! { <i class="fa-solid fa-spinner fa-spin"></i> },
        StepStatus::Info => html! { <i class="fa-solid fa-circle-info"></i> },
    };

    let ts_html = if let Some(ref ts) = props.timestamp {
        html! {
            <span class="step-timestamp" title={format_timestamp(ts)}>
                {relative_time(ts)}
            </span>
        }
    } else {
        html! {}
    };

    html! {
        <div class={classes!("chain-step", if props.is_last { "chain-step-last" } else { "" })}>
            <div class="step-connector">
                <div class={classes!("step-node", status_class)}>
                    {status_icon}
                </div>
                if !props.is_last {
                    <div class="step-line"></div>
                }
            </div>
            <div class="step-content">
                <div class="step-header" onclick={toggle.clone()}>
                    <div class="step-title-row">
                        <span class="step-number">{format!("{}", props.step_num)}</span>
                        <h3 class="step-title">{props.title}</h3>
                        {ts_html}
                    </div>
                    <p class="step-simple">{&props.simple}</p>
                    <button class="step-toggle" onclick={toggle}>
                        if *expanded {
                            <i class="fa-solid fa-chevron-up"></i>
                            {" Less details"}
                        } else {
                            <i class="fa-solid fa-chevron-down"></i>
                            {" More details"}
                        }
                    </button>
                </div>
                if *expanded {
                    <div class="step-details">
                        {for props.children.iter()}
                    </div>
                }
            </div>
        </div>
    }
}

// -- Main page --

#[function_component(TrustChainPage)]
pub fn trust_chain_page() -> Html {
    use_seo(SeoMeta {
        title: "Trust Chain - Lightfriend",
        description: "Live verification chain showing exactly what code is running and how you can verify it.",
        canonical: "https://lightfriend.ai/trust-chain",
        og_type: "website",
    });

    let data = use_state(|| None::<TrustChainData>);
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);

    // Scroll to top on mount
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

    // Fetch trust chain data
    {
        let data = data.clone();
        let loading = loading.clone();
        let error = error.clone();

        use_effect_with_deps(
            move |_| {
                wasm_bindgen_futures::spawn_local(async move {
                    let url = format!("{}/api/trust-chain", get_backend_url());
                    match Request::get(&url).send().await {
                        Ok(resp) => match resp.json::<TrustChainData>().await {
                            Ok(chain_data) => {
                                data.set(Some(chain_data));
                                loading.set(false);
                            }
                            Err(e) => {
                                error.set(Some(format!("Failed to parse data: {}", e)));
                                loading.set(false);
                            }
                        },
                        Err(e) => {
                            error.set(Some(format!("Failed to fetch: {}", e)));
                            loading.set(false);
                        }
                    }
                });
                || ()
            },
            (),
        );
    }

    let content = if *loading {
        html! {
            <div class="chain-loading">
                <i class="fa-solid fa-spinner fa-spin"></i>
                {" Loading trust chain data..."}
            </div>
        }
    } else if let Some(ref err) = *error {
        html! {
            <div class="chain-error">
                <p>{"Could not load live data. The chain structure is shown below with placeholder values."}</p>
                <p class="chain-error-detail">{err}</p>
            </div>
        }
    } else {
        html! {}
    };

    let d = (*data).clone().unwrap_or(TrustChainData {
        commit_sha: None,
        workflow_run_id: None,
        image_ref: None,
        eif_sha256: None,
        pcr0: None,
        pcr1: None,
        pcr2: None,
        image_id: None,
        kms_contract_address: None,
        built_at: None,
        build_metadata_url: None,
        blockchain: None,
        history: vec![],
    });

    let has_data = d.commit_sha.is_some();
    let has_blockchain = d.blockchain.is_some();
    let blockchain_approved = d.blockchain.as_ref().map_or(false, |b| b.approved);

    // GitHub URLs
    let commit_url = d.commit_sha.as_ref().map(|sha| {
        format!("https://github.com/ahtavarasmus/lightfriend/commit/{}", sha)
    });
    let actions_url = d.workflow_run_id.as_ref().map(|id| {
        format!(
            "https://github.com/ahtavarasmus/lightfriend/actions/runs/{}",
            id
        )
    });
    let contract_url = d.kms_contract_address.as_ref().map(|addr| {
        format!("https://arbiscan.io/address/{}", addr)
    });

    html! {
        <>
        <style>{STYLES}</style>
        <div class="trust-chain-page">
            <div class="trust-chain-header">
                <h1>{"Trust Chain"}</h1>
                <p class="trust-chain-subtitle">
                    {"Live verification of what's running right now. Each link in this chain is independently verifiable."}
                </p>
                if has_data {
                    <div class="chain-status-banner chain-status-live">
                        <i class="fa-solid fa-signal"></i>
                        {" Connected to live instance"}
                    </div>
                }
            </div>

            {content}

            <div class="chain-steps">
                // Step 1: Source Code
                <ChainStep
                    step_num={1}
                    icon="fa-code"
                    title="Source Code"
                    simple={format!("All of Lightfriend's code is public on GitHub. Anyone can read every single line.")}
                    status={if has_data { StepStatus::Verified } else { StepStatus::Loading }}
                    timestamp={None::<String>}
                >
                    <div class="detail-grid">
                        if let Some(ref sha) = d.commit_sha {
                            <div class="detail-row">
                                <span class="detail-label">{"Commit"}</span>
                                <code class="detail-value">{short_hash(sha, 12)}</code>
                            </div>
                        }
                        if let Some(ref url) = commit_url {
                            <a href={url.clone()} target="_blank" rel="noopener noreferrer" class="detail-link">
                                <i class="fa-solid fa-arrow-up-right-from-square"></i>
                                {" View source code on GitHub"}
                            </a>
                        }
                        <a href="https://github.com/ahtavarasmus/lightfriend" target="_blank" rel="noopener noreferrer" class="detail-link">
                            <i class="fa-solid fa-arrow-up-right-from-square"></i>
                            {" Browse full repository"}
                        </a>
                    </div>
                </ChainStep>

                // Step 2: Automated Build
                <ChainStep
                    step_num={2}
                    icon="fa-gears"
                    title="Automated Build"
                    simple={format!("An automated system (GitHub Actions) built the app from the source code. No human could tamper with the build.")}
                    status={if d.workflow_run_id.is_some() { StepStatus::Verified } else if has_data { StepStatus::Unverified } else { StepStatus::Loading }}
                    timestamp={d.built_at.clone()}
                >
                    <div class="detail-grid">
                        if let Some(ref run_id) = d.workflow_run_id {
                            <div class="detail-row">
                                <span class="detail-label">{"Workflow Run"}</span>
                                <code class="detail-value">{format!("#{}", run_id)}</code>
                            </div>
                        }
                        if let Some(ref img) = d.image_ref {
                            <div class="detail-row">
                                <span class="detail-label">{"Docker Image"}</span>
                                <code class="detail-value detail-value-wrap">{short_hash(img, 40)}</code>
                            </div>
                        }
                        if let Some(ref ts) = d.built_at {
                            <div class="detail-row">
                                <span class="detail-label">{"Built At"}</span>
                                <span class="detail-value">{format_timestamp(ts)}</span>
                            </div>
                        }
                        if let Some(ref url) = actions_url {
                            <a href={url.clone()} target="_blank" rel="noopener noreferrer" class="detail-link">
                                <i class="fa-solid fa-arrow-up-right-from-square"></i>
                                {" View build logs on GitHub Actions"}
                            </a>
                        }
                    </div>
                </ChainStep>

                // Step 3: Fingerprint Published
                <ChainStep
                    step_num={3}
                    icon="fa-fingerprint"
                    title="Fingerprint Published"
                    simple={format!("The build's unique fingerprint (PCR values) was published publicly so anyone can check it later.")}
                    status={if d.pcr0.is_some() { StepStatus::Verified } else if has_data { StepStatus::Unverified } else { StepStatus::Loading }}
                    timestamp={d.built_at.clone()}
                >
                    <div class="detail-grid">
                        <p class="detail-explainer">
                            {"PCR values are like a DNA fingerprint for the code. If even one tiny thing changes, the fingerprint is completely different."}
                        </p>
                        if let Some(ref pcr) = d.pcr0 {
                            <div class="detail-row">
                                <span class="detail-label">{"PCR0 (code)"}</span>
                                <code class="detail-value detail-value-mono">{short_hash(pcr, 24)}</code>
                            </div>
                        }
                        if let Some(ref pcr) = d.pcr1 {
                            <div class="detail-row">
                                <span class="detail-label">{"PCR1 (kernel)"}</span>
                                <code class="detail-value detail-value-mono">{short_hash(pcr, 24)}</code>
                            </div>
                        }
                        if let Some(ref pcr) = d.pcr2 {
                            <div class="detail-row">
                                <span class="detail-label">{"PCR2 (config)"}</span>
                                <code class="detail-value detail-value-mono">{short_hash(pcr, 24)}</code>
                            </div>
                        }
                        if let Some(ref sha) = d.eif_sha256 {
                            <div class="detail-row">
                                <span class="detail-label">{"EIF SHA256"}</span>
                                <code class="detail-value detail-value-mono">{short_hash(sha, 24)}</code>
                            </div>
                        }
                        if let Some(ref url) = d.build_metadata_url {
                            <a href={url.clone()} target="_blank" rel="noopener noreferrer" class="detail-link">
                                <i class="fa-solid fa-arrow-up-right-from-square"></i>
                                {" View raw build metadata (JSON)"}
                            </a>
                        }
                    </div>
                </ChainStep>

                // Step 4: Blockchain Approval
                <ChainStep
                    step_num={4}
                    icon="fa-link"
                    title="Blockchain Approval"
                    simple={format!("A public smart contract on Arbitrum (a blockchain) recorded this build as approved. Anyone can check this record.")}
                    status={if blockchain_approved { StepStatus::Verified } else if has_blockchain { StepStatus::Unverified } else if has_data { StepStatus::Info } else { StepStatus::Loading }}
                    timestamp={d.blockchain.as_ref().and_then(|b| b.activate_timestamp.clone())}
                >
                    <div class="detail-grid">
                        if let Some(ref img_id) = d.image_id {
                            <div class="detail-row">
                                <span class="detail-label">{"Image ID"}</span>
                                <code class="detail-value detail-value-mono">{short_hash(img_id, 16)}</code>
                            </div>
                        }
                        if let Some(ref addr) = d.kms_contract_address {
                            <div class="detail-row">
                                <span class="detail-label">{"Contract"}</span>
                                <code class="detail-value detail-value-mono">{short_hash(addr, 12)}</code>
                            </div>
                        }
                        if let Some(ref bc) = d.blockchain {
                            <div class="detail-row">
                                <span class="detail-label">{"Status"}</span>
                                <span class={classes!("detail-value", if bc.approved { "detail-approved" } else { "detail-pending" })}>
                                    {if bc.approved { "Approved" } else { "Not approved" }}
                                </span>
                            </div>
                            if let Some(ref tx) = bc.propose_tx {
                                <div class="detail-row">
                                    <span class="detail-label">{"Propose Tx"}</span>
                                    <a href={format!("https://arbiscan.io/tx/{}", tx)} target="_blank" rel="noopener noreferrer" class="detail-value detail-value-mono detail-link-inline">
                                        {short_hash(tx, 12)}
                                        {" "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                                    </a>
                                </div>
                            }
                            if let Some(ref tx) = bc.activate_tx {
                                <div class="detail-row">
                                    <span class="detail-label">{"Activate Tx"}</span>
                                    <a href={format!("https://arbiscan.io/tx/{}", tx)} target="_blank" rel="noopener noreferrer" class="detail-value detail-value-mono detail-link-inline">
                                        {short_hash(tx, 12)}
                                        {" "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                                    </a>
                                </div>
                            }
                        }
                        if let Some(ref url) = contract_url {
                            <a href={url.clone()} target="_blank" rel="noopener noreferrer" class="detail-link">
                                <i class="fa-solid fa-arrow-up-right-from-square"></i>
                                {" View contract on Arbiscan"}
                            </a>
                        }
                    </div>
                </ChainStep>

                // Step 5: Sealed Computer Running
                <ChainStep
                    step_num={5}
                    icon="fa-lock"
                    title="Sealed Computer Running"
                    simple={format!("The code runs inside a sealed computer (AWS Nitro Enclave) that nobody can peek inside - not even us, not even Amazon.")}
                    status={if has_data { StepStatus::Verified } else { StepStatus::Loading }}
                    timestamp={None::<String>}
                >
                    <div class="detail-grid">
                        <p class="detail-explainer">
                            {"A Nitro Enclave is like a locked room with no doors or windows. You can put things in and get results out, but nobody can look inside or tamper with what's running."}
                        </p>
                        if let Some(ref img) = d.image_ref {
                            <div class="detail-row">
                                <span class="detail-label">{"Image"}</span>
                                <code class="detail-value detail-value-mono detail-value-wrap">{short_hash(img, 40)}</code>
                            </div>
                        }
                        <a href="https://aws.amazon.com/ec2/nitro/nitro-enclaves/" target="_blank" rel="noopener noreferrer" class="detail-link">
                            <i class="fa-solid fa-arrow-up-right-from-square"></i>
                            {" Learn about AWS Nitro Enclaves"}
                        </a>
                    </div>
                </ChainStep>

                // Step 6: Key Released
                <ChainStep
                    step_num={6}
                    icon="fa-key"
                    title="Encryption Key Released"
                    simple={format!("An independent key guardian (Marlin) verified the sealed computer is running approved code, then released the encryption key. Nobody else ever sees this key.")}
                    status={if blockchain_approved { StepStatus::Verified } else if has_data { StepStatus::Info } else { StepStatus::Loading }}
                    timestamp={None::<String>}
                >
                    <div class="detail-grid">
                        <p class="detail-explainer">
                            {"The key guardian only gives the encryption key to sealed computers that can prove two things: (1) they are real sealed computers (signed by Amazon) and (2) they are running approved code (checked against the blockchain)."}
                        </p>
                        if let Some(ref img_id) = d.image_id {
                            <div class="detail-row">
                                <span class="detail-label">{"Verified Image ID"}</span>
                                <code class="detail-value detail-value-mono">{short_hash(img_id, 16)}</code>
                            </div>
                        }
                        <a href="https://github.com/marlinprotocol/oyster-monorepo" target="_blank" rel="noopener noreferrer" class="detail-link">
                            <i class="fa-solid fa-arrow-up-right-from-square"></i>
                            {" Marlin's open-source key service code"}
                        </a>
                    </div>
                </ChainStep>

                // Step 7: Live Verification
                <ChainStep
                    step_num={7}
                    icon="fa-shield-halved"
                    title="Verify It Live"
                    simple={format!("You can verify all of this yourself, right now. The sealed computer can prove what code it's running.")}
                    status={if has_data { StepStatus::Info } else { StepStatus::Loading }}
                    timestamp={None::<String>}
                    is_last={true}
                >
                    <div class="detail-grid">
                        <p class="detail-explainer">
                            {"Anyone can ask the sealed computer to cryptographically prove what code it's running. Amazon signs this proof - we can't fake it."}
                        </p>
                        <div class="detail-row">
                            <span class="detail-label">{"Attestation"}</span>
                            <a href="/.well-known/lightfriend/attestation" target="_blank" rel="noopener noreferrer" class="detail-value detail-link-inline">
                                {"/.well-known/lightfriend/attestation"}
                                {" "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                            </a>
                        </div>
                        <div class="detail-row">
                            <span class="detail-label">{"Verify script"}</span>
                            <code class="detail-value detail-value-mono detail-value-wrap">
                                {"./scripts/verify_live_attestation.sh https://lightfriend.ai"}
                            </code>
                        </div>
                        <a href="https://github.com/ahtavarasmus/lightfriend/tree/master/tools/attestation-verifier" target="_blank" rel="noopener noreferrer" class="detail-link">
                            <i class="fa-solid fa-arrow-up-right-from-square"></i>
                            {" Verification tool source code"}
                        </a>
                    </div>
                </ChainStep>
            </div>

            // History section
            if !d.history.is_empty() {
                <div class="chain-history">
                    <h2>{"Past Approved Builds"}</h2>
                    <p class="chain-history-subtitle">
                        {"Every version that was ever approved on the blockchain. Each one links to its source code and approval transaction."}
                    </p>
                    <div class="history-list">
                        {for d.history.iter().map(|build| {
                            let commit_url = if !build.commit_hash.is_empty() {
                                Some(format!("https://github.com/ahtavarasmus/lightfriend/commit/{}", build.commit_hash))
                            } else {
                                None
                            };
                            let propose_url = if !build.propose_tx.is_empty() {
                                Some(format!("https://arbiscan.io/tx/{}", build.propose_tx))
                            } else {
                                None
                            };
                            let activate_url = build.activate_tx.as_ref().map(|tx| {
                                format!("https://arbiscan.io/tx/{}", tx)
                            });

                            html! {
                                <div class={classes!("history-item", if build.is_current { "history-current" } else { "" })}>
                                    <div class="history-item-header">
                                        if build.is_current {
                                            <span class="history-badge">{"CURRENT"}</span>
                                        }
                                        if !build.commit_hash.is_empty() {
                                            <code class="history-commit">{short_hash(&build.commit_hash, 8)}</code>
                                        }
                                        if let Some(ref ts) = build.propose_timestamp {
                                            <span class="history-time" title={format_timestamp(ts)}>
                                                {relative_time(ts)}
                                            </span>
                                        }
                                    </div>
                                    <div class="history-links">
                                        if let Some(ref url) = commit_url {
                                            <a href={url.clone()} target="_blank" rel="noopener noreferrer">
                                                <i class="fa-solid fa-code"></i>{" Source"}
                                            </a>
                                        }
                                        if let Some(ref url) = propose_url {
                                            <a href={url.clone()} target="_blank" rel="noopener noreferrer">
                                                <i class="fa-solid fa-file-signature"></i>{" Proposed"}
                                            </a>
                                        }
                                        if let Some(ref url) = activate_url {
                                            <a href={url.clone()} target="_blank" rel="noopener noreferrer">
                                                <i class="fa-solid fa-circle-check"></i>{" Activated"}
                                            </a>
                                        }
                                    </div>
                                </div>
                            }
                        })}
                    </div>
                </div>
            }

            // Cross-links
            <div class="chain-footer-links">
                <Link<Route> to={Route::Trustless} classes="chain-learn-link">
                    {"Learn how this system works"}
                    {" "}<i class="fa-solid fa-arrow-right"></i>
                </Link<Route>>
            </div>

            <div class="legal-links">
                <Link<Route> to={Route::Home}>{"Home"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Terms}>{"Terms & Conditions"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Privacy}>{"Privacy Policy"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Trustless}>{"Verifiably Private"}</Link<Route>>
            </div>
        </div>
        </>
    }
}

const STYLES: &str = r#"
.trust-chain-page {
    max-width: 720px;
    margin: 0 auto;
    padding: 2rem 1.5rem 3rem;
    color: #fff;
}

.trust-chain-header {
    text-align: center;
    margin-bottom: 2.5rem;
}

.trust-chain-header h1 {
    font-size: 2rem;
    font-weight: 700;
    margin: 0 0 0.5rem;
}

.trust-chain-subtitle {
    color: rgba(255, 255, 255, 0.6);
    font-size: 1.05rem;
    margin: 0 0 1rem;
    line-height: 1.5;
}

.chain-status-banner {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.4rem 1rem;
    border-radius: 20px;
    font-size: 0.85rem;
    font-weight: 500;
}

.chain-status-live {
    background: rgba(76, 175, 80, 0.15);
    border: 1px solid rgba(76, 175, 80, 0.3);
    color: #4CAF50;
}

.chain-loading, .chain-error {
    text-align: center;
    padding: 2rem;
    color: rgba(255, 255, 255, 0.5);
}

.chain-error-detail {
    font-size: 0.8rem;
    color: rgba(255, 255, 255, 0.3);
    margin-top: 0.5rem;
}

/* Chain steps */
.chain-steps {
    position: relative;
}

.chain-step {
    display: flex;
    gap: 1rem;
    min-height: 80px;
}

.step-connector {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 40px;
    flex-shrink: 0;
}

.step-node {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.9rem;
    flex-shrink: 0;
    z-index: 1;
}

.step-verified {
    background: rgba(76, 175, 80, 0.2);
    border: 2px solid #4CAF50;
    color: #4CAF50;
}

.step-unverified {
    background: rgba(255, 152, 0, 0.2);
    border: 2px solid #FF9800;
    color: #FF9800;
}

.step-loading {
    background: rgba(255, 255, 255, 0.08);
    border: 2px solid rgba(255, 255, 255, 0.2);
    color: rgba(255, 255, 255, 0.4);
}

.step-info {
    background: rgba(30, 144, 255, 0.15);
    border: 2px solid rgba(30, 144, 255, 0.5);
    color: #1E90FF;
}

.step-line {
    width: 2px;
    flex-grow: 1;
    background: rgba(255, 255, 255, 0.12);
    min-height: 20px;
}

.chain-step-last .step-line {
    display: none;
}

.step-content {
    flex-grow: 1;
    padding-bottom: 1.5rem;
}

.chain-step-last .step-content {
    padding-bottom: 0;
}

.step-header {
    cursor: pointer;
}

.step-title-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
}

.step-number {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.08);
    font-size: 0.7rem;
    color: rgba(255, 255, 255, 0.5);
    font-weight: 600;
}

.step-title {
    font-size: 1.05rem;
    font-weight: 600;
    margin: 0;
    flex-grow: 1;
}

.step-timestamp {
    font-size: 0.8rem;
    color: rgba(255, 255, 255, 0.4);
    white-space: nowrap;
}

.step-simple {
    color: rgba(255, 255, 255, 0.6);
    font-size: 0.9rem;
    line-height: 1.5;
    margin: 0.3rem 0 0.5rem;
}

.step-toggle {
    background: none;
    border: none;
    color: var(--color-accent, #1E90FF);
    font-size: 0.8rem;
    cursor: pointer;
    padding: 0;
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
}

.step-toggle:hover {
    text-decoration: underline;
}

/* Details panel */
.step-details {
    margin-top: 0.75rem;
    padding: 1rem;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
}

.detail-grid {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
}

.detail-explainer {
    color: rgba(255, 255, 255, 0.5);
    font-size: 0.85rem;
    line-height: 1.4;
    margin: 0 0 0.3rem;
    font-style: italic;
}

.detail-row {
    display: flex;
    align-items: baseline;
    gap: 0.75rem;
    flex-wrap: wrap;
}

.detail-label {
    font-size: 0.8rem;
    color: rgba(255, 255, 255, 0.4);
    min-width: 90px;
    flex-shrink: 0;
}

.detail-value {
    font-size: 0.85rem;
    color: rgba(255, 255, 255, 0.85);
    word-break: break-all;
}

.detail-value-mono {
    font-family: monospace;
    font-size: 0.8rem;
}

.detail-value-wrap {
    word-break: break-all;
}

.detail-approved {
    color: #4CAF50;
    font-weight: 600;
}

.detail-pending {
    color: #FF9800;
    font-weight: 600;
}

.detail-link {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    color: var(--color-accent, #1E90FF);
    text-decoration: none;
    font-size: 0.85rem;
    margin-top: 0.2rem;
}

.detail-link:hover {
    text-decoration: underline;
}

.detail-link-inline {
    color: var(--color-accent, #1E90FF);
    text-decoration: none;
}

.detail-link-inline:hover {
    text-decoration: underline;
}

/* History section */
.chain-history {
    margin-top: 3rem;
    padding-top: 2rem;
    border-top: 1px solid rgba(255, 255, 255, 0.1);
}

.chain-history h2 {
    font-size: 1.3rem;
    margin: 0 0 0.3rem;
}

.chain-history-subtitle {
    color: rgba(255, 255, 255, 0.5);
    font-size: 0.9rem;
    margin: 0 0 1.5rem;
}

.history-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
}

.history-item {
    padding: 0.75rem 1rem;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
}

.history-current {
    border-color: rgba(76, 175, 80, 0.3);
    background: rgba(76, 175, 80, 0.05);
}

.history-item-header {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    margin-bottom: 0.4rem;
    flex-wrap: wrap;
}

.history-badge {
    font-size: 0.65rem;
    font-weight: 700;
    letter-spacing: 0.05em;
    padding: 0.15rem 0.5rem;
    border-radius: 4px;
    background: rgba(76, 175, 80, 0.2);
    color: #4CAF50;
    border: 1px solid rgba(76, 175, 80, 0.3);
}

.history-commit {
    font-family: monospace;
    font-size: 0.85rem;
    color: rgba(255, 255, 255, 0.8);
}

.history-time {
    font-size: 0.8rem;
    color: rgba(255, 255, 255, 0.4);
    margin-left: auto;
}

.history-links {
    display: flex;
    gap: 1rem;
    flex-wrap: wrap;
}

.history-links a {
    font-size: 0.8rem;
    color: var(--color-accent, #1E90FF);
    text-decoration: none;
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
}

.history-links a:hover {
    text-decoration: underline;
}

/* Footer */
.chain-footer-links {
    margin-top: 2.5rem;
    text-align: center;
}

.chain-learn-link {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.6rem 1.5rem;
    background: rgba(30, 144, 255, 0.1);
    border: 1px solid rgba(30, 144, 255, 0.3);
    border-radius: 8px;
    color: #1E90FF;
    text-decoration: none;
    font-size: 0.9rem;
    transition: all 0.2s ease;
}

.chain-learn-link:hover {
    background: rgba(30, 144, 255, 0.2);
    border-color: rgba(30, 144, 255, 0.5);
}

.trust-chain-page .legal-links {
    margin-top: 2rem;
    text-align: center;
    font-size: 0.85rem;
    color: rgba(255, 255, 255, 0.4);
}

.trust-chain-page .legal-links a {
    color: rgba(255, 255, 255, 0.5);
    text-decoration: none;
}

.trust-chain-page .legal-links a:hover {
    color: #1E90FF;
}

/* Responsive */
@media (max-width: 600px) {
    .trust-chain-page {
        padding: 1.5rem 1rem 2rem;
    }

    .trust-chain-header h1 {
        font-size: 1.5rem;
    }

    .step-connector {
        width: 32px;
    }

    .step-node {
        width: 30px;
        height: 30px;
        font-size: 0.75rem;
    }

    .detail-row {
        flex-direction: column;
        gap: 0.2rem;
    }

    .detail-label {
        min-width: auto;
    }
}
"#;
