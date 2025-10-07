use yew::prelude::*;
use yew_router::prelude::*;
use log::{info, Level};
use web_sys::{window, MouseEvent};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
mod config;
mod profile {
    pub mod billing_credits;
    pub mod billing_payments;
    pub mod billing_models;
    pub mod profile;
    pub mod settings;
    pub mod timezone_detector;
}
mod pages {
    pub mod home;
    pub mod termsprivacy;
    pub mod proactive;
}
mod components {
    pub mod notification;
}
mod proactive {
    pub mod common;
    pub mod waiting_checks;
    pub mod constant_monitoring;
    pub mod digest;
    pub mod critical;
    pub mod agent_on;
}
mod connections {
    pub mod email;
    pub mod calendar;
    pub mod whatsapp;
    pub mod telegram;
    pub mod signal;
    pub mod tasks;
    pub mod uber;
    pub mod messenger;
    pub mod instagram;
}
mod auth {
    pub mod connect;
    pub mod signup;
    pub mod oauth_flow;
}
use pages::{
    home::Home,
    home::is_logged_in,
    termsprivacy::{TermsAndConditions, PrivacyPolicy},
};
use auth::{
    signup::login::Login,
};
use profile::profile::Billing;
#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Home,
    #[at("/login")]
    Login,
    #[at("/billing")]
    Billing,
    #[at("/terms")]
    Terms,
    #[at("/privacy")]
    Privacy,
}
fn switch(routes: Route, logged_in: bool) -> Html {
    if !logged_in {
        if routes == Route::Login {
            info!("Rendering Login page");
            html! { <Login /> }
        } else {
            html! { <Redirect<Route> to={Route::Login} /> }
        }
    } else {
        match routes {
            Route::Login => {
                html! { <Redirect<Route> to={Route::Home} /> }
            }
            Route::Home => {
                info!("Rendering Home page");
                html! { <Home /> }
            }
            Route::Billing => {
                info!("Rendering Billing page");
                html! { <Billing /> }
            }
            Route::Terms => {
                info!("Rendering Terms page");
                html! { <TermsAndConditions /> }
            }
            Route::Privacy => {
                info!("Rendering Privacy page");
                html! { <PrivacyPolicy /> }
            }
        }
    }
}
#[derive(Properties, PartialEq)]
pub struct NavProps {
    pub logged_in: bool,
    pub on_logout: Callback<()>,
}
#[function_component(Nav)]
pub fn nav(props: &NavProps) -> Html {
    let NavProps { logged_in, on_logout } = props;
    let menu_open = use_state(|| false);
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
                    <>
                    </>
                    {
                        if *logged_in {
                            html! {
                                <>
                                    <div onclick={close_menu.clone()}>
                                        <Link<Route> to={Route::Billing} classes="nav-profile-link">
                                            {"Billing"}
                                        </Link<Route>>
                                    </div>
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
                            html! {}
                        }
                    }
                </div>
            </div>
        </nav>
    }
}
#[function_component]
fn App() -> Html {
    let logged_in = use_state(|| is_logged_in());
    let handle_logout = {
        Callback::from(move |_| {
            if let Some(window) = window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    let _ = storage.remove_item("token");
                    let _ = window.location().reload();
                }
            }
        })
    };
    html! {
        <>
            <BrowserRouter>
                <Nav logged_in={*logged_in} on_logout={handle_logout} />
                <Switch<Route> render={move |routes| switch(routes, *logged_in)} />
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
