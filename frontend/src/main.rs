use yew::prelude::*;
use yew_router::prelude::*;
use log::{info, Level};
use web_sys::{window, MouseEvent};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

mod config;
mod profile {
    pub mod stripe;
    pub mod billing_credits;
    pub mod billing_payments;
    pub mod billing_models;
    pub mod profile;
    pub mod settings;
    pub mod timezone_detector;
}
mod pages {
    pub mod home;
    pub mod landing;
    pub mod money;
    pub mod termsprivacy;
    pub mod proactive;
    pub mod faq;
    pub mod supported_countries;
    pub mod twilio_self_host_instructions;
    pub mod llm_self_host_instructions;
    pub mod voice_self_host_instructions;
    pub mod server_self_host_instructions;
    pub mod self_host_instructions;
}

mod proactive {
    pub mod email;
    pub mod calendar;
    pub mod common;
    pub mod waiting_checks;
    pub mod constant_monitoring;
    pub mod digest;
    pub mod critical;
}

mod connections {
    pub mod email;
    pub mod calendar;
    pub mod whatsapp;
    pub mod telegram;
    pub mod tasks;
}

/*
mod components {
    pub mod idea_widget;
}
*/

mod auth {
    pub mod connect;
    pub mod verify;
    pub mod signup;
    pub mod oauth_flow;
}
mod admin {
    pub mod dashboard;
    pub mod usage;
}

use pages::{
    home::Home,
    faq::Faq,
    supported_countries::SupportedCountries,
    home::is_logged_in,
    termsprivacy::{TermsAndConditions, PrivacyPolicy},
    money::{Pricing},
    self_host_instructions::SelfHostInstructions,
};

use auth::{
    signup::register::Register,
    signup::login::Login,
    signup::password_reset::PasswordReset,
    verify::Verify,
};

use profile::profile::Billing;
use admin::dashboard::AdminDashboard;
/*
use crate::components::idea_widget::IdeaWidget;
*/



#[derive(Clone, PartialEq)]
pub enum SelfHostingStatus {
    SelfHostedSignup,
    SelfHostedLogin,
    Normal,
}

#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/self-hosted")]
    SelfHosted,
    #[at("/password-reset")]
    PasswordReset,
    #[at("/faq")]
    Faq,
    #[at("/host-instructions")]
    SelfHostInstructions,
    #[at("/supported-countries")]
    SupportedCountries,
    #[at("/")]
    Home,
    #[at("/login")]
    Login,
    #[at("/register")]
    Register,
    #[at("/admin")]
    Admin,
    #[at("/billing")]
    Billing,
    #[at("/verify")]
    Verify,
    #[at("/terms")]
    Terms,
    #[at("/privacy")]
    Privacy,
    #[at("/pricing")]
    Pricing,
}


use crate::profile::billing_models::UserProfile;
use gloo_net::http::Request;

fn switch(routes: Route, self_hosting_status: &SelfHostingStatus, logged_in: bool) -> Html {

    // If in self-hosted mode, redirect to self-hosted page
    if matches!(self_hosting_status, SelfHostingStatus::SelfHostedSignup | SelfHostingStatus::SelfHostedLogin) {
        return match routes {
            Route::SelfHosted => {
                info!("Rendering Self Hosted page");
                html! { <Register self_hosting_status={self_hosting_status.clone()} /> }
            },
            _ => {
                html! { <Redirect<Route> to={Route::SelfHosted} /> }
            }
        };
    }

    match routes {
        Route::SelfHosted => {
            if !logged_in {
                info!("Rendering Self Hosted page");
                html! { <Register self_hosting_status={self_hosting_status.clone()} /> }
            } else {
                html! { <Redirect<Route> to={Route::Home} /> }
            }
        },
        Route::PasswordReset => {
            info!("Rendering Password Reset page");
            html! { <PasswordReset /> }
        },
        Route::Faq => {
            info!("Rendering FAQ page");
            html! { <Faq /> }
        },
        Route::SelfHostInstructions=> {
            info!("Rendering Self Host Instructions page");
            html! { <SelfHostInstructionsWrapper /> }
        },
        Route::SupportedCountries => {
            info!("Rendering SupportedCountries page");
            html! { <SupportedCountries/> }
        },
        Route::Home => {
            info!("Rendering Home page");
            html! { <Home /> }
        },
        Route::Login => {
            info!("Rendering Login page");
            html! { <Login /> }
        },
        Route::Register => {
            info!("Rendering Register page");
            html! { <Register /> }
        },
        Route::Admin => {
            info!("Rendering Admin page");
            html! { <AdminDashboard /> }
        },
        Route::Billing => {
            info!("Rendering Billing page");
            html! { <Billing /> }
        },
        Route::Verify => {
            info!("Rendering Verify page");
            html! { <Verify /> }
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
    }
}


#[function_component(SelfHostInstructionsWrapper)]
pub fn self_host_instructions_wrapper() -> Html {
    let profile_data = use_state(|| None::<UserProfile>);
    
    {
        let profile_data = profile_data.clone();
        
        use_effect_with_deps(move |_| {
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
                            if let Ok(profile) = response.json::<UserProfile>().await {
                                profile_data.set(Some(profile));
                            }
                        }
                        Err(_) => {}
                    }
                }
            });
            
            || ()
        }, ());
    }

    if let Some(profile) = (*profile_data).as_ref() {
        html! {
            <SelfHostInstructions
                is_logged_in={true}
                sub_tier={profile.sub_tier.clone()}
                server_ip={profile.server_ip.clone()}
                twilio_phone={profile.preferred_number.clone()}
                twilio_sid={profile.twilio_sid.clone()}
                twilio_token={profile.twilio_token.clone()}
                openrouter_api_key={profile.openrouter_api_key.clone()}
            />
        }
    } else {
        html! {
            <SelfHostInstructions
                is_logged_in={false}
                sub_tier={None::<String>}
                server_ip={None::<String>}
                twilio_phone={None::<String>}
                twilio_sid={None::<String>}
                twilio_token={None::<String>}
                openrouter_api_key={None::<String>}
            />
        }
    }
}

#[function_component(PricingWrapper)]
pub fn pricing_wrapper() -> Html {
    let profile_data = use_state(|| None::<UserProfile>);
    
    {
        let profile_data = profile_data.clone();
        
        use_effect_with_deps(move |_| {
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
                            if let Ok(profile) = response.json::<UserProfile>().await {
                                profile_data.set(Some(profile));
                            }
                        }
                        Err(_) => {}
                    }
                }
            });
            
            || ()
        }, ());
    }

    if let Some(profile) = (*profile_data).as_ref() {
        html! {
            <Pricing
                user_id={profile.id}
                user_email={profile.email.clone()}
                sub_tier={profile.sub_tier.clone()}
                is_logged_in={true}
                phone_number={profile.phone_number.clone()}
                verified={profile.verified.clone()}
            />
        }
    } else {
        html! {
            <Pricing
                user_id={0}
                user_email={"".to_string()}
                sub_tier={None::<String>}
                is_logged_in={false}
                phone_number={None::<String>}
                verified=false
            />
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct NavProps {
    pub logged_in: bool,
    pub on_logout: Callback<()>,
    pub self_hosting_status: SelfHostingStatus,
}

#[function_component(Nav)]
pub fn nav(props: &NavProps) -> Html {
    let NavProps { logged_in, on_logout, self_hosting_status } = props;
    let menu_open = use_state(|| false);
    let is_scrolled = use_state(|| false);

    {
        let is_scrolled = is_scrolled.clone();
        use_effect_with_deps(move |_| {
            let window = web_sys::window().unwrap();
            let document = window.document().unwrap();
            
            let scroll_callback = Closure::wrap(Box::new(move || {
                let scroll_top = document.document_element().unwrap().scroll_top();
                is_scrolled.set(scroll_top > 2500); // Increased threshold to match hero image height
            }) as Box<dyn FnMut()>);
            
            window.add_event_listener_with_callback("scroll", scroll_callback.as_ref().unchecked_ref())
                .unwrap();
            
            move || {
                window.remove_event_listener_with_callback("scroll", scroll_callback.as_ref().unchecked_ref())
                    .unwrap();
            }
        }, ());
    }
    
    let handle_logout = {
        let on_logout = on_logout.clone();
        Callback::from(move |_| {
            on_logout.emit(());
        })
    };

let toggle_menu = {
    let menu_open = menu_open.clone();
    Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        menu_open.set(!*menu_open);
    })
};

let close_menu = {
    let menu_open = menu_open.clone();
    Callback::from(move |_: MouseEvent| {
        menu_open.set(false);
    })
};

    let menu_class = if *menu_open {
        "nav-right mobile-menu-open"
    } else {
        "nav-right"
    };

    let close_class = if *menu_open {
        "burger-menu close-burger-menu"
    } else {
        "burger-menu"
    };

    html! {
        <nav class={classes!("top-nav", (*is_scrolled).then(|| "scrolled"))}>
            <div class="nav-content">
                <Link<Route> to={Route::Home} classes="nav-logo">
                    {"lightfriend"}
                </Link<Route>>
                <button class={close_class} onclick={toggle_menu}>
                    <span></span>
                    <span></span>
                    <span></span>
                </button>
                <div class={menu_class}>
                    <button class="close-menu" onclick={close_menu.clone()}>{"✕"}</button>
                    {
                        if !matches!(self_hosting_status, SelfHostingStatus::SelfHostedSignup | SelfHostingStatus::SelfHostedLogin) {
                            html! {
                                <>
                                    <div onclick={close_menu.clone()}>
                                        <Link<Route> to={Route::Faq} classes="nav-link">
                                            {"FAQ"}
                                        </Link<Route>>
                                    </div>
                                    <div onclick={close_menu.clone()}>
                                        <Link<Route> to={Route::Pricing} classes="nav-link">
                                            {"Pricing"}
                                        </Link<Route>>
                                    </div>
                                </>
                            }
                        } else {
                            html! {}
                        }
                    }
                    {
                        if *logged_in {
                            html! {
                                <>
                                    {
                                        if !matches!(self_hosting_status, SelfHostingStatus::SelfHostedSignup | SelfHostingStatus::SelfHostedLogin) {
                                            html! {
                                                <div onclick={close_menu.clone()}>
                                                    <Link<Route> to={Route::Billing} classes="nav-profile-link">
                                                        {"Billing"}
                                                    </Link<Route>>
                                                </div>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                    <button onclick={
                                        let close = close_menu.clone();
                                        let logout = handle_logout.clone();
                                        Callback::from(move |e: MouseEvent| {
                                            close.emit(e);
                                            logout.emit(());
                                        })
                                    } class="nav-logout-button">
                                        {"Logout"}
                                    </button>
                                </>
                            }
                        } else {
                            html! {
                                <div onclick={close_menu.clone()}>
                                    <Link<Route> to={Route::Login} classes="nav-login-button">
                                        {"Login"}
                                    </Link<Route>>
                                </div>
                            }
                        }
                    }
                </div>
            </div>
        </nav>
    }
}

#[function_component]
fn App() -> Html {
    let logged_in = use_state(|| is_logged_in());  // Import is_logged_in from home module
    let self_hosting_status = use_state(|| SelfHostingStatus::Normal);

    {
        let self_hosting_status = self_hosting_status.clone();
        use_effect_with_deps(move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                info!("Fetching self-hosting status...");
                if let Ok(response) = Request::get(&format!("{}/api/self-hosting-status", config::get_backend_url()))
                    .send()
                    .await
                {
                    if let Ok(status) = response.text().await {
                        info!("Received self-hosting status: {}", status);
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&status) {
                            if let Some(status_value) = json.get("status").and_then(|s| s.as_str()) {
                                match status_value {
                                    "self-hosted-signup" => {
                                        info!("Setting status to SelfHostedSignup");
                                        self_hosting_status.set(SelfHostingStatus::SelfHostedSignup)
                                    },
                                    "self-hosted-login" => {
                                        info!("Setting status to SelfHostedLogin");
                                        self_hosting_status.set(SelfHostingStatus::SelfHostedLogin)
                                    },
                                    _ => {
                                        info!("Setting status to Normal");
                                        self_hosting_status.set(SelfHostingStatus::Normal)
                                    },
                                }
                            } else {
                                self_hosting_status.set(SelfHostingStatus::Normal)
                            }
                        } else {
                            self_hosting_status.set(SelfHostingStatus::Normal)
                        }
                    }
                } else {
                    info!("Failed to fetch self-hosting status");
                }
            });
            || ()
        }, ());
    }
    let handle_logout = {
        Callback::from(move |_| {
            if let Some(window) = window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    let _ = storage.remove_item("token");
                    // Reload the page to reflect the logged out state
                    let _ = window.location().reload();
                }
            }
        })
    };



    html! {
        <>
            <BrowserRouter>
                <Nav logged_in={*logged_in} on_logout={handle_logout} self_hosting_status={(*self_hosting_status).clone()} />
                <Switch<Route> render={move |routes| switch(routes, &self_hosting_status, *logged_in)} />
            </BrowserRouter>
        </>
    }
}


fn main() {
    // Initialize console error panic hook for better error messages
    console_error_panic_hook::set_once();
    
    // Initialize logging
    console_log::init_with_level(Level::Info).expect("error initializing log");
    
    info!("Starting application");
    yew::Renderer::<App>::new().render();
}

        
