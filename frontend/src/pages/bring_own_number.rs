use yew::prelude::*;
use crate::Route;
use yew_router::prelude::*;
use web_sys::{MouseEvent, window, Event};
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use serde_json::json;
use crate::config;

#[derive(Properties, PartialEq)]
pub struct TwilioHostedInstructionsProps {
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
    pub message: String,
    #[prop_or_default]
    pub country: Option<String>,
}

#[function_component(TwilioHostedInstructions)]
pub fn twilio_hosted_instructions(props: &TwilioHostedInstructionsProps) -> Html {
    let modal_visible = use_state(|| false);
    let selected_image = use_state(|| String::new());
    let selected_country = use_state(|| "".to_string());
    {
        let selected_country = selected_country.clone();
        let country = props.country.clone();
        use_effect_with_deps(
            move |_| {
                selected_country.set(country.unwrap_or("".to_string()).to_lowercase());
                || ()
            },
            props.country.clone(), // Dependency to trigger effect when props.country changes
        );
    }


    let phone_number = use_state(|| props.twilio_phone.clone().unwrap_or_default());
    let account_sid = use_state(|| props.twilio_sid.clone().unwrap_or_default());
    let auth_token = use_state(|| props.twilio_token.clone().unwrap_or_default());

    let phone_save_status = use_state(|| None::<Result<(), String>>);
    let creds_save_status = use_state(|| None::<Result<(), String>>);

    {
        let phone_number = phone_number.clone();
        let account_sid = account_sid.clone();
        let auth_token = auth_token.clone();
        use_effect_with_deps(
            move |(new_phone, new_sid, new_token)| {
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
                || {}
            },
            (
                props.twilio_phone.clone(),
                props.twilio_sid.clone(),
                props.twilio_token.clone(),
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

    let on_country_change = {
        let selected_country = selected_country.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlSelectElement = e.target_unchecked_into();
            selected_country.set(input.value());
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

    let country = (*selected_country).clone();

    let twilio_cost = match country.as_str() {
        "fi" => "normal 5$-25$/month (for 20-100 messages)".to_string(),
        "gb" => "3.5$-17.5$/month (for 20-100 messages)".to_string(),
        "au" => "3.5$-17.5$/month (for 20-100 messages)".to_string(),
        "se" => "normal 4$-20$/month (for 20-100 messages)".to_string(),
        "dk" => "3.5$-17.5$/month (for 20-100 messages)".to_string(),
        "de" => "Hard to setup: 15$/month number with registered business and 120 day validation time. Messages cost ~2x UK rates. Easier to use a UK number.".to_string(),
        _ => "3.5$-17.5$/month (for 20-100 messages)".to_string(),
    };

    let twilio_link = match country.as_str() {
        "fi" => "https://www.twilio.com/en-us/sms/pricing/fi".to_string(),
        "gb" => "https://www.twilio.com/en-us/sms/pricing/gb".to_string(),
        "au" => "https://www.twilio.com/en-us/sms/pricing/au".to_string(),
        "se" => "https://www.twilio.com/en-us/sms/pricing/se".to_string(),
        "dk" => "https://www.twilio.com/en-us/sms/pricing/dk".to_string(),
        "de" => "https://www.twilio.com/en-us/sms/pricing/de".to_string(),
        _ => "https://www.twilio.com/en-us/sms/pricing/en".to_string(),
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
                        <p>{"Lightfriend uses Twilio for SMS messaging and voice calls, giving your AI assistant the ability to communicate via a dedicated phone number. International users can bring their own number and pay for messages straight to Twilio."}</p>
                    </div>
                </div>

                <div class="instruction-block">
                    <div class="instruction-content">
                        <h2>{"Twilio Expected Costs"}</h2>
                        <p>{"Select a country to view the estimated costs for Twilio messaging services."}</p>
                        <div class="country-selector">
                            <label for="country-select">{"Country: "}</label>
                            <select id="country-select" onchange={on_country_change}>
                                <option value="fi" selected={country == "fi"}>{"FI"}</option>
                                <option value="gb" selected={country == "gb"}>{"GB"}</option>
                                <option value="au" selected={country == "au"}>{"AU"}</option>
                                <option value="se" selected={country == "se"}>{"SE"}</option>
                                <option value="dk" selected={country == "dk"}>{"DK"}</option>
                                <option value="de" selected={country == "de"}>{"DE"}</option>
                            </select>
                        </div>
                        <table class="setup-table">
                            <thead>
                                <tr>
                                    <th>{"Service"}</th>
                                    <th>{"Approx Cost"}</th>
                                </tr>
                            </thead>
                            <tbody>
                                <tr>
                                    <td>{"Twilio for Messaging"}</td>
                                    <td><a href={twilio_link} target="_blank" class="cost-link">{twilio_cost}</a></td>
                                </tr>
                            </tbody>
                        </table>
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
                        <h2>{"Twilio Buy a Phone Number"}</h2>
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

                <div class="back-home-container">
                    <Link<Route> to={Route::Home} classes="back-home-button">
                        {"Back to Home"}
                    </Link<Route>>
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

                .country-selector {
                    display: flex;
                    align-items: center;
                    gap: 1rem;
                    margin-bottom: 2rem;
                }

                .country-selector label {
                    color: #7EB2FF;
                    font-size: 1.1rem;
                }

                .country-selector select {
                    padding: 0.75rem;
                    background: rgba(26, 26, 26, 0.5);
                    border: 1px solid rgba(30, 144, 255, 0.3);
                    color: #fff;
                    border-radius: 6px;
                    font-size: 1rem;
                    cursor: pointer;
                    transition: all 0.3s ease;
                }

                .country-selector select:focus {
                    outline: none;
                    border-color: rgba(30, 144, 255, 0.8);
                }

                .setup-table {
                    width: 100%;
                    border-collapse: separate;
                    border-spacing: 0;
                    border-radius: 8px;
                    overflow: hidden;
                }

                .setup-table th,
                .setup-table td {
                    padding: 1rem 1.5rem;
                    text-align: left;
                    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
                    color: #999;
                }

                .setup-table th {
                    background: rgba(30, 144, 255, 0.1);
                    color: #fff;
                    font-weight: normal;
                }

                .setup-table td:first-child {
                    color: #fff;
                }

                .cost-link {
                    color: #1E90FF;
                    text-decoration: none;
                }

                .cost-link:hover {
                    text-decoration: underline;
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

                    .setup-table th,
                    .setup-table td {
                        padding: 0.75rem 1rem;
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

                .back-home-container {
                    text-align: center;
                    margin-top: 2rem;
                    margin-bottom: 2rem;
                }

                .back-home-button {
                    padding: 0.75rem 1.5rem;
                    background: #1E90FF;
                    color: white;
                    border: none;
                    border-radius: 6px;
                    cursor: pointer;
                    font-size: 1rem;
                    text-decoration: none;
                    display: inline-block;
                    transition: all 0.3s ease;
                }

                .back-home-button:hover {
                    background: #1976D2;
                }

                .back-home-button:active {
                    transform: translateY(1px);
                }
                "#}
            </style>
        </div>
    }
}
