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
    pub mod seo;
    pub mod voice_web;
    pub mod webauthn;
}
mod profile {
    pub mod billing_credits;
    pub mod billing_models;
    pub mod commitment_dashboard;
    pub mod danger_zone;
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
    pub mod light_phone_panel;
    pub mod media_panel;
    pub mod people_list;
    pub mod rule_builder;
    pub mod rules_section;
    pub mod settings_panel;
    pub mod tesla_quick_panel;
    pub mod timeline_view;
    pub mod triage_indicator;
    pub mod webhooks_panel;
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
use pages::{
    blog::Blog,
    bring_own_number::TwilioHostedInstructions,
    home::Home,
    lightphone3_whatsapp_guide::LightPhone3WhatsappGuide,
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

#[function_component(PricingWrapper)]
pub fn pricing_wrapper() -> Html {
    use_seo(SeoMeta {
        title: "Pricing \u{2013} Lightfriend AI Assistant for Dumbphones",
        description: "Lightfriend pricing plans. AI that watches your WhatsApp, Telegram, Signal, and email and notifies you when something matters. Open source with cryptographically verifiable deployment measurements.",
        canonical: "https://lightfriend.ai/#plans",
        og_type: "website",
    });
    {
        use_effect_with_deps(
            move |_| {
                if let Some(window) = web_sys::window() {
                    let _ = window.location().set_href("/#plans");
                }
                || ()
            },
            (),
        );
    }

    html! {
        <main class="pricing-redirect">
            {"Redirecting to plans..."}
        </main>
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
    let is_public_landing = matches!(
        route,
        Some(
            Route::Home
                | Route::Login
                | Route::SetPassword
                | Route::SetPasswordWithToken { .. }
                | Route::SubscriptionSuccess
                | Route::PasswordReset
                | Route::PasswordResetWithToken { .. }
        )
    ) && *auth_state != AuthState::LoggedIn;
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
    if is_public_landing {
        return html! {
            <nav class="top-nav landing-mini-nav">
                <div class="landing-mini-nav-content">
                    <a href="/#plans" class="nav-link landing-mini-nav-link">
                        {"Pricing"}
                    </a>
                    <Link<Route> to={Route::Login} classes="nav-login-button landing-mini-login-button">
                        {"Login"}
                    </Link<Route>>
                </div>
            </nav>
        };
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
                                            <span>{"Private by Design"}</span>
                                        </Link<Route>>
                                    </div>
                                    if !is_pricing {
                                        <a href="/#plans" class="nav-link">
                                            {"Pricing"}
                                        </a>
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
