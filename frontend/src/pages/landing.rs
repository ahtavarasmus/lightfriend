
use yew::prelude::*;
use crate::auth::connect::Connect;
use wasm_bindgen::prelude::*;
use web_sys::{Element, HtmlElement, HtmlElement as HtmlElementTrait};
use crate::pages::proactive::Proactive;
use yew_router::prelude::*;
use crate::Route;
use yew_router::components::Link;
use crate::config;
use web_sys::{window, HtmlInputElement};
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use serde_json::json;
use wasm_bindgen_futures::spawn_local;


#[function_component(Landing)]
pub fn landing() -> Html {
    use_effect(|| {
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let window_clone = window.clone();
        
let scroll_callback = Closure::wrap(Box::new(move || {
                // Handle intro section visibility and image transitions
                if let Some(intro_section) = document.query_selector(".intro-section").ok().flatten() {
                    let intro_html = intro_section.clone().dyn_into::<web_sys::HtmlElement>().unwrap();
                    let scroll_pos = window_clone.scroll_y().unwrap();
                    let window_height = window_clone.inner_height().unwrap().as_f64().unwrap();
                    
                    let sticky_scroll = scroll_pos - (window_height * 0.5);  // Increased to 0.5 to delay appearance
                    let sticky_duration = window_height * 4.0;  // Keep this the same
                    
                    // Calculate intro section opacity based on scroll position
                    if sticky_scroll > sticky_duration * 0.75 {
                        let fade_progress = ((sticky_scroll - (sticky_duration * 0.75)) / (sticky_duration * 0.25)).min(1.0);
                        let intro_opacity = (1.0 - fade_progress).max(0.0);
                        let _ = intro_html.set_attribute("style", &format!(
                            "opacity: {}; position: fixed; top: 0; left: 0; width: 100%; z-index: 2;", 
                            intro_opacity
                        ));
                    } else {
                        let _ = intro_html.set_attribute("style", "opacity: 1; position: fixed; top: 0; left: 0; width: 100%; z-index: 2;");
                    }
                    
                    // Show intro section when scrolled past hero
                    let current_classes = intro_section.class_name();
                    let base_classes = "intro-section";
                    
                    if scroll_pos > window_height * 0.4 {  // Decreased to 0.4 to start transition earlier
                        if !current_classes.contains("visible") {

                            intro_section.set_class_name(&format!("{} visible", base_classes));
                        }
                        
                        // Calculate relative scroll position within the sticky section
                        let sticky_scroll = scroll_pos - (window_height * 0.4);  // Increased to match the above change
                        let sticky_duration = window_height * 2.0; // Reduced from 4.0 to 2.0
                        
                        // Handle image transitions based on sticky scroll position
                        if let Some(whatsapp_image) = document.query_selector(".whatsapp-image").ok().flatten() {
                            if let Some(email_image) = document.query_selector(".email-image").ok().flatten() {
                                if let Some(calendar_image) = document.query_selector(".calendar-image").ok().flatten() {
                                    // Reset all images first
                                    whatsapp_image.set_class_name("whatsapp-image example-image");
                                    email_image.set_class_name("email-image example-image");
                                    calendar_image.set_class_name("calendar-image example-image");

                                    if sticky_scroll < sticky_duration * 0.25 {
                                        // First quarter: show WhatsApp image
                                        whatsapp_image.set_class_name("whatsapp-image example-image visible");
                                        let _ = intro_html.set_attribute("style", "opacity: 1");
                                    } else if sticky_scroll < sticky_duration * 0.5 {
                                        // Second quarter: show email image
                                        email_image.set_class_name("email-image example-image visible");
                                        let _ = intro_html.set_attribute("style", "opacity: 1");
                                    } else if sticky_scroll < sticky_duration * 0.75 {
                                        // Third quarter: show calendar image
                                        calendar_image.set_class_name("calendar-image example-image visible");
                                        let _ = intro_html.set_attribute("style", "opacity: 1");
                                    } else {
                                        // Final quarter: fade out intro section
                                        calendar_image.set_class_name("calendar-image example-image visible");
                                        let _ = intro_html.set_attribute("style", "opacity: 0");
                                    }
                                }
                            }
                        }

                        // Add sticky class when in the sticky range
                        if sticky_scroll < sticky_duration {
                            if !current_classes.contains("sticky") {
                                intro_section.set_class_name(&format!("{} visible sticky", base_classes));
                            }
                        } else {
                            // Remove sticky class after duration
                            if current_classes.contains("sticky") {
                                intro_section.set_class_name(&format!("{} visible", base_classes));
                            }
                        }
                    } else {
                        // Reset to base classes when not visible
                        intro_section.set_class_name(base_classes);
                        let _ = intro_html.set_attribute("style", "opacity: 0");
                    }
                }

        }) as Box<dyn FnMut()>);

        window.add_event_listener_with_callback(
            "scroll",
            scroll_callback.as_ref().unchecked_ref(),
        ).unwrap();

        // Initial check
        scroll_callback.as_ref().unchecked_ref::<web_sys::js_sys::Function>().call0(&JsValue::NULL).unwrap();

        let scroll_callback = scroll_callback;  // Move ownership to closure
        move || {
            window.remove_event_listener_with_callback(
                "scroll",
                scroll_callback.as_ref().unchecked_ref(),
            ).unwrap();
        }
    });

    let is_privacy_expanded = use_state(|| false);
    let onclick = {
        let is_privacy_expanded = is_privacy_expanded.clone();
        Callback::from(move |_| {
            is_privacy_expanded.set(!*is_privacy_expanded);
        })
    };


    html! {

        <div class="landing-page">
        <header class="hero">
                <div class="hero-background"></div>
                <div class="hero-content">
                    <div class="hero-title">{"Break Free Without Vanishing"}</div>

                </div>
        </header>        



            <section class="intro-section">
            <div class="intro-content">
                        <div class="sticky-image">
                            <img src="/assets/whatsappexample.png" alt="WhatsApp example interface" class="example-image whatsapp-image" />
                            <img src="/assets/emailexample.png" alt="Email example interface" class="example-image email-image" />
                            <img src="/assets/calendarexample.png" alt="Calendar example interface" class="example-image calendar-image" />
                        </div>
                    </div>
            </section>


        <section class="main-features">
            <div class="section-header">
                <h2>{"Freedom, Not Isolation"}</h2>
                    <div class="section-intro">
                        <Link<Route> to={Route::Register} classes="forward-link">
                            <button class="hero-cta">{"Start Your Dumbphone Journey"}</button>
                        </Link<Route>>
                    </div>
            </div>
            <div class="feature-block on-demand">
                <div class="feature-content">
                    <h2>{"Everything just a call or text away"}</h2>
                    <p>{"Need your calendar? A WhatsApp reply? Just call or text LightFriend."}</p>
                    <ul class="feature-list">
                        <li><img src="/assets/whatsapplogo.png" alt="WhatsApp" class="feature-logo" /> {"WhatsApp"}</li>
                        <li>{"📧 Emails"}</li>
                        <li>{"📅 Calendar"}</li>
                        <li><img src="/assets/perplexitylogo.png" alt="Perplexity" class="perplexity-logo" /> {"Perplexity AI search"}</li>
                        <li>{"☀️ Weather, Tasks and even Shazam"}</li>
                    </ul>
                    <div class="demo-link-container">
                        <a href="https://www.youtube.com/shorts/KrVdJbHPB-o" target="_blank" rel="noopener noreferrer" class="demo-link">
                            {"▶️ See It in Action"}
                        </a>
                    </div>
                </div>
                <div class="feature-image">
                    <img src="/assets/train.png" alt="Showcase of dumbphone screen showing user getting traing tickets" />
                </div>
            </div>

            <div class="feature-block proactive">
                <div class="feature-content">
                    <h2>{"Only What Matters"}</h2>
                    <p>{"LightFriend filters the noise, pinging you only for important stuff, like that urgent email or a friend’s text from WhatsApp."}</p>
                    <ul class="feature-list">
                        <li>{"Smart alerts for key messages"}</li>
                        <li>{"Custom filters for your priorities"}</li>
                        <li>{"No spam, no distractions"}</li>
                    </ul>
                </div>
                <div class="feature-image">
                    <img src="/assets/notifications.png" alt="Person receiving a meaningful notification" />
                </div>
            </div>

            <div class="feature-block privacy">
                <div class="feature-content">
                    <h2>{"Your Data, Your Rules"}</h2>
                    <p>{"We’re not Big Tech. LightFriend’s open-source code and privacy-first design keep your info safe."}</p>
                    <ul class="feature-list">
                        <li>{"🔒 No call recordings, ever"}</li>
                        <li>{"🤖 Sensitive info auto-redacted"}</li>
                        <li>{"📱 Secure SMS storage with Twilio"}</li>
                        <li>{"🗑️ Data deleted when you’re done"}</li>
                        <li>{"💻 Fully open-source—check it yourself"}</li>
                    </ul>
                    <div class="privacy-example">
                        {
                            html! {
                                <>
                                <button class="privacy-toggle" {onclick}>
                                    <h3>{"How We Protect You"}</h3>
                                    <span class="toggle-icon">{if *is_privacy_expanded {"▼"} else {"▶"}}</span>
                                </button>
                                {
                                    if *is_privacy_expanded {
                                    html! {
                                        <div class="privacy-content">
                                            <p>{"We keep your data minimal and secure:"}</p>
                                            <ul class="privacy-details">
                                                <li><strong>{"Calls:"}</strong> {"No recordings. Just anonymous metrics to improve service."}</li>
                                                <li><strong>{"Messages:"}</strong> {"Sensitive info redacted, stored securely with Twilio, fetched only when needed."}</li>
                                            </ul>
                                            <p class="context-example">{"Example redaction:"}</p>
                                            <pre class="redaction-example">
                                                {"Original: \"Check if John Smith sent the $5000 invoice\"\nStored: \"Check if [NAME_REDACTED] sent the [CONTENT_REDACTED]\""}
                                            </pre>
                                        </div>
                                    }
                                    } else {
                                        html! {}
                                    }
                                }
                                </>
                            }
                        }
                    </div>
                </div>
                <div class="feature-image-chield">
                    <img src="/assets/privacy_shield.png" alt="Privacy-focused illustration" />
                </div>
            </div>
        </section>

        /* i don't these fit rn
        <section class="testimonials">
            <h2>{"Real People, Real Freedom"}</h2>
            <div class="testimonials-grid">
                <div class="testimonial-card">
                    <div class="testimonial-content">
                        <p>{"“LightFriend got me off my phone addiction. I’m not kidding—I feel alive again. And yeah, days are LONG when you’re not scrolling 4 hours!”"}</p>
                    </div>
                    <div class="testimonial-author">
                        <span class="author-name">{"Sarah K."}</span>
                        <span class="author-title">{"Student"}</span>
                    </div>
                </div>
                <div class="testimonial-card">
                    <div class="testimonial-content">
                        <p>{"“I was tethered to WhatsApp and social media. LightFriend let me ditch my smartphone for good. Such a relief.”"}</p>
                    </div>
                    <div class="testimonial-author">
                        <span class="author-name">{"Michael R."}</span>
                        <span class="author-title">{"Software Developer"}</span>
                    </div>
                </div>
                <div class="testimonial-card">
                    <div class="testimonial-content">
                        <p>{"“ADHD here. Forgot a delivery code in my email, but LightFriend saved me with one text. It’s a game-changer.”"}</p>
                    </div>
                    <div class="testimonial-author">
                        <span class="author-name">{"Long Time Dumbphone User"}</span>
                        <span class="author-title">{"Artist"}</span>
                    </div>
                </div>
                <div class="testimonial-card">
                    <div class="testimonial-content">
                        <p>{"“My old Nokia + LightFriend = perfection. I call to hear my emails while driving. It’s so chill.”"}</p>
                    </div>
                    <div class="testimonial-author">
                        <span class="author-name">{"David W."}</span>
                        <span class="author-title">{"Designer"}</span>
                    </div>
                </div>
                <div class="testimonial-card">
                    <div class="testimonial-content">
                        <p>{"“Tried every minimalist phone. LightFriend’s the only one that nails it—WhatsApp, email, no distractions. Worth every penny.”"}</p>
                    </div>
                    <div class="testimonial-author">
                        <span class="author-name">{"Patrick C."}</span>
                        <span class="author-title">{"Entrepreneur"}</span>
                    </div>
                </div>
            </div>
        </section>
        */

        <section class="how-it-works">
            <h2>{"Escape the Scroll in 3 Steps"}</h2>
            <p>{"LightFriend makes going dumbphone stupidly easy."}</p>
            <div class="steps-grid">
                <div class="step">
                    <h3>{"Link Your Stuff"}</h3>
                    <p>{"Connect your email, calendar, and messages via our secure web portal."}</p>
                </div>
                <div class="step">
                    <h3>{"Use Any Basic Phone"}</h3>
                    <p>{"Text or call LightFriend to get what you need, anywhere, anytime."}</p>
                </div>
                <div class="step">
                    <h3>{"Live Free"}</h3>
                    <p>{"Stay connected without the apps that suck you in. Just you, living."}</p>
                </div>
            </div>
        </section>

        <footer class="footer-cta">
            <div class="footer-content">
                <h2>{"Ready to Take Your Life Back?"}</h2>
                <p class="subtitle">{"LightFriend lets you quit the scroll without disappearing. Join the dumbphone revolution."}</p>
                <Link<Route> to={Route::Register} classes="forward-link">
                    <button class="hero-cta">{"Start Living Today"}</button>
                </Link<Route>>
                <p class="disclaimer">{"Works with any basic phone. No smartphone needed."}</p>
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
.intro-section {
    padding: 6rem 2rem;
    background: transparent;
    min-height: 100vh;
    width: 100%;
    opacity: 0;
    visibility: hidden;
    transition: opacity 0.5s cubic-bezier(0.4, 0, 0.2, 1);
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    will-change: opacity, transform;
    height: 100vh;
    z-index: 1;
    overflow-y: scroll;
    transform: translateZ(0); /*force gpu acc*/
    -webkit-overflow-scrolling: touch;
    -webkit-backface-visibility: hidden;
    scrollbar-width: none; /* Firefox */
    backface-visibility: hidden;
    pointer-events: none;
}

.intro-section.visible {
    pointer-events: auto;
    z-index: 1;
}

.main-features {
    position: relative;
    z-index: 2;
}

.footer-cta {
    position: relative;
    z-index: 2;
}

.hero {
    position: relative;
    z-index: 2;
}

.hero-content {
    position: relative;
    z-index: 2;
    pointer-events: auto;
}

.hero-background {
    z-index: 0;
}

.intro-section.visible {
    pointer-events: auto;
}

.intro-section::-webkit-scrollbar {
    display: none; /* Safari and Chrome */
}

@media (max-width: 768px) {
    .intro-section {
        padding: 0;
        display: flex;
        align-items: center;
        justify-content: center;
    }

    .intro-section.visible {
        background: rgba(26, 26, 26, 0.4);
        backdrop-filter: blur(5px);
    }
}

.intro-section.sticky {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    z-index: 2;
}

.intro-section.visible {
    opacity: 1;
    visibility: visible;
    transform: translateY(0);
    pointer-events: auto;
    z-index: 3;
    transition: opacity 0.8s ease;
}

.intro-section.visible {
    opacity: 1;
    visibility: visible;
    transform: translateY(0);
    pointer-events: auto;
    z-index: 3;
}

.intro-section.sticky {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
}

/* Add space after sticky section to prevent content jump */
/* Remove this rule as we're handling spacing with margin-bottom */
.intro-content {
    max-width: 1200px;
    margin: 0 auto;
    display: flex;
    align-items: center;
    gap: 4rem;
    position: relative;
    padding: 0 2rem;
}

@media (max-width: 1024px) {
    .intro-content {
        flex-direction: column;
        text-align: center;
        gap: 2rem;
        padding-top: 2rem;
    }

    .intro-text {
        text-align: center;
        align-items: center;
        margin-top: 0 !important;
    }

    .intro-text .hero-cta {
        margin: 1.5rem auto;
    }

    .hero-image {
        max-width: 400px;
        position: relative;
        top: 0;
    }
}

.example-image {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
    border-radius: 12px;
    opacity: 0;
    transition: opacity 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    pointer-events: none;
    transform: translateZ(0);
    -webkit-transform: translateZ(0);
    backface-visibility: hidden;
    -webkit-backface-visibility: hidden;
    max-width: 400px;
    will-change: opacity;
    z-index: 5;
}

@media (max-width: 768px) {
    .intro-content {
        flex-direction: column;
        text-align: center;
        gap: 2rem;
        padding-top: 0;
    }

    .sticky-image {
        position: fixed !important;
        top: 50% !important;
        left: 50% !important;
        transform: translate(-50%, -50%) !important;
        width: 280px !important;
        height: 280px !important;
        transform: translate3d(-50%, -50%, 0) !important; /* Force GPU acceleration */
        -webkit-transform: translate3d(-50%, -50%, 0) !important;
        backface-visibility: hidden;
        -webkit-backface-visibility: hidden;
        margin: 0 !important;
        z-index: 10;
    }

    .example-image {
        position: absolute;
        max-width: 280px;
        height: auto;
        left: 50%;
        top: 50%;
    }

    .intro-text {
        margin-top: 100vh !important;
        padding: 2rem;
        background: rgba(26, 26, 26, 0.95);
        border-radius: 16px;
        border: 1px solid rgba(30, 144, 255, 0.1);
        backdrop-filter: blur(10px);
        position: relative;
        z-index: 20;
    }
}


@media (max-width: 768px) {
    .example-image {
        position: absolute;
        max-width: 280px;
        height: auto;
    }
}

.example-image.visible {
    opacity: 1;
    z-index: 6;
}

.example-image.visible {
    opacity: 1;
    z-index: 2;
}

.sticky-image {
    position: sticky;
    top: 20vh;
    width: 400px;
    height: 600px;
    flex-shrink: 0;
    margin-left: auto; /* Push to right side */
    z-index: 5;
}
@media (max-width: 768px) {
    .sticky-image {
        position: fixed !important;
        top: 50% !important;
        left: 50% !important;
        transform: translate(-50%, -50%) !important;
        width: 480px !important;
        height: 480px !important;
        margin: 0 !important;
        z-index: 10;
    }
}

.whatsapp-image, .email-image, .calendar-image {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
}

@media (max-width: 768px) {
                        .intro-section {
                            padding: 1rem;
                        }

                        .intro-content {
                            flex-direction: column;
                            text-align: center;
                            padding: 1rem;
                            gap: 2rem;
                        }

                        .sticky-image {
                            width: 100%;
                            height: 400px;
                            margin: 2rem auto;
                        }

                        .example-image {
                            width: 100%;
                            max-width: 100%;
                            margin: 0 auto;
                        }

                        .intro-text {
                            text-align: center;
                            align-items: center;
                        }
                    }

                    .hero-image {
                        flex: 1;
                        max-width: 500px;
                        width: 100%;
                        position: sticky;
                        top: 100px;
                        height: fit-content;
                        animation: float-gentle 6s ease-in-out infinite;
                    }

                    .quote-section {
                        padding: 4rem 2rem;
                        text-align: center;
                        background: #1a1a1a;
                        position: relative;
                    }

                    .quote-text {
                        font-size: 1.5rem;
                        color: rgba(255, 255, 255, 0.9);

                        max-width: 800px;
                        margin: 0 auto;
                        line-height: 1.6;

                        position: relative;
                    }



                    @media (max-width: 768px) {
                        .quote-section {
                            padding: 3rem 1rem;
                        }

                        .quote-text {
                            font-size: 1.2rem;
                        }

                        .quote-text::before,
                        .quote-text::after {
                            font-size: 2rem;
                        }

                        .quote-text::before {
                            left: -1rem;
                            top: -0.5rem;
                        }

                        .quote-text::after {
                            right: -1rem;
                            bottom: -1.5rem;
                        }
                    }


.intro-text {
    flex: 1;
    text-align: left;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    position: relative;
    z-index: 2;
    padding-right: 2rem;
    margin-top: 50vh; /* Added this line to move the text lower */
}

.intro-text h2 {
    font-size: 2.5rem;
    background: linear-gradient(45deg, #fff, #7EB2FF);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    margin-bottom: 1.5rem;
}

.intro-text p {
    color: #999;
    font-size: 1.2rem;
    line-height: 1.6;
}

                    @media (max-width: 968px) {
                        .intro-content {
                            flex-direction: column;
                            text-align: center;
                            gap: 2rem;
                            padding-top: 2rem;
                        }

                        .intro-text {
                            text-align: center;
                            align-items: center;
                        }

                        .intro-text .hero-cta {
                            margin: 1.5rem auto;
                        }

                        .hero-image {
                            max-width: 400px;
                            position: relative;
                            top: 0;
                        }
                    }

@media (max-width: 768px) {
    .intro-section {
        padding: 4rem 1rem;
    }

    .intro-text h2 {
        font-size: 2rem;
    }

    .intro-text p {
        font-size: 1.1rem;
    }

    .hero-image {
        max-width: 300px;
    }
}
                    .testimonials {
                        padding: 6rem 2rem;
                        text-align: center;
                        background: linear-gradient(
                            to bottom,
                            rgba(30, 144, 255, 0.05),
                            transparent
                        );
                    }

                    .testimonials h2 {
                        font-size: 3rem;
                        margin-bottom: 3rem;
                        background: linear-gradient(45deg, #fff, #7EB2FF);
                        -webkit-background-clip: text;
                        -webkit-text-fill-color: transparent;
                    }

                    .testimonials-grid {
                        display: grid;
                        grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
                        gap: 2rem;
                        max-width: 1200px;
                        margin: 0 auto;
                    }

.testimonial-card {
    background: rgba(30, 30, 30, 0.7);
    border: 1px solid rgba(30, 144, 255, 0.1);
    border-radius: 16px;
    padding: 2rem;
    text-align: left;
    transition: transform 1.5s cubic-bezier(0.4, 0, 0.2, 1),
                border-color 1.5s ease,
                box-shadow 1.5s ease;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    position: relative;
    overflow: hidden;
}


.testimonial-card:hover {
    transform: translateY(-5px) scale(1.02);
    box-shadow: 0 4px 20px rgba(30, 144, 255, 0.15);
    border-color: rgba(30, 144, 255, 0.3);
}

                    .testimonial-content {
                        margin-bottom: 1.5rem;
                    }

                    .testimonial-content p {
                        color: #e0e0e0;
                        font-size: 1.1rem;
                        line-height: 1.6;
                        font-style: italic;
                    }

                    .testimonial-author {
                        display: flex;
                        flex-direction: column;
                        gap: 0.25rem;
                    }

                    .author-name {
                        color: #7EB2FF;
                        font-weight: 600;
                    }

                    .author-title {
                        color: #999;
                        font-size: 0.9rem;
                    }

                    @media (max-width: 768px) {
                        .testimonials {
                            padding: 4rem 1rem;
                        }

                        .testimonials h2 {
                            font-size: 2rem;
                            margin-bottom: 2rem;
                        }

                        .testimonial-card {
                            padding: 1.5rem;
                        }

                        .testimonial-content p {
                            font-size: 1rem;
                        }
                    }

                    .featured-sections {
                        padding: 4rem 0;
                        text-align: center;
                    }

                    .featured-grid {
                        display: grid;
                        grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
                        gap: 2rem;
                        max-width: 1200px;
                        margin: 0 auto;
                        padding: 0 2rem;
                    }

                    .featured-item {
                        background: rgba(30, 30, 30, 0.7);
                        border: 1px solid rgba(30, 144, 255, 0.1);
                        border-radius: 16px;
                        padding: 2rem;
                        text-align: center;
                    }

                    .featured-item h2 {
                        font-size: 1.8rem;
                        margin-bottom: 1.5rem;
                        color: #7EB2FF;
                    }

                    .feature-block:hover {
                        transform: translateY(-5px);
                        box-shadow: 0 4px 20px rgba(30, 144, 255, 0.15);
                        border-color: rgba(30, 144, 255, 0.3);
                    }

                    .privacy-example {
                        margin-top: 2rem;
                        padding: 1.5rem;
                        background: rgba(0, 0, 0, 0.2);
                        border-radius: 12px;
                        border: 1px solid rgba(30, 144, 255, 0.1);
                    }

                    .privacy-toggle {
                        width: 100%;
                        background: none;
                        border: none;
                        padding: 0;
                        cursor: pointer;
                        display: flex;
                        justify-content: space-between;
                        align-items: center;
                        color: inherit;
                        transition: all 0.3s ease;
                    }

                    .privacy-toggle:hover {
                        color: #7EB2FF;
                    }

                    .privacy-toggle h3 {
                        margin: 0;
                        text-align: left;
                    }

                    .toggle-icon {
                        font-size: 0.8rem;
                        color: #7EB2FF;
                        transition: transform 0.3s ease;
                    }

                    .privacy-content {
                        margin-top: 1.5rem;
                        animation: slideDown 0.3s ease;
                    }

                    @keyframes slideDown {
                        from {
                            opacity: 0;
                            transform: translateY(-10px);
                        }
                        to {
                            opacity: 1;
                            transform: translateY(0);
                        }
                    }

                    .privacy-example h3 {
                        color: #7EB2FF;
                        font-size: 1.2rem;
                        margin-bottom: 1rem;
                    }

                    .redaction-example {
                        background: rgba(0, 0, 0, 0.3);
                        padding: 1rem;
                        border-radius: 8px;
                        font-family: monospace;
                        font-size: 0.9rem;
                        color: #999;
                        white-space: pre-wrap;
                        overflow-x: auto;
                    }

                    .privacy-details {
                        list-style: none;
                        padding: 0;
                        margin: 1.5rem 0;
                    }

                    .privacy-details li {
                        margin-bottom: 1rem;
                        color: #e0e0e0;
                        line-height: 1.6;
                    }

                    .privacy-details strong {
                        color: #7EB2FF;
                        display: block;
                        margin-bottom: 0.3rem;
                    }

                    .context-example {
                        color: #7EB2FF;
                        margin: 1.5rem 0 0.5rem 0;
                        font-size: 0.9rem;
                    }



                    .feature-block.privacy:hover {
                        border-color: rgba(30, 144, 255, 0.4);
                    }

                    @media (max-width: 768px) {
                        .privacy-example {
                            padding: 1rem;
                        }

                        .redaction-example {
                            font-size: 0.8rem;
                            padding: 0.75rem;
                        }
                    }

                    .solopush-content {
                        display: flex;
                        flex-direction: column;
                        align-items: center;
                        gap: 1.5rem;
                    }

                    .solopush-logo {
                        max-width: 200px;
                        height: auto;
                    }

                    .solopush-content p {
                        color: #fff;
                        font-size: 1.2rem;
                    }

                    .solopush-link {
                        display: inline-block;
                        padding: 0.8rem 1.5rem;
                        background: linear-gradient(45deg, #1E90FF, #4169E1);
                        color: white;
                        text-decoration: none;
                        border-radius: 8px;
                        font-size: 1rem;
                        transition: all 0.3s ease;
                    }

                    .solopush-link:hover {
                        transform: translateY(-2px);
                        box-shadow: 0 4px 20px rgba(30, 144, 255, 0.3);
                    }

                    .producthunt-demo {
                        padding: 0;
                        text-align: center;
                    }

                    .producthunt-iframe-container {
                        margin: 2rem auto;
                        max-width: 500px;
                        width: 100%;
                        display: flex;
                        justify-content: center;
                    }

                    @media (max-width: 520px) {
                        .producthunt-iframe-container iframe {
                            width: 100%;
                            height: auto;
                            min-height: 405px;
                        }
                    }

                    .problems {
                        padding: 6rem 2rem;
                        text-align: center;
                        background: linear-gradient(to bottom, #2d2d2d, #1a1a1a);
                    }

                    .problems h2 {
                        font-size: 3rem;
                        margin-bottom: 2rem;
                        color: #7EB2FF;
                    }

                    .challenges-grid {
                        display: grid;
                        grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
                        gap: 2rem;
                        margin-top: 4rem;
                        padding: 2rem;
                    }

                    .challenge-item {
                        padding: 2rem;
                        border: 1px solid rgba(30, 144, 255, 0.2);
                        border-radius: 12px;
                        background: linear-gradient(to bottom, rgba(30, 144, 255, 0.05), rgba(30, 144, 255, 0.02));
                        transition: all 0.3s ease;
                    }

                    .challenge-item:hover {
                        transform: translateY(-5px);
                        box-shadow: 0 4px 20px rgba(30, 144, 255, 0.15);
                        border-color: rgba(30, 144, 255, 0.4);
                    }

                    .challenge-item h3 {
                        color: #1E90FF;
                        font-size: 1.5rem;
                        margin-bottom: 1rem;
                    }

                    .challenge-item p {
                        color: #999;
                        font-size: 1rem;
                        line-height: 1.6;
                    }

                    .transformation {
                        padding: 6rem 2rem;
                        text-align: center;
                        background: linear-gradient(to bottom, #1a1a1a, #2d2d2d);
                    }

                    .transformation h2 {
                        font-size: 3rem;
                        margin-bottom: 1rem;
                        color: #7EB2FF;
                    }

                    .benefits-grid {
                        display: grid;
                        grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
                        gap: 2rem;
                        margin-top: 4rem;
                        padding: 2rem;
                    }

                    .benefit-item {
                        padding: 2rem;
                        border: 1px solid rgba(30, 144, 255, 0.2);
                        border-radius: 12px;
                        background: linear-gradient(to bottom, rgba(30, 144, 255, 0.05), transparent);
                        transition: all 0.3s ease;
                    }

                    .benefit-item:hover {
                        transform: translateY(-5px);
                        box-shadow: 0 4px 20px rgba(30, 144, 255, 0.15);
                        border-color: rgba(30, 144, 255, 0.4);
                    }

                    .benefit-item h3 {
                        color: #1E90FF;
                        font-size: 1.5rem;
                        margin-bottom: 1rem;
                    }

                    .video-demo {
                        margin-top: 2rem;
                        padding: 1.5rem;
                        background: rgba(30, 144, 255, 0.05);
                        border-radius: 12px;
                        border: 1px solid rgba(30, 144, 255, 0.2);
                    }

                    .video-demo p {
                        color: #7EB2FF;
                        margin-bottom: 1rem;
                        font-size: 1.1rem;
                    }

                    .demo-link {
                        display: inline-flex;
                        align-items: center;
                        gap: 0.5rem;
                        padding: 0.8rem 1.5rem;
                        background: linear-gradient(45deg, #1E90FF, #4169E1);
                        color: white;
                        text-decoration: none;
                        border-radius: 8px;
                        font-size: 1rem;
                        transition: all 0.3s ease;
                        border: none;
                        cursor: pointer;
                    }

                    .demo-link:hover {
                        transform: translateY(-2px);
                        box-shadow: 0 4px 20px rgba(30, 144, 255, 0.3);
                    }

                    /* Add responsive styles for the video demo */
                    @media (max-width: 768px) {
                        .video-demo {
                            padding: 1rem;
                            margin-top: 1.5rem;
                        }

                        .video-demo p {
                            font-size: 1rem;
                        }

                        .demo-link {
                            padding: 0.6rem 1.2rem;
                            font-size: 0.9rem;
                        }
                    }

.how-it-works {
    padding: 6rem 2rem;
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
                        background: rgba(255, 255, 255, 0.03);
                        border-radius: 16px;
                        padding: 2.5rem;
                        border: 1px solid rgba(30, 144, 255, 0.2);
                        backdrop-filter: blur(5px);
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
    border-color: rgba(30, 144, 255, 0.4);
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

                    /* Add step numbers */
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

                    /* Shazam Showcase Section */
                    .shazam-showcase {
                        padding: 6rem 2rem;
                        text-align: center;
                        position: relative;
                        background: linear-gradient(
                            to bottom,
                            rgba(30, 144, 255, 0.05),
                            transparent
                        );
                        border-top: 1px solid rgba(30, 144, 255, 0.1);
                        border-bottom: 1px solid rgba(30, 144, 255, 0.1);
                    }

                    .shazam-showcase h2 {
                        font-size: 3rem;
                        margin-bottom: 3rem;
                        text-shadow: 0 0 20px rgba(30, 144, 255, 0.2);
                        background: linear-gradient(45deg, #fff, #7EB2FF);
                        -webkit-background-clip: text;
                        -webkit-text-fill-color: transparent;
                    }

                    .showcase-content {
                        display: flex;
                        justify-content: center;
                        align-items: center;
                        gap: 4rem;
                        max-width: 1200px;
                        margin: 0 auto;
                    }

                    .showcase-text {
                        text-align: left;
                        flex: 1;
                        max-width: 600px;
                        padding: 2rem;
                        background: rgba(30, 144, 255, 0.05);
                        border-radius: 16px;
                        border: 1px solid rgba(30, 144, 255, 0.2);
                        backdrop-filter: blur(5px);
                    }

                    .showcase-text h3 {
                        color: #1E90FF;
                        font-size: 1.5rem;
                        margin-bottom: 1.5rem;
                    }

                    .showcase-text ol {
                        list-style: none;
                        counter-reset: shazam-steps;
                        padding: 0;
                        margin: 0;
                    }

                    .showcase-text ol li {
                        counter-increment: shazam-steps;
                        padding: 1rem 0;
                        padding-left: 3rem;
                        position: relative;
                        color: #999;
                        font-size: 1.1rem;
                    }

                    .showcase-text ol li::before {
                        content: counter(shazam-steps);
                        position: absolute;
                        left: 0;
                        top: 50%;
                        transform: translateY(-50%);
                        width: 32px;
                        height: 32px;
                        background: rgba(30, 144, 255, 0.1);
                        border: 1px solid rgba(30, 144, 255, 0.3);
                        border-radius: 50%;
                        display: flex;
                        align-items: center;
                        justify-content: center;
                        color: #1E90FF;
                        font-weight: bold;
                    }

                    .showcase-highlight {
                        margin-top: 2rem;
                        padding: 1rem;
                        background: rgba(30, 144, 255, 0.1);
                        border-radius: 8px;
                        color: #7EB2FF;
                        font-size: 1.1rem;
                        text-align: center;
                    }

                    /* Responsive design for Shazam showcase */
                    @media (max-width: 768px) {
                        .shazam-showcase {
                            padding: 4rem 1rem;
                        }

                        .shazam-showcase h2 {
                            font-size: 2rem;
                            margin-bottom: 2rem;
                        }

                        .showcase-content {
                            flex-direction: column;
                            gap: 2rem;
                        }

                        .showcase-text {
                            padding: 1.5rem;
                        }

                        .showcase-text ol li {
                            font-size: 1rem;
                        }

                        .showcase-highlight {
                            font-size: 1rem;
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

                .hero {
                    text-align: center;
                    padding: 6rem 2rem;
                    margin: 0 auto;
                }

.main-features {
    max-width: 1200px;
    margin: 0 auto;
    padding: 2rem 2rem;
    position: relative;
    z-index: 3;
    background: transparent;
    opacity: 1;
    margin-bottom: 4rem;
    pointer-events: auto;
}

.feature-block {
    display: flex;
    align-items: center;
    gap: 4rem;
    margin-bottom: 6rem;
    background: rgba(30, 30, 30, 0.8);
    border: 1px solid rgba(30, 144, 255, 0.15);
    border-radius: 24px;
    padding: 3rem;
    transition: transform 1.8s cubic-bezier(0.4, 0, 0.2, 1), 
                border-color 1.8s ease, 
                box-shadow 1.8s ease;
    position: relative;
    overflow: hidden;

}

.feature-block:hover {
    transform: translateY(-5px) scale(1.02);
    box-shadow: 0 8px 32px rgba(30, 144, 255, 0.15);
    border-color: rgba(30, 144, 255, 0.3);
}

                .feature-content {
                    flex: 1;
                }

                .feature-image {
                    flex: 1;
                    display: flex;
                    justify-content: center;
                    align-items: center;
                }

                .feature-image-chield {
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
                .feature-image-chield img {
                    max-width: 100%;
                    height: auto;
                    border-radius: 12px;
                }

                .feature-block h2 {
                    font-size: 2.5rem;
                    margin-bottom: 1rem;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                }

                .feature-block p {
                    color: #999;
                    font-size: 1.1rem;
                    line-height: 1.6;
                    margin-bottom: 2rem;
                }

                .feature-list {
                    list-style: none;
                    padding: 0;
                    margin: 0 0 2rem 0;
                }

                .feature-list li {
                    color: #fff;
                    font-size: 1.1rem;
                    padding: 0.5rem 0;
                    padding-left: 1.8rem;
                    position: relative;
                }

                .feature-list li::before {
                    content: '•';
                    position: absolute;
                    left: 0.5rem;
                    color: #1E90FF;
                }

                    .perplexity-logo, .feature-logo {
                        height: 1em;
                        width: auto;
                        vertical-align: middle;
                        margin-right: 0.2em;
                    }


                .demo-link-container {
                    margin-top: 2rem;
                }

                .demo-link {
                    display: inline-flex;
                    align-items: center;
                    gap: 0.5rem;
                    padding: 0.8rem 1.5rem;
                    background: linear-gradient(45deg, #1E90FF, #4169E1);
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
                    }

                    .feature-image {
                        order: -1;
                    }
                    .feature-image-chield {
                        order: -1;
                    }


                    .feature-block h2 {
                        font-size: 2rem;
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
                        
                        .hero h1 {
                            font-size: 3.0rem !important;
                            padding: 0 1rem;
                        }

                        .hero-subtitle {
                            font-size: 1rem;
                            padding: 0 1rem;
                        }

                        .features {
                            padding: 3rem 1rem;
                        }
                        
                        .features h2 {
                            font-size: 1.75rem;
                            padding: 0 1rem;
                        }

                        .features-grid {
                            padding: 1rem;
                            margin-top: 2rem;
                        }

                        .how-it-works {
                            padding: 3rem 1rem;
                        }

                        .how-it-works h2 {
                            font-size: 1.75rem;
                        }

                        .steps-grid {
                            grid-template-columns: 1fr;
                            gap: 1.5rem;
                            padding: 0 1rem;
                        }

                        .shazam-showcase {
                            padding: 3rem 1rem;
                        }

                        .shazam-showcase h2 {
                            font-size: 1.75rem;
                        }

                        .showcase-text {
                            padding: 1.5rem;
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

                        .feature-item {
                            padding: 1.5rem;
                        }

                        .development-links {
                            padding: 0 1rem;
                        }
                    }



.footer-cta {
    padding: 6rem 0;
    background: linear-gradient(
        to bottom,
        transparent,
        rgba(30, 144, 255, 0.05)
    );
    border-top: 1px solid rgba(30, 144, 255, 0.1);
    text-align: left;
    position: relative;
    z-index: 1;
    margin-top: 0;
}

.footer-cta {
    position: relative;
    z-index: 3;
    margin-top: 0;
    background: rgba(26, 26, 26, 0.9);
    pointer-events: auto;
}

.footer-cta::before {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: linear-gradient(
        to bottom,
        rgba(26, 26, 26, 0.9),
        rgba(26, 26, 26, 0.95)
    );
    z-index: -1;
    pointer-events: none;
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
                        padding: 8rem 0;
                        position: relative;
                        background: transparent;
                        z-index: 1;
                        margin-bottom: 200vh; /* Increased to give more space for the transition */
                    }

                    .landing-page {

                        position: relative;
                        z-index: 2;
                    }

.hero-content {
    position: relative;
    z-index: 3;
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    padding: 40px;
    pointer-events: auto;
}

                    .hero-title {
                        font-size: 3.4rem;
                        line-height: 1.1;
                        color: rgba(255, 255, 255, 0.85);
                        font-weight: 200;
                        max-width: 400px;
                        font-family: 'Cormorant Garamond', serif;
                        letter-spacing: 0.02em;
                        text-align: left;
                        margin-bottom: 20px;
                        position: absolute;
                        bottom: 40px;
                        left: 10%;
                        text-shadow: 0 4px 8px rgba(0, 0, 0, 0.2);
                        font-style: italic;
                        transform: translateZ(0);
                        animation: whisperIn 1.5s ease-out forwards;
                    }

                    @keyframes whisperIn {
                        0% {
                            opacity: 0;
                            transform: translateY(20px);
                        }
                        100% {
                            opacity: 1;
                            transform: translateY(0);
                        }
                    }

                    /* Add subtle glow effect on hover */
                    .hero-title:hover {
                        color: rgba(255, 255, 255, 0.95);
                        text-shadow: 
                            0 4px 8px rgba(0, 0, 0, 0.2),
                            0 0 20px rgba(255, 255, 255, 0.1);
                        transition: all 0.5s ease;
                    }



                    @media (max-width: 768px) {
                        .hero-content {
                            padding: 20px;
                        }

                        .hero-title {
                            font-size: 2.4rem;
                            bottom: 20px;
                            left: 20px;
                            max-width: 300px;
                            bottom: 100px;
                        }

                        .hero-subtitle {
                            font-size: 1rem;
                            right: 20px;
                            max-width: 200px;
                        }
                    }
.hero-background {
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100vh;
    background-image: url('/assets/boy_holding_dumbphone_in_crowded_place.png');
    background-size: cover;
    background-position: center;
    background-repeat: no-repeat;
    opacity: 1;
    z-index: -2;
    pointer-events: none;
}

/* Add a gradient overlay only at the bottom of the hero background */
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


                    }




    @media (max-width: 700px) {
        .hero-background {
            background-position: 70% center;
        }
    }

                    .hero-image {
                        position: relative;
                        margin: 0;
                        max-width: 500px;
                        width: 100%;
                        animation: float-gentle 6s ease-in-out infinite;
                        z-index: 2;
                    }

                    .hero-image img {
                        width: 100%;
                        height: auto;
                        object-fit: contain;
                        filter: drop-shadow(0 10px 20px rgba(30, 144, 255, 0.2));
                    }

                    @keyframes float-gentle {
                        0%, 100% {
                            transform: translateY(0);
                        }
                        50% {
                            transform: translateY(-20px);
                        }
                    }

                    @media (max-width: 768px) {
                        .hero-image {
                            max-width: 300px;
                            margin: 1rem 0;
                        }
                    }

                    .lifestyle-benefits {
                        padding: 6rem 2rem;
                        background: linear-gradient(
                            to bottom,
                            transparent,
                            rgba(30, 144, 255, 0.05)
                        );
                    }

                    .benefit-block {
                        max-width: 1200px;
                        margin: 0 auto 4rem;
                        padding: 3rem;
                        background: rgba(30, 30, 30, 0.7);
                        border: 1px solid rgba(30, 144, 255, 0.1);
                        border-radius: 24px;
                        transition: all 0.3s ease;
                    }

                    .equation-grid {
                        display: grid;
                        grid-template-columns: repeat(5, 1fr);
                        gap: 1rem;
                        align-items: center;
                        margin-top: 3rem;
                        padding: 2rem;
                    }

                    .equation-item {
                        background: rgba(30, 30, 30, 0.7);
                        border: 1px solid rgba(30, 144, 255, 0.1);
                        border-radius: 16px;
                        padding: 2rem;
                        text-align: center;
                        transition: all 0.3s ease;
                    }

                    .equation-item:hover {
                        transform: translateY(-5px);
                        box-shadow: 0 8px 32px rgba(30, 144, 255, 0.15);
                        border-color: rgba(30, 144, 255, 0.3);
                    }

                    .equation-image {
                        width: 120px;
                        height: 120px;
                        object-fit: contain;
                        margin-bottom: 1.5rem;
                    }

                    .equation-symbol {
                        font-size: 3rem;
                        color: #7EB2FF;
                        text-align: center;
                        font-weight: bold;
                    }

                    .equation-item h3 {
                        color: #7EB2FF;
                        font-size: 1.5rem;
                        margin-bottom: 1.5rem;
                    }

                    .pros-cons {
                        text-align: left;
                    }

                    .pros-cons .label {
                        color: #7EB2FF;
                        font-weight: bold;
                        display: block;
                        margin-bottom: 0.5rem;
                    }

                    .pros-cons p {
                        color: #999;
                        margin: 0.5rem 0;
                        font-size: 0.9rem;
                        padding-left: 1rem;
                        position: relative;
                    }

                    .pros p::before {
                        content: '•';
                        position: absolute;
                        left: 0;
                        color: #1E90FF;
                    }

                    .cons p::before {
                        content: '•';
                        position: absolute;
                        left: 0;
                        color: #ff4444;
                    }

                    @media (max-width: 1200px) {
                        .equation-grid {
                            grid-template-columns: 1fr;
                            gap: 2rem;
                        }

                        .equation-symbol {
                            transform: rotate(90deg);
                            margin: 1rem 0;
                        }
                    }

                    .benefit-block:hover {
                        transform: translateY(-5px);
                        box-shadow: 0 8px 32px rgba(30, 144, 255, 0.15);
                        border-color: rgba(30, 144, 255, 0.3);
                    }

                    .time-calculator {
                        display: flex;
                        justify-content: space-around;
                        align-items: center;
                        margin: 3rem 0;
                        gap: 2rem;
                    }

                    .stat-block {
                        text-align: center;
                        padding: 2rem;
                        background: rgba(30, 30, 30, 0.5);
                        border: 1px solid rgba(30, 144, 255, 0.1);
                        border-radius: 16px;
                        transition: all 0.3s ease;
                    }

                    .stat-block:hover {
                        transform: translateY(-5px);
                        border-color: rgba(30, 144, 255, 0.3);
                    }

                    .stat-block.highlight {
                        background: linear-gradient(
                            45deg,
                            rgba(30, 144, 255, 0.1),
                            rgba(65, 105, 225, 0.1)
                        );
                        border-color: rgba(30, 144, 255, 0.3);
                    }

                    .stat-number {
                        display: block;
                        font-size: 3rem;
                        font-weight: 700;
                        color: #7EB2FF;
                        margin-bottom: 0.5rem;
                    }

                    .stat-label {
                        color: #999;
                        font-size: 1rem;
                    }

                    .source-note {
                        text-align: center;
                        color: #666;
                        font-size: 0.9rem;
                        margin-top: 2rem;
                    }

                    .lifestyle-grid {
                        display: grid;
                        grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
                        gap: 2rem;
                        margin-top: 3rem;
                    }

                    .lifestyle-item {
                        padding: 2rem;
                        background: rgba(30, 30, 30, 0.5);
                        border: 1px solid rgba(30, 144, 255, 0.1);
                        border-radius: 16px;
                        transition: all 0.3s ease;
                    }

                    .lifestyle-item:hover {
                        transform: translateY(-5px);
                        border-color: rgba(30, 144, 255, 0.3);
                    }

                    .lifestyle-item h3 {
                        color: #7EB2FF;
                        font-size: 1.5rem;
                        margin-bottom: 1rem;
                    }

                    .lifestyle-item p {
                        color: #999;
                        font-size: 1.1rem;
                        line-height: 1.6;
                    }

                    .section-header {
                        text-align: center;
                        margin-bottom: 4rem;
                    }

                    .section-header h2 {
                        font-size: 3rem;
                        background: linear-gradient(45deg, #fff, #7EB2FF);
                        -webkit-background-clip: text;
                        -webkit-text-fill-color: transparent;
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

                    .section-intro h3 {
                        font-size: 2rem;
                        color: #fff;
                        margin-bottom: 1rem;
                        text-shadow: 0 2px 4px rgba(0, 0, 0, 0.3);
                    }

                    .section-intro p {
                        color: #e0e0e0;
                        font-size: 1.2rem;
                        line-height: 1.6;
                        margin-bottom: 2rem;
                        text-shadow: 0 1px 2px rgba(0, 0, 0, 0.2);
                        letter-spacing: 0.01em;
                        width: 100%;
                        text-align: center;
                    }

                    .section-intro .hero-cta {
                        margin: 1rem auto;
                        display: block;
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

                        .section-intro h3 {
                            font-size: 1.5rem;
                        }

                        .section-intro p {
                            font-size: 1rem;
                        }
                    }

                    .section-subtitle {
                        color: #999;
                        font-size: 1.2rem;
                        margin: 0;
                        text-align: center;
                    }

                    @media (max-width: 768px) {
                        .section-header {
                            margin-bottom: 2rem;
                        }

                        .section-header h2 {
                            font-size: 2rem;
                        }

                        .section-subtitle {
                            font-size: 1rem;
                        }
                    }

                    @media (max-width: 768px) {
                        .time-calculator {
                            flex-direction: column;
                            gap: 1rem;
                        }

                        .stat-block {
                            width: 100%;
                            padding: 1.5rem;
                        }

                        .lifestyle-grid {
                            grid-template-columns: 1fr;
                        }

                        .benefit-block {
                            padding: 2rem;
                        }
                    }

                    .hero h1 {
                        font-size: 3.4rem;
                        line-height: 1.1;
                        margin-bottom: 1.5rem;
                        background: linear-gradient(
                            45deg,
                            #2d2d2d,
                            #1a1a1a
                        );
                        -webkit-background-clip: text;
                        -webkit-text-fill-color: transparent;
                        font-weight: 700;
                        max-width: 400px;
                        position: relative;
                        font-family: 'Inter', sans-serif;
                        letter-spacing: -0.02em;
                    }

                    @media (max-width: 768px) {
                        .hero h1 {
                            font-size: 2.4rem;
                            padding-top: 20px;
                        }
                    }

                    .producthunt-badge {
                        margin-bottom: 2rem;
                        display: flex;
                        justify-content: center;
                        align-items: center;
                    }

                    @media (max-width: 768px) {
                        .producthunt-badge {
                            margin-bottom: 1.5rem;
                        }
                        
                        .producthunt-badge img {
                            width: 200px;
                            height: auto;
                        }
                    }


                    .hero-subtitle {
                        font-size: 1.2rem;
                        color: rgba(255, 255, 255, 0.9);
                        text-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
                        max-width: 600px;
                        margin: 0 auto 3rem;
                        line-height: 1.6;
                    }

.hero-cta {
    background: linear-gradient(
        45deg,
        #1E90FF,
        #4169E1
    );
    color: white;
    border: none;
    padding: 1rem 2.5rem;
    border-radius: 8px;
    font-size: 1.1rem;
    cursor: pointer;
    transition: transform 1.5s cubic-bezier(0.4, 0, 0.2, 1),
                box-shadow 1.5s ease;
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    position: relative;
    overflow: hidden;
    margin: 2rem 0 3rem 0;
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
    box-shadow: 0 4px 20px rgba(30, 144, 255, 0.3);
}


                    .hero::before {
                        top: 10%;
                        left: 5%;
                        animation: float 20s infinite alternate;
                    }

                    .hero::after {
                        bottom: 10%;
                        right: 5%;
                        animation: float 15s infinite alternate-reverse;
                    }

                    @keyframes float {
                        0% {
                            transform: translate(0, 0);
                        }
                        100% {
                            transform: translate(20px, 20px);
                        }
                    }

                    .features {
                        padding: 6rem 0;
                        text-align: center;
                    }

                    .features h2 {
                        font-size: 3rem;
                        margin-bottom: 1rem;
                    }

                    .features > p {
                        color: #999;
                        margin-bottom: 4rem;
                        font-size: 1.2rem;
                    }

                    .features-grid {
                        display: grid;
                        grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
                        gap: 2rem;
                        text-align: center;
                        margin-top: 4rem;
                        padding: 2rem;
                        max-width: 100%;
                        overflow-x: hidden;
                    }

                    .feature-item {
                        padding: 2rem;
                        border: 1px solid rgba(30, 144, 255, 0.2); /* dodgerblue with opacity */
                        border-radius: 12px;
                        background: linear-gradient(
                            to bottom,
                            rgba(30, 144, 255, 0.05),
                            rgba(30, 144, 255, 0.02)
                        );
                        transition: all 0.3s ease;
                    }

.feature-item:hover {
    transform: translateY(-5px);
    box-shadow: 0 4px 20px rgba(30, 144, 255, 0.15);
    border-color: rgba(30, 144, 255, 0.4);
}

                    .feature-item h3 {
                        margin: 1rem 0;
                        font-size: 1.2rem;
                        color: #1E90FF; /* dodgerblue */
                    }

                    .feature-item p {
                        color: #999;
                        font-size: 0.9rem;
                        line-height: 1.5;
                    }

                    /* Add a subtle blue glow to the section title */
                    .features h2 {
                        font-size: 3rem;
                        margin-bottom: 1rem;
                        text-shadow: 0 0 20px rgba(30, 144, 255, 0.2);
                    }

                    /* Optional: Add blue accent to the subtitle */
                    .features > p {
                        color: #7EB2FF;
                        margin-bottom: 4rem;
                        font-size: 1.2rem;
                    }

    .panel-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 1.5rem;
    }

    .panel-title {
        font-size: 1.5rem;
        background: linear-gradient(45deg, #fff, #7EB2FF);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
        margin: 0;
    }

    @media (min-width: 768px) {
        .panel-header {
            margin-bottom: 2.5rem;
        }

        .panel-title {
            font-size: 2rem;
        }
    }

                    .back-link {
                        color: #1E90FF;
                        text-decoration: none;
                        font-size: 0.9rem;
                        transition: color 0.3s ease;
                    }

                    .back-link:hover {
                        color: #7EB2FF;
                    }


    .section-title {
        color: #7EB2FF;
        font-size: 1.2rem;
        margin-bottom: 1rem;
    }

    .phone-display {
        margin: 1rem 0;
        max-height: 300px;
        overflow-y: auto;
    }

    @media (min-width: 768px) {
        .section-title {
            font-size: 1.5rem;
            margin-bottom: 1.5rem;
        }

        .phone-display {
            margin: 2rem 0;
        }
    }


                    .phone-number {
                        font-family: monospace;
                        font-size: 1.5rem !important;
                        color: white;
                        letter-spacing: 2px;
                    }

                    .instruction-text {
                        color: #999;
                        font-size: 0.9rem;
                        margin-top: 1rem;
                    }

    .feature-status {
        margin-top: 1.5rem;
        text-align: left;
        background: rgba(30, 144, 255, 0.05);
        border: 1px solid rgba(30, 144, 255, 0.1);
        border-radius: 12px;
        padding: 1rem;
        font-size: 0.9rem;
    }

    .feature-status h3 {
        color: #7EB2FF;
        font-size: 1rem;
        margin: 0.75rem 0 0.5rem 0;
    }

    @media (min-width: 768px) {
        .feature-status {
            margin-top: 2rem;
            padding: 1.5rem;
            font-size: 1rem;
        }

        .feature-status h3 {
            font-size: 1.1rem;
            margin: 1rem 0 0.5rem 0;
        }
    }

                    .feature-status h3:first-child {
                        margin-top: 0;
                    }

                    .feature-status h4 {
                        color: #7EB2FF;
                        font-size: 0.9rem;
                        margin: 1rem 0 0.5rem 0;
                    }

                    .feature-status h3:first-child {
                        margin-top: 0;
                    }

                    .feature-status ul {
                        list-style: none;
                        padding: 0;
                        margin: 0 0 1.5rem 0;
                    }

                    .feature-status li {
                        color: #999;
                        font-size: 0.9rem;
                        padding: 0.3rem 0;
                        padding-left: 1.5rem;
                        position: relative;
                    }

                    .feature-status li::before {
                        content: '•';
                        position: absolute;
                        left: 0.5rem;
                        color: #1E90FF;
                    }

                    .feature-suggestion {
                        margin-top: 1.5rem;
                        padding-top: 1.5rem;
                        border-top: 1px solid rgba(30, 144, 255, 0.1);
                        color: #999;
                        font-size: 0.9rem;
                    }

                    .feature-suggestion a {
                        color: #1E90FF;
                        text-decoration: none;
                        transition: color 0.3s ease;
                    }

                    .feature-suggestion a:hover {
                        color: #7EB2FF;
                        text-decoration: underline;
                    }

                    .warning-card {
                        background: rgba(255, 193, 7, 0.1);
                        border: 1px solid rgba(255, 193, 7, 0.2);
                        border-radius: 12px;
                        padding: 1.5rem;
                        text-align: center;
                        margin: 1.5rem 0;
                    }

                    .warning-card a {
                        color: #1E90FF;
                        text-decoration: none;
                        transition: color 0.3s ease;
                    }

                    .warning-card a:hover {
                        color: #7EB2FF;
                    }

                    .warning-icon {
                        font-size: 1.5rem;
                        margin-right: 0.5rem;
                    }

                    .action-button {
                        background: linear-gradient(45deg, #1E90FF, #4169E1);
                        color: white;
                        border: none;
                        width: 100%;
                        padding: 1rem;
                        border-radius: 8px;
                        font-size: 1rem;
                        cursor: pointer;
                        transition: all 0.3s ease;
                        margin-top: 1.5rem;
                    }

                    .action-button:hover {
                        transform: translateY(-2px);
                        box-shadow: 0 4px 20px rgba(30, 144, 255, 0.3);
                    }

                    /* Responsive design */
                    @media (max-width: 768px) {
                        .dashboard-container {
                            padding: 2rem 1rem;
                        }

                        .phone-number {
                            font-size: 1.5rem;
                        }

                        .panel-title {
                            font-size: 1.75rem;
                        }
                    }

                    .instruction-text {
                        color: #999;
                        font-size: 0.9rem;
                        margin-top: 1rem;
                    }

    

    .dashboard-panel {
        background: rgba(30, 30, 30, 0.7);
        border: 1px solid rgba(30, 144, 255, 0.1);
        border-radius: 16px;
        padding: 1.5rem;
        width: 100%;
        max-width: 700px;
        backdrop-filter: blur(10px);
        box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
        overflow: hidden;
    }

    .dashboard-tabs {
        display: flex;
        gap: 0.5rem;
        margin-bottom: 1.5rem;
        border-bottom: 1px solid rgba(30, 144, 255, 0.1);
        padding-bottom: 0.75rem;
        overflow-x: auto;
        -webkit-overflow-scrolling: touch;
    }

    .tab-button {
        background: transparent;
        border: none;
        color: #999;
        padding: 0.5rem 0.75rem;
        cursor: pointer;
        font-size: 0.9rem;
        transition: all 0.3s ease;
        position: relative;
        white-space: nowrap;
    }

    @media (min-width: 768px) {
        .dashboard-tabs {
            gap: 1rem;
            margin-bottom: 2rem;
            padding-bottom: 1rem;
        }

        .tab-button {
            padding: 0.5rem 1rem;
            font-size: 1rem;
        }
    }

                    .tab-button::after {
                        content: '';
                        position: absolute;
                        bottom: -1rem;
                        left: 0;
                        width: 100%;
                        height: 2px;
                        background: transparent;
                        transition: background-color 0.3s ease;
                    }

                    .tab-button.active {
                        color: #1E90FF;
                    }

                    .tab-button.active::after {
                        background: #1E90FF;
                    }

                    .tab-button:hover {
                        color: #7EB2FF;
                    }

                    .proactive-tab .coming-soon {
                        text-align: center;
                        padding: 2rem;
                        background: rgba(30, 144, 255, 0.05);
                        border-radius: 12px;
                        border: 1px solid rgba(30, 144, 255, 0.1);
                        margin: 2rem 0;
                    }

                    .proactive-tab .coming-soon h3 {
                        color: #7EB2FF;
                        font-size: 1.5rem;
                        margin-bottom: 1rem;
                    }

                    .proactive-tab .coming-soon p {
                        color: #999;
                        margin-bottom: 1.5rem;
                    }

                    .proactive-tab .coming-soon ul {
                        list-style: none;
                        padding: 0;
                        text-align: left;
                        max-width: 300px;
                        margin: 0 auto;
                    }

                    .proactive-tab .coming-soon li {
                        color: #999;
                        padding: 0.5rem 0;
                        padding-left: 1.5rem;
                        position: relative;
                    }

                    .proactive-tab .coming-soon li::before {
                        content: '•';
                        position: absolute;
                        left: 0.5rem;
                        color: #1E90FF;
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

                    .nice-link {
                        color: #007bff;
                        text-decoration: none;
                        position: relative;
                        padding: 0 2px;
                        transition: all 0.3s ease;
                    }

                    .nice-link::after {
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

                    .nice-link:hover {
                        color: #7EB2FF;
                        text-shadow: 0 0 8px rgba(30, 144, 255, 0.3);
                    }

                    .nice-link:hover::after {
                        transform: scaleX(1);
                        transform-origin: bottom left;
                    }

                    .development-links a:hover::after {
                        transform: scaleX(1);
                        transform-origin: bottom left;
                    }

                    /* Blog Styles */
                    .blog-container {
                        max-width: 800px;
                        margin: 0 auto;
                        padding: 2rem;
                        margin-top: 74px;
                        min-height: 100vh;
                    }

                    .blog-post {
                        background: rgba(30, 30, 30, 0.7);
                        border: 1px solid rgba(30, 144, 255, 0.1);
                        border-radius: 16px;
                        padding: 3rem;
                        backdrop-filter: blur(10px);
                        box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
                    }

                    .blog-header {
                        margin-bottom: 3rem;
                        text-align: center;
                    }

                    .blog-header h1 {
                        font-size: 2.5rem;
                        margin-bottom: 1rem;
                        background: linear-gradient(45deg, #fff, #7EB2FF);
                        -webkit-background-clip: text;
                        -webkit-text-fill-color: transparent;
                        line-height: 1.2;
                    }

                    .blog-meta {
                        color: #999;
                        font-size: 0.9rem;
                        display: flex;
                        justify-content: center;
                        gap: 1rem;
                    }

                    .blog-content {
                        color: #e0e0e0;
                        line-height: 1.8;
                    }

                    .blog-content h2 {
                        color: #7EB2FF;
                        font-size: 1.8rem;
                        margin: 2rem 0 1rem;
                    }

                    .blog-content p {
                        margin-bottom: 1.5rem;
                        font-size: 1.1rem;
                    }

                    .blog-image {
                        max-width: 40%;
                        height: auto;
                        margin: 2rem 0;
                        border-radius: 8px;
                        box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
                        display: block;
                    }

                    .blog-content ul {
                        margin-bottom: 1.5rem;
                        padding-left: 1.5rem;
                        list-style-type: disc;
                        color: #e0e0e0;
                    }

                    .blog-content li {
                        margin-bottom: 0.5rem;
                        font-size: 1.1rem;
                        line-height: 1.6;
                    }

                    .blog-content a {
                        color: #1E90FF;
                        text-decoration: none;
                        position: relative;
                        padding: 0 2px;
                        transition: all 0.3s ease;
                    }

                    .blog-content a::after {
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

                    .blog-content a:hover {
                        color: #7EB2FF;
                        text-shadow: 0 0 8px rgba(30, 144, 255, 0.3);
                    }

                    .blog-content a:hover::after {
                        transform: scaleX(1);
                        transform-origin: bottom left;
                    }

                    .blog-navigation {
                        margin-top: 3rem;
                        padding-top: 2rem;
                        border-top: 1px solid rgba(30, 144, 255, 0.1);
                    }

                    .blog-nav-link {
                        display: inline-block;
                        color: #1E90FF;
                        text-decoration: none;
                        font-size: 1.1rem;
                        transition: all 0.3s ease;
                    }

                    .blog-nav-link:hover {
                        color: #7EB2FF;
                        transform: translateX(5px);
                    }

                    .blog-content ul {
                        margin-bottom: 1.5rem;
                        padding-left: 1.5rem;
                        list-style-type: disc;
                        color: #e0e0e0;
                    }

                    .blog-content li {
                        margin-bottom: 0.5rem;
                        font-size: 1.1rem;
                        line-height: 1.6;
                    }

                    .blog-navigation {
                        margin-top: 3rem;
                        padding-top: 2rem;
                        border-top: 1px solid rgba(30, 144, 255, 0.1);
                    }

                    .blog-nav-link {
                        display: inline-block;
                        color: #1E90FF;
                        text-decoration: none;
                        font-size: 1.1rem;
                        transition: all 0.3s ease;
                    }

                    .blog-nav-link:hover {
                        color: #7EB2FF;
                        transform: translateX(5px);
                    }

                    @media (max-width: 768px) {
                        .blog-container {
                            padding: 1rem;
                        }

                        .blog-post {
                            padding: 1.5rem;
                        }

                        .blog-header h1 {
                            font-size: 2rem;
                        }

                        .blog-content h2 {
                            font-size: 1.5rem;
                        }

                        .blog-content p {
                            font-size: 1rem;
                        }
                    }

                   
                "#}
            </style>
        </div>

    }
}
