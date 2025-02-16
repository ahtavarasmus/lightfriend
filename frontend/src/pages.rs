pub mod home {
    use yew::prelude::*;
    use yew_router::prelude::*;
    use crate::Route;
    use crate::config;
    use web_sys::window;
    use gloo_net::http::Request;
    use serde::Deserialize;

    const PHONE_NUMBERS: &[(&str, &str)] = &[
        ("us", "+18153684737"),
        ("fin", "+358454901522"),
        ("nl", "+3197006520696"),
        ("cz", "+420910921902"),
    ];

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
        preferred_number: Option<String>,
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

    #[function_component(PrivacyPolicy)]
    pub fn privacy_policy() -> Html {
        html! {
            <div class="legal-content privacy-policy">
                <h1>{"lightfriend Privacy Policy"}</h1>

                <section>
                    <h2>{"1. Introduction"}</h2>
                    <p>{"This Privacy Policy outlines how lightfriend (\"we,\" \"our,\" or \"us\") collects, uses, and protects your personal information. By accessing or using our services, you agree to the terms described herein."}</p>
                </section>

                <section>
                    <h2>{"2. Information We Collect"}</h2>
                    <p>{"We may collect the following types of information:"}</p>
                    <ul>
                        <li>{"Personal Information: Such as your name, email address, and other contact details provided during account registration."}</li>
                        <li>{"Usage Data: Information about your interactions with our services, including access times, pages viewed, and other related metrics."}</li>
                        <li>{"Device Information: Details about the device you use to access our services, including hardware model, operating system, and unique device identifiers."}</li>
                    </ul>
                </section>

                <section>
                    <h2>{"3. How We Use Your Information"}</h2>
                    <p>{"The information we collect is used to:"}</p>
                    <ul>
                        <li>{"Provide and Improve Services: Enhance user experience and develop new features."}</li>
                        <li>{"Communication: Send updates, security alerts, and support messages."}</li>
                        <li>{"Analytics: Monitor and analyze usage to ensure platform stability and performance."}</li>
                    </ul>
                </section>

                <section>
                    <h2>{"4. Sharing Your Information"}</h2>
                    <p>{"We do not sell or rent your personal information. We may share data with:"}</p>
                    <ul>
                        <li>{"Service Providers: Trusted third parties that assist in operating our services, under strict confidentiality agreements."}</li>
                        <li>{"Legal Obligations: When required by law or to protect our rights and users."}</li>
                    </ul>
                </section>

                <section>
                    <h2>{"5. Data Security"}</h2>
                    <p>{"We implement industry-standard security measures to protect your data. However, no method of transmission over the internet is entirely secure, and we cannot guarantee absolute security."}</p>
                </section>

                <section>
                    <h2>{"6. Data Retention"}</h2>
                    <p>{"Personal data is retained only as long as necessary to fulfill the purposes outlined in this policy or as required by law."}</p>
                </section>

                <section>
                    <h2>{"7. Your Rights"}</h2>
                    <p>{"You have the right to:"}</p>
                    <ul>
                        <li>{"Access and Correction: Request access to your data and correct inaccuracies."}</li>
                        <li>{"Deletion: Request the deletion of your data, subject to certain conditions."}</li>
                        <li>{"Opt-Out: Unsubscribe from non-essential communications."}</li>
                    </ul>
                </section>

                <section>
                    <h2>{"8. Children's Privacy"}</h2>
                    <p>{"Our services are not intended for individuals under the age of 13. We do not knowingly collect personal information from children under 13. If we become aware of such data, we will take steps to delete it."}</p>
                </section>

                <section>
                    <h2>{"9. Changes to This Policy"}</h2>
                    <p>{"We may update this policy periodically. Significant changes will be communicated via email or through our platform."}</p>
                </section>

                <section>
                    <h2>{"10. Contact Us"}</h2>
                    <p>
                        {"For questions or concerns regarding this policy, please contact us at "}
                        <a href="mailto:rasmus@ahtava.com">{"rasmus@ahtava.com"}</a>
                    </p>
                </section>
            </div>
        }
    }

    #[function_component(TermsAndConditions)]
    pub fn terms_and_conditions() -> Html {
        html! {
            <div class="legal-content terms-and-conditions">
                <h1>{"lightfriend Terms and Conditions"}</h1>
    
                <section>
                    <h2>{"1. Introduction"}</h2>
                    <p>{"These Terms and Conditions (\"Terms\") govern your use of the lightfriend platform (\"Service\"). By accessing or using our Service, you agree to comply with and be bound by these Terms."}</p>
                </section>
    
                <section>
                    <h2>{"2. User Accounts"}</h2>
                    <p>{"To access certain features, you must create an account. You are responsible for maintaining the confidentiality of your account information and for all activities that occur under your account. lightfriend never has access to your passwords. Please do not provide your password to anyone claiming to be a representative of lightfriend, or any third party."}</p>
                </section>
    
                <section>
                    <h2>{"3. Acceptable Use"}</h2>
                    <p>{"You agree not to use the Service for any unlawful purpose or in any way that could harm the Service or impair anyone else's use of it."}</p>
                </section>
    
                <section>
                    <h2>{"4. Intellectual Property"}</h2>
                    <p>{"All content provided on the Service, including text, graphics, logos, and software, is the property of lightfriend or its content suppliers and is protected by intellectual property laws."}</p>
                </section>
    
                <section>
                    <h2>{"5. Termination"}</h2>
                    <p>{"We reserve the right to suspend or terminate your access to the Service at our discretion, without notice, for conduct that we believe violates these Terms or is harmful to other users."}</p>
                </section>
    
                <section>
                    <h2>{"6. Limitation of Liability"}</h2>
                    <p>{"The Service is provided \"as is\" without warranties of any kind. lightfriend will not be liable for any damages arising from the use or inability to use the Service."}</p>
                </section>
    
                <section>
                    <h2>{"7. Changes to Terms"}</h2>
                    <p>{"We may update these Terms from time to time. Continued use of the Service after any such changes constitutes your acceptance of the new Terms."}</p>
                </section>
    
                <section>
                    <h2>{"8. Governing Law"}</h2>
                    <p>{"These Terms are governed by and construed in accordance with the laws of the jurisdiction in which lightfriend operates, in Tampere, Finland."}</p>
                </section>
    
                <section>
                    <h2>{"9. Service Usage Policy"}</h2>
                    <p>{"By using our platform, you agree to use the AI-powered voice and text assistance services responsibly. lightfriend is designed to provide smart tools for dumbphone users, including calendar access, email integration, messaging services, and Perplexity search capabilities. Users are responsible for using these features in accordance with applicable laws and regulations. The service should not be used for any malicious or harmful purposes that could compromise the platform's integrity or other users' experience."}</p>
                </section>
    
                <section>
                    <h2>{"10. Contact Us"}</h2>
                    <p>
                        {"For questions or concerns regarding these Terms, please contact us at "}
                        <a href="mailto:rasmus@ahtava.com">{"rasmus@ahtava.com"}</a>
                        {". GDPR and other international compliances are coming soon."}
                    </p>
                </section>
            </div>
        }
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
                        <div class="legal-links">
                            <Link<Route> to={Route::Terms}>{"Terms & Conditions"}</Link<Route>>
                            {" | "}
                            <Link<Route> to={Route::Privacy}>{"Privacy Policy"}</Link<Route>>
                        </div>
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
        let is_expanded = use_state(|| false);
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
                    <div class="dashboard-container">
                        <div class="dashboard-panel">
                            <div class="panel-header">
                                <h1 class="panel-title">{"Dashboard"}</h1>
                            </div>
                            <div class="info-section">
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
                                            { PHONE_NUMBERS.iter().map(|(country, number)| {
                                                let number = number.to_string();
                                                let is_selected = if let Some(profile) = (*profile_data).as_ref() {
                                                    profile.preferred_number.as_ref().map_or(false, |pref| pref == &number)
                                                } else {
                                                    false
                                                };
                                                
                                                let onclick = {
                                                    let number = number.clone();
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
                                                            <span class="number">{number}</span>
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
                                    <div class="legal-links">
                                        <Link<Route> to={Route::Terms}>{"Terms & Conditions"}</Link<Route>>
                                        {" | "}
                                        <Link<Route> to={Route::Privacy}>{"Privacy Policy"}</Link<Route>>
                                    </div>
                                </div>
                            </footer>
                        </div>
                    </div>
                </div>
            }
        }
    }
}

