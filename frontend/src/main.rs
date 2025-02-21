use yew::prelude::*;
use yew_router::prelude::*;
use log::{info, Level};
use web_sys::window;
use wasm_bindgen::prelude::*;

mod config;
mod pages;
mod admin;
mod profile;
mod verify;
mod termsprivacy;
use pages::{
    home::Home,
    home::is_logged_in,
};
use termsprivacy::{TermsAndConditions, PrivacyPolicy, Pricing};
use verify::Verify;
use admin::Admin;
use profile::Profile;

mod auth_components;
use auth_components::{
    login::Login,
    register::Register,
};


#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Home,
    #[at("/login")]
    Login,
    #[at("/register")]
    Register,
    #[at("/admin")]
    Admin,
    #[at("/profile")]
    Profile,
    #[at("/verify")]
    Verify,
    #[at("/terms")]
    Terms,
    #[at("/privacy")]
    Privacy,
    #[at("/pricing")]
    Pricing,
}


fn switch(routes: Route) -> Html {
    match routes {
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
        }
        Route::Admin => {
            info!("Rendering Admin page");
            html! { <Admin /> }
        },
        Route::Profile => {
            info!("Rendering Profile page");
            html! { <Profile /> }
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
            html! { <Pricing /> }
        },
    }
}

// Create a new component called Nav.rs
use yew::prelude::*;
use yew_router::prelude::*;

#[derive(Properties, PartialEq)]
pub struct NavProps {
    pub logged_in: bool,
    pub on_logout: Callback<()>,
}

#[function_component(Nav)]
pub fn nav(props: &NavProps) -> Html {
    let NavProps { logged_in, on_logout } = props;
    
    let handle_logout = {
        let on_logout = on_logout.clone();
        Callback::from(move |_| {
            on_logout.emit(());
        })
    };

    html! {
        <nav class="top-nav">
            <div class="nav-content">
                <Link<Route> to={Route::Home} classes="nav-logo">
                    {"lightfriend"}
                </Link<Route>>
                
                <div class="nav-right">
                    <Link<Route> to={Route::Pricing} classes="nav-link">
                        {"Pricing"}
                    </Link<Route>>
                    {
                        if *logged_in {
                            html! {
                                <>
                                    <Link<Route> to={Route::Profile} classes="nav-profile-link">
                                        {"Profile"}
                                    </Link<Route>>
                                    <button onclick={handle_logout} class="nav-logout-button">
                                        {"Logout"}
                                    </button>
                                </>
                            }
                        } else {
                            html! {
                                <Link<Route> to={Route::Login} classes="nav-login-button">
                                    {"Login"}
                                </Link<Route>>
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
        <BrowserRouter>
            <Nav logged_in={*logged_in} on_logout={handle_logout} />
            <Switch<Route> render={switch} />
        </BrowserRouter>
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

        
