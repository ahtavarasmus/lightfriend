use crate::components::notification::AnimationComponent;
use crate::Route;
use crate::utils::api::Api;
use crate::utils::seo::{use_seo, SeoMeta};
use serde::Deserialize;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use yew::prelude::*;
use yew_router::components::Link;
use web_sys::HtmlInputElement;
use serde_json::json;
use js_sys;
use gloo_timers::callback::Timeout;

#[derive(Deserialize, Clone)]
struct SmartphoneFreeDaysResponse {
    days: i64,
}
#[function_component(Landing)]
pub fn landing() -> Html {
    use_seo(SeoMeta {
        title: "Lightfriend: Private Proactive AI Assistant for Any Phone - Open Source",
        description: "Private AI assistant for any phone - flip phones, dumbphones, and smartphones. Fully open source, zero prompt injection risk. AI processing in cryptographically verified secure enclaves. WhatsApp, email, web search via SMS and voice calls.",
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
                    let _ = js_sys::eval(r#"
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
                    "#);
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
                        <h1 class="hero-title hero-anim hero-anim-1">{"Private Proactive AI Assistant"}</h1>
                        <p class="hero-subtitle hero-anim hero-anim-2">{"For Any Phone - No Setup Required"}</p>
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

            <section class="demo-story-section scroll-animate">
                <div class="demo-story-grid">
                    <div class="demo-story-left scroll-animate">
                        <div class="sms-demo-tabs">
                        {
                            scenarios.iter().enumerate().map(|(i, (label, icon, _))| {
                                let is_active = i == *scenario_idx;
                                let onclick = {
                                    let scenario_idx = scenario_idx.clone();
                                    Callback::from(move |_: MouseEvent| {
                                        scenario_idx.set(i);
                                    })
                                };
                                html! {
                                    <button
                                        class={classes!("sms-tab", is_active.then_some("active"))}
                                        onclick={onclick}
                                    >
                                        <i class={icon.to_string()}></i>
                                        <span>{*label}</span>
                                    </button>
                                }
                            }).collect::<Html>()
                        }
                        </div>
                        <div class="phone-frame">
                            <div class="phone-earpiece"></div>
                            <div class="sms-demo">
                                <div class="sms-demo-header">
                                    <span class="sms-demo-dot"></span>
                                    <span class="sms-demo-name">{"Lightfriend"}</span>
                                </div>
                                <div class="sms-demo-messages" key={format!("scenario-{}", *scenario_idx)}>
                                {
                                    scenarios[*scenario_idx].2.iter().enumerate().map(|(i, (is_user, text))| {
                                        let delay = 0.5 + (i as f64) * 1.5;
                                        let bubble_class = if *is_user { "sms-bubble sms-user" } else { "sms-bubble sms-assistant" };
                                        let style = format!("animation: smsAppear 0.4s ease {:.1}s forwards;", delay);
                                        html! {
                                            <div class={bubble_class} style={style}>{*text}</div>
                                        }
                                    }).collect::<Html>()
                                }
                                </div>
                            </div>
                            <div class="phone-chin">
                                <div class="phone-button"></div>
                            </div>
                        </div>
                        <p class="demo-customization-note">{"All proactive features can be customized or disabled to your preference."}</p>
                    </div>
                    <div class="demo-story-right scroll-animate">
                        <h2>{"Built for the ADHD Brain"}</h2>
                        <p class="adhd-subtitle">{"Smartphones hijack your attention. Lightfriend gives it back."}</p>
                        <div class="adhd-grid-inline">
                            <div class="adhd-card">
                                <div class="adhd-card-icon"><i class="fa-solid fa-calendar-check"></i></div>
                                <h3>{"Never Forget Again"}</h3>
                                <p>{"Lightfriend has saved me so many times. I\u{2019}ll forget a deadline or miss an important email \u{2014} but then Lightfriend pings me about it before it\u{2019}s too late. It watches my inbox so I don\u{2019}t have to. Honestly, I\u{2019}d be lost without it. \u{2014} Kasperi"}</p>
                            </div>
                            <div class="adhd-card">
                                <div class="adhd-card-icon"><i class="fa-solid fa-filter"></i></div>
                                <h3>{"Smart Filtering"}</h3>
                                <p>{"AI reads your WhatsApp, email, and Telegram so you don't have to. Only genuinely important messages reach you \u{2014} the rest waits until you ask."}</p>
                            </div>
                            <div class="adhd-card">
                                <div class="adhd-card-icon"><i class="fa-solid fa-ban"></i></div>
                                <h3>{"No Scroll Traps"}</h3>
                                <p>{"SMS and voice calls mean zero infinite scroll, no app switching, no \"just one more video\". Your phone becomes a tool, not a trap."}</p>
                            </div>
                        </div>
                    </div>
                </div>
            </section>

            <section class="features-section">
                <div class="features-grid-compact scroll-animate">
                    <h2>{"What You Get"}</h2>
                    <p class="features-subheader">{"Lightfriend is a private AI assistant that gives any phone user access to WhatsApp, email, web search, and more via SMS and voice calls."}</p>
                    <div class="features-flat-grid">
                        <div class="feature-chip"><i class="fab fa-whatsapp"></i><span>{"WhatsApp"}</span></div>
                        <div class="feature-chip"><i class="fab fa-telegram"></i><span>{"Telegram"}</span></div>
                        <div class="feature-chip"><i class="fab fa-signal-messenger"></i><span>{"Signal"}</span></div>
                        <div class="feature-chip"><i class="fas fa-envelope"></i><span>{"Email"}</span></div>
                        <div class="feature-chip"><i class="fas fa-search"></i><span>{"Web Search"}</span></div>
                        <div class="feature-chip"><i class="fas fa-cloud-sun"></i><span>{"Weather"}</span></div>
                        <div class="feature-chip"><i class="fas fa-image"></i><span>{"Photo Analysis"}</span></div>
                        <div class="feature-chip"><i class="fas fa-qrcode"></i><span>{"QR Scanning"}</span></div>
                        <div class="feature-chip"><i class="fas fa-bell"></i><span>{"Proactive Alerts"}</span></div>
                        <div class="feature-chip"><i class="fas fa-newspaper"></i><span>{"Daily Digests"}</span></div>
                        <div class="feature-chip"><i class="fas fa-car"></i><span>{"Tesla Control"}</span></div>
                        <div class="feature-chip feature-chip-availability"><i class="fas fa-globe"></i><span>{"40+ Countries"}</span></div>
                    </div>
                </div>
            </section>

            <div class="filter-concept">
                <div class="filter-content">
                    <AnimationComponent />
                </div>
            </div>

            <section class="why-lightfriend-section scroll-animate">
                <h2>{"Why Lightfriend"}</h2>
                <div class="why-cards-grid">
                    <div class="why-card scroll-animate">
                        <div class="why-card-icon"><i class="fas fa-lock"></i></div>
                        <h3>{"Private AI Processing"}</h3>
                        <p>{"AI runs inside Tinfoil's cryptographically verified secure enclaves using open source models. Your prompts and AI responses can't be seen in plaintext outside the enclave - not even by the cloud provider."}</p>
                    </div>
                    <div class="why-card scroll-animate">
                        <div class="why-card-icon"><i class="fas fa-shield-halved"></i></div>
                        <h3>{"Zero Prompt Injection"}</h3>
                        <p>{"Requests are processed through isolated pipelines. Your AI assistant can't be manipulated by malicious content in your messages."}</p>
                    </div>
                    <div class="why-card scroll-animate">
                        <div class="why-card-icon"><i class="fas fa-code-branch"></i></div>
                        <h3>{"Fully Open Source"}</h3>
                        <p>{"Every line of code is on GitHub. Audit it, self-host it, or contribute."}</p>
                    </div>
                    <div class="why-card scroll-animate">
                        <div class="why-card-icon"><i class="fas fa-plug"></i></div>
                        <h3>{"No Setup Required"}</h3>
                        <p>{"Sign up, connect your apps, done. No extra hardware or server configuration needed."}</p>
                    </div>
                </div>
            </section>

            <section class="trust-proof scroll-animate">
                <div class="section-intro">
                    <h2>{"The Story"}</h2>
                    <img src="/assets/rasmus-pfp.png" alt="Rasmus, founder of Lightfriend" loading="lazy" style="max-width: 200px; border-radius: 50%; margin: 0 auto 1.5rem; display: block;"/>
                    <p>{"I\u{2019}m Rasmus. I built Lightfriend because I switched to a dumbphone and needed a way to keep WhatsApp and email without a smartphone."}</p>
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
                </div>
            </section>
            <div class="filter-concept">
                <div class="filter-content">
                    <div class="faq-in-filter scroll-animate">
                        <h2>{"Frequently Asked Questions"}</h2>
                        <div class="faq-item">
                            <h3>{"Do I need a phone with internet connection?"}</h3>
                            <p>{"No, Lightfriend works through normal voice calling and text messaging (SMS)."}</p>
                        </div>
                        <div class="faq-item">
                            <h3>{"Can Lightfriend also send messages?"}</h3>
                            <p>{"Yes, it can send messages and fetch them when you call or text it."}</p>
                        </div>
                        <div class="faq-item">
                            <h3>{"How private is Lightfriend?"}</h3>
                            <p>{"AI inference runs inside Tinfoil’s cryptographically verified secure enclaves using open source models - your prompts and AI responses can’t be seen in plaintext outside the enclave. Beyond that, Lightfriend runs on a secure EU server with no logging of your chats, searches, or personal info. All credentials are encrypted, and optional conversation history gets deleted automatically as you go. Messaging app chats (like WhatsApp) are temporary too: only accessible for 2 days, then gone. The code’s open-source on GitHub, anyone can check it’s legit. You own your data and can delete it anytime."}</p>
                        </div>
                        <div class="faq-item">
                            <h3>{"Do I need any extra hardware?"}</h3>
                            <p>{"No. Sign up, connect your apps, done. No devices, no server setup."}</p>
                        </div>
                        <div class="faq-item">
                            <h3>{"How does Lightfriend help with ADHD?"}</h3>
                            <p>{"Lightfriend removes the biggest ADHD triggers: infinite scroll, notification overload, and app-switching. Instead of picking up a smartphone and losing focus, you get important updates via SMS or voice call. AI filtering means only critical messages reach you, and scheduled digests add structure to your day without requiring willpower. Many of our users switched to dumbphones specifically because of ADHD."}</p>
                        </div>
                    </div>
                </div>
            </div>
            <footer class="footer-cta scroll-animate">
                <div class="footer-content">
                    <h2>{"Ready for Digital Peace?"}</h2>
                    <p class="subtitle">{"Join the other 100+ early adopters! You will have more impact on the direction of the service and permanently cheaper prices."}</p>
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
                            <Link<Route> to={Route::Changelog}>{"Updates"}</Link<Route>>
                        </div>
                    </div>
                </div>
            </footer>
            <style>
                {r#"
    .hero-overlay {
        position: fixed;
        top: 0;
        left: 0;
        width: 100%;
        height: 100vh;
        background: linear-gradient(to bottom, transparent 0%, rgba(0, 0, 0, 0.5) 100%);
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
        background: linear-gradient(45deg, #fff, #7EB2FF);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
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
        background: linear-gradient(45deg, #fff, #7EB2FF);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
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
        content: '';
        display: block;
        height: 2px;
        width: 60%;
        margin: 0 auto 2rem;
        background: linear-gradient(to right, transparent, rgba(126, 178, 255, 0.4), transparent);
    }
    .trust-proof h2 {
        font-size: 2.5rem;
        margin-bottom: 1.5rem;
        background: linear-gradient(45deg, #fff, #7EB2FF);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
        font-weight: 700;
        text-shadow: 0 0 20px rgba(255, 255, 255, 0.2);
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
    .faq-item {
        margin-bottom: 1.5rem;
        background: rgba(126, 178, 255, 0.03);
        backdrop-filter: blur(8px);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 12px;
        padding: 1.5rem;
        box-shadow: 0 0 15px rgba(126, 178, 255, 0.05);
        transition: all 0.3s ease;
    }
    .faq-item:hover {
        border-color: rgba(255, 255, 255, 0.25);
        box-shadow: 0 0 25px rgba(255, 255, 255, 0.1);
    }
    .faq-item h3 {
        font-size: 1.4rem;
        margin-bottom: 0.75rem;
        color: #fff;
    }
    .faq-item p {
        font-size: 1.1rem;
        color: #bbb;
        line-height: 1.6;
    }
    @media (max-width: 768px) {
        .filter-concept {
            padding: 2rem;
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
        .faq-item h3 {
            font-size: 1.2rem;
        }
        .faq-item p {
            font-size: 1rem;
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
        backdrop-filter: blur(10px);
        border: 1px solid rgba(255, 255, 255, 0.15);
        border-radius: 20px;
        padding: 2.5rem;
    }
    .dual-section-card h2 {
        font-size: 1.8rem;
        margin-bottom: 1rem;
        background: linear-gradient(45deg, #fff, #7EB2FF);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
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
        transform: translateY(-5px);
    }
    .difference-text {
        flex: 1;
    }
    .difference-text h2 {
        font-size: 2.5rem;
        margin-bottom: 1.5rem;
        background: linear-gradient(45deg, #fff, #7EB2FF);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
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
        background: linear-gradient(45deg, #fff, #7EB2FF);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
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
        background: linear-gradient(45deg, #7EB2FF, #4169E1);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
        text-shadow: 0 0 8px rgba(255, 255, 255, 0.3);
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
        box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3), 0 0 25px rgba(255, 255, 255, 0.12);
        border: 1px solid rgba(255, 255, 255, 0.1);
        transition: all 0.3s ease;
    }
    .difference-image img:hover {
        box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3), 0 0 35px rgba(255, 255, 255, 0.2);
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
        background-color: transparent;
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
        min-height: 70vh;
        display: flex;
        flex-direction: column;
        justify-content: center;
        align-items: center;
        padding-top: 3rem;
        padding-bottom: 3rem;
        box-sizing: border-box;
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
        transform: translateY(-5px) scale(1.02);
        box-shadow: 0 8px 32px rgba(30, 144, 255, 0.15);
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
        content: '';
        position: absolute;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        background: linear-gradient(
            to bottom,
            rgba(26, 26, 26, 0),
            rgba(26, 26, 26, 0.7),
            rgba(26, 26, 26, 0.9)
        );
        z-index: -1;
        pointer-events: none;
    }
    .how-it-works * {
        pointer-events: auto;
    }
    .how-it-works h2 {
        font-size: 3rem;
        margin-bottom: 1rem;
        text-shadow: 0 0 20px rgba(255, 255, 255, 0.2);
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
        transform: translateY(-5px);
        box-shadow: 0 4px 20px rgba(30, 144, 255, 0.15);
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
        background: linear-gradient(45deg, #fff, #7EB2FF);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
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
        background: linear-gradient(45deg, #fff, #7EB2FF);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
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
        position: fixed;
        top: 0;
        left: 0;
        width: 100%;
        height: 100vh;
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
            rgba(26, 26, 26, 0) 0%,
            rgba(26, 26, 26, 1) 100%
        );
    }
    @media (max-width: 768px) {
        .hero-background {
            position: absolute;
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
        background: linear-gradient(45deg, #fff, #7EB2FF);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
    }
    .highlight-icon {
        font-size: 1.2rem;
        margin: 0 0.2rem;
        background: linear-gradient(45deg, #7EB2FF, #4169E1);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
        text-shadow: 0 0 8px rgba(255, 255, 255, 0.3);
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
        backdrop-filter: blur(5px);
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
        transform: translateY(-2px);
        box-shadow: 0 4px 20px rgba(255, 255, 255, 0.2), 0 0 30px rgba(200, 200, 200, 0.15);
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
        background: linear-gradient(45deg, #7EB2FF, #fff);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
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
            padding-top: 18vh;
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
        box-shadow: 0 8px 40px rgba(0, 0, 0, 0.4), 0 0 30px rgba(126, 178, 255, 0.08);
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
        box-shadow: 0 0 8px rgba(76, 175, 80, 0.5);
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
        position: fixed;
        top: 0;
        left: 0;
        width: 100%;
        height: 100vh;
        z-index: -1;
        pointer-events: none;
        overflow: hidden;
    }
    .particle {
        position: absolute;
        bottom: -10px;
        border-radius: 50%;
        background: rgba(255, 255, 255, 0.6);
        box-shadow: 0 0 6px 2px rgba(255, 255, 255, 0.3);
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
            0 20px 60px rgba(0, 0, 0, 0.5),
            0 0 30px rgba(126, 178, 255, 0.06),
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
        background: linear-gradient(45deg, #fff, #7EB2FF);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
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
        text-shadow: 0 0 8px rgba(255, 255, 255, 0.3);
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
        background: linear-gradient(45deg, #fff, #7EB2FF);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
        font-weight: 700;
        text-shadow: 0 0 20px rgba(255, 255, 255, 0.2);
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
        box-shadow: 0 0 10px rgba(255, 255, 255, 0.2);
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
        transform: translateY(-2px);
        box-shadow: 0 4px 20px rgba(255, 255, 255, 0.2), 0 0 30px rgba(200, 200, 200, 0.15);
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
        text-shadow: 0 0 8px rgba(255, 255, 255, 0.3);
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
        background: rgba(255, 255, 255, 0.1);
        backdrop-filter: blur(10px);
        border: 1px solid rgba(255, 255, 255, 0.2);
        box-shadow: 0 0 20px rgba(255, 255, 255, 0.15), inset 0 0 20px rgba(126, 178, 255, 0.05);
        transition: all 0.3s ease;
    }
    .trust-link:hover {
        background: rgba(255, 255, 255, 0.15);
        box-shadow: 0 0 30px rgba(255, 255, 255, 0.25), inset 0 0 20px rgba(255, 255, 255, 0.1);
        border-color: rgba(126, 178, 255, 0.4);
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
        transform: translateY(60px) scale(0.97);
        filter: blur(8px);
        transition: opacity 0.9s cubic-bezier(0.16, 1, 0.3, 1),
                    transform 0.9s cubic-bezier(0.16, 1, 0.3, 1),
                    filter 0.9s cubic-bezier(0.16, 1, 0.3, 1);
    }
    .scroll-animate.visible {
        opacity: 1;
        transform: translateY(0) scale(1);
        filter: blur(0);
    }
    /* Demo-story section staggered entrance */
    .demo-story-left.scroll-animate {
        transform: translateX(-60px) scale(0.97);
        filter: blur(8px);
        transition: opacity 1s cubic-bezier(0.16, 1, 0.3, 1),
                    transform 1s cubic-bezier(0.16, 1, 0.3, 1),
                    filter 0.9s cubic-bezier(0.16, 1, 0.3, 1);
    }
    .demo-story-right.scroll-animate {
        transform: translateX(60px) scale(0.97);
        filter: blur(8px);
        transition: opacity 1s cubic-bezier(0.16, 1, 0.3, 1) 0.5s,
                    transform 1s cubic-bezier(0.16, 1, 0.3, 1) 0.5s,
                    filter 0.9s cubic-bezier(0.16, 1, 0.3, 1) 0.5s;
    }
    .demo-story-left.scroll-animate.visible {
        opacity: 1;
        transform: translateX(0) scale(1);
        filter: blur(0);
    }
    .demo-story-right.scroll-animate.visible {
        opacity: 1;
        transform: translateX(0) scale(1);
        filter: blur(0);
    }
    /* SMS demo section special entrance */
    .sms-demo-section.scroll-animate {
        transform: translateY(80px) scale(0.9);
        filter: blur(12px);
        transition: opacity 1.2s cubic-bezier(0.16, 1, 0.3, 1),
                    transform 1.2s cubic-bezier(0.16, 1, 0.3, 1),
                    filter 1s cubic-bezier(0.16, 1, 0.3, 1);
    }
    .sms-demo-section.scroll-animate.visible {
        transform: translateY(0) scale(1);
        filter: blur(0);
    }
    /* Section headings glow in */
    .trust-proof.scroll-animate {
        transform: translateY(50px) scale(0.98);
        filter: blur(6px);
        transition: opacity 1s cubic-bezier(0.16, 1, 0.3, 1),
                    transform 1s cubic-bezier(0.16, 1, 0.3, 1),
                    filter 0.8s ease;
    }
    .trust-proof.scroll-animate.visible {
        transform: translateY(0) scale(1);
        filter: blur(0);
    }

    /* ========== Compact Features Grid ========== */
    .features-grid-compact {
        padding: 4rem 2rem;
        max-width: 900px;
        margin: 0 auto;
        text-align: center;
        position: relative;
        z-index: 2;
    }
    .features-grid-compact h2 {
        font-size: 2.5rem;
        margin-bottom: 2rem;
        background: linear-gradient(45deg, #fff, #7EB2FF);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
    }
    .features-subheader {
        font-size: 1.15rem;
        color: #bbb;
        margin-bottom: 2rem;
        line-height: 1.6;
        max-width: 600px;
        margin-left: auto;
        margin-right: auto;
    }
    .features-flat-grid {
        display: grid;
        grid-template-columns: repeat(4, 1fr);
        gap: 1rem;
    }
    .feature-chip {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 0.5rem;
        padding: 1rem 0.5rem;
        background: rgba(126, 178, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 12px;
        transition: all 0.3s ease;
    }
    .feature-chip:hover {
        border-color: rgba(255, 255, 255, 0.25);
        background: rgba(126, 178, 255, 0.08);
        transform: translateY(-2px);
    }
    .feature-chip i {
        font-size: 1.3rem;
        color: #7EB2FF;
    }
    .feature-chip span {
        font-size: 0.85rem;
        color: #ddd;
        font-weight: 500;
    }
    .feature-chip-availability {
        border-color: rgba(126, 178, 255, 0.25);
        background: rgba(126, 178, 255, 0.06);
    }
    @media (max-width: 768px) {
        .features-flat-grid {
            grid-template-columns: repeat(3, 1fr);
            gap: 0.75rem;
        }
        .features-grid-compact h2 {
            font-size: 2rem;
        }
    }
    @media (max-width: 480px) {
        .features-flat-grid {
            grid-template-columns: repeat(2, 1fr);
        }
    }
    /* ========== Why Lightfriend Section ========== */
    .why-lightfriend-section {
        padding: 4rem 2rem;
        max-width: 1000px;
        margin: 0 auto;
        text-align: center;
        position: relative;
        z-index: 2;
    }
    .why-lightfriend-section h2 {
        font-size: 2.5rem;
        margin-bottom: 2rem;
        background: linear-gradient(45deg, #fff, #7EB2FF);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
    }
    .why-cards-grid {
        display: grid;
        grid-template-columns: repeat(2, 1fr);
        gap: 1.5rem;
    }
    .why-card {
        background: rgba(126, 178, 255, 0.03);
        backdrop-filter: blur(8px);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 16px;
        padding: 2rem;
        text-align: left;
        transition: all 0.3s ease;
    }
    .why-card:hover {
        border-color: rgba(255, 255, 255, 0.25);
        box-shadow: 0 0 25px rgba(255, 255, 255, 0.1);
        transform: translateY(-3px);
    }
    .why-card-icon {
        margin-bottom: 1rem;
    }
    .why-card-icon i {
        font-size: 1.5rem;
        color: #7EB2FF;
    }
    .why-card h3 {
        font-size: 1.2rem;
        color: #fff;
        margin-bottom: 0.75rem;
        font-weight: 600;
    }
    .why-card p {
        font-size: 0.95rem;
        color: #bbb;
        line-height: 1.6;
        margin: 0;
    }
    @media (max-width: 768px) {
        .why-cards-grid {
            grid-template-columns: 1fr;
            gap: 1rem;
        }
        .why-lightfriend-section h2 {
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
        background: linear-gradient(45deg, #fff, #7EB2FF);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
        font-weight: 700;
        text-shadow: 0 0 20px rgba(255, 255, 255, 0.2);
    }
    .testimonial {
        background: rgba(126, 178, 255, 0.05);
        backdrop-filter: blur(10px);
        border-radius: 12px;
        padding: 2rem;
        margin: 1rem 0;
        border: 1px solid rgba(255, 255, 255, 0.15);
        box-shadow: 0 0 20px rgba(255, 255, 255, 0.1);
        transition: all 0.3s ease;
    }
    .testimonial:hover {
        border-color: rgba(255, 255, 255, 0.3);
        box-shadow: 0 0 30px rgba(255, 255, 255, 0.15);
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
        background: linear-gradient(135deg, #e0e0e0, #a8a8a8 30%, #f0f0f0 50%, #a8a8a8 70%, #c0c0c0);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
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
        background: linear-gradient(145deg, rgba(200, 200, 200, 0.08), rgba(160, 160, 160, 0.04));
        backdrop-filter: blur(8px);
        border: 1px solid rgba(200, 200, 200, 0.15);
        border-radius: 16px;
        padding: 2rem 1.5rem;
        text-align: center;
        transition: all 0.3s ease;
        box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2), inset 0 1px 0 rgba(255, 255, 255, 0.08);
    }
    .adhd-card:hover {
        border-color: rgba(220, 220, 220, 0.3);
        box-shadow: 0 4px 20px rgba(200, 200, 200, 0.1), inset 0 1px 0 rgba(255, 255, 255, 0.12);
        transform: translateY(-2px);
    }
    .adhd-card-icon {
        font-size: 2rem;
        background: linear-gradient(135deg, #d4d4d4, #a8a8a8 30%, #e8e8e8 50%, #a8a8a8 70%, #c0c0c0);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
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
