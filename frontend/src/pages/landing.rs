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
        title: "Lightfriend: The Best AI Assistant for Dumbphones \u{2013} WhatsApp, Telegram, Signal, Email & More",
        description: "AI assistant for dumbphones like Light Phone 3, Nokia flip phones, and other minimalist phones. Access WhatsApp, Telegram, Signal, email, calendar, AI search, and GPS via SMS/voice.",
        canonical: "https://lightfriend.ai",
        og_type: "website",
    });

    let dim_opacity = use_state(|| 0.0);

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
    // Add scroll listener for dimming
    {
        let dim_opacity = dim_opacity.clone();
        use_effect_with_deps(
            move |_| {
                let destructor: Box<dyn FnOnce()> = if let Some(window) = web_sys::window() {
                    let callback = Closure::<dyn Fn()>::new({
                        let dim_opacity = dim_opacity.clone();
                        move || {
                            if let Some(win) = web_sys::window() {
                                if let Ok(scroll_y) = win.scroll_y() {
                                    let factor = (scroll_y / 500.0).min(1.0);
                                    dim_opacity.set(factor * 0.6);
                                }
                            }
                        }
                    });
                    window
                        .add_event_listener_with_callback(
                            "scroll",
                            callback.as_ref().unchecked_ref(),
                        )
                        .unwrap();
                    // Initial call
                    if let Ok(scroll_y) = window.scroll_y() {
                        let factor = (scroll_y / 500.0).min(1.0);
                        dim_opacity.set(factor * 0.6);
                    }
                    Box::new(move || {
                        if let Some(win) = web_sys::window() {
                            win.remove_event_listener_with_callback(
                                "scroll",
                                callback.as_ref().unchecked_ref(),
                            )
                            .unwrap();
                        }
                    })
                } else {
                    Box::new(|| ())
                };
                move || {
                    destructor();
                }
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
    let scenarios: Vec<(&str, &str, Vec<(bool, &str)>)> = vec![
        ("WhatsApp", "fab fa-whatsapp", vec![
            (true, "Check my WhatsApp"),
            (false, "3 new messages. Mom: \"Coming for dinner?\""),
            (true, "Reply: sounds great, see you at 7!"),
            (false, "Message sent to Mom \u{2713}"),
        ]),
        ("Weather", "fas fa-cloud-sun", vec![
            (true, "Weather in Helsinki"),
            (false, "Helsinki: -5\u{00b0}C, light snow. Tomorrow: -8\u{00b0}C, clear skies. Wind 3m/s from north."),
        ]),
        ("Directions", "fas fa-route", vec![
            (true, "Walking from Central Station to Market Square"),
            (false, "Head east on Kaivokatu (200m) \u{2192} Turn right onto Unioninkatu (400m) \u{2192} Market Square on your left. ~8 min walk."),
        ]),
        ("Email", "fas fa-envelope", vec![
            (true, "Check my email"),
            (false, "3 new emails. 1) Amazon: Your order shipped. 2) Boss: Meeting moved to 3pm. 3) Newsletter from TechCrunch."),
        ]),
        ("Calendar", "fas fa-calendar-days", vec![
            (true, "What\u{2019}s on my calendar today?"),
            (false, "2 events: 10:00 Team standup (30min), 14:00 Dentist appointment at Smile Clinic."),
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

    // Feature grid expanded state
    let comic_open = use_state(|| false);
    let expanded_feature = use_state(|| String::new());
    let ef_toggle = expanded_feature.clone();
    let make_toggle = move |key: &'static str| -> Callback<MouseEvent> {
        let ef = ef_toggle.clone();
        let key_str = key.to_string();
        Callback::from(move |_: MouseEvent| {
            if *ef == key_str {
                ef.set(String::new());
            } else {
                ef.set(key_str.clone());
            }
        })
    };

    let feature_css = r#"
        .feature-list {
            padding: 4rem 2rem;
            max-width: 900px;
            margin: 0 auto;
            text-align: left;
            position: relative;
            z-index: 2;
        }
        .feature-list h2 {
            font-size: 2.5rem;
            margin-bottom: 1.5rem;
            background: linear-gradient(45deg, #fff, #7EB2FF);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
            text-align: center;
        }
        .feature-categories-grid {
            display: grid;
            grid-template-columns: repeat(2, 1fr);
            gap: 2rem;
        }
        .feature-category-card {
            background: rgba(126, 178, 255, 0.03);
            backdrop-filter: blur(8px);
            border: 1px solid rgba(255, 255, 255, 0.12);
            border-radius: 16px;
            padding: 1.5rem;
            box-shadow: 0 0 15px rgba(126, 178, 255, 0.05);
            transition: all 0.3s ease;
        }
        .feature-category-card:hover {
            border-color: rgba(255, 255, 255, 0.25);
            box-shadow: 0 0 25px rgba(255, 255, 255, 0.1);
        }
        .feature-category-header {
            display: flex;
            align-items: center;
            gap: 0.75rem;
            margin-bottom: 1rem;
            padding-bottom: 0.75rem;
            border-bottom: 1px solid rgba(255, 255, 255, 0.1);
        }
        .feature-category-header i {
            font-size: 1.3rem;
            background: linear-gradient(45deg, #7EB2FF, #4169E1);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
        }
        .feature-category-header h3 {
            font-size: 1.2rem;
            margin: 0;
            background: linear-gradient(45deg, #fff, #7EB2FF);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
            font-weight: 600;
        }
        .feature-item {
            margin-bottom: 0.25rem;
        }
        .feature-item-header {
            display: flex;
            align-items: center;
            gap: 0.75rem;
            padding: 0.5rem;
            cursor: pointer;
            border-radius: 8px;
            transition: background 0.2s ease;
        }
        .feature-item-header:hover {
            background: rgba(126, 178, 255, 0.08);
        }
        .feature-item-header i {
            color: #7EB2FF;
            font-size: 1rem;
            width: 1.2rem;
            text-align: center;
            flex-shrink: 0;
        }
        .feature-item-header span {
            font-size: 1rem;
            color: #ddd;
        }
        .feature-item-arrow {
            font-size: 0.7rem;
            color: #7EB2FF;
            margin-left: auto;
            transition: transform 0.2s ease;
            flex-shrink: 0;
        }
        .feature-item-arrow.expanded {
            transform: rotate(90deg);
        }
        .feature-desc {
            padding: 1rem;
            background: rgba(0, 0, 0, 0.2);
            border-radius: 8px;
            color: #ddd;
            font-size: 0.95rem;
            margin: 0.25rem 0 0.5rem;
        }
        .feature-desc iframe {
            width: 100%;
            aspect-ratio: 16/9;
            margin-top: 1rem;
            border: none;
        }
        .feature-video {
            width: 100%;
            max-width: 400px;
            margin: 1rem auto;
            display: block;
            border-radius: 12px;
        }
        .feature-preview-img {
            display: block;
            max-width: 100%;
            width: 100%;
            margin: 1rem auto;
            border-radius: 12px;
            border: 1px solid rgba(255, 255, 255, 0.2);
            box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
        }
        .feature-preview-wrapper {
            position: relative;
            display: inline-block;
            margin: 1rem auto;
            width: 100%;
            text-align: center;
        }
        .feature-preview-badge {
            position: absolute;
            top: calc(1rem + 8px);
            right: 8px;
            background: rgba(30, 144, 255, 0.85);
            color: white;
            padding: 4px 12px;
            border-radius: 20px;
            font-size: 11px;
            font-weight: 600;
            text-transform: uppercase;
            letter-spacing: 0.5px;
            z-index: 1;
        }
        @media (max-width: 768px) {
            .feature-list {
                padding: 2rem 1rem;
            }
            .feature-list h2 {
                font-size: 2rem;
            }
            .feature-categories-grid {
                grid-template-columns: 1fr;
                gap: 1.5rem;
            }
            .feature-desc {
                font-size: 0.9rem;
            }
        }
    "#;

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
                                    } else {
                                        entry.target.classList.remove('visible');
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
                <div class="hero-overlay" style={format!("opacity: {};", *dim_opacity)}></div>
                <div class="hero-particles">
                    { for particles_html }
                </div>
                <div class="hero-content">
                    <div class="hero-right-panel">
                        <h1 class="hero-title hero-anim hero-anim-1">{"Safe AI Assistant"}</h1>
                        <p class="hero-subtitle hero-anim hero-anim-2">{"WhatsApp & Email on Your Flip Phone"}</p>
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
                    </div>
                    <div class="demo-story-right scroll-animate">
                        <h2>{"Built for the ADHD Brain"}</h2>
                        <p class="adhd-subtitle">{"Smartphones hijack your attention. Lightfriend gives it back."}</p>
                        <div class="adhd-grid-inline">
                            <div class="adhd-card">
                                <div class="adhd-card-icon"><i class="fa-solid fa-calendar-check"></i></div>
                                <h3>{"Never Forget Again"}</h3>
                                <p>{"Lightfriend has saved me so many times. I\u{2019}ll forget a deadline or miss an important email \u{2014} but then Lightfriend pings me about it before it\u{2019}s too late. It watches my inbox and calendar so I don\u{2019}t have to. Honestly, I\u{2019}d be lost without it. \u{2014} Kasperi"}</p>
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

            <section class="capabilities-summary scroll-animate">
                <div class="capabilities-content">
                    <p class="capabilities-tagline">{"Lightfriend is an AI assistant that gives dumbphone users access to WhatsApp, email, calendar, web search, and more via SMS and voice calls."}</p>
                    <div class="capabilities-grid">
                        <div class="capability-category">
                            <h3>{"Integrations"}</h3>
                            <p>{"WhatsApp, Telegram, Signal, Email, Google Calendar, MCP-Server"}</p>
                        </div>
                        <div class="capability-category">
                            <h3>{"Interfaces"}</h3>
                            <p>{"Voice calls, SMS"}</p>
                        </div>
                        <div class="capability-category">
                            <h3>{"Features"}</h3>
                            <p>{"Web search, directions, image translation, QR scanning"}</p>
                        </div>
                        <div class="capability-category">
                            <h3>{"Works with"}</h3>
                            <p>{"Any phone that can call or text"}</p>
                        </div>
                    </div>
                    <div class="availability-info">
                        <p><strong>{"Full service: "}</strong>{"US, Canada, UK, Finland, Netherlands, Australia"}</p>
                        <p><strong>{"Also available: "}</strong>{"15+ EU countries and more"}</p>
                    </div>
                </div>
            </section>

            <div class="filter-concept">
                <div class="filter-content">
                    <AnimationComponent />
                </div>
            </div>
            <section class="trust-proof scroll-animate">
                <div class="section-intro">
                    <h2>{"The Story"}</h2>
                    <img src="/assets/rasmus-pfp.png" alt="Rasmus" loading="lazy" style="max-width: 200px; border-radius: 50%; margin: 0 auto 1.5rem; display: block;"/>
                    <p class="why-lead">{"This isn\u{2019}t a compromise. It\u{2019}s what made things possible."}</p>
                    <p>{"I\u{2019}m Rasmus. I didn\u{2019}t build Lightfriend despite using a dumbphone. I built it because of it."}</p>
                    <button class="story-toggle-btn" onclick={
                        let comic_open = comic_open.clone();
                        Callback::from(move |_: MouseEvent| {
                            comic_open.set(!*comic_open);
                        })
                    }>
                        <i class={if *comic_open { "fa-solid fa-chevron-up" } else { "fa-solid fa-book-open" }}></i>
                        <span>{if *comic_open { "Hide the story" } else { "Read the full story" }}</span>
                    </button>
                </div>
            </section>
            <section class={classes!("comic-section", if *comic_open { "comic-open" } else { "comic-closed" })}>
                <div class="comic-grid">
                    // Panel 1: Rasmus hunched over smartphone, scrolling
                    <div class="comic-panel comic-panel-1 scroll-animate">
                        <div class="comic-icon">
                            <svg viewBox="0 0 140 160" class="comic-svg">
                                // Head - slightly tilted down
                                <circle cx="70" cy="28" r="18" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                // Eyes looking down at phone
                                <circle cx="63" cy="26" r="2" fill="currentColor"/>
                                <circle cx="77" cy="26" r="2" fill="currentColor"/>
                                // Neutral mouth
                                <line x1="65" y1="35" x2="75" y2="35" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
                                // Body - hunched posture
                                <path d="M 70 46 Q 68 70, 70 95" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                // Left arm reaching to phone
                                <path d="M 70 58 Q 55 68, 58 82" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                // Right arm reaching to phone
                                <path d="M 70 58 Q 85 68, 82 82" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                // Legs
                                <path d="M 70 95 Q 60 115, 52 140" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                <path d="M 70 95 Q 80 115, 88 140" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                // Big smartphone in hands
                                <rect x="56" y="76" width="28" height="42" rx="4" fill="rgba(126,178,255,0.1)" stroke="#7EB2FF" stroke-width="2"/>
                                // Screen glow lines
                                <line x1="62" y1="84" x2="78" y2="84" stroke="#7EB2FF" stroke-width="1" opacity="0.5"/>
                                <line x1="62" y1="90" x2="75" y2="90" stroke="#7EB2FF" stroke-width="1" opacity="0.5"/>
                                <line x1="62" y1="96" x2="78" y2="96" stroke="#7EB2FF" stroke-width="1" opacity="0.5"/>
                                <line x1="62" y1="102" x2="72" y2="102" stroke="#7EB2FF" stroke-width="1" opacity="0.5"/>
                                // Screen glow effect
                                <rect x="58" y="78" width="24" height="36" rx="2" fill="rgba(126,178,255,0.06)"/>
                                // Notification bubbles floating up
                                <circle cx="48" cy="65" r="5" fill="none" stroke="#ff6b6b" stroke-width="1.5" opacity="0.6" class="comic-float"/>
                                <circle cx="95" cy="58" r="4" fill="none" stroke="#ff6b6b" stroke-width="1.5" opacity="0.4" class="comic-float-slow"/>
                                <circle cx="42" cy="48" r="3" fill="none" stroke="#ff6b6b" stroke-width="1.5" opacity="0.3" class="comic-float"/>
                            </svg>
                        </div>
                        <p class="comic-text">{"Rasmus use smartphone"}</p>
                    </div>

                    // Panel 2: Apps eating time
                    <div class="comic-panel comic-panel-2 scroll-animate">
                        <div class="comic-icon">
                            <svg viewBox="0 0 140 160" class="comic-svg">
                                // Dizzy head
                                <circle cx="70" cy="28" r="18" fill="none" stroke="currentColor" stroke-width="2.5"/>
                                // Spiral eyes (dizzy)
                                <path d="M 60 24 Q 63 20, 66 24 Q 63 28, 60 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
                                <path d="M 74 24 Q 77 20, 80 24 Q 77 28, 74 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
                                // Open mouth - confused
                                <ellipse cx="70" cy="37" rx="4" ry="3" fill="none" stroke="currentColor" stroke-width="2"/>
                                // Body
                                <path d="M 70 46 L 70 95" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                // Arms up in frustration
                                <path d="M 70 60 Q 45 50, 35 60" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                <path d="M 70 60 Q 95 50, 105 60" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                // Legs
                                <path d="M 70 95 L 55 140" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                <path d="M 70 95 L 85 140" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                // Floating clocks around
                                <circle cx="25" cy="35" r="14" fill="none" stroke="#ff6b6b" stroke-width="1.5" opacity="0.7" class="comic-float"/>
                                <line x1="25" y1="35" x2="25" y2="25" stroke="#ff6b6b" stroke-width="1.5" opacity="0.7"/>
                                <line x1="25" y1="35" x2="33" y2="35" stroke="#ff6b6b" stroke-width="1.5" opacity="0.7"/>
                                <circle cx="115" cy="40" r="12" fill="none" stroke="#ff6b6b" stroke-width="1.5" opacity="0.5" class="comic-float-slow"/>
                                <line x1="115" y1="40" x2="115" y2="32" stroke="#ff6b6b" stroke-width="1.5" opacity="0.5"/>
                                <line x1="115" y1="40" x2="121" y2="40" stroke="#ff6b6b" stroke-width="1.5" opacity="0.5"/>
                                // App icons floating
                                <rect x="18" y="80" width="16" height="16" rx="4" fill="none" stroke="#ff6b6b" stroke-width="1.5" opacity="0.4" class="comic-float-slow"/>
                                <rect x="106" y="85" width="16" height="16" rx="4" fill="none" stroke="#ff6b6b" stroke-width="1.5" opacity="0.3" class="comic-float"/>
                                // Sweat drop
                                <path d="M 88 18 Q 92 10, 90 22" fill="#7EB2FF" opacity="0.5"/>
                            </svg>
                        </div>
                        <p class="comic-text">{"Apps waste Rasmus time"}</p>
                    </div>

                    // Panel 3: Rasmus sad
                    <div class="comic-panel comic-panel-3 scroll-animate">
                        <div class="comic-icon">
                            <svg viewBox="0 0 140 160" class="comic-svg">
                                // Small rain cloud above
                                <path d="M 50 15 Q 50 5, 60 5 Q 65 -2, 75 5 Q 80 0, 85 5 Q 95 5, 95 15 Q 100 15, 95 22 L 50 22 Q 45 15, 50 15" fill="none" stroke="rgba(255,255,255,0.3)" stroke-width="1.5"/>
                                // Rain drops
                                <line x1="58" y1="25" x2="56" y2="33" stroke="rgba(126,178,255,0.4)" stroke-width="1.5" class="comic-rain"/>
                                <line x1="70" y1="25" x2="68" y2="33" stroke="rgba(126,178,255,0.4)" stroke-width="1.5" class="comic-rain-delay"/>
                                <line x1="82" y1="25" x2="80" y2="33" stroke="rgba(126,178,255,0.4)" stroke-width="1.5" class="comic-rain"/>
                                // Sad head - tilted down
                                <circle cx="70" cy="52" r="18" fill="none" stroke="currentColor" stroke-width="2.5"/>
                                // Sad eyes
                                <path d="M 60 50 Q 63 47, 66 50" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
                                <path d="M 74 50 Q 77 47, 80 50" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
                                // Sad frown
                                <path d="M 62 62 Q 70 56, 78 62" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                // Hunched body
                                <path d="M 70 70 Q 68 85, 72 110" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                // Arms hanging down
                                <path d="M 70 80 Q 52 90, 48 108" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                <path d="M 70 80 Q 88 90, 92 108" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                // Legs - sitting posture
                                <path d="M 72 110 Q 58 120, 50 140" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                <path d="M 72 110 Q 82 120, 90 140" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                            </svg>
                        </div>
                        <p class="comic-text">{"Rasmus sad"}</p>
                    </div>

                    // Panel 4: Rasmus picks up Nokia
                    <div class="comic-panel comic-panel-4 scroll-animate">
                        <div class="comic-icon">
                            <svg viewBox="0 0 140 160" class="comic-svg">
                                // Head - determined
                                <circle cx="70" cy="28" r="18" fill="none" stroke="currentColor" stroke-width="2.5"/>
                                // Determined eyes
                                <circle cx="63" cy="26" r="2.5" fill="currentColor"/>
                                <circle cx="77" cy="26" r="2.5" fill="currentColor"/>
                                // Slight smile
                                <path d="M 65 36 Q 70 39, 75 36" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
                                // Straight body - good posture
                                <line x1="70" y1="46" x2="70" y2="100" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                // Left arm on hip
                                <path d="M 70 62 Q 48 68, 50 80 Q 52 88, 62 85" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                // Right arm holding small Nokia
                                <path d="M 70 62 Q 90 65, 95 75" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                // Legs - standing straight
                                <line x1="70" y1="100" x2="56" y2="142" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                <line x1="70" y1="100" x2="84" y2="142" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                // Feet
                                <line x1="56" y1="142" x2="48" y2="142" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                <line x1="84" y1="142" x2="92" y2="142" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                // Small Nokia in right hand
                                <rect x="92" y="70" width="12" height="20" rx="3" fill="none" stroke="#66BB6A" stroke-width="2"/>
                                <line x1="95" y1="76" x2="101" y2="76" stroke="#66BB6A" stroke-width="1" opacity="0.5"/>
                                <circle cx="98" cy="84" r="2" fill="none" stroke="#66BB6A" stroke-width="1" opacity="0.5"/>
                                // Sparkle near Nokia
                                <path d="M 110 65 L 112 60 L 114 65 L 119 67 L 114 69 L 112 74 L 110 69 L 105 67 Z" fill="#66BB6A" opacity="0.5" class="comic-sparkle"/>
                            </svg>
                        </div>
                        <p class="comic-text">{"Rasmus use old Nokia"}</p>
                    </div>

                    // Panel 5: Rasmus calls Lightfriend
                    <div class="comic-panel comic-panel-5 scroll-animate">
                        <div class="comic-icon">
                            <svg viewBox="0 0 140 160" class="comic-svg">
                                // Head
                                <circle cx="55" cy="50" r="18" fill="none" stroke="currentColor" stroke-width="2.5"/>
                                // Happy eyes
                                <circle cx="48" cy="48" r="2.5" fill="currentColor"/>
                                <circle cx="62" cy="48" r="2.5" fill="currentColor"/>
                                // Talking mouth
                                <ellipse cx="55" cy="58" rx="5" ry="3.5" fill="none" stroke="currentColor" stroke-width="2"/>
                                // Body
                                <line x1="55" y1="68" x2="55" y2="115" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                // Left arm down
                                <path d="M 55 80 Q 38 90, 35 105" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                // Right arm holding phone to ear
                                <path d="M 55 78 Q 65 70, 68 55" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                // Phone at ear
                                <rect x="65" y="45" width="10" height="18" rx="3" fill="none" stroke="#66BB6A" stroke-width="2"/>
                                // Legs
                                <line x1="55" y1="115" x2="42" y2="148" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                <line x1="55" y1="115" x2="68" y2="148" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                // Speech bubble going to AI
                                <path d="M 78 38 Q 85 30, 95 28 Q 105 25, 115 28 Q 128 30, 130 40 Q 132 50, 122 55 Q 112 58, 100 55 Q 90 52, 85 45 Q 82 42, 78 38" fill="rgba(126,178,255,0.08)" stroke="#7EB2FF" stroke-width="1.5"/>
                                // "AI" text in bubble
                                <text x="100" y="44" fill="#7EB2FF" font-size="14" font-weight="700" text-anchor="middle">{"AI"}</text>
                                // Signal waves from phone
                                <path d="M 78 48 Q 82 45, 82 50" fill="none" stroke="#7EB2FF" stroke-width="1" opacity="0.5" class="comic-wave"/>
                                <path d="M 82 44 Q 88 40, 88 52" fill="none" stroke="#7EB2FF" stroke-width="1" opacity="0.3" class="comic-wave"/>
                            </svg>
                        </div>
                        <p class="comic-text">{"Rasmus call Lightfriend when need smart thing"}</p>
                    </div>

                    // Panel 6: Rasmus happy and free
                    <div class="comic-panel comic-panel-6 scroll-animate">
                        <div class="comic-icon">
                            <svg viewBox="0 0 140 160" class="comic-svg">
                                // Sun in corner
                                <circle cx="115" cy="20" r="12" fill="none" stroke="#FFD54F" stroke-width="2" opacity="0.6"/>
                                <line x1="115" y1="2" x2="115" y2="-2" stroke="#FFD54F" stroke-width="2" opacity="0.5"/>
                                <line x1="115" y1="38" x2="115" y2="42" stroke="#FFD54F" stroke-width="2" opacity="0.5"/>
                                <line x1="97" y1="20" x2="93" y2="20" stroke="#FFD54F" stroke-width="2" opacity="0.5"/>
                                <line x1="133" y1="20" x2="137" y2="20" stroke="#FFD54F" stroke-width="2" opacity="0.5"/>
                                <line x1="103" y1="8" x2="100" y2="5" stroke="#FFD54F" stroke-width="2" opacity="0.4"/>
                                <line x1="127" y1="8" x2="130" y2="5" stroke="#FFD54F" stroke-width="2" opacity="0.4"/>
                                <line x1="103" y1="32" x2="100" y2="35" stroke="#FFD54F" stroke-width="2" opacity="0.4"/>
                                <line x1="127" y1="32" x2="130" y2="35" stroke="#FFD54F" stroke-width="2" opacity="0.4"/>
                                // Happy head
                                <circle cx="65" cy="42" r="18" fill="none" stroke="currentColor" stroke-width="2.5"/>
                                // Happy closed eyes (arcs)
                                <path d="M 56 39 Q 59 35, 62 39" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                <path d="M 68 39 Q 71 35, 74 39" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                // Big smile
                                <path d="M 56 50 Q 65 60, 74 50" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                // Body - jumping
                                <line x1="65" y1="60" x2="65" y2="105" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                // Arms up in joy
                                <path d="M 65 72 Q 42 55, 30 40" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                <path d="M 65 72 Q 85 58, 92 48" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                // Open hands at top
                                <circle cx="28" cy="38" r="3" fill="none" stroke="currentColor" stroke-width="2"/>
                                <circle cx="94" cy="46" r="3" fill="none" stroke="currentColor" stroke-width="2"/>
                                // Legs spread - jumping
                                <path d="M 65 105 Q 50 120, 40 140" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                <path d="M 65 105 Q 80 120, 90 140" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/>
                                // Ground shadow
                                <ellipse cx="65" cy="150" rx="25" ry="4" fill="rgba(255,255,255,0.06)"/>
                                // Sparkles around
                                <path d="M 20 65 L 22 60 L 24 65 L 29 67 L 24 69 L 22 74 L 20 69 L 15 67 Z" fill="#FFD54F" opacity="0.5" class="comic-sparkle"/>
                                <path d="M 100 75 L 101 72 L 102 75 L 105 76 L 102 77 L 101 80 L 100 77 L 97 76 Z" fill="#66BB6A" opacity="0.5" class="comic-sparkle-delay"/>
                                <path d="M 35 90 L 36 87 L 37 90 L 40 91 L 37 92 L 36 95 L 35 92 L 32 91 Z" fill="#7EB2FF" opacity="0.4" class="comic-sparkle"/>
                            </svg>
                        </div>
                        <p class="comic-text">{"Rasmus happy and free"}</p>
                    </div>
                </div>
            </section>
            <section class="features-section">
                <div class="feature-list scroll-animate">
                    <style>{feature_css}</style>
                    <h2>{"Your Pocket AI"}</h2>
                    <div class="feature-categories-grid">
                        // Messaging & Communication
                        <div class="feature-category-card">
                            <div class="feature-category-header">
                                <i class="fa-solid fa-comments"></i>
                                <h3>{"Messaging & Communication"}</h3>
                            </div>
                            <div class="feature-category-items">
                                <div class="feature-item">
                                    <div class="feature-item-header" onclick={make_toggle("whatsapp")}>
                                        <i class="fab fa-whatsapp"></i>
                                        <span>{"WhatsApp"}</span>
                                        <span class={classes!("feature-item-arrow", (*expanded_feature == "whatsapp").then_some("expanded"))}>{"\u{25b6}"}</span>
                                    </div>
                                    if *expanded_feature == "whatsapp" {
                                        <div class="feature-desc">
                                            <p>{"Link your WhatsApp account in the web dashboard. Then, send messages, fetch recent messages or from specific chats, and monitor for new messages with automatic SMS or call notifications for important updates."}</p>
                                            <div class="feature-preview-wrapper">
                                                <span class="feature-preview-badge">{"Preview"}</span>
                                                <img class="feature-preview-img" src="/assets/previews/whatsapp-preview.gif" alt="WhatsApp integration preview" loading="lazy"/>
                                            </div>
                                        </div>
                                    }
                                </div>
                                <div class="feature-item">
                                    <div class="feature-item-header" onclick={make_toggle("telegram")}>
                                        <i class="fab fa-telegram"></i>
                                        <span>{"Telegram"}</span>
                                        <span class={classes!("feature-item-arrow", (*expanded_feature == "telegram").then_some("expanded"))}>{"\u{25b6}"}</span>
                                    </div>
                                    if *expanded_feature == "telegram" {
                                        <div class="feature-desc">
                                            <p>{"Link your Telegram account in the web dashboard. Then, send messages, fetch recent messages or from specific chats, and monitor for new messages with automatic SMS or call notifications for important updates."}</p>
                                        </div>
                                    }
                                </div>
                                <div class="feature-item">
                                    <div class="feature-item-header" onclick={make_toggle("signal")}>
                                        <i class="fab fa-signal-messenger"></i>
                                        <span>{"Signal"}</span>
                                        <span class={classes!("feature-item-arrow", (*expanded_feature == "signal").then_some("expanded"))}>{"\u{25b6}"}</span>
                                    </div>
                                    if *expanded_feature == "signal" {
                                        <div class="feature-desc">
                                            <p>{"Link your Signal account in the web dashboard. Then, send messages, fetch recent messages or from specific chats, and monitor for new messages with automatic SMS or call notifications for important updates."}</p>
                                        </div>
                                    }
                                </div>
                                <div class="feature-item">
                                    <div class="feature-item-header" onclick={make_toggle("email")}>
                                        <i class="fas fa-envelope"></i>
                                        <span>{"Email"}</span>
                                        <span class={classes!("feature-item-arrow", (*expanded_feature == "email").then_some("expanded"))}>{"\u{25b6}"}</span>
                                    </div>
                                    if *expanded_feature == "email" {
                                        <div class="feature-desc">
                                            <p>{"Integrate your email (Gmail, Outlook, etc.) in the settings. Fetch recent emails, find specific information, and monitor for important ones with AI-filtered notifications sent to your phone via SMS or make it call you."}</p>
                                            <div class="feature-preview-wrapper">
                                                <span class="feature-preview-badge">{"Preview"}</span>
                                                <img class="feature-preview-img" src="/assets/previews/email-preview.gif" alt="Email integration preview" loading="lazy"/>
                                            </div>
                                        </div>
                                    }
                                </div>
                                <div class="feature-item">
                                    <div class="feature-item-header" onclick={make_toggle("calendar")}>
                                        <i class="fas fa-calendar-days"></i>
                                        <span>{"Calendar"}</span>
                                        <span class={classes!("feature-item-arrow", (*expanded_feature == "calendar").then_some("expanded"))}>{"\u{25b6}"}</span>
                                    </div>
                                    if *expanded_feature == "calendar" {
                                        <div class="feature-desc">
                                            <p>{"Sync with Google Calendar. View events, create new ones, set reminders, and get reminded via SMS or call."}</p>
                                            <div class="feature-preview-wrapper">
                                                <span class="feature-preview-badge">{"Preview"}</span>
                                                <img class="feature-preview-img" src="/assets/previews/calendar-preview.gif" alt="Calendar integration preview" loading="lazy"/>
                                            </div>
                                        </div>
                                    }
                                </div>
                            </div>
                        </div>
                        // Smart Tools
                        <div class="feature-category-card">
                            <div class="feature-category-header">
                                <i class="fa-solid fa-lightbulb"></i>
                                <h3>{"Smart Tools"}</h3>
                            </div>
                            <div class="feature-category-items">
                                <div class="feature-item">
                                    <div class="feature-item-header" onclick={make_toggle("voice")}>
                                        <i class="fas fa-phone"></i>
                                        <span>{"Voice Calls"}</span>
                                        <span class={classes!("feature-item-arrow", (*expanded_feature == "voice").then_some("expanded"))}>{"\u{25b6}"}</span>
                                    </div>
                                    if *expanded_feature == "voice" {
                                        <div class="feature-desc">
                                            <p>{"Access all of Lightfriend's features through natural voice calls. Simply dial and have a conversation with your AI assistant. No smartphone or internet connection needed - works with any basic phone that can make calls."}</p>
                                            <video class="feature-video" src="/assets/lightfriend-demo.mp4" controls=true autoplay=false loop=false muted=false></video>
                                        </div>
                                    }
                                </div>
                                <div class="feature-item">
                                    <div class="feature-item-header" onclick={make_toggle("sms")}>
                                        <i class="fa-solid fa-comment-sms"></i>
                                        <span>{"SMS Chat"}</span>
                                        <span class={classes!("feature-item-arrow", (*expanded_feature == "sms").then_some("expanded"))}>{"\u{25b6}"}</span>
                                    </div>
                                    if *expanded_feature == "sms" {
                                        <div class="feature-desc">
                                            <p>{"Use all of Lightfriend's capabilities through simple text messages. Your conversation context is remembered between SMS and voice calls for seamless continuity. Works with any basic phone that can send texts."}</p>
                                        </div>
                                    }
                                </div>
                                <div class="feature-item">
                                    <div class="feature-item-header" onclick={make_toggle("search")}>
                                        <i class="fas fa-search"></i>
                                        <span>{"Web Search"}</span>
                                        <span class={classes!("feature-item-arrow", (*expanded_feature == "search").then_some("expanded"))}>{"\u{25b6}"}</span>
                                    </div>
                                    if *expanded_feature == "search" {
                                        <div class="feature-desc">
                                            <p>{"Powered by Perplexity AI. Query anything via voice call or SMS \u{2014} from local restaurant reviews to stock prices, store hours to landmark info. Provides accurate, real-time answers with sources."}</p>
                                        </div>
                                    }
                                </div>
                                <div class="feature-item">
                                    <div class="feature-item-header" onclick={make_toggle("weather")}>
                                        <i class="fas fa-cloud-sun"></i>
                                        <span>{"Weather"}</span>
                                        <span class={classes!("feature-item-arrow", (*expanded_feature == "weather").then_some("expanded"))}>{"\u{25b6}"}</span>
                                    </div>
                                    if *expanded_feature == "weather" {
                                        <div class="feature-desc">
                                            <p>{"Request weather information for any location via SMS or voice. Receive current conditions, temperature, a detailed 6-hour forecast, or up to 7 days ahead."}</p>
                                        </div>
                                    }
                                </div>
                                <div class="feature-item">
                                    <div class="feature-item-header" onclick={make_toggle("directions")}>
                                        <i class="fas fa-route"></i>
                                        <span>{"Directions"}</span>
                                        <span class={classes!("feature-item-arrow", (*expanded_feature == "directions").then_some("expanded"))}>{"\u{25b6}"}</span>
                                    </div>
                                    if *expanded_feature == "directions" {
                                        <div class="feature-desc">
                                            <p>{"Get detailed turn-by-turn walking directions between any two locations via SMS or voice call. Powered by Google Maps."}</p>
                                        </div>
                                    }
                                </div>
                                <div class="feature-item">
                                    <div class="feature-item-header" onclick={make_toggle("photo")}>
                                        <i class="fas fa-image"></i>
                                        <span>{"Photo Analysis"}</span>
                                        <span class={classes!("feature-item-arrow", (*expanded_feature == "photo").then_some("expanded"))}>{"\u{25b6}"}</span>
                                    </div>
                                    if *expanded_feature == "photo" {
                                        <div class="feature-desc">
                                            <p>{"Send a photo via MMS to Lightfriend; the AI analyzes the image content or translates any visible text. Available in US, Canada and Australia."}</p>
                                        </div>
                                    }
                                </div>
                                <div class="feature-item">
                                    <div class="feature-item-header" onclick={make_toggle("qr")}>
                                        <i class="fas fa-qrcode"></i>
                                        <span>{"QR Scanning"}</span>
                                        <span class={classes!("feature-item-arrow", (*expanded_feature == "qr").then_some("expanded"))}>{"\u{25b6}"}</span>
                                    </div>
                                    if *expanded_feature == "qr" {
                                        <div class="feature-desc">
                                            <p>{"Take a photo of a QR code and send it via MMS; Lightfriend decodes it and sends back the information. Available in US, Canada and Australia."}</p>
                                        </div>
                                    }
                                </div>
                            </div>
                        </div>
                        // Proactive Monitoring
                        <div class="feature-category-card">
                            <div class="feature-category-header">
                                <i class="fa-solid fa-bell"></i>
                                <h3>{"Proactive Monitoring"}</h3>
                            </div>
                            <div class="feature-category-items">
                                <div class="feature-item">
                                    <div class="feature-item-header" onclick={make_toggle("critical")}>
                                        <i class="fas fa-eye"></i>
                                        <span>{"24/7 Critical Alerts"}</span>
                                        <span class={classes!("feature-item-arrow", (*expanded_feature == "critical").then_some("expanded"))}>{"\u{25b6}"}</span>
                                    </div>
                                    if *expanded_feature == "critical" {
                                        <div class="feature-desc">
                                            <p>{"AI constantly scans your connected apps for critical or urgent messages. If detected as critical, you'll receive an immediate notification via SMS or call."}</p>
                                        </div>
                                    }
                                </div>
                                <div class="feature-item">
                                    <div class="feature-item-header" onclick={make_toggle("digests")}>
                                        <i class="fas fa-newspaper"></i>
                                        <span>{"Daily Digests"}</span>
                                        <span class={classes!("feature-item-arrow", (*expanded_feature == "digests").then_some("expanded"))}>{"\u{25b6}"}</span>
                                    </div>
                                    if *expanded_feature == "digests" {
                                        <div class="feature-desc">
                                            <p>{"Get automated, AI-summarized digests of your messages, emails, and calendar events sent via SMS at set times: morning overview, midday update, and evening recap."}</p>
                                        </div>
                                    }
                                </div>
                                <div class="feature-item">
                                    <div class="feature-item-header" onclick={make_toggle("temp-monitor")}>
                                        <i class="fas fa-clock"></i>
                                        <span>{"Temporary Monitoring"}</span>
                                        <span class={classes!("feature-item-arrow", (*expanded_feature == "temp-monitor").then_some("expanded"))}>{"\u{25b6}"}</span>
                                    </div>
                                    if *expanded_feature == "temp-monitor" {
                                        <div class="feature-desc">
                                            <p>{"Set up short-term monitoring for specific content in your apps. Notifications are sent via SMS/call and once found the temporary monitoring task is removed."}</p>
                                        </div>
                                    }
                                </div>
                                <div class="feature-item">
                                    <div class="feature-item-header" onclick={make_toggle("priority")}>
                                        <i class="fas fa-star"></i>
                                        <span>{"Priority Senders"}</span>
                                        <span class={classes!("feature-item-arrow", (*expanded_feature == "priority").then_some("expanded"))}>{"\u{25b6}"}</span>
                                    </div>
                                    if *expanded_feature == "priority" {
                                        <div class="feature-desc">
                                            <p>{"Designate priority contacts in the dashboard. Any messages from them trigger instant notifications to your phone via SMS or voice call."}</p>
                                        </div>
                                    }
                                </div>
                            </div>
                        </div>
                        // More
                        <div class="feature-category-card">
                            <div class="feature-category-header">
                                <i class="fa-solid fa-rocket"></i>
                                <h3>{"More"}</h3>
                            </div>
                            <div class="feature-category-items">
                                <div class="feature-item">
                                    <div class="feature-item-header" onclick={make_toggle("tesla")}>
                                        <i class="fas fa-car"></i>
                                        <span>{"Tesla Control"}</span>
                                        <span class={classes!("feature-item-arrow", (*expanded_feature == "tesla").then_some("expanded"))}>{"\u{25b6}"}</span>
                                    </div>
                                    if *expanded_feature == "tesla" {
                                        <div class="feature-desc">
                                            <p>{"Connect your Tesla and control it via voice call, SMS, or the web dashboard. Lock/unlock doors, climate control, defrost mode, and battery status."}</p>
                                            <div class="feature-preview-wrapper">
                                                <span class="feature-preview-badge">{"Preview"}</span>
                                                <img class="feature-preview-img" src="/assets/previews/tesla-controls-preview.gif" alt="Tesla controls preview" loading="lazy"/>
                                            </div>
                                        </div>
                                    }
                                </div>
                                <div class="feature-item">
                                    <div class="feature-item-header" onclick={make_toggle("future")}>
                                        <i class="fas fa-rocket"></i>
                                        <span>{"All Future Features"}</span>
                                        <span class={classes!("feature-item-arrow", (*expanded_feature == "future").then_some("expanded"))}>{"\u{25b6}"}</span>
                                    </div>
                                    if *expanded_feature == "future" {
                                        <div class="feature-desc">
                                            <p>{"As a subscriber, you'll automatically receive access to all upcoming features and updates without any price increase. Early subscribers keep their original lower price permanently."}</p>
                                        </div>
                                    }
                                </div>
                                <div class="feature-item">
                                    <div class="feature-item-header" onclick={make_toggle("support")}>
                                        <i class="fas fa-headset"></i>
                                        <span>{"Priority Support"}</span>
                                        <span class={classes!("feature-item-arrow", (*expanded_feature == "support").then_some("expanded"))}>{"\u{25b6}"}</span>
                                    </div>
                                    if *expanded_feature == "support" {
                                        <div class="feature-desc">
                                            <p>{"Dedicated, fast-response support from the developer. Reach out via email (rasmus@ahtava.com) for help with setup, troubleshooting, or feature requests."}</p>
                                        </div>
                                    }
                                </div>
                            </div>
                        </div>
                    </div>
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
                            {"I have ADHD and smartphones were my kryptonite — I'd pick it up to check one message and lose an hour. With Lightfriend, I get a text with what actually matters. No apps to open, no rabbit holes. It's been life-changing for my focus and productivity."}
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
                            <p>{"Your data’s safe. Lightfriend runs on a secure EU server with no logging of your chats, searches, or personal info. All credentials are encrypted, and optional conversation history gets deleted automatically as you go - my server would fill up fast otherwise. Messaging app chats (like WhatsApp) are temporary too: they’re only accessible for 2 days after receiving them, then gone. I’m a solo dev, not some data-hungry corp. The code’s open-source on GitHub, anyone can check it’s legit. It’s a hosted app, so some trust is needed, but you own your data and can delete it anytime, no questions."}</p>
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
                    <p class="disclaimer">{"Works with smartphones and basic phones. Customize to your needs."}</p>
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
                        <p>{"Source code on "}
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
    details {
        cursor: pointer;
        margin-bottom: 0.5rem;
    }
    summary {
        display: flex;
        align-items: center;
        list-style: none;
        gap: 1rem;
        padding-right: 0; /* No extra padding, arrow will be in its own space */
    }
    summary::after {
        content: '▶';
        font-size: 0.8rem;
        color: #7EB2FF;
        margin-left: auto; /* Pushes arrow to the right without stretching it */
        flex-shrink: 0; /* Prevent arrow from moving when content changes */
        transition: transform 0.3s ease;
    }
    details summary {
        display: flex;
        align-items: center;
        cursor: pointer;
    }
    details summary::after {
        content: "▶";
        margin-left: 8px;
        transition: transform 0.2s;
    }
    details[open] summary::after {
        transform: rotate(90deg);
    }
    .feature-desc {
        padding: 1rem;
        background: rgba(0, 0, 0, 0.2);
        border-radius: 8px;
        color: #ddd;
        font-size: 1rem;
        margin-top: 0.5rem;
    }
    @media (max-width: 768px) {
        .feature-desc {
            font-size: 0.9rem;
        }
    }
    .hero-overlay {
        position: fixed;
        top: 0;
        left: 0;
        width: 100%;
        height: 100vh;
        background: rgba(0, 0, 0, 0.7);
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
    .trust-proof .why-lead {
        font-size: 1.5rem;
        color: #fff;
        font-weight: 500;
        font-style: italic;
        margin-bottom: 2rem;
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
        overflow-x: hidden;
        box-sizing: border-box;
        z-index: 0;
    }
    .landing-page > section:not(.comic-section),
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
        padding-top: 45vh;
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
    .story-toggle-btn {
        display: inline-flex;
        align-items: center;
        gap: 0.6rem;
        margin-top: 1.5rem;
        padding: 0.8rem 1.6rem;
        background: rgba(255, 255, 255, 0.12);
        border: 1px solid rgba(255, 255, 255, 0.3);
        border-radius: 30px;
        color: #7EB2FF;
        font-size: 0.95rem;
        font-weight: 500;
        cursor: pointer;
        transition: all 0.3s ease;
        backdrop-filter: blur(10px);
    }
    .story-toggle-btn:hover {
        background: rgba(126, 178, 255, 0.22);
        border-color: rgba(255, 255, 255, 0.4);
        transform: translateY(-2px);
        box-shadow: 0 4px 20px rgba(255, 255, 255, 0.2);
    }
    .story-toggle-btn i {
        font-size: 1rem;
    }
    /* Comic section toggle */
    .comic-section.comic-closed {
        max-height: 0;
        overflow: hidden;
        opacity: 0;
        padding: 0;
        margin: 0;
        transition: max-height 0.6s cubic-bezier(0.16, 1, 0.3, 1),
                    opacity 0.4s ease,
                    padding 0.6s ease;
    }
    .comic-section.comic-open {
        max-height: 2000px;
        opacity: 1;
        transition: max-height 0.8s cubic-bezier(0.16, 1, 0.3, 1),
                    opacity 0.6s ease 0.1s,
                    padding 0.6s ease;
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
    /* ========== Animated Comic Strip ========== */
    .comic-section {
        padding: 4rem 2rem;
        max-width: 900px;
        margin: 0 auto;
        position: relative;
        z-index: 2;
    }
    .comic-grid {
        display: grid;
        grid-template-columns: repeat(3, 1fr);
        gap: 1.5rem;
    }
    .comic-panel {
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 16px;
        padding: 2rem 1.5rem;
        text-align: center;
        transition: all 0.4s ease;
        position: relative;
        overflow: hidden;
    }
    .comic-panel:hover {
        border-color: rgba(255, 255, 255, 0.25);
        background: rgba(126, 178, 255, 0.04);
        transform: translateY(-3px);
    }
    .comic-icon {
        width: 100%;
        max-width: 180px;
        height: 200px;
        margin: 0 auto 1.25rem;
        display: flex;
        align-items: center;
        justify-content: center;
    }
    .comic-svg {
        width: 100%;
        height: 100%;
        color: rgba(255, 255, 255, 0.8);
    }
    .comic-text {
        font-family: 'Cormorant Garamond', Georgia, serif;
        font-style: italic;
        font-size: 1.2rem;
        color: rgba(255, 255, 255, 0.75);
        font-weight: 300;
        letter-spacing: 0.02em;
        line-height: 1.5;
    }
    .comic-panel-1 { transition-delay: 0s; }
    .comic-panel-2 { transition-delay: 0.1s; }
    .comic-panel-3 { transition-delay: 0.2s; }
    .comic-panel-4 { transition-delay: 0.3s; }
    .comic-panel-5 { transition-delay: 0.4s; }
    .comic-panel-6 { transition-delay: 0.5s; }
    .comic-panel-3 { border-color: rgba(255, 107, 107, 0.12); }
    .comic-panel-3:hover { border-color: rgba(255, 107, 107, 0.3); }
    .comic-panel-6 { border-color: rgba(255, 213, 79, 0.12); }
    .comic-panel-6:hover {
        border-color: rgba(255, 213, 79, 0.3);
        box-shadow: 0 0 20px rgba(255, 213, 79, 0.08);
    }
    .comic-panel::before {
        content: '';
        position: absolute;
        top: 8px;
        left: 8px;
        width: 20px;
        height: 20px;
        border-radius: 50%;
        border: 1px solid rgba(255, 255, 255, 0.12);
        font-size: 0.65rem;
        color: rgba(255, 255, 255, 0.25);
        display: flex;
        align-items: center;
        justify-content: center;
    }
    .comic-panel-1::before { content: '1'; }
    .comic-panel-2::before { content: '2'; }
    .comic-panel-3::before { content: '3'; }
    .comic-panel-4::before { content: '4'; }
    .comic-panel-5::before { content: '5'; }
    .comic-panel-6::before { content: '6'; }
    @keyframes comicFloat {
        0%, 100% { transform: translateY(0); opacity: 0.6; }
        50% { transform: translateY(-6px); opacity: 0.3; }
    }
    @keyframes comicFloatSlow {
        0%, 100% { transform: translateY(0); opacity: 0.5; }
        50% { transform: translateY(-4px); opacity: 0.2; }
    }
    @keyframes comicRain {
        0% { transform: translateY(0); opacity: 0.5; }
        100% { transform: translateY(8px); opacity: 0; }
    }
    @keyframes comicSparkle {
        0%, 100% { transform: scale(1); opacity: 0.5; }
        50% { transform: scale(1.3); opacity: 0.9; }
    }
    @keyframes comicWave {
        0%, 100% { transform: scale(1); opacity: 0.5; }
        50% { transform: scale(1.15); opacity: 0.2; }
    }
    .comic-float { animation: comicFloat 3s ease-in-out infinite; }
    .comic-float-slow { animation: comicFloatSlow 4s ease-in-out infinite; }
    .comic-rain { animation: comicRain 1.5s ease-in infinite; }
    .comic-rain-delay { animation: comicRain 1.5s ease-in 0.5s infinite; }
    .comic-sparkle { animation: comicSparkle 2s ease-in-out infinite; transform-origin: center; }
    .comic-sparkle-delay { animation: comicSparkle 2s ease-in-out 0.7s infinite; transform-origin: center; }
    .comic-wave { animation: comicWave 1.5s ease-in-out infinite; transform-origin: left center; }
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
    /* Staggered children in grids */
    .comic-panel.scroll-animate {
        transform: translateY(80px) scale(0.92) rotate(-2deg);
        filter: blur(10px);
        transition: opacity 1s cubic-bezier(0.16, 1, 0.3, 1),
                    transform 1s cubic-bezier(0.16, 1, 0.3, 1),
                    filter 0.8s cubic-bezier(0.16, 1, 0.3, 1);
    }
    .comic-panel.scroll-animate.visible {
        transform: translateY(0) scale(1) rotate(0deg);
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
    .trust-proof.scroll-animate,
    .capabilities-summary.scroll-animate {
        transform: translateY(50px) scale(0.98);
        filter: blur(6px);
        transition: opacity 1s cubic-bezier(0.16, 1, 0.3, 1),
                    transform 1s cubic-bezier(0.16, 1, 0.3, 1),
                    filter 0.8s ease;
    }
    .trust-proof.scroll-animate.visible,
    .capabilities-summary.scroll-animate.visible {
        transform: translateY(0) scale(1);
        filter: blur(0);
    }
    @media (max-width: 768px) {
        .comic-grid {
            grid-template-columns: repeat(2, 1fr);
            gap: 1rem;
        }
        .comic-icon {
            max-width: 140px;
            height: 160px;
        }
        .comic-text {
            font-size: 1.05rem;
        }
    }
    @media (max-width: 480px) {
        .comic-grid {
            grid-template-columns: repeat(2, 1fr);
            gap: 0.75rem;
        }
        .comic-panel {
            padding: 1.25rem 0.75rem;
        }
        .comic-icon {
            max-width: 110px;
            height: 130px;
        }
        .comic-text {
            font-size: 0.95rem;
        }
    }

    .capabilities-summary {
        padding: 3rem 2rem;
        padding-top: 15rem;
        max-width: 900px;
        margin: 0 auto;
        position: relative;
        z-index: 2;
    }
    .capabilities-content {
        background: rgba(126, 178, 255, 0.03);
        backdrop-filter: blur(8px);
        border: 1px solid rgba(255, 255, 255, 0.15);
        border-radius: 16px;
        padding: 2rem;
        box-shadow: 0 0 25px rgba(126, 178, 255, 0.08);
    }
    .capabilities-tagline {
        font-size: 1.3rem;
        color: #fff;
        text-align: center;
        margin-bottom: 2rem;
        line-height: 1.6;
        font-weight: 500;
    }
    .capabilities-grid {
        display: grid;
        grid-template-columns: repeat(2, 1fr);
        gap: 1.5rem;
        margin-bottom: 1.5rem;
    }
    .capability-category {
        text-align: center;
        padding: 1rem;
    }
    .capability-category h3 {
        font-size: 0.85rem;
        color: #7EB2FF;
        text-transform: uppercase;
        letter-spacing: 0.1em;
        margin-bottom: 0.5rem;
        font-weight: 600;
    }
    .capability-category p {
        font-size: 1rem;
        color: #ddd;
        margin: 0;
        line-height: 1.5;
    }
    .availability-info {
        text-align: center;
        padding-top: 1rem;
        border-top: 1px solid rgba(255, 255, 255, 0.1);
    }
    .availability-info p {
        font-size: 0.95rem;
        color: #bbb;
        margin: 0.5rem 0;
    }
    .availability-info strong {
        color: #7EB2FF;
    }
    @media (max-width: 768px) {
        .capabilities-summary {
            padding: 2rem 1rem;
        }
        .capabilities-tagline {
            font-size: 1.1rem;
        }
        .capabilities-grid {
            grid-template-columns: 1fr;
            gap: 1rem;
        }
        .capability-category {
            padding: 0.75rem;
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
