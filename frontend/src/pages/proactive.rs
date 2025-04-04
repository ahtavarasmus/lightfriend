use yew::prelude::*;
use yew::{Properties, function_component};
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use web_sys::window;
use crate::config;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ServiceStatus {
    gmail: bool,
    calendar: bool,
    imap: bool,
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub user_id: i32,
}

#[function_component(ConnectedServices)]
pub fn connected_services(props: &Props) -> Html {
    let service_status = use_state(|| ServiceStatus {
        gmail: false,
        calendar: false,
        imap: false,
    });
    let error = use_state(|| None::<String>);

    // Fetch service statuses
    {
        let service_status = service_status.clone();
        let error = error.clone();
        let user_id = props.user_id;

        use_effect_with_deps(move |_| {
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|storage| storage.get_item("token").ok())
                .flatten()
            {
                wasm_bindgen_futures::spawn_local(async move {
                    match Request::get(&format!("{}/api/profile/services/{}", config::get_backend_url(), user_id))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if let Ok(status) = response.json::<ServiceStatus>().await {
                                service_status.set(status);
                            } else {
                                error.set(Some("Failed to parse service status".to_string()));
                            }
                        }
                        Err(_) => {
                            error.set(Some("Failed to fetch service status".to_string()));
                        }
                    }
                });
            }
            || ()
        }, ());
    }

    html! {
        <div class="connected-services">
            <h2>{"Proactive messaging"}</h2>
            <div class="service-grid">

                <div class={classes!("service-box", if service_status.calendar { "connected" } else { "disconnected" })}>
                    <i class="service-icon calendar-icon"></i>
                    <h3>{"Google Calendar"}</h3>
                </div>

                <div class={classes!("service-box", if service_status.imap { "connected" } else { "disconnected" })}>
                    <i class="service-icon email-icon"></i>
                    <h3>{"IMAP Email"}</h3>
                </div>
            </div>
            // TODO make route handler that gets the connected services and then if some box is clicked, it should fetch the keywords and stuff and they should be editable. 

            if let Some(err) = (*error).as_ref() {
                <div class="error-message">
                    {err}
                </div>
            }
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct ProactiveProps {
    pub user_id: i32,
}

#[function_component(Proactive)]
pub fn proactive(props: &ProactiveProps) -> Html {
    // We don't need to fetch the user_id since it's passed as a prop
    let user_id = props.user_id;


    html! {
        <div class="proactive-page">
            <ConnectedServices user_id={user_id} />
        </div>
    }
}

