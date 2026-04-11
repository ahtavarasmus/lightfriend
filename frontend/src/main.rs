use log::{info, Level};
use std::panic;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::MouseEvent;
use yew::prelude::*;
use yew_router::prelude::*;

#[derive(Clone, Copy, PartialEq)]
pub enum AuthState {
    Checking, // Initial - don't show Login or Logout
    LoggedIn,
    LoggedOut,
}
mod config;
mod utils {
    pub mod api;
    pub mod voice_web;
    pub mod seo;
    pub mod webauthn;
}
mod profile {
    pub mod billing_credits;
    pub mod billing_models;
    pub mod profile;
    pub mod security;
    pub mod settings;
    pub mod stripe;
    pub mod timezone_detector;
}
mod blog {
    pub mod read_books_accidentally;
    pub mod switch_to_dumbphone;
}
mod pages {
    pub mod blog;
    pub mod bring_own_number;
    pub mod home;
    pub mod landing;
    pub mod lightphone3_whatsapp_guide;
    pub mod money;
    pub mod prompt_injection_safe;
    pub mod signal_on_dumbphone;
    pub mod subscription_success;
    pub mod telegram_on_dumbphone;
    pub mod termsprivacy;
    pub mod trust_chain;
    pub mod trustless;
}
mod components {
    pub mod notification;
}
mod dashboard {
    pub mod activity_feed;
    pub mod chat_box;
    pub mod dashboard_view;
    pub mod emoji_utils;
    pub mod media_panel;
    pub mod people_list;
    pub mod rule_builder;
    pub mod rules_section;
    pub mod settings_panel;
    pub mod tesla_quick_panel;
    pub mod timeline_view;
    pub mod triage_indicator;
    pub mod youtube_quick_panel;
}
mod proactive {
    pub mod contact_profiles;
}
mod connections {
    pub mod bridge_connect;
    pub mod email;
    pub mod mcp;
    pub mod signal;
    pub mod telegram;
    pub mod tesla;
    pub mod whatsapp;
    pub mod youtube;
}
mod auth {
    pub mod connect;
    pub mod set_password;
    pub mod signup;
}
mod admin {
    pub mod dashboard;
}
use crate::profile::billing_models::UserProfile;
use crate::utils::api::Api;
use admin::dashboard::AdminDashboard;
use auth::{
    set_password::SetPassword,
    signup::login::Login,
    signup::password_reset::{PasswordReset, PasswordResetWithToken},
};
use blog::{
    read_books_accidentally::ReadMoreAccidentallyGuide, switch_to_dumbphone::SwitchToDumbphoneGuide,
};
use gloo_net::http::Request;
use pages::{
    blog::Blog,
    bring_own_number::TwilioHostedInstructions,
    home::Home,
    lightphone3_whatsapp_guide::LightPhone3WhatsappGuide,
    money::UnifiedPricing,
    prompt_injection_safe::PromptInjectionSafe,
    signal_on_dumbphone::SignalOnDumbphone,
    subscription_success::SubscriptionSuccess,
    telegram_on_dumbphone::TelegramOnDumbphone,
    termsprivacy::{PrivacyPolicy, TermsAndConditions},
    trust_chain::TrustChainPage,
    trustless::TrustlessVerification,
};

#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/password-reset")]
    PasswordReset,
    #[at("/password-reset/:token")]
    PasswordResetWithToken { token: String },
    #[at("/blog")]
    Blog,
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
    #[at("/trustless")]
    Trustless,
    #[at("/trust-chain")]
    TrustChain,
    #[at("/pricing")]
    Pricing,
    #[at("/light-phone-3-whatsapp-guide")]
    LightPhone3WhatsappGuide,
    #[at("/how-to-switch-to-dumbphone")]
    SwitchToDumbphoneGuide,
    #[at("/how-to-read-more-accidentally")]
    ReadMoreAccidentallyGuide,
    #[at("/telegram-on-dumbphone")]
    TelegramOnDumbphone,
    #[at("/signal-on-dumbphone")]
    SignalOnDumbphone,
    #[at("/prompt-injection-safe")]
    PromptInjectionSafe,
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
        }
        Route::PasswordResetWithToken { token } => {
            info!("Rendering Password Reset page with token");
            html! { <PasswordResetWithToken token={token.clone()} /> }
        }
        Route::Blog => {
            info!("Rendering Blog page");
            html! { <Blog /> }
        }
        Route::TwilioHostedInstructions => {
            info!("Rendering TwilioHostedInstructions page");
            html! { <TwilioHostedInstructionsWrapper /> }
        }
        Route::Home => {
            info!("Rendering Home page");
            html! { <Home /> }
        }
        Route::Login => {
            info!("Rendering Login page");
            html! { <Login /> }
        }
        Route::Admin => {
            info!("Rendering Admin page");
            html! { <AdminDashboard /> }
        }
        Route::Billing => {
            // Redirect to Home with billing tab parameter
            html! { <Redirect<Route> to={Route::Home} /> }
        }
        Route::Terms => {
            info!("Rendering Terms page");
            html! { <TermsAndConditions /> }
        }
        Route::Privacy => {
            info!("Rendering Privacy page");
            html! { <PrivacyPolicy /> }
        }
        Route::Trustless => {
            info!("Rendering Trustless page");
            html! { <TrustlessVerification /> }
        }
        Route::TrustChain => {
            info!("Rendering Trust Chain page");
            html! { <TrustChainPage /> }
        }
        Route::Pricing => {
            info!("Rendering Pricing page");
            html! { <PricingWrapper /> }
        }
        Route::LightPhone3WhatsappGuide => {
            info!("Rendering LightPhone3WhatsappGuide page");
            html! { <LightPhone3WhatsappGuide /> }
        }
        Route::SwitchToDumbphoneGuide => {
            info!("Rendering SwitchToDumbphoneGuide page");
            html! { <SwitchToDumbphoneGuide /> }
        }
        Route::ReadMoreAccidentallyGuide => {
            info!("Rendering ReadMoreAccidentallyGuide page");
            html! { <ReadMoreAccidentallyGuide /> }
        }
        Route::TelegramOnDumbphone => {
            info!("Rendering TelegramOnDumbphone page");
            html! { <TelegramOnDumbphone /> }
        }
        Route::SignalOnDumbphone => {
            info!("Rendering SignalOnDumbphone page");
            html! { <SignalOnDumbphone /> }
        }
        Route::PromptInjectionSafe => {
            info!("Rendering PromptInjectionSafe page");
            html! { <PromptInjectionSafe /> }
        }
        Route::SetPassword => {
            info!("Rendering SetPassword page");
            html! { <SetPassword /> }
        }
        Route::SetPasswordWithToken { token } => {
            info!("Rendering SetPassword page with token");
            html! { <SetPassword token={Some(token)} /> }
        }
        Route::SubscriptionSuccess => {
            info!("Rendering SubscriptionSuccess page");
            html! { <SubscriptionSuccess /> }
        }
    }
}
#[function_component(TwilioHostedInstructionsWrapper)]
pub fn twilio_hosted_instructions_wrapper() -> Html {
    let profile_data = use_state(|| None::<UserProfile>);

    {
        let profile_data = profile_data.clone();

        use_effect_with_deps(
            move |_| {
                wasm_bindgen_futures::spawn_local(async move {
                    match Api::get("/api/profile").send().await {
                        Ok(response) if response.ok() => {
                            if let Ok(profile) = response.json::<UserProfile>().await {
                                profile_data.set(Some(profile));
                            }
                        }
                        _ => {}
                    }
                });

                || ()
            },
            (),
        );
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
use crate::utils::seo::{use_seo, SeoMeta};
use serde_json::Value;
use std::collections::HashMap;

#[function_component(PricingWrapper)]
pub fn pricing_wrapper() -> Html {
    use_seo(SeoMeta {
        title: "Pricing \u{2013} Lightfriend AI Assistant for Dumbphones",
        description: "Lightfriend pricing plans. AI that watches your WhatsApp, Telegram, Signal, and email - only interrupts when it matters. Works with any phone including dumbphones. Privacy verifiable on blockchain.",
        canonical: "https://lightfriend.ai/pricing",
        og_type: "website",
    });
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
        ("GB".to_string(), "United Kingdom".to_string()),
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
        ("GR".to_string(), "Greece".to_string()),
        ("HU".to_string(), "Hungary".to_string()),
        ("RO".to_string(), "Romania".to_string()),
        ("SK".to_string(), "Slovakia".to_string()),
        ("BG".to_string(), "Bulgaria".to_string()),
        ("HR".to_string(), "Croatia".to_string()),
        ("SI".to_string(), "Slovenia".to_string()),
        ("LT".to_string(), "Lithuania".to_string()),
        ("LV".to_string(), "Latvia".to_string()),
        ("EE".to_string(), "Estonia".to_string()),
        ("LU".to_string(), "Luxembourg".to_string()),
        ("MT".to_string(), "Malta".to_string()),
        ("CY".to_string(), "Cyprus".to_string()),
        ("IS".to_string(), "Iceland".to_string()),
        ("JP".to_string(), "Japan".to_string()),
        ("KR".to_string(), "South Korea".to_string()),
        ("SG".to_string(), "Singapore".to_string()),
        ("HK".to_string(), "Hong Kong".to_string()),
        ("TW".to_string(), "Taiwan".to_string()),
        ("IL".to_string(), "Israel".to_string()),
        // Other
        ("Other".to_string(), "Other".to_string()),
    ]
    .iter()
    .cloned()
    .collect();

    {
        let selected_country = selected_country.clone();
        let country_name = country_name.clone();
        let ip_country_name = ip_country_name.clone();
        let is_logged_in = is_logged_in.clone();
        let profile_data = profile_data.clone();
        let country_map = country_map.clone();

        use_effect_with_deps(
            move |_| {
                if let Some(window) = web_sys::window() {
                    let _ = window.scroll_to_with_x_and_y(0.0, 0.0);
                }

                wasm_bindgen_futures::spawn_local(async move {
                    let mut ip_code = "Other".to_string();
                    let mut ip_name = "your country".to_string();

                    if let Ok(resp) = Request::get(&format!("{}/api/geo/country", crate::config::get_backend_url())).send().await {
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
                        "US", "CA", "FI", "NL", "GB", "AU",
                        // Notification-only countries (receive SMS from US number)
                        "DE", "FR", "ES", "IT", "PT", "BE", "AT", "CH", "PL", "CZ", "SE", "DK",
                        "NO", "IE", "NZ", "GR", "HU", "RO", "SK", "BG", "HR", "SI", "LT", "LV",
                        "EE", "LU", "MT", "CY", "IS", // Asia-Pacific
                        "JP", "KR", "SG", "HK", "TW", // Middle East
                        "IL",
                    ];
                    if !known_countries.contains(&ip_code.as_str()) {
                        ip_code = "Other".to_string();
                    }

                    selected_country.set(ip_code.clone());
                    country_name.set(if ip_code == "Other" {
                        ip_name.clone()
                    } else {
                        country_map
                            .get(&ip_code)
                            .cloned()
                            .unwrap_or(ip_name.clone())
                    });

                    // Try to get profile with cookie-based auth
                    if let Ok(response) = Api::get("/api/profile").send().await {
                        if response.ok() {
                            if let Ok(profile) = response.json::<UserProfile>().await {
                                profile_data.set(Some(profile.clone()));
                                is_logged_in.set(true);
                            } else {
                                is_logged_in.set(false);
                            }
                        } else {
                            is_logged_in.set(false);
                        }
                    } else {
                        is_logged_in.set(false);
                    }
                });

                || ()
            },
            (),
        );
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
    let route = use_route::<Route>();
    let is_pricing = matches!(route, Some(Route::Pricing));
    let is_scrolled = use_state(|| false);
    {
        let is_scrolled = is_scrolled.clone();
        use_effect_with_deps(
            move |_| {
                let window = web_sys::window().unwrap();
                let document = window.document().unwrap();

                let scroll_callback = Closure::wrap(Box::new(move || {
                    let scroll_top = document.document_element().unwrap().scroll_top();
                    is_scrolled.set(scroll_top > 2500);
                }) as Box<dyn FnMut()>);

                window
                    .add_event_listener_with_callback(
                        "scroll",
                        scroll_callback.as_ref().unchecked_ref(),
                    )
                    .unwrap();

                move || {
                    window
                        .remove_event_listener_with_callback(
                            "scroll",
                            scroll_callback.as_ref().unchecked_ref(),
                        )
                        .unwrap();
                }
            },
            (),
        );
    }
    html! {
        <nav class={classes!("top-nav", (*is_scrolled).then(|| "scrolled"), is_pricing.then(|| "nav-static"), (*auth_state == AuthState::LoggedOut).then(|| "nav-landing"))}>
            <div class="nav-content">
                <div class="nav-left">
                    <Link<Route> to={Route::Home} classes="nav-logo">
                        {"lightfriend"}
                    </Link<Route>>
                    if is_pricing {
                        <Link<Route> to={Route::Home} classes="nav-back-button">
                            <i class="fa-solid fa-arrow-left"></i>
                        </Link<Route>>
                    }
                </div>
                <div class="nav-right">
                    {
                        match auth_state {
                            AuthState::LoggedOut => html! {
                                <>
                                    <div class="nav-trust-badges">
                                        <a href="https://github.com/ahtavarasmus/lightfriend" target="_blank" rel="noopener noreferrer" class="nav-trust-badge">
                                            <i class="fa-brands fa-github"></i>
                                            <span>{"Open Source"}</span>
                                        </a>
                                        <Link<Route> to={Route::TrustChain} classes="nav-trust-badge">
                                            <i class="fa-solid fa-shield-halved"></i>
                                            <span>{"Verifiably Private"}</span>
                                        </Link<Route>>
                                    </div>
                                    if !is_pricing {
                                        <Link<Route> to={Route::Pricing} classes="nav-link">
                                            {"Pricing"}
                                        </Link<Route>>
                                    }
                                    <a href="mailto:support@lightfriend.ai" class="nav-link nav-support-link">
                                        {"Support"}
                                    </a>
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
                                    <>
                                        <Link<Route> to={Route::TrustChain} classes="nav-trust-icon">
                                            <i class="fa-solid fa-shield-halved"></i>
                                        </Link<Route>>
                                        <button {onclick} class="nav-link">
                                            {"Settings"}
                                        </button>
                                    </>
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
        use_effect_with_deps(
            move |_| {
                // Only run if we haven't started the check yet
                if !*auth_check_started {
                    auth_check_started.set(true);

                    let auth_state = auth_state.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Ok(response) = Api::get("/api/auth/status").send().await {
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
            },
            (),
        );
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
    panic::set_hook(Box::new(|panic_info| {
        console_error_panic_hook::hook(panic_info);
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                if let Some(body) = document.body() {
                    body.set_inner_html(
                        r#"<div style="min-height:100vh;display:flex;align-items:center;justify-content:center;padding:24px;background:#f5f1e8;color:#1f1a17;font-family:Georgia,serif;">
<div style="max-width:560px;text-align:center;">
<h1 style="margin-bottom:12px;font-size:32px;">Lightfriend crashed</h1>
<p style="margin-bottom:16px;font-size:18px;line-height:1.5;">The app hit an unexpected error. Reload the page and try again.</p>
<button onclick="window.location.reload()" style="padding:12px 18px;border:none;border-radius:999px;background:#1f1a17;color:#f5f1e8;cursor:pointer;font-size:16px;">Reload</button>
</div>
</div>"#,
                    );
                }
            }
        }
    }));
    console_log::init_with_level(Level::Info).expect("error initializing log");
    info!("Starting application");
    yew::Renderer::<App>::new().render();
}
