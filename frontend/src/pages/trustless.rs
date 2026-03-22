use crate::utils::seo::{use_seo, SeoMeta};
use crate::Route;
use yew::prelude::*;
use yew_router::prelude::*;

#[function_component(TrustlessVerification)]
pub fn trustless_verification() -> Html {
    use_seo(SeoMeta {
        title: "Verifiably Private - Lightfriend",
        description: "How to verify that the running Lightfriend enclave matches the CI-built EIF, published PCRs, and approved KMS contract state.",
        canonical: "https://lightfriend.ai/trustless",
        og_type: "website",
    });

    html! {
        <div class="legal-content trustless-page">
            <h1>{"Verifiably Private"}</h1>

            <section>
                <h2>{"What You Can Verify"}</h2>
                <p>{"Lightfriend's production enclave is designed so that a third party can verify the live service against four independent pieces of evidence:"}</p>
                <ol>
                    <li>{"GitHub Actions built the EIF and published the PCR values for that build."}</li>
                    <li>{"The Marlin KMS contract approves the expected image measurement."}</li>
                    <li>{"The live enclave returns a Nitro attestation document signed by AWS Nitro's attestation chain."}</li>
                    <li>{"The live attestation matches the PCR values the build pipeline published."}</li>
                </ol>
            </section>

            <section>
                <h2>{"Architecture"}</h2>
                <p>{"The privacy and verification boundary is intentionally split across CI, chain state, and the enclave:"}</p>
                <ul>
                    <li>{"GitHub Actions builds the Docker image, builds the EIF from the immutable image digest, measures PCR0/1/2, and uploads the EIF artifact."}</li>
                    <li>{"The deployment pipeline proposes and activates the measured image in the permanent Lightfriend KMS verification contract."}</li>
                    <li>{"The runtime EC2 host does not build the EIF. It only downloads the CI-built EIF, verifies its hash, and runs it."}</li>
                    <li>{"The enclave derives the backup encryption key from Marlin only if the attested image is approved by the contract."}</li>
                    <li>{"The live app publishes attestation and metadata endpoints so anyone can verify the running enclave."}</li>
                </ul>
            </section>

            <section>
                <h2>{"Live Verification Endpoints"}</h2>
                <ul>
                    <li><code>{"/.well-known/lightfriend/attestation"}</code>{" returns the live enclave's claimed commit, EIF hash, PCR values, workflow run id, KMS contract address, and a public GitHub-hosted build metadata URL for the current commit."}</li>
                    <li><code>{"/.well-known/lightfriend/attestation/raw"}</code>{" returns the raw attestation document from the live enclave."}</li>
                    <li><code>{"/.well-known/lightfriend/attestation/hex"}</code>{" returns the same attestation as hex."}</li>
                </ul>
                <p>{"For freshness, verification should use a random challenge in the attestation's user_data field rather than trusting a static cached attestation blob."}</p>
            </section>

            <section>
                <h2>{"Verifier Script"}</h2>
                <p>{"This repo includes a verifier wrapper and a small Rust CLI so anyone can run the check themselves:"}</p>
                <pre><code>{"./scripts/verify_live_attestation.sh https://lightfriend.ai --rpc-url https://arb1.arbitrum.io/rpc"}</code></pre>
                <p>{"On first run the wrapper builds a tiny verifier binary locally with Cargo and the pinned Marlin Oyster SDK dependency."}</p>
                <p>{"What the verifier does:"}</p>
                <ul>
                    <li>{"Fetches the live metadata endpoint."}</li>
                    <li>{"Generates a fresh random challenge and requests a live attestation document with that challenge embedded in user_data."}</li>
                    <li>{"Cryptographically verifies the attestation document against AWS Nitro's root public key."}</li>
                    <li>{"Checks that the challenge came back in the signed attestation payload, which prevents replay of an old attestation."}</li>
                    <li>{"Fetches the public GitHub-hosted build metadata JSON for the current commit and compares commit, workflow run id, EIF hash, and PCR values against the live metadata endpoint."}</li>
                    <li>{"Checks that the attested PCR0/1/2 values match the live metadata endpoint."}</li>
                    <li>{"Optionally checks on-chain that the KMS contract currently approves the attested image."}</li>
                </ul>
            </section>

            <section>
                <h2>{"What This Does Not Claim"}</h2>
                <p>{"The metadata endpoint by itself is not authoritative. A malicious operator could lie in that JSON. The real proof is the signed attestation document returned by the enclave plus the on-chain contract approval and the CI-published build metadata."}</p>
                <p>{"In other words: do not trust the metadata endpoint alone. Trust the attestation verification result."}</p>
            </section>

            <section>
                <h2>{"Source"}</h2>
                <p>
                    {"The full implementation is open source in the "}
                    <a href="https://github.com/ahtavarasmus/lightfriend" target="_blank" rel="noopener noreferrer">{"Lightfriend repository"}</a>
                    {"."}
                </p>
            </section>

            <div class="legal-links">
                <Link<Route> to={Route::Home}>{"Home"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Terms}>{"Terms & Conditions"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Privacy}>{"Privacy Policy"}</Link<Route>>
            </div>
        </div>
    }
}
