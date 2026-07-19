use crate::utils::seo::{use_seo, SeoMeta};
use crate::Route;
use yew::prelude::*;
use yew_router::prelude::*;

#[function_component(TrustlessVerification)]
pub fn trustless_verification() -> Html {
    use_seo(SeoMeta {
        title: "Privacy by Design, Open to Verification - Lightfriend",
        description:
            "Lightfriend is designed so no one else can see your chats or personal data, including during AI processing. Open code and cryptographic evidence make production independently verifiable.",
        canonical: "https://lightfriend.ai/trustless",
        og_type: "website",
    });

    html! {
        <div class="legal-content trustless-page">
            <h1>{"Designed for Privacy. Open to Verification."}</h1>
            <p class="trustless-subtitle">{"Designed so no one else can see your chats or personal data, including while AI processes them."}</p>

            <section>
                <h2>{"The Problem With Most Apps"}</h2>
                <p>{"Most cloud applications ask users to trust operational policies that are difficult to inspect from the outside."}</p>
                <p>{"Lightfriend's entire architecture is designed around that privacy goal. All of the code is open source, the production deployment exposes cryptographic verification evidence, and each control below can be inspected independently."}</p>
            </section>

            <section>
                <h2>{"Step 1: Hardware-Isolated Processing"}</h2>
                <p>{"Lightfriend's production application runs inside an AWS Nitro Enclave, an isolated virtual machine with a deliberately restricted interface to its parent server."}</p>
                <p>{"The deployed enclave configuration has these properties:"}</p>
                <ul>
                    <li>{"It exposes no SSH login, interactive shell, or administrative endpoint."}</li>
                    <li>{"The parent EC2 instance is isolated from the enclave's CPU and memory."}</li>
                    <li>{"It has no persistent storage or direct external networking; communication uses the explicitly implemented enclave channel."}</li>
                </ul>
                <p>{"Lightfriend processes messages inside the enclave. Before application data is written to storage outside the enclave, the application encrypts it with AES-256-GCM."}</p>
                <p>{"The database outside the enclave stores ciphertext. Decryption keys are requested from an independently operated key service using the enclave's attestation evidence."}</p>
            </section>

            <section>
                <h2>{"Step 2: Proving What Code Is Running"}</h2>
                <p>{"A sealed room is great, but how do you know we actually put the right code in there? What if we secretly put in code that leaks your data?"}</p>
                <p>{"Here's how you can check:"}</p>
                <ol>
                    <li>{"Lightfriend's source code is public on GitHub and can be inspected or built independently."}</li>
                    <li>{"When we build the app, an automated system (GitHub Actions) creates the sealed room's package and publishes a unique fingerprint (PCR values) of exactly what's inside. Think of this fingerprint like a DNA test for the code - if even one tiny thing changes, the fingerprint is completely different."}</li>
                    <li>{"The enclave returns an AWS-signed attestation document containing the measurement of its running image. The signature can be validated against the AWS Nitro trust chain."}</li>
                    <li>{"Anyone can compare the measurement reported by the live enclave with the measurement produced by the public build."}</li>
                </ol>
            </section>

            <section>
                <h2>{"Step 3: Updating Code Without Exposing Data"}</h2>
                <p>{"Software needs updates - bug fixes, new features, security patches. The key-release process therefore needs a way to evaluate each new enclave image before it can decrypt existing data."}</p>
                <p>{"Lightfriend does not provision the master encryption key directly. An independently operated service evaluates attestation evidence before releasing key material to an enclave."}</p>

                <h3>{"How it works:"}</h3>
                <ol>
                    <li><strong>{"A public approval list (Smart Contract): "}</strong>{"Approved Lightfriend image measurements are published to an Arbitrum smart contract. Additions and removals appear as public on-chain transactions."}</li>
                    <li><strong>{"A key guardian (Marlin Key Service): "}</strong>{"An independently operated service holds the master key. Its release policy evaluates two inputs:"}
                        <ul>
                            <li>{"An AWS-signed Nitro attestation document"}</li>
                            <li>{"The reported image measurement's status in the public approval list"}</li>
                        </ul>
                    </li>
                </ol>

                <h3>{"So when we release an update:"}</h3>
                <ol>
                    <li>{"We build the new version publicly - everyone can see the code and the fingerprint."}</li>
                    <li>{"We add the new fingerprint to the public approval list."}</li>
                    <li>{"The new sealed room starts up and asks the key guardian for the key."}</li>
                    <li>{"The key guardian checks: \"Is this a real sealed room? Is its fingerprint on the approved list?\" If both yes, it hands over the key."}</li>
                    <li>{"The new sealed room uses the key to decrypt (unscramble) the stored data, and continues running."}</li>
                </ol>
                <p>{"The implemented key path runs from the attested Marlin key service to enclave memory. The Lightfriend operator does not manually provision or handle the master key."}</p>
            </section>

            <section>
                <h2>{"What About Other Services?"}</h2>
                <p>{"Lightfriend doesn't do everything alone - it talks to a few outside services. Here's how each one is handled:"}</p>
                <ul>
                    <li><strong>{"AI (Tinfoil): "}</strong>{"When Lightfriend runs AI models, it sends requests to "}<a href="https://tinfoil.sh" target="_blank" rel="noopener noreferrer">{"Tinfoil"}</a>{", which publishes source code and attestation evidence for its confidential-computing inference environment."}</li>
                    <li><strong>{"Optional voice calls (OpenAI Realtime): "}</strong>{"Lightfriend currently sends call audio and transcripts to OpenAI Realtime to provide a faster, more natural voice experience. This processing happens outside Lightfriend's independently verifiable trust chain. "}<a href="https://developers.openai.com/api/docs/guides/your-data" target="_blank" rel="noopener noreferrer">{"OpenAI's published API data controls"}</a>{" state that API data is not used to train its models unless the customer opts in, but Realtime customer content may be retained in abuse-monitoring logs for up to 30 days by default. OpenAI controls that environment and access to retained logs; Lightfriend cannot independently verify or technically prevent access there. Voice calls are optional. Lightfriend will switch as soon as a suitable open-source, attested voice alternative can provide a comparable experience."}</li>
                    <li><strong>{"SMS (Twilio): "}</strong>{"Twilio carries text messages back and forth, but Lightfriend's code is designed to automatically delete message bodies from Twilio's logs as soon as each message is delivered."}</li>
                </ul>
            </section>

            <section>
                <h2>{"What You Still Have to Trust"}</h2>
                <p>{"We want to be honest about what's verifiable and what's not."}</p>
                <ul>
                    <li><strong>{"Amazon (AWS Nitro Enclaves): "}</strong>{"You trust that their sealed room technology actually prevents peeking inside. Amazon's entire cloud business depends on this security working - if Nitro Enclaves were broken, it would affect thousands of companies using them, not just Lightfriend."}</li>
                    <li><strong>{"Marlin (Key Guardian): "}</strong>{"You trust that Marlin honestly checks proofs before handing out keys. But Marlin's key service itself runs inside its own sealed room (Nitro Enclave) with its own attestation - so Marlin's behavior is verifiable the same way ours is. Their code is also "}<a href="https://github.com/marlinprotocol/oyster-monorepo" target="_blank" rel="noopener noreferrer">{"open source"}</a>{"."}</li>
                </ul>
            </section>

            <section class="trustless-live-link">
                <h2>{"See the Live Trust Chain"}</h2>
                <p>{"Want to see the actual verification data for the deployment running right now? Our Trust Chain page shows live timestamps, links, and status for every step."}</p>
                <p><Link<Route> to={Route::TrustChain}>{"View Live Trust Chain"}</Link<Route>></p>
            </section>

            <section>
                <h2>{"Verify It Yourself"}</h2>
                <p>{"You don't have to take our word for any of this. We provide a verification tool that anyone can run:"}</p>
                <pre><code>{"./scripts/verify_live_attestation.sh https://lightfriend.ai --rpc-url https://arb1.arbitrum.io/rpc"}</code></pre>
                <p>{"This tool:"}</p>
                <ul>
                    <li>{"Requests a live attestation document bound to a fresh random challenge."}</li>
                    <li>{"Validates the attestation signature against AWS Nitro's trust chain."}</li>
                    <li>{"Compares the code fingerprint against the public build to confirm it matches the code on GitHub."}</li>
                    <li>{"Checks the public approval list (smart contract) to confirm this version is approved."}</li>
                </ul>
                <p>{"If all checks pass, the reported live measurement matches the published build and appears on the public approval list. Attestation verifies deployment identity; it does not prove that the software is free of bugs."}</p>
            </section>

            <section>
                <h2>{"Source Code and Endpoints"}</h2>
                <p>
                    {"Everything is open source. Read every line, run the verification, build it yourself: "}
                    <a href="https://github.com/ahtavarasmus/lightfriend" target="_blank" rel="noopener noreferrer">{"Lightfriend on GitHub"}</a>
                    {"."}
                </p>
                <p>{"For developers who want to build their own verification tools:"}</p>
                <ul>
                    <li><code>{"/.well-known/lightfriend/attestation"}</code>{" - live metadata: commit hash, code fingerprint (PCR values), build ID, contract address."}</li>
                    <li><code>{"/.well-known/lightfriend/attestation/raw"}</code>{" - raw signed proof from the sealed room (Nitro Attestation document)."}</li>
                    <li><code>{"/.well-known/lightfriend/attestation/hex"}</code>{" - same proof in hex format."}</li>
                </ul>
            </section>

            <section>
                <p style="font-size: 0.85rem; color: #777;">{"This page describes how the system is designed and intended to work. It is not a warranty or guarantee. No system is perfectly secure, and the properties described here depend on third-party infrastructure (AWS, Marlin, and others) functioning correctly. See our "}<Link<Route> to={Route::Privacy}>{"Privacy Policy"}</Link<Route>>{" for full disclaimers."}</p>
            </section>

            <div class="legal-links">
                <Link<Route> to={Route::Home}>{"Home"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Terms}>{"Terms & Conditions"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Privacy}>{"Privacy Policy"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::TrustChain}>{"Trust Chain"}</Link<Route>>
            </div>
        </div>
    }
}
