use yew::prelude::*;
use yew_router::prelude::*;
use log::{info, Level};
use web_sys::MouseEvent;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

#[derive(Clone, Copy, PartialEq)]
pub enum AuthState {
    Checking,  // Initial - don't show Login or Logout
    LoggedIn,
    LoggedOut,
}
mod config;
mod utils {
    pub mod api;
    pub mod webauthn;
    pub mod elevenlabs_web;
}
mod profile {
    pub mod stripe;
    pub mod billing_credits;
    pub mod billing_models;
    pub mod profile;
    pub mod settings;
    pub mod timezone_detector;
    pub mod security;
}
mod blog {
    pub mod switch_to_dumbphone;
    pub mod read_books_accidentally;
}
mod pages {
    pub mod home;
    pub mod landing;
    pub mod money;
    pub mod termsprivacy;
    pub mod proactive;
    pub mod faq;
    pub mod supported_countries;
    pub mod setup_costs;
    pub mod bring_own_number;
    pub mod lightphone3_whatsapp_guide;
    pub mod blog;
    pub mod change_log;
    pub mod subscription_success;
}
mod components {
    pub mod notification;
    pub mod feature_preview;
}
mod dashboard {
    pub mod dashboard_view;
    pub mod chat_box;
    pub mod triage_indicator;
    pub mod next_scheduled;
    pub mod timeline_view;
    pub mod dashboard_footer;
    pub mod settings_panel;
    pub mod activity_panel;
    pub mod quiet_mode;
    pub mod media_panel;
    pub mod tesla_quick_panel;
    pub mod youtube_quick_panel;
}
mod proactive {
    pub mod common;
    pub mod waiting_checks;
    pub mod digest;
    pub mod critical;
    pub mod contact_profiles;
}
mod connections {
    pub mod bridge_connect;
    pub mod email;
    pub mod calendar;
    pub mod whatsapp;
    pub mod telegram;
    pub mod signal;
    pub mod uber;
    pub mod messenger;
    pub mod instagram;
    pub mod tesla;
    pub mod youtube;
    pub mod mcp;
}
mod controls {
    pub mod tesla_controls;
}
mod media {
    pub mod youtube_hub;
}
mod auth {
    pub mod connect;
    pub mod signup;
    pub mod set_password;
}
mod admin {
    pub mod dashboard;
}
use pages::{
    home::Home,
    faq::Faq,
    supported_countries::SupportedCountries,
    termsprivacy::{TermsAndConditions, PrivacyPolicy},
    money::UnifiedPricing,
    bring_own_number::TwilioHostedInstructions,
    lightphone3_whatsapp_guide::LightPhone3WhatsappGuide,
    blog::Blog,
    change_log::Changelog,
    subscription_success::SubscriptionSuccess,
};
use blog::{
    switch_to_dumbphone::SwitchToDumbphoneGuide,
    read_books_accidentally::ReadMoreAccidentallyGuide,
};
use auth::{
    signup::login::Login,
    signup::password_reset::{PasswordReset, PasswordResetWithToken},
    set_password::SetPassword,
};
use admin::dashboard::AdminDashboard;
use crate::profile::billing_models::UserProfile;
use crate::utils::api::Api;
use gloo_net::http::Request;

#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/password-reset")]
    PasswordReset,
    #[at("/password-reset/:token")]
    PasswordResetWithToken { token: String },
    #[at("/faq")]
    Faq,
    #[at("/blog")]
    Blog,
    #[at("/updates")]
    Changelog,
    #[at("/supported-countries")]
    SupportedCountries,
    #[at("/bring-own-number")]
    TwilioHostedInstructions,
    #[at("/")]
    Home,
    #[at("/login")]
    Login,
    #[at("/admin")]
    Admin,
    #[at("/billing")]
    Billing,
    #[at("/terms")]
    Terms,
    #[at("/privacy")]
    Privacy,
    #[at("/pricing")]
    Pricing,
    #[at("/light-phone-3-whatsapp-guide")]
    LightPhone3WhatsappGuide,
    #[at("/how-to-switch-to-dumbphone")]
    SwitchToDumbphoneGuide,
    #[at("/how-to-read-more-accidentally")]
    ReadMoreAccidentallyGuide,
    #[at("/set-password")]
    SetPassword,
    #[at("/set-password/:token")]
    SetPasswordWithToken { token: String },
    #[at("/subscription-success")]
    SubscriptionSuccess,
}
fn switch(routes: Route) -> Html {
    match routes {
        Route::PasswordReset => {
            info!("Rendering Password Reset page");
            html! { <PasswordReset /> }
        },
        Route::PasswordResetWithToken { token } => {
            info!("Rendering Password Reset page with token");
            html! { <PasswordResetWithToken token={token.clone()} /> }
        },
        Route::Faq => {
            info!("Rendering FAQ page");
            html! { <Faq /> }
        },
        Route::Blog => {
            info!("Rendering Blog page");
            html! { <Blog /> }
        },
        Route::Changelog => {
            info!("Rendering Changelog page");
            html! { <Changelog /> }
        },
        Route::SupportedCountries => {
            info!("Rendering SupportedCountries page");
            html! { <SupportedCountries/> }
        },
        Route::TwilioHostedInstructions => {
            info!("Rendering TwilioHostedInstructions page");
            html! { <TwilioHostedInstructionsWrapper /> }
        },
        Route::Home => {
            info!("Rendering Home page");
            html! { <Home /> }
        },
        Route::Login => {
            info!("Rendering Login page");
            html! { <Login /> }
        },
        Route::Admin => {
            info!("Rendering Admin page");
            html! { <AdminDashboard /> }
        },
        Route::Billing => {
            // Redirect to Home with billing tab parameter
            html! { <Redirect<Route> to={Route::Home} /> }
        },
        Route::Terms => {
            info!("Rendering Terms page");
            html! { <TermsAndConditions /> }
        },
        Route::Privacy => {
            info!("Rendering Privacy page");
            html! { <PrivacyPolicy /> }
        },
        Route::Pricing => {
            info!("Rendering Pricing page");
            html! { <PricingWrapper /> }
        },
        Route::LightPhone3WhatsappGuide => {
            info!("Rendering LightPhone3WhatsappGuide page");
            html! { <LightPhone3WhatsappGuide /> }
        },
        Route::SwitchToDumbphoneGuide => {
            info!("Rendering SwitchToDumbphoneGuide page");
            html! { <SwitchToDumbphoneGuide /> }
        },
        Route::ReadMoreAccidentallyGuide => {
            info!("Rendering ReadMoreAccidentallyGuide page");
            html! { <ReadMoreAccidentallyGuide /> }
        },
        Route::SetPassword => {
            info!("Rendering SetPassword page");
            html! { <SetPassword /> }
        },
        Route::SetPasswordWithToken { token } => {
            info!("Rendering SetPassword page with token");
            html! { <SetPassword token={Some(token)} /> }
        },
        Route::SubscriptionSuccess => {
            info!("Rendering SubscriptionSuccess page");
            html! { <SubscriptionSuccess /> }
        },
    }
}
#[function_component(TwilioHostedInstructionsWrapper)]
pub fn twilio_hosted_instructions_wrapper() -> Html {
    let profile_data = use_state(|| None::<UserProfile>);
   
    {
        let profile_data = profile_data.clone();

        use_effect_with_deps(move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                match Api::get("/api/profile")
                    .send()
                    .await
                {
                    Ok(response) => {
                        if let Ok(profile) = response.json::<UserProfile>().await {
                            profile_data.set(Some(profile));
                        }
                    }
                    Err(_) => {}
                }
            });

            || ()
        }, ());
    }
    if let Some(profile) = (*profile_data).as_ref() {
        html! {
            <TwilioHostedInstructions
                is_logged_in={true}
                sub_tier={profile.sub_tier.clone()}
                twilio_phone={profile.preferred_number.clone()}
                twilio_sid={profile.twilio_sid.clone()}
                twilio_token={profile.twilio_token.clone()}
                textbee_api_key={profile.textbee_api_key.clone()}
                textbee_device_id={profile.textbee_device_id.clone()}
                country={profile.sub_country.clone()}
            />
        }
    } else {
        html! {
            <TwilioHostedInstructions
                is_logged_in={false}
                sub_tier={None::<String>}
                twilio_phone={None::<String>}
                twilio_sid={None::<String>}
                twilio_token={None::<String>}
                textbee_api_key={None::<String>}
                textbee_device_id={None::<String>}
                country={None::<String>}
            />
        }
    }
}
use serde_json::Value;
use std::collections::HashMap;

#[function_component(PricingWrapper)]
pub fn pricing_wrapper() -> Html {
    let profile_data = use_state(|| None::<UserProfile>);
    let selected_country = use_state(|| "US".to_string());
    let country_name = use_state(|| String::new());
    let ip_country_name = use_state(|| String::new());
    let is_logged_in = use_state(|| false);

    let country_map: HashMap<String, String> = [
        // Local number countries
        ("US".to_string(), "United States".to_string()),
        ("CA".to_string(), "Canada".to_string()),
        ("FI".to_string(), "Finland".to_string()),
        ("NL".to_string(), "Netherlands".to_string()),
        ("UK".to_string(), "United Kingdom".to_string()),
        ("AU".to_string(), "Australia".to_string()),
        // Notification-only countries
        ("DE".to_string(), "Germany".to_string()),
        ("FR".to_string(), "France".to_string()),
        ("ES".to_string(), "Spain".to_string()),
        ("IT".to_string(), "Italy".to_string()),
        ("PT".to_string(), "Portugal".to_string()),
        ("BE".to_string(), "Belgium".to_string()),
        ("AT".to_string(), "Austria".to_string()),
        ("CH".to_string(), "Switzerland".to_string()),
        ("PL".to_string(), "Poland".to_string()),
        ("CZ".to_string(), "Czech Republic".to_string()),
        ("SE".to_string(), "Sweden".to_string()),
        ("DK".to_string(), "Denmark".to_string()),
        ("NO".to_string(), "Norway".to_string()),
        ("IE".to_string(), "Ireland".to_string()),
        ("NZ".to_string(), "New Zealand".to_string()),
        // Other
        ("Other".to_string(), "Other".to_string()),
    ].iter().cloned().collect();

    {
        let selected_country = selected_country.clone();
        let country_name = country_name.clone();
        let ip_country_name = ip_country_name.clone();
        let is_logged_in = is_logged_in.clone();
        let profile_data = profile_data.clone();
        let country_map = country_map.clone();

        use_effect_with_deps(move |_| {
            if let Some(window) = web_sys::window() {
                let _ = window.scroll_to_with_x_and_y(0.0, 0.0);
            }

            wasm_bindgen_futures::spawn_local(async move {
                let mut ip_code = "Other".to_string();
                let mut ip_name = "your country".to_string();

                if let Ok(resp) = Request::get("https://ipapi.co/json/").send().await {
                    if let Ok(json) = resp.json::<Value>().await {
                        if let Some(code) = json.get("country_code").and_then(|v| v.as_str()) {
                            ip_code = code.to_uppercase();
                        }
                        if let Some(name) = json.get("country_name").and_then(|v| v.as_str()) {
                            ip_name = name.to_string();
                        }
                    }
                }

                ip_country_name.set(ip_name.clone());

                // Local number countries + notification-only countries
                let known_countries = [
                    // Local number countries
                    "US", "CA", "FI", "NL", "UK", "AU",
                    // Notification-only countries (receive SMS from US number)
                    "DE", "FR", "ES", "IT", "PT", "BE", "AT", "CH", "PL", "CZ", "SE", "DK", "NO", "IE", "NZ"
                ];
                if !known_countries.contains(&ip_code.as_str()) {
                    ip_code = "Other".to_string();
                }

                selected_country.set(ip_code.clone());
                country_name.set(if ip_code == "Other" { ip_name.clone() } else { country_map.get(&ip_code).cloned().unwrap_or(ip_name.clone()) });

                // Try to get profile with cookie-based auth
                if let Ok(response) = Api::get("/api/profile")
                    .send()
                    .await
                {
                    if let Ok(profile) = response.json::<UserProfile>().await {
                        profile_data.set(Some(profile.clone()));
                        is_logged_in.set(true);
                        // Country is determined from IP geolocation (set earlier in this effect)
                    } else {
                        is_logged_in.set(false);
                    }
                } else {
                    is_logged_in.set(false);
                }
            });

            || ()
        }, ());
    }

    let on_country_change = if !*is_logged_in {
        let selected_country = selected_country.clone();
        let country_name = country_name.clone();
        let ip_country_name = ip_country_name.clone();
        let country_map = country_map.clone();
        Some(Callback::from(move |e: Event| {
            if let Some(target) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                let value = target.value();
                selected_country.set(value.clone());
                let new_name = if value == "Other" {
                    (*ip_country_name).clone()
                } else {
                    country_map.get(&value).cloned().unwrap_or(value.clone())
                };
                country_name.set(new_name);
            }
        }))
    } else {
        None
    };

    html! {
        <UnifiedPricing
            user_id={profile_data.as_ref().map(|p| p.id).unwrap_or(0)}
            user_email={profile_data.as_ref().map(|p| p.email.clone()).unwrap_or("".to_string())}
            sub_tier={profile_data.as_ref().and_then(|p| p.sub_tier.clone())}
            user_plan_type={profile_data.as_ref().and_then(|p| p.plan_type.clone())}
            is_logged_in={*is_logged_in}
            phone_number={profile_data.as_ref().and_then(|p| Some(p.phone_number.clone()))}
            verified={profile_data.as_ref().map(|p| p.verified).unwrap_or(false)}
            selected_country={(*selected_country).clone()}
            country_name={(*country_name).clone()}
            on_country_change={on_country_change}
        />
    }
}

#[derive(Properties, PartialEq)]
pub struct NavProps {
    pub auth_state: AuthState,
}
#[function_component(Nav)]
pub fn nav(props: &NavProps) -> Html {
    let NavProps { auth_state } = props;
    let is_scrolled = use_state(|| false);
    {
        let is_scrolled = is_scrolled.clone();
        use_effect_with_deps(move |_| {
            let window = web_sys::window().unwrap();
            let document = window.document().unwrap();

            let scroll_callback = Closure::wrap(Box::new(move || {
                let scroll_top = document.document_element().unwrap().scroll_top();
                is_scrolled.set(scroll_top > 2500);
            }) as Box<dyn FnMut()>);

            window.add_event_listener_with_callback("scroll", scroll_callback.as_ref().unchecked_ref())
                .unwrap();

            move || {
                window.remove_event_listener_with_callback("scroll", scroll_callback.as_ref().unchecked_ref())
                    .unwrap();
            }
        }, ());
    }
    html! {
        <nav class={classes!("top-nav", (*is_scrolled).then(|| "scrolled"))}>
            <div class="nav-content">
                <Link<Route> to={Route::Home} classes="nav-logo">
                    {"lightfriend"}
                </Link<Route>>
                <div class="nav-right">
                    {
                        match auth_state {
                            AuthState::LoggedOut => html! {
                                <>
                                    <Link<Route> to={Route::Pricing} classes="nav-link">
                                        {"Pricing"}
                                    </Link<Route>>
                                    <Link<Route> to={Route::Login} classes="nav-login-button">
                                        {"Login"}
                                    </Link<Route>>
                                </>
                            },
                            AuthState::LoggedIn => {
                                let onclick = Callback::from(|e: MouseEvent| {
                                    e.prevent_default();
                                    if let Some(window) = web_sys::window() {
                                        let event = web_sys::CustomEvent::new("open-settings").unwrap();
                                        let _ = window.dispatch_event(&event);
                                    }
                                });
                                html! {
                                    <button {onclick} class="nav-link">
                                        {"Settings"}
                                    </button>
                                }
                            },
                            AuthState::Checking => html! {},
                        }
                    }
                </div>
            </div>
        </nav>
    }
}
#[function_component]
fn App() -> Html {
    let auth_state = use_state(|| AuthState::Checking); // Start in checking state
    let auth_check_started = use_state(|| false); // Track if auth check has started

    // Check authentication status with automatic token refresh
    {
        let auth_state = auth_state.clone();
        let auth_check_started = auth_check_started.clone();
        use_effect_with_deps(move |_| {
            // Only run if we haven't started the check yet
            if !*auth_check_started {
                auth_check_started.set(true);

                let auth_state = auth_state.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(response) = Api::get("/api/auth/status").send().await
                    {
                        if response.ok() {
                            auth_state.set(AuthState::LoggedIn);
                        } else {
                            auth_state.set(AuthState::LoggedOut);
                        }
                    } else {
                        auth_state.set(AuthState::LoggedOut);
                    }
                });
            }
            || ()
        }, ());
    }

    html! {
        <>
            <BrowserRouter>
                <Nav auth_state={*auth_state} />
                <Switch<Route> render={switch} />
            </BrowserRouter>
        </>
    }
}
fn main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(Level::Info).expect("error initializing log");
    info!("Starting application");
    yew::Renderer::<App>::new().render();
}
