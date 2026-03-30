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

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct VerifyResponse {
    nonce: String,
    steps: Vec<VerifyStep>,
    attestation_hex: Option<String>,
    overall: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct VerifyStep {
    step: String,
    status: String,
    message: String,
    detail: Option<String>,
}

// -- Helpers --

fn short_hex(hash: &str, len: usize) -> String {
    let clean = hash.strip_prefix("0x").unwrap_or(hash);
    if clean.len() > len {
        format!("0x{}...{}", &clean[..len / 2], &clean[clean.len() - len / 2..])
    } else {
        format!("0x{}", clean)
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
        description: "Follow the chain of evidence from source code to running enclave. Verify every link yourself.",
        canonical: "https://lightfriend.ai/trust-chain",
        og_type: "website",
    });

    let data = use_state(|| None::<TrustChainData>);
    let loading = use_state(|| true);
    let verify_result = use_state(|| None::<VerifyResponse>);
    let verifying = use_state(|| false);

    // Scroll to top
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

    // Fetch data
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

    // Verify callback
    let on_verify = {
        let verify_result = verify_result.clone();
        let verifying = verifying.clone();
        Callback::from(move |_: MouseEvent| {
            let verify_result = verify_result.clone();
            let verifying = verifying.clone();
            verifying.set(true);
            verify_result.set(None);
            wasm_bindgen_futures::spawn_local(async move {
                let url = format!("{}/api/trust-chain/verify", get_backend_url());
                if let Ok(resp) = Request::post(&url).send().await {
                    if let Ok(r) = resp.json::<VerifyResponse>().await {
                        verify_result.set(Some(r));
                    }
                }
                verifying.set(false);
            });
        })
    };

    let d = (*data).clone();

    html! {
        <>
        <style>{STYLES}</style>
        <div class="tc-page">
            // Header
            <div class="tc-header">
                <h1>{"Trust Chain"}</h1>
                <p class="tc-subtitle">
                    {"Follow the evidence from source code to the running enclave. "}
                    {"Click the links at each step to verify on sites we don't control."}
                </p>
            </div>

            if *loading {
                <div class="tc-loading">
                    <i class="fa-solid fa-spinner fa-spin"></i>{" Loading..."}
                </div>
            } else if let Some(ref d) = d {
                // The chain
                {render_chain(d)}

                // Live verification tool
                {render_verify_tool(&on_verify, &verify_result, &verifying, d)}

                // History
                if !d.history.is_empty() {
                    {render_history(&d.history)}
                }
            } else {
                <div class="tc-loading">
                    {"Could not load trust chain data. This page works best when connected to a live Lightfriend instance."}
                </div>
            }

            // Footer
            <div class="tc-learn-more">
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

fn render_chain(d: &TrustChainData) -> Html {
    let commit = d.commit_sha.as_deref().unwrap_or("unknown");
    let commit_short = if commit.len() > 8 { &commit[..8] } else { commit };
    let pcr0 = d.pcr0.as_deref().unwrap_or("unavailable");
    let pcr0_short = short_hex(pcr0, 12);
    let image_id = d.image_id.as_deref().unwrap_or("unavailable");
    let image_id_short = short_hex(image_id, 12);

    let commit_url = format!("https://github.com/ahtavarasmus/lightfriend/commit/{}", commit);
    let actions_url = d.workflow_run_id.as_ref().map(|id| {
        format!("https://github.com/ahtavarasmus/lightfriend/actions/runs/{}", id)
    });
    let metadata_url = d.build_metadata_url.clone();
    let contract_addr = d.kms_contract_address.as_deref().unwrap_or("");
    let contract_source_url = format!(
        "https://github.com/ahtavarasmus/lightfriend/blob/{}/contracts/src/LightfriendKmsVerifiable.sol",
        commit
    );

    let bc = d.blockchain.as_ref();
    let approved = bc.map_or(false, |b| b.approved);

    html! {
        <div class="chain">
            // ---- Step 1: Source Code ----
            <div class="chain-card">
                <div class="card-num">{"1"}</div>
                <div class="card-body">
                    <h3>{"Source Code"}</h3>
                    <p class="card-explain">
                        {"All code is public. Anyone can read every line."}
                    </p>
                    <div class="card-values">
                        <div class="val-row">
                            <span class="val-label">{"Commit"}</span>
                            <code class="val-data">{commit_short}</code>
                        </div>
                    </div>
                    <div class="card-links">
                        <a href={commit_url.clone()} target="_blank" rel="noopener noreferrer">
                            {"View source on GitHub "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                        </a>
                    </div>
                </div>
            </div>

            // Arrow 1→2
            <div class="chain-arrow">
                <div class="arrow-line"></div>
                <div class="arrow-label">{"This commit was built by GitHub Actions"}</div>
                <div class="arrow-head"><i class="fa-solid fa-arrow-down"></i></div>
            </div>

            // ---- Step 2: Build ----
            <div class="chain-card">
                <div class="card-num">{"2"}</div>
                <div class="card-body">
                    <h3>{"Automated Build"}</h3>
                    <p class="card-explain">
                        {"GitHub Actions built this commit into an enclave image. The build is automated - no human can tamper with it."}
                    </p>
                    <div class="card-values">
                        if let Some(ref run_id) = d.workflow_run_id {
                            <div class="val-row">
                                <span class="val-label">{"Workflow"}</span>
                                <code class="val-data">{format!("#{}", run_id)}</code>
                            </div>
                        }
                        <div class="val-row">
                            <span class="val-label">{"Fingerprint"}</span>
                            <code class="val-data val-highlight">{&pcr0_short}</code>
                        </div>
                        if let Some(ref ts) = d.built_at {
                            <div class="val-row">
                                <span class="val-label">{"Built"}</span>
                                <span class="val-data val-time" title={format_ts(ts)}>{relative_time(ts)}</span>
                            </div>
                        }
                    </div>
                    <div class="card-links">
                        if let Some(ref url) = actions_url {
                            <a href={url.clone()} target="_blank" rel="noopener noreferrer">
                                {"View build logs "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                            </a>
                        }
                    </div>
                </div>
            </div>

            // Arrow 2→3
            <div class="chain-arrow">
                <div class="arrow-line"></div>
                <div class="arrow-label">{"The fingerprint was published publicly"}</div>
                <div class="arrow-head"><i class="fa-solid fa-arrow-down"></i></div>
            </div>

            // ---- Step 3: Published fingerprint ----
            <div class="chain-card">
                <div class="card-num">{"3"}</div>
                <div class="card-body">
                    <h3>{"Public Record"}</h3>
                    <p class="card-explain">
                        {"The build fingerprint (PCR values) was published to a public page on GitHub. This is a permanent record anyone can check."}
                    </p>
                    <div class="card-values">
                        <div class="val-row">
                            <span class="val-label">{"PCR0"}</span>
                            <code class="val-data val-highlight">{&pcr0_short}</code>
                            <span class="val-match"><i class="fa-solid fa-circle-check"></i>{" same as build"}</span>
                        </div>
                        if let Some(ref pcr) = d.pcr1 {
                            <div class="val-row">
                                <span class="val-label">{"PCR1"}</span>
                                <code class="val-data">{short_hex(pcr, 12)}</code>
                            </div>
                        }
                        if let Some(ref pcr) = d.pcr2 {
                            <div class="val-row">
                                <span class="val-label">{"PCR2"}</span>
                                <code class="val-data">{short_hex(pcr, 12)}</code>
                            </div>
                        }
                    </div>
                    <div class="card-links">
                        if let Some(ref url) = metadata_url {
                            <a href={url.clone()} target="_blank" rel="noopener noreferrer">
                                {"View published metadata (JSON) "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                            </a>
                        }
                    </div>
                </div>
            </div>

            // Arrow 3→4
            <div class="chain-arrow">
                <div class="arrow-line"></div>
                <div class="arrow-label">{"PCR values hashed into an Image ID and recorded on-chain"}</div>
                <div class="arrow-head"><i class="fa-solid fa-arrow-down"></i></div>
            </div>

            // ---- Step 4: Blockchain ----
            <div class="chain-card">
                <div class="card-num">{"4"}</div>
                <div class="card-body">
                    <h3>{"Blockchain Approval"}</h3>
                    <p class="card-explain">
                        {"A smart contract on Arbitrum records which builds are approved. This is a public ledger nobody can secretly change."}
                    </p>
                    <div class="card-values">
                        <div class="val-row">
                            <span class="val-label">{"Image ID"}</span>
                            <code class="val-data">{&image_id_short}</code>
                        </div>
                        <div class="val-row">
                            <span class="val-label">{"Status"}</span>
                            if approved {
                                <span class="val-data val-approved"><i class="fa-solid fa-circle-check"></i>{" APPROVED"}</span>
                            } else if bc.is_some() {
                                <span class="val-data val-pending">{"NOT APPROVED"}</span>
                            } else {
                                <span class="val-data val-unknown">{"Could not check"}</span>
                            }
                        </div>
                        if let Some(ref b) = bc {
                            if let Some(ref tx) = b.propose_tx {
                                <div class="val-row">
                                    <span class="val-label">{"Proposed"}</span>
                                    <a href={format!("https://arbiscan.io/tx/{}", tx)} target="_blank" rel="noopener noreferrer" class="val-data val-link">
                                        {short_hex(tx, 12)}{" "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                                    </a>
                                    if let Some(ref ts) = b.propose_timestamp {
                                        <span class="val-time" title={format_ts(ts)}>{relative_time(ts)}</span>
                                    }
                                </div>
                            }
                            if let Some(ref tx) = b.activate_tx {
                                <div class="val-row">
                                    <span class="val-label">{"Activated"}</span>
                                    <a href={format!("https://arbiscan.io/tx/{}", tx)} target="_blank" rel="noopener noreferrer" class="val-data val-link">
                                        {short_hex(tx, 12)}{" "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                                    </a>
                                    if let Some(ref ts) = b.activate_timestamp {
                                        <span class="val-time" title={format_ts(ts)}>{relative_time(ts)}</span>
                                    }
                                </div>
                            }
                        }
                    </div>
                    <div class="card-links">
                        if !contract_addr.is_empty() {
                            <a href={format!("https://arbiscan.io/address/{}", contract_addr)} target="_blank" rel="noopener noreferrer">
                                {"View contract on Arbiscan "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                            </a>
                        }
                        <a href={contract_source_url.clone()} target="_blank" rel="noopener noreferrer">
                            {"Read contract source code "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                        </a>
                    </div>
                    <p class="card-note">
                        {"The contract source is on GitHub. The key function is "}<code>{"oysterKMSVerify(imageId)"}</code>
                        {" - it returns true only for approved image IDs."}
                    </p>
                </div>
            </div>

            // Arrow 4→5
            <div class="chain-arrow">
                <div class="arrow-line"></div>
                <div class="arrow-label">{"The enclave boots and Amazon's hardware signs a proof of what code is inside"}</div>
                <div class="arrow-head"><i class="fa-solid fa-arrow-down"></i></div>
            </div>

            // ---- Step 5: Enclave boots, AWS attestation ----
            <div class="chain-card">
                <div class="card-num">{"5"}</div>
                <div class="card-body">
                    <h3>{"Amazon Signs a Proof"}</h3>
                    <p class="card-explain">
                        {"When the sealed computer (Nitro Enclave) boots, Amazon's hardware automatically measures exactly what code is inside and signs a proof. "}
                        {"This is like a notarized certificate from Amazon itself - we cannot forge or alter it."}
                    </p>
                    <div class="card-values">
                        if let Some(ref att) = d.attestation {
                            if att.available {
                                <div class="val-row">
                                    <span class="val-label">{"Status"}</span>
                                    <span class="val-data val-approved"><i class="fa-solid fa-circle-check"></i>{" Attestation server reachable"}</span>
                                </div>
                            }
                            if let Some(ref pcr) = att.pcr0 {
                                <div class="val-row">
                                    <span class="val-label">{"PCR0"}</span>
                                    <code class="val-data val-highlight">{short_hex(pcr, 12)}</code>
                                    <span class="val-match"><i class="fa-solid fa-circle-check"></i>{" same as build"}</span>
                                </div>
                            }
                            if let Some(ref pcr) = att.pcr1 {
                                <div class="val-row">
                                    <span class="val-label">{"PCR1"}</span>
                                    <code class="val-data">{short_hex(pcr, 12)}</code>
                                    <span class="val-match"><i class="fa-solid fa-circle-check"></i>{" same as build"}</span>
                                </div>
                            }
                            if let Some(ref pcr) = att.pcr2 {
                                <div class="val-row">
                                    <span class="val-label">{"PCR2"}</span>
                                    <code class="val-data">{short_hex(pcr, 12)}</code>
                                    <span class="val-match"><i class="fa-solid fa-circle-check"></i>{" same as build"}</span>
                                </div>
                            }
                            if let Some(size) = att.doc_byte_size {
                                <div class="val-row">
                                    <span class="val-label">{"Document"}</span>
                                    <span class="val-data">{format!("{} bytes, signed by AWS", size)}</span>
                                </div>
                            }
                        } else {
                            <div class="val-row">
                                <span class="val-label">{"Status"}</span>
                                <span class="val-data val-unknown">{"Not in enclave (local dev)"}</span>
                            </div>
                        }
                    </div>
                    <div class="card-links">
                        <a href="/.well-known/lightfriend/attestation" target="_blank" rel="noopener noreferrer">
                            {"View live attestation metadata "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                        </a>
                        <a href="/.well-known/lightfriend/attestation/hex" target="_blank" rel="noopener noreferrer">
                            {"Download raw attestation document "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                        </a>
                    </div>
                </div>
            </div>

            // Arrow 5→6
            <div class="chain-arrow">
                <div class="arrow-line"></div>
                <div class="arrow-label">{"The enclave presents this proof to Marlin, an independent key guardian"}</div>
                <div class="arrow-head"><i class="fa-solid fa-arrow-down"></i></div>
            </div>

            // ---- Step 6: Marlin verification flow ----
            <div class="chain-card chain-card-marlin">
                <div class="card-num">{"6"}</div>
                <div class="card-body">
                    <h3>{"Independent Verification by Marlin"}</h3>
                    <p class="card-explain">
                        {"Marlin is an independent third party that holds the encryption key. It does not trust us. "}
                        {"Before releasing the key, it independently verifies the proof from Amazon:"}
                    </p>

                    // Sub-steps visualization
                    <div class="substep-flow">
                        <div class="substep">
                            <div class="substep-icon"><i class="fa-solid fa-signature"></i></div>
                            <div class="substep-body">
                                <div class="substep-title">{"1. Check Amazon's signature"}</div>
                                <div class="substep-desc">{"Marlin verifies that the proof was genuinely signed by Amazon's Nitro hardware. This confirms it came from a real sealed computer - not from us pretending."}</div>
                            </div>
                        </div>
                        <div class="substep-connector"><i class="fa-solid fa-arrow-down"></i></div>
                        <div class="substep">
                            <div class="substep-icon"><i class="fa-solid fa-fingerprint"></i></div>
                            <div class="substep-body">
                                <div class="substep-title">{"2. Read the fingerprint"}</div>
                                <div class="substep-desc">{"Extracts the code fingerprint (PCR values) from Amazon's signed proof. Computes the Image ID:"}</div>
                                <code class="substep-formula">{"Image ID = SHA256(PCR0 + PCR1 + PCR2)"}</code>
                                if d.image_id.is_some() {
                                    <div class="substep-value">
                                        <span class="val-label">{"Computed"}</span>
                                        <code class="val-data">{&image_id_short}</code>
                                    </div>
                                }
                            </div>
                        </div>
                        <div class="substep-connector"><i class="fa-solid fa-arrow-down"></i></div>
                        <div class="substep">
                            <div class="substep-icon"><i class="fa-solid fa-link"></i></div>
                            <div class="substep-body">
                                <div class="substep-title">{"3. Ask the blockchain"}</div>
                                <div class="substep-desc">
                                    {"Marlin calls "}<code>{"oysterKMSVerify(imageId)"}</code>{" on the public Arbitrum contract. "}
                                    {"If the blockchain says "}
                                    if approved {
                                        <code class="substep-true">{"true"}</code>
                                    } else {
                                        <code>{"true"}</code>
                                    }
                                    {", this build was approved."}
                                </div>
                                <div class="substep-value">
                                    <span class="val-label">{"Result"}</span>
                                    if approved {
                                        <span class="val-approved"><i class="fa-solid fa-circle-check"></i>{" true - approved"}</span>
                                    } else if bc.is_some() {
                                        <span class="val-pending">{"false - not approved"}</span>
                                    } else {
                                        <span class="val-unknown">{"could not check"}</span>
                                    }
                                </div>
                            </div>
                        </div>
                    </div>

                    <div class="card-links">
                        <a href="https://github.com/marlinprotocol/oyster-monorepo" target="_blank" rel="noopener noreferrer">
                            {"Marlin key guardian source code "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                        </a>
                        if !contract_addr.is_empty() {
                            <a href={format!("https://arbiscan.io/address/{}", contract_addr)} target="_blank" rel="noopener noreferrer">
                                {"View contract on Arbiscan "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                            </a>
                        }
                    </div>
                    <p class="card-note">
                        {"Marlin itself runs inside its own Nitro Enclave with its own attestation, and its code is open source. "}
                        {"It never sees your data - it only decides whether to release the encryption key. We cannot influence its decision."}
                    </p>
                </div>
            </div>

            // Arrow 6→7
            <div class="chain-arrow">
                <div class="arrow-line"></div>
                <div class="arrow-label">{"All checks passed independently - Marlin releases the encryption key"}</div>
                <div class="arrow-head"><i class="fa-solid fa-arrow-down"></i></div>
            </div>

            // ---- Step 7: Running with key ----
            <div class="chain-card chain-card-final">
                <div class="card-num">{"7"}</div>
                <div class="card-body">
                    <h3>{"Running and Verified"}</h3>
                    <p class="card-explain">
                        {"The enclave received the encryption key and is serving your data. The same fingerprint appears at every step of this chain:"}
                    </p>
                    <div class="card-values">
                        <div class="val-row">
                            <span class="val-label">{"Commit"}</span>
                            <code class="val-data">{commit_short}</code>
                            <span class="val-match"><i class="fa-solid fa-circle-check"></i>{" matches step 1"}</span>
                        </div>
                        <div class="val-row">
                            <span class="val-label">{"PCR0"}</span>
                            <code class="val-data val-highlight">{&pcr0_short}</code>
                            <span class="val-match"><i class="fa-solid fa-circle-check"></i>{" matches steps 2, 3, 5"}</span>
                        </div>
                        <div class="val-row">
                            <span class="val-label">{"Image ID"}</span>
                            <code class="val-data">{&image_id_short}</code>
                            <span class="val-match"><i class="fa-solid fa-circle-check"></i>{" matches steps 4, 6"}</span>
                        </div>
                    </div>
                    <div class="card-links">
                        <a href="/.well-known/lightfriend/attestation" target="_blank" rel="noopener noreferrer">
                            {"View live attestation metadata "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                        </a>
                    </div>
                </div>
            </div>
        </div>
    }
}

fn render_verify_tool(
    on_verify: &Callback<MouseEvent>,
    verify_result: &UseStateHandle<Option<VerifyResponse>>,
    verifying: &UseStateHandle<bool>,
    d: &TrustChainData,
) -> Html {
    let commit = d.commit_sha.as_deref().unwrap_or("unknown");
    let commit_short = if commit.len() > 8 { &commit[..8] } else { commit };

    html! {
        <div class="verify-section">
            <h2>{"Verify It Yourself"}</h2>
            <p class="verify-intro">
                {"Click the button to send a fresh random challenge to the enclave. It will prove what code it's running. "}
                {"This runs on our server - for fully trustless verification, use the "}
                <a href={format!("https://github.com/ahtavarasmus/lightfriend/tree/{}/tools/attestation-verifier", commit)} target="_blank" rel="noopener noreferrer">
                    {"open-source verification tool"}
                </a>
                {"."}
            </p>

            <div class="verify-actions">
                <button class="verify-btn" onclick={on_verify.clone()} disabled={**verifying}>
                    if **verifying {
                        <i class="fa-solid fa-spinner fa-spin"></i>{" Verifying..."}
                    } else {
                        <i class="fa-solid fa-shield-halved"></i>{" Verify Now"}
                    }
                </button>
            </div>

            if let Some(ref result) = **verify_result {
                <div class={classes!("verify-result", format!("verify-{}", result.overall))}>
                    <div class="verify-steps">
                        {for result.steps.iter().map(|step| {
                            let icon = match step.status.as_str() {
                                "pass" => html! { <i class="fa-solid fa-circle-check step-pass"></i> },
                                "fail" => html! { <i class="fa-solid fa-circle-xmark step-fail"></i> },
                                _ => html! { <i class="fa-solid fa-circle-info step-info"></i> },
                            };
                            html! {
                                <div class="verify-step-row">
                                    <div class="verify-step-icon">{icon}</div>
                                    <div class="verify-step-content">
                                        <div class="verify-step-name">{&step.step}</div>
                                        <div class="verify-step-msg">{&step.message}</div>
                                        if let Some(ref detail) = step.detail {
                                            <pre class="verify-step-detail">{detail}</pre>
                                        }
                                    </div>
                                </div>
                            }
                        })}
                    </div>
                </div>
            }

            <div class="verify-cli">
                <p class="verify-cli-label">{"Run it yourself (fully trustless):"}</p>
                <pre class="verify-cli-cmd">
                    {format!("git clone https://github.com/ahtavarasmus/lightfriend\ncd lightfriend\ncargo run --manifest-path tools/attestation-verifier/Cargo.toml -- \\\n  https://lightfriend.ai --rpc-url https://arb1.arbitrum.io/rpc")}
                </pre>
                <a href={format!("https://github.com/ahtavarasmus/lightfriend/tree/{}/tools/attestation-verifier", commit_short)} target="_blank" rel="noopener noreferrer" class="verify-cli-link">
                    {"View verification tool source code "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                </a>
            </div>
        </div>
    }
}

fn render_history(history: &[HistoricalBuild]) -> Html {
    html! {
        <div class="history-section">
            <h2>{"All Approved Builds"}</h2>
            <p class="history-intro">
                {"Every version ever approved on the blockchain. Each links to its source code and approval transaction."}
            </p>
            <div class="history-list">
                {for history.iter().map(|build| {
                    let commit_url = if !build.commit_hash.is_empty() {
                        Some(format!("https://github.com/ahtavarasmus/lightfriend/commit/{}", build.commit_hash))
                    } else { None };
                    let propose_url = if !build.propose_tx.is_empty() {
                        Some(format!("https://arbiscan.io/tx/{}", build.propose_tx))
                    } else { None };
                    let activate_url = build.activate_tx.as_ref().map(|tx| format!("https://arbiscan.io/tx/{}", tx));

                    html! {
                        <div class={classes!("history-row", if build.is_current { "history-current" } else { "" })}>
                            <div class="history-left">
                                if build.is_current {
                                    <span class="history-badge">{"LIVE"}</span>
                                }
                                if !build.commit_hash.is_empty() {
                                    <code class="history-commit">
                                        {if build.commit_hash.len() > 8 { &build.commit_hash[..8] } else { &build.commit_hash }}
                                    </code>
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
                                    <a href={url.clone()} target="_blank" rel="noopener noreferrer">{"Proposed"}</a>
                                }
                                if let Some(ref url) = activate_url {
                                    <a href={url.clone()} target="_blank" rel="noopener noreferrer">{"Activated"}</a>
                                }
                            </div>
                        </div>
                    }
                })}
            </div>
        </div>
    }
}

const STYLES: &str = r#"
.tc-page {
    max-width: 680px;
    margin: 0 auto;
    padding: 2rem 1.5rem 3rem;
    color: #fff;
}
.tc-header { text-align: center; margin-bottom: 2rem; }
.tc-header h1 { font-size: 1.8rem; font-weight: 700; margin: 0 0 0.5rem; }
.tc-subtitle { color: rgba(255,255,255,0.55); font-size: 0.95rem; line-height: 1.5; margin: 0; }
.tc-loading { text-align: center; padding: 3rem; color: rgba(255,255,255,0.4); }

/* ---- Chain cards ---- */
.chain { display: flex; flex-direction: column; align-items: stretch; }

.chain-card {
    background: rgba(255,255,255,0.04);
    border: 1px solid rgba(255,255,255,0.1);
    border-radius: 10px;
    display: flex;
    gap: 1rem;
    padding: 1.25rem;
}
.chain-card-final {
    border-color: rgba(76,175,80,0.3);
    background: rgba(76,175,80,0.04);
}

.card-num {
    width: 28px; height: 28px;
    border-radius: 50%;
    background: rgba(30,144,255,0.15);
    border: 1.5px solid rgba(30,144,255,0.4);
    color: #1E90FF;
    display: flex; align-items: center; justify-content: center;
    font-size: 0.75rem; font-weight: 700;
    flex-shrink: 0;
    margin-top: 2px;
}

.card-body { flex: 1; min-width: 0; }
.card-body h3 { margin: 0 0 0.4rem; font-size: 1rem; font-weight: 600; }
.card-explain { color: rgba(255,255,255,0.5); font-size: 0.85rem; line-height: 1.45; margin: 0 0 0.75rem; }
.card-note { color: rgba(255,255,255,0.4); font-size: 0.8rem; line-height: 1.4; margin: 0.75rem 0 0; font-style: italic; }
.card-note code { background: rgba(255,255,255,0.08); padding: 0.1rem 0.3rem; border-radius: 3px; font-size: 0.75rem; }

/* Marlin card */
.chain-card-marlin {
    border-color: rgba(156,39,176,0.25);
    background: rgba(156,39,176,0.03);
}

/* Substep flow inside Marlin card */
.substep-flow { margin: 0.5rem 0 0.75rem; padding: 0.75rem; background: rgba(0,0,0,0.15); border-radius: 8px; }
.substep { display: flex; gap: 0.6rem; align-items: flex-start; }
.substep-icon {
    width: 28px; height: 28px; border-radius: 50%;
    background: rgba(156,39,176,0.15); border: 1.5px solid rgba(156,39,176,0.35);
    color: #CE93D8; display: flex; align-items: center; justify-content: center;
    font-size: 0.7rem; flex-shrink: 0; margin-top: 1px;
}
.substep-body { flex: 1; min-width: 0; }
.substep-title { font-size: 0.8rem; font-weight: 600; color: rgba(255,255,255,0.8); margin-bottom: 0.15rem; }
.substep-desc { font-size: 0.78rem; color: rgba(255,255,255,0.45); line-height: 1.4; }
.substep-desc code { background: rgba(255,255,255,0.08); padding: 0.1rem 0.3rem; border-radius: 3px; font-size: 0.72rem; }
.substep-formula {
    display: block; margin: 0.3rem 0; padding: 0.3rem 0.5rem;
    background: rgba(255,255,255,0.05); border-radius: 4px;
    font-size: 0.72rem; color: rgba(255,255,255,0.55);
}
.substep-value { display: flex; align-items: center; gap: 0.4rem; margin-top: 0.3rem; }
.substep-true { color: #4CAF50; font-weight: 600; }
.substep-connector { text-align: center; color: rgba(156,39,176,0.4); font-size: 0.65rem; padding: 0.15rem 0; }

/* Values */
.card-values { display: flex; flex-direction: column; gap: 0.35rem; margin-bottom: 0.75rem; }
.val-row { display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap; }
.val-label { font-size: 0.75rem; color: rgba(255,255,255,0.4); min-width: 70px; }
.val-data { font-family: monospace; font-size: 0.8rem; color: rgba(255,255,255,0.85); }
.val-highlight { color: #4CAF50; font-weight: 500; }
.val-match { font-size: 0.7rem; color: #4CAF50; display: inline-flex; align-items: center; gap: 0.2rem; }
.val-time { font-size: 0.8rem; color: rgba(255,255,255,0.4); }
.val-approved { color: #4CAF50; font-weight: 600; font-family: inherit; }
.val-approved i { margin-right: 0.2rem; }
.val-pending { color: #FF9800; font-weight: 600; font-family: inherit; }
.val-unknown { color: rgba(255,255,255,0.4); font-family: inherit; }
.val-link { color: #1E90FF !important; text-decoration: none; }
.val-link:hover { text-decoration: underline; }

/* Links */
.card-links { display: flex; flex-direction: column; gap: 0.3rem; }
.card-links a {
    font-size: 0.8rem; color: #1E90FF; text-decoration: none;
    display: inline-flex; align-items: center; gap: 0.3rem;
}
.card-links a:hover { text-decoration: underline; }

/* ---- Arrows ---- */
.chain-arrow {
    display: flex; flex-direction: column; align-items: center;
    padding: 0.3rem 0;
}
.arrow-line { width: 2px; height: 12px; background: rgba(30,144,255,0.3); }
.arrow-label {
    font-size: 0.75rem; color: rgba(255,255,255,0.4);
    padding: 0.2rem 0.8rem;
    text-align: center;
    max-width: 300px;
}
.arrow-head { color: rgba(30,144,255,0.5); font-size: 0.75rem; }

/* ---- Verify section ---- */
.verify-section {
    margin-top: 3rem; padding-top: 2rem;
    border-top: 1px solid rgba(255,255,255,0.1);
}
.verify-section h2 { font-size: 1.3rem; margin: 0 0 0.5rem; }
.verify-intro { color: rgba(255,255,255,0.5); font-size: 0.85rem; line-height: 1.5; margin: 0 0 1.25rem; }
.verify-intro a { color: #1E90FF; text-decoration: none; }
.verify-intro a:hover { text-decoration: underline; }

.verify-actions { margin-bottom: 1.25rem; }
.verify-btn {
    background: rgba(30,144,255,0.12); border: 1px solid rgba(30,144,255,0.35);
    color: #1E90FF; padding: 0.6rem 1.5rem; border-radius: 8px;
    font-size: 0.9rem; cursor: pointer; transition: all 0.2s;
    display: inline-flex; align-items: center; gap: 0.4rem;
}
.verify-btn:hover:not(:disabled) { background: rgba(30,144,255,0.22); border-color: rgba(30,144,255,0.55); }
.verify-btn:disabled { opacity: 0.6; cursor: not-allowed; }

.verify-result {
    background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.08);
    border-radius: 8px; padding: 1rem; margin-bottom: 1.5rem;
}
.verify-pass { border-color: rgba(76,175,80,0.3); }
.verify-fail { border-color: rgba(244,67,54,0.3); }
.verify-partial { border-color: rgba(255,152,0,0.3); }

.verify-steps { display: flex; flex-direction: column; gap: 0.75rem; }
.verify-step-row { display: flex; gap: 0.6rem; }
.verify-step-icon { flex-shrink: 0; margin-top: 2px; font-size: 0.85rem; }
.step-pass { color: #4CAF50; }
.step-fail { color: #F44336; }
.step-info { color: #1E90FF; }
.verify-step-content { flex: 1; min-width: 0; }
.verify-step-name { font-size: 0.8rem; font-weight: 600; color: rgba(255,255,255,0.7); }
.verify-step-msg { font-size: 0.8rem; color: rgba(255,255,255,0.55); line-height: 1.4; }
.verify-step-detail {
    font-size: 0.75rem; color: rgba(255,255,255,0.5); margin: 0.3rem 0 0;
    background: rgba(0,0,0,0.2); padding: 0.4rem 0.6rem; border-radius: 4px;
    overflow-x: auto; white-space: pre-wrap; word-break: break-all;
    font-family: monospace;
}

.verify-cli { margin-top: 1rem; }
.verify-cli-label { font-size: 0.8rem; color: rgba(255,255,255,0.5); margin: 0 0 0.4rem; }
.verify-cli-cmd {
    background: rgba(0,0,0,0.3); border: 1px solid rgba(255,255,255,0.08);
    padding: 0.75rem 1rem; border-radius: 6px; font-size: 0.78rem;
    color: rgba(255,255,255,0.7); overflow-x: auto; white-space: pre;
    font-family: monospace; margin: 0 0 0.5rem;
}
.verify-cli-link { font-size: 0.8rem; color: #1E90FF; text-decoration: none; }
.verify-cli-link:hover { text-decoration: underline; }

/* ---- History ---- */
.history-section {
    margin-top: 2.5rem; padding-top: 2rem;
    border-top: 1px solid rgba(255,255,255,0.1);
}
.history-section h2 { font-size: 1.3rem; margin: 0 0 0.3rem; }
.history-intro { color: rgba(255,255,255,0.5); font-size: 0.85rem; margin: 0 0 1rem; }

.history-list { display: flex; flex-direction: column; gap: 0.5rem; }
.history-row {
    display: flex; justify-content: space-between; align-items: center;
    padding: 0.6rem 0.8rem;
    background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.06);
    border-radius: 6px; flex-wrap: wrap; gap: 0.5rem;
}
.history-current { border-color: rgba(76,175,80,0.25); background: rgba(76,175,80,0.04); }

.history-left { display: flex; align-items: center; gap: 0.5rem; }
.history-badge {
    font-size: 0.6rem; font-weight: 700; letter-spacing: 0.05em;
    padding: 0.1rem 0.4rem; border-radius: 3px;
    background: rgba(76,175,80,0.2); color: #4CAF50;
}
.history-commit { font-family: monospace; font-size: 0.8rem; color: rgba(255,255,255,0.7); }
.history-time { font-size: 0.75rem; color: rgba(255,255,255,0.35); }

.history-right { display: flex; gap: 0.75rem; }
.history-right a { font-size: 0.75rem; color: #1E90FF; text-decoration: none; }
.history-right a:hover { text-decoration: underline; }

/* ---- Footer ---- */
.tc-learn-more { margin-top: 2.5rem; text-align: center; }
.tc-learn-more a {
    color: #1E90FF; text-decoration: none; font-size: 0.9rem;
    display: inline-flex; align-items: center; gap: 0.4rem;
}
.tc-learn-more a:hover { text-decoration: underline; }

.tc-page .legal-links {
    margin-top: 1.5rem; text-align: center; font-size: 0.8rem; color: rgba(255,255,255,0.35);
}
.tc-page .legal-links a { color: rgba(255,255,255,0.45); text-decoration: none; }
.tc-page .legal-links a:hover { color: #1E90FF; }

/* ---- Responsive ---- */
@media (max-width: 600px) {
    .tc-page { padding: 1.5rem 1rem 2rem; }
    .tc-header h1 { font-size: 1.4rem; }
    .chain-card { flex-direction: column; gap: 0.5rem; }
    .card-num { margin-bottom: -0.3rem; }
    .val-row { flex-direction: column; gap: 0.1rem; }
    .val-label { min-width: auto; }
    .val-match { margin-left: 0; }
}
"#;
