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
                // Section 1: Code verification
                {render_code_chain(d)}

                // Section 2: Key protection
                {render_key_chain(d)}

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

// Shared helpers
fn v(d: &TrustChainData) -> (&str, &str, &str, &str) {
    let commit = d.commit_sha.as_deref().unwrap_or("unknown");
    let commit_short = if commit.len() > 8 { &commit[..8] } else { commit };
    let pcr0 = d.pcr0.as_deref().unwrap_or("unavailable");
    let contract_addr = d.kms_contract_address.as_deref().unwrap_or("");
    (commit, commit_short, pcr0, contract_addr)
}

// ---- Section 1: Verify the code ----
fn render_code_chain(d: &TrustChainData) -> Html {
    let (commit, commit_short, pcr0, contract_addr) = v(d);
    let pcr0_display = short_hex(pcr0, 16);
    let commit_url = format!("https://github.com/ahtavarasmus/lightfriend/commit/{}", commit);
    let actions_url = d.workflow_run_id.as_ref().map(|id|
        format!("https://github.com/ahtavarasmus/lightfriend/actions/runs/{}", id)
    );

    html! {
        <div class="tc-section">
            <h2 class="tc-section-title">
                <i class="fa-solid fa-magnifying-glass"></i>
                {" Verify the Code"}
            </h2>
            <p class="tc-section-desc">{"Is the code running on Lightfriend really the open-source code on GitHub?"}</p>

            <div class="chain">
                // Step 1
                <div class="chain-card">
                    <div class="card-num">{"1"}</div>
                    <div class="card-body">
                        <h3>{"The code is public on GitHub"}</h3>
                        <p class="card-explain">{"Every line of Lightfriend's code is public. The version running right now was built from this commit:"}</p>
                        <pre class="code-block">{format!("commit {}", commit)}</pre>
                        <div class="card-links">
                            <a href={commit_url.clone()} target="_blank" rel="noopener noreferrer">
                                {"Open this commit on GitHub"}<span class="link-hint">{" - you'll see every file that was changed"}</span>
                                {" "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                            </a>
                        </div>
                    </div>
                </div>

                <div class="chain-arrow">
                    <div class="arrow-line"></div>
                    <div class="arrow-label">{"GitHub Actions built this commit automatically"}</div>
                    <div class="arrow-head"><i class="fa-solid fa-arrow-down"></i></div>
                </div>

                // Step 2
                <div class="chain-card">
                    <div class="card-num">{"2"}</div>
                    <div class="card-body">
                        <h3>{"An automated system built it"}</h3>
                        <p class="card-explain">
                            {"GitHub Actions (a robot, not a human) took that commit and built it into a sealed computer image. "}
                            {"The build produced a unique fingerprint:"}
                        </p>
                        <pre class="code-block code-highlight">{format!("PCR0: {}", pcr0)}</pre>
                        <p class="card-explain">
                            {"This fingerprint is like DNA - if even one character of code changes, this value would be completely different."}
                        </p>
                        if let Some(ref ts) = d.built_at {
                            <p class="card-time">{"Built "}{relative_time(ts)}{" ("}{format_ts(ts)}{")"}</p>
                        }
                        <div class="card-links">
                            if let Some(ref url) = actions_url {
                                <a href={url.clone()} target="_blank" rel="noopener noreferrer">
                                    {"Open the build logs"}<span class="link-hint">{" - search for \"PCR0\" to find this exact value"}</span>
                                    {" "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                                </a>
                            }
                            if let Some(ref url) = d.build_metadata_url {
                                <a href={url.clone()} target="_blank" rel="noopener noreferrer">
                                    {"View published fingerprint (GitHub Pages)"}<span class="link-hint">{" - same PCR0 value stored permanently"}</span>
                                    {" "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                                </a>
                            }
                        </div>
                    </div>
                </div>

                <div class="chain-arrow">
                    <div class="arrow-line"></div>
                    <div class="arrow-label">{"The fingerprint was saved to a public page"}</div>
                    <div class="arrow-head"><i class="fa-solid fa-arrow-down"></i></div>
                </div>

                // Step 3
                <div class="chain-card">
                    <div class="card-num">{"3"}</div>
                    <div class="card-body">
                        <h3>{"The fingerprint is published permanently"}</h3>
                        <p class="card-explain">{"The build saved the fingerprint to a public JSON file on GitHub Pages. Click the link and look for the same PCR0 value:"}</p>
                        <pre class="code-block">{format!("{{\n  \"pcr0\": \"{}\",\n  \"commit_sha\": \"{}\"\n}}", pcr0, commit_short)}</pre>
                        <div class="card-links">
                            if let Some(ref url) = d.build_metadata_url {
                                <a href={url.clone()} target="_blank" rel="noopener noreferrer">
                                    {"Open the published metadata"}<span class="link-hint">{" - compare the pcr0 value with step 2"}</span>
                                    {" "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                                </a>
                            }
                        </div>
                    </div>
                </div>

                <div class="chain-arrow">
                    <div class="arrow-line"></div>
                    <div class="arrow-label">{"Now check: does the live server report the same fingerprint?"}</div>
                    <div class="arrow-head"><i class="fa-solid fa-arrow-down"></i></div>
                </div>

                // Step 4
                <div class="chain-card chain-card-final">
                    <div class="card-num">{"4"}</div>
                    <div class="card-body">
                        <h3>{"The live server matches"}</h3>
                        <p class="card-explain">{"The server running right now reports the same fingerprint. Amazon's hardware signs this value - we cannot fake it."}</p>
                        <pre class="code-block code-highlight">{format!("PCR0: {}", pcr0)}</pre>
                        <p class="card-explain card-explain-match">
                            <i class="fa-solid fa-circle-check"></i>
                            {" This matches the build (step 2) and the published record (step 3). The code on GitHub is what's running."}
                        </p>
                        <p class="card-explain card-explain-howto">
                            <strong>{"Verify it yourself: "}</strong>
                            {"Run the open-source verification tool to independently check Amazon's signature and PCR values:"}
                        </p>
                        <pre class="code-block">{format!("git clone https://github.com/ahtavarasmus/lightfriend\ncd lightfriend/tools/attestation-verifier\ncargo run -- https://lightfriend.ai")}</pre>
                        <div class="card-links">
                            <a href="/.well-known/lightfriend/attestation" target="_blank" rel="noopener noreferrer">
                                {"Open live attestation"}<span class="link-hint">{" - compare the pcr0 value yourself"}</span>
                                {" "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                            </a>
                            <a href={format!("https://github.com/ahtavarasmus/lightfriend/tree/{}/tools/attestation-verifier", commit)} target="_blank" rel="noopener noreferrer">
                                {"Read the verification tool source code"}<span class="link-hint">{" - see exactly what it checks"}</span>
                                {" "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                            </a>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}

// ---- Section 2: Encryption key protection ----
fn render_key_chain(d: &TrustChainData) -> Html {
    let (_commit, _commit_short, pcr0, contract_addr) = v(d);
    let image_id = d.image_id.as_deref().unwrap_or("unavailable");
    let bc = d.blockchain.as_ref();
    let approved = bc.map_or(false, |b| b.approved);
    let actions_url = d.workflow_run_id.as_ref().map(|id|
        format!("https://github.com/ahtavarasmus/lightfriend/actions/runs/{}", id)
    );

    html! {
        <div class="tc-section tc-section-key">
            <h2 class="tc-section-title">
                <i class="fa-solid fa-key"></i>
                {" How the Encryption Key Is Protected"}
            </h2>
            <p class="tc-section-desc">
                {"Your data is encrypted with a key that only exists inside sealed rooms. Here's how that key is managed:"}
            </p>

            <div class="chain">
                // Step A
                <div class="chain-card">
                    <div class="card-num-key">{"A"}</div>
                    <div class="card-body">
                        <h3>{"The build registers itself on a blockchain"}</h3>
                        <p class="card-explain">
                            {"During the GitHub Actions build (step 2 above), the workflow computes an Image ID from the fingerprint and registers it on a public blockchain (Arbitrum):"}
                        </p>
                        <div class="image-id-calc">
                            <div class="calc-row">
                                <span class="calc-label">{"PCR0"}</span>
                                <code class="calc-val">{short_hex(pcr0, 12)}</code>
                            </div>
                            <div class="calc-op">{"+"}</div>
                            <div class="calc-row">
                                <span class="calc-label">{"PCR1"}</span>
                                <code class="calc-val">{short_hex(d.pcr1.as_deref().unwrap_or("?"), 12)}</code>
                            </div>
                            <div class="calc-op">{"+"}</div>
                            <div class="calc-row">
                                <span class="calc-label">{"PCR2"}</span>
                                <code class="calc-val">{short_hex(d.pcr2.as_deref().unwrap_or("?"), 12)}</code>
                            </div>
                            <div class="calc-op calc-eq">{"= SHA256 ="}</div>
                            <div class="calc-row calc-result">
                                <span class="calc-label">{"Image ID"}</span>
                                <code class="calc-val">{short_hex(image_id, 16)}</code>
                            </div>
                        </div>
                        <div class="card-links">
                            if let Some(ref url) = actions_url {
                                <a href={url.clone()} target="_blank" rel="noopener noreferrer">
                                    {"Open the build workflow"}<span class="link-hint">{" - search for \"Approve image for Marlin KMS\" to see this step"}</span>
                                    {" "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                                </a>
                            }
                        </div>
                    </div>
                </div>

                <div class="chain-arrow">
                    <div class="arrow-line"></div>
                    <div class="arrow-label">{"Registered on a public smart contract"}</div>
                    <div class="arrow-head"><i class="fa-solid fa-arrow-down"></i></div>
                </div>

                // Step B
                <div class="chain-card">
                    <div class="card-num-key">{"B"}</div>
                    <div class="card-body">
                        <h3>{"Anyone can check the smart contract"}</h3>
                        <p class="card-explain">
                            {"The smart contract is verified on Arbiscan - you can read its source code and check which builds are approved. The key function is:"}
                        </p>
                        <pre class="code-block">{format!("approvedImages[{}] = {}", short_hex(image_id, 16), if approved { "true" } else { "?" })}</pre>
                        if let Some(ref b) = bc {
                            if let Some(ref tx) = b.propose_tx {
                                <div class="card-values">
                                    <div class="val-row">
                                        <span class="val-label">{"Proposed"}</span>
                                        <a href={format!("https://arbiscan.io/tx/{}", tx)} target="_blank" rel="noopener noreferrer" class="val-data val-link">
                                            {short_hex(tx, 12)}{" "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                                        </a>
                                        if let Some(ref ts) = b.propose_timestamp {
                                            <span class="val-time" title={format_ts(ts)}>{relative_time(ts)}</span>
                                        }
                                    </div>
                                </div>
                            }
                        }
                        <p class="card-explain card-explain-howto">
                            <strong>{"Try it yourself: "}</strong>
                            {"Go to Arbiscan (link below), click \"Read Contract\", find "}<code>{"approvedImages"}</code>
                            {", paste the Image ID above, and you'll see "}<code>{"true"}</code>{"."}
                        </p>
                        <div class="card-links">
                            if !contract_addr.is_empty() {
                                <a href={format!("https://arbiscan.io/address/{}#readContract", contract_addr)} target="_blank" rel="noopener noreferrer">
                                    {"Open \"Read Contract\" on Arbiscan"}<span class="link-hint">{" - check approvedImages yourself"}</span>
                                    {" "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                                </a>
                                <a href={format!("https://arbiscan.io/address/{}#code", contract_addr)} target="_blank" rel="noopener noreferrer">
                                    {"Read the verified source code"}<span class="link-hint">{" - Arbiscan confirms it matches the deployed bytecode"}</span>
                                    {" "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                                </a>
                                <a href={format!("https://arbiscan.io/address/{}#events", contract_addr)} target="_blank" rel="noopener noreferrer">
                                    {"View all approved builds (Events)"}<span class="link-hint">{" - every ImageProposed and ImageActivated event"}</span>
                                    {" "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                                </a>
                            }
                        </div>
                    </div>
                </div>

                <div class="chain-arrow">
                    <div class="arrow-line"></div>
                    <div class="arrow-label">{"Marlin (independent key guardian) checks this same contract"}</div>
                    <div class="arrow-head"><i class="fa-solid fa-arrow-down"></i></div>
                </div>

                // Step C
                <div class="chain-card chain-card-marlin">
                    <div class="card-num-key">{"C"}</div>
                    <div class="card-body">
                        <h3>{"Marlin verifies independently"}</h3>
                        <p class="card-explain">
                            {"Marlin holds the encryption key. It does not trust us. Before releasing the key, it does exactly what you just did:"}
                        </p>

                        <div class="substep-flow">
                            <div class="substep">
                                <div class="substep-num">{"1"}</div>
                                <div class="substep-body">
                                    {"Asks the enclave for Amazon's signed proof (we can't forge this)"}
                                </div>
                            </div>
                            <div class="substep">
                                <div class="substep-num">{"2"}</div>
                                <div class="substep-body">
                                    {"Reads the fingerprint from the proof, computes the Image ID"}
                                </div>
                            </div>
                            <div class="substep">
                                <div class="substep-num">{"3"}</div>
                                <div class="substep-body">
                                    {"Calls "}<code>{"approvedImages[imageId]"}</code>{" on the same contract you checked above"}
                                </div>
                            </div>
                        </div>

                        <p class="card-explain">
                            {"If the contract says "}<code>{"true"}</code>{", Marlin releases the key."}
                        </p>

                        <div class="card-links">
                            <a href="https://github.com/marlinprotocol/oyster-monorepo" target="_blank" rel="noopener noreferrer">
                                {"Marlin's source code (open source)"}{" "}<i class="fa-solid fa-arrow-up-right-from-square"></i>
                            </a>
                        </div>
                        <p class="card-note">
                            {"Marlin runs in its own sealed computer. We cannot influence its decision."}
                        </p>
                    </div>
                </div>

                <div class="chain-arrow">
                    <div class="arrow-line"></div>
                    <div class="arrow-label">{"Key released directly into the sealed room"}</div>
                    <div class="arrow-head"><i class="fa-solid fa-arrow-down"></i></div>
                </div>

                // Step D
                <div class="chain-card chain-card-final">
                    <div class="card-num-key">{"D"}</div>
                    <div class="card-body">
                        <h3>{"The key never leaves the sealed room"}</h3>
                        <p class="card-explain">
                            {"The key goes directly into the sealed room. We don't just choose not to see it - we literally cannot. "}
                            {"There is no mechanism for the key to exist outside of a sealed room."}
                        </p>
                        <p class="card-explain">
                            {"When we release a new version, the same process repeats: the new sealed room proves itself, and the key moves from one sealed room to the next. It is never exposed."}
                        </p>
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
    padding: 5rem 1.5rem 3rem;
    color: #fff;
}
.tc-header { text-align: center; margin-bottom: 2rem; }
.tc-header h1 { font-size: 1.8rem; font-weight: 700; margin: 0 0 0.5rem; }
.tc-subtitle { color: rgba(255,255,255,0.55); font-size: 0.95rem; line-height: 1.5; margin: 0; }
.tc-loading { text-align: center; padding: 3rem; color: rgba(255,255,255,0.4); }

/* Sections */
.tc-section { margin-bottom: 2.5rem; }
.tc-section-key { padding-top: 2rem; border-top: 1px solid rgba(255,255,255,0.1); }
.tc-section-title {
    font-size: 1.2rem; font-weight: 600; margin: 0 0 0.4rem;
    display: flex; align-items: center; gap: 0.5rem;
}
.tc-section-title i { color: var(--color-accent, #1E90FF); font-size: 1rem; }
.tc-section-desc { color: rgba(255,255,255,0.5); font-size: 0.9rem; line-height: 1.5; margin: 0 0 1.25rem; }

.card-num-key {
    width: 28px; height: 28px;
    border-radius: 50%;
    background: rgba(156,39,176,0.15);
    border: 1.5px solid rgba(156,39,176,0.4);
    color: #CE93D8;
    display: flex; align-items: center; justify-content: center;
    font-size: 0.75rem; font-weight: 700;
    flex-shrink: 0;
    margin-top: 2px;
}

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
.card-time { font-size: 0.8rem; color: rgba(255,255,255,0.35); margin: 0.3rem 0 0.5rem; }
.card-explain-match { color: #4CAF50; font-size: 0.85rem; }
.card-explain-match i { margin-right: 0.2rem; }
.card-explain-howto {
    background: rgba(30,144,255,0.06); border: 1px solid rgba(30,144,255,0.15);
    border-radius: 6px; padding: 0.6rem 0.8rem; font-size: 0.82rem;
}
.card-explain-howto code { background: rgba(255,255,255,0.1); padding: 0.1rem 0.3rem; border-radius: 3px; font-size: 0.78rem; }

/* Code blocks */
.code-block {
    background: rgba(0,0,0,0.3); border: 1px solid rgba(255,255,255,0.08);
    padding: 0.6rem 0.8rem; border-radius: 6px; font-size: 0.78rem;
    color: rgba(255,255,255,0.7); overflow-x: auto; white-space: pre-wrap;
    word-break: break-all; font-family: monospace; margin: 0.5rem 0;
}
.code-highlight { color: #4CAF50; border-color: rgba(76,175,80,0.2); }

/* Link hints */
.link-hint { color: rgba(255,255,255,0.35); font-size: 0.78rem; }

/* Image ID calculation visual */
.image-id-calc {
    background: rgba(0,0,0,0.25); border: 1px solid rgba(255,255,255,0.08);
    border-radius: 8px; padding: 0.75rem; margin: 0.5rem 0;
    display: flex; flex-direction: column; align-items: center; gap: 0.2rem;
}
.calc-row { display: flex; align-items: center; gap: 0.5rem; width: 100%; }
.calc-label { font-size: 0.72rem; color: rgba(255,255,255,0.4); min-width: 40px; text-align: right; }
.calc-val { font-family: monospace; font-size: 0.78rem; color: rgba(255,255,255,0.7); }
.calc-op { color: rgba(255,255,255,0.3); font-size: 0.8rem; font-weight: 600; }
.calc-eq { color: rgba(30,144,255,0.6); font-size: 0.72rem; margin: 0.15rem 0; }
.calc-result .calc-val { color: #4CAF50; font-weight: 500; }
.calc-result .calc-label { color: rgba(76,175,80,0.7); }

/* Marlin card */
.chain-card-marlin {
    border-color: rgba(156,39,176,0.25);
    background: rgba(156,39,176,0.03);
}

/* Substep flow inside Marlin card */
.substep-flow { margin: 0.5rem 0 0.75rem; display: flex; flex-direction: column; gap: 0.4rem; }
.substep {
    display: flex; gap: 0.5rem; align-items: baseline;
    font-size: 0.82rem; color: rgba(255,255,255,0.6); line-height: 1.4;
}
.substep code { background: rgba(255,255,255,0.08); padding: 0.1rem 0.3rem; border-radius: 3px; font-size: 0.75rem; }
.substep-num {
    width: 20px; height: 20px; border-radius: 50%;
    background: rgba(156,39,176,0.15); border: 1px solid rgba(156,39,176,0.3);
    color: #CE93D8; display: flex; align-items: center; justify-content: center;
    font-size: 0.65rem; font-weight: 700; flex-shrink: 0;
}
.substep-body { flex: 1; }

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
