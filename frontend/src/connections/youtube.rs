use yew::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::MouseEvent;
use crate::utils::api::Api;

#[derive(Properties, PartialEq)]
pub struct YouTubeConnectProps {
    pub user_id: i32,
    pub sub_tier: Option<String>,
    pub discount: bool,
}

#[function_component(YouTubeConnect)]
pub fn youtube_connect(props: &YouTubeConnectProps) -> Html {
    let error = use_state(|| None::<String>);
    let connecting = use_state(|| false);
    let youtube_connected = use_state(|| false);
    let youtube_scope = use_state(|| "readonly".to_string());
    let downgrading = use_state(|| false);
    let youtube_available = use_state(|| false);

    // Check connection status on component mount
    {
        let youtube_connected = youtube_connected.clone();
        let youtube_scope = youtube_scope.clone();
        let youtube_available = youtube_available.clone();
        use_effect_with_deps(
            move |_| {
                let youtube_connected = youtube_connected.clone();
                let youtube_scope = youtube_scope.clone();
                let youtube_available = youtube_available.clone();
                spawn_local(async move {
                    let request = Api::get("/api/auth/youtube/status")
                        .send()
                        .await;
                    if let Ok(response) = request {
                        if response.ok() {
                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                // Check if YouTube is available for this user
                                let available = data.get("available").and_then(|v| v.as_bool()).unwrap_or(true);
                                youtube_available.set(available);

                                if let Some(connected) = data.get("connected").and_then(|v| v.as_bool()) {
                                    youtube_connected.set(connected);
                                }
                                if let Some(scope) = data.get("scope").and_then(|v| v.as_str()) {
                                    youtube_scope.set(scope.to_string());
                                }
                            }
                        }
                    }
                });
                || ()
            },
            (),
        );
    }

    // Don't render YouTube integration if not available for this user
    if !*youtube_available {
        return html! {};
    }

    let onclick_connect = {
        let connecting = connecting.clone();
        let error = error.clone();
        Callback::from(move |_: MouseEvent| {
            let connecting = connecting.clone();
            let error = error.clone();
            connecting.set(true);
            error.set(None);

            spawn_local(async move {
                let request = Api::get("/api/auth/youtube/login")
                    .send()
                    .await;
                match request {
                    Ok(response) => {
                        if (200..300).contains(&response.status()) {
                            match response.json::<serde_json::Value>().await {
                                Ok(data) => {
                                    if let Some(auth_url) = data.get("auth_url").and_then(|u| u.as_str()) {
                                        if let Some(window) = web_sys::window() {
                                            let _ = window.location().set_href(auth_url);
                                        }
                                    } else {
                                        error.set(Some("YouTube integration coming soon".to_string()));
                                    }
                                }
                                Err(_) => {
                                    error.set(Some("YouTube integration coming soon".to_string()));
                                }
                            }
                        } else {
                            error.set(Some("YouTube integration coming soon".to_string()));
                        }
                    }
                    Err(_) => {
                        error.set(Some("YouTube integration coming soon".to_string()));
                    }
                }
                connecting.set(false);
            });
        })
    };

    let onclick_disconnect = {
        let youtube_connected = youtube_connected.clone();
        let error = error.clone();
        Callback::from(move |_: MouseEvent| {
            let youtube_connected = youtube_connected.clone();
            let error = error.clone();

            spawn_local(async move {
                let request = Api::delete("/api/auth/youtube/connection")
                    .send()
                    .await;
                match request {
                    Ok(response) => {
                        if response.ok() {
                            youtube_connected.set(false);
                        } else {
                            error.set(Some("Failed to disconnect YouTube".to_string()));
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error: {}", e)));
                    }
                }
            });
        })
    };

    let onclick_downgrade = {
        let downgrading = downgrading.clone();
        let error = error.clone();
        Callback::from(move |_: MouseEvent| {
            let downgrading = downgrading.clone();
            let error = error.clone();
            downgrading.set(true);
            error.set(None);

            spawn_local(async move {
                let request = Api::get("/api/auth/youtube/downgrade")
                    .send()
                    .await;
                match request {
                    Ok(response) => {
                        if response.ok() {
                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                if let Some(auth_url) = data.get("auth_url").and_then(|u| u.as_str()) {
                                    if let Some(window) = web_sys::window() {
                                        let _ = window.location().set_href(auth_url);
                                    }
                                }
                            }
                        } else {
                            error.set(Some("Failed to initiate downgrade".to_string()));
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error: {}", e)));
                    }
                }
                downgrading.set(false);
            });
        })
    };

    html! {
        <div class="service-item">
            <div class="service-header">
                <div class="service-name">
                    <img src="https://upload.wikimedia.org/wikipedia/commons/0/09/YouTube_full-color_icon_%282017%29.svg" alt="YouTube" width="24" height="24"/>
                    {"YouTube"}
                </div>
                <button class="info-button" onclick={Callback::from(|_| {
                    if let Some(element) = web_sys::window()
                        .and_then(|w| w.document())
                        .and_then(|d| d.get_element_by_id("youtube-info"))
                    {
                        let display = element.get_attribute("style")
                            .unwrap_or_else(|| "display: none".to_string());

                        if display.contains("none") {
                            let _ = element.set_attribute("style", "display: block");
                        } else {
                            let _ = element.set_attribute("style", "display: none");
                        }
                    }
                })}>
                    {"ⓘ"}
                </button>
                if *youtube_connected {
                    <span class="service-status">{"Connected ✓"}</span>
                }
            </div>
            <p class="service-description">
                {"View your YouTube subscriptions intentionally, without algorithmic recommendations or infinite scroll."}
            </p>
            <div id="youtube-info" class="info-section" style="display: none">
                <h4>{"Intentional Media Access"}</h4>
                <div class="info-subsection">
                    <h5>{"Planned Features"}</h5>
                    <ul>
                        <li>{"Subscription Feed: See recent uploads from channels you subscribe to (chronological order)"}</li>
                        <li>{"Search: Find specific videos without getting lost in recommendations"}</li>
                        <li>{"No Infinite Scroll: Limited results to prevent mindless browsing"}</li>
                        <li>{"No Autoplay: You choose what to watch, one video at a time"}</li>
                    </ul>
                </div>
                <div class="info-subsection">
                    <h5>{"Why This Exists"}</h5>
                    <p style="color: #CCC; margin: 0;">
                        {"Living with a dumbphone doesn't mean you can't watch YouTube. It means watching intentionally - coming with a purpose, finding what you need, and leaving without getting sucked into the algorithm."}
                    </p>
                </div>
                <div class="info-subsection security-notice">
                    <h5>{"Privacy"}</h5>
                    <p>{"Your YouTube data is protected through:"}</p>
                    <ul>
                        <li>{"OAuth 2.0: Secure authentication with Google"}</li>
                        <li>{"Read-Only: We only access your subscriptions, not post on your behalf"}</li>
                        <li>{"Revocable: Disconnect anytime through Lightfriend or Google Account settings"}</li>
                    </ul>
                </div>
            </div>
            if *youtube_connected {
                <div class="youtube-controls">
                    if *youtube_scope == "write" {
                        <div class="scope-info">
                            <span class="scope-badge write">{"Extended Access"}</span>
                            <p class="scope-description">
                                {"You have extended permissions enabled for subscribing and comments."}
                            </p>
                            <button
                                onclick={onclick_downgrade}
                                class="downgrade-button"
                                disabled={*downgrading}
                            >
                                if *downgrading {
                                    {"Revoking..."}
                                } else {
                                    {"Revoke Extended Access"}
                                }
                            </button>
                        </div>
                    }
                    <button
                        onclick={onclick_disconnect}
                        class="disconnect-button"
                    >
                        {"Disconnect YouTube"}
                    </button>
                </div>
            } else {
                if props.sub_tier.is_some() {
                    <button
                        onclick={onclick_connect}
                        class="connect-button"
                        disabled={*connecting}
                    >
                        if *connecting {
                            {"Connecting..."}
                        } else {
                            {"Connect YouTube"}
                        }
                    </button>
                } else {
                    <div class="upgrade-prompt">
                        <div class="upgrade-content">
                            <h3>{"Subscribe to Enable YouTube Integration"}</h3>
                            <a href="/pricing" class="upgrade-button">
                                {"View Pricing Plans"}
                            </a>
                        </div>
                    </div>
                }
            }
            if let Some(err) = (*error).as_ref() {
                <div class="error-message">
                    {err}
                </div>
            }
            <style>
                {r#"
                    .youtube-controls {
                        display: flex;
                        flex-direction: column;
                        gap: 1rem;
                        margin-top: 1rem;
                    }
                    .scope-info {
                        background: rgba(245, 158, 11, 0.1);
                        border: 1px solid rgba(245, 158, 11, 0.2);
                        border-radius: 8px;
                        padding: 1rem;
                    }
                    .scope-badge {
                        display: inline-block;
                        padding: 4px 8px;
                        border-radius: 4px;
                        font-size: 0.8rem;
                        font-weight: 500;
                    }
                    .scope-badge.write {
                        background: rgba(245, 158, 11, 0.2);
                        color: #f59e0b;
                    }
                    .scope-description {
                        color: #999;
                        font-size: 0.85rem;
                        margin: 0.5rem 0;
                    }
                    .downgrade-button {
                        background: transparent;
                        border: 1px solid rgba(245, 158, 11, 0.4);
                        color: #f59e0b;
                        padding: 8px 16px;
                        border-radius: 6px;
                        cursor: pointer;
                        font-size: 0.9rem;
                        transition: all 0.2s;
                    }
                    .downgrade-button:hover:not(:disabled) {
                        background: rgba(245, 158, 11, 0.1);
                    }
                    .downgrade-button:disabled {
                        opacity: 0.6;
                        cursor: not-allowed;
                    }
                "#}
            </style>
        </div>
    }
}
