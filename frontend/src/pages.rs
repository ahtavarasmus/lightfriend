pub mod home {
    use yew::prelude::*;
    use yew_router::prelude::*;
    use crate::Route;
    use crate::config;
    use web_sys::window;
    use gloo_net::http::Request;
    use serde::Deserialize;

    #[derive(Deserialize, Clone)]
    struct UserProfile {
        id: i32,
        email: String,
        phone_number: String,
        nickname: Option<String>,
        verified: bool,
        time_to_live: i32,
        time_to_delete: bool,
        iq: i32,
        notify_credits: bool,
        local_phone_number: String,
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
                <section class="hero">
                    <h1>{"Smart Tools for Dumbphones"}</h1>
                    <p class="hero-subtitle">
                        {"Use your dumbphone smarter with AI-powered voice and text assistance."}
                    </p>
                    <button class="hero-cta">
                        <Link<Route> to={Route::Register} classes="forward-link">
                        {"Get Started"}
                        </Link<Route>>
                    </button>
                </section>

                // Features Section
                <section class="features">
                    <h2>{"Essential Tools, Minimal Distractions"}</h2>
                    <p>{"Access everything you need through your dumbphone, while staying focused and present."}</p>
                    
                    <div class="features-grid">
                        <div class="feature-item">
                            <i class="calendar-icon"></i>
                            <h3>{"Calendar Access"}</h3>
                            <p>{"Check and manage your schedule through simple voice calls or text messages."}</p>
                        </div>
                        
                        <div class="feature-item">
                            <i class="email-icon"></i>
                            <h3>{"Email Integration"}</h3>
                            <p>{"Stay on top of important emails without the constant notifications."}</p>
                        </div>

                        <div class="feature-item">
                            <i class="message-icon"></i>
                            <h3>{"Smart Messaging"}</h3>
                            <p>{"Access your messages across platforms through your dumbphone."}</p>
                        </div>

                        <div class="feature-item">
                            <i class="search-icon"></i>
                            <h3>{"Perplexity Search"}</h3>
                            <p>{"Get instant answers and information via SMS or voice call."}</p>
                        </div>
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

                <section class="footer-cta">
                    <div class="footer-content">
                        <h2>{"Ready to Reclaim Your Focus?"}</h2>
                        <p class="subtitle">
                            {"Join the digital minimalism movement without sacrificing essential connectivity."}
                        </p>
                        <button class="hero-cta">
                            <Link<Route> to={Route::Register} classes="forward-link">
                                {"Get Started Now"}
                            </Link<Route>>
                        </button>
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
                    </div>
                    </div>
                </section>
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

        // Polling effect
        {
            let profile_data = profile_data.clone();
            let user_verified = user_verified.clone();
            let error = error.clone();
            
            use_effect_with_deps(move |_| {
                let profile_data = profile_data.clone();
                let user_verified = user_verified.clone();
                let error = error.clone();

                // Create a handle to store the interval
                let interval_handle: std::rc::Rc<std::cell::RefCell<Option<gloo_timers::callback::Interval>>> = 
                std::rc::Rc::new(std::cell::RefCell::new(None));
                let interval_handle_clone = interval_handle.clone();

                // Function to fetch profile
                let fetch_profile = move || {
                    let profile_data = profile_data.clone();
                    let user_verified = user_verified.clone();
                    let error = error.clone();
let interval_handle = interval_handle.clone();

                    gloo_console::log!("Fetching profile...");
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
                                            gloo_console::log!("Profile fetched successfully:", format!("verified: {}", profile.verified));
                                            // Check if unverified profile has expired
                                            if !profile.verified && profile.time_to_delete {
                                                // Profile has expired, delete account and logout
                                                //delete_unverified_account(profile.id, token.clone());
                                                return;
                                            }
                                            
                                            // If user becomes verified, clear the interval
                                            if profile.verified {
                                                if let Some(interval) = interval_handle.borrow_mut().take() {
                                                    gloo_console::log!("User verified, stopping polling");
                                                    drop(interval); // This will stop the interval
                                                }
                                            }
                                            
                                            user_verified.set(profile.verified);
                                            profile_data.set(Some(profile));
                                            error.set(None);
                                        }
                                        Err(_) => {
                                            gloo_console::error!("Failed to parse profile data");
                                            error.set(Some("Failed to parse profile data".to_string()));
                                        }
                                    }
                                }
                                Err(_) => {
                                    gloo_console::error!("Failed to fetch profile");
                                    error.set(Some("Failed to fetch profile".to_string()));
                                }
                            }
                        }
                    });
                };

                // Initial fetch
                fetch_profile();

                
                // Set up interval for polling
                let interval = gloo_timers::callback::Interval::new(5000, move || {
                    fetch_profile();
                });

                // Store the interval in our handle
                *interval_handle_clone.borrow_mut() = Some(interval);

                move || {
                    // Clean up interval on component unmount
                    if let Some(interval) = interval_handle_clone.borrow_mut().take() {
                        drop(interval);
                    }
                }
            }, ());
        }

        if !logged_in {
            html! { <Landing /> }
        } else {
            if !*user_verified {
                html! {
                    <div class="verification-container">
                        <div class="verification-panel">
                            <h1>{"Verify Your Account"}</h1>
                            <p>{"Call the one of the following numbers to verify your account"}</p>
                                <div class="phone-number">{"us +18153684737"}</div >
                                <div class="phone-number">{"fin +358454901522"}</div>
                                <div class="phone-number">{"nl +3197006520696"}</div>
                            <div class="verification-status">
                                <i class="verification-icon"></i>
                                <span>{"Waiting for verification..."}</span>
                            </div>
                            <p class="instruction-text">
                                {"Want a local phone number to call? Please send me an email(rasmus@ahtava.com) or telegram(@ahtavarasmus)"}
                            </p>
                            <p class="verification-help">
                                <span>{"Having trouble? Make sure you typed your number correctly. You can change it in the profile."}</span>
                                <Link<Route> to={Route::Profile}>
                                    {"profile"}
                                </Link<Route>>

                            </p>
                        </div>
                    </div>
                }
            } else {
                html! {
                    <div class="dashboard-container">
                        <div class="dashboard-panel">
                            <div class="panel-header">
                                <h1 class="panel-title">{"Your lightfriend Dashboard"}</h1>
                            </div>
                            <div class="info-section">
                                <h2 class="section-title">{"Your lightfriend is Ready!"}</h2>
                                <div class="phone-display">
                                    <div class="phone-number">{"us +18153684737"}</div >
                                    <div class="phone-number">{"fin +358454901522"}</div>
                                    <div class="phone-number">{"nl +3197006520696"}</div>
                                </div>
                                <p class="instruction-text">
                                    {"Call these numbers to access your services."}
                                    <br/>
                                    <br/>
                                    {"If too expensive and want a another phone number to call please send me an email or message telegram(@ahtavarasmus) and we'll see what I can do."}
                                </p>
                                <div class="feature-status">
                                    <h3>{"Currently Available"}</h3>
                                    <ul>
                                        <li>{"Perplexity AI search through voice calls"}</li>
                                        <li>{"Adding more IQ(credits) for free in the profile(when you ran out)"}</li>
                                    </ul>
                                    
                                    <h3>{"Coming Soon"}</h3>
                                    <ul>
                                        <li>{"Purchase additional IQ(credits)"}</li>
                                        <li>{"Text messaging support"}</li>
                                        <li>{"WhatsApp and Telegram integration"}</li>
                                        <li>{"Reminder setting"}</li>
                                        <li>{"Email and calendar integration"}</li>
                                        <li>{"Camera functionality for photo translation and more"}</li>
                                    </ul>
                                    
                                    <p class="feature-suggestion">
                                        {"Have a feature in mind? Email your suggestions to "}
                                        <a href="mailto:rasmus@ahtava.com">{"rasmus@ahtava.com"}</a>
                                    </p>
                            </div>
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
                                </div>
                            </footer>
                        </div>
                    </div>
                </div>
                }
            }
        }
    }
}

