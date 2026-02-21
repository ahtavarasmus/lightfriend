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

    let feature_css = r#"
        .feature-list {
            padding: 4rem 2rem;
            max-width: 800px;
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
        .feature-list ul {
            list-style: none;
            padding: 0;
        }
        .feature-list li {
            font-size: 1.2rem;
            color: #ddd;
            margin-bottom: 1rem;
            display: flex;
            align-items: center;
            gap: 1rem;
        }
        .feature-list i {
            color: #7EB2FF;
            font-size: 1.5rem;
        }
        .feature-desc iframe {
            width: 100%;
            aspect-ratio: 16/9;
            margin-top: 1rem;
            border: none;
        }
        .feature-preview-img {
            display: block;
            max-width: 100%;
            width: 400px;
            margin: 1rem auto;
            border-radius: 12px;
            border: 1px solid rgba(30, 144, 255, 0.2);
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
            right: calc(50% - 200px + 8px);
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
        @media (max-width: 480px) {
            .feature-preview-img {
                width: 100%;
            }
            .feature-preview-badge {
                right: 8px;
            }
        }
        @media (max-width: 768px) {
            .feature-list {
                padding: 2rem 1rem;
            }
            .feature-list h2 {
                font-size: 2rem;
            }
            .feature-list li {
                font-size: 1.1rem;
            }
        }
    "#;
    html! {
        <div class="landing-page">
            <head>
                <link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.5.2/css/all.min.css" integrity="sha512-SnH5WK+bZxgPHs44uWIX+LLJAJ9/2PkPKZ5QiAj6Ta86w+fsb2TkcmfRyVX3pBnMFcV7oQPJkl9QevSCWr3W6A==" crossorigin="anonymous" referrerpolicy="no-referrer" />
            </head>
            <header class="hero">
                <div class="hero-background"></div>
                <div class="hero-overlay" style={format!("opacity: {};", *dim_opacity)}></div>
                <div class="hero-content">
                    <div class="hero-header">
                        <h1 class="hero-title">{"Break Free of Smartphones"}</h1>
                        <p class="hero-subtitle">
                            {"An Assistant so useful you can live with a flip phone"}
                        </p>
                        <div class="hero-cta-group">
                            <Link<Route> to={Route::Pricing} classes="forward-link">
                                <button class="hero-cta">{"See Plans"}</button>
                            </Link<Route>>
                        </div>
                        <div class="hero-metric">
                            <span class="hero-metric-number">{days_smartphone_free.clone()}</span>
                            <span class="hero-metric-label">{"smartphone-free days powered"}</span>
                        </div>
                    </div>
                </div>
            </header>

            <section class="trust-signal">
                <span class="trust-label">{"As seen on"}</span>
                <a href="https://www.thelightphone.com/blog/lightos-tips" target="_blank" rel="noopener noreferrer" class="trust-link">
                    <img src="/assets/lightphone-logo.svg" alt="The Light Phone" class="trust-logo" />
                </a>
            </section>

            <section class="story-section">
                <img src="/assets/rasmus-story.png" alt="Rasmus story" loading="lazy" />
            </section>

            <section class="capabilities-summary">
                <div class="capabilities-content">
                    <p class="capabilities-tagline">{"Lightfriend is an AI assistant that gives dumbphone users access to WhatsApp, email, calendar, web search, and more via SMS and voice calls."}</p>
                    <div class="capabilities-grid">
                        <div class="capability-category">
                            <h3>{"Integrations"}</h3>
                            <p>{"WhatsApp, Telegram, Signal, Email, Google Calendar"}</p>
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

            <div class="difference-section">
                <div class="difference-content">
                    <div class="difference-text">
                        <h2>{"It's got your back."}</h2>
                        <p>{"No need to reach out, unless you want to. Lightfriend will let you know when it's important!"}</p>
                    </div>
                    <div class="difference-image">
                        <img src="/assets/critical-noti-example.png" alt="Lightfriend proactive notification" loading="lazy" />
                    </div>
                </div>
            </div>
            <div class="filter-concept">
                <div class="filter-content">
                    <AnimationComponent />
                </div>
            </div>
            <section class="features-section">
                <div class="feature-list">
                    <style>{feature_css}</style>
                    <h2>{"Current Capabilities"}</h2>
                    <ul>
                        <li>
                            <details>
                            <summary><i class="fas fa-phone"></i>{"Voice calling interface"}</summary>
                                <div class="feature-desc">
                                    <p>{"Access all of Lightfriend's features through natural voice calls. Simply dial and have a conversation with your AI assistant. No smartphone or internet connection needed - works with any basic phone that can make calls."}</p>
                                    <video class="feature-video" src="/assets/lightfriend-demo.mp4" controls=true autoplay=false loop=false muted=false></video>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fa-solid fa-comment-sms"></i>{"SMS chat interface"}</summary>
                                <div class="feature-desc">
                                    <p>{"Use all of Lightfriend's capabilities through simple text messages. Your optional conversation context is remembered between SMS and voice calls, allowing for seamless continuity across both interfaces. Conversation history can be saved from zero up to 10 back and forths. Works with any basic phone that can send texts."}</p>
                                    // <iframe class="feature-video" src="https://www.youtube.com/embed/VIDEO_ID" allowfullscreen=true></iframe>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-search"></i>{"Perplexity AI Web Search"}</summary>
                                <div class="feature-desc">
                                    <p>{"Query anything you'd search on Google - from local restaurant reviews to stock prices, store hours to landmark info - via voice call or SMS. Powered by Perplexity AI, it provides accurate, real-time answers with sources, just like ChatGPT but with up-to-date information. Example: Text or say 'What's the latest news on AI advancements?' or 'Is the coffee shop on Main Street open now?' to get instant, reliable answers."}</p>
                                    // <iframe class="feature-video" src="https://www.youtube.com/embed/VIDEO_ID" allowfullscreen=true></iframe>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-cloud-sun"></i>{"Weather Search and forecast up to 7 days"}</summary>
                                <div class="feature-desc">
                                    <p>{"Request weather information for any location via SMS or voice. Receive current conditions, temperature, a detailed 6-hour forecast, or up to 7 days ahead. Example: 'Weather in London' or 'What's the weather like this weekend in Paris?' returns instant updates."}</p>
                                    // <iframe class="feature-video" src="https://www.youtube.com/embed/VIDEO_ID" allowfullscreen=true></iframe>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-route"></i>{"Get Directions"}</summary>
                                <div class="feature-desc">
                                    <p>{"Get detailed turn-by-turn walking directions between any two locations via SMS or voice call. Example: 'How do I get walking from Central Park South & 5th Avenue, New York to Rockefeller Center, 45 Rockefeller Plaza, New York.' Note: You'll need to specify your starting location including city/area as we can't detect it automatically. On longer trips, just ask lightfriend for more information at any point during the trip. This tool uses Google Maps behind the scenes."}</p>
                                    // <iframe class="feature-video" src="https://www.youtube.com/embed/VIDEO_ID" allowfullscreen=true></iframe>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-image"></i>{"Photo Analysis & Translation (US, CA & AUS only)"}</summary>
                                <div class="feature-desc">
                                    <p>{"Send a photo via MMS to Lightfriend; the AI analyzes the image content (e.g., describes objects or scenes) or translates any visible text. Limited to US, Canada and Australia due to carrier MMS support. Example: Send a picture of a menu for translation."}</p>
                                    // <iframe class="feature-video" src="https://www.youtube.com/embed/VIDEO_ID" allowfullscreen=true></iframe>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-qrcode"></i>{"QR Code Scanning (US, CA & AUS only)"}</summary>
                                <div class="feature-desc">
                                    <p>{"Take a photo of a QR code and send it via MMS; Lightfriend decodes it and sends back the embedded information, such as links or text. Available only in US, Canada and Australia. Example: Scan a product QR for details on the go."}</p>
                                    // <iframe class="feature-video" src="https://www.youtube.com/embed/VIDEO_ID" allowfullscreen=true></iframe>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fab fa-whatsapp"></i>{"Send, Fetch and Monitor WhatsApp Messages"}</summary>
                                <div class="feature-desc">
                                    <p>{"Link your WhatsApp account in the web dashboard. Then, send messages (e.g., 'Send whatsapp message to Alice saying 'Hi!'), fetch recent messages ('Check whatsapp') or from specific chat ('see if Luukas has sent me anything on whatsapp') and monitor for new messages with automatic SMS or call notifications for important updates."}</p>
                                    <div class="feature-preview-wrapper">
                                        <span class="feature-preview-badge">{"Preview"}</span>
                                        <img class="feature-preview-img" src="/assets/previews/whatsapp-preview.gif" alt="WhatsApp integration preview" loading="lazy"/>
                                    </div>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fab fa-telegram"></i>{"Send, Fetch and Monitor Telegram Messages"}</summary>
                                <div class="feature-desc">
                                    <p>{"Link your Telegram account in the web dashboard. Then, send messages (e.g., 'send telegram to Bob saying I'm outside right now'), fetch recent messages ('fetch telegram pls') or from specific chat ('Check telegram for mom') and monitor for new messages with automatic SMS or call notifications for important updates."}</p>
                                    // <iframe class="feature-video" src="https://www.youtube.com/embed/VIDEO_ID" allowfullscreen=true></iframe>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fab fa-signal-messenger"></i>{"Send, Fetch and Monitor Signal Messages"}</summary>
                                <div class="feature-desc">
                                    <p>{"Link your Signal account in the web dashboard. Then, send messages (e.g., 'Send message on signal to Bob saying '5 min'), fetch recent messages ('Check signal') or from specific chat ('see if signal messages from Greg') and monitor for new messages with automatic SMS or call notifications for important updates."}</p>
                                    // <iframe class="feature-video" src="https://www.youtube.com/embed/VIDEO_ID" allowfullscreen=true></iframe>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-envelope"></i>{"Fetch, Send, Reply and Monitor Emails"}</summary>
                                <div class="feature-desc">
                                    <p>{"Integrate your email (Gmail, Outlook, etc.) in the settings. Fetch recent emails (e.g., 'Check email'), find specific email ('Can you find the Delta Airlines reservation number from email?') and monitor for important ones with AI-filtered notifications sent to your phone via SMS or make it call you."}</p>
                                    <div class="feature-preview-wrapper">
                                        <span class="feature-preview-badge">{"Preview"}</span>
                                        <img class="feature-preview-img" src="/assets/previews/email-preview.gif" alt="Email integration preview" loading="lazy"/>
                                    </div>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-calendar-days"></i>{"Fetch, Create and Monitor Calendar events"}</summary>
                                <div class="feature-desc">
                                    <p>{"Sync with Google Calendar. View events (e.g., 'What's on my calendar today?'), create new ones ('Create new calendar event for Doctor at 10am tomorrow'), Set reminder on the event on either straight with lightfriend or in the calendar and get reminded via SMS or call."}</p>
                                    <div class="feature-preview-wrapper">
                                        <span class="feature-preview-badge">{"Preview"}</span>
                                        <img class="feature-preview-img" src="/assets/previews/calendar-preview.gif" alt="Calendar integration preview" loading="lazy"/>
                                    </div>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-car"></i>{"Tesla Vehicle Control"}</summary>
                                <div class="feature-desc">
                                    <p>{"Connect your Tesla and control it via voice call, SMS, or the Lightfriend web dashboard. Lock and unlock doors, turn on climate control, start defrost mode, and check battery status. Perfect for cold mornings - just text 'warm up my Tesla' before heading out. On the dashboard, sensitive actions like unlocking or starting the car require entering your passkey each time for security."}</p>
                                    <div class="feature-preview-wrapper">
                                        <span class="feature-preview-badge">{"Preview"}</span>
                                        <img class="feature-preview-img" src="/assets/previews/tesla-controls-preview.gif" alt="Tesla controls preview" loading="lazy"/>
                                    </div>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-eye"></i>{"24/7 Critical Message Monitoring"}</summary>
                                <div class="feature-desc">
                                    <p>{"AI constantly scans your connected apps (WhatsApp, Telegram, email) for critical or urgent messages. If detected as critical (cannot wait 2 more hours), you'll receive an immediate notification via SMS or call."}</p>
                                    // <iframe class="feature-video" src="https://www.youtube.com/embed/VIDEO_ID" allowfullscreen=true></iframe>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-newspaper"></i>{"Morning, Day and Evening Digests"}</summary>
                                <div class="feature-desc">
                                    <p>{"Get automated, AI-summarized digests of your messages, emails, calendar events sent via SMS at set times: morning overview, midday update, and evening recap to keep you informed without constant checking."}</p>
                                    // <iframe class="feature-video" src="https://www.youtube.com/embed/VIDEO_ID" allowfullscreen=true></iframe>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-clock"></i>{"Temporary Monitoring for Specific Content"}</summary>
                                <div class="feature-desc">
                                    <p>{"Set up short-term monitoring for specific content in your apps (e.g., 'Monitor email for package update'). Notifications are sent via SMS/call and once found the temporary monitoring task is removed."}</p>
                                    // <iframe class="feature-video" src="https://www.youtube.com/embed/VIDEO_ID" allowfullscreen=true></iframe>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-bell"></i>{"Priority Sender Notifications"}</summary>
                                <div class="feature-desc">
                                    <p>{"Designate priority contacts in the dashboard. Any messages from them across integrations trigger instant notifications to your phone via SMS or voice call, ensuring you never miss important communications."}</p>
                                    // <iframe class="feature-video" src="https://www.youtube.com/embed/VIDEO_ID" allowfullscreen=true></iframe>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-rocket"></i>{"All Future Features Included"}</summary>
                                <div class="feature-desc">
                                    <p>{"As a subscriber, you'll automatically receive access to all upcoming features and updates, such as new app integrations, enhanced AI capabilities, or additional tools, without any price increase. While subscription prices will go up for new users as more features are added, early subscribers like you will keep their original lower price permanently."}</p>
                                    // <iframe class="feature-video" src="https://www.youtube.com/embed/VIDEO_ID" allowfullscreen=true></iframe>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-headset"></i>{"Priority Support"}</summary>
                                <div class="feature-desc">
                                    <p>{"Enjoy dedicated, fast-response support from the developer. Reach out via email (rasmus@ahtava.com) for help with setup, troubleshooting, or feature requests."}</p>
                                    // <iframe class="feature-video" src="https://www.youtube.com/embed/VIDEO_ID" allowfullscreen=true></iframe>
                                </div>
                            </details>
                        </li>
                    </ul>
                </div>
            </section>
            <section class="trust-proof">
                <div class="section-intro">
                    <h2>{"Why this exists"}</h2>
                    <img src="/assets/rasmus-pfp.png" alt="Rasmus" loading="lazy" style="max-width: 200px; border-radius: 50%; margin: 0 auto 1.5rem; display: block;"/>
                    <p class="why-lead">{"This isn't a compromise. It's what made things possible."}</p>
                    <p>{"I'm Rasmus. I've lived without a smartphone for over three years. During that time, I finished a computer science degree, won an international skiing medal, built three startups, and work as a software engineer."}</p>
                    <p>{"Lightfriend is my fourth startup - and the first one with paying customers. Not because it's the cleverest idea, but because it's the one I actually needed."}</p>
                    <p>{"I didn't do those things despite using a dumbphone. I did them because of it."}</p>
                </div>
            </section>
            <section class="testimonials-section">
                <div class="testimonials-content">
                    <h2>{"What Users Are Saying"}</h2>
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
                </div>
            </section>
            <div class="filter-concept">
                <div class="filter-content">
                    <div class="faq-in-filter">
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
                    </div>
                </div>
            </div>
            <footer class="footer-cta">
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
        text-shadow: 0 0 20px rgba(30, 144, 255, 0.2);
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
        border: 1px solid rgba(126, 178, 255, 0.1);
        border-radius: 12px;
        padding: 1.5rem;
        box-shadow: 0 0 15px rgba(126, 178, 255, 0.05);
        transition: all 0.3s ease;
    }
    .faq-item:hover {
        border-color: rgba(126, 178, 255, 0.25);
        box-shadow: 0 0 25px rgba(126, 178, 255, 0.1);
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
        border-bottom: 1px solid rgba(126, 178, 255, 0.2);
    }
    .comparison-table th {
        background: rgba(0, 0, 0, 0.5);
        color: #7EB2FF;
    }
    .comparison-table tr:hover {
        background: rgba(126, 178, 255, 0.1);
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
        text-shadow: 0 0 8px rgba(30, 144, 255, 0.3);
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
        box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3), 0 0 25px rgba(126, 178, 255, 0.12);
        border: 1px solid rgba(126, 178, 255, 0.1);
        transition: all 0.3s ease;
    }
    .difference-image img:hover {
        box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3), 0 0 35px rgba(126, 178, 255, 0.2);
        border-color: rgba(126, 178, 255, 0.25);
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
        box-shadow: 0 4px 20px rgba(30, 144, 255, 0.3);
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
        .hero {
            padding: 2rem 1rem;
            padding-top: 100px;
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
        text-shadow: 0 0 20px rgba(30, 144, 255, 0.2);
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
            rgba(30, 144, 255, 0.3),
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
        border: 2px solid rgba(30, 144, 255, 0.3);
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
        border-top: 1px solid rgba(30, 144, 255, 0.1);
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
        height: 100vh;
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
        height: 100%;
        display: flex;
        flex-direction: column;
        pointer-events: auto;
        padding: 0 2rem;
    }
    .hero-header {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        flex: 1;
        text-align: center;
        padding-top: 28vh;
    }
    .hero-background {
        position: fixed;
        top: 0;
        left: 0;
        width: 100%;
        height: 100vh;
        background-image: url('/assets/rain.gif');
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
    @media (max-width: 700px) {
        .hero-background {
            background-position: 70% center;
        }
    }
    .hero-title {
        font-size: 3rem;
        font-weight: 700;
        background: linear-gradient(45deg, #fff, #F5F0E1);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
        text-shadow: 0 0 20px rgba(30, 144, 255, 0.2);
        margin: 0 auto 1rem;
        max-width: 600px;
    }
    .hero-subtitle {
        position: relative;
        font-size: 1.3rem;
        font-weight: 300;
        letter-spacing: 0.02em;
        max-width: 600px;
        margin: 0 auto 1rem;
        line-height: 1.8;
        font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
        text-align: center;
        background: linear-gradient(45deg, #fff, #7EB2FF);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
        text-shadow: none;
    }
    .highlight-icon {
        font-size: 1.2rem;
        margin: 0 0.2rem;
        background: linear-gradient(45deg, #7EB2FF, #4169E1);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
        text-shadow: 0 0 8px rgba(30, 144, 255, 0.3);
        vertical-align: middle;
    }
    @media (max-width: 768px) {
        .hero-content {
            padding: 0 1rem;
        }
        .hero-title {
            font-size: 2rem;
        }
        .hero-subtitle {
            font-size: 1.1rem;
            line-height: 1.6;
        }
        .highlight-icon {
            font-size: 1rem;
        }
    }
    .hero-cta {
        background: linear-gradient(
            45deg,
            #7EB2FF,
            #4169E1
        );
        color: white;
        border: none;
        padding: 1rem 2.5rem;
        border-radius: 8px;
        font-size: 1.1rem;
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
        border: 1px solid rgba(255, 255, 255, 0.2);
        backdrop-filter: blur(5px);
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
        box-shadow: 0 4px 20px rgba(126, 178, 255, 0.4), 0 0 40px rgba(126, 178, 255, 0.3);
        background: linear-gradient(
            45deg,
            #90c2ff,
            #5479f1
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
        border: 1px solid rgba(126, 178, 255, 0.15);
        border-radius: 30px;
        margin-top: 1rem;
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
        text-shadow: 0 0 8px rgba(30, 144, 255, 0.3);
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
        text-shadow: 0 0 20px rgba(30, 144, 255, 0.2);
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
        border-top: 1px solid rgba(30, 144, 255, 0.1);
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
        border: 1px solid rgba(30, 144, 255, 0.3);
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
        box-shadow: 0 0 10px rgba(30, 144, 255, 0.2);
    }
    .waitlist-input::placeholder {
        color: #666;
    }
    .waitlist-button {
        padding: 0.75rem 1.5rem;
        background: linear-gradient(45deg, #1E90FF, #4169E1);
        border: none;
        border-radius: 8px;
        color: white;
        font-size: 1rem;
        cursor: pointer;
        transition: all 0.3s ease;
    }
    .waitlist-button:hover:not(:disabled) {
        transform: translateY(-2px);
        box-shadow: 0 4px 15px rgba(30, 144, 255, 0.3);
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
        text-shadow: 0 0 8px rgba(30, 144, 255, 0.3);
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
        padding: 1.5rem;
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
        background: rgba(126, 178, 255, 0.1);
        backdrop-filter: blur(10px);
        border: 1px solid rgba(126, 178, 255, 0.2);
        box-shadow: 0 0 20px rgba(126, 178, 255, 0.15), inset 0 0 20px rgba(126, 178, 255, 0.05);
        transition: all 0.3s ease;
    }
    .trust-link:hover {
        background: rgba(126, 178, 255, 0.15);
        box-shadow: 0 0 30px rgba(126, 178, 255, 0.25), inset 0 0 20px rgba(126, 178, 255, 0.1);
        border-color: rgba(126, 178, 255, 0.4);
    }
    .trust-logo {
        height: 22px;
        width: auto;
        filter: brightness(0) invert(1);
        opacity: 0.9;
    }
    .story-section {
        padding: 4rem 2rem;
        max-width: 1200px;
        margin: 0 auto;
        text-align: center;
        position: relative;
        z-index: 2;
    }
    .story-section img {
        max-width: 100%;
        height: auto;
        border-radius: 12px;
        box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
    }
    .capabilities-summary {
        padding: 3rem 2rem;
        max-width: 900px;
        margin: 0 auto;
        position: relative;
        z-index: 2;
    }
    .capabilities-content {
        background: rgba(126, 178, 255, 0.03);
        backdrop-filter: blur(8px);
        border: 1px solid rgba(126, 178, 255, 0.15);
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
        border-top: 1px solid rgba(126, 178, 255, 0.1);
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
    .story-grid {
        display: grid;
        grid-template-columns: repeat(2, 1fr);
        gap: 2rem;
    }
    .story-item {
        background: transparent;
        border: none;
        border-radius: 24px;
        padding: 1.5rem;
        display: flex;
        flex-direction: column;
        align-items: center;
        transition: transform 0.3s ease, box-shadow 0.3s ease;
    }
    .story-item:hover {
        transform: translateY(-5px);
    }
    .story-item img {
        max-width: 100%;
        height: auto;
        border-radius: 12px;
        margin-bottom: 1rem;
    }
    .story-item p {
        color: #ddd;
        font-size: 1.4rem;
        font-weight: 500;
        margin: 0;
        text-shadow: 0 1px 2px rgba(0, 0, 0, 0.5);
    }
    .story-text {
        color: #ddd;
        font-size: 1.2rem;
        line-height: 1.6;
        margin: 1rem 0;
    }
    .story-text a.learn-more {
        color: #7EB2FF;
        text-decoration: none;
        font-weight: 600;
        transition: color 0.3s ease;
    }
    .story-text a.learn-more:hover {
        color: #90c2ff;
        text-shadow: 0 0 8px rgba(30, 144, 255, 0.3);
    }
    @media (max-width: 768px) {
        .story-section {
            padding: 2rem 1rem;
        }
        .story-grid {
            grid-template-columns: 1fr;
        }
        .story-item p {
            font-size: 1.2rem;
        }
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
        text-shadow: 0 0 20px rgba(30, 144, 255, 0.2);
    }
    .testimonial {
        background: rgba(126, 178, 255, 0.05);
        backdrop-filter: blur(10px);
        border-radius: 12px;
        padding: 2rem;
        margin: 1rem 0;
        border: 1px solid rgba(126, 178, 255, 0.15);
        box-shadow: 0 0 20px rgba(126, 178, 255, 0.1);
        transition: all 0.3s ease;
    }
    .testimonial:hover {
        border-color: rgba(126, 178, 255, 0.3);
        box-shadow: 0 0 30px rgba(126, 178, 255, 0.15);
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
                "#}
            </style>
        </div>
    }
}
