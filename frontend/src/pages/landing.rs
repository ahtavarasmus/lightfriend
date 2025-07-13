
use yew::prelude::*;
use crate::auth::connect::Connect;
use wasm_bindgen::prelude::*;
use web_sys::{Element, HtmlElement, HtmlElement as HtmlElementTrait};
use yew_router::prelude::*;
use crate::Route;
use yew_router::components::Link;
use crate::config;
use web_sys::{window, HtmlInputElement};
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use serde_json::json;
use wasm_bindgen_futures::spawn_local;
use gloo_timers::callback::Timeout;


#[function_component(Landing)]
pub fn landing() -> Html {
    let current_phone_word = use_state(|| 0);


    // Scroll to top only on initial mount
    {
        use_effect_with_deps(
            move |_| {
                if let Some(window) = web_sys::window() {
                    window.scroll_to_with_x_and_y(0.0, 0.0);
                }
                || ()
            },
            (), // Empty dependencies array means this effect runs only once on mount
        );
    }


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
                
                let sticky_scroll = scroll_pos - (window_height*2.3 * 1.2);  // Increased to 0.5 to delay appearance
                let sticky_duration = window_height*2.3 * 4.0;  // Keep this the same
                
                // Calculate intro section opacity based on scroll position
                if sticky_scroll > sticky_duration * 0.6 {  // Changed from 0.75 to 0.6 to start fading earlier
                    let fade_progress = ((sticky_scroll - (sticky_duration * 0.6)) / (sticky_duration * 0.4)).min(1.0);  // Adjusted fade range
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
                
                if scroll_pos > window_height*2.3 * 0.8 {  // Increased to 0.6 to start transition later
                    if !current_classes.contains("visible") {

                        intro_section.set_class_name(&format!("{} visible", base_classes));
                    }
                    
                    // Calculate relative scroll position within the sticky section
                    let sticky_scroll = scroll_pos - (window_height*2.3 * 0.8);  // Increased to match the above change
                    let sticky_duration = window_height * 1.5; // Reduced to 1.5 for shorter duration
                    
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
                                } else if sticky_scroll < sticky_duration * 0.45 {  // Adjusted from 0.5 to 0.45
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
    /*
    let onclick = {
        let is_privacy_expanded = is_privacy_expanded.clone();
        Callback::from(move |_| {
            is_privacy_expanded.set(!*is_privacy_expanded);
        })
    };
    */


    html! {

        <div class="landing-page">
        <header class="hero">
                <div class="hero-background"></div>
                <div class="hero-content">
                    <div class="hero-header">
                        <p class="hero-subtitle">
                            {"Let lightfriend monitor the situation. Smart notifications let you skip distractions or revive that dumbphone."}
                        </p>
                    </div>
                    <div class="hero-cta-group">
                        <Link<Route> to={Route::Pricing} classes="forward-link">
                            <button class="hero-cta">{"Get Started"}</button>
                        </Link<Route>>
                        <a href="/faq#try-service" class="faq-link">
                            {"Try demo first"}
                        </a>
                    </div>
                </div>
        </header>        

            <div class="filter-concept">
                <div class="filter-content">
                    <div class="filter-text">
                        <h2>{"Your Digital Filter"}</h2>
                        <p>{"Lightfriend acts as your intelligent filter between all your digital services and your phone. Whether you use a dumbphone or want to silence your smartphone, Lightfriend ensures you never miss what's truly important."}</p>
                    </div>
                    <div class="filter-image">
                        <img src="/assets/lightfriend-filter.png" alt="Lightfriend filtering concept" loading="lazy" />
                    </div>
                </div>
            </div>

            <div class="feature-block">
                <div class="feature-content">
                    <h2>{"Filter the Noise"}</h2>
                    <p>{"Lightfriend sends instant SMS/call alerts ONLY for critical messages, emails or events."}</p>
                    <ul class="feature-list">
                        <li>{"🔔 Instant SMS/Call Alerts for Critical WhatsApp Messages or Emails"}</li>
                        <li>{"⏰ Scheduled SMS Summaries of received messages and upcoming events"}</li>
                        <li>{"⭐ Priority Sender Notifications"}</li>
                        <li>{"🔍 Set Waiting Checks for Specific Content"}</li>
                    </ul>
                </div>
                <div class="cta-image-container">
                    <div class="feature-image">
                        <img src="/assets/notifications.png" loading="lazy"  alt="Person receiving a meaningful notification" />
                    </div>
                    <div class="demo-link-container">
                        <a href="https://www.youtube.com/shorts/KrVdJbHPB-o" target="_blank" rel="noopener noreferrer" class="demo-link">
                            {"▶️ See It in Action"}
                        </a>
                        <a href="/faq#try-service" class="faq-link">
                            {"Try Demo Chat"}
                        </a>
                    </div>
                </div>
            </div>

            <section class="intro-section">
                <div class="intro-content">
                    <div class="intro-text">
                        <h2>{"Ask Anything, Anytime"}</h2>
                        <p>{"Voice call or text from any phone (even dumbphone) to ask about your stuff, search the web with Perplexity or analyze photos."}</p>
                        <ul class="feature-list">
                            <li><img src="/assets/perplexitylogo.png" loading="lazy" alt="Perplexity" class="perplexity-logo" /> {"Perplexity AI Web Search & ☀️ Weather"}</li>
                            <li>{"📧 Check or Create Messages, Emails, Events & Tasks"}</li>
                            <li>{"📸 Photo Analysis, Translation & 📱 QR Code Reader (US & AUS only)"}</li>
                        </ul>
                    </div>

                        <div class="sticky-image">
                            <img src="/assets/whatsappexample.png" alt="WhatsApp example interface" loading="lazy" class="example-image whatsapp-image" />
                            <img src="/assets/calendarexample1.webp" alt="Calendar example interface" loading="lazy" class="example-image email-image" />
                            <img src="/assets/phone_translation_example.png" alt="Photo example interface" loading="lazy" class="example-image calendar-image" />
                        </div>
                </div>

            </section>


        <section class="main-features">

            // Add mobile-only intro content first
            <div class="intro-mobile">
                    <div class="feature-content">
                        <h2>{"Ask Anything, Anytime"}</h2>
                        <p>{"Voice call or text from any phone (even dumbphone) to ask about your stuff, search the web with Perplexity or analyze photos."}</p>
                        <ul class="feature-list">
                            <li><img src="/assets/perplexitylogo.png" loading="lazy" alt="Perplexity" class="perplexity-logo" /> {"Perplexity AI Web Search & ☀️ Weather"}</li>
                            <li>{"📧 Check or Create Messages, Emails, Events & Tasks"}</li>
                            <li>{"📸 Photo Analysis, Translation & 📱 QR Code Reader (US & AUS only)"}</li>
                        </ul>
                    </div>

                        <div class="sticky-image">
                            <img src="/assets/whatsappexample.png" alt="WhatsApp example interface" loading="lazy" class="example-image whatsapp-image" />
                            <img src="/assets/calendarexample1.webp" alt="Calendar example interface" loading="lazy" class="example-image email-image" />
                            <img src="/assets/phone_translation_example.png" alt="Photo example interface" loading="lazy" class="example-image calendar-image" />
                        </div>
            </div>
            <div class="section-header">
                <div class="section-intro">
                    <Link<Route> to={Route::Pricing} classes="forward-link">
                        <button class="hero-cta">{"Get Started"}</button>
                    </Link<Route>>
                </div>
            </div>
        </section>

        <section class="how-it-works">
            <h2>{"Off Load Monitoring"}</h2>
            <p>{"Whether you use a smartphone or dumbphone, Lightfriend lets you stay focused while catching what matters."}</p>
            <div class="steps-grid">
                <div class="step">
                    <h3>{"Connect Your Apps"}</h3>
                    <p>{"Link your messaging apps, email, and calendar through our secure portal and let Lightfriend monitor them."}</p>
                </div>
                <div class="step">
                    <h3>{"Set Your Priorities"}</h3>
                    <p>{"Choose what's important - schedule summaries, set waiting checks or priority senders."}</p>
                </div>
                <div class="step">
                    <h3>{"Stay Present"}</h3>
                    <p>{"Focus on life knowing LightFriend will alert you about important messages. Perfect for both smartphone and dumbphone users."}</p>
                </div>
            </div>
        </section>

        <footer class="footer-cta">
            <div class="footer-content">
                <h2>{"Ready for Digital Peace?"}</h2>
                <p class="subtitle">{"Stop checking your phone constantly. Let LightFriend monitor your messages and notify you only about what matters."}</p>
                <Link<Route> to={Route::Pricing} classes="forward-link">
                    <button class="hero-cta">{"Start Monitoring Today"}</button>
                </Link<Route>>
                <p class="disclaimer">{"Works with both smartphones and basic phones - even on Nokia 3310. Customize notifications to your needs."}</p>
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
        transform: translateZ(0);
        -webkit-overflow-scrolling: touch;
        -webkit-backface-visibility: hidden;
        scrollbar-width: none;
        backface-visibility: hidden;
        pointer-events: none;
    }

    @media (max-width: 768px) {
        .intro-section {
            display: none;
        }
    }

    .intro-mobile {
        display: none;
    }

    @media (max-width: 768px) {
        .intro-mobile {
            display: block !important;
            margin: 4rem 1rem 2rem 1rem;
            position: relative;
            z-index: 5;
            background: rgba(30, 30, 30, 0.8);
            border: 1px solid rgba(30, 144, 255, 0.15);
            border-radius: 24px;
            padding: 2rem;
        }

        .intro-mobile p {
            color: #999;
            font-size: 1.1rem;
            line-height: 1.6;
            margin-bottom: 2rem;
        }
    }

    @media (min-width: 769px) {
        .intro-mobile {
            display: none;
        }
    }

    .intro-section.visible {
        pointer-events: auto;
        z-index: 1;
        opacity: 1;
        visibility: visible;
        transform: translateY(0);
        transition: opacity 0.8s ease;
    }

    .intro-section.sticky {
        position: fixed;
        top: 0;
        left: 0;
        right: 0;
        z-index: 2;
    }

    .intro-section::-webkit-scrollbar {
        display: none;
    }

    @media (max-width: 768px) {
        .intro-content {
            flex-direction: column;
            text-align: center;
            gap: 0;
            padding: 0;
            height: 100vh;
            overflow: hidden;
        }
    }

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
        .example-image {
            position: absolute;
            max-width: 280px;
            height: auto;
            max-height: none;
            left: 50%;
            top: 50%;
            transform: translate(-50%, -50%);
            -webkit-transform: translate(-50%, -50%);
            object-fit: contain;
        }
    }

    .example-image.visible {
        opacity: 1;
        z-index: 6;
    }

    .sticky-image {
        position: sticky;
        top: 20vh;
        width: 400px;
        height: 600px;
        margin: 0 !important;
        z-index: 10;
    }

    @media (max-width: 768px) {
        .sticky-image {
            position: fixed !important;
            top: 50% !important;
            left: 50% !important;
            transform: translate3d(-50%, -50%, 0) !important;
            -webkit-transform: translate3d(-50%, -50%, 0) !important;
            width: 320px !important;
            height: 500px !important;
            margin: 0 !important;
            z-index: 10;
            overflow: visible;
            backface-visibility: hidden;
            -webkit-backface-visibility: hidden;
        }
    }

    .whatsapp-image, .email-image, .calendar-image {
        position: absolute;
        top: 0;
        left: 0;
        width: 100%;
        height: 100%;
    }

    .intro-text {
        z-index: 2;
        padding: 20px 30px;
        display: flex;
        flex-direction: column;
        justify-content: space-around;
        padding-right: 2rem;
        background: rgba(30, 30, 30, 0.8);
        border: 1px solid rgba(30, 144, 255, 0.15);
        border-radius: 24px;
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
    }

    @media (max-width: 768px) {
        .intro-section {
            padding: 4rem 1rem;
        }
        .intro-text {
            display: none !important;
        }
    }

    .filter-concept {
        padding: 4rem 2rem;
        margin: 0 auto;
        max-width: 1200px;
        position: relative;
        z-index: 2;
    }

    .filter-content {
        display: flex;
        align-items: center;
        gap: 4rem;
        background: rgba(30, 30, 30, 0.8);
        border: 1px solid rgba(30, 144, 255, 0.15);
        border-radius: 24px;
        padding: 3rem;
        transition: transform 0.3s ease, box-shadow 0.3s ease;
    }

    .filter-content:hover {
        transform: translateY(-5px);
        box-shadow: 0 8px 32px rgba(30, 144, 255, 0.15);
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

    .filter-text p {
        color: #999;
        font-size: 1.2rem;
        line-height: 1.6;
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

    @media (max-width: 768px) {
        .filter-concept {
            padding: 2rem 1rem;
        }

        .filter-content {
            flex-direction: column;
            padding: 2rem;
            gap: 2rem;
            text-align: center;
        }

        .filter-text h2 {
            font-size: 2rem;
        }

        .filter-text p {
            font-size: 1.1rem;
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
        background: rgba(30, 30, 30, 0.8);
        border: 1px solid rgba(30, 144, 255, 0.15);
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

    .feature-image img {
        max-width: 100%;
        height: auto;
        border-radius: 12px;
        box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
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
        padding: 40px;
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

    @media (max-width: 768px) {
        .hero-content {
            padding: 20px;
            flex-direction: column;
            justify-content: flex-end;
        }

        .hero-subtitle {
            font-size: 1.1rem;
            line-height: 1.6;
            margin-bottom: 2rem;
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

        .section-header h2 {
            font-size: 2rem;
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
                   
                "#}
            </style>
        </div>

    }
}
