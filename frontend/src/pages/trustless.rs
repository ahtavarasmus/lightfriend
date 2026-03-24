use crate::utils::seo::{use_seo, SeoMeta};
use crate::Route;
use yew::prelude::*;
use yew_router::prelude::*;

#[function_component(TrustlessVerification)]
pub fn trustless_verification() -> Html {
    use_seo(SeoMeta {
        title: "Verifiably Private - Lightfriend",
        description: "How Lightfriend keeps your data private - even from us - and how anyone can verify it.",
        canonical: "https://lightfriend.ai/trustless",
        og_type: "website",
    });

    html! {
        <div class="legal-content trustless-page">
            <h1>{"Verifiably Private"}</h1>
            <p class="trustless-subtitle">{"How Lightfriend keeps your data private - even from us."}</p>

            <section>
                <h2>{"The Problem With Most Apps"}</h2>
                <p>{"When you use most apps, the people who built and run them can see your data. Your messages, your files, your personal info - it all sits on their servers, and you just have to trust that they won't look at it. That's a promise, not a guarantee."}</p>
                <p>{"Lightfriend is different. We designed it so that even we can't access your data, even if we wanted to. And you don't have to take our word for it - anyone can verify this, at any time."}</p>
            </section>

            <section>
                <h2>{"Step 1: A Sealed Room Nobody Can Enter"}</h2>
                <p>{"Imagine a room with no doors, no windows, and no way to peek inside. You can put things into the room through a tiny slot, and get results back out through another slot, but nobody - not even the building owner - can go inside and look around."}</p>
                <p>{"That's basically what a secure sealed computer (AWS Nitro Enclave) is. It's a special isolated computer that runs inside Amazon's cloud. Once it starts up:"}</p>
                <ul>
                    <li>{"Nobody can log into it or connect to it - not Amazon, not us, not anyone."}</li>
                    <li>{"Nobody can peek at what's in its memory."}</li>
                    <li>{"It has no internet access, no way to save files outside itself, and no admin backdoor."}</li>
                </ul>
                <p>{"Your data lives inside this sealed room. Lightfriend's code runs in there, processes your messages, and sends back answers - but nobody on the outside can see what's happening inside."}</p>
                <p>{"But data needs to be stored somewhere permanent - sealed rooms lose their memory when they restart. So here's what Lightfriend does: inside the sealed room, it encrypts (scrambles) all your data using a secret key that only exists inside the room. Then it passes the encrypted data out through the tiny slot to be stored. The data sitting outside is just scrambled nonsense - nobody can read it because the key to unscramble it only lives inside the sealed room."}</p>
            </section>

            <section>
                <h2>{"Step 2: Proving What Code Is Running"}</h2>
                <p>{"A sealed room is great, but how do you know we actually put the right code in there? What if we secretly put in code that leaks your data?"}</p>
                <p>{"Here's how you can check:"}</p>
                <ol>
                    <li>{"All of Lightfriend's code is public on GitHub. Anyone can read every single line."}</li>
                    <li>{"When we build the app, an automated system (GitHub Actions) creates the sealed room's package and publishes a unique fingerprint (PCR values) of exactly what's inside. Think of this fingerprint like a DNA test for the code - if even one tiny thing changes, the fingerprint is completely different."}</li>
                    <li>{"The sealed room can produce a signed certificate from Amazon that says \"here is the fingerprint of the code running inside me right now\" (Nitro Attestation). Amazon signs this - we can't fake it."}</li>
                    <li>{"Anyone can compare: does the fingerprint from the live sealed room match the fingerprint from the public build? If yes, you know the exact same code from GitHub is what's actually running."}</li>
                </ol>
            </section>

            <section>
                <h2>{"Step 3: Updating Code Without Exposing Data"}</h2>
                <p>{"Software needs updates - bug fixes, new features, security patches. But if the encryption key only lives inside a sealed room, how does a new version of the room get the key so it can read the existing data?"}</p>
                <p>{"If we (the developers) held the key, we could read your data anytime. That defeats the whole purpose. So instead, we use a system where the key is managed by an independent guardian, and only handed to approved sealed rooms. Nobody else ever sees it."}</p>

                <h3>{"How it works:"}</h3>
                <ol>
                    <li><strong>{"A public approval list (Smart Contract): "}</strong>{"We publish a list on a public record book (blockchain) that says \"these specific code fingerprints are approved versions of Lightfriend.\" Anyone can see this list - it's completely public and can't be secretly changed."}</li>
                    <li><strong>{"A key guardian (Marlin Key Service): "}</strong>{"There's an independent service that holds the master key. It will only give the key to a sealed room that can prove two things:"}
                        <ul>
                            <li>{"\"I am a real sealed room\" - proven by Amazon's signed certificate (Nitro Attestation)"}</li>
                            <li>{"\"I am running approved code\" - proven by checking the fingerprint against the public approval list (Smart Contract)"}</li>
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
                <p>{"At no point during this process does the key exist outside of an approved sealed room. It goes directly from the key guardian into the sealed room's memory. We never see it. We never touch it."}</p>
            </section>

            <section>
                <h2>{"What About Other Services?"}</h2>
                <p>{"Lightfriend doesn't do everything alone - it talks to a few outside services. Here's how each one is handled:"}</p>
                <ul>
                    <li><strong>{"AI (Tinfoil): "}</strong>{"When Lightfriend needs to think (run AI models), it sends requests to "}<a href="https://tinfoil.sh" target="_blank" rel="noopener noreferrer">{"Tinfoil"}</a>{", which runs AI workloads inside the same kind of sealed rooms (TEEs) with the same cryptographic guarantees as Nitro Enclaves. Verifiable, not trust-based."}</li>
                    <li><strong>{"SMS (Twilio): "}</strong>{"Twilio carries text messages back and forth, but Lightfriend's code is designed to automatically delete message bodies from Twilio's logs as soon as each message is delivered."}</li>
                    <li><strong>{"Voice calls (ElevenLabs): "}</strong>{"This is the one area where we currently rely on trust. ElevenLabs handles voice call audio and does not provide cryptographic privacy guarantees. We are moving voice calls in-house as soon as Tinfoil provides a verifiable text-to-speech model, which will bring voice calls under the same sealed room protections as everything else."}</li>
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

            <section>
                <h2>{"Verify It Yourself"}</h2>
                <p>{"You don't have to take our word for any of this. We provide a verification tool that anyone can run:"}</p>
                <pre><code>{"./scripts/verify_live_attestation.sh https://lightfriend.ai --rpc-url https://arb1.arbitrum.io/rpc"}</code></pre>
                <p>{"This tool:"}</p>
                <ul>
                    <li>{"Asks the live sealed room to prove what code it's running (with a fresh random challenge so it can't replay an old proof)."}</li>
                    <li>{"Verifies that Amazon actually signed the proof (cryptographic verification against AWS Nitro's trust chain)."}</li>
                    <li>{"Compares the code fingerprint against the public build to confirm it matches the code on GitHub."}</li>
                    <li>{"Checks the public approval list (smart contract) to confirm this version is approved."}</li>
                </ul>
                <p>{"If all checks pass, you know for a fact that the code on GitHub is what's running, and that only this code has access to user data."}</p>
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
            </div>
        </div>
    }
}
