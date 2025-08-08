use yew::prelude::*;
use crate::Route;
use yew_router::components::Link;
use crate::components::notification::AnimationComponent;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;

#[function_component(Landing)]
pub fn landing() -> Html {
    let dim_opacity = use_state(|| 0.0);

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
                        .add_event_listener_with_callback("scroll", callback.as_ref().unchecked_ref())
                        .unwrap();

                    // Initial call
                    if let Ok(scroll_y) = window.scroll_y() {
                        let factor = (scroll_y / 500.0).min(1.0);
                        dim_opacity.set(factor * 0.6);
                    }

                    Box::new(move || {
                        if let Some(win) = web_sys::window() {
                            win.remove_event_listener_with_callback("scroll", callback.as_ref().unchecked_ref()).unwrap();
                        }
                    })
                } else {
                    Box::new(|| ())
                };
                move || { destructor(); }
            },
            (),
        );
    }

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
                        <h1 class="hero-title">{"Break Your Phone Addiction Without Willpower"}</h1>
                        <p class="hero-subtitle">
                            {"Switch to a dumbphone without feeling isolated, even if your life depends on "}
                            <span class="highlight-icon"><i class="fab fa-whatsapp"></i></span>
                            <span class="highlight-icon"><i class="fab fa-telegram"></i></span>
                            <span class="highlight-icon"><i class="fas fa-envelope"></i></span>
                            <span class="highlight-icon"><i class="fas fa-calendar"></i></span>
                            <span class="highlight-icon"><i class="fas fa-globe"></i></span>
                            {"."}
                        </p>
                    </div>
                    <div class="hero-cta-group">
                        <Link<Route> to={Route::Pricing} classes="forward-link">
                            <button class="hero-cta">{"Get Started"}</button>
                        </Link<Route>>
                    </div>
                </div>
            </header>
            <div class="difference-section">
                <div class="difference-content">
                    <div class="difference-text">
                        <h2>{"Your attention is the product."}</h2>
                        <p>{"Every feed is optimized to hook you."}</p>
                        <p>{"Every ping is a trap."}</p>
                        <p>{"You’re up against casinos built by behavioral psychologists."}</p>
                        <p>{"Lightfriend helps you "}<span class="highlight">{"opt out."}</span>{" You stay connected, but free."}</p>
                    </div>
                    <div class="difference-image">
                        <img src="/assets/lightfriend-filter.png" alt="Lightfriend being a filter between you and the apps." loading="lazy" />
                    </div>
                </div>
            </div>
            <section class="story-section">
                <div class="story-grid">
                    <div class="story-item">
                        <img src="/assets/lightfriend-robot-scene-3.png" alt="Get notified" loading="lazy" />
                        <p>{"Leave your smartphone behind. If it matters, Lightfriend will call you."}</p>
                    </div>
                    <div class="story-item">
                        <img src="/assets/lightfriend-robot-scene-4.png" alt="Ask whenever whatever" loading="lazy" />
                        <p>{"Enjoy the freedom knowing everything is just a call or text away."}</p>
                    </div>
                </div>
            </section>
            <div class="filter-concept">
                <div class="filter-content">
                    <AnimationComponent />
                </div>
            </div>
            <div class="difference-section">
                <div class="difference-content">
                    <div class="difference-text">
                        <h2>{"Willpower is not the solution."}</h2>
                        <p>{"Your mind burns energy just knowing you could scroll. "}<span class="highlight">{"Make it impossible"}</span>{"."}</p>
                    </div>
                    <div class="difference-image">
                        <img src="/assets/delete-blocker.png" alt="Man thinking about checking IG with delete blocker prompt" loading="lazy" />
                    </div>
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
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fa-solid fa-comment-sms"></i>{"SMS chat interface"}</summary>
                                <div class="feature-desc">
                                    <p>{"Use all of Lightfriend's capabilities through simple text messages. Your optional conversation context is remembered between SMS and voice calls, allowing for seamless continuity across both interfaces. Conversation history can be saved from zero up to 10 back and forths. Works with any basic phone that can send texts."}</p>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-search"></i>{"Perplexity AI Web Search"}</summary>
                                <div class="feature-desc">
                                    <p>{"Query anything you'd search on Google - from local restaurant reviews to stock prices, store hours to landmark info - via voice call or SMS. Powered by Perplexity AI, it provides accurate, real-time answers with sources, just like ChatGPT but with up-to-date information. Example: Text or say 'What's the latest news on AI advancements?' or 'Is the coffee shop on Main Street open now?' to get instant, reliable answers."}</p>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-cloud-sun"></i>{"Weather Search and forecast of the next 6 hours"}</summary>
                                <div class="feature-desc">
                                    <p>{"Request weather information for any location via SMS or voice. Receive current conditions, temperature, and a detailed 6-hour forecast. Example: 'Weather in London' returns instant updates."}</p>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-image"></i>{"Photo Analysis & Translation (US & AUS only)"}</summary>
                                <div class="feature-desc">
                                    <p>{"Send a photo via MMS to Lightfriend; the AI analyzes the image content (e.g., describes objects or scenes) or translates any visible text. Limited to US and Australia due to carrier MMS support. Example: Send a picture of a menu for translation."}</p>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-qrcode"></i>{"QR Code Scanning (US & AUS only)"}</summary>
                                <div class="feature-desc">
                                    <p>{"Take a photo of a QR code and send it via MMS; Lightfriend decodes it and sends back the embedded information, such as links or text. Available only in US and Australia. Example: Scan a product QR for details on the go."}</p>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fab fa-whatsapp"></i>{"Send, Fetch and Monitor WhatsApp Messages"}</summary>
                                <div class="feature-desc">
                                    <p>{"Link your WhatsApp account in the web dashboard. Then, send messages (e.g., 'Send whatsapp message to Alice saying 'Hi!'), fetch recent messages ('Check whatsapp') or from specific chat ('see if Luukas has sent me anything on whatsapp') and monitor for new messages with automatic SMS or call notifications for important updates."}</p>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fab fa-telegram"></i>{"Send, Fetch and Monitor Telegram Messages"}</summary>
                                <div class="feature-desc">
                                    <p>{"Link your Telegram account in the web dashboard. Then, send messages (e.g., 'send telegram to Bob saying I'm outside right now'), fetch recent messages ('fetch telegram pls') or from specific chat ('Check telegram for mom') and monitor for new messages with automatic SMS or call notifications for important updates."}</p>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-envelope"></i>{"Fetch and Monitor Emails"}</summary>
                                <div class="feature-desc">
                                    <p>{"Integrate your email (Gmail, Outlook, etc.) in the settings. Fetch recent emails (e.g., 'Check email'), find specific email ('Can you find the Delta Airlines reservation number from email?') and monitor for important ones with AI-filtered notifications sent to your phone via SMS or make it call you."}</p>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-calendar-days"></i>{"Fetch, Create and Monitor Calendar events"}</summary>
                                <div class="feature-desc">
                                    <p>{"Sync with Google Calendar. View events (e.g., 'What's on my calendar today?'), create new ones ('Create new calendar event for Doctor at 10am tomorrow'), Set reminder on the event on either straight with lightfriend or in the calendar and get reminded via SMS or call."}</p>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-list-check"></i>{"Fetch and Create Tasks and Ideas"}</summary>
                                <div class="feature-desc">
                                    <p>{"Manage a personal task list or idea notebook. Create entries (e.g. call lightfriend and ask, 'Hey save this brilliant billion dollar idea i got'), fetch them ('List my saved ideas'), and organize via voice or SMS. Stored in google tasks and accessible anytime. Will not affect your existing google tasks."}</p>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-eye"></i>{"24/7 Critical Message Monitoring"}</summary>
                                <div class="feature-desc">
                                    <p>{"AI constantly scans your connected apps (WhatsApp, Telegram, email) for critical or urgent messages. If detected as critical (cannot wait 2 more hours), you'll receive an immediate notification via SMS or call."}</p>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-newspaper"></i>{"Morning, Day and Evening Digests"}</summary>
                                <div class="feature-desc">
                                    <p>{"Get automated, AI-summarized digests of your messages, emails, calendar events sent via SMS at set times: morning overview, midday update, and evening recap to keep you informed without constant checking."}</p>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-clock"></i>{"Temporary Monitoring for Specific Content"}</summary>
                                <div class="feature-desc">
                                    <p>{"Set up short-term monitoring for specific content in your apps (e.g., 'Monitor email for package update'). Notifications are sent via SMS/call and once found the temporary monitoring task is removed."}</p>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-bell"></i>{"Priority Sender Notifications"}</summary>
                                <div class="feature-desc">
                                    <p>{"Designate priority contacts in the dashboard. Any messages from them across integrations trigger instant notifications to your phone via SMS or voice call, ensuring you never miss important communications."}</p>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-rocket"></i>{"All Future Features Included"}</summary>
                                <div class="feature-desc">
                                    <p>{"As a subscriber, you'll automatically receive access to all upcoming features and updates, such as new app integrations, enhanced AI capabilities, or additional tools, without any price increase. While subscription prices will go up for new users as more features are added, early subscribers like you will keep their original lower price permanently."}</p>
                                </div>
                            </details>
                        </li>
                        <li>
                            <details>
                                <summary><i class="fas fa-headset"></i>{"Priority Support"}</summary>
                                <div class="feature-desc">
                                    <p>{"Enjoy dedicated, fast-response support from the developer. Reach out via email (rasmus@ahtava.com) for help with setup, troubleshooting, or feature requests."}</p>
                                </div>
                            </details>
                        </li>
                    </ul>
                </div>
            </section>
            <section class="trust-proof">
                <div class="section-intro">
                    <h2>{"Why I'm Building Lightfriend"}</h2>
                    <p>{"Hi, I'm Rasmus, a solo developer behind this project and honestly, I have very low willpower. I work in bursts of inspiration, but that’s not always enough when things have deadlines. Smartphones were stealing my time and focus. I knew I needed to engineer my environment to work for me, not against me. Tried blockers, detox apps, everything. But I always found a way around them."}</p>
                    <p>{"Before all this, I was a full-time athlete who had just started studying Computer Science. My first semester was brutal. I had to be sharp in every short study session I had between training. But scrolling wrecked my focus and stole what little time I had."}</p>
                    <p>{"That’s when I switched to a dumbphone. Everything changed. I could finally focus. I wasn’t always behind anymore. I stopped saying no to friends because I actually got my school work done. I had time and energy again, and the freedom to say yes to things I actually wanted to do."}</p>
                    <p>{"Now I’m juggling a CS master's, high-level sports, part-time work, and building Lightfriend every day. And I never feel rushed. I can direct my attention where I want it."}</p>
                    <p>{"I've been using the Light Phone for 3 years, starting with the Light Phone 2 and upgrading to the Light Phone 3 this summer. It's beautifully designedand I love using it. It has maps and hotspot, but that's about it. I needed to access WhatsApp messages while on the go. I needed email. I needed internet search. The issue is that dumbphones can't have these features directly - if a phone has an app store to download WhatsApp, then you can download any app from it, which defeats the whole purpose of avoiding distractions."}</p>
                    <p>{"So I built Lightfriend as my own assistant. Something I could call or text from a dumbphone to check WhatsApp messages, send replies, search the web, get calendar updates, and handle email. The magic is that I can access what I need without having the infinite scroll right there in my pocket."}</p>
                    <p>{"I posted the first version on Reddit. It only had voice-activated AI search. The number one request was WhatsApp integration. Then email. Then calendar. Then QR code reader. I realized I could help other other people too."}</p>
                    <p>{"I use Lightfriend daily and rely on it to stay updated. I wouldn't go back to a smartphone, not even close. When you make scrolling physically impossible, you can finally relax. You don't have to fight the addiction anymore. It's such an insane feeling when you experience it. The phone had been draining my brain like an anti-virus software slowing down a computer. Books started to feel entertaining again. I want others to experience it too."}</p>
                    <p>{"I recently switched from usage-based pricing with over 100 users to a subscription model. The project is open-source, and 55 developers have starred it on GitHub. I'm trying to make it better every day and I'm always open to feedback. You can reach me at rasmus@ahtava.com."}</p>
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
                            <p>{"Lightfriend is hosted on a secure EU server with a strict zero-logging policy - nothing is stored beyond what's necessary. As a bootstrapped solo dev, I prioritize transparency: the code is open source for anyone to audit. The hosted setup requires some trust (zero access isn't feasible), but your data remains yours and can be deleted anytime."}</p>
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
                    <a href="/faq#try-service" class="faq-link">
                        {"Try demo first"}
                    </a>
                    <p class="disclaimer">{"Works with smartphones and basic phones. Customize to your needs."}</p>
                    <div class="development-links">
                        <p>{"Source code on "}
                            <a href="https://github.com/ahtavarasmus/lightfriend" target="_blank" rel="noopener noreferrer">{"GitHub"}</a>
                        </p>
                        <p>{"Follow us at "}
                            <a href="https://pacepeek.com/ahtavarasmus" target="_blank" rel="noopener noreferrer">{"pacepeek.com/ahtavarasmus"}</a>
                            {" and "}
                            <a href="https://x.com/rasmuscodes" target="_blank" rel="noopener noreferrer">{"x.com/rasmuscodes"}</a>
                        </p>
                        <div class="legal-links">
                            <Link<Route> to={Route::Terms}>{"Terms & Conditions"}</Link<Route>>
                            {" | "}
                            <Link<Route> to={Route::Privacy}>{"Privacy Policy"}</Link<Route>>
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
        gap: 1rem;
        list-style: none;
        position: relative;
        padding-right: 1.5rem; /* Space for arrow */
    }
    summary::-webkit-details-marker {
        display: none;
    }
    summary::after {
        content: '▶'; /* Right-pointing arrow (closed state) */
        position: absolute;
        right: 0;
        top: 50%;
        transform: translateY(-50%);
        transition: transform 0.3s ease;
        font-size: 0.8rem;
        color: #7EB2FF;
    }
    details[open] summary::after {
        content: '▼'; /* Down-pointing arrow (open state) */
        transform: translateY(-50%) rotate(90deg); /* Optional rotation for visual feedback */
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
        background: transparent;
        border: none;
        border-radius: 12px;
        padding: 1.5rem;
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
        box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
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
        justify-content: space-around;
        pointer-events: auto;
    }
    .hero-header {
        display: flex;
        flex-direction: column;
        justify-content: flex-end;
    }
    .hero-background {
        position: fixed;
        top: 0;
        left: 0;
        width: 100%;
        height: 100vh;
        background-image: url('/assets/boy_holding_dumbphone_in_crowded_place.webp');
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
        margin: 0 auto 3rem;
        line-height: 1.8;
        font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
        text-align: left;
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
            flex-direction: column;
            justify-content: flex-end;
        }
        .hero-title {
            font-size: 2rem;
        }
        .hero-subtitle {
            font-size: 1.1rem;
            line-height: 1.6;
            margin-bottom: 2rem;
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
        box-shadow: 0 4px 20px rgba(126, 178, 255, 0.4);
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
        gap: 1rem;
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
    .story-section {
        padding: 4rem 2rem;
        max-width: 1200px;
        margin: 0 auto;
        text-align: center;
        position: relative;
        z-index: 2;
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
                "#}
            </style>
        </div>
    }
}
