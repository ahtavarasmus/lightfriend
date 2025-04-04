use yew::prelude::*;
use crate::auth::connect::Connect;
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
                <div class="producthunt-badge">
                    <a href="https://www.producthunt.com/posts/lightfriend?embed=true&utm_source=badge-featured&utm_medium=badge&utm_souce=badge-lightfriend" target="_blank">
                        <img src="https://api.producthunt.com/widgets/embed-image/v1/featured.svg?post_id=947660&theme=light&t=1743335214644" 
                            alt="lightfriend - Use your dumbphone smarter with AI-powered assistance. | Product Hunt" 
                            style="width: 250px; height: 54px;" 
                            width="250" 
                            height="54" />
                    </a>
                </div>
                <h1>{"Stay Connected with lightfriend"}</h1>
                <p class="hero-subtitle">
                    {"Use your dumbphone smarter with AI-powered voice and text assistance—check messages and essential info when you need it, without endless scrolling or distractions."}
                </p>
                <Link<Route> to={Route::Register} classes="forward-link">
                    <button class="hero-cta">
                        {"Try Now with Free Credits"}
                    </button>
                </Link<Route>>
            </header>

            // Problem Section: User Challenges
            <section class="problems">
                <h2>{"Are You Facing These Challenges?"}</h2>
                <div class="challenges-grid">
                    <div class="challenge-item smartphone-addict">
                        <h3>{"Smartphone Addict"}</h3>
                        <p>{"Tired of endless scrolling and feeling overwhelmed by notifications? Spending hours watching endless feeds and regretting it after?"}</p>
                    </div>
                    <div class="challenge-item dumbphone-basic">
                        <h3>{"Basic Dumbphone User"}</h3>
                        <p>{"Worried about missing important messages or updates because your dumbphone lacks app access? Standing alone at the meeting spot when your friends changed plans last minute on Telegram? Picking up a new package from postal office, but the code was sent to your email?"}</p>
                    </div>
                </div>
            </section>

            // Transformation Section: Lightfriend’s Benefits
            <section class="transformation">
                <h2>{"lightfriend Helps You Stay in the Loop, Simply"}</h2>
                <p>{"See how lightfriend lets you access messages and key information on your dumbphone, keeping you connected when you need it — without turning it into a smartphone."}</p>
                <div class="benefits-grid">
                    <div class="benefit-item">
                        <h3>{"For Smartphone Addicts"}</h3>
                        <p>{"Break free from scrolling addiction and switch to a dumbphone with lightfriend’s minimal, on-demand tools. With lightfriend you can give up the algorithms while still staying connected when you need."}</p>
                    </div>
                    <div class="benefit-item">
                        <h3>{"For Basic Dumbphone Users"}</h3>
                        <p>{"Enjoy the simple, distraction-free life with lightfriend’s AI assistance when it matters. Access your email, calendar, messaging apps or just search internet answers when needed through natural voice calling or sms interface. Don't worry, it's not too convenient ;) - just enough to get the job done."}</p>
                    </div>
                </div>
                <div class="video-demo">
                    <p>{"Watch lightfriend in action:"}</p>
                    <a 
                        href="https://www.youtube.com/shorts/KrVdJbHPB-o" 
                        target="_blank" 
                        rel="noopener noreferrer"
                        class="demo-link"
                    >
                        {"▶️ See Demo"}
                    </a>
                </div>

            </section>

            // Features Section: Practical Tools
            <section class="features">
                <h2>{"Essential Tools, Minimal Distractions"}</h2>
                <p>{"Access what you need with lightfriend’s AI-powered voice calls and SMS, keeping your dumbphone simple but functional."}</p>
                <div class="features-grid">
                    <div class="feature-item">
                        <i class="calendar-icon"></i>
                        <h3>{"Calendar Access"}</h3>
                        <p>{"Check and manage your schedule when needed."}</p>
                    </div>
                    <div class="feature-item">
                        <i class="email-icon"></i>
                        <h3>{"Email Access"}</h3>
                        <p>{"Check and manage important emails via voice or text when needed."}</p>
                    </div>
                    <div class="feature-item">
                        <i class="message-icon"></i>
                        <h3>{"Smart Messaging"}</h3>
                        <p>{"Access key messages across platforms like Telegram and WhatsApp."}</p>
                    </div>
                    <div class="feature-item">
                        <i class="search-icon"></i>
                        <h3>{"Real-Time Internet Search"}</h3>
                        <p>{"Ask questions or search the web via voice or text for quick answers from Perplexity."}</p>
                    </div>
                    <div class="feature-item">
                        <i class="weather-icon"></i>
                        <h3>{"Weather Updates"}</h3>
                        <p>{"Get current weather info for your location with a simple call or text."}</p>
                    </div>
                    <div class="feature-item highlight">
                        <i class="music-icon"></i>
                        <h3>{"Shazam Integration"}</h3>
                        <p>{"Identify songs with a simple call."}</p>
                    </div>
                </div>
            </section>


            <section class="shazam-showcase">
                <h2>{"Identify Any Song, Anywhere"}</h2>
                <div class="showcase-content">
                    <div class="showcase-text">
                        <h3>{"Simple Song Recognition:"}</h3>
                        <ol>
                            <li>{"Text or call lightfriend saying 'What song is this?' or simply 'shazam'"}</li>
                            <li>{"lightfriend calls you back ready to listen"}</li>
                            <li>{"Hold your phone near the music"}</li>
                            <li>{"lightfriend sends you the song details by text"}</li>
                        </ol>
                        <p class="showcase-highlight">
                            {"No app needed - just your basic phone and the music you want to identify!"}
                        </p>
                        <div class="video-demo">
                            <p>{"Watch it in action:"}</p>
                            <a 
                                href="https://youtube.com/shorts/4ZYnhtm9dkk" 
                                target="_blank" 
                                rel="noopener noreferrer"
                                class="demo-link"
                            >
                                {"▶️ See Shazam Demo"}
                            </a>
                        </div>
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
    let styles = stylist::Style::new(include_str!("home.css"))
        .expect("Failed to create Style");

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
                                    if !profile.verified && profile.time_to_delete {
                                        delete_unverified_account(profile.id, token);
                                        return;
                                    }
                                    
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
            <div class={styles}>
                <div class="dashboard-container">
                <div class="dashboard-panel">
                    <div class="panel-header">
                        <h1 class="panel-title">{"Dashboard"}</h1>
                    </div>
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
                                                                <div class="calendar-section">
                            {
                                if let Some(profile) = (*profile_data).as_ref() {
                                    html! {
                                        <Connect user_id={profile.id} />
                                    }
                                } else {
                                    html! {}
                                }
                            }
                        </div>

                        <div class="feature-status">
                            <h3>{"Currently Available"}</h3>
                            <h4>{"Tools"}</h4>
                            <ul>
                                <li>{"Perplexity AI search"}</li>
                                <li>{"Email and calendar integration"}</li>
                                <li>{"Dedicated Weather search"}</li>
                                <li>{"Send info to you by sms during voice calls"}</li>
                                <li>{"Shazam song recognition - Get a call, play the song, AI sends it to you by sms."}</li>
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
                                <li>{"Proactive messages(notifications for critical messages)"}</li>
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
                                        <div class="coming-soon">
                                            <h3>{"Coming Soon!"}</h3>
                                            <p>{"Proactive features will allow you to set up automatic notifications for important messages and events."}</p>
                                            <ul>
                                                <li>{"Message priority filtering"}</li>
                                                <li>{"Custom notification rules"}</li>
                                                <li>{"Time-based alerts"}</li>
                                                <li>{"Emergency contact notifications"}</li>
                                            </ul>
                                        </div>

                                        {render_notification_settings((*profile_data).as_ref())}
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
            </div>
        </div>
        }
    }
}

