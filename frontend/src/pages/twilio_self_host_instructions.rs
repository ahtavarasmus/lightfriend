use yew::prelude::*;
use web_sys::{MouseEvent, window};
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use serde_json::json;
use crate::config;

#[derive(Properties, PartialEq)]
pub struct TwilioSelfHostInstructionsProps {
    #[prop_or_default]
    pub is_logged_in: bool,
    #[prop_or_default]
    pub sub_tier: Option<String>,
    #[prop_or_default]
    pub twilio_phone: Option<String>,
    #[prop_or_default]
    pub twilio_sid: Option<String>,
    #[prop_or_default]
    pub twilio_token: Option<String>,
    #[prop_or_default]
    pub textbee_api_key: Option<String>,
    #[prop_or_default]
    pub textbee_device_id: Option<String>,
    #[prop_or_default]
    pub textbee_phone_number: Option<String>,
    #[prop_or_default]
    pub message: String,
}

#[function_component(TwilioSelfHostInstructions)]
pub fn twilio_self_host_instructions(props: &TwilioSelfHostInstructionsProps) -> Html {
    let modal_visible = use_state(|| false);
    let selected_image = use_state(|| String::new());

    let phone_number = use_state(|| props.twilio_phone.clone().unwrap_or_default());
    let account_sid = use_state(|| props.twilio_sid.clone().unwrap_or_default());
    let auth_token = use_state(|| props.twilio_token.clone().unwrap_or_default());
    let textbee_api_key = use_state(|| props.textbee_api_key.clone().unwrap_or_default());
    let textbee_device_id = use_state(|| props.textbee_device_id.clone().unwrap_or_default());

    let phone_save_status = use_state(|| None::<Result<(), String>>);
    let creds_save_status = use_state(|| None::<Result<(), String>>);
    let textbee_save_status = use_state(|| None::<Result<(), String>>);

    {
        let phone_number = phone_number.clone();
        let account_sid = account_sid.clone();
        let auth_token = auth_token.clone();
        let textbee_api_key = textbee_api_key.clone();
        let textbee_device_id = textbee_device_id.clone();
        use_effect_with_deps(
            move |(new_phone, new_sid, new_token, new_textbee_key, new_textbee_id)| {
                if let Some(phone) = new_phone {
                    if phone != &*phone_number {
                        phone_number.set(phone.clone());
                    }
                }
                if let Some(sid) = new_sid {
                    if sid != &*account_sid {
                        account_sid.set(sid.clone());
                    }
                }
                if let Some(token) = new_token {
                    if token != &*auth_token {
                        auth_token.set(token.clone());
                    }
                }
                if let Some(key) = new_textbee_key {
                    if key != &*textbee_api_key {
                        textbee_api_key.set(key.clone());
                    }
                }
                if let Some(id) = new_textbee_id {
                    if id != &*textbee_device_id {
                        textbee_device_id.set(id.clone());
                    }
                }
                || {}
            },
            (
                props.twilio_phone.clone(),
                props.twilio_sid.clone(),
                props.twilio_token.clone(),
                props.textbee_api_key.clone(),
                props.textbee_device_id.clone(),
            ),
        );
    }

    let on_phone_change = {
        let phone_number = phone_number.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            phone_number.set(input.value());
        })
    };

    let on_sid_change = {
        let account_sid = account_sid.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            account_sid.set(input.value());
        })
    };

    let on_token_change = {
        let auth_token = auth_token.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            auth_token.set(input.value());
        })
    };

    let on_textbee_key_change = {
        let textbee_api_key = textbee_api_key.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            textbee_api_key.set(input.value());
        })
    };

    let on_textbee_id_change = {
        let textbee_device_id = textbee_device_id.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            textbee_device_id.set(input.value());
        })
    };

    let on_save_phone = {
        let phone_number = phone_number.clone();
        let phone_save_status = phone_save_status.clone();
        Callback::from(move |_| {
            let phone_number = phone_number.clone();
            let phone_save_status = phone_save_status.clone();
            
            let val = (*phone_number).clone();
            if val.is_empty() || !val.starts_with('+') || val.len() < 10 || !val[1..].chars().all(|c| c.is_ascii_digit()) || val.starts_with("...") {
                phone_save_status.set(Some(Err("Invalid phone number format".to_string())));
                return;
            }
            
            phone_save_status.set(None);
            
            spawn_local(async move {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|storage| storage.get_item("token").ok())
                    .flatten()
                {
                    let result = Request::post(&format!("{}/api/profile/twilio-phone", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .json(&json!({
                            "twilio_phone": *phone_number
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
                                phone_save_status.set(Some(Err("Session expired. Please log in again.".to_string())));
                            } else if response.ok() {
                                phone_save_status.set(Some(Ok(())));
                            } else {
                                phone_save_status.set(Some(Err("Failed to save Twilio phone".to_string())));
                            }
                        }
                        Err(_) => {
                            phone_save_status.set(Some(Err("Network error occurred".to_string())));
                        }
                    }
                } else {
                    phone_save_status.set(Some(Err("Please log in to save Twilio phone".to_string())));
                }
            });
        })
    };

    let on_save_creds = {
        let account_sid = account_sid.clone();
        let auth_token = auth_token.clone();
        let creds_save_status = creds_save_status.clone();
        Callback::from(move |_| {
            let account_sid = account_sid.clone();
            let auth_token = auth_token.clone();
            let creds_save_status = creds_save_status.clone();
            
            let sid_val = (*account_sid).clone();
            if sid_val.len() != 34 || !sid_val.starts_with("AC") || !sid_val[2..].chars().all(|c| c.is_ascii_hexdigit()) || sid_val.starts_with("...") {
                creds_save_status.set(Some(Err("Invalid Account SID format".to_string())));
                return;
            }
            
            let token_val = (*auth_token).clone();
            if token_val.len() != 32 || !token_val.chars().all(|c| c.is_ascii_hexdigit()) || token_val.starts_with("...") {
                creds_save_status.set(Some(Err("Invalid Auth Token format".to_string())));
                return;
            }
            
            creds_save_status.set(None);
            
            spawn_local(async move {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|storage| storage.get_item("token").ok())
                    .flatten()
                {
                    let result = Request::post(&format!("{}/api/profile/twilio-creds", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .json(&json!({
                            "account_sid": *account_sid,
                            "auth_token": *auth_token
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
                                creds_save_status.set(Some(Err("Session expired. Please log in again.".to_string())));
                            } else if response.ok() {
                                creds_save_status.set(Some(Ok(())));
                            } else {
                                creds_save_status.set(Some(Err("Failed to save Twilio credentials".to_string())));
                            }
                        }
                        Err(_) => {
                            creds_save_status.set(Some(Err("Network error occurred".to_string())));
                        }
                    }
                } else {
                    creds_save_status.set(Some(Err("Please log in to save Twilio credentials".to_string())));
                }
            });
        })
    };

    let on_save_textbee = {
        let textbee_api_key = textbee_api_key.clone();
        let textbee_device_id = textbee_device_id.clone();
        let textbee_save_status = textbee_save_status.clone();
        Callback::from(move |_| {
            let textbee_api_key = textbee_api_key.clone();
            let textbee_device_id = textbee_device_id.clone();
            let textbee_save_status = textbee_save_status.clone();
            
            let key_val = (*textbee_api_key).clone();
            if key_val.is_empty() || key_val.starts_with("...") {
                textbee_save_status.set(Some(Err("Invalid API Key".to_string())));
                return;
            }
            
            let id_val = (*textbee_device_id).clone();
            if id_val.is_empty() || id_val.starts_with("...") {
                textbee_save_status.set(Some(Err("Invalid Device ID".to_string())));
                return;
            }
            
            textbee_save_status.set(None);
            
            spawn_local(async move {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|storage| storage.get_item("token").ok())
                    .flatten()
                {
                    let result = Request::post(&format!("{}/api/profile/textbee-creds", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .json(&json!({
                            "textbee_api_key": *textbee_api_key,
                            "textbee_device_id": *textbee_device_id
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
                                textbee_save_status.set(Some(Err("Session expired. Please log in again.".to_string())));
                            } else if response.ok() {
                                textbee_save_status.set(Some(Ok(())));
                            } else {
                                textbee_save_status.set(Some(Err("Failed to save TextBee credentials".to_string())));
                            }
                        }
                        Err(_) => {
                            textbee_save_status.set(Some(Err("Network error occurred".to_string())));
                        }
                    }
                } else {
                    textbee_save_status.set(Some(Err("Please log in to save TextBee credentials".to_string())));
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

    let is_phone_valid = {
        let val = &*phone_number;
        !val.is_empty() && val.starts_with('+') && val.len() >= 10 && val[1..].chars().all(|c| c.is_ascii_digit()) && !val.starts_with("...")
    };

    let is_sid_valid = {
        let val = &*account_sid;
        val.len() == 34 && val.starts_with("AC") && val[2..].chars().all(|c| c.is_ascii_hexdigit()) && !val.starts_with("...")
    };

    let is_token_valid = {
        let val = &*auth_token;
        val.len() == 32 && val.chars().all(|c| c.is_ascii_hexdigit()) && !val.starts_with("...")
    };

    let is_textbee_key_valid = {
        let val = &*textbee_api_key;
        !val.is_empty() && !val.starts_with("...")
    };

    let is_textbee_id_valid = {
        let val = &*textbee_device_id;
        !val.is_empty() && !val.starts_with("...")
    };

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
                        <h2>{"SMS and Voice Communication Setup"}</h2>
                        <p>{"Lightfriend uses Twilio for SMS messaging and voice calls, giving your AI assistant the ability to communicate via a dedicated phone number. For SMS only, you can alternatively use TextBee with your own Android phone for cost-effective messaging. Setting up your own accounts ensures privacy and control over communications."}</p>
                    </div>
                </div>

                /*
                <div class="instruction-block">
                    <div class="instruction-content">
                        <h2>{"Alternative: TextBee for SMS Messaging"}</h2>
                        <p>{"If you have a spare Android phone (version 7.0+) with a secondary phone number lying around, connect it to Lightfriend via TextBee. This lets your AI send and receive texts using your existing phone plan, supporting up to 300 messages per month on the free tier at no extra cost beyond your carrier's standard SMS charges."}</p>
                        <p>{"Note: TextBee is for texting only. If you need phone calls, set up Twilio in addition."}</p>
                        <p>{"Note: TextBee does not support sending images or other media."}</p>
                        <p>{"TextBee also offers a Pro plan ($6.99/month currently) for higher limits (up to 5,000 messages/month) and additional features like multi-device support."}</p>
                        <h3>{"Setup Steps"}</h3>
                        <ul>
                            <li>{"Register for a free account at "}<a href="https://textbee.dev" target="_blank" style="color: #7EB2FF; text-decoration: underline;">{"textbee.dev"}</a>{" using email/password or Google."}</li>
                            <li>{"Download and install the TextBee app on your Android phone from "}<a href="https://dl.textbee.dev" target="_blank" style="color: #7EB2FF; text-decoration: underline;">{"dl.textbee.dev"}</a>{"."}</li>
                            <li>{"Grant SMS permissions in the app."}</li>
                            <li>{"Link the device:"}
                                <ul>
                                    <li>{"Recommended: In the dashboard, click 'Register Device', scan the QR code with the app."}</li>
                                    <li>{"Alternative: Generate an API key in the dashboard, enter it in the app."}</li>
                                </ul>
                            </li>
                            <li>{"Once linked, note your Device ID from the devices list in the dashboard."}</li>
                            <li>{"Generate or use your API Key from the dashboard."}</li>
                        </ul>
                        {
                            if props.is_logged_in && props.sub_tier.as_deref() == Some("tier 2") {
                                html! {
                                    <>

                                        <div class="input-field">
                                            <label for="textbee-device-id">{"Your TextBee Device ID:"}</label>
                                            <div class="input-with-button">
                                                <input 
                                                    type="text" 
                                                    id="textbee-device-id" 
                                                    placeholder="your_device_id_here" 
                                                    value={(*textbee_device_id).clone()}
                                                    onchange={on_textbee_id_change.clone()}
                                                />
                                            </div>
                                        </div>
                                        <div class="input-field">
                                            <label for="textbee-api-key">{"Your TextBee API Key:"}</label>
                                            <div class="input-with-button">
                                                <input 
                                                    type="text" 
                                                    id="textbee-api-key" 
                                                    placeholder="your_api_key_here" 
                                                    value={(*textbee_api_key).clone()}
                                                    onchange={on_textbee_key_change.clone()}
                                                />
                                            </div>
                                        </div>
                                        <button 
                                            class={classes!("save-button", if !(is_textbee_key_valid && is_textbee_id_valid) { "invalid" } else { "" })}
                                            onclick={on_save_textbee.clone()}
                                        >
                                            {"Save TextBee Credentials"}
                                        </button>
                                        {
                                            match &*textbee_save_status {
                                                Some(Ok(_)) => html! {
                                                    <span class="save-status success">{"✓ Saved"}</span>
                                                },
                                                Some(Err(err)) => html! {
                                                    <span class="save-status error">{format!("Error: {}", err)}</span>
                                                },
                                                None => html! {}
                                            }
                                        }
                                    </>
                                }
                            } else {
                                html! {}
                            }
                        }
                    </div>
                </div>
                */

                <div class="instruction-block">
                    <div class="instruction-content">
                        <h2>{"Twilio Expected Costs"}</h2>
                        <p>{"With typical usage (around 5 messages per day), expect to pay approximately $5+ per month. This includes a phone number (€1-15/month) and message costs. For details, visit "}<a href="https://www.twilio.com/en-us/sms/pricing/us" target="_blank" style="color: #7EB2FF; text-decoration: underline;">{"Twilio's pricing page"}</a>{"."}</p>
                    </div>
                </div>
                <div class="instruction-block">
                    <div class="instruction-content">
                        <h2>{"Twilio Sign up and Add Funds"}</h2>
                        <ul>
                            <li>{"Go to Twilio's website (twilio.com) and click 'Sign up'"}</li>
                            <li>{"Complete the registration process with your email and other required information"}</li>
                            <li>{"Once registered, you'll need to add funds to your account:"}</li>
                            <li>{"1. Click on 'Admin' in the top right"}</li>
                            <li>{"2. Select 'Account billing' from the dropdown"}</li>
                            <li>{"3. Click 'Add funds' on the new billing page that opens up and input desired amount (minimum usually $20)"}</li>
                            <li>{"After adding funds, your account will be ready to purchase a phone number"}</li>
                        </ul>
                    </div>
                    <div class="instruction-image">
                        <img 
                            src="/assets/billing-twilio.png" 
                            alt="Navigating to Twilio Billing Page" 
                            loading="lazy"
                            onclick={let open_modal = open_modal.clone(); let src = "/assets/billing-twilio.png".to_string(); 
                                Callback::from(move |_| open_modal.emit(src.clone()))}
                            style="cursor: pointer;"
                        />
                    </div>
                </div>

                <div class="instruction-block">
                    <div class="instruction-content">
                        <h2>{"Twilio Buy a phone number"}</h2>
                        <ul>
                            <li>{"1. On the Twilio Dashboard, click on the 'Phone Numbers' button in the left sidebar when 'Develop' is selected above."}</li>
                            <li>{"2. Click the 'Buy a number' button under the new sub menu"}</li>
                            <li>{"3. Use the country search box to select your desired country"}</li>
                            <li>{"4. (Optional) Use advanced search options to find specific number types"}</li>
                            <li>{"5. Check the capabilities column to ensure the number supports your needs (Voice, SMS, MMS, etc.)"}</li>
                            <li>{"6. Click the 'Buy' button next to your chosen number and follow the steps"}</li>
                        </ul>
                        {
                            if props.is_logged_in && props.sub_tier.as_deref() == Some("tier 2") {
                                html! {
                                    <div class="input-field">
                                        <label for="phone-number">{"Your Twilio Phone Number:"}</label>
                                        <div class="input-with-button">
                                            <input 
                                                type="text" 
                                                id="phone-number" 
                                                placeholder="+1234567890" 
                                                value={(*phone_number).clone()}
                                                onchange={on_phone_change.clone()}
                                            />
                                            <button 
                                                class={classes!("save-button", if !is_phone_valid { "invalid" } else { "" })}
                                                onclick={on_save_phone.clone()}
                                            >
                                                {"Save"}
                                            </button>
                                            {
                                                match &*phone_save_status {
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
                                    </div>
                                }
                            } else {
                                html! {}
                            }
                        }
                    </div>
                    <div class="instruction-image">
                        <img 
                            src="/assets/number-twilio.png" 
                            alt="Buy Twilio Phone Number Image" 
                            loading="lazy"
                            onclick={let open_modal = open_modal.clone(); let src = "/assets/number-twilio.png".to_string(); 
                                Callback::from(move |_| open_modal.emit(src.clone()))}
                            style="cursor: pointer;"
                        />
                    </div>
                </div>

                <div class="instruction-block">
                    <div class="instruction-content">
                        <h2>{"Twilio Finding Credentials"}</h2>
                        <ul>
                            <li>{"1. Click on the 'Account Dashboard' in the left sidebar"}</li>
                            <li>{"2. Find and copy your 'Account SID' from the dashboard"}</li>
                            <li>{"3. Reveal and copy your 'Auth Token' from the dashboard"}</li>
                        </ul>
                        {
                            if props.is_logged_in && props.sub_tier.as_deref() == Some("tier 2") {
                                html! {
                                    <>
                                        <div class="input-field">
                                            <label for="account-sid">{"Your Account SID:"}</label>
                                            <div class="input-with-button">
                                                <input 
                                                    type="text" 
                                                    id="account-sid" 
                                                    placeholder="ACxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx" 
                                                    value={(*account_sid).clone()}
                                                    onchange={on_sid_change.clone()}
                                                />
                                            </div>
                                        </div>
                                        <div class="input-field">
                                            <label for="auth-token">{"Your Auth Token:"}</label>
                                            <div class="input-with-button">
                                                <input 
                                                    type="text" 
                                                    id="auth-token" 
                                                    placeholder="your_auth_token_here" 
                                                    value={(*auth_token).clone()}
                                                    onchange={on_token_change.clone()}
                                                />
                                            </div>
                                        </div>
                                        <button 
                                            class={classes!("save-button", if !(is_sid_valid && is_token_valid) { "invalid" } else { "" })}
                                            onclick={on_save_creds.clone()}
                                        >
                                            {"Save"}
                                        </button>
                                        {
                                            match &*creds_save_status {
                                                Some(Ok(_)) => html! {
                                                    <span class="save-status success">{"✓ Saved"}</span>
                                                },
                                                Some(Err(err)) => html! {
                                                    <span class="save-status error">{format!("Error: {}", err)}</span>
                                                },
                                                None => html! {}
                                            }
                                        }
                                    </>
                                }
                            } else {
                                html! {}
                            }
                        }
                    </div>
                    <div class="instruction-image">
                        <img 
                            src="/assets/creds-twilio.png" 
                            alt="Twilio Credentials Dashboard" 
                            loading="lazy"
                            onclick={let open_modal = open_modal.clone(); let src = "/assets/creds-twilio.png".to_string(); 
                                Callback::from(move |_| open_modal.emit(src.clone()))}
                            style="cursor: pointer;"
                        />
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

                .instructions-hero {
                    text-align: center;
                    padding: 6rem 2rem;
                    background: rgba(26, 26, 26, 0.75);
                    backdrop-filter: blur(5px);
                    margin-top: 2rem;
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    margin-bottom: 2rem;
                }

                .instructions-hero h1 {
                    font-size: 3.5rem;
                    margin-bottom: 1.5rem;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                }

                .instructions-hero p {
                    font-size: 1.5rem;
                    color: #999;
                    max-width: 600px;
                    margin: 0 auto;
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

                .instruction-content ul ul li::before {
                    content: '◦';
                }

                .instruction-image {
                    flex: 1.2;  /* Increased from 1 to 1.2 to give more space for images */
                    display: flex;
                    justify-content: center;
                    align-items: center;
                }

                .instruction-image img {
                    max-width: 110%;  /* Increased from 100% to 110% */
                    height: auto;
                    border-radius: 12px;
                    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
                    transition: transform 0.3s ease;
                }

                .instruction-image img:hover {
                    transform: scale(1.02);
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

                    .instructions-hero h1 {
                        font-size: 2.5rem;
                    }

                    .instruction-content h2 {
                        font-size: 1.75rem;
                    }

                    .instructions-section {
                        padding: 1rem;
                    }
                }

                .input-field {
                    margin-top: 1.5rem;
                }

                .input-field label {
                    display: block;
                    margin-bottom: 0.5rem;
                    color: #7EB2FF;
                }

                .input-field input {
                    width: 100%;
                    padding: 0.75rem;
                    border: 1px solid rgba(30, 144, 255, 0.3);
                    border-radius: 6px;
                    background: rgba(26, 26, 26, 0.5);
                    color: #fff;
                    font-size: 1rem;
                    transition: all 0.3s ease;
                }

                .input-field input:focus {
                    outline: none;
                    border-color: rgba(30, 144, 255, 0.8);
                    box-shadow: 0 0 0 2px rgba(30, 144, 255, 0.2);
                }

                .input-field input::placeholder {
                    color: rgba(255, 255, 255, 0.3);
                }

                .input-with-button {
                    display: flex;
                    gap: 0.5rem;
                }

                .input-with-button input {
                    flex: 1;
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

                .save-button.invalid {
                    background: #cccccc;
                    color: #666666;
                    cursor: not-allowed;
                }

                .save-button.invalid:hover {
                    background: #cccccc;
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
                    border: 2px solid rgba(255, 255,255, 0.5);
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
