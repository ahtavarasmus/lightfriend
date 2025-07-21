use yew::prelude::*;
use web_sys::{MouseEvent, window};
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use serde_json::json;
use crate::config;

#[derive(Properties, PartialEq)]
pub struct ServerSelfHostInstructionsProps {
    #[prop_or_default]
    pub is_logged_in: bool,
    #[prop_or_default]
    pub sub_tier: Option<String>,
    #[prop_or_default]
    pub server_ip: Option<String>,
    #[prop_or_default]
    pub user_id: Option<String>,
    #[prop_or_default]
    pub message: String,
}

#[function_component(ServerSelfHostInstructions)]
pub fn server_self_host_instructions(props: &ServerSelfHostInstructionsProps) -> Html {
    let modal_visible = use_state(|| false);
    let selected_image = use_state(|| String::new());
    let server_ip = use_state(|| props.server_ip.clone().unwrap_or_default());
    let save_status = use_state(|| None::<Result<(), String>>);

    let on_input_change = {
        let server_ip = server_ip.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            server_ip.set(input.value());
        })
    };

    let on_save = {
        let server_ip = server_ip.clone();
        let save_status = save_status.clone();
        Callback::from(move |_| {
            let server_ip = server_ip.clone();
            let save_status = save_status.clone();
            
            save_status.set(None);
            
            spawn_local(async move {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|storage| storage.get_item("token").ok())
                    .flatten()
                {
                    let result = Request::post(&format!("{}/api/profile/server-ip", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .json(&json!({
                            "server_ip": *server_ip
                        }))
                        .unwrap()
                        .send()
                        .await;

                    match result {
                        Ok(response) => {
                            if response.status() == 401 {
                                // Token is invalid or expired
                                if let Some(window) = window() {
                                    if let Ok(Some(storage)) = window.local_storage() {
                                        let _ = storage.remove_item("token");
                                    }
                                }
                                save_status.set(Some(Err("Session expired. Please log in again.".to_string())));
                            } else if response.ok() {
                                save_status.set(Some(Ok(())));
                            } else {
                                save_status.set(Some(Err("Failed to save server IP".to_string())));
                            }
                        }
                        Err(_) => {
                            save_status.set(Some(Err("Network error occurred".to_string())));
                        }
                    }
                } else {
                    save_status.set(Some(Err("Please log in to save server IP".to_string())));
                }
            });
        })
    };

    let close_modal = {
        let modal_visible = modal_visible.clone();
        Callback::from(move |_: MouseEvent| {
            modal_visible.set(false);
        })
    };

    let open_modal = {
        let modal_visible = modal_visible.clone();
        let selected_image = selected_image.clone();
        Callback::from(move |src: String| {
            selected_image.set(src);
            modal_visible.set(true);
        })
    };

    let has_server_ip = props.server_ip.is_some();

    html! {
        <div class="instructions-page">
            <div class="instructions-background"></div>
            <section class="instructions-section">
                { if !props.message.is_empty() {
                    html! {
                        <div class="applicable-message">
                            { props.message.clone() }
                        </div>
                    }
                } else {
                    html! {}
                } }
                <div class="instruction-block overview-block">
                    <div class="instruction-content">
                        <h2>{"Setting Up Your Server on Hostinger"}</h2>
                        <p>{"Follow these steps to set up your own server for Lightfriend. No prerequisite knowledge is needed."}</p>
                    </div>
                </div>

                <div class="instruction-block">
                    <div class="instruction-content">
                        <h2>{"Expected Costs"}</h2>
                        <ul>
                            <li>{"Hostinger Server: $5-8/month (lowest tier is enough)"}</li>
                            <li>{"Cloudron License: Free for running 1 app"}</li>
                        </ul>
                    </div>
                </div>

                <div class="instruction-block">
                    <div class="instruction-content">
                        <h2>{"Choose Correct Server"}</h2>
                        <ul>
                            <li>{"Go to "}<a href="https://www.hostinger.com/pricing?content=vps-hosting" target="_blank" style="color: #1E90FF; text-decoration: underline;">{"Hostinger's pricing page"}</a></li>
                            <li>{"1. Choose your preferred length of subscription"}</li>
                            <li>{"2. Click on the KVM 1 Plan"}</li>
                        </ul>
                    </div>
                    <div class="instruction-image">
                        <img 
                            src="/assets/billing-hostinger.png" 
                            alt="Hostinger Pricing Page" 
                            loading="lazy"
                            onclick={let open_modal = open_modal.clone(); let src = "/assets/billing-hostinger.png".to_string();
                                Callback::from(move |_| open_modal.emit(src.clone()))}
                            style="cursor: pointer;"
                        />
                    </div>
                </div>

                <div class="instruction-block">
                    <div class="instruction-content">
                        <h2>{"Choose settings for the server and pay"}</h2>
                        <ul>
                            <li>{"1. Choose a server closest to you"}</li>
                            <li>{"2. With 'OS With Panel' selected choose Cloudron"}</li>
                            <li>{"3. Click 'Continue', make the payment and wait for 5 minutes for the server to build"}</li>
                        </ul>
                    </div>
                    <div class="instruction-images">
                        <div class="instruction-image">
                            <img 
                                src="/assets/server-settings-hostinger.png" 
                                alt="Server settings selection" 
                                loading="lazy"
                                onclick={let open_modal = open_modal.clone(); let src = "/assets/server-settings-hostinger.png".to_string(); 
                                    Callback::from(move |_| open_modal.emit(src.clone()))}
                                style="cursor: pointer;"
                            />
                        </div>
                        <div class="instruction-image">
                            <img 
                                src="/assets/cloudron-hostinger.png" 
                                alt="Cloudron Setup Page" 
                                loading="lazy"
                                onclick={let open_modal = open_modal.clone(); let src = "/assets/cloudron-hostinger.png".to_string(); 
                                    Callback::from(move |_| open_modal.emit(src.clone()))}
                                style="cursor: pointer;"
                            />
                        </div>
                    </div>
                </div>

                <div class="instruction-block">
                    <div class="instruction-content">
                        <h2>{"Copy your server's IP address"}</h2>
                        <ul>
                            <li>{"1. Once the server is ready, click on 'Manage VPS'"}</li>
                            <li>{"2. Then make sure 'Overview' is selected, scroll down and copy the IPV4 address"}</li>
                        </ul>
                        {
                            if props.is_logged_in {
                                html! {
                                    <div class="input-field">
                                        <label for="server-ip">{"Your Server's IP Address:"}</label>
                                        <div class="input-with-button">
                                            <input 
                                                type="text" 
                                                id="server-ip" 
                                                placeholder={if props.sub_tier.as_deref() == Some("tier 3") { "Enter your server's IP address".to_string() } else { "Managed by lightfriend.ai".to_string() }}
                                                value={if props.sub_tier.as_deref() == Some("tier 3") { (*server_ip).clone() } else { "Hosted on lightfriend.ai servers".to_string() }}
                                                onchange={if props.sub_tier.as_deref() == Some("tier 3") { on_input_change.clone() } else { Callback::noop() }}
                                                disabled={props.sub_tier.as_deref() != Some("tier 3")}
                                            />
                                            { if props.sub_tier.as_deref() == Some("tier 3") {
                                                html! {
                                                    <button 
                                                        class="save-button"
                                                        onclick={on_save.clone()}
                                                    >
                                                        {"Save"}
                                                    </button>
                                                }
                                            } else {
                                                html! {}
                                            } }
                                            {
                                                match &*save_status {
                                                    Some(Ok(_)) => html! {
                                                        <span class="save-status success">{"✓ Saved"}</span>
                                                    },
                                                    Some(Err(err)) => html! {
                                                        <span class="save-status error">{format!("Error: {}", err)}</span>
                                                    },
                                                    None => html! {}
                                                }
                                            }
                                        </div>
                                        { if props.sub_tier.as_deref() != Some("tier 3") {
                                            html! {
                                                <p class="note-text">{"These fields are filled in the hosted lightfriend.ai server."}</p>
                                            }
                                        } else {
                                            html! {}
                                        } }
                                    </div>
                                }
                            } else {
                                html! {}
                            }
                        }
                    </div>
                    <div class="instruction-images">
                        <div class="instruction-image">
                            <img 
                                src="/assets/server-ready-hostinger.png" 
                                alt="Manage VPS Page" 
                                loading="lazy"
                                onclick={let open_modal = open_modal.clone(); let src = "/assets/server-ready-hostinger.png".to_string(); 
                                    Callback::from(move |_| open_modal.emit(src.clone()))}
                                style="cursor: pointer;"
                            />
                        </div>
                        <div class="instruction-image">
                            <img 
                                src="/assets/server-ip-hostinger.png" 
                                alt="Server IP Location" 
                                loading="lazy"
                                onclick={let open_modal = open_modal.clone(); let src = "/assets/server-ip-hostinger.png".to_string(); 
                                    Callback::from(move |_| open_modal.emit(src.clone()))}
                                style="cursor: pointer;"
                            />
                        </div>
                    </div>
                </div>

                <div class={classes!("instruction-block", if !has_server_ip { "grayed-out" } else { "" })}>
                    <div class="instruction-content">
                        <h2>{"Your Personal Lightfriend Domain"}</h2>
                        <p class="highlight-text">
                            {
                                if let Some(user_id) = &props.user_id {
                                    format!("{}.lightfriend.ai", user_id)
                                } else {
                                    "Loading...".to_string()
                                }
                            }
                        </p>
                        <p class="info-text">
                            {"Your domain is being set up. This process typically takes 5-30 minutes for DNS propagation. Once complete, your domain will automatically route to your server."}
                        </p>
                        <p class="note-text">
                            {"Note: During this time, you can proceed with the next step to set up your Cloudron account."}
                        </p>
                    </div>
                </div>

                <div class={classes!("instruction-block", if !has_server_ip { "grayed-out" } else { "" })}>
                    <div class="instruction-content">
                        <h2>{"Set Up Your Cloudron Account"}</h2>
                        <ul>
                            <li>{"1. Open your server's IP address in a browser: "}{
                                if has_server_ip {
                                    let ip = props.server_ip.as_ref().unwrap().clone();
                                    html! {
                                        <a href={format!("http://{}", ip)} target="_blank" style="color: #1E90FF; text-decoration: underline;">{ip}</a>
                                    }
                                } else {
                                    html! {
                                        <span style="color: #1E90FF; text-decoration: underline;">{"<your_server_ip>"}</span>
                                    }
                                }
                            }</li>
                            <li>{"2. You'll see the Cloudron setup page"}</li>
                            <li>{"3. Create your admin account"}</li>
                            <li>{"4. Follow the setup wizard to complete the installation"}</li>
                        </ul>
                        <p class="note-text">
                            {"Once your domain is propagated, you can access Cloudron through your personal domain instead of the IP address."}
                        </p>
                    </div>
                </div>

            </section>

            {
                if *modal_visible {
                    html! {
                        <div class="modal-overlay" onclick={close_modal.clone()}>
                            <div class="modal-content" onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                                <img src={(*selected_image).clone()} alt="Large preview" />
                                <button class="modal-close" onclick={close_modal}>{"×"}</button>
                            </div>
                        </div>
                    }
                } else {
                    html! {}
                }
            }

            <style>
                {r#"
                .instructions-page {
                    padding-top: 74px;
                    min-height: 100vh;
                    color: #ffffff;
                    position: relative;
                    background: transparent;
                }

                .instructions-background {
                    position: fixed;
                    top: 0;
                    left: 0;
                    width: 100%;
                    height: 100vh;
                    background-image: url('/assets/bicycle_field.webp');
                    background-size: cover;
                    background-position: center;
                    background-repeat: no-repeat;
                    opacity: 1;
                    z-index: -2;
                    pointer-events: none;
                }

                .instructions-background::after {
                    content: '';
                    position: absolute;
                    bottom: 0;
                    left: 0;
                    width: 100%;
                    height: 50%;
                    background: linear-gradient(
                        to bottom, 
                        rgba(26, 26, 26, 0) 0%,
                        rgba(26, 26, 26, 1) 100%
                    );
                }

                .instructions-section {
                    max-width: 1200px;
                    margin: 0 auto;
                    padding: 2rem;
                }

                .instruction-block {
                    display: flex;
                    align-items: center;
                    gap: 4rem;
                    margin-bottom: 4rem;
                    background: rgba(26, 26, 26, 0.85);
                    backdrop-filter: blur(10px);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 12px;
                    padding: 4rem;
                    transition: all 0.3s ease;
                }

                .instruction-block:hover {
                    border-color: rgba(30, 144, 255, 0.3);
                }

                .instruction-block.grayed-out {
                    opacity: 0.5;
                    pointer-events: none;
                }

                .instruction-content {
                    flex: 1;
                    order: 1;
                }

                .instruction-image {
                    flex: 1;
                    order: 2;
                }

                .instruction-content h2 {
                    font-size: 2rem;
                    margin-bottom: 1.5rem;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                }

                .instruction-content ul {
                    list-style: none;
                    padding: 0;
                }

                .instruction-content li {
                    color: #999;
                    padding: 0.75rem 0;
                    padding-left: 1.5rem;
                    position: relative;
                    line-height: 1.6;
                }

                .instruction-content li::before {
                    content: '•';
                    position: absolute;
                    left: 0.5rem;
                    color: #1E90FF;
                }

                .instruction-image {
                    flex: 1.2;
                    display: flex;
                    justify-content: center;
                    align-items: center;
                }

                .instruction-image img {
                    max-width: 110%;
                    height: auto;
                    border-radius: 12px;
                    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
                    transition: transform 0.3s ease;
                }

                .instruction-image img:hover {
                    transform: scale(1.02);
                }

                .instruction-images {
                    display: flex;
                    flex-direction: column;
                    gap: 2rem;
                    flex: 1.2;
                    order: 2;
                }

                .instruction-images .instruction-image {
                    flex: 1;
                    width: 100%;
                }

                .instruction-images .instruction-image img {
                    width: 100%;
                    max-width: 100%;
                    height: auto;
                    border-radius: 12px;
                    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
                    transition: transform 0.3s ease;
                }

                .instruction-images .instruction-image img:hover {
                    transform: scale(1.02);
                }

                .input-field {
                    margin-top: 1.5rem;
                }

                .input-field label {
                    display: block;
                    margin-bottom: 0.5rem;
                    color: #7EB2FF;
                }

                .input-with-button {
                    display: flex;
                    gap: 0.5rem;
                }

                .input-with-button input {
                    flex: 1;
                    padding: 0.75rem;
                    border: 1px solid rgba(30, 144, 255, 0.3);
                    border-radius: 6px;
                    background: rgba(26, 26, 26, 0.5);
                    color: #fff;
                    font-size: 1rem;
                    transition: all 0.3s ease;
                }

                .input-with-button input:focus {
                    outline: none;
                    border-color: rgba(30, 144, 255, 0.8);
                    box-shadow: 0 0 0 2px rgba(30, 144, 255, 0.2);
                }

                .input-with-button input::placeholder {
                    color: rgba(255, 255, 255, 0.3);
                }

                .save-button {
                    padding: 0.75rem 1.5rem;
                    background: #1E90FF;
                    color: white;
                    border: none;
                    border-radius: 6px;
                    cursor: pointer;
                    font-size: 1rem;
                    transition: all 0.3s ease;
                }

                .save-button:hover {
                    background: #1976D2;
                }

                .save-button:active {
                    transform: translateY(1px);
                }

                .save-status {
                    margin-left: 1rem;
                    padding: 0.5rem 1rem;
                    border-radius: 4px;
                    font-size: 0.9rem;
                }

                .save-status.success {
                    color: #4CAF50;
                    background: rgba(76, 175, 80, 0.1);
                }

                .save-status.error {
                    color: #f44336;
                    background: rgba(244, 67, 54, 0.1);
                }

                .highlight-text {
                    font-size: 1.5rem;
                    color: #1E90FF;
                    padding: 1rem;
                    background: rgba(30, 144, 255, 0.1);
                    border-radius: 8px;
                    margin: 1rem 0;
                    text-align: center;
                }

                .info-text {
                    color: #999;
                    margin: 1rem 0;
                    line-height: 1.6;
                }

                .note-text {
                    color: #7EB2FF;
                    font-style: italic;
                    margin-top: 1rem;
                    padding-left: 1rem;
                    border-left: 3px solid rgba(126, 178, 255, 0.3);
                }

                .modal-overlay {
                    position: fixed;
                    top: 0;
                    left: 0;
                    width: 100%;
                    height: 100%;
                    background: rgba(0, 0, 0, 0.85);
                    display: flex;
                    justify-content: center;
                    align-items: center;
                    z-index: 1000;
                    backdrop-filter: blur(5px);
                }

                .modal-content {
                    position: relative;
                    max-width: 90%;
                    max-height: 90vh;
                    border-radius: 12px;
                    overflow: hidden;
                    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
                }

                .modal-content img {
                    display: block;
                    max-width: 100%;
                    max-height: 90vh;
                    object-fit: contain;
                }

                .modal-close {
                    position: absolute;
                    top: 10px;
                    right: 10px;
                    width: 40px;
                    height: 40px;
                    border-radius: 50%;
                    background: rgba(0, 0, 0, 0.5);
                    border: 2px solid rgba(255, 255, 255, 0.5);
                    color: white;
                    font-size: 24px;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    cursor: pointer;
                    transition: all 0.3s ease;
                }

                .modal-close:hover {
                    background: rgba(0, 0, 0, 0.8);
                    border-color: white;
                }

                @media (max-width: 968px) {
                    .instruction-block {
                        flex-direction: column;
                        gap: 2rem;
                    }

                    .instruction-content {
                        order: 1;
                    }

                    .instruction-image {
                        order: 2;
                    }

                    .instruction-content h2 {
                        font-size: 1.75rem;
                    }

                    .instructions-section {
                        padding: 1rem;
                    }
                }

                .applicable-message {
                    color: #ffcc00;
                    font-size: 1.2rem;
                    margin-bottom: 2rem;
                    text-align: center;
                    padding: 1rem;
                    background: rgba(255, 204, 0, 0.1);
                    border: 1px solid rgba(255, 204, 0, 0.3);
                    border-radius: 6px;
                }
                "#}
            </style>
        </div>
    }
}
