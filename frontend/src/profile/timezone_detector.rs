use yew::prelude::*;
use web_sys::window;
use gloo_net::http::Request;
use crate::config;
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen::JsValue;
use web_sys::js_sys;

#[derive(Properties, PartialEq, Clone)]
pub struct TimezoneDetectorProps {
    pub on_timezone_update: Option<Callback<String>>,
}

#[function_component]
pub fn TimezoneDetector(props: &TimezoneDetectorProps) -> Html {
    let timezone = use_state(String::default);

    {
        let timezone = timezone.clone();
        use_effect_with_deps(
            move |_| {
                if let Some(window) = window() {
                    let locales = js_sys::Array::new();
                    let options = js_sys::Object::new();

                    let tz = {
                        let resolved = js_sys::Intl::DateTimeFormat::new(&locales, &options)
                            .resolved_options();
                        js_sys::Reflect::get(&resolved, &"timeZone".into())
                            .map(|val| val.as_string().unwrap_or_else(|| String::from("UTC")))
                            .unwrap_or_else(|_| {
                                log::warn!("Failed to get timeZone property, defaulting to UTC");
                                String::from("UTC")
                            })
                    };

                    if *timezone != tz {
                        timezone.set(tz.clone());
                        if let Some(token) = window.local_storage().ok().flatten().and_then(|s| s.get_item("token").ok().flatten()) {
                            spawn_local(async move {
                                if let Err(e) = Request::post(&format!("{}/api/profile/timezone", config::get_backend_url()))
                                    .header("Authorization", &format!("Bearer {}", token))
                                    .json(&serde_json::json!({"timezone": tz}))
                                    .expect("Failed to build request")
                                    .send()
                                    .await {
                                        log::error!("Failed to send timezone update: {:?}", e);
                                    }
                            });
                        }
                    }
                }
                || ()
            },
            (), // First effect has no dependencies
        );
    }

    {
        let on_timezone_update = props.on_timezone_update.clone();
        let timezone_for_closure = timezone.clone(); // Clone for closure
        let timezone_for_deps = (*timezone).clone(); // Clone the String value for deps
        use_effect_with_deps(
            move |_| {
                if let Some(callback) = on_timezone_update {
                    callback.emit((*timezone_for_closure).clone());
                }
                || ()
            },
            timezone_for_deps, // Use the cloned String, not the UseStateHandle
        );
    }

    html! {}
}
