use crate::components::notification::AnimationComponent;
use crate::utils::api::Api;
use crate::utils::seo::{use_seo, SeoMeta};
use crate::Route;
use gloo_timers::callback::Timeout;
use js_sys;
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
        title: "Lightfriend: Mute Everything. Miss Nothing. AI Assistant for Dumbphones",
        description: "AI watches your WhatsApp, Telegram, Signal, and email - and only interrupts you when something actually matters. Works with any phone including dumbphones and Light Phone via SMS and voice calls. Privacy verifiable on blockchain - no trust required.",
        canonical: "https://lightfriend.ai",
        og_type: "website",
    });

    // Waitlist form state
    let waitlist_email = use_state(String::new);
    let waitlist_loading = use_state(|| false);
    let waitlist_success = use_state(|| false);
    let waitlist_error = use_state(|| None::<String>);

    // Scroll to top only on initial mount
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

    // State for expanded integration (-1 = none, 0-5 = which one)
    let expanded_integration = use_state(|| -1i32);

    // State for expanded FAQ item (-1 = none, 0+ = which one)
    let expanded_faq = use_state(|| -1i32);

    // SMS demo scenarios: (label, icon_class, messages)
    // is_user=false means Lightfriend sends it proactively
    let scenarios: Vec<(&str, &str, Vec<(bool, &str)>)> = vec![
        ("WhatsApp", "fab fa-whatsapp", vec![
            (false, "WhatsApp from Mom: \"Are you coming for dinner tonight?\" - Sent 10 min ago."),
            (true, "Reply: sounds great, see you at 7!"),
            (false, "Message sent to Mom \u{2713}"),
        ]),
        ("Email", "fas fa-envelope", vec![
            (false, "Your Amazon package has been delivered to your front porch. Order: wireless headphones."),
            (true, "Any other deliveries today?"),
            (false, "Yes - IKEA order is out for delivery, estimated arrival by 4pm."),
        ]),
        ("Reminder", "fas fa-bell", vec![
            (false, "Reminder: Pick up your prescription from the pharmacy. They close at 6pm today."),
            (true, "Thanks, what's the pharmacy's number?"),
            (false, "City Pharmacy: (555) 234-5678. Open until 6pm."),
        ]),
        ("Tracking", "fas fa-eye", vec![
            (false, "You haven't replied to your neighbor Jake's Signal message from yesterday about borrowing the ladder. Want to respond?"),
            (true, "Reply: Hey sure, come grab it anytime this weekend!"),
            (false, "Message sent to Jake \u{2713}"),
        ]),
        ("Tesla", "fas fa-car", vec![
            (true, "Preheat my Tesla"),
            (false, "Done - cabin heating started \u{2713} Current battery: 72%, outside temp: -15\u{00b0}C."),
        ]),
    ];
    let scenario_idx = use_state(|| 0usize);
    let scenario_count = scenarios.len();

    // Auto-rotation timer for SMS demo
    {
        let scenario_idx_effect = scenario_idx.clone();
        let dep = *scenario_idx;
        use_effect_with_deps(
            move |idx: &usize| {
                let idx_val = *idx;
                let scenario_idx = scenario_idx_effect.clone();
                let timeout = Timeout::new(8_000, move || {
                    scenario_idx.set((idx_val + 1) % scenario_count);
                });
                move || drop(timeout)
            },
            dep,
        );
    }

    // Set up IntersectionObserver for scroll animations
    {
        use_effect_with_deps(
            move |_| {
                let setup = Closure::<dyn Fn()>::new(move || {
                    let _ = js_sys::eval(
                        r#"
                        if (!window._scrollObserver) {
                            var delay = 0;
                            window._scrollObserver = new IntersectionObserver(function(entries) {
                                entries.forEach(function(entry) {
                                    if (entry.isIntersecting) {
                                        var el = entry.target;
                                        var stagger = el.dataset.stagger || 0;
                                        setTimeout(function() {
                                            el.classList.add('visible');
                                        }, stagger * 120);
                                    }
                                });
                            }, { threshold: 0.08, rootMargin: '0px 0px -80px 0px' });
                        }
                        var groups = {};
                        document.querySelectorAll('.scroll-animate:not(.visible)').forEach(function(el) {
                            var parent = el.parentElement;
                            var key = parent ? parent.className : 'root';
                            if (!groups[key]) groups[key] = 0;
                            el.dataset.stagger = groups[key]++;
                            window._scrollObserver.observe(el);
                        });
                    "#,
                    );
                });
                if let Some(window) = web_sys::window() {
                    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                        setup.as_ref().unchecked_ref(),
                        100,
                    );
                }
                setup.forget();
                || ()
            },
            (),
        );
    }

    // Integration data: (icon_class, label, description)
    let integration_data = vec![
        ("fab fa-whatsapp", "WhatsApp", "Receive and reply to WhatsApp messages via SMS or voice call. AI filters noise and only forwards what matters."),
        ("fab fa-telegram", "Telegram", "Access your Telegram chats from any phone. Get notified about important messages, reply directly."),
        ("fab fa-signal-messenger", "Signal", "Stay on Signal without a smartphone. Messages are bridged securely to your phone via SMS."),
        ("fas fa-envelope", "Email", "Read and respond to emails via text or voice. AI summarizes long threads and flags urgent ones."),
        ("fas fa-car", "Tesla", "Lock, unlock, preheat, and check battery status of your Tesla via SMS commands."),
        ("fas fa-plug", "MCP / Custom", "Connect any external tool or service via MCP (Model Context Protocol). Extend what Lightfriend can do."),
    ];

    let integration_buttons_html: Vec<Html> = integration_data.iter().enumerate().map(|(idx, (icon, label, _))| {
        let expanded = expanded_integration.clone();
        let i = idx as i32;
        let is_active = *expanded == i;
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
            <button class={classes!("integration-btn", if is_active { "active" } else { "" })} onclick={onclick} title={*label}>
                <i class={*icon}></i>
                <span class="integration-label">{label}</span>
            </button>
        }
    }).collect();

    let integration_detail_html = if *expanded_integration >= 0 {
        let idx = *expanded_integration as usize;
        let (_, label, desc) = &integration_data[idx];
        html! {
            <div class={classes!("integration-detail", "visible")}>
                <div class="integration-detail-content">
                    <h3>{label}</h3>
                    <p>{desc}</p>
                </div>
            </div>
        }
    } else {
        html! { <div class="integration-detail"></div> }
    };

    // FAQ data: (question, answer_html)
    let faq_data: Vec<(&str, Html)> = vec![
        ("Do I need a phone with internet?", html! {
            <p>{"No. Lightfriend works through normal voice calls and SMS. Any phone that can call and text will work."}</p>
        }),
        ("Can I send and receive messages?", html! {
            <p>{"Yes. You can reply to WhatsApp, Telegram, Signal, and email directly via SMS or voice call. Lightfriend forwards your reply to the right place."}</p>
        }),
        ("What can Lightfriend actually do?", html! {
            <ul>
                <li><strong>{"Message bridges:"}</strong>{" WhatsApp, Telegram, Signal - receive and reply from any phone."}</li>
                <li><strong>{"Email:"}</strong>{" Read and respond to emails via text."}</li>
                <li><strong>{"Critical notifications:"}</strong>{" AI screens messages and only alerts you about urgent ones."}</li>
                <li><strong>{"Smart digests:"}</strong>{" Get a summary of what happened, delivered when you want."}</li>
                <li><strong>{"Web search:"}</strong>{" Ask any question, get a concise answer."}</li>
                <li><strong>{"Image understanding:"}</strong>{" Send a photo of a menu, sign, or QR code."}</li>
                <li><strong>{"Tesla control:"}</strong>{" Lock, unlock, preheat via SMS."}</li>
                <li><strong>{"Rule builder:"}</strong>{" Create custom automations with triggers and conditions."}</li>
                <li><strong>{"MCP integrations:"}</strong>{" Connect external tools and services."}</li>
                <li><strong>{"Learns over time:"}</strong>{" Lightfriend builds context about who matters to you and what's urgent. The longer you use it, the better it gets at surfacing the right things."}</li>
            </ul>
        }),
        ("Which countries are supported?", html! {
            <>
                <p><strong>{"Full service:"}</strong>{" US, Canada, UK, Finland, Netherlands, Australia."}</p>
                <p><strong>{"Notification-only:"}</strong>{" 30+ countries across Europe and Asia-Pacific."}</p>
                <p><strong>{"Elsewhere:"}</strong>{" Bring your own Twilio number."}</p>
            </>
        }),
        ("How does it protect my data?", html! {
            <>
                <p>{"Lightfriend runs in its own hardware-isolated enclave, and all AI requests are processed through Tinfoil's verified enclaves. No one - not even the developer - can access your data. Period. Fully open source, with privacy cryptographically verifiable on blockchain."}</p>
                <p><a href="/trustless" style="color: #7EB2FF;">{"See exactly how it works"}</a></p>
            </>
        }),
        ("How do critical notifications work?", html! {
            <p>{"When a message arrives on WhatsApp, Telegram, Signal, or email, AI evaluates whether it needs your immediate attention. Urgent messages get forwarded instantly via SMS or phone call. Everything else goes into your digest."}</p>
        }),
    ];

    let faq_items_html: Vec<Html> = faq_data.into_iter().enumerate().map(|(idx, (question, answer))| {
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
                <div class="hero-background"></div>
                <div class="hero-overlay"></div>
                <div class="hero-particles">
                    { for particles_html }
                </div>
                <div class="hero-content">
                    <div class="hero-right-panel">
                        <h1 class="hero-title hero-anim hero-anim-1">{"Mute everything. Miss nothing."}</h1>
                        <p class="hero-subtitle hero-anim hero-anim-2">{"AI watches your WhatsApp, email, and messages. If something matters, it calls or texts you."}</p>
                        <div class="hero-cta-group hero-anim hero-anim-3">
                            <Link<Route> to={Route::Pricing} classes="forward-link">
                                <button class="hero-cta">{"See Plans"}</button>
                            </Link<Route>>
                        </div>
                        <div class="trust-signal hero-anim hero-anim-3">
                            <span class="trust-label">{"As seen on"}</span>
                            <a href="https://www.thelightphone.com/blog/lightos-tips" target="_blank" rel="noopener noreferrer" class="trust-link">
                                <img src="/assets/lightphone-logo.svg" alt="The Light Phone" class="trust-logo" />
                            </a>
                        </div>
                        <div class="hero-metric hero-anim hero-anim-3">
                            <span class="hero-metric-number">{days_smartphone_free}</span>
                            <span class="hero-metric-label">{"smartphone-free days powered"}</span>
                        </div>
                    </div>
                </div>
            </header>

            // Animation - immediately shows how it works
            <div class="filter-concept">
                <h2>{"Stay in Your Life"}</h2>
                <p class="filter-concept-subtitle">{"We'll pull you out only when it's urgent."}</p>
                <div class="filter-content">
                    <AnimationComponent />
                </div>
                <div class="integrations-row">
                    <div class="integration-buttons">
                        { for integration_buttons_html }
                    </div>
                    { integration_detail_html }
                </div>
            </div>

            // Section: Verifiably Private
            <section class="privacy-section scroll-animate">
                <h2>{"Verifiably Private"}</h2>
                <div class="privacy-content">
                    <div class="privacy-visibility">
                        <div class="privacy-vis-card privacy-can-see">
                            <h3><i class="fas fa-eye"></i>{" What we can see"}</h3>
                            <p>{"Only your phone number and email (for subscription management)."}</p>
                        </div>
                        <div class="privacy-vis-card privacy-cannot-see">
                            <h3><i class="fas fa-eye-slash"></i>{" What we can NOT see"}</h3>
                            <p>{"Your messages, emails, contacts, or any private data."}</p>
                        </div>
                    </div>
                    <div class="privacy-bold-statement">
                        <h3>{"We Can\u{2019}t See Your Data. Even If We Wanted To."}</h3>
                        <p>{"Open source. Verified on-chain. AI processed through Tinfoil\u{2019}s verified enclaves. Not a promise - cryptographic proof."}</p>
                        <a href="/trustless" class="privacy-link">{"See exactly how it works \u{2192}"}</a>
                    </div>
                </div>
            </section>

            <section class="trust-proof scroll-animate">
                <div class="section-intro">
                    <h2>{"The Story"}</h2>
                    <img src="/assets/rasmus-pfp.png" alt="Rasmus, founder of Lightfriend" loading="lazy" style="max-width: 200px; border-radius: 50%; margin: 0 auto 1.5rem; display: block;"/>
                    <p>{"I\u{2019}m "}<a href="https://rasmus.ahtava.com" target="_blank" rel="noopener noreferrer">{"Rasmus"}</a>{". I built Lightfriend because I switched to a dumbphone and needed a way to keep WhatsApp and email without a smartphone."}</p>
                </div>
            </section>

            <section class="testimonials-section scroll-animate">
                <div class="testimonials-content">
                    <h2>{"Life After Smartphones"}</h2>
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
                    <div class="testimonial">
                        <blockquote>
                            {"As a dumbphone user, I couldn't live without lightfriend. It's useful, smart and most importantly, reliable. A true must have for living a distraction free life."}
                        </blockquote>
                    </div>
                    <div class="testimonial">
                        <blockquote>
                            {"I have ADHD so smartphones were basically impossible for me. I'd check one notification and suddenly an hour was gone. Now I just get a text with the important stuff. No apps, nothing to get lost in. Honestly it's changed everything for how I get through my day."}
                        </blockquote>
                    </div>
                    <div class="testimonial">
                        <blockquote>
                            {"Lightfriend has saved me so many times. I\u{2019}ll forget a deadline or miss an important email \u{2014} but then Lightfriend pings me about it before it\u{2019}s too late. It watches my inbox so I don\u{2019}t have to. Honestly, I\u{2019}d be lost without it."}
                        </blockquote>
                        <p class="testimonial-author">{"- Kasperi"}</p>
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
            <footer class="footer-cta scroll-animate">
                <div class="footer-content">
                    <h2>{"Ready for Digital Peace?"}</h2>
                    <Link<Route> to={Route::Pricing} classes="forward-link">
                        <button class="hero-cta">{"Start Today"}</button>
                    </Link<Route>>
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
    .hero-overlay {
        position: absolute;
        top: 0;
        left: 0;
        width: 100%;
        height: 100%;
        background: linear-gradient(to bottom, transparent 0%, rgba(0, 0, 0, 0.5) 80%, #0d0d0d 100%);
        z-index: -1;
        pointer-events: none;
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
        background: #0d0d0d;
        text-align: center;
    }
    .filter-concept h2 {
        font-size: 2.5rem;
        margin-bottom: 1rem;
        color: #fff;
    }
    .filter-concept-subtitle {
        font-size: 1.2rem;
        color: #bbb;
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
        color: rgba(255, 255, 255, 0.5);
    }
    .integration-btn:hover {
        border-color: rgba(255, 255, 255, 0.25);
        color: rgba(255, 255, 255, 0.8);
        background: rgba(255, 255, 255, 0.07);
    }
    .integration-btn.active {
        border-color: rgba(126, 178, 255, 0.5);
        color: #7EB2FF;
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
        color: #999;
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
        color: #bbb;
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
        color: #7EB2FF;
    }
    .landing-faq-question .toggle-icon {
        font-size: 1.5rem;
        color: #7EB2FF;
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
        color: #999;
        line-height: 1.6;
        margin-bottom: 0.75rem;
    }
    .landing-faq-answer ul {
        list-style: none;
        padding: 0;
        margin: 0.5rem 0;
    }
    .landing-faq-answer li {
        color: #999;
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
        color: #7EB2FF;
    }
    .landing-faq-answer a {
        color: #7EB2FF;
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
        color: #7EB2FF;
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
        color: #bbb;
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
        color: #ddd;
        margin-bottom: 1.5rem;
    }
    .comparison-table table {
        width: 100%;
        border-collapse: collapse;
        margin: 0 auto;
        font-size: 1rem;
        color: #ddd;
    }
    .comparison-table th, .comparison-table td {
        padding: 1rem;
        text-align: left;
        border-bottom: 1px solid rgba(255, 255, 255, 0.2);
    }
    .comparison-table th {
        background: rgba(0, 0, 0, 0.5);
        color: #7EB2FF;
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
        color: #7EB2FF;
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
        background: #0d0d0d;
        border-top: 1px solid rgba(255, 255, 255, 0.06);
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
        color: #7EB2FF;
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
        color: #999;
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
        border-top: 1px solid rgba(255, 255, 255, 0.1);
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
        color: #999;
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
    .hero-background {
        position: absolute;
        top: 0;
        left: 0;
        width: 100%;
        height: 100%;
        background-image: url('/assets/aurora-bg.jpg');
        background-size: cover;
        background-position: center;
        background-repeat: no-repeat;
        opacity: 1;
        z-index: -2;
        pointer-events: none;
    }
    .hero-background::after {
        content: '';
        position: absolute;
        bottom: 0;
        left: 0;
        width: 100%;
        height: 50%;
        background: linear-gradient(to bottom,
            rgba(13, 13, 13, 0) 0%,
            rgba(13, 13, 13, 1) 100%
        );
    }
    @media (max-width: 768px) {
        .hero-background {
            background-position: 70% center;
        }
    }
    .hero-title {
        font-size: clamp(2.5rem, 8vw, 5.5rem);
        font-weight: 800;
        color: #fff;
        text-shadow: none;
        margin: 0 auto 0.5rem;
        width: 100%;
        line-height: 1.15;
        text-align: center;
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
        font-size: 1.4rem;
        font-weight: 300;
        letter-spacing: 0.02em;
        max-width: 600px;
        margin: 0 auto 1rem;
        line-height: 1.6;
        text-align: center;
        color: #fff;
    }
    .highlight-icon {
        font-size: 1.2rem;
        margin: 0 0.2rem;
        color: #7EB2FF;
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
            font-size: clamp(1.8rem, 7vw, 2.5rem);
        }
        .hero-tagline {
            font-size: 0.95rem;
        }
        .hero-subtitle {
            font-size: 1.15rem;
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
        color: #7EB2FF;
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
        max-width: 800px;
        padding-top: 30vh;
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
            padding-top: 28vh;
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
        color: #ddd;
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
        color: rgba(255, 255, 255, 0.5);
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
        color: rgba(255, 255, 255, 0.4);
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
        opacity: 0;
        animation: heroFadeIn 0.7s ease forwards;
    }
    .hero-anim-1 { animation-delay: 0.2s; }
    .hero-anim-2 { animation-delay: 0.5s; }
    .hero-anim-3 { animation-delay: 0.8s; }
    .hero-anim-4 { animation-delay: 1.1s; }
    .hero-anim-5 { animation-delay: 1.5s; }

    .faq-link {
        color: #7EB2FF;
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
        color: #bbb;
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
        color: #666;
        text-decoration: none;
        transition: color 0.3s ease;
    }
    .legal-links a:hover {
        color: #7EB2FF;
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
        color: #888;
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
        color: #666;
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
        color: #666;
    }
    .development-links p {
        margin: 0.5rem 0;
    }
    .development-links a {
        color: #007bff;
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
        color: #7EB2FF;
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
    .scroll-animate {
        opacity: 0;
        transform: translateY(30px);
        transition: opacity 0.7s ease, transform 0.7s ease;
    }
    .scroll-animate.visible {
        opacity: 1;
        transform: translateY(0);
    }
    /* Demo-story section staggered entrance */
    .demo-story-left.scroll-animate {
        transform: translateX(-30px);
        transition: opacity 0.7s ease, transform 0.7s ease;
    }
    .demo-story-right.scroll-animate {
        transform: translateX(30px);
        transition: opacity 0.7s ease 0.2s, transform 0.7s ease 0.2s;
    }
    .demo-story-left.scroll-animate.visible {
        opacity: 1;
        transform: translateX(0);
    }
    .demo-story-right.scroll-animate.visible {
        opacity: 1;
        transform: translateX(0);
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
        color: rgba(255, 255, 255, 0.5);
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
        color: #7EB2FF;
    }
    .connected-item span {
        font-size: 0.95rem;
        color: #ddd;
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
        color: #7EB2FF;
    }
    .rule-block h3 {
        font-size: 1.2rem;
        color: #fff;
        margin-bottom: 0.5rem;
        font-weight: 600;
    }
    .rule-block-desc {
        font-size: 0.9rem;
        color: rgba(255, 255, 255, 0.5);
        margin-bottom: 1rem;
    }
    .rule-block-list {
        list-style: none;
        padding: 0;
        margin: 0;
    }
    .rule-block-list li {
        font-size: 0.9rem;
        color: #bbb;
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
        color: rgba(255, 255, 255, 0.5);
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
        color: #7EB2FF;
    }
    .interaction-card h3 {
        font-size: 1.15rem;
        color: #fff;
        margin-bottom: 0.5rem;
        font-weight: 600;
    }
    .interaction-card p {
        font-size: 0.9rem;
        color: #bbb;
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
        color: #7EB2FF;
        margin-right: 0.3rem;
    }
    .privacy-vis-card p {
        font-size: 0.95rem;
        color: #bbb;
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
        color: #bbb;
        line-height: 1.7;
        margin-bottom: 1.5rem;
    }
    .privacy-link {
        color: #7EB2FF;
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
        color: #ddd;
        line-height: 1.6;
        margin: 0;
        font-style: italic;
    }
    .testimonial-author {
        text-align: right;
        font-size: 1rem;
        color: #bbb;
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
        color: #999;
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
        color: #999;
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
