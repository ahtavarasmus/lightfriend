use crate::profile::stripe::StripePricingTable;
use crate::utils::api::Api;
use crate::utils::seo::{use_seo, SeoMeta};
use crate::Route;
use serde::Deserialize;
use serde_json::json;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use web_sys::{HtmlInputElement, MouseEvent};
use yew::prelude::*;
use yew_router::components::Link;

#[derive(Deserialize, Clone)]
struct SmartphoneFreeDaysResponse {
    days: i64,
}
#[function_component(Landing)]
pub fn landing() -> Html {
    use_seo(SeoMeta {
        title: "Lightfriend: Free.",
        description: "The feed cannot follow you to a dumbphone. Lightfriend keeps important WhatsApp, Telegram, Signal, and email messages reachable by SMS or call.",
        canonical: "https://lightfriend.ai",
        og_type: "website",
    });

    // Waitlist form state
    let waitlist_email = use_state(String::new);
    let waitlist_loading = use_state(|| false);
    let waitlist_success = use_state(|| false);
    let waitlist_error = use_state(|| None::<String>);

    // Respect deep links like /#plans; otherwise start the landing page at top.
    {
        use_effect_with_deps(
            move |_| {
                if let Some(window) = web_sys::window() {
                    let hash = window.location().hash().unwrap_or_default();
                    let mut scheduled_hash_scroll = false;
                    if let Some(target_id) = hash.strip_prefix('#') {
                        if !target_id.is_empty() {
                            let target_id = target_id.to_string();
                            let scroll_to_hash = Closure::<dyn Fn()>::new(move || {
                                if let Some(window) = web_sys::window() {
                                    if let Some(document) = window.document() {
                                        if let Some(element) =
                                            document.get_element_by_id(&target_id)
                                        {
                                            element.scroll_into_view();
                                        }
                                    }
                                }
                            });
                            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                                scroll_to_hash.as_ref().unchecked_ref(),
                                100,
                            );
                            scroll_to_hash.forget();
                            scheduled_hash_scroll = true;
                        }
                    }
                    if !scheduled_hash_scroll {
                        window.scroll_to_with_x_and_y(0.0, 0.0);
                    }
                }
                || ()
            },
            (),
        );
    }

    // State for smartphone-free days powered metric
    let smartphone_free_days = use_state(|| None::<i64>);

    // Fetch smartphone-free days metric from API
    {
        let smartphone_free_days = smartphone_free_days.clone();
        use_effect_with_deps(
            move |_| {
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(response) = Api::get("/api/stats/smartphone-free-days").send().await {
                        if response.ok() {
                            if let Ok(data) = response.json::<SmartphoneFreeDaysResponse>().await {
                                smartphone_free_days.set(Some(data.days));
                            }
                        }
                    }
                });
                || ()
            },
            (),
        );
    }

    // Format days with thousands separator
    let days_smartphone_free = {
        let days = (*smartphone_free_days).unwrap_or(0);
        let s = days.to_string();
        let mut result = String::new();
        for (i, c) in s.chars().rev().enumerate() {
            if i > 0 && i % 3 == 0 {
                result.insert(0, ',');
            }
            result.insert(0, c);
        }
        result
    };

    // State for expanded FAQ item (-1 = none, 0+ = which one)
    let expanded_faq = use_state(|| -1i32);

    // State for expanded capability card (-1 = none, 0-3 = which one)
    let expanded_cap = use_state(|| -1i32);

    // FAQ data: (question, answer_html)
    let faq_data: Vec<(&str, Html)> = vec![
        (
            "Do I need a phone with internet?",
            html! {
                <p>{"No. Lightfriend works through normal voice calls and SMS. Any phone that can call and text will work."}</p>
            },
        ),
        (
            "How does it protect my data?",
            html! {
                <>
                    <p>{"Lightfriend runs in its own hardware-isolated enclave, and all AI requests are processed through Tinfoil's verified enclaves. No one - not even the developer - can access your data. Period. Fully open source, with privacy cryptographically verifiable on blockchain."}</p>
                    <p><a href="/trustless" style="color: var(--landing-blue);">{"See exactly how it works"}</a></p>
                </>
            },
        ),
        (
            "Can I send and receive messages?",
            html! {
                <p>{"Yes. You can reply to WhatsApp, Telegram, Signal, and email directly via SMS or voice call. Lightfriend forwards your reply to the right place."}</p>
            },
        ),
        (
            "What can Lightfriend actually do?",
            html! {
                <>
                    <p>{"Lightfriend includes the smart assistant and all connected tools as context: WhatsApp, Telegram, Signal, email, web search, image understanding, Tesla, MCP integrations, and more."}</p>
                    <p>{"Autopilot adds the proactive layer: automatic critical notifications, smart digests, and custom rules that watch for what matters without you asking first."}</p>
                </>
            },
        ),
        (
            "How much usage is included?",
            html! {
                <>
                    <p>{"Every billing timeline includes $25/month in messaging credits for SMS and voice delivery. The cap is mainly there to prevent abuse and runaway spam. For normal use - asking questions when needed and receiving updates through the day on Autopilot - it should be more than enough."}</p>
                    <p>{"Actual usage depends on Twilio delivery costs in your country. US and Canada SMS is usually around 1.5 cents per message. Europe is often around 15-30 cents per message, and some countries can be closer to $1 depending on destination and carrier fees."}</p>
                    <p>
                        {"You can check current costs on Twilio's "}
                        <a href="https://www.twilio.com/en-us/sms/pricing" target="_blank" rel="noopener noreferrer">{"SMS pricing"}</a>
                        {" and "}
                        <a href="https://www.twilio.com/en-us/voice/pricing/us" target="_blank" rel="noopener noreferrer">{"Voice pricing"}</a>
                        {" pages."}
                    </p>
                </>
            },
        ),
        (
            "Which countries are supported?",
            html! {
                <>
                    <p><strong>{"Full service:"}</strong>{" US, Canada, UK, Finland, Netherlands, Australia."}</p>
                    <p><strong>{"Notification-only:"}</strong>{" 30+ countries across Europe and Asia-Pacific."}</p>
                    <p><strong>{"Elsewhere:"}</strong>{" Bring your own Twilio number."}</p>
                </>
            },
        ),
        (
            "How do critical notifications work?",
            html! {
                <p>{"When a message arrives on WhatsApp, Telegram, Signal, or email, AI evaluates whether it needs your immediate attention. Urgent messages get forwarded instantly via SMS or phone call. Everything else goes into your digest."}</p>
            },
        ),
    ];

    let faq_items_html: Vec<Html> = faq_data
        .into_iter()
        .enumerate()
        .map(|(idx, (question, answer))| {
            let expanded = expanded_faq.clone();
            let i = idx as i32;
            let is_open = *expanded == i;
            let onclick = {
                let expanded = expanded.clone();
                Callback::from(move |e: MouseEvent| {
                    e.prevent_default();
                    if *expanded == i {
                        expanded.set(-1);
                    } else {
                        expanded.set(i);
                    }
                })
            };
            html! {
                <div class={classes!("landing-faq-item", if is_open { "open" } else { "" })}>
                    <button class="landing-faq-question" onclick={onclick}>
                        <span class="question-text">{question}</span>
                        <span class="toggle-icon">{if is_open { "\u{2212}" } else { "+" }}</span>
                    </button>
                    <div class="landing-faq-answer">
                        {answer}
                    </div>
                </div>
            }
        })
        .collect();

    // Capability cards data: (icon_class, title, one_liner, detail_html)
    let cap_data: Vec<(&str, &str, &str, Html)> = vec![
        (
            "fas fa-comments",
            "Messages",
            "WhatsApp, Telegram, Signal, email - from any phone.",
            html! {
                <p>{"Reply to messages, send new ones, and get summaries of long threads - all via SMS or voice call. This is what makes switching to a dumbphone possible in the first place."}</p>
            },
        ),
        (
            "fas fa-bell",
            "Alerts",
            "Urgent things reach you. Everything else waits.",
            html! {
                <p>{"AI evaluates every incoming message across all your apps. Time-critical ones - lunch invites, emergencies, deadlines - get forwarded immediately as SMS or a phone call. No setup needed, works out of the box."}</p>
            },
        ),
        (
            "fas fa-list-check",
            "Digests",
            "Catch up once. Not all day.",
            html! {
                <p>{"Non-urgent messages are batched into a digest delivered on your schedule. Keeps you in the loop without constant interruptions throughout the day."}</p>
            },
        ),
        (
            "fas fa-sliders",
            "Rules",
            "Optional automation when defaults aren't enough.",
            html! {
                <>
                    <p>{"Create WHEN/IF/THEN rules: trigger on message arrival, a schedule, or a keyword. Conditions can use AI evaluation, keyword matching, or sender filters - like always forwarding messages from a specific person. Actions include forwarding, summarizing, replying, or running a check."}</p>
                    <p>{"Set simple reminders. Schedule recurring checks. Everything is optional and customizable."}</p>
                </>
            },
        ),
    ];

    let cap_cards_html: Vec<Html> = cap_data.into_iter().enumerate().map(|(idx, (icon, title, one_liner, detail))| {
        let expanded = expanded_cap.clone();
        let i = idx as i32;
        let is_open = *expanded == i;
        let onclick = {
            let expanded = expanded.clone();
            Callback::from(move |e: MouseEvent| {
                e.prevent_default();
                if *expanded == i {
                    expanded.set(-1);
                } else {
                    expanded.set(i);
                }
            })
        };
        html! {
            <div class={classes!("cap-card", if is_open { "open" } else { "" })} onclick={onclick}>
                <div class="cap-card-header">
                    <i class={icon}></i>
                    <div class="cap-card-text">
                        <span class="cap-card-title">{title}</span>
                        <span class="cap-card-liner">{one_liner}</span>
                    </div>
                    <span class="cap-card-toggle">{if is_open { "\u{2212}" } else { "+" }}</span>
                </div>
                <div class="cap-card-detail">
                    {detail}
                </div>
            </div>
        }
    }).collect();

    // Generate particle elements for hero background
    let particles_html: Vec<Html> = (0..25).map(|i| {
        let left = ((i * 17 + 5) % 100) as f64;
        let delay = (i as f64) * 0.8;
        let duration = 6.0 + ((i % 5) as f64) * 2.0;
        let size = 1.0 + ((i % 3) as f64);
        let style = format!(
            "left: {}%; animation-delay: {:.1}s; animation-duration: {:.1}s; width: {:.0}px; height: {:.0}px;",
            left, delay, duration, size, size
        );
        html! { <span class="particle" style={style}></span> }
    }).collect();

    html! {
        <div class="landing-page">
            <head>
                <link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.5.2/css/all.min.css" integrity="sha512-SnH5WK+bZxgPHs44uWIX+LLJAJ9/2PkPKZ5QiAj6Ta86w+fsb2TkcmfRyVX3pBnMFcV7oQPJkl9QevSCWr3W6A==" crossorigin="anonymous" referrerpolicy="no-referrer" />
            </head>
            <header class="hero">
                <div class="hero-particles">
                    { for particles_html }
                </div>
                <div class="hero-content">
                    <div class="hero-right-panel hero-word-panel">
                        <h1 class="hero-title hero-word-title hero-anim hero-anim-1">{"FREE LIKE A CHILD"}</h1>
                    </div>
                </div>
            </header>

            // TODO: Image carousel goes here when "Removed" style photos are ready

            <section class="fight-section scroll-animate">
                <div class="fight-copy">
                    <h2>{"THIS FIGHT IS RIGGED"}</h2>
                    <p>{"Stop fighting the phone. Leave the ring."}</p>
                    <a href="#plans" class="story-cta-link">{"Start 7-day free trial"}</a>
                </div>
            </section>

            <section id="apps" class="app-bridge-section scroll-animate">
                <div class="app-bridge-copy">
                    <p>{"Lightfriend watches WhatsApp, Signal, Telegram, and email. Important things reach any phone by SMS or call."}</p>
                </div>
                <div class="hero-diagram app-bridge-visual">
                    <div class="diagram-left">
                        <img src="/assets/empty-phone.png" alt="Your phone" class="diagram-nokia" />
                        <span class="diagram-node-label">{"Your phone"}</span>
                    </div>
                    <div class="diagram-center-group">
                        <svg class="diagram-left-line" viewBox="0 0 100 24" preserveAspectRatio="none">
                            <defs>
                                <marker id="arrow-right" markerWidth="6" markerHeight="4" refX="5" refY="2" orient="auto">
                                    <path d="M0,0 L6,2 L0,4" fill="rgba(126,178,255,0.6)" />
                                </marker>
                                <marker id="arrow-left" markerWidth="6" markerHeight="4" refX="1" refY="2" orient="auto">
                                    <path d="M6,0 L0,2 L6,4" fill="rgba(126,178,255,0.6)" />
                                </marker>
                            </defs>
                            <line x1="4" y1="12" x2="96" y2="12" stroke="rgba(126,178,255,0.4)" stroke-width="1" marker-start="url(#arrow-left)" marker-end="url(#arrow-right)" />
                        </svg>
                        <span class="diagram-edge-label">{"SMS / Call"}</span>
                        <div class="diagram-lf-wrapper">
                            <img src="/assets/fav.png" alt="Lightfriend" class="diagram-lf-icon" />
                        </div>
                    </div>
                    <div class="diagram-right-group">
                        <svg class="diagram-fan-svg" viewBox="0 0 70 130" preserveAspectRatio="none">
                            <defs>
                                <marker id="fan-arrow-r" markerWidth="5" markerHeight="4" refX="4" refY="2" orient="auto">
                                    <path d="M0,0 L5,2 L0,4" fill="rgba(126,178,255,0.5)" />
                                </marker>
                                <marker id="fan-arrow-l" markerWidth="5" markerHeight="4" refX="1" refY="2" orient="auto">
                                    <path d="M5,0 L0,2 L5,4" fill="rgba(126,178,255,0.5)" />
                                </marker>
                            </defs>
                            <line x1="0" y1="65" x2="65" y2="5" stroke="rgba(126,178,255,0.3)" stroke-width="1" marker-start="url(#fan-arrow-l)" marker-end="url(#fan-arrow-r)" />
                            <line x1="0" y1="65" x2="65" y2="35" stroke="rgba(126,178,255,0.3)" stroke-width="1" marker-start="url(#fan-arrow-l)" marker-end="url(#fan-arrow-r)" />
                            <line x1="0" y1="65" x2="65" y2="65" stroke="rgba(126,178,255,0.3)" stroke-width="1" marker-start="url(#fan-arrow-l)" marker-end="url(#fan-arrow-r)" />
                            <line x1="0" y1="65" x2="65" y2="95" stroke="rgba(126,178,255,0.3)" stroke-width="1" marker-start="url(#fan-arrow-l)" marker-end="url(#fan-arrow-r)" />
                            <line x1="0" y1="65" x2="65" y2="125" stroke="rgba(126,178,255,0.3)" stroke-width="1" marker-start="url(#fan-arrow-l)" marker-end="url(#fan-arrow-r)" />
                        </svg>
                        <div class="diagram-apps-list">
                            <div class="diagram-app-row"><i class="fab fa-whatsapp"></i><span>{"WhatsApp"}</span></div>
                            <div class="diagram-app-row"><i class="fab fa-telegram"></i><span>{"Telegram"}</span></div>
                            <div class="diagram-app-row"><i class="fab fa-signal-messenger"></i><span>{"Signal"}</span></div>
                            <div class="diagram-app-row"><i class="fas fa-envelope"></i><span>{"Email"}</span></div>
                            <div class="diagram-app-row"><i class="fas fa-plug"></i><span>{"MCP"}</span></div>
                        </div>
                    </div>
                </div>
            </section>

            <section id="tradeoff" class="choice-section scroll-animate">
                <h2>{"No tradeoff."}</h2>
                <div class="choice-grid">
                    <div class="choice-card">
                        <span class="choice-label">{"Smartphone"}</span>
                        <strong>{"Connected."}</strong>
                        <strong class="choice-bad">{"Distracted."}</strong>
                    </div>
                    <div class="choice-card">
                        <span class="choice-label">{"Dumbphone"}</span>
                        <strong>{"Calm."}</strong>
                        <strong class="choice-bad">{"Cut off."}</strong>
                    </div>
                    <div class="choice-card choice-card-primary">
                        <span class="choice-label">{"Lightfriend"}</span>
                        <strong>{"Calm."}</strong>
                        <strong>{"Connected."}</strong>
                    </div>
                </div>
                <a href="#plans" class="story-cta-link">{"Start 7-day free trial"}</a>
            </section>

            // Capabilities section - what you can do with Lightfriend
            <section class="capabilities-section scroll-animate">
                <h2 class="capabilities-title">{"What you get"}</h2>
                <div class="capabilities-grid">
                    { for cap_cards_html }
                </div>
                <p class="capabilities-footnote">{"Gets better over time - Lightfriend learns what matters to you."}</p>
            </section>

            // Privacy section
            <section class="privacy-hook scroll-animate">
                <h2 class="privacy-hook-title">{"Nobody can read your messages. Not even us."}</h2>
                <p class="privacy-hook-subtitle">{"Lightfriend runs inside a hardware-isolated enclave. Your data is end-to-end private, cryptographically verifiable."}</p>
                <div class="privacy-hook-links">
                    <Link<Route> to={Route::Trustless} classes="privacy-hook-link">
                        {"Verifiably Private →"}
                    </Link<Route>>
                    <Link<Route> to={Route::TrustChain} classes="privacy-hook-link">
                        {"Trust Chain →"}
                    </Link<Route>>
                </div>
                <a href="#plans" class="story-cta-link">{"Start 7-day free trial"}</a>
            </section>

            <section class="testimonials-section scroll-animate">
                <div class="testimonials-content">
                    <div class="testimonial-metric">
                        <span class="testimonial-metric-number">{days_smartphone_free}</span>
                        <span class="testimonial-metric-label">{"smartphone-free days powered"}</span>
                    </div>
                    <h2>{"Life After Smartphones"}</h2>
                    <div class="testimonial">
                        <blockquote>
                            {"I have ADHD so smartphones were basically impossible for me. I'd check one notification and suddenly an hour was gone. Now I just get a text with the important stuff. No apps, nothing to get lost in. Honestly it's changed everything for how I get through my day."}
                        </blockquote>
                    </div>
                    <div class="testimonial">
                        <blockquote>
                            {"As a dumbphone user, I couldn't live without lightfriend. It's useful, smart and most importantly, reliable. A true must have for living a distraction free life."}
                        </blockquote>
                    </div>
                    <div class="testimonial">
                        <blockquote>
                            {"Lightfriend has saved me so many times. I\u{2019}ll forget a deadline or miss an important email \u{2014} but then Lightfriend pings me about it before it\u{2019}s too late. It watches my inbox so I don\u{2019}t have to. Honestly, I\u{2019}d be lost without it."}
                        </blockquote>
                        <p class="testimonial-author">{"- Kasperi"}</p>
                    </div>
                    <div class="testimonial">
                        <blockquote>
                            {"Lightfriend proactively alerted me of a security alert in my email when my notifications were disabled making me aware of a threat which I then took care of before anything permanent damage could be done. Thanks to lightfriend monitoring, the issue was resolved and I could go back to work swiftly."}
                        </blockquote>
                    </div>
                    <div class="testimonial">
                        <blockquote>
                            {"lightfriend fills in the gaps that the LP3(light phone 3) is missing, without making me want to use my iphone. Also I love that I can talk to Perplexity while I'm out"}
                        </blockquote>
                        <p class="testimonial-author">{"- Max"}</p>
                    </div>
                </div>
            </section>
            <div class="filter-concept">
                <div class="filter-content">
                    <div class="faq-in-filter scroll-animate">
                        <h2>{"Frequently Asked Questions"}</h2>
                        { for faq_items_html }
                        <div class="faq-more-link">
                            <a href="mailto:rasmus@lightfriend.ai" class="privacy-link">{"More questions? Email rasmus@lightfriend.ai"}</a>
                        </div>
                    </div>
                </div>
            </div>
            <section class="trust-proof scroll-animate">
                <div class="section-intro">
                    <h2>{"The Story"}</h2>
                    <img src="/assets/rasmus-pfp.png" alt="Rasmus, founder of Lightfriend" loading="lazy" style="max-width: 200px; border-radius: 50%; margin: 0 auto 1.5rem; display: block;"/>
                    <p>{"I\u{2019}m "}<a class="story-link" href="https://rasmus.ahtava.com" target="_blank" rel="noopener noreferrer">{"Rasmus"}</a>{". I built Lightfriend because I switched to a dumbphone and needed a way to keep WhatsApp and email without a smartphone."}</p>
                </div>
            </section>
            <section id="plans" class="landing-pricing-section scroll-animate">
                <div class="section-intro">
                    <h2>{"Choose your billing timeline"}</h2>
                </div>
                <StripePricingTable />
            </section>
            <footer class="footer-cta scroll-animate">
                <div class="footer-content">
                    <h2>{"Ready for Digital Peace?"}</h2>
                    <a href="#plans" class="forward-link">
                        <button class="hero-cta">{"Start 7-day free trial"}</button>
                    </a>
                    <p class="disclaimer">{"Works with any phone - smartphones, flip phones, and feature phones. No extra hardware required."}</p>
                    <div class="waitlist-section">
                        <p class="waitlist-intro">{"Not ready yet? Get updates when new features launch:"}</p>
                        {
                            if *waitlist_success {
                                html! {
                                    <p class="waitlist-success">{"Thanks! We'll keep you posted."}</p>
                                }
                            } else {
                                let waitlist_email_clone = waitlist_email.clone();
                                let waitlist_loading_clone = waitlist_loading.clone();
                                let waitlist_success_clone = waitlist_success.clone();
                                let waitlist_error_clone = waitlist_error.clone();
                                let on_submit = Callback::from(move |e: SubmitEvent| {
                                    e.prevent_default();
                                    let email = (*waitlist_email_clone).clone();
                                    let loading = waitlist_loading_clone.clone();
                                    let success = waitlist_success_clone.clone();
                                    let error = waitlist_error_clone.clone();

                                    if email.is_empty() || !email.contains('@') {
                                        error.set(Some("Please enter a valid email".to_string()));
                                        return;
                                    }

                                    loading.set(true);
                                    error.set(None);

                                    wasm_bindgen_futures::spawn_local(async move {
                                        match Api::post("/api/waitlist")
                                            .json(&json!({ "email": email }))
                                            .unwrap()
                                            .send()
                                            .await
                                        {
                                            Ok(response) => {
                                                loading.set(false);
                                                if response.ok() {
                                                    success.set(true);
                                                } else {
                                                    error.set(Some("Could not join waitlist. Try again.".to_string()));
                                                }
                                            }
                                            Err(_) => {
                                                loading.set(false);
                                                error.set(Some("Network error. Please try again.".to_string()));
                                            }
                                        }
                                    });
                                });

                                let on_email_change = {
                                    let waitlist_email = waitlist_email.clone();
                                    Callback::from(move |e: Event| {
                                        let input: HtmlInputElement = e.target_unchecked_into();
                                        waitlist_email.set(input.value());
                                    })
                                };

                                html! {
                                    <form class="waitlist-form" onsubmit={on_submit}>
                                        <input
                                            type="email"
                                            placeholder="your@email.com"
                                            class="waitlist-input"
                                            onchange={on_email_change}
                                            disabled={*waitlist_loading}
                                        />
                                        <button type="submit" class="waitlist-button" disabled={*waitlist_loading}>
                                            {if *waitlist_loading { "Joining..." } else { "Get Updates" }}
                                        </button>
                                        {
                                            if let Some(err) = (*waitlist_error).as_ref() {
                                                html! { <p class="waitlist-error">{err}</p> }
                                            } else {
                                                html! {}
                                            }
                                        }
                                    </form>
                                }
                            }
                        }
                    </div>
                    <div class="development-links">
                        <p>{"Fully open source on "}
                            <a href="https://github.com/ahtavarasmus/lightfriend" target="_blank" rel="noopener noreferrer">{"GitHub"}</a>
                        </p>
                        <div class="legal-links">
                            <Link<Route> to={Route::Terms}>{"Terms & Conditions"}</Link<Route>>
                            {" | "}
                            <Link<Route> to={Route::Privacy}>{"Privacy Policy"}</Link<Route>>
                            {" | "}
                            <Link<Route> to={Route::Trustless}>{"Verifiably Private"}</Link<Route>>
                            {" | "}
                            <Link<Route> to={Route::TrustChain}>{"Trust Chain"}</Link<Route>>
                        </div>
                    </div>
                </div>
            </footer>
            <style>
                {r#"
    .story-cta-link {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        margin-top: 2rem;
        padding: 0.8rem 1.15rem;
        border-radius: 8px;
        color: #111;
        background: rgba(255, 255, 255, 0.9);
        border: 1px solid rgba(255, 255, 255, 0.42);
        box-shadow: 0 8px 30px rgba(0, 0, 0, 0.18);
        text-decoration: none;
        font-size: 0.95rem;
        font-weight: 800;
    }
    .story-cta-link:hover {
        transform: translateY(-1px);
        background: #fff;
    }
    .story-cta-dark {
        color: #fff;
        background: rgba(0, 0, 0, 0.78);
        border-color: rgba(0, 0, 0, 0.18);
    }
    .story-cta-dark:hover {
        background: #000;
    }
    /* Hero diagram */
    .hero-diagram {
        display: flex;
        align-items: center;
        justify-content: center;
        gap: 0;
        margin: 2rem auto;
        max-width: 700px;
        width: 100%;
    }
    .diagram-left, .diagram-center, .diagram-right {
        display: flex;
        flex-direction: column;
        align-items: center;
    }
    .diagram-node {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 0.4rem;
    }
    .diagram-nokia {
        width: 60px;
        height: auto;
        filter: drop-shadow(0 2px 8px rgba(0,0,0,0.5));
    }
    .diagram-node-label {
        font-size: 0.75rem;
        color: rgba(255, 255, 255, 0.72);
        text-shadow: 0 1px 12px rgba(0, 0, 0, 0.28);
        white-space: nowrap;
    }
    .diagram-lf-icon {
        width: 48px;
        height: 48px;
        border-radius: 12px;
    }
    .diagram-center-group {
        display: flex;
        align-items: center;
        position: relative;
        align-self: center;
    }
    .diagram-left-line {
        width: 80px;
        height: 24px;
        flex-shrink: 0;
    }
    .diagram-edge-label {
        position: absolute;
        bottom: -14px;
        left: 20px;
        font-size: 0.65rem;
        color: #60a5ff;
        font-weight: 800;
        text-shadow: 0 1px 12px rgba(0, 0, 0, 0.3);
        white-space: nowrap;
    }
    .diagram-lf-wrapper {
        flex-shrink: 0;
        margin-top: 8px;
    }
    .diagram-right-group {
        display: flex;
        align-items: center;
        gap: 0;
    }
    .diagram-fan-svg {
        width: 70px;
        height: 130px;
        flex-shrink: 0;
    }
    .diagram-apps-list {
        display: flex;
        flex-direction: column;
        gap: 0.15rem;
    }
    .diagram-app-row {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        height: 1.55rem;
    }
    .diagram-app-row i {
        font-size: 1.1rem;
        color: rgba(255, 255, 255, 0.78);
        width: 1.3rem;
        text-align: center;
        flex-shrink: 0;
        filter: drop-shadow(0 1px 8px rgba(0, 0, 0, 0.2));
    }
    .diagram-app-row span {
        font-size: 0.8rem;
        color: rgba(255, 255, 255, 0.76);
        font-weight: 700;
        text-shadow: 0 1px 12px rgba(0, 0, 0, 0.28);
        white-space: nowrap;
    }
    @media (max-width: 768px) {
        .hero-diagram {
            gap: 0;
            max-width: 100%;
            padding: 0 0.5rem;
        }
        .diagram-nokia {
            width: 40px;
        }
        .diagram-lf-icon {
            width: 36px;
            height: 36px;
        }
        .diagram-left-line {
            width: 40px;
        }
        .diagram-fan-svg {
            width: 40px;
            height: 110px;
        }
        .diagram-app-row span {
            font-size: 0.7rem;
        }
        .diagram-app-row i {
            font-size: 0.9rem;
        }
        .diagram-app-row {
            height: 1.3rem;
        }
    }
    /* Testimonial metric */
    .testimonial-metric {
        display: flex;
        align-items: center;
        justify-content: center;
        gap: 0.5rem;
        margin-bottom: 1rem;
    }
    .testimonial-metric-number {
        font-size: 1.5rem;
        font-weight: 700;
        color: var(--landing-blue);
    }
    .testimonial-metric-label {
        font-size: 0.85rem;
        color: var(--landing-faint);
    }
    /* Visual proof section */
    .word-section {
        min-height: 100vh;
        padding: 0 2rem !important;
        background: transparent !important;
        border-top: 0 !important;
        text-align: center;
    }
    .word-section h2,
    .fight-copy h2,
    .app-bridge-copy h2 {
        color: #fff;
        font-size: 7rem;
        line-height: 0.95;
        margin: 0;
        font-weight: 900;
        overflow-wrap: anywhere;
    }
    .landing-page > section.child-word-section {
        background: transparent !important;
    }
    .landing-page > section.child-word-section h2 {
        color: #111;
    }
    .fight-section {
        min-height: 100vh;
        padding: 0 2rem !important;
        background: transparent !important;
        border-top: 0 !important;
        text-align: center;
    }
    .fight-copy {
        max-width: 980px;
        margin: 0 auto;
    }
    .fight-copy p {
        margin: 1.4rem auto 0;
        color: var(--landing-text);
        font-size: 1.25rem;
        line-height: 1.45;
        font-weight: 650;
    }
    .scroll-proof-section {
        padding: 5rem 2rem;
        gap: 2.5rem;
        background: transparent;
        position: relative;
        z-index: 2;
    }
    .scroll-proof-copy {
        max-width: 760px;
        margin: 0 auto;
        text-align: center;
    }
    .section-eyebrow {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        color: var(--landing-blue);
        font-size: 0.86rem;
        font-weight: 700;
        text-transform: uppercase;
        letter-spacing: 0;
        margin-bottom: 0.8rem;
    }
    .scroll-proof-copy h2 {
        color: #fff;
        font-size: clamp(2rem, 5vw, 4rem);
        line-height: 1.05;
        margin: 0;
        font-weight: 800;
    }
    .scroll-proof-copy p {
        max-width: 560px;
        margin: 1rem auto 0;
        color: var(--landing-muted);
        font-size: 1rem;
        line-height: 1.6;
    }
    .proof-flow {
        display: grid;
        grid-template-columns: minmax(0, 1.05fr) minmax(200px, 0.58fr) minmax(0, 0.86fr);
        gap: 1rem;
        align-items: stretch;
        width: 100%;
        max-width: 1216px;
        margin: 0 auto;
    }
    .proof-card {
        min-height: 390px;
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 8px;
        background: rgba(255, 255, 255, 0.035);
        overflow: hidden;
        position: relative;
        display: flex;
        flex-direction: column;
    }
    .proof-step {
        color: #fff;
        font-size: 0.9rem;
        font-weight: 700;
        padding: 1rem 1rem 0;
    }
    .signal-window {
        margin: 1rem;
        padding: 1rem;
        border-radius: 8px;
        background: #050505;
        min-height: 300px;
        display: flex;
        flex-direction: column;
        justify-content: center;
        gap: 0.8rem;
    }
    .signal-bubble {
        align-self: flex-start;
        max-width: 82%;
        padding: 0.82rem 1rem;
        border-radius: 18px;
        background: #3a3a3a;
        color: #f3f3f3;
        font-size: 0.98rem;
        line-height: 1.35;
    }
    .signal-bubble-wide {
        max-width: 96%;
    }
    .proof-signal-shot {
        width: calc(100% - 2rem);
        height: 330px;
        object-fit: contain;
        object-position: center;
        border-radius: 8px;
        margin: 1rem;
        padding: 0.4rem;
        border: 1px solid rgba(255, 255, 255, 0.12);
        background: #050505;
    }
    .proof-decision-card {
        justify-content: space-between;
        align-items: center;
        text-align: center;
        padding-bottom: 1rem;
    }
    .decision-core {
        display: grid;
        place-items: center;
        gap: 0.9rem;
        margin: auto 0;
    }
    .decision-core img {
        width: 68px;
        height: 68px;
        border-radius: 16px;
    }
    .decision-core span {
        color: #fff;
        font-size: 1.7rem;
        font-weight: 800;
    }
    .decision-actions {
        display: flex;
        gap: 0.6rem;
        justify-content: center;
        flex-wrap: wrap;
    }
    .decision-actions span {
        color: #0d0d0d;
        background: #f0f0f0;
        border-radius: 999px;
        padding: 0.45rem 0.8rem;
        font-size: 0.82rem;
        font-weight: 800;
    }
    .proof-phone-card {
        background: #050505;
    }
    .proof-phone-shot {
        width: calc(100% - 2rem);
        height: 330px;
        object-fit: cover;
        object-position: top center;
        border-radius: 8px;
        margin: 1rem;
        border: 1px solid rgba(255, 255, 255, 0.12);
        background: #000;
    }
    .choice-section {
        padding: 4.5rem 2rem;
        background: transparent;
        position: relative;
        z-index: 2;
    }
    .choice-section h2 {
        font-size: clamp(2.4rem, 6vw, 5rem);
        line-height: 1;
        margin: 0 0 2rem;
        color: #fff;
        font-weight: 850;
        text-align: center;
    }
    .choice-grid {
        display: grid;
        grid-template-columns: repeat(3, minmax(0, 1fr));
        gap: 1rem;
        width: 100%;
        max-width: 880px;
        margin: 0 auto;
    }
    .choice-card {
        min-height: 190px;
        display: flex;
        flex-direction: column;
        justify-content: center;
        gap: 0.35rem;
        padding: 1.4rem;
        border: 1px solid rgba(255, 255, 255, 0.22);
        border-radius: 8px;
        background: rgba(255, 255, 255, 0.105);
        box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.18), 0 20px 60px rgba(20, 36, 48, 0.12);
        backdrop-filter: blur(14px) saturate(1.1);
        -webkit-backdrop-filter: blur(14px) saturate(1.1);
    }
    .choice-label {
        color: var(--landing-blue);
        font-size: 0.82rem;
        font-weight: 800;
        text-transform: uppercase;
        margin-bottom: 0.6rem;
    }
    .choice-card strong {
        color: #fff;
        font-size: clamp(1.55rem, 3vw, 2.25rem);
        line-height: 1.05;
        text-shadow: 0 2px 20px rgba(0, 0, 0, 0.32);
    }
    .choice-card .choice-bad {
        color: rgba(255, 255, 255, 0.64);
    }
    .choice-card-primary {
        border-color: rgba(126, 178, 255, 0.54);
        background: rgba(126, 178, 255, 0.13);
    }
    .app-bridge-section {
        min-height: 100vh;
        padding: 4rem 2rem 5rem;
        background: transparent !important;
        position: relative;
        z-index: 2;
        gap: 1.8rem;
    }
    .app-bridge-copy {
        max-width: 920px;
        margin: 0 auto;
        text-align: center;
    }
    .app-bridge-copy h2 {
        font-size: 5.7rem;
    }
    .app-bridge-copy p {
        max-width: 720px;
        margin: 1.4rem auto 0;
        color: var(--landing-text);
        font-size: 1.18rem;
        line-height: 1.5;
        font-weight: 760;
        text-shadow: var(--landing-text-shadow);
    }
    .app-bridge-visual {
        margin: 1.2rem auto 0;
        padding: 2rem;
        border-radius: 8px;
        border: 1px solid rgba(255, 255, 255, 0.2);
        background: rgba(255, 255, 255, 0.085);
        box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.16), 0 20px 70px rgba(20, 36, 48, 0.14);
        backdrop-filter: blur(12px) saturate(1.08);
        -webkit-backdrop-filter: blur(12px) saturate(1.08);
        max-width: 760px;
        box-sizing: border-box;
    }
    @media (max-width: 900px) {
        .proof-flow {
            grid-template-columns: 1fr;
            max-width: 520px;
        }
        .proof-card {
            min-height: auto;
        }
        .signal-window {
            min-height: 260px;
        }
        .proof-phone-shot {
            height: 360px;
        }
        .proof-signal-shot {
            height: 260px;
        }
    }
    @media (max-width: 768px) {
        .word-section h2,
        .fight-copy h2 {
            font-size: 3.7rem;
        }
        .app-bridge-copy h2 {
            font-size: 3rem;
        }
        .fight-copy p,
        .app-bridge-copy p {
            font-size: 1.02rem;
        }
        .scroll-proof-section,
        .choice-section,
        .app-bridge-section {
            padding: 3.5rem 1rem;
        }
        .scroll-proof-copy p {
            font-size: 0.95rem;
        }
        .app-bridge-visual {
            padding: 1rem;
            width: 100%;
        }
        .choice-grid {
            grid-template-columns: 1fr;
        }
        .choice-card {
            min-height: 150px;
        }
    }
    /* Capabilities section */
    .capabilities-section {
        padding: 5rem 2rem;
        background: transparent;
        border-top: 0;
        position: relative;
        z-index: 2;
    }
    .capabilities-title {
        font-size: 2rem;
        color: #fff;
        font-weight: 700;
        text-align: center;
        margin-bottom: 2rem;
    }
    .capabilities-grid {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 1rem;
        max-width: 750px;
        margin: 0 auto;
    }
    .cap-card {
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 12px;
        overflow: hidden;
        cursor: pointer;
        transition: all 0.3s ease;
    }
    .cap-card:hover {
        border-color: rgba(255, 255, 255, 0.25);
    }
    .cap-card.open {
        border-color: rgba(126, 178, 255, 0.3);
    }
    .cap-card-header {
        padding: 1.2rem;
        display: flex;
        align-items: flex-start;
        gap: 0.8rem;
    }
    .cap-card-header i {
        font-size: 1.2rem;
        color: var(--landing-blue);
        margin-top: 0.15rem;
        flex-shrink: 0;
    }
    .cap-card-text {
        display: flex;
        flex-direction: column;
        gap: 0.3rem;
        flex: 1;
        min-width: 0;
    }
    .cap-card-title {
        font-size: 1rem;
        font-weight: 600;
        color: #fff;
    }
    .cap-card-liner {
        font-size: 0.85rem;
        color: var(--landing-muted);
        line-height: 1.5;
    }
    .cap-card-toggle {
        font-size: 1.3rem;
        color: var(--landing-blue);
        flex-shrink: 0;
        margin-left: 0.5rem;
    }
    .cap-card-detail {
        max-height: 0;
        overflow: hidden;
        transition: max-height 0.5s ease;
        padding: 0 1.2rem;
    }
    .cap-card.open .cap-card-detail {
        max-height: 500px;
        padding: 0 1.2rem 1.2rem;
    }
    .cap-card-detail p {
        font-size: 0.9rem;
        color: var(--landing-muted);
        line-height: 1.6;
        margin-bottom: 0.5rem;
    }
    .capabilities-footnote {
        text-align: center;
        color: var(--landing-faint);
        font-size: 0.85rem;
        margin-top: 1.5rem;
        font-style: italic;
    }
    @media (max-width: 768px) {
        .capabilities-section {
            padding: 3rem 1.5rem;
        }
        .capabilities-title {
            font-size: 1.5rem;
        }
        .capabilities-grid {
            grid-template-columns: 1fr;
        }
    }
    .landing-pricing-section {
        padding: 5rem 2rem;
        background: transparent;
        border-top: 0;
        position: relative;
        z-index: 2;
    }
    .landing-pricing-section .stripe-pricing-table-wrap {
        width: 100%;
        max-width: 1120px;
        margin: 0 auto;
    }
    .landing-pricing-section .stripe-pricing-table-wrap stripe-pricing-table {
        display: block;
        width: 100%;
    }
    .stripe-pricing-loading,
    .stripe-pricing-error {
        min-height: 160px;
        display: grid;
        place-items: center;
        color: var(--landing-muted);
        font-size: 0.95rem;
    }
    .stripe-pricing-error {
        color: #ffb4a8;
    }
    @media (max-width: 768px) {
        .landing-pricing-section {
            padding: 3rem 1rem;
        }
    }
    /* Privacy hook section */
    .privacy-hook {
        padding: 5rem 2rem;
        text-align: center;
        background: transparent;
        border-top: 0;
        position: relative;
        z-index: 2;
    }
    .privacy-hook-title {
        font-size: 2rem;
        color: #fff;
        font-weight: 700;
        margin-bottom: 1rem;
        max-width: 700px;
        margin-left: auto;
        margin-right: auto;
    }
    .privacy-hook-subtitle {
        font-size: 1.05rem;
        color: var(--landing-muted);
        line-height: 1.6;
        max-width: 600px;
        margin: 0 auto 2rem;
    }
    .privacy-hook-links {
        display: flex;
        justify-content: center;
        gap: 2rem;
        flex-wrap: wrap;
    }
    .privacy-hook-link {
        color: var(--landing-blue);
        text-decoration: none;
        font-size: 0.95rem;
        font-weight: 500;
        transition: color 0.3s ease;
    }
    .privacy-hook-link:hover {
        color: #c6ddff;
    }
    @media (max-width: 768px) {
        .privacy-hook {
            padding: 3rem 1.5rem;
        }
        .privacy-hook-title {
            font-size: 1.5rem;
        }
        .privacy-hook-subtitle {
            font-size: 0.95rem;
        }
        .privacy-hook-links {
            gap: 1.2rem;
        }
    }
    /* Cost hook section */
    .cost-hook {
        padding: 4rem 2rem;
        text-align: center;
        background: #0d0d0d;
        position: relative;
        z-index: 2;
    }
    .cost-hook-label {
        font-size: 1rem;
        color: var(--landing-faint);
        text-transform: uppercase;
        letter-spacing: 0.1em;
        margin-bottom: 0.5rem;
    }
    .cost-hook-number {
        font-size: 4rem;
        font-weight: 800;
        color: #fff;
        margin: 0.3rem 0;
    }
    .cost-hook-toggle {
        background: none;
        border: none;
        color: var(--landing-blue);
        font-size: 0.9rem;
        cursor: pointer;
        padding: 0.5rem 0;
        margin-top: 0.5rem;
        transition: color 0.3s ease;
    }
    .cost-hook-toggle:hover {
        color: #a8ccff;
    }
    .cost-hook-breakdown {
        max-width: 700px;
        margin: 2rem auto 0;
        text-align: left;
    }
    .cost-hook-item {
        padding: 1.2rem 0;
        border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    }
    .cost-hook-item h3 {
        font-size: 1.1rem;
        color: #fff;
        margin-bottom: 0.5rem;
        font-weight: 600;
    }
    .cost-hook-item p {
        font-size: 0.9rem;
        color: var(--landing-muted);
        line-height: 1.7;
        margin: 0;
    }
    .cost-hook-source {
        font-size: 0.7rem;
        color: #555;
        display: block;
        margin-top: 0.4rem;
    }
    .cost-hook-footer {
        font-size: 0.85rem;
        color: #777;
        text-align: center;
        margin-top: 1.5rem;
        font-style: italic;
    }
    @media (max-width: 768px) {
        .cost-hook-number {
            font-size: 3rem;
        }
        .cost-hook {
            padding: 3rem 1.5rem;
        }
    }
    /* Image carousel section */
    .image-carousel-section {
        padding: 2rem 0;
        margin: 0 auto;
        max-width: 900px;
        position: relative;
        z-index: 2;
        background: #0d0d0d;
    }
    .carousel-container {
        width: 100%;
        aspect-ratio: 3 / 2;
        overflow: hidden;
        border-radius: 8px;
        background: #111;
        display: flex;
        align-items: center;
        justify-content: center;
    }
    .carousel-image {
        width: 100%;
        height: 100%;
        object-fit: cover;
        filter: grayscale(100%);
        transition: opacity 0.6s ease;
    }
    .carousel-dots {
        display: flex;
        justify-content: center;
        gap: 0.5rem;
        margin-top: 1rem;
    }
    .carousel-dot {
        width: 8px;
        height: 8px;
        border-radius: 50%;
        border: 1px solid rgba(255, 255, 255, 0.3);
        background: transparent;
        cursor: pointer;
        padding: 0;
        transition: all 0.3s ease;
    }
    .carousel-dot.active {
        background: #fff;
        border-color: #fff;
    }
    .carousel-dot:hover {
        border-color: rgba(255, 255, 255, 0.6);
    }
    @media (max-width: 768px) {
        .image-carousel-section {
            padding: 1rem;
        }
        .carousel-container {
            border-radius: 4px;
        }
    }
    .cta-image-container {
        max-width: 300px;
        margin: 0 auto;
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 1rem;
        position: relative;
        padding: 0 2rem;
    }
    .filter-concept {
        padding: 4rem 4rem;
        margin: 0 auto;
        max-width: 1200px;
        position: relative;
        z-index: 2;
        background: transparent;
        text-align: center;
    }
    .filter-concept h2 {
        font-size: 2.5rem;
        margin-bottom: 1rem;
        color: #fff;
    }
    .filter-concept-subtitle {
        font-size: 1.2rem;
        color: var(--landing-muted);
        line-height: 1.6;
        max-width: 700px;
        margin: 0 auto 2rem;
    }
    .integrations-row {
        margin-top: 3rem;
        padding-top: 2rem;
        border-top: 1px solid rgba(255, 255, 255, 0.06);
    }
    .integration-buttons {
        display: flex;
        justify-content: center;
        gap: 1rem;
        flex-wrap: wrap;
    }
    .integration-btn {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 0.4rem;
        padding: 0.8rem 1.2rem;
        background: rgba(255, 255, 255, 0.04);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 12px;
        cursor: pointer;
        transition: all 0.3s ease;
        color: var(--landing-faint);
    }
    .integration-btn:hover {
        border-color: rgba(255, 255, 255, 0.25);
        color: rgba(255, 255, 255, 0.8);
        background: rgba(255, 255, 255, 0.07);
    }
    .integration-btn.active {
        border-color: rgba(126, 178, 255, 0.5);
        color: var(--landing-blue);
        background: rgba(126, 178, 255, 0.08);
    }
    .integration-btn i {
        font-size: 1.6rem;
    }
    .integration-label {
        font-size: 0.75rem;
        font-weight: 500;
        letter-spacing: 0.02em;
    }
    .integration-detail {
        max-height: 0;
        overflow: hidden;
        transition: max-height 0.4s ease, padding 0.4s ease;
        padding: 0 1rem;
    }
    .integration-detail.visible {
        max-height: 200px;
        padding: 1.5rem 1rem 0.5rem;
    }
    .integration-detail-content {
        text-align: center;
        max-width: 500px;
        margin: 0 auto;
    }
    .integration-detail-content h3 {
        font-size: 1.2rem;
        color: #fff;
        margin-bottom: 0.5rem;
    }
    .integration-detail-content p {
        font-size: 1rem;
        color: var(--landing-muted);
        line-height: 1.6;
    }
    @media (max-width: 768px) {
        .integration-buttons {
            gap: 0.6rem;
        }
        .integration-btn {
            padding: 0.6rem 0.8rem;
        }
        .integration-btn i {
            font-size: 1.3rem;
        }
        .integration-label {
            font-size: 0.65rem;
        }
    }
    .filter-content {
        display: flex;
        align-items: center;
    }
    .filter-text {
        flex: 1;
    }
    .filter-text h2 {
        font-size: 2.5rem;
        margin-bottom: 1.5rem;
        color: #fff;
    }
    .filter-image {
        flex: 1;
        display: flex;
        justify-content: center;
        align-items: center;
    }
    .filter-image img {
        max-width: 100%;
        height: auto;
        border-radius: 12px;
        box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
    }
    .faq-in-filter {
        max-width: 800px;
        margin: 0 auto;
        padding: 2rem 0;
    }
    .faq-in-filter h2 {
        font-size: 2.5rem;
        margin-bottom: 1.5rem;
        color: #fff;
        text-align: center;
    }
    .trust-proof {
        padding: 4rem 2rem;
        max-width: 800px;
        margin: 0 auto;
        text-align: center;
        position: relative;
        z-index: 2;
    }
    .trust-proof::before {
        content: none;
    }
    .trust-proof h2 {
        font-size: 2.5rem;
        margin-bottom: 1.5rem;
        color: #fff;
        font-weight: 700;
    }
    .trust-proof p {
        font-size: 1.3rem;
        color: var(--landing-muted);
        line-height: 1.8;
        font-weight: 400;
        margin-bottom: 1.5rem;
    }
    @media (max-width: 768px) {
        .trust-proof h2 {
            font-size: 2rem;
        }
        .trust-proof p {
            font-size: 1.1rem;
        }
    }
    .landing-faq-item {
        margin-bottom: 0.75rem;
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 12px;
        overflow: hidden;
        transition: all 0.3s ease;
    }
    .landing-faq-item:hover {
        border-color: rgba(255, 255, 255, 0.25);
    }
    .landing-faq-question {
        width: 100%;
        padding: 1.2rem 1.5rem;
        background: none;
        border: none;
        color: #fff;
        font-size: 1.15rem;
        text-align: left;
        cursor: pointer;
        display: flex;
        justify-content: space-between;
        align-items: center;
        transition: color 0.3s ease;
    }
    .landing-faq-question:hover {
        color: var(--landing-blue);
    }
    .landing-faq-question .toggle-icon {
        font-size: 1.5rem;
        color: var(--landing-blue);
        flex-shrink: 0;
        margin-left: 1rem;
    }
    .landing-faq-answer {
        max-height: 0;
        overflow: hidden;
        transition: max-height 0.5s ease;
        padding: 0 1.5rem;
    }
    .landing-faq-item.open .landing-faq-answer {
        max-height: 1000px;
        padding: 0 1.5rem 1.2rem;
    }
    .landing-faq-answer p {
        font-size: 1rem;
        color: var(--landing-muted);
        line-height: 1.6;
        margin-bottom: 0.75rem;
    }
    .landing-faq-answer ul {
        list-style: none;
        padding: 0;
        margin: 0.5rem 0;
    }
    .landing-faq-answer li {
        color: var(--landing-muted);
        padding: 0.4rem 0;
        padding-left: 1.2rem;
        position: relative;
        font-size: 0.95rem;
        line-height: 1.5;
    }
    .landing-faq-answer li::before {
        content: '\2022';
        position: absolute;
        left: 0.3rem;
        color: var(--landing-blue);
    }
    .landing-faq-answer a {
        color: var(--landing-blue);
        text-decoration: none;
    }
    .landing-faq-answer a:hover {
        color: #a8ccff;
    }
    .faq-more-link {
        margin-top: 1.5rem;
        text-align: center;
    }
    .faq-more-link a {
        color: var(--landing-blue);
        text-decoration: none;
        font-size: 1rem;
    }
    .faq-more-link a:hover {
        color: #a8ccff;
    }
    @media (max-width: 768px) {
        .filter-concept {
            padding: 2rem;
        }
        .filter-concept h2 {
            font-size: 2rem;
        }
        .filter-concept-subtitle {
            font-size: 1.05rem;
        }
        .filter-content {
            flex-direction: column;
            min-height: 1000px;
            padding: 2rem;
            gap: 2rem;
            text-align: center;
        }
        .filter-text h2 {
            font-size: 2rem;
        }
        .faq-in-filter h2 {
            font-size: 2rem;
        }
        .landing-faq-question {
            font-size: 1rem;
            padding: 1rem 1.2rem;
        }
        .landing-faq-answer {
            padding: 0 1.2rem;
        }
        .landing-faq-item.open .landing-faq-answer {
            padding: 0 1.2rem 1rem;
        }
    }
    .dual-section {
        padding: 4rem 2rem;
        margin: 0 auto;
        max-width: 1200px;
        position: relative;
        z-index: 2;
    }
    .dual-section-grid {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 2.5rem;
        align-items: start;
    }
    .dual-section-card {
        background: rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.15);
        border-radius: 20px;
        padding: 2.5rem;
    }
    .dual-section-card h2 {
        font-size: 1.8rem;
        margin-bottom: 1rem;
        color: #fff;
    }
    .dual-section-card p {
        font-size: 1rem;
        color: rgba(255, 255, 255, 0.7);
        line-height: 1.7;
        margin-bottom: 0.8rem;
    }
    .dual-section-card .why-lead {
        font-size: 1.1rem;
        font-weight: 500;
        color: rgba(255, 255, 255, 0.85);
        font-style: italic;
    }
    .dual-section-image {
        margin-top: 1.5rem;
    }
    .dual-section-image img {
        width: 100%;
        border-radius: 12px;
    }
    .dual-section-demo {
        display: flex;
        flex-direction: column;
        align-items: center;
    }
    .dual-section-demo .phone-frame {
        max-width: 320px;
        width: 100%;
    }
    .dual-section-notification {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        text-align: center;
    }
    .dual-section-notification .hero-notification-img {
        max-width: 100%;
        border-radius: 12px;
        margin-top: 1rem;
    }
    .rasmus-pfp {
        max-width: 120px;
        border-radius: 50%;
        margin-bottom: 1rem;
    }
    @media (max-width: 768px) {
        .dual-section-grid {
            grid-template-columns: 1fr;
        }
    }
    .difference-section {
        padding: 4rem 2rem;
        margin: 0 auto;
        max-width: 1200px;
        position: relative;
        z-index: 2;
    }
    .difference-content {
        display: flex;
        align-items: center;
        gap: 4rem;
        background: transparent;
        border: none;
        border-radius: 24px;
        padding: 3rem;
        transition: transform 0.3s ease, box-shadow 0.3s ease;
    }
    .difference-content:hover {
        border-color: rgba(255, 255, 255, 0.1);
    }
    .difference-text {
        flex: 1;
    }
    .difference-text h2 {
        font-size: 2.5rem;
        margin-bottom: 1.5rem;
        color: #fff;
    }
    .difference-text p {
        font-size: 1.4rem;
        color: var(--landing-muted);
        line-height: 1.8;
        font-weight: 400;
    }
    .comparison-table {
        margin-top: 2rem;
        overflow-x: auto;
    }
    .comparison-table h3 {
        font-size: 1.8rem;
        text-align: center;
        color: #fff;
    }
    .comparison-table p {
        text-align: center;
        color: var(--landing-text);
        margin-bottom: 1.5rem;
    }
    .comparison-table table {
        width: 100%;
        border-collapse: collapse;
        margin: 0 auto;
        font-size: 1rem;
        color: var(--landing-text);
    }
    .comparison-table th, .comparison-table td {
        padding: 1rem;
        text-align: left;
        border-bottom: 1px solid rgba(255, 255, 255, 0.2);
    }
    .comparison-table th {
        background: rgba(0, 0, 0, 0.5);
        color: var(--landing-blue);
    }
    .comparison-table tr:hover {
        background: rgba(255, 255, 255, 0.1);
    }
    @media (max-width: 768px) {
        .comparison-table table {
            font-size: 0.9rem;
        }
        .comparison-table th, .comparison-table td {
            padding: 0.75rem;
        }
    }
    .highlight {
        font-weight: 700;
        color: var(--landing-blue);
    }
    .difference-image {
        flex: 1;
        display: flex;
        justify-content: center;
        align-items: center;
    }
    .difference-image img {
        max-width: 100%;
        height: auto;
        border-radius: 12px;
        box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
        border: 1px solid rgba(255, 255, 255, 0.1);
        transition: all 0.3s ease;
    }
    .difference-image img:hover {
        border-color: rgba(255, 255, 255, 0.25);
    }
    @media (max-width: 768px) {
        .difference-section {
            padding: 2rem 1rem;
        }
        .difference-content {
            flex-direction: column;
            padding: 2rem;
            gap: 2rem;
            text-align: center;
        }
        .difference-text h2 {
            font-size: 2rem;
        }
        .difference-text p {
            font-size: 1.2rem;
        }
    }
    .landing-page {
        --landing-text-shadow: 0 2px 18px rgba(0, 0, 0, 0.46), 0 1px 2px rgba(0, 0, 0, 0.42);
        --landing-soft-shadow: 0 1px 14px rgba(0, 0, 0, 0.34);
        --landing-text: rgba(255, 255, 255, 0.96);
        --landing-muted: rgba(255, 255, 255, 0.84);
        --landing-faint: rgba(255, 255, 255, 0.70);
        --landing-blue: #8fc5ff;
        position: relative;
        min-height: 100vh;
        background: #0d0d0d;
        color: #ffffff;
        font-family: system-ui, -apple-system, sans-serif;
        margin: 0 auto;
        width: 100%;
        overflow-x: clip;
        box-sizing: border-box;
        z-index: 0;
        isolation: isolate;
    }
    .landing-page::before {
        content: '';
        position: fixed;
        inset: 0;
        background-image: url('/assets/child-field-hero.png');
        background-size: cover;
        background-position: center;
        background-repeat: no-repeat;
        z-index: 0;
        pointer-events: none;
    }
    .landing-page::after {
        content: '';
        position: fixed;
        inset: 0;
        background: linear-gradient(to bottom,
            rgba(13, 13, 13, 0.08) 0%,
            rgba(13, 13, 13, 0.18) 35%,
            rgba(13, 13, 13, 0.74) 100%
        );
        z-index: 0;
        pointer-events: none;
    }
    .landing-page h1,
    .landing-page h2,
    .landing-page h3,
    .landing-page p,
    .landing-page li,
    .landing-page blockquote,
    .landing-page strong,
    .landing-page .choice-label,
    .landing-page .landing-faq-question,
    .landing-page .story-link,
    .landing-page .privacy-hook-link,
    .landing-page .development-links a,
    .landing-page .legal-links a {
        text-shadow: var(--landing-text-shadow);
    }
    .landing-page > header,
    .landing-page > section,
    .landing-page > footer,
    .landing-page > .filter-concept {
        position: relative;
        z-index: 1;
    }
    .landing-page > section,
    .landing-page > footer {
        display: flex;
        flex-direction: column;
        justify-content: center;
        align-items: center;
        padding-top: 3rem;
        padding-bottom: 3rem;
        box-sizing: border-box;
        background: transparent;
        border-top: 0;
    }
    .landing-page > section:first-of-type {
        border-top: none;
    }
    .main-features {
        max-width: 1200px;
        margin: 0 auto;
        padding: 0 2rem;
        position: relative;
        z-index: 3;
        background: transparent;
        opacity: 1;
        pointer-events: auto;
        margin-top: -30vh;
    }
    .feature-block {
        display: flex;
        align-items: center;
        gap: 4rem;
        background: transparent;
        border: none;
        border-radius: 24px;
        padding: 3rem;
        z-index: 3;
        transition: transform 1.8s cubic-bezier(0.4, 0, 0.2, 1),
                    border-color 1.8s ease,
                    box-shadow 1.8s ease;
        overflow: hidden;
        position: relative;
        margin: 10%;
        margin-bottom: 180vh;
    }
    .feature-block:hover {
        border-color: rgba(255, 255, 255, 0.15);
    }
    .feature-image {
        flex: 1;
        display: flex;
        justify-content: center;
        align-items: center;
    }
    .feature-image img {
        max-width: 100%;
        height: auto;
        border-radius: 12px;
        box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
    }
    .demo-link-container {
        margin-top: 2rem;
        display: flex;
        justify-content: center;
    }
    .demo-link {
        display: inline-flex;
        align-items: center;
        gap: 0.5rem;
        padding: 0.8rem 1.5rem;
        background: linear-gradient(
            45deg,
            #7EB2FF,
            #4169E1
        );
        color: white;
        text-decoration: none;
        border-radius: 8px;
        font-size: 1rem;
        transition: all 0.3s ease;
    }
    .demo-link:hover {
        transform: translateY(-2px);
        box-shadow: 0 4px 20px rgba(255, 255, 255, 0.3);
    }
    @media (max-width: 1024px) {
        .feature-block {
            flex-direction: column;
            padding: 2rem;
            gap: 2rem;
            margin-bottom: 50vh;
        }
        .feature-image {
            order: -1;
        }
    }
    @media (max-width: 768px) {
        .landing-page {
            padding: 0;
        }
        .hero-subtitle {
            font-size: 1rem;
            padding: 0 1rem;
        }
        .how-it-works {
            padding: 0 3rem;
        }
        .how-it-works h2 {
            font-size: 1.75rem;
        }
        .steps-grid {
            grid-template-columns: 1fr;
            gap: 1.5rem;
            padding: 0 1rem;
        }
        .footer-cta {
            padding: 3rem 1rem;
        }
        .footer-cta h2 {
            font-size: 2rem;
        }
        .footer-cta .subtitle {
            font-size: 1rem;
        }
        .footer-content {
            padding: 0 1rem;
        }
        .development-links {
            padding: 0 1rem;
        }
    }
    .how-it-works {
        padding: 2rem 2rem;
        text-align: center;
        position: relative;
        z-index: 1;
        margin-top: 0;
    }
    .how-it-works::before {
        content: none;
    }
    .how-it-works * {
        pointer-events: auto;
    }
    .how-it-works h2 {
        font-size: 3rem;
        margin-bottom: 1rem;
    }
    .how-it-works > p {
        color: var(--landing-blue);
        margin-bottom: 4rem;
        font-size: 1.2rem;
    }
    .steps-grid {
        display: grid;
        grid-template-columns: repeat(3, 1fr);
        gap: 2rem;
        margin-top: 4rem;
    }
    .step {
        background: transparent;
        border-radius: 16px;
        padding: 2.5rem;
        border: none;
        backdrop-filter: none;
        transition: all 0.3s ease;
        position: relative;
        overflow: hidden;
    }
    .step::before {
        content: '';
        position: absolute;
        top: 0;
        left: 0;
        right: 0;
        height: 1px;
        background: linear-gradient(
            90deg,
            transparent,
            rgba(255, 255, 255, 0.3),
            transparent
        );
    }
    .step:hover {
        border-color: rgba(255, 255, 255, 0.2);
    }
    .step h3 {
        color: #1E90FF;
        font-size: 1.5rem;
        margin-bottom: 1.5rem;
        font-weight: 600;
    }
    .step p {
        color: var(--landing-muted);
        font-size: 1rem;
        line-height: 1.6;
    }
    .step::after {
        content: '';
        position: absolute;
        top: 1rem;
        right: 1rem;
        width: 30px;
        height: 30px;
        border-radius: 50%;
        border: 2px solid rgba(255, 255, 255, 0.3);
        display: flex;
        align-items: center;
        justify-content: center;
        font-size: 0.9rem;
        color: #1E90FF;
    }
    .step:nth-child(1)::after {
        content: '1';
    }
    .step:nth-child(2)::after {
        content: '2';
    }
    .step:nth-child(3)::after {
        content: '3';
    }
    .footer-cta {
        padding: 6rem 0;
        background: transparent;
        border-top: 0;
        text-align: left;
        position: relative;
        z-index: 1;
        margin-top: 0;
        pointer-events: auto;
    }
    .footer-cta::before {
        content: none;
    }
    .footer-content {
        max-width: 800px;
        margin: 0 auto;
        padding: 0 2rem;
        width: 100%;
        box-sizing: border-box;
    }
    .footer-cta h2 {
        font-size: 3.5rem;
        margin-bottom: 1.5rem;
        color: #fff;
        font-weight: 700;
    }
    .footer-cta .subtitle {
        font-size: 1.2rem;
        color: var(--landing-muted);
        margin-bottom: 2.5rem;
        line-height: 1.6;
    }
    .hero {
        min-height: 100vh;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: flex-start;
        text-align: center;
        position: relative;
        background: transparent;
        z-index: 1;
    }
    .hero-content {
        z-index: 3;
        width: 100%;
        display: flex;
        flex-direction: column;
        pointer-events: auto;
        padding: 0 2rem;
        box-sizing: border-box;
        overflow-x: clip;
    }
    .hero-notification-card {
        text-align: center;
        max-width: 500px;
        margin: 0 auto;
    }
    .hero-notification-card h2 {
        font-size: 1.6rem;
        margin-bottom: 0.5rem;
        color: #fff;
    }
    .hero-notification-card p {
        font-size: 1rem;
        color: rgba(255, 255, 255, 0.65);
        line-height: 1.6;
        margin-bottom: 1rem;
    }
    .hero-notification-img {
        width: 100%;
        max-width: 400px;
        border-radius: 12px;
    }
    .hero-title {
        font-size: clamp(3rem, 7vw, 6.2rem);
        font-weight: 800;
        color: #fff;
        text-shadow: none;
        margin: 0 auto 0.8rem;
        width: 100%;
        line-height: 0.98;
        text-align: center;
        max-width: 100%;
        overflow-wrap: break-word;
    }
    .hero-title span {
        display: block;
    }
    .hero-word-title {
        font-size: clamp(3.2rem, 8.5vw, 7.25rem);
        line-height: 0.9;
        font-weight: 900;
        margin: 0;
        white-space: normal;
        text-shadow: 0 4px 30px rgba(0, 0, 0, 0.45);
    }
    .hero-kicker {
        color: var(--landing-blue);
        font-size: 0.86rem;
        font-weight: 800;
        text-transform: uppercase;
        letter-spacing: 0;
        margin: 0 0 1rem;
    }
    .hero-tagline {
        font-size: 1.15rem;
        font-weight: 400;
        color: rgba(255, 255, 255, 0.65);
        max-width: 600px;
        margin: 0 auto 0.5rem;
        letter-spacing: 0.03em;
        line-height: 1.6;
        text-align: center;
    }
    .hero-subtitle {
        font-size: clamp(1rem, 1.8vw, 1.28rem);
        font-weight: 500;
        letter-spacing: 0;
        max-width: 640px;
        margin: 0 auto 1.2rem;
        line-height: 1.45;
        text-align: center;
        color: rgba(255, 255, 255, 0.82);
    }
    .hero-pill-row {
        display: flex;
        align-items: center;
        justify-content: center;
        gap: 0.6rem;
        flex-wrap: wrap;
        margin-top: 0.3rem;
        max-width: 100%;
    }
    .hero-pill-row span {
        color: #f5f5f5;
        background: rgba(0, 0, 0, 0.42);
        border: 1px solid rgba(255, 255, 255, 0.13);
        border-radius: 999px;
        padding: 0.48rem 0.76rem;
        font-size: 0.86rem;
        font-weight: 750;
    }
    .highlight-icon {
        font-size: 1.2rem;
        margin: 0 0.2rem;
        color: var(--landing-blue);
        vertical-align: middle;
    }
    @media (max-width: 768px) {
        .hero {
            align-items: center;
        }
        .hero-content {
            padding: 0 1rem;
        }
        .hero-title {
            font-size: clamp(2.5rem, 12vw, 4.4rem);
        }
        .hero-word-title {
            font-size: clamp(3.1rem, 15vw, 4.8rem);
        }
        .hero-tagline {
            font-size: 0.95rem;
        }
        .hero-subtitle {
            font-size: 1rem;
        }
        .hero-pill-row {
            gap: 0.4rem;
        }
        .hero-pill-row span {
            font-size: 0.78rem;
            padding: 0.42rem 0.62rem;
        }
        .highlight-icon {
            font-size: 1rem;
        }
    }
    .hero-cta {
        background: linear-gradient(
            135deg,
            #d4d4d4,
            #a8a8a8 30%,
            #e8e8e8 50%,
            #a8a8a8 70%,
            #c0c0c0
        );
        color: #1a1a2e;
        border: none;
        padding: 1rem 2.5rem;
        border-radius: 8px;
        font-size: 1.1rem;
        font-weight: 600;
        cursor: pointer;
        transition: transform 1.5s cubic-bezier(0.4, 0, 0.2, 1),
                    box-shadow 1.5s ease,
                    background 0.3s ease;
        display: inline-flex;
        align-items: center;
        gap: 0.5rem;
        position: relative;
        overflow: hidden;
        margin: 2rem 0 3rem 0;
        border: 1px solid rgba(255, 255, 255, 0.4);
        box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3), inset 0 1px 0 rgba(255, 255, 255, 0.5);
    }
    @media (min-width: 769px) {
        .hero-cta {
            margin: 3rem 0 3rem 0;
        }
    }
    .hero-cta::before {
        content: '';
        position: absolute;
        top: 0;
        left: 0;
        width: 100%;
        height: 100%;
        background: linear-gradient(
            45deg,
            transparent,
            rgba(255, 255, 255, 0.1),
            transparent
        );
        transform: translateX(-100%);
        transition: transform 0.6s;
    }
    .hero-cta::after {
        content: '→';
    }
    .hero-cta:hover::before {
        transform: translateX(100%);
    }
    .hero-cta:hover {
        transform: translateY(-1px);
        background: linear-gradient(
            135deg,
            #e0e0e0,
            #b8b8b8 30%,
            #f0f0f0 50%,
            #b8b8b8 70%,
            #d0d0d0
        );
    }
    .hero-cta-group {
        display: flex;
        flex-direction: row;
        align-items: center;
        justify-content: center;
        gap: 1rem;
        margin-top: 1rem;
    }
    .hero-metric {
        display: flex;
        align-items: baseline;
        justify-content: center;
        gap: 0.5rem;
        padding: 0.6rem 1rem;
        background: rgba(0, 0, 0, 0.35);
        backdrop-filter: blur(10px);
        border: 1px solid rgba(255, 255, 255, 0.15);
        border-radius: 30px;
        margin-top: 1rem;
        margin-bottom: 4rem;
    }
    .hero-metric-number {
        font-size: 1.3rem;
        font-weight: 700;
        color: var(--landing-blue);
    }
    .hero-metric-label {
        font-size: 0.85rem;
        color: rgba(255, 255, 255, 0.6);
        font-weight: 400;
    }
    @media (max-width: 768px) {
        .hero-metric {
            padding: 0.5rem 0.9rem;
            gap: 0.4rem;
        }
        .hero-metric-number {
            font-size: 1.1rem;
        }
        .hero-metric-label {
            font-size: 0.75rem;
        }
    }

    /* ========== Hero Center Panel Layout ========== */
    .hero-right-panel {
        display: flex;
        flex-direction: column;
        align-items: center;
        margin: 0 auto;
        max-width: 920px;
        width: 100%;
        box-sizing: border-box;
        padding-top: 24vh;
        padding-bottom: 4rem;
        z-index: 3;
    }
    .hero-right-panel .hero-title {
        text-align: center;
    }
    .hero-right-panel .hero-subtitle {
        text-align: center;
    }
    .hero-right-panel .hero-cta-group {
        justify-content: center;
    }
    .hero-right-panel .hero-metric {
        justify-content: center;
    }
    @media (max-width: 768px) {
        .hero-right-panel {
            max-width: 100%;
            padding: 0 1rem;
            padding-top: 22vh;
        }
    }
    .hero-word-panel {
        justify-content: center;
        min-height: 100vh;
        padding-top: 0;
        padding-bottom: 0;
    }
    @media (max-width: 768px) {
        .hero-word-panel {
            padding-top: 0;
            padding-bottom: 0;
        }
    }

    /* ========== SMS Demo Animation ========== */
    .sms-demo {
        background: rgba(0, 0, 0, 0.55);
        backdrop-filter: blur(20px);
        border: 1px solid rgba(255, 255, 255, 0.2);
        border-radius: 24px;
        padding: 1.4rem;
        width: 100%;
        max-width: 420px;
        box-shadow: 0 8px 40px rgba(0, 0, 0, 0.4);
    }
    .sms-demo-header {
        display: flex;
        align-items: center;
        gap: 0.6rem;
        padding: 0.6rem 0.9rem;
        border-bottom: 1px solid rgba(255, 255, 255, 0.1);
        margin-bottom: 1rem;
    }
    .sms-demo-dot {
        width: 10px;
        height: 10px;
        border-radius: 50%;
        background: #4CAF50;
    }
    .sms-demo-name {
        font-size: 1rem;
        color: rgba(255, 255, 255, 0.7);
        font-weight: 500;
    }
    .sms-demo-messages {
        display: flex;
        flex-direction: column;
        gap: 0.8rem;
        min-height: 260px;
    }
    .sms-bubble {
        padding: 0.8rem 1.1rem;
        border-radius: 16px;
        font-size: 0.95rem;
        line-height: 1.5;
        max-width: 85%;
        opacity: 0;
        transform: translateY(10px);
    }
    .sms-user {
        background: rgba(30, 144, 255, 0.25);
        color: #c5dfff;
        align-self: flex-end;
        border-bottom-right-radius: 4px;
        margin-left: auto;
    }
    .sms-assistant {
        background: rgba(255, 255, 255, 0.1);
        color: var(--landing-text);
        align-self: flex-start;
        border-bottom-left-radius: 4px;
    }
    @keyframes smsAppear {
        from { opacity: 0; transform: translateY(10px); }
        to { opacity: 1; transform: translateY(0); }
    }
    @media (max-width: 400px) {
        .sms-demo {
            max-width: 100%;
        }
    }

    /* ========== Hero Particles ========== */
    .hero-particles {
        position: absolute;
        top: 0;
        left: 0;
        width: 100%;
        height: 100%;
        z-index: -1;
        pointer-events: none;
        overflow: hidden;
    }
    .particle {
        position: absolute;
        bottom: -10px;
        border-radius: 50%;
        background: rgba(255, 255, 255, 0.4);
        animation: particleRise linear infinite;
    }
    @keyframes particleRise {
        0% {
            transform: translateY(0) translateX(0);
            opacity: 0;
        }
        10% {
            opacity: 0.6;
        }
        50% {
            transform: translateY(-50vh) translateX(15px);
            opacity: 0.4;
        }
        90% {
            opacity: 0;
        }
        100% {
            transform: translateY(-105vh) translateX(-10px);
            opacity: 0;
        }
    }
    @media (max-width: 768px) {
        .particle:nth-child(n+15) {
            display: none;
        }
    }

    /* ========== Phone Frame ========== */
    .phone-frame {
        background: linear-gradient(145deg, #1a1a1a, #0d0d0d);
        border-radius: 38px;
        padding: 16px;
        position: relative;
        width: 100%;
        max-width: 460px;
        box-shadow:
            0 20px 60px rgba(0, 0, 0, 0.4),
            inset 0 1px 0 rgba(255, 255, 255, 0.08);
        border: 1px solid rgba(255, 255, 255, 0.08);
    }
    .phone-earpiece {
        width: 70px;
        height: 5px;
        background: rgba(255, 255, 255, 0.1);
        border-radius: 4px;
        margin: 8px auto 10px;
    }
    .phone-chin {
        display: flex;
        justify-content: center;
        padding: 10px 0 6px;
    }
    .phone-button {
        width: 32px;
        height: 32px;
        border-radius: 50%;
        border: 2px solid rgba(255, 255, 255, 0.12);
        background: transparent;
    }
    .phone-frame .sms-demo {
        border-radius: 16px;
        border: none;
        box-shadow: none;
        max-width: 100%;
    }
    @media (max-width: 480px) {
        .phone-frame {
            border-radius: 28px;
            padding: 10px;
            max-width: 100%;
        }
        .phone-frame .sms-demo {
            max-width: 100%;
        }
    }

    /* ========== Demo + Story Section ========== */
    .demo-story-section {
        padding: 6rem 2rem;
        margin-top: 6rem;
        position: relative;
        z-index: 2;
        max-width: 1200px;
        margin-left: auto;
        margin-right: auto;
        background: #0d0d0d;
    }
    .demo-story-grid {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 3rem;
        align-items: center;
    }
    .demo-story-left {
        display: flex;
        flex-direction: column;
        align-items: center;
    }
    .demo-story-right {
        text-align: left;
    }
    .demo-story-right h2 {
        font-size: 2rem;
        margin-bottom: 1rem;
        color: #fff;
    }
    .demo-story-right .rasmus-pfp {
        max-width: 100px;
        border-radius: 50%;
        margin-bottom: 1rem;
    }
    .demo-story-right p {
        font-size: 1.05rem;
        color: rgba(255, 255, 255, 0.7);
        line-height: 1.7;
        margin-bottom: 0.8rem;
    }
    .demo-story-right .why-lead {
        font-size: 1.15rem;
        font-weight: 500;
        color: rgba(255, 255, 255, 0.85);
        font-style: italic;
    }
    @media (max-width: 768px) {
        .demo-story-grid {
            grid-template-columns: 1fr;
            gap: 2rem;
        }
        .demo-story-right {
            text-align: center;
        }
    }
    .sms-demo-section-inner {
        display: flex;
        flex-direction: column;
        align-items: center;
        max-width: 500px;
        width: 100%;
    }

    /* ========== SMS Demo Tabs ========== */
    .sms-demo-tabs {
        display: flex;
        justify-content: center;
        gap: 0.6rem;
        margin-bottom: 1.2rem;
        flex-wrap: wrap;
        max-width: 460px;
        width: 100%;
    }
    .sms-tab {
        display: flex;
        align-items: center;
        gap: 0.4rem;
        padding: 0.45rem 0.85rem;
        border: none;
        border-radius: 20px;
        font-size: 0.82rem;
        cursor: pointer;
        transition: all 0.3s ease;
        background: rgba(255, 255, 255, 0.08);
        color: var(--landing-faint);
        font-family: inherit;
    }
    .sms-tab i {
        font-size: 0.8rem;
    }
    .sms-tab.active {
        background: linear-gradient(135deg, #d4d4d4, #a8a8a8 30%, #e8e8e8 50%, #a8a8a8 70%, #c0c0c0);
        color: #1a1a2e;
        box-shadow: 0 2px 10px rgba(200, 200, 200, 0.3), inset 0 1px 0 rgba(255, 255, 255, 0.5);
    }
    .sms-tab:hover:not(.active) {
        background: rgba(255, 255, 255, 0.12);
        color: rgba(255, 255, 255, 0.7);
    }
    @media (max-width: 480px) {
        .sms-demo-tabs {
            gap: 0.3rem;
        }
        .sms-tab {
            font-size: 0.65rem;
            padding: 0.3rem 0.5rem;
        }
    }

    /* ========== Demo Customization Note ========== */
    .demo-customization-note {
        text-align: center;
        font-size: 0.78rem;
        color: var(--landing-faint);
        margin-top: 0.8rem;
        font-style: italic;
    }

    /* ========== Hero Entrance Animation ========== */
    @keyframes heroFadeIn {
        from {
            opacity: 0;
            transform: translateY(20px);
        }
        to {
            opacity: 1;
            transform: translateY(0);
        }
    }
    .hero-anim {
        opacity: 1;
        animation: none;
    }
    .hero-anim-1,
    .hero-anim-2,
    .hero-anim-3,
    .hero-anim-4,
    .hero-anim-5 {
        animation-delay: 0s;
    }

    .faq-link {
        color: var(--landing-blue);
        text-decoration: none;
        font-size: 1rem;
        transition: all 0.3s ease;
        position: relative;
        padding: 0.5rem 1rem;
    }
    .faq-link::after {
        content: '';
        position: absolute;
        width: 100%;
        height: 1px;
        bottom: -2px;
        left: 0;
        background: linear-gradient(90deg, #1E90FF, #4169E1);
        transform: scaleX(0);
        transform-origin: bottom right;
        transition: transform 0.3s ease;
    }
    .faq-link:hover {
        color: #90c2ff;
    }
    .faq-link:hover::after {
        transform: scaleX(1);
        transform-origin: bottom left;
    }
    @media (max-width: 768px) {
        .hero-cta-group {
            gap: 0.75rem;
        }
    }
    .section-header {
        text-align: center;
    }
    .section-intro {
        max-width: 600px;
        margin: 0 auto;
        text-align: center;
        padding: 2rem;
        border-radius: 16px;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
    }
    .section-intro .hero-cta {
        margin: 1rem auto;
        display: block;
    }
    .section-intro .story-link {
        color: var(--landing-blue);
        text-decoration: none;
        transition: color 0.3s ease;
    }
    .section-intro .story-link:hover {
        color: #a8ccff;
    }
    .before-after {
        padding: 4rem 2rem;
        max-width: 800px;
        margin: 0 auto;
        text-align: center;
        position: relative;
        z-index: 2;
    }
    .before-after h2 {
        font-size: 2.5rem;
        margin-bottom: 1.5rem;
        color: #fff;
        font-weight: 700;
    }
    .before-after p {
        font-size: 1.3rem;
        color: var(--landing-muted);
        line-height: 1.8;
        font-weight: 400;
        max-width: 700px;
        margin: 0 auto;
    }
    @media (max-width: 768px) {
        .before-after h2 {
            font-size: 2rem;
        }
        .before-after p {
            font-size: 1.1rem;
        }
    }
    .legal-links {
        margin-top: 1rem;
    }
    .legal-links a {
        color: var(--landing-faint);
        text-decoration: none;
        transition: color 0.3s ease;
    }
    .legal-links a:hover {
        color: var(--landing-blue);
    }
    @media (max-width: 768px) {
        .section-intro {
            padding: 1.5rem;
            margin-top: 2rem;
        }
    }
    .waitlist-section {
        margin-top: 2.5rem;
        padding-top: 2rem;
        border-top: 1px solid rgba(255, 255, 255, 0.1);
    }
    .waitlist-intro {
        color: var(--landing-muted);
        font-size: 1rem;
        margin-bottom: 1rem;
    }
    .waitlist-form {
        display: flex;
        flex-wrap: wrap;
        gap: 0.75rem;
        justify-content: center;
        align-items: center;
    }
    .waitlist-input {
        padding: 0.75rem 1rem;
        border: 1px solid rgba(255, 255, 255, 0.3);
        border-radius: 8px;
        background: rgba(30, 30, 30, 0.7);
        color: #fff;
        font-size: 1rem;
        min-width: 200px;
        flex: 1;
        max-width: 300px;
    }
    .waitlist-input:focus {
        outline: none;
        border-color: #1E90FF;
    }
    .waitlist-input::placeholder {
        color: rgba(255, 255, 255, 0.62);
    }
    .waitlist-button {
        padding: 0.75rem 1.5rem;
        background: linear-gradient(135deg, #d4d4d4, #a8a8a8 30%, #e8e8e8 50%, #a8a8a8 70%, #c0c0c0);
        border: none;
        border-radius: 8px;
        color: #1a1a2e;
        font-size: 1rem;
        font-weight: 600;
        cursor: pointer;
        transition: all 0.3s ease;
        border: 1px solid rgba(255, 255, 255, 0.4);
        box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3), inset 0 1px 0 rgba(255, 255, 255, 0.5);
    }
    .waitlist-button:hover:not(:disabled) {
        transform: translateY(-1px);
    }
    .waitlist-button:disabled {
        opacity: 0.7;
        cursor: not-allowed;
    }
    .waitlist-success {
        color: #4ecdc4;
        font-size: 1rem;
    }
    .waitlist-error {
        color: #ff6b6b;
        font-size: 0.9rem;
        margin-top: 0.5rem;
        width: 100%;
        text-align: center;
    }
    @media (max-width: 768px) {
        .waitlist-form {
            flex-direction: column;
        }
        .waitlist-input {
            width: 100%;
            max-width: none;
        }
        .waitlist-button {
            width: 100%;
        }
    }
    .development-links {
        margin-top: 2rem;
        font-size: 0.9rem;
        color: var(--landing-faint);
    }
    .development-links p {
        margin: 0.5rem 0;
    }
    .development-links a {
        color: var(--landing-blue);
        text-decoration: none;
        position: relative;
        padding: 0 2px;
        transition: all 0.3s ease;
    }
    .development-links a::after {
        content: '';
        position: absolute;
        width: 100%;
        height: 1px;
        bottom: -2px;
        left: 0;
        background: linear-gradient(90deg, #1E90FF, #4169E1);
        transform: scaleX(0);
        transform-origin: bottom right;
        transition: transform 0.3s ease;
    }
    .development-links a:hover {
        color: #c6ddff;
    }
    .development-links a:hover::after {
        transform: scaleX(1);
        transform-origin: bottom left;
    }
    .trust-signal {
        display: flex;
        align-items: center;
        justify-content: center;
        gap: 1rem;
        padding: 1.5rem 0;
        position: relative;
        z-index: 2;
    }
    .trust-label {
        color: rgba(255, 255, 255, 0.6);
        font-size: 0.8rem;
        text-transform: uppercase;
        letter-spacing: 0.12em;
    }
    .trust-link {
        display: flex;
        align-items: center;
        padding: 0.5rem 1rem;
        border-radius: 20px;
        background: rgba(255, 255, 255, 0.08);
        border: 1px solid rgba(255, 255, 255, 0.15);
        transition: all 0.3s ease;
    }
    .trust-link:hover {
        border-color: rgba(255, 255, 255, 0.3);
    }
    .trust-logo {
        height: 22px;
        width: auto;
        filter: brightness(0) invert(1);
        opacity: 0.9;
    }
    /* ========== Scroll Animations ========== */
    .scroll-animate,
    .scroll-animate.visible,
    .demo-story-left.scroll-animate,
    .demo-story-right.scroll-animate,
    .demo-story-left.scroll-animate.visible,
    .demo-story-right.scroll-animate.visible {
        opacity: 1;
        transform: none;
        transition: none;
    }

    /* ========== Your Data, Connected ========== */
    .connected-section {
        padding: 4rem 2rem;
        max-width: 900px;
        margin: 0 auto;
        text-align: center;
        position: relative;
        z-index: 2;
    }
    .connected-section h2 {
        font-size: 2.5rem;
        margin-bottom: 2.5rem;
        color: #fff;
    }
    .connected-grid {
        display: grid;
        grid-template-columns: repeat(3, 1fr);
        gap: 2rem;
    }
    .connected-group {
        text-align: center;
    }
    .connected-group-label {
        font-size: 0.8rem;
        text-transform: uppercase;
        letter-spacing: 0.12em;
        color: var(--landing-faint);
        margin-bottom: 1rem;
        font-weight: 600;
    }
    .connected-items {
        display: flex;
        flex-direction: column;
        gap: 0.75rem;
    }
    .connected-item {
        display: flex;
        align-items: center;
        justify-content: center;
        gap: 0.6rem;
        padding: 0.75rem 1rem;
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 12px;
        transition: all 0.3s ease;
    }
    .connected-item:hover {
        border-color: rgba(255, 255, 255, 0.25);
    }
    .connected-item i {
        font-size: 1.2rem;
        color: var(--landing-blue);
    }
    .connected-item span {
        font-size: 0.95rem;
        color: var(--landing-text);
        font-weight: 500;
    }
    @media (max-width: 768px) {
        .connected-grid {
            grid-template-columns: 1fr;
            gap: 1.5rem;
        }
        .connected-section h2 {
            font-size: 2rem;
        }
    }

    /* ========== Automate Anything with Rules ========== */
    .rules-section {
        padding: 4rem 2rem;
        max-width: 1000px;
        margin: 0 auto;
        text-align: center;
        position: relative;
        z-index: 2;
    }
    .rules-section h2 {
        font-size: 2.5rem;
        margin-bottom: 2.5rem;
        color: #fff;
    }
    .rules-grid {
        display: grid;
        grid-template-columns: repeat(3, 1fr);
        gap: 1.5rem;
        text-align: left;
    }
    .rule-block {
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 16px;
        padding: 2rem;
        transition: all 0.3s ease;
    }
    .rule-block:hover {
        border-color: rgba(255, 255, 255, 0.25);
    }
    .rule-block-label {
        display: inline-block;
        font-size: 0.75rem;
        font-weight: 700;
        letter-spacing: 0.1em;
        text-transform: uppercase;
        padding: 0.25rem 0.75rem;
        border-radius: 20px;
        margin-bottom: 0.75rem;
        background: rgba(126, 178, 255, 0.15);
        color: var(--landing-blue);
    }
    .rule-block h3 {
        font-size: 1.2rem;
        color: #fff;
        margin-bottom: 0.5rem;
        font-weight: 600;
    }
    .rule-block-desc {
        font-size: 0.9rem;
        color: var(--landing-faint);
        margin-bottom: 1rem;
    }
    .rule-block-list {
        list-style: none;
        padding: 0;
        margin: 0;
    }
    .rule-block-list li {
        font-size: 0.9rem;
        color: var(--landing-muted);
        line-height: 1.6;
        padding: 0.3rem 0;
        padding-left: 1.2rem;
        position: relative;
    }
    .rule-block-list li::before {
        content: '';
        position: absolute;
        left: 0;
        top: 0.7rem;
        width: 6px;
        height: 6px;
        border-radius: 50%;
        background: rgba(126, 178, 255, 0.5);
    }
    .rules-note {
        margin-top: 1.5rem;
        font-size: 0.95rem;
        color: var(--landing-faint);
        max-width: 700px;
        margin-left: auto;
        margin-right: auto;
        line-height: 1.6;
    }
    @media (max-width: 768px) {
        .rules-grid {
            grid-template-columns: 1fr;
            gap: 1rem;
        }
        .rules-section h2 {
            font-size: 2rem;
        }
    }

    /* ========== Talk to Lightfriend Your Way ========== */
    .interaction-section {
        padding: 4rem 2rem;
        max-width: 800px;
        margin: 0 auto;
        text-align: center;
        position: relative;
        z-index: 2;
    }
    .interaction-section h2 {
        font-size: 2.5rem;
        margin-bottom: 2.5rem;
        color: #fff;
    }
    .interaction-grid {
        display: grid;
        grid-template-columns: repeat(3, 1fr);
        gap: 1.5rem;
    }
    .interaction-card {
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 16px;
        padding: 2rem 1.5rem;
        text-align: center;
        transition: all 0.3s ease;
    }
    .interaction-card:hover {
        border-color: rgba(255, 255, 255, 0.25);
    }
    .interaction-icon {
        margin-bottom: 1rem;
    }
    .interaction-icon i {
        font-size: 1.8rem;
        color: var(--landing-blue);
    }
    .interaction-card h3 {
        font-size: 1.15rem;
        color: #fff;
        margin-bottom: 0.5rem;
        font-weight: 600;
    }
    .interaction-card p {
        font-size: 0.9rem;
        color: var(--landing-muted);
        line-height: 1.5;
        margin: 0;
    }
    @media (max-width: 768px) {
        .interaction-grid {
            grid-template-columns: 1fr;
            gap: 1rem;
        }
        .interaction-section h2 {
            font-size: 2rem;
        }
    }

    /* ========== Verifiably Private ========== */
    .privacy-section {
        padding: 4rem 2rem;
        max-width: 900px;
        margin: 0 auto;
        text-align: center;
        position: relative;
        z-index: 2;
    }
    .privacy-section h2 {
        font-size: 2.5rem;
        margin-bottom: 2rem;
        color: #fff;
    }
    .privacy-content {
        text-align: left;
    }
    .privacy-visibility {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 1.5rem;
        margin-bottom: 2rem;
    }
    .privacy-vis-card {
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 16px;
        padding: 1.5rem;
    }
    .privacy-vis-card h3 {
        font-size: 1.1rem;
        color: #fff;
        margin-bottom: 0.75rem;
        font-weight: 600;
    }
    .privacy-vis-card h3 i {
        color: var(--landing-blue);
        margin-right: 0.3rem;
    }
    .privacy-vis-card p {
        font-size: 0.95rem;
        color: var(--landing-muted);
        line-height: 1.6;
        margin: 0;
    }
    .privacy-bold-statement {
        text-align: center;
        margin-top: 2rem;
    }
    .privacy-bold-statement h3 {
        font-size: 1.6rem;
        color: #fff;
        font-weight: 700;
        margin-bottom: 1rem;
    }
    .privacy-bold-statement p {
        font-size: 1.05rem;
        color: var(--landing-muted);
        line-height: 1.7;
        margin-bottom: 1.5rem;
    }
    .privacy-link {
        color: var(--landing-blue);
        font-size: 1.05rem;
        font-weight: 600;
        text-decoration: none;
        transition: opacity 0.2s;
    }
    .privacy-link:hover {
        opacity: 0.8;
    }
    @media (max-width: 768px) {
        .privacy-visibility {
            grid-template-columns: 1fr;
            gap: 1rem;
        }
        .privacy-section h2 {
            font-size: 2rem;
        }
    }

    @media (max-width: 768px) {
        .spacer-headline {
            font-size: 1.75rem;
        }
    }
    .testimonials-section {
        padding: 4rem 2rem;
        max-width: 800px;
        margin: 0 auto;
        text-align: center;
        position: relative;
        z-index: 2;
    }
    .testimonials-section h2 {
        font-size: 2.5rem;
        margin-bottom: 1.5rem;
        color: #fff;
        font-weight: 700;
    }
    .testimonial {
        background: rgba(255, 255, 255, 0.03);
        border-radius: 12px;
        padding: 2rem;
        margin: 1rem 0;
        border: 1px solid rgba(255, 255, 255, 0.15);
        transition: all 0.3s ease;
    }
    .testimonial:hover {
        border-color: rgba(255, 255, 255, 0.3);
    }
    .testimonial blockquote {
        font-size: 1.2rem;
        color: var(--landing-text);
        line-height: 1.6;
        margin: 0;
        font-style: italic;
    }
    .testimonial-author {
        text-align: right;
        font-size: 1rem;
        color: var(--landing-muted);
        margin-top: 1rem;
    }
    @media (max-width: 768px) {
        .testimonials-section h2 {
            font-size: 2rem;
        }
        .testimonial blockquote {
            font-size: 1.1rem;
        }
    }

    /* ========== ADHD Section ========== */
    .adhd-section {
        padding: 4rem 2rem;
        max-width: 1100px;
        margin: 0 auto;
        text-align: center;
        position: relative;
        z-index: 2;
    }
    .adhd-section h2 {
        font-size: 2.5rem;
        margin-bottom: 0.75rem;
        color: #fff;
    }
    .adhd-subtitle {
        color: var(--landing-muted);
        font-size: 1.1rem;
        margin-bottom: 2.5rem;
        max-width: 600px;
        margin-left: auto;
        margin-right: auto;
    }
    .adhd-grid {
        display: grid;
        grid-template-columns: repeat(3, 1fr);
        gap: 1.5rem;
    }
    .adhd-card {
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(200, 200, 200, 0.15);
        border-radius: 16px;
        padding: 2rem 1.5rem;
        text-align: center;
        transition: all 0.3s ease;
    }
    .adhd-card:hover {
        border-color: rgba(220, 220, 220, 0.3);
    }
    .adhd-card-icon {
        font-size: 2rem;
        color: rgba(255, 255, 255, 0.7);
        margin-bottom: 1rem;
    }
    .adhd-card h3 {
        font-size: 1.2rem;
        color: #e0e0e0;
        margin-bottom: 0.75rem;
    }
    .adhd-card p {
        color: var(--landing-muted);
        font-size: 0.95rem;
        line-height: 1.5;
    }
    @media (max-width: 768px) {
        .adhd-section { padding: 2rem 1rem; }
        .adhd-section h2 { font-size: 2rem; }
        .adhd-grid { grid-template-columns: 1fr; gap: 1rem; }
    }
                "#}
            </style>
        </div>
    }
}
