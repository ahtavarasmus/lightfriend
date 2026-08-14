use crate::utils::api::Api;
use crate::utils::seo::{use_seo, SeoMeta};
use serde::Deserialize;
use yew::prelude::*;

#[derive(Clone, Copy, PartialEq)]
pub enum PublicGuideKind {
    SupportedCountries,
    CompatiblePhones,
    HowItWorks,
    Limitations,
    PrivacyArchitecture,
    AiAssistantBySms,
    EmailOnDumbphone,
    WhatsappOnDumbphone,
    McpIntegrations,
}

#[derive(Properties, PartialEq)]
pub struct PublicGuideProps {
    pub kind: PublicGuideKind,
}

#[derive(Clone, Deserialize, PartialEq)]
struct CountryInfo {
    country_code: String,
    country_name: String,
}

#[derive(Clone, Deserialize, PartialEq)]
struct CountriesResponse {
    local_number_countries: Vec<CountryInfo>,
    notification_only_countries: Vec<CountryInfo>,
}

fn guide_meta(kind: PublicGuideKind) -> SeoMeta {
    match kind {
        PublicGuideKind::SupportedCountries => SeoMeta {
            title: "Supported Countries for Lightfriend SMS and Voice",
            description: "Check where Lightfriend offers local numbers, notification-only service, or bring-your-own Twilio support.",
            canonical: "https://lightfriend.ai/supported-countries",
            og_type: "website",
        },
        PublicGuideKind::CompatiblePhones => SeoMeta {
            title: "Lightfriend Works With Any Phone That Can Text or Call",
            description: "Lightfriend runs outside the handset and works through ordinary SMS and calls. No app, mobile data, browser, or particular phone brand is required.",
            canonical: "https://lightfriend.ai/compatible-phones",
            og_type: "article",
        },
        PublicGuideKind::HowItWorks => SeoMeta {
            title: "How Lightfriend Works on a Dumbphone",
            description: "See how Lightfriend connects WhatsApp, Signal, Telegram, and email to ordinary SMS and phone calls without installing another app.",
            canonical: "https://lightfriend.ai/how-it-works",
            og_type: "article",
        },
        PublicGuideKind::Limitations => SeoMeta {
            title: "Lightfriend Limitations: What It Can and Cannot Do",
            description: "An honest list of Lightfriend limitations covering setup devices, media, delivery, AI classification, carriers, privacy, and provider availability.",
            canonical: "https://lightfriend.ai/limitations",
            og_type: "article",
        },
        PublicGuideKind::PrivacyArchitecture => SeoMeta {
            title: "Lightfriend Privacy Architecture",
            description: "How Lightfriend combines an AWS Nitro Enclave, encrypted storage, remote attestation, reproducible builds, and verifiable AI inference.",
            canonical: "https://lightfriend.ai/privacy-architecture",
            og_type: "article",
        },
        PublicGuideKind::AiAssistantBySms => SeoMeta {
            title: "AI Assistant by SMS for Any Phone",
            description: "Use an AI assistant through ordinary SMS and calls to search, check messages, manage email, and receive focused alerts on a dumbphone.",
            canonical: "https://lightfriend.ai/ai-assistant-by-sms",
            og_type: "article",
        },
        PublicGuideKind::EmailOnDumbphone => SeoMeta {
            title: "Email on a Dumbphone: Read and Reply by SMS",
            description: "Connect email to Lightfriend and use ordinary text messages to check recent email, search senders, and request supported replies.",
            canonical: "https://lightfriend.ai/email-on-dumbphone",
            og_type: "article",
        },
        PublicGuideKind::WhatsappOnDumbphone => SeoMeta {
            title: "WhatsApp on a Dumbphone: What Actually Works",
            description: "Compare native WhatsApp, linked devices, keeping a smartphone at home, and Lightfriend's text-first WhatsApp access over SMS.",
            canonical: "https://lightfriend.ai/whatsapp-on-dumbphone",
            og_type: "article",
        },
        PublicGuideKind::McpIntegrations => SeoMeta {
            title: "MCP Integrations for a Dumbphone AI Assistant",
            description: "Connect compatible Model Context Protocol servers to Lightfriend and make selected external tools available through SMS and calls.",
            canonical: "https://lightfriend.ai/mcp",
            og_type: "article",
        },
    }
}

fn guide_heading(kind: PublicGuideKind) -> (&'static str, &'static str, &'static str) {
    match kind {
        PublicGuideKind::SupportedCountries => (
            "Availability",
            "Where does Lightfriend work?",
            "There are three routes: a local Lightfriend number, notification-only service from a shared number, or an eligible Twilio number you bring yourself.",
        ),
        PublicGuideKind::CompatiblePhones => (
            "Any phone",
            "Choose the phone for how it feels to carry.",
            "Lightfriend runs in the service, not on the handset. Any phone with ordinary SMS or calls can be the interface; it does not need an app, mobile data, a browser, or a particular operating system.",
        ),
        PublicGuideKind::HowItWorks => (
            "How it works",
            "Your people and services, through one quiet thread.",
            "Lightfriend connects selected accounts on the web, evaluates what matters, and gives your everyday phone a compact SMS and call interface.",
        ),
        PublicGuideKind::Limitations => (
            "Limitations",
            "Useful, deliberately incomplete, and honest about it.",
            "Lightfriend does not turn a feature phone into a smartphone. It preserves selected communication and assistant functions while leaving feeds, apps, and much rich media behind.",
        ),
        PublicGuideKind::PrivacyArchitecture => (
            "Privacy architecture",
            "Designed to make production code independently inspectable.",
            "Open source is paired with hardware isolation, remote attestation, encrypted storage, reproducible builds, and public deployment evidence.",
        ),
        PublicGuideKind::AiAssistantBySms => (
            "AI by text",
            "An AI assistant without another app.",
            "Ask through an ordinary text message, receive a compact answer, and put the phone away. The limited interface is part of the design.",
        ),
        PublicGuideKind::EmailOnDumbphone => (
            "Email by SMS",
            "Check important email without carrying an inbox.",
            "Lightfriend can connect supported email accounts and let you ask about recent messages, search for a sender, and request supported replies through SMS.",
        ),
        PublicGuideKind::WhatsappOnDumbphone => (
            "WhatsApp without carrying a smartphone",
            "Text-first access, with real tradeoffs.",
            "Lightfriend can bridge selected WhatsApp text workflows to SMS, but pairing, recovery, and periodic linked-device maintenance still need a supported device with the native app.",
        ),
        PublicGuideKind::McpIntegrations => (
            "Model Context Protocol",
            "Bring compatible tools into the SMS assistant.",
            "Lightfriend can connect to user-provided MCP servers, discover their tools, and make enabled capabilities available to the assistant without putting another app on the phone.",
        ),
    }
}

fn country_content(countries: &UseStateHandle<Option<CountriesResponse>>) -> Html {
    let local_names = countries
        .as_ref()
        .map(|data| {
            data.local_number_countries
                .iter()
                .map(|country| country.country_name.clone())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| {
            "United States, Canada, Finland, Netherlands, United Kingdom, and Australia".to_string()
        });

    html! {
        <>
            <section class="answer-card answer-card-primary">
                <h2>{"Countries with local Lightfriend numbers"}</h2>
                <p>{local_names}</p>
                <p>{"Availability is checked during setup and can change with number inventory, carrier capabilities, destination permissions, and local regulation."}</p>
            </section>
            <section class="answer-grid">
                <article class="answer-card">
                    <h2>{"Notification-only countries"}</h2>
                    <p>{"Lightfriend supports many additional destinations using outbound notifications. These users may receive selected alerts and digests, but replying to the sending number is not the same as having a local two-way number."}</p>
                </article>
                <article class="answer-card">
                    <h2>{"Bring your own Twilio number"}</h2>
                    <p>{"An eligible Twilio number can provide another route. It needs the right voice and SMS capabilities, destination permissions, and regulatory documents. Twilio bills its usage separately."}</p>
                    <a href="/bring-own-number">{"Read the setup requirements →"}</a>
                </article>
            </section>
            if let Some(data) = countries.as_ref() {
                <details class="country-list">
                    <summary>{format!("See {} notification destinations", data.notification_only_countries.len())}</summary>
                    <ul>
                        {for data.notification_only_countries.iter().map(|country| html! {
                            <li>{format!("{} ({})", country.country_name, country.country_code)}</li>
                        })}
                    </ul>
                </details>
            }
        </>
    }
}

fn compatible_phones_content() -> Html {
    html! {
        <>
            <section class="answer-card answer-card-primary">
                <h2>{"The universal requirement"}</h2>
                <ul>
                    <li>{"Ordinary SMS for the complete text-first interface and focused alerts"}</li>
                    <li>{"Voice calling only if you want the optional call interface"}</li>
                    <li>{"A supported Lightfriend number route for your country and carrier"}</li>
                </ul>
                <p>{"That is the entire handset integration. The phone does not need Wi-Fi, mobile data, a browser, Android, iOS, KaiOS, or an app store."}</p>
                <p><a href="/can-i-leave-my-smartphone">{"Build a smartphone-exit plan for the services you need →"}</a></p>
            </section>
            <section class="answer-grid">
                <article class="answer-card"><h2>{"The phone is only the interface"}</h2><p>{"Connected clients, account monitoring, AI processing, and automations stay in Lightfriend. The carried handset sees an ordinary SMS conversation or call."}</p></article>
                <article class="answer-card"><h2>{"Your carrier still matters"}</h2><p>{"The phone must work on your carrier. Confirm regional bands, VoLTE, ordinary SMS behavior, and whether the manufacturer blocks calls or texts to assistant services."}</p></article>
                <article class="answer-card"><h2>{"Provider setup is separate"}</h2><p>{"WhatsApp and some other connected services can require their native app on a supported device for pairing, account recovery, and occasional maintenance. That does not change which phone you carry."}</p></article>
                <article class="answer-card"><h2>{"Why there are phone-specific guides"}</h2><p>{"People search by handset because the native app options differ. Lightfriend's answer remains the same: if the phone can text or call, the brand does not need its own Lightfriend integration."}</p></article>
            </section>
            <section class="answer-card"><h2>{"Examples—not a compatibility list"}</h2><p>{"Light Phone II and III, Punkt MP02, Mudita Kompakt, conventional Nokia feature phones, flip phones, candy-bar phones, and old smartphones used in a minimal way can all provide the ordinary SMS or call interface."}</p><p><a href="/light-phone-3-whatsapp-guide">{"Light Phone III"}</a>{" · "}<a href="/blog/punkt-mp02-whatsapp">{"Punkt MP02"}</a>{" · "}<a href="/blog/mudita-kompakt-whatsapp">{"Mudita Kompakt"}</a>{" · "}<a href="/blog/nokia-2780-whatsapp">{"Nokia 2780 Flip"}</a></p></section>
        </>
    }
}

fn how_it_works_content() -> Html {
    html! {
        <>
            <ol class="step-list">
                <li><strong>{"Choose a number route."}</strong><span>{"Use an available Lightfriend number or connect an eligible Twilio number."}</span></li>
                <li><strong>{"Connect selected services."}</strong><span>{"Pair the WhatsApp, Signal, Telegram, or email accounts you want Lightfriend to help with."}</span></li>
                <li><strong>{"Set interruption rules."}</strong><span>{"Choose focused alerts, scheduled digests, or on-demand checking rather than forwarding everything."}</span></li>
                <li><strong>{"Text or call."}</strong><span>{"Ask about recent conversations, request a compact summary, search for a sender, or use a supported action."}</span></li>
                <li><strong>{"Approve consequential actions."}</strong><span>{"Sending and other write actions pass through explicit application controls and can include a cancellation window."}</span></li>
            </ol>
            <section class="answer-card"><h2>{"What stays outside the phone"}</h2><p>{"The connected-service clients, encrypted storage, AI processing, and automation run in Lightfriend's service. Your handset sees a normal call or SMS conversation."}</p></section>
        </>
    }
}

fn limitations_content() -> Html {
    html! {
        <section class="answer-grid">
            <article class="answer-card"><h2>{"No full app replica"}</h2><p>{"Rich media, video calls, reactions, stickers, status features, and complex provider interfaces are not reproduced over SMS."}</p></article>
            <article class="answer-card"><h2>{"Setup devices remain necessary"}</h2><p>{"WhatsApp and some other connected services require their supported native app for initial pairing, recovery, or renewed authorization."}</p></article>
            <article class="answer-card"><h2>{"AI can be wrong"}</h2><p>{"Urgency classification, summaries, extraction, and generated responses are fallible. Do not rely on Lightfriend as the only channel for emergencies."}</p></article>
            <article class="answer-card"><h2>{"Delivery is not guaranteed"}</h2><p>{"Carriers, external providers, account sessions, number inventory, rate limits, outages, and destination rules can delay or reject requests."}</p></article>
            <article class="answer-card"><h2>{"SMS is not end-to-end encrypted"}</h2><p>{"The final call or SMS leg is processed by your carrier. Connected providers also process their side of each communication."}</p></article>
            <article class="answer-card"><h2>{"Voice has a separate trust boundary"}</h2><p>{"Optional voice calls currently use an external realtime AI service for latency and are not inside the same independently verifiable inference chain."}</p></article>
            <article class="answer-card"><h2>{"Usage varies"}</h2><p>{"AI processing, carrier fees, destinations, message volume, and call duration affect metered usage. Bring-your-own Twilio charges are separate."}</p></article>
            <article class="answer-card"><h2>{"Carrier and number routes still vary"}</h2><p>{"Lightfriend does not require a phone integration, but the handset and number still need working carrier service. VoLTE, SMS routing, shortcodes, and assistant-number restrictions can vary."}</p></article>
        </section>
    }
}

fn privacy_content() -> Html {
    html! {
        <>
            <section class="answer-grid">
                <article class="answer-card"><h2>{"Hardware isolation"}</h2><p>{"The production application runs inside an AWS Nitro Enclave without ordinary SSH access, persistent enclave disks, or direct external networking."}</p></article>
                <article class="answer-card"><h2>{"Remote attestation"}</h2><p>{"AWS hardware signs a document containing enclave measurements. Those measurements can be compared with public build and deployment evidence."}</p></article>
                <article class="answer-card"><h2>{"Independent key release"}</h2><p>{"The key-management path evaluates attestation before releasing encryption keys instead of relying on an operator manually provisioning the production master key."}</p></article>
                <article class="answer-card"><h2>{"Verifiable AI inference"}</h2><p>{"Text inference uses a confidential-computing provider that publishes source and attestation evidence for its inference environment."}</p></article>
            </section>
            <section class="answer-card answer-card-primary">
                <h2>{"What this proves—and what it does not"}</h2>
                <p>{"Attestation makes the reported deployment identity independently inspectable. It does not prove the code has no bugs, eliminate external providers, or make the cellular SMS leg end-to-end encrypted."}</p>
                <p><a href="/trustless">{"Read the complete architecture →"}</a>{" · "}<a href="/trust-chain">{"Inspect the live trust chain →"}</a></p>
            </section>
        </>
    }
}

fn sms_assistant_content() -> Html {
    html! {
        <>
            <section class="answer-card answer-card-primary"><h2>{"What you can ask"}</h2><p>{"Ask a concise question, search connected conversations, check what arrived, request a digest, look for an email from a sender, or initiate a supported reply. The exact result depends on connected services and current provider availability."}</p></section>
            <section class="answer-grid">
                <article class="answer-card"><h2>{"Why SMS"}</h2><p>{"It works on small phones, has no feed, and naturally ends after the answer. The constraint helps Lightfriend remain a tool rather than another place to spend time."}</p></article>
                <article class="answer-card"><h2>{"Why calls"}</h2><p>{"Voice provides a faster interface for longer questions when you choose to use it. It is optional and has a separate privacy boundary described on the limitations page."}</p></article>
                <article class="answer-card"><h2>{"Proactive without constant noise"}</h2><p>{"Focused alerts can interrupt for selected messages while routine items wait for a scheduled digest. Classification is useful but not infallible."}</p></article>
                <article class="answer-card"><h2>{"Actions remain constrained"}</h2><p>{"Lightfriend is read-only by default. Supported write actions pass through application controls rather than giving untrusted message content unrestricted access to tools."}</p></article>
            </section>
        </>
    }
}

fn email_content() -> Html {
    html! {
        <>
            <section class="answer-grid">
                <article class="answer-card"><h2>{"Check recent email"}</h2><p>{"Ask what arrived today, whether a particular sender wrote, or for a compact summary of unread messages."}</p></article>
                <article class="answer-card"><h2>{"Focused alerts and digests"}</h2><p>{"Selected time-sensitive email can trigger an SMS while routine messages wait for a scheduled digest."}</p></article>
                <article class="answer-card"><h2>{"Supported replies"}</h2><p>{"Request a reply in plain language and confirm the intended recipient and content. Provider delivery can still fail after Lightfriend queues the action."}</p></article>
                <article class="answer-card"><h2>{"What does not fit SMS"}</h2><p>{"Attachments, rich HTML layouts, complex threads, and full inbox administration remain better suited to a computer or the provider's native client."}</p></article>
            </section>
            <section class="answer-card answer-card-primary"><h2>{"Setup"}</h2><p>{"Connect a supported email account from the web dashboard, choose what may interrupt you, and keep account recovery available outside the dumbphone."}</p></section>
        </>
    }
}

fn whatsapp_content() -> Html {
    html! {
        <>
            <section class="answer-card answer-card-primary"><h2>{"The short answer"}</h2><p>{"Most true feature phones cannot run current native WhatsApp. Lightfriend does not install WhatsApp on the dumbphone; it gives the phone a text-first route to selected functions on a separately connected account."}</p></section>
            <section class="answer-grid">
                <article class="answer-card"><h2>{"Option 1: native app"}</h2><p>{"An Android-based minimalist phone may run the official app, but it also brings back an app platform and more smartphone-like behavior."}</p></article>
                <article class="answer-card"><h2>{"Option 2: smartphone at home"}</h2><p>{"Carry the simple phone and check WhatsApp deliberately on a separate device. This is simple and keeps the official client as the source of truth."}</p></article>
                <article class="answer-card"><h2>{"Option 3: Lightfriend"}</h2><p>{"Ask about recent messages, receive selected alerts or digests, search context, and request supported text replies over SMS."}</p></article>
                <article class="answer-card"><h2>{"What you still need"}</h2><p>{"Keep a supported device with the native WhatsApp app for pairing, periodic activity, recovery, and reauthorization. Linked access can expire."}</p></article>
            </section>
            <section class="answer-card"><h2>{"Compare specific phones"}</h2><p><a href="/light-phone-3-whatsapp-guide">{"Light Phone III"}</a>{" · "}<a href="/blog/punkt-mp02-whatsapp">{"Punkt MP02"}</a>{" · "}<a href="/blog/mudita-kompakt-whatsapp">{"Mudita Kompakt"}</a>{" · "}<a href="/blog/sunbeam-f1-pro-whatsapp">{"Sunbeam F1 Pro"}</a>{" · "}<a href="/blog/nokia-2780-whatsapp">{"Nokia 2780 Flip"}</a></p></section>
        </>
    }
}

fn mcp_content() -> Html {
    html! {
        <>
            <section class="answer-card answer-card-primary"><h2>{"What MCP adds"}</h2><p>{"Model Context Protocol provides a standard way for an AI client to discover tools exposed by another service. In Lightfriend, an enabled MCP server can extend what the assistant can do while the user still interacts through SMS or calls."}</p></section>
            <section class="answer-grid">
                <article class="answer-card"><h2>{"Connection flow"}</h2><p>{"Add a compatible server URL in the authenticated dashboard, test the connection, review the discovered tools, and enable the server only when its capabilities and operator are trusted."}</p></article>
                <article class="answer-card"><h2>{"No handset app"}</h2><p>{"MCP runs between Lightfriend and the external server. The carried phone remains a normal call-and-text interface and does not connect to the MCP server directly."}</p></article>
                <article class="answer-card"><h2>{"Trust boundary"}</h2><p>{"An MCP server is an external system with its own data handling, authentication, uptime, and security. Its tools are not automatically covered by Lightfriend's enclave or attestation claims."}</p></article>
                <article class="answer-card"><h2>{"Use the smallest scope"}</h2><p>{"Connect only servers you trust, expose only necessary tools, avoid placing secrets in prompts, and disable a server when it is no longer needed."}</p></article>
            </section>
            <section class="answer-card"><h2>{"Availability"}</h2><p>{"MCP configuration is available from the authenticated Lightfriend dashboard. Exact tool behavior depends on the server and its protocol compatibility; Lightfriend cannot guarantee third-party server availability or results."}</p></section>
        </>
    }
}

#[function_component(PublicGuide)]
pub fn public_guide(props: &PublicGuideProps) -> Html {
    let kind = props.kind;
    use_seo(guide_meta(kind));
    let countries = use_state(|| None::<CountriesResponse>);

    {
        let countries = countries.clone();
        use_effect_with_deps(
            move |kind| {
                if *kind == PublicGuideKind::SupportedCountries {
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Ok(response) = Api::get("/api/pricing/all-countries").send().await {
                            if response.ok() {
                                if let Ok(data) = response.json::<CountriesResponse>().await {
                                    countries.set(Some(data));
                                }
                            }
                        }
                    });
                }
                || ()
            },
            kind,
        );
    }

    let (eyebrow, title, introduction) = guide_heading(kind);
    let content = match kind {
        PublicGuideKind::SupportedCountries => country_content(&countries),
        PublicGuideKind::CompatiblePhones => compatible_phones_content(),
        PublicGuideKind::HowItWorks => how_it_works_content(),
        PublicGuideKind::Limitations => limitations_content(),
        PublicGuideKind::PrivacyArchitecture => privacy_content(),
        PublicGuideKind::AiAssistantBySms => sms_assistant_content(),
        PublicGuideKind::EmailOnDumbphone => email_content(),
        PublicGuideKind::WhatsappOnDumbphone => whatsapp_content(),
        PublicGuideKind::McpIntegrations => mcp_content(),
    };

    html! {
        <main class="public-guide-page">
            <header class="public-guide-hero">
                <a class="guide-logo" href="/">{"lightfriend"}</a>
                <p class="guide-eyebrow">{eyebrow}</p>
                <h1>{title}</h1>
                <p class="guide-intro">{introduction}</p>
                <div class="guide-actions">
                    <a class="guide-primary" href="/#plans">{"Start 7-day free trial"}</a>
                    <a class="guide-secondary" href="/limitations">{"Read the limitations"}</a>
                </div>
            </header>
            <div class="public-guide-content">
                {content}
                <nav class="guide-related" aria-label="Related Lightfriend guides">
                    <h2>{"Keep exploring"}</h2>
                    <div>
                        <a href="/how-it-works">{"How it works"}</a>
                        <a href="/can-i-leave-my-smartphone">{"Plan your switch"}</a>
                        <a href="/compatible-phones">{"Any phone"}</a>
                        <a href="/supported-countries">{"Supported countries"}</a>
                        <a href="/whatsapp-on-dumbphone">{"WhatsApp on dumbphone"}</a>
                        <a href="/email-on-dumbphone">{"Email on dumbphone"}</a>
                        <a href="/privacy-architecture">{"Privacy architecture"}</a>
                        <a href="/mcp">{"MCP integrations"}</a>
                    </div>
                </nav>
            </div>
            <style>{PUBLIC_GUIDE_CSS}</style>
        </main>
    }
}

const PUBLIC_GUIDE_CSS: &str = r#"
    .public-guide-page { min-height: 100vh; background: #101419; color: #f4f7fb; padding: 0 1.25rem 4rem; }
    .public-guide-hero { max-width: 920px; margin: 0 auto; padding: 7rem 0 4rem; text-align: left; }
    .guide-logo { color: #fff; font-weight: 700; font-size: 1.1rem; text-decoration: none; }
    .guide-eyebrow { margin: 4rem 0 1rem; color: #8dcdff; text-transform: uppercase; letter-spacing: .14em; font-size: .78rem; font-weight: 700; }
    .public-guide-hero h1 { max-width: 850px; margin: 0; color: #fff; font-size: clamp(2.7rem, 7vw, 5.5rem); line-height: .98; letter-spacing: -.055em; }
    .guide-intro { max-width: 720px; margin: 1.75rem 0 0; color: rgba(244,247,251,.72); font-size: 1.18rem; line-height: 1.7; }
    .guide-actions { display: flex; flex-wrap: wrap; gap: .85rem; margin-top: 2rem; }
    .guide-actions a { display: inline-flex; padding: .9rem 1.2rem; border-radius: 999px; text-decoration: none; font-weight: 650; }
    .guide-primary { background: #8dcdff; color: #071018; }
    .guide-secondary { border: 1px solid rgba(255,255,255,.2); color: #fff; }
    .public-guide-content { max-width: 920px; margin: 0 auto; }
    .answer-grid { display: grid; grid-template-columns: repeat(2, minmax(0,1fr)); gap: 1rem; margin-bottom: 1rem; }
    .answer-card { padding: 1.6rem; border: 1px solid rgba(255,255,255,.11); border-radius: 18px; background: rgba(255,255,255,.035); }
    .answer-card-primary { margin-bottom: 1rem; background: rgba(141,205,255,.075); border-color: rgba(141,205,255,.25); }
    .answer-card h2, .guide-related h2 { margin: 0 0 .8rem; color: #fff; font-size: 1.25rem; }
    .answer-card p, .answer-card li { color: rgba(244,247,251,.7); line-height: 1.65; }
    .answer-card p:last-child { margin-bottom: 0; }
    .answer-card a, .country-list a { color: #8dcdff; }
    .step-list { list-style: none; padding: 0; margin: 0 0 1rem; counter-reset: steps; display: grid; gap: .8rem; }
    .step-list li { counter-increment: steps; display: grid; grid-template-columns: 3rem 1fr; gap: .3rem 1rem; padding: 1.35rem; border: 1px solid rgba(255,255,255,.11); border-radius: 16px; background: rgba(255,255,255,.035); }
    .step-list li::before { content: counter(steps); grid-row: span 2; display: grid; place-items: center; width: 2.5rem; height: 2.5rem; border-radius: 50%; background: #8dcdff; color: #071018; font-weight: 800; }
    .step-list strong { color: #fff; }
    .step-list span { color: rgba(244,247,251,.68); line-height: 1.55; }
    .country-list { margin: 1rem 0; padding: 1.25rem; border: 1px solid rgba(255,255,255,.11); border-radius: 16px; }
    .country-list summary { color: #fff; cursor: pointer; font-weight: 650; }
    .country-list ul { columns: 3; margin-top: 1rem; padding-left: 1.2rem; }
    .country-list li { color: rgba(244,247,251,.65); margin-bottom: .35rem; break-inside: avoid; }
    .guide-related { margin-top: 3rem; padding-top: 2rem; border-top: 1px solid rgba(255,255,255,.1); }
    .guide-related div { display: flex; flex-wrap: wrap; gap: .65rem; }
    .guide-related a { color: #b9dfff; text-decoration: none; border: 1px solid rgba(141,205,255,.2); border-radius: 999px; padding: .6rem .85rem; }
    @media (max-width: 700px) { .public-guide-hero { padding-top: 5.5rem; } .guide-eyebrow { margin-top: 3rem; } .answer-grid { grid-template-columns: 1fr; } .country-list ul { columns: 1; } }
"#;
