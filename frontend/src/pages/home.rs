use yew::prelude::*;
use crate::auth::connect::Connect;
use crate::pages::proactive::Proactive;
use yew_router::prelude::*;
use crate::Route;
use crate::config;
use web_sys::{window, HtmlInputElement};
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use serde_json::json;
use wasm_bindgen_futures::spawn_local;

fn render_notification_settings(profile: Option<&UserProfile>) -> Html {
    html! {
        <div class="notification-settings">
            {
                if let Some(profile) = profile {
                    html! {
                        <>
                            <div class="notify-toggle">
                                <span>{"Notifications"}</span>
                                <span class="toggle-status">
                                    {if profile.notify {"Active"} else {"Inactive"}}
                                </span>
                                <label class="switch">
                                    <input 
                                        type="checkbox" 
                                        checked={profile.notify}
                                        onchange={{
                                            let user_id = profile.id;
                                            Callback::from(move |e: Event| {
                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                let notify = input.checked();
                                                
                                                if let Some(token) = window()
                                                    .and_then(|w| w.local_storage().ok())
                                                    .flatten()
                                                    .and_then(|storage| storage.get_item("token").ok())
                                                    .flatten()
                                                {
                                                    spawn_local(async move {
                                                        let _ = Request::post(&format!("{}/api/profile/update-notify/{}", config::get_backend_url(), user_id))
                                                            .header("Authorization", &format!("Bearer {}", token))
                                                            .header("Content-Type", "application/json")
                                                            .json(&json!({"notify": notify}))
                                                            .expect("Failed to serialize notify request")
                                                            .send()
                                                            .await;
                                                    });
                                                }
                                            })
                                        }}
                                    />
                                    <span class="slider round"></span>
                                </label>
                            </div>
                            <p class="notification-description">
                                {"Receive notifications about new feature updates."}
                            </p>
                        </>
                    }
                } else {
                    html! {}
                }
            }
        </div>
    }
}

const PHONE_NUMBERS: &[(&str, &str, Option<&str>)] = &[
    ("us", "+18153684737", None),
    ("fin", "+358454901522", None),
    ("nl", "+3197006520696", None),
    ("gb", "+447383240344", None),
    ("aus", "+61489260976", None),
    ("cz", "+420910921902", Some("(no sms)")),
];

#[derive(Deserialize, Clone)]
struct UserProfile {
    id: i32,
    verified: bool,
    time_to_delete: bool,
    preferred_number: Option<String>,
    notify: bool,
    sub_tier: Option<String>,
}

#[derive(Clone, PartialEq)]
enum DashboardTab {
    Connections,
    Proactive,
}

pub fn is_logged_in() -> bool {
    if let Some(window) = window() {
        if let Ok(Some(storage)) = window.local_storage() {
            if let Ok(Some(_token)) = storage.get_item("token") {
                return true;
            }
        }
    }
    false
}


#[function_component(Landing)]
pub fn landing() -> Html {
    html! {

        <div class="landing-page">
            // Hero Section
            <header class="hero">
                <h1>{"Your AI Assistant for Dumbphones"}</h1>
                <p class="hero-subtitle">
                    {"Access your digital life through simple calls and texts - no smartphone needed."}
                </p>
                <Link<Route> to={Route::Register} classes="forward-link">
                    <button class="hero-cta">
                        {"Try Now with Free Credits"}
                    </button>
                </Link<Route>>
            </header>

            // Main Features Section
            <section class="main-features">
                <div class="feature-block on-demand">
                    <div class="feature-content">
                        <h2>{"Ask Anything, Anytime"}</h2>
                        <p>{"Need to check your email? Want to know your next meeting? Just ask lightfriend through a call or text."}</p>
                        <ul class="feature-list">
                            <li>{"Access emails, calendar and messaging apps"}</li>
                            <li>{"Search the internet"}</li>
                            <li>{"Check weather"}</li>
                            <li>{"Identify songs with Shazam"}</li>
                        </ul>
                        <div class="demo-link-container">
                            <a 
                                href="https://www.youtube.com/shorts/KrVdJbHPB-o" 
                                target="_blank" 
                                rel="noopener noreferrer"
                                class="demo-link"
                            >
                                {"▶️ Watch Demo"}
                            </a>
                        </div>
                    </div>
                    <div class="feature-image">
                        <img src="/assets/train.png" alt="On-demand assistance demo" />
                    </div>
                </div>

                <div class="feature-block proactive">
                    <div class="feature-content">
                        <h2>{"Never Miss What Matters"}</h2>
                        <p>{"Set up smart notifications for important emails and updates. lightfriend monitors your accounts and notifies you only when it matters."}</p>
                        <ul class="feature-list">
                            <li>{"Smart email filtering"}</li>
                            <li>{"Custom keyword alerts"}</li>
                            <li>{"Priority sender notifications"}</li>
                            <li>{"Temporary waiting checks"}</li>
                        </ul>
                        <div class="demo-link-container">
                        /*
                            <a 
                                href="https://youtube.com/shorts/4ZYnhtm9dkk" 
                                target="_blank" 
                                rel="noopener noreferrer"
                                class="demo-link"
                            >
                                {"▶️ Watch Demo"}
                            </a>
                        */
                        </div>
                    </div>
                    <div class="feature-image">
                        <img src="/assets/delivery.png" alt="Proactive notifications demo" />
                    </div>
                </div>
            </section>
            // ProductHunt Launch Demo section
            <section class="producthunt-demo">
                <h2>{"Featured on ProductHunt"}</h2>
                <div class="producthunt-iframe-container">
                    <iframe 
                        style="border: none;" 
                        src="https://cards.producthunt.com/cards/products/1050798" 
                        width="500" 
                        height="405" 
                        frameborder="0" 
                        scrolling="no" 
                        allowfullscreen={true}
                    >
                    </iframe>
                </div>
            </section>

            // How It Works section
            <section class="how-it-works">
                <h2>{"How lightfriend Works"}</h2>
                <p>{"Three simple steps to digital freedom"}</p>

                <div class="steps-grid">
                    <div class="step">
                        <h3>{"Connect Your Services"}</h3>
                        <p>{"Link your calendar, email, and messaging accounts through our secure web interface."}</p>
                    </div>

                    <div class="step">
                        <h3>{"Use Your Dumbphone"}</h3>
                        <p>{"Call or text your lightfriend to access your connected services anytime, anywhere."}</p>
                    </div>

                    <div class="step">
                        <h3>{"Stay Present"}</h3>
                        <p>{"Enjoy life without digital distractions, knowing essential information is just a call away."}</p>
                    </div>
                </div>
            </section>

            <footer class="footer-cta">
                <div class="footer-content">
                    <h2>{"Ready to Simplify and Stay Connected?"}</h2>
                    <p class="subtitle">
                        {"Join the digital minimalism movement with lightfriend — stay informed without endless distractions."}
                    </p>
                    <Link<Route> to={Route::Register} classes="forward-link">
                        <button class="hero-cta">
                                {"Start Now with Free Credits"}

                        </button>
                    </Link<Route>>
                    <p class="disclaimer">
                        {"No smartphone required. Works with any basic phone."}
                    </p>
                <div class="development-links">
                    <p>{"Source code available on "}
                        <a href="https://github.com/ahtavarasmus/lightfriend" target="_blank" rel="noopener noreferrer">
                            {"GitHub"}
                        </a>
                    </p>
                    <p>{"Follow development progress at "}
                        <a href="https://pacepeek.com/ahtavarasmus" target="_blank" rel="noopener noreferrer">
                            {"pacepeek.com/ahtavarasmus"}
                        </a>
                    {" and "}
                    <a href="https://x.com/rasmuscodes" target="_blank" rel="noopener noreferrer">
                        {"x.com/rasmuscodes"}
                    </a>
                    </p>
                    <div class="legal-links">
                        <a href="/terms">{"Terms & Conditions"}</a>
                        {" | "}
                        <a href="/privacy">{"Privacy Policy"}</a>
                    </div>
                </div>
                </div>
            </footer>
            <style>
                {r#"

                    .producthunt-demo {
                        padding: 2rem 0;
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
                    background-color: #1a1a1a;
                    color: #ffffff;
                    font-family: system-ui, -apple-system, sans-serif;
                    margin: 0 auto;
                    width: 100%;
                    overflow-x: hidden;
                    box-sizing: border-box;
                }

                .hero {
                    text-align: center;
                    padding: 6rem 2rem;
                    margin: 0 auto;
                }

                .hero h1 {
                    font-size: 4rem;
                    line-height: 1.1;
                    margin-bottom: 1.5rem;
                    background: linear-gradient(
                        45deg,
                        #fff,
                        rgba(126, 178, 255, 0.8)
                    );
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                    font-weight: 700;
                    max-width: 800px;
                    margin: 0 auto 1.5rem;
                }

                .main-features {
                    max-width: 1200px;
                    margin: 0 auto;
                    padding: 4rem 2rem;
                }

                .feature-block {
                    display: flex;
                    align-items: center;
                    gap: 4rem;
                    margin-bottom: 6rem;
                    background: rgba(30, 30, 30, 0.5);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 24px;
                    padding: 3rem;
                    transition: all 0.3s ease;
                }

                .feature-block:hover {
                    transform: translateY(-5px);
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
                    content: "→";
                    position: absolute;
                    left: 0;
                    color: #1E90FF;
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

                    .feature-block h2 {
                        font-size: 2rem;
                    }
                }

                @media (max-width: 768px) {
                    .hero h1 {
                        font-size: 2.5rem;
                    }

                    .hero {
                        padding: 4rem 1rem;
                    }

                    .main-features {
                        padding: 2rem 1rem;
                    }

                    .feature-block {
                        margin-bottom: 3rem;
                    }

                    .feature-list li {
                        font-size: 1rem;
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
                            font-size: 2rem;
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
                        justify-content: center;
                        text-align: center;
                        padding: 6rem 0;
                        position: relative;
                        padding-top: 120px; /* Add extra padding to prevent content from hiding under nav */
                        background: linear-gradient(
                            to bottom,
                            rgba(30, 144, 255, 0.05),
                            transparent
                        );
                    }

                    .hero h1 {
                        font-size: 4.5rem;
                        line-height: 1.1;
                        margin-bottom: 1.5rem;
                        background: linear-gradient(
                            45deg,
                            #fff,
                            rgba(126, 178, 255, 0.8)
                        );
                        -webkit-background-clip: text;
                        -webkit-text-fill-color: transparent;
                        font-weight: 700;
                        max-width: 900px;
                        position: relative;
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
                        color: #999;
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
                        transition: all 0.3s ease;
                        display: inline-flex;
                        align-items: center;
                        gap: 0.5rem;
                        position: relative;
                        overflow: hidden;
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

                    /* Add floating elements effect */
                    .hero::before,
                    .hero::after {
                        content: '';
                        position: absolute;
                        width: 300px;
                        height: 300px;
                        border-radius: 50%;
                        background: radial-gradient(
                            circle,
                            rgba(30, 144, 255, 0.1),
                            transparent
                        );
                        z-index: -1;
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

                    .info-section {
                        padding: 1.5rem 0;
                        margin: 1rem 0;
                        text-align: center;
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

// Separate the deletion logic
fn delete_unverified_account(profile_id: i32, token: String) {
    wasm_bindgen_futures::spawn_local(async move {
        let _ = Request::delete(&format!("{}/api/profile/delete/{}", config::get_backend_url(), profile_id))
            .header("Authorization", &format!("Bearer {}", token))
            .send()
            .await;
        
        if let Some(window) = window() {
            if let Ok(Some(storage)) = window.local_storage() {
                let _ = storage.remove_item("token");
                let _ = window.location().set_href("/");
            }
        }
    });
}

#[function_component]
pub fn Home() -> Html {

    let logged_in = is_logged_in();
    let profile_data = use_state(|| None::<UserProfile>);
    let user_verified = use_state(|| true);
    let error = use_state(|| None::<String>);
    let is_expanded = use_state(|| false);
    let active_tab = use_state(|| DashboardTab::Connections);
    let navigator = use_navigator().unwrap();

    // Single profile fetch effect
    {
        let profile_data = profile_data.clone();
        let user_verified = user_verified.clone();
        let error = error.clone();
        
        use_effect_with_deps(move |_| {
            let profile_data = profile_data.clone();
            let user_verified = user_verified.clone();
            let error = error.clone();


            wasm_bindgen_futures::spawn_local(async move {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|storage| storage.get_item("token").ok())
                    .flatten()
                {
                    match Request::get(&format!("{}/api/profile", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.status() == 401 {
                                if let Some(window) = window() {
                                    if let Ok(Some(storage)) = window.local_storage() {
                                        let _ = storage.remove_item("token");
                                        let _ = window.location().set_href("/");
                                    }
                                }
                                return;
                            }
                            
                            match response.json::<UserProfile>().await {
                                Ok(profile) => {
                                    user_verified.set(profile.verified);
                                    profile_data.set(Some(profile));
                                    error.set(None);
                                }
                                Err(_) => {
                                    error.set(Some("Failed to parse profile data".to_string()));
                                }
                            }
                        }
                        Err(_) => {
                            error.set(Some("Failed to fetch profile".to_string()));
                        }
                    }
                }
            });
            
            || ()
        }, ());
    }

    // If not logged in, show landing page
    if !logged_in {
        html! { <Landing /> }
    } else if !*user_verified {
        // If logged in but not verified, redirect to verify page
        navigator.push(&Route::Verify);
        html! {}
    } else {
            html! {
                <>
                <div class="dashboard-container">
                    <h1 class="panel-title">{"Dashboard"}</h1>
                    <h2 class="section-title">{"Your lightfriend is Ready!"}</h2>
                        <div class="phone-selector">
                            <button 
                                class="selector-btn"
                                onclick={let is_expanded = is_expanded.clone(); 
                                    move |_| is_expanded.set(!*is_expanded)}>
                                {
                                    if let Some(profile) = (*profile_data).as_ref() {
                                        if let Some(preferred) = &profile.preferred_number {
                                            format!("Your lightfriend's Number: {}", preferred)
                                        } else {
                                            "Select Your lightfriend's Number".to_string()
                                        }
                                    } else {
                                        "Select Your lightfriend's Number".to_string()
                                    }
                                }
                            </button>
                            
                            if *is_expanded {
                                <div class="phone-display">
                                    { PHONE_NUMBERS.iter().map(|(country, number, note)| {
                                        let number_clean = number.to_string();  // Store clean number for API use
                                        let display_number = if let Some(note_text) = note {
                                            format!("{} {}", number, note_text)
                                        } else {
                                            number.to_string()
                                        };
                                        let is_selected = if let Some(profile) = (*profile_data).as_ref() {
                                            profile.preferred_number.as_ref().map_or(false, |pref| pref.trim() == number_clean.trim())
                                        } else {
                                            false
                                        };
                                        
                                        let onclick = {
                                            let number = number_clean.clone();
                                            let profile_data = profile_data.clone();
                                            let is_expanded = is_expanded.clone();
                                            
                                            Callback::from(move |_| {
                                                let number = number.clone();
                                                let profile_data = profile_data.clone();
                                                
                                                if let Some(token) = window()
                                                    .and_then(|w| w.local_storage().ok())
                                                    .flatten()
                                                    .and_then(|storage| storage.get_item("token").ok())
                                                    .flatten()
                                                {
                                                    wasm_bindgen_futures::spawn_local(async move {
                                                        let response = Request::post(&format!("{}/api/profile/preferred-number", config::get_backend_url()))
                                                            .header("Authorization", &format!("Bearer {}", token))
                                                            .header("Content-Type", "application/json")
                                                            .body(format!("{{\"preferred_number\": \"{}\"}}", number))
                                                            .send()
                                                            .await;
                                                        
                                                        if let Ok(response) = response {
                                                            if response.status() == 200 {
                                                                if let Some(mut current_profile) = (*profile_data).clone() {
                                                                    current_profile.preferred_number = Some(number);
                                                                    profile_data.set(Some(current_profile));
                                                                }
                                                            }
                                                        }
                                                    });
                                                }
                                                is_expanded.set(false);
                                            })
                                        };

                                        html! {
                                            <div 
                                                class={classes!("phone-number-item", if is_selected { "selected" } else { "" })}
                                                onclick={onclick}
                                            >
                                                <div class="number-info">
                                                    <span class="country">{country}</span>
                                                    <span class="number">{display_number}</span>
                                                    if is_selected {
                                                        <span class="selected-indicator">{"✓"}</span>
                                                    }
                                                </div>
                                            </div>
                                        }
                                    }).collect::<Html>() }
                                </div>
                            }
                            
                        </div>
                        
                        <p class="instruction-text">
                            {"Select the best number for you above."}
                            <br/>
                            <br/>
                        </p>


                    <div class="dashboard-tabs">
                        <button 
                            class={classes!("tab-button", (*active_tab == DashboardTab::Connections).then(|| "active"))}
                            onclick={{
                                let active_tab = active_tab.clone();
                                Callback::from(move |_| active_tab.set(DashboardTab::Connections))
                            }}
                        >
                            {"Connections"}
                        </button>
                        <button 
                            class={classes!("tab-button", (*active_tab == DashboardTab::Proactive).then(|| "active"))}
                            onclick={{
                                let active_tab = active_tab.clone();
                                Callback::from(move |_| active_tab.set(DashboardTab::Proactive))
                            }}
                        >
                            {"Proactive"}
                        </button>
                    </div>
                        {
                            match *active_tab {
                                DashboardTab::Connections => html! {
                                    <div class="connections-tab">
                            {
                                if let Some(profile) = (*profile_data).as_ref() {
                                    html! {
                                        <Connect user_id={profile.id} />
                                    }
                                } else {
                                    html! {}
                                }
                            }

                        <div class="feature-status">
                            <h3>{"Currently Available"}</h3>
                            <h4>{"Proactive messaging (email)(subscription needed)"}</h4>
                            <h4>{"Tools"}</h4>
                            <ul>
                                <li>{"Perplexity AI search"}</li>
                                <li>{"Email and calendar integration"}</li>
                                <li>{"Dedicated Weather search"}</li>
                                <li>{"Send info to you by sms during voice calls"}</li>
                                <li>{"Shazam song recognition - Get a call, play the song, AI sends it to you by sms."}</li>
                                <li>{"Set up waiting checks for proactive messaging (subscription needed)"}</li>
                            </ul>
                            <h4>{"Methods"}</h4>
                            <ul>
                                <li>{"Voice calling"}</li>
                                <li>{"Text messaging"}</li>
                            </ul>

                            <h4>{"Tips"}</h4>
                            <ul>
                                <li>{"You can ask multiple questions in a single SMS to save money. Note that answers will be less detailed due to SMS character limits. Example: 'did sam altman tweet today and whats the weather?' -> 'Sam Altman hasn't tweeted today. Last tweet was on March 3, a cryptic \"!!!\" image suggesting a major AI development. Weather in Tampere: partly cloudy, 0°C, 82% humidity, wind at 4 m/s.'"}</li>
                                <li>{"Start your message with 'forget' to make the assistant forget previous conversation context and start fresh. Note that this only applies to that one message - the next message will again remember previous context."}</li>
                                <li>{"For Shazam song recognition, ask the assistant to use shazam or identify a song. Then assistant will make a call to you and it will listen to audio. Once recognized the song name will be texted to you and you can close the call."}</li>
                            </ul>
                            <h3>{"Coming Soon"}</h3>
                            <ul>
                                <li>{"WhatsApp and Telegram integration"}</li>
                                <li>{"Reminder setting"}</li>
                                <li>{"Camera functionality for photo translation and more"}</li>
                            </ul>
                            
                            <p class="feature-suggestion">
                                {"Have a feature in mind? Email your suggestions to "}
                                <a href="mailto:rasmus@ahtava.com">{"rasmus@ahtava.com"}</a>
                            </p>

                        </div>

                        <div class="notification-settings">
                                {
                                    if let Some(profile) = (*profile_data).as_ref() {
                                        html! {
                                            <>
                                                <div class="notify-toggle">
                                                    <span>{"Notifications"}</span>
                                                    <span class="toggle-status">
                                                        {if profile.notify {"Active"} else {"Inactive"}}
                                                    </span>
                                                    <label class="switch">
                                                        <input 
                                                            type="checkbox" 
                                                            checked={profile.notify}
                                                            onchange={{
                                                                let user_id = profile.id;
                                                                let profile_data = profile_data.clone();
                                                                Callback::from(move |e: Event| {
                                                                    let input: HtmlInputElement = e.target_unchecked_into();
                                                                    let notify = input.checked();
                                                                    let profile_data = profile_data.clone();
                                                                    
                                                                    if let Some(token) = window()
                                                                        .and_then(|w| w.local_storage().ok())
                                                                        .flatten()
                                                                        .and_then(|storage| storage.get_item("token").ok())
                                                                        .flatten()
                                                                    {
                                                                        spawn_local(async move {
                                                                            let _ = Request::post(&format!("{}/api/profile/update-notify/{}", config::get_backend_url(), user_id))
                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                .header("Content-Type", "application/json")
                                                                                .json(&json!({"notify": notify}))
                                                                                .expect("Failed to serialize notify request")
                                                                                .send()
                                                                                .await;

                                                                            // Update local state after successful API call
                                                                            if let Some(mut current_profile) = (*profile_data).clone() {
                                                                                current_profile.notify = notify;
                                                                                profile_data.set(Some(current_profile));
                                                                            }
                                                                        });
                                                                    }
                                                                })
                                                            }}
                                                        />
                                                        <span class="slider round"></span>
                                                    </label>
                                                </div>
                                                <p class="notification-description">
                                                    {"Receive notifications about new feature updates."}
                                                </p>
                                            </>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
                            </div>

                                    </div>
                                },
                                DashboardTab::Proactive => html! {
                                    <div class="proactive-tab">
                                        {
                                            if let Some(profile) = (*profile_data).as_ref() {
                                                if profile.sub_tier.is_some() {
                                                    html! {
                                                        <>
                                                            <Proactive user_id={profile.id} />
                                                        </>
                                                    }
                                                } else {
                                                    html! {
                                                        <div class="subscription-required">
                                                            <h3>{"Proactive Features Require a Subscription"}</h3>
                                                            <p>{"Get access to proactive features like:"}</p>
                                                            <ul>
                                                                <li>{"Priority message filtering"}</li>
                                                                <li>{"Keyword-based notifications"}</li>
                                                                <li>{"Waiting checks for important content"}</li>
                                                            </ul>
                                                            <a href="/pricing" class="upgrade-button">{"Upgrade Now"}</a>
                                                            <style>
                                                                {r#"
                                                                .subscription-required {
                                                                    background: rgba(30, 30, 30, 0.7);
                                                                    border: 1px solid rgba(30, 144, 255, 0.1);
                                                                    border-radius: 12px;
                                                                    padding: 2rem;
                                                                    text-align: center;
                                                                    margin: 2rem auto;
                                                                    max-width: 600px;
                                                                }

                                                                .subscription-required h3 {
                                                                    color: #7EB2FF;
                                                                    font-size: 1.5rem;
                                                                    margin-bottom: 1rem;
                                                                }

                                                                .subscription-required p {
                                                                    color: #fff;
                                                                    margin-bottom: 1.5rem;
                                                                }

                                                                .subscription-required ul {
                                                                    list-style: none;
                                                                    padding: 0;
                                                                    margin: 0 0 2rem 0;
                                                                    text-align: left;
                                                                }

                                                                .subscription-required ul li {
                                                                    color: #fff;
                                                                    padding: 0.5rem 0;
                                                                    position: relative;
                                                                    padding-left: 1.5rem;
                                                                }

                                                                .subscription-required ul li:before {
                                                                    content: "✓";
                                                                    color: #7EB2FF;
                                                                    position: absolute;
                                                                    left: 0;
                                                                }

                                                                .upgrade-button {
                                                                    display: inline-block;
                                                                    padding: 1rem 2rem;
                                                                    background: linear-gradient(45deg, #1E90FF, #4169E1);
                                                                    color: white;
                                                                    text-decoration: none;
                                                                    border-radius: 8px;
                                                                    font-weight: bold;
                                                                    transition: all 0.3s ease;
                                                                }

                                                                .upgrade-button:hover {
                                                                    transform: translateY(-2px);
                                                                    box-shadow: 0 4px 20px rgba(30, 144, 255, 0.3);
                                                                }
                                                                "#}
                                                            </style>
                                                        </div>
                                                    }
                                                }
                                            } else {
                                                html! {}
                                            }
                                        }
                                    </div>
                                }
                            }
                        }
                        <footer class="dashboard-footer">
                            <div class="development-links">
                                <p>{"Follow development progress at "}
                                    <a href="https://pacepeek.com/ahtavarasmus" target="_blank" rel="noopener noreferrer">
                                        {"pacepeek.com/ahtavarasmus"}
                                    </a>
                                    {" or "}
                                    <a href="https://x.com/rasmuscodes" target="_blank" rel="noopener noreferrer">
                                        {"x.com/rasmuscodes"}
                                    </a>
                                </p>
                                <div class="legal-links">
                                    <a href="/terms">{"Terms & Conditions"}</a>
                                    {" | "}
                                    <a href="/privacy">{"Privacy Policy"}</a>
                                </div>
                            </div>
                        </footer>
                </div>
            <style>
                {r#"

                    .producthunt-demo {
                        padding: 2rem 0;
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
                        background-color: #1a1a1a;
                        color: #ffffff;
                        font-family: system-ui, -apple-system, sans-serif;
                        margin: 0 auto;
                        width: 100%;
                        overflow-x: hidden;
                        box-sizing: border-box;
                    }

                    .hero h1 {
                        font-size: 4.5rem;
                        line-height: 1.1;
                        margin-bottom: 1.5rem;
                        background: linear-gradient(
                            45deg,
                            #fff,
                            rgba(126, 178, 255, 0.8)
                        );
                        -webkit-background-clip: text;
                        -webkit-text-fill-color: transparent;
                        font-weight: 700;
                        max-width: 900px;
                        position: relative;
                        width: 100%;
                        box-sizing: border-box;
                        word-wrap: break-word;
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
                            font-size: 2rem;
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
                        padding: 6rem 0;
                        text-align: center;
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
                        color: #999;
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
                        transition: all 0.3s ease;
                        display: inline-flex;
                        align-items: center;
                        gap: 0.5rem;
                        position: relative;
                        overflow: hidden;
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

                    /* Add floating elements effect */
                    .hero::before,
                    .hero::after {
                        content: '';
                        position: absolute;
                        width: 300px;
                        height: 300px;
                        border-radius: 50%;
                        background: radial-gradient(
                            circle,
                            rgba(30, 144, 255, 0.1),
                            transparent
                        );
                        z-index: -1;
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


                    .panel-title {
                        font-size: 2.5rem;
                        background: linear-gradient(45deg, #fff, #7EB2FF);
                        -webkit-background-clip: text;
                        -webkit-text-fill-color: transparent;
                        margin: 0 0 1.5rem 0;
                        text-align: center;
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

                    .info-section {
                        background: rgba(0, 0, 0, 0.2);
                        border-radius: 12px;
                        padding: 2rem;
                        margin: 1.5rem 0;
                        text-align: center;
                    }

                    .section-title {
                        color: #7EB2FF;
                        font-size: 1.5rem;
                        margin-bottom: 1.5rem;
                    }

                    .phone-display {
                        margin: 2rem 0;
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

                    .feature-status, .calendar-section {
                        margin-top: 3rem;
                        text-align: left;
                        padding: 2rem;
                        background: rgba(30, 30, 30, 0.7);
                        border-radius: 12px;
                        border: 1px solid rgba(30, 144, 255, 0.1);
                        backdrop-filter: blur(10px);
                    }

                    .calendar-section {
                        margin-top: 0;
                    }

                    .feature-status h3 {
                        color: #7EB2FF;
                        font-size: 1.1rem;
                        margin: 1rem 0 0.5rem 0;
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
                        .dashboard-panel {
                            padding: 2rem;
                        }

                        .panel-header {
                            flex-direction: column;
                            text-align: center;
                            gap: 1rem;
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

                    .dashboard-container {
                        min-height: 100vh;

                        border-radius: 12px;
                        background: #1a1a1a;
                        padding: 3rem 2rem;
                        width: 100%;
                        max-width: 800px;
                        margin: 4rem auto;
                    }

                    .dashboard-tabs {
                        display: flex;
                        gap: 1rem;
                        margin-bottom: 2rem;
                        border-bottom: 1px solid rgba(30, 144, 255, 0.1);
                        padding-bottom: 1rem;
                    }

                    .tab-button {
                        background: transparent;
                        border: none;
                        color: #999;
                        padding: 0.5rem 1rem;
                        cursor: pointer;
                        font-size: 1rem;
                        transition: all 0.3s ease;
                        position: relative;
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
            
            </>
        }
    }
}

