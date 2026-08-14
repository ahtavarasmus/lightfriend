use crate::utils::datafast::track_goal;
use crate::utils::seo::{use_seo, SeoMeta};
use yew::prelude::*;

#[derive(Clone, Copy, PartialEq)]
enum ServiceKind {
    Whatsapp,
    Signal,
    Telegram,
    Email,
    SearchAndWeather,
    PhotosAndQr,
    Tesla,
    Mcp,
}

impl ServiceKind {
    fn label(self) -> &'static str {
        match self {
            Self::Whatsapp => "WhatsApp",
            Self::Signal => "Signal",
            Self::Telegram => "Telegram",
            Self::Email => "Email",
            Self::SearchAndWeather => "Web search and weather",
            Self::PhotosAndQr => "Photos and QR codes",
            Self::Tesla => "Tesla controls",
            Self::Mcp => "Custom MCP tools",
        }
    }

    fn summary(self) -> &'static str {
        match self {
            Self::Whatsapp => {
                "Selected message checks, focused alerts, digests, search, and supported text replies over SMS."
            }
            Self::Signal => {
                "Selected Signal conversations can be checked and acted on through the Lightfriend SMS interface."
            }
            Self::Telegram => {
                "Selected Telegram chats and groups can be checked, summarized, and used through SMS."
            }
            Self::Email => {
                "Ask about recent mail or a sender, receive focused alerts and digests, and request supported replies."
            }
            Self::SearchAndWeather => {
                "Ask a question or request a forecast and receive a compact answer by SMS or optional voice call."
            }
            Self::PhotosAndQr => {
                "In supported MMS countries, send an image for analysis, translation, or QR decoding."
            }
            Self::Tesla => {
                "Check selected vehicle information and use supported controls through a text conversation."
            }
            Self::Mcp => {
                "Connect compatible external tool servers in the dashboard, then use enabled tools through Lightfriend."
            }
        }
    }

    fn setup_note(self) -> &'static str {
        match self {
            Self::Whatsapp => {
                "Keep a supported device with native WhatsApp for pairing, recovery, and periodic linked-device maintenance."
            }
            Self::Signal => {
                "Initial connection and future recovery can require a supported device and access to the Signal account."
            }
            Self::Telegram => {
                "Initial authorization and future recovery require access to the Telegram account."
            }
            Self::Email => {
                "Connect the account in the web dashboard and retain a separate account-recovery method."
            }
            Self::SearchAndWeather => "No app is installed on the carried phone.",
            Self::PhotosAndQr => {
                "Requires MMS support and is currently limited to supported number routes and countries."
            }
            Self::Tesla => {
                "Connect and authorize the vehicle account in the dashboard before using supported controls."
            }
            Self::Mcp => {
                "Add a compatible server in the dashboard and review its tools and external trust boundary."
            }
        }
    }

    fn needs_setup_device(self) -> bool {
        matches!(self, Self::Whatsapp | Self::Signal | Self::Telegram)
    }
}

const SERVICES: [ServiceKind; 8] = [
    ServiceKind::Whatsapp,
    ServiceKind::Signal,
    ServiceKind::Telegram,
    ServiceKind::Email,
    ServiceKind::SearchAndWeather,
    ServiceKind::PhotosAndQr,
    ServiceKind::Tesla,
    ServiceKind::Mcp,
];

#[function_component(SmartphoneExitPlanner)]
pub fn smartphone_exit_planner() -> Html {
    use_seo(SeoMeta {
        title: "Can I Leave My Smartphone? Keep Essential Services on Any Phone",
        description: "Build a practical smartphone-exit plan. Lightfriend keeps selected WhatsApp, Signal, Telegram, email, search, and other tools available through SMS and calls on any phone.",
        canonical: "https://lightfriend.ai/can-i-leave-my-smartphone",
        og_type: "website",
    });

    let has_sms = use_state(|| true);
    let has_calls = use_state(|| true);
    let has_setup_device = use_state(|| true);
    let selected = use_state(|| {
        vec![
            ServiceKind::Whatsapp,
            ServiceKind::Email,
            ServiceKind::SearchAndWeather,
        ]
    });
    let show_plan = use_state(|| false);

    let set_sms = {
        let has_sms = has_sms.clone();
        Callback::from(move |_| has_sms.set(!*has_sms))
    };
    let set_calls = {
        let has_calls = has_calls.clone();
        Callback::from(move |_| has_calls.set(!*has_calls))
    };
    let set_setup_device = {
        let has_setup_device = has_setup_device.clone();
        Callback::from(move |_| has_setup_device.set(!*has_setup_device))
    };
    let build_plan = {
        let has_sms = has_sms.clone();
        let has_calls = has_calls.clone();
        let has_setup_device = has_setup_device.clone();
        let selected = selected.clone();
        let show_plan = show_plan.clone();
        Callback::from(move |_| {
            let selected_count = selected.len().to_string();
            let sms_value = if *has_sms { "yes" } else { "no" };
            let call_value = if *has_calls { "yes" } else { "no" };
            let setup_value = if *has_setup_device { "yes" } else { "no" };
            track_goal(
                "exit_plan_completed",
                &[
                    ("selected_services", selected_count.as_str()),
                    ("sms", sms_value),
                    ("calls", call_value),
                    ("setup_device", setup_value),
                ],
            );
            show_plan.set(true);
        })
    };

    let needs_setup_device = selected.iter().any(|service| service.needs_setup_device());
    let phone_ready = *has_sms || *has_calls;

    html! {
        <main class="exit-planner-page">
            <header class="exit-hero">
                <a class="exit-logo" href="/">{"lightfriend"}</a>
                <p class="exit-eyebrow">{"Smartphone exit planner"}</p>
                <h1>{"Choose any phone. Keep the services that matter."}</h1>
                <p class="exit-intro">
                    {"Lightfriend runs outside the handset. If your phone can use ordinary SMS or calls, it does not need apps, mobile data, a browser, or a particular operating system."}
                </p>
                <div class="universal-proof" role="note">
                    <strong>{"The phone is not the platform."}</strong>
                    <span>{"SMS and calls are the interface. Lightfriend maintains the connected-service side."}</span>
                </div>
            </header>

            <section class="planner-shell" aria-labelledby="planner-heading">
                <div class="planner-heading">
                    <p class="exit-eyebrow">{"Build your plan"}</p>
                    <h2 id="planner-heading">{"What does your next phone need to preserve?"}</h2>
                    <p>{"Nothing is submitted. Your selections only build the plan in this browser."}</p>
                </div>

                <div class="planner-step">
                    <div class="step-number">{"1"}</div>
                    <div>
                        <h3>{"How can the phone reach Lightfriend?"}</h3>
                        <div class="choice-grid choice-grid-small">
                            <button type="button" class={classes!("choice-card", (*has_sms).then_some("selected"))} aria-pressed={(*has_sms).to_string()} onclick={set_sms}>
                                <strong>{"SMS"}</strong>
                                <span>{"The complete text-first interface and focused alerts."}</span>
                            </button>
                            <button type="button" class={classes!("choice-card", (*has_calls).then_some("selected"))} aria-pressed={(*has_calls).to_string()} onclick={set_calls}>
                                <strong>{"Voice calls"}</strong>
                                <span>{"An optional voice interface for longer questions."}</span>
                            </button>
                        </div>
                    </div>
                </div>

                <div class="planner-step">
                    <div class="step-number">{"2"}</div>
                    <div>
                        <h3>{"Select what you want to keep"}</h3>
                        <div class="choice-grid">
                            {for SERVICES.iter().copied().map(|service| {
                                let selected_state = selected.clone();
                                let is_selected = selected.contains(&service);
                                let onclick = Callback::from(move |_| {
                                    let mut next = (*selected_state).clone();
                                    if let Some(index) = next.iter().position(|item| *item == service) {
                                        next.remove(index);
                                    } else {
                                        next.push(service);
                                    }
                                    selected_state.set(next);
                                });
                                html! {
                                    <button type="button" class={classes!("choice-card", is_selected.then_some("selected"))} aria-pressed={is_selected.to_string()} {onclick}>
                                        <strong>{service.label()}</strong>
                                        <span>{service.summary()}</span>
                                    </button>
                                }
                            })}
                        </div>
                    </div>
                </div>

                <div class="planner-step">
                    <div class="step-number">{"3"}</div>
                    <div>
                        <h3>{"Can you retain a setup device when a provider requires one?"}</h3>
                        <p class="step-explainer">{"The carried phone can be anything. Some connected accounts still require their own supported app for initial authorization, recovery, or maintenance."}</p>
                        <button type="button" class={classes!("choice-card", "single-choice", (*has_setup_device).then_some("selected"))} aria-pressed={(*has_setup_device).to_string()} onclick={set_setup_device}>
                            <strong>{if *has_setup_device { "Yes, I can keep one available" } else { "No, I cannot keep a setup device" }}</strong>
                            <span>{"This changes the account-setup plan, not which phone you can carry."}</span>
                        </button>
                    </div>
                </div>

                <button type="button" class="build-plan-button" onclick={build_plan}>{"Build my smartphone-exit plan"}</button>

                if *show_plan {
                    <section class="plan-result" aria-live="polite">
                        <p class="exit-eyebrow">{"Your plan"}</p>
                        if phone_ready {
                            <h2>{"Your chosen phone can be the Lightfriend interface."}</h2>
                            if *has_sms {
                                <p class="result-lead">{"Ordinary SMS gives you the full text-first experience. The handset brand and operating system do not need a Lightfriend integration."}</p>
                            } else {
                                <p class="result-lead">{"Voice calls can provide an interface, but SMS is needed for text responses, focused alerts, and the complete text-first experience."}</p>
                            }
                        } else {
                            <h2>{"Add an SMS or call route first."}</h2>
                            <p class="result-lead">{"Lightfriend does not install an app on the handset. The phone needs an ordinary way to call or text the Lightfriend number."}</p>
                        }

                        if selected.is_empty() {
                            <div class="result-warning">{"Select at least one service above to build a useful account plan."}</div>
                        } else {
                            <div class="result-list">
                                {for selected.iter().copied().map(|service| html! {
                                    <article>
                                        <h3>{service.label()}</h3>
                                        <p>{service.summary()}</p>
                                        <small>{service.setup_note()}</small>
                                    </article>
                                })}
                            </div>
                        }

                        if needs_setup_device && !*has_setup_device {
                            <div class="result-warning">
                                <strong>{"Account setup is the constraint—not your carried phone."}</strong>
                                <span>{"At least one selected messaging provider can require its native app for pairing or recovery. Consider a secured device kept at home, or remove that provider from the plan."}</span>
                            </div>
                        }

                        <div class="result-actions">
                            <a class="primary-action" href="/supported-countries">{"Check my country and number route"}</a>
                            <a class="secondary-action" href="/get-started">{"See how it works and start a trial"}</a>
                        </div>
                        <p class="result-footnote">{"Availability still depends on number inventory, carrier capabilities, destination permissions, provider rules, and local regulation."}</p>
                    </section>
                }
            </section>

            <section class="capability-section" aria-labelledby="capability-heading">
                <p class="exit-eyebrow">{"Universal capability matrix"}</p>
                <h2 id="capability-heading">{"What stays outside the phone"}</h2>
                <p class="capability-intro">{"The handset only handles SMS and calls. Connected clients, account monitoring, AI processing, and automations run in Lightfriend's service."}</p>
                <div class="capability-table-wrap">
                    <table>
                        <thead>
                            <tr><th>{"Capability"}</th><th>{"Phone interface"}</th><th>{"What remains elsewhere"}</th></tr>
                        </thead>
                        <tbody>
                            <tr><th>{"WhatsApp, Signal, Telegram"}</th><td>{"SMS questions, alerts, digests, supported replies"}</td><td>{"Provider account, pairing, recovery, rich interfaces"}</td></tr>
                            <tr><th>{"Email"}</th><td>{"SMS search, summaries, alerts, supported replies"}</td><td>{"Account administration, attachments, complex inbox work"}</td></tr>
                            <tr><th>{"AI search and weather"}</th><td>{"SMS or optional voice question and compact answer"}</td><td>{"Search and AI processing in the service"}</td></tr>
                            <tr><th>{"Images and QR codes"}</th><td>{"MMS where the number route supports it"}</td><td>{"Analysis in the service; availability varies by country"}</td></tr>
                            <tr><th>{"Tesla and MCP tools"}</th><td>{"Supported text or voice requests"}</td><td>{"External account authorization and third-party trust boundaries"}</td></tr>
                        </tbody>
                    </table>
                </div>
            </section>

            <section class="exit-bottom-cta">
                <p class="exit-eyebrow">{"The simple rule"}</p>
                <h2>{"Choose the phone for how it feels to carry. Let Lightfriend handle the services."}</h2>
                <div class="result-actions">
                    <a class="primary-action" href="/how-it-works">{"See how Lightfriend works"}</a>
                    <a class="secondary-action" href="/limitations">{"Read the honest limitations"}</a>
                </div>
            </section>

            <style>{PLANNER_CSS}</style>
        </main>
    }
}

const PLANNER_CSS: &str = r#"
    .exit-planner-page { min-height: 100vh; padding: 0 1.25rem 5rem; background: #0d1217; color: #f4f7fb; }
    .exit-hero, .planner-shell, .capability-section, .exit-bottom-cta { max-width: 980px; margin: 0 auto; }
    .exit-hero { padding: 7rem 0 3.5rem; }
    .exit-logo { color: #fff; font-size: 1.1rem; font-weight: 750; text-decoration: none; }
    .exit-eyebrow { margin: 3.6rem 0 .9rem; color: #8dcdff; font-size: .76rem; font-weight: 750; letter-spacing: .15em; text-transform: uppercase; }
    .exit-hero h1 { max-width: 920px; margin: 0; font-size: clamp(2.8rem, 7vw, 6rem); line-height: .96; letter-spacing: -.06em; text-wrap: balance; }
    .exit-intro { max-width: 760px; margin: 1.6rem 0 0; color: rgba(244,247,251,.72); font-size: 1.2rem; line-height: 1.7; text-wrap: pretty; }
    .universal-proof { display: grid; gap: .3rem; max-width: 720px; margin-top: 2rem; padding: 1.2rem 1.35rem; border: 1px solid rgba(141,205,255,.28); border-radius: 16px; background: rgba(141,205,255,.07); }
    .universal-proof strong { color: #fff; }
    .universal-proof span { color: rgba(244,247,251,.68); line-height: 1.5; }
    .planner-shell { padding: 2rem; border: 1px solid rgba(255,255,255,.12); border-radius: 26px; background: rgba(255,255,255,.035); }
    .planner-heading h2, .capability-section h2, .exit-bottom-cta h2, .plan-result h2 { margin: 0; font-size: clamp(2rem, 4vw, 3.5rem); line-height: 1.05; letter-spacing: -.035em; text-wrap: balance; }
    .planner-heading .exit-eyebrow, .capability-section .exit-eyebrow, .exit-bottom-cta .exit-eyebrow, .plan-result .exit-eyebrow { margin-top: 0; }
    .planner-heading > p:last-child, .capability-intro { color: rgba(244,247,251,.62); line-height: 1.6; }
    .planner-step { display: grid; grid-template-columns: 2.8rem 1fr; gap: 1rem; padding: 2rem 0; border-top: 1px solid rgba(255,255,255,.09); }
    .planner-step:first-of-type { margin-top: 1.5rem; }
    .step-number { display: grid; place-items: center; width: 2.5rem; height: 2.5rem; border-radius: 50%; background: #8dcdff; color: #071018; font-weight: 850; }
    .planner-step h3 { margin: .4rem 0 1rem; font-size: 1.3rem; }
    .step-explainer { max-width: 760px; color: rgba(244,247,251,.62); line-height: 1.6; }
    .choice-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: .8rem; }
    .choice-grid-small { max-width: 720px; }
    .choice-card { display: grid; gap: .42rem; width: 100%; min-height: 72px; padding: 1.1rem; text-align: left; color: #f4f7fb; border: 1px solid rgba(255,255,255,.12); border-radius: 15px; background: rgba(0,0,0,.18); cursor: pointer; transition: border-color 150ms ease, background 150ms ease, transform 150ms ease; }
    .choice-card:hover { transform: translateY(-1px); border-color: rgba(141,205,255,.45); }
    .choice-card:active, .build-plan-button:active, .result-actions a:active { transform: scale(.98); }
    .choice-card:focus-visible, .build-plan-button:focus-visible, .result-actions a:focus-visible { outline: 3px solid #fff; outline-offset: 3px; }
    .choice-card.selected { border-color: #8dcdff; background: rgba(141,205,255,.1); box-shadow: inset 0 0 0 1px rgba(141,205,255,.18); }
    .choice-card strong { font-size: 1rem; }
    .choice-card span { color: rgba(244,247,251,.62); line-height: 1.45; }
    .single-choice { max-width: 720px; }
    .build-plan-button { width: 100%; min-height: 48px; margin-top: 1rem; padding: 1rem 1.3rem; border: 0; border-radius: 999px; background: #8dcdff; color: #071018; font-size: 1rem; font-weight: 800; cursor: pointer; transition: background 150ms ease, transform 150ms ease; }
    .build-plan-button:hover { background: #b5dfff; }
    .plan-result { margin-top: 2rem; padding: 1.7rem; border-radius: 20px; background: #f1f7fb; color: #111820; }
    .plan-result .exit-eyebrow { color: #28688d; }
    .result-lead { max-width: 760px; color: #40505c; font-size: 1.05rem; line-height: 1.65; }
    .result-list { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: .75rem; margin: 1.4rem 0; }
    .result-list article { padding: 1.1rem; border: 1px solid #d4e1e8; border-radius: 14px; background: #fff; }
    .result-list h3 { margin: 0 0 .45rem; }
    .result-list p { color: #40505c; line-height: 1.5; }
    .result-list small { display: block; color: #677985; line-height: 1.5; }
    .result-warning { display: grid; gap: .35rem; margin: 1rem 0; padding: 1rem; border-left: 4px solid #d58a2f; border-radius: 8px; background: #fff4e5; color: #5c3a12; line-height: 1.5; }
    .result-actions { display: flex; flex-wrap: wrap; gap: .75rem; margin-top: 1.4rem; }
    .result-actions a { display: inline-flex; min-height: 44px; box-sizing: border-box; align-items: center; padding: .85rem 1.1rem; border-radius: 999px; font-weight: 750; text-decoration: none; transition: transform 150ms ease, background 150ms ease; }
    .primary-action { background: #8dcdff; color: #071018; }
    .secondary-action { border: 1px solid currentColor; color: inherit; }
    .plan-result .secondary-action { color: #1c4c68; }
    .result-footnote { color: #677985; font-size: .86rem; line-height: 1.55; }
    .capability-section, .exit-bottom-cta { padding-top: 5rem; }
    .capability-table-wrap { margin-top: 1.5rem; overflow-x: auto; border: 1px solid rgba(255,255,255,.12); border-radius: 18px; }
    table { width: 100%; min-width: 680px; border-collapse: collapse; background: rgba(255,255,255,.025); }
    th, td { padding: 1rem; text-align: left; vertical-align: top; border-bottom: 1px solid rgba(255,255,255,.09); line-height: 1.5; }
    thead th { color: #8dcdff; font-size: .78rem; letter-spacing: .08em; text-transform: uppercase; }
    tbody th { width: 24%; color: #fff; }
    tbody td { color: rgba(244,247,251,.65); }
    tbody tr:last-child th, tbody tr:last-child td { border-bottom: 0; }
    .exit-bottom-cta { padding-bottom: 2rem; }
    .exit-bottom-cta h2 { max-width: 850px; }
    @media (max-width: 720px) {
        .exit-hero { padding-top: 5.5rem; }
        .planner-shell { padding: 1.2rem; border-radius: 20px; }
        .planner-step { grid-template-columns: 1fr; }
        .choice-grid, .result-list { grid-template-columns: 1fr; }
        .result-actions a { width: 100%; justify-content: center; }
    }
    @media (prefers-reduced-motion: reduce) {
        .choice-card, .build-plan-button, .result-actions a { transition: none; }
    }
"#;
