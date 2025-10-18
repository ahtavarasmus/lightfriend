use yew::prelude::*;
use crate::config;
use web_sys::window;
use gloo_net::http::Request;
use crate::profile::models::UserProfile;
use crate::profile::settings::SettingsPage;
use crate::auth::connect::Connect;
use serde::Serialize;

#[derive(Clone, PartialEq)]
enum DashboardTab {
    Connections,
    Personal,
}

#[derive(Serialize)]
struct UpdateProfileRequest {
    phone_number: String,
    nickname: String,
    info: String,
    timezone: String,
    timezone_auto: bool,
    agent_language: String,
    notification_type: Option<String>,
    save_context: Option<i32>,
    location: String,
    nearby_places: String,
}

pub fn is_logged_in() -> bool {
    if let Some(window) = window() {
        if let Ok(Some(storage)) = window.local_storage() {
            if let Ok(Some(_token)) = storage.get_item("token") {
                return true;
            }
        }
    }
    false
}

#[function_component]
pub fn MonthlyCredits(props: &Props) -> Html {
    let profile = &props.profile;
    html! {
        <div class="credit-item" tabindex="0">
            <span class="credit-label">{"Monthly Message Quota"}</span>
            <span class="credit-value">{profile.credits_left as i32}{" Messages"}</span>
            <div class="credit-tooltip">
                {"Your monthly quota Message quota. Can be used to ask questions, voice calls(1 Message = 1 minute) or to receive priority sender notifications(1 Message = 2 notifications). Not enough? Buy overage credits or trade in unused digest slots for Messages."}
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub profile: UserProfile,
}

#[function_component]
pub fn Home() -> Html {
    let profile_data = use_state(|| None::<UserProfile>);
    let error = use_state(|| None::<String>);
    let success = use_state(|| None::<String>);
    let is_expanded = use_state(|| false);
    let active_tab = use_state(|| DashboardTab::Connections);
    let phone_input = use_state(String::new);
    let is_phone_editing = use_state(|| false);
    // Single profile fetch effect
    {
        let profile_data = profile_data.clone();
        let error = error.clone();
    
        use_effect_with_deps(move |_| {
            let profile_data = profile_data.clone();
            let error = error.clone();
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
                            if response.status() == 401 {
                                if let Some(window) = window() {
                                    if let Ok(Some(storage)) = window.local_storage() {
                                        let _ = storage.remove_item("token");
                                        let _ = window.location().set_href("/login");
                                    }
                                }
                                return;
                            }
                        
                            match response.json::<UserProfile>().await {
                                Ok(profile) => {
                                    profile_data.set(Some(profile));
                                    error.set(None);
                                }
                                Err(e) => {
                                    web_sys::console::log_1(&format!("JSON parse error: {:?}", e).into());
                                    error.set(Some("Failed to parse profile data".to_string()));
                                }
                            }
                        }
                        Err(_) => {
                            error.set(Some("Failed to fetch profile".to_string()));
                        }
                    }
                } else {
                    let _ = window().unwrap().location().set_href("/login");
                }
            });
        
            || ()
        }, ());
    }

    let on_phone_save = {
        let profile_data = profile_data.clone();
        let phone_input = phone_input.clone();
        let error = error.clone();
        let success = success.clone();
        let is_phone_editing = is_phone_editing.clone();
        Callback::from(move |_| {
            let profile_data = profile_data.clone();
            let phone_input = phone_input.clone();
            let error = error.clone();
            let success = success.clone();
            let is_phone_editing = is_phone_editing.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|storage| storage.get_item("token").ok())
                    .flatten()
                {
                    if let Some(profile) = (*profile_data).as_ref() {
                        match Request::post(&format!("{}/api/profile/update", config::get_backend_url()))
                            .header("Authorization", &format!("Bearer {}", token))
                            .json(&UpdateProfileRequest {
                                phone_number: (*phone_input).clone(),
                                nickname: profile.nickname.clone().unwrap_or_default(),
                                info: profile.info.clone().unwrap_or_default(),
                                timezone: profile.timezone.clone().unwrap_or_else(|| String::from("UTC")),
                                timezone_auto: profile.timezone_auto.unwrap_or(true),
                                agent_language: profile.agent_language.clone(),
                                notification_type: profile.notification_type.clone(),
                                save_context: profile.save_context,
                                location: profile.location.clone().unwrap_or_default(),
                                nearby_places: profile.nearby_places.clone().unwrap_or_default(),
                            })
                            .expect("Failed to build request")
                            .send()
                            .await
                        {
                            Ok(response) => {
                                if response.status() == 401 {
                                    if let Some(window) = window() {
                                        if let Ok(Some(storage)) = window.local_storage() {
                                            let _ = storage.remove_item("token");
                                            let _ = window.location().set_href("/login");
                                        }
                                    }
                                    return;
                                } else if response.ok() {
                                    // Refetch profile to get updated data
                                    match Request::get(&format!("{}/api/profile", config::get_backend_url()))
                                        .header("Authorization", &format!("Bearer {}", token))
                                        .send()
                                        .await
                                    {
                                        Ok(refetch_response) => {
                                            if let Ok(updated_profile) = refetch_response.json::<UserProfile>().await {
                                                profile_data.set(Some(updated_profile));
                                                success.set(Some("Phone number updated successfully".to_string()));
                                                error.set(None);
                                                is_phone_editing.set(false);
                                                // Clear success after 3s
                                                let success_clone = success.clone();
                                                wasm_bindgen_futures::spawn_local(async move {
                                                    gloo_timers::future::TimeoutFuture::new(3000).await;
                                                    success_clone.set(None);
                                                });
                                            } else {
                                                error.set(Some("Failed to refetch updated profile".to_string()));
                                            }
                                        }
                                        Err(_) => {
                                            error.set(Some("Failed to refetch profile".to_string()));
                                        }
                                    }
                                } else {
                                    error.set(Some("Failed to update phone number".to_string()));
                                }
                            }
                            Err(_) => {
                                error.set(Some("Failed to send request".to_string()));
                            }
                        }
                    }
                }
            });
        })
    };

    let on_phone_edit = {
        let is_phone_editing = is_phone_editing.clone();
        Callback::from(move |_| is_phone_editing.set(true))
    };

    html! {
        <>
            <div class="dashboard-container">
                <h1 class="panel-title">{"Dashboard"}</h1>
                <div class="status-section">
                    <div class="credits-info">
                    {
                        if let Some(profile) = (*profile_data).as_ref() {
                            html! {
                                <div class="credits-grid">
                                    {
                                        if profile.credits > 0.00 {
                                            html! {
                                                <div class="credit-item" tabindex="0">
                                                    <span class="credit-label">{"Message Credits"}</span>
                                                    <span class="credit-value">{format!("{:.2}€", profile.credits)}</span>
                                                    <div class="credit-tooltip">
                                                        {"Your message credits."}
                                                    </div>
                                                </div>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                </div>
                            }
                        } else {
                            html! { <div class="loading-profile">{"Loading profile..."}</div> }
                        }
                    }
                    </div>
                </div>
                {
                    if let Some(profile) = (*profile_data).as_ref() {
                        if let Some(preferred_number) = &profile.preferred_number {
                            html! {
                                <div class="phone-selector">
                                    <div class="preferred-number-display">
                                        <span class="preferred-number-label configured">
                                            {format!("Your lightfriend's Number: {}", preferred_number)}
                                        </span>
                                    </div>
                                </div>
                            }
                        } else if profile.phone_number.is_empty() {
                            html! {
                                <div class="phone-setup">
                                    {
                                        if let Some(err) = (*error).as_ref() {
                                            html! { <div class="message error-message">{err}</div> }
                                        } else if let Some(suc) = (*success).as_ref() {
                                            html! { <div class="message success-message">{suc}</div> }
                                        } else {
                                            html! {}
                                        }
                                    }
                                    <div class="profile-field">
                                        <span class="field-label">{"Phone Number (Required to start)"}</span>
                                        {
                                            if *is_phone_editing {
                                                html! {
                                                    <>
                                                        <input
                                                            type="tel"
                                                            class="profile-input"
                                                            value={(*phone_input).clone()}
                                                            placeholder="+1234567890"
                                                            onchange={let phone_input = phone_input.clone(); move |e: Event| {
                                                                let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                                                phone_input.set(input.value());
                                                            }}
                                                        />
                                                        <button onclick={on_phone_save} class="edit-button confirming">{"Save Phone Number"}</button>
                                                    </>
                                                }
                                            } else {
                                                html! {
                                                    <>
                                                        <span class="field-value not-configured">{"Not set"}</span>
                                                        <button onclick={on_phone_edit} class="edit-button">{"Add Phone Number"}</button>
                                                    </>
                                                }
                                            }
                                        }
                                    </div>
                                </div>
                            }
                        } else {
                            html! {
                                <div class="phone-selector">
                                    <div class="preferred-number-display">
                                        <span class="preferred-number-label not-configured">
                                            {"Phone number set, but lightfriend number not ready yet."}
                                        </span>
                                    </div>
                                </div>
                            }
                        }
                    } else {
                        html! {}
                    }
                }
            
            
                <br/>
                <br/>
                <div class="dashboard-tabs">
                    <button
                        class={classes!("tab-button", (*active_tab == DashboardTab::Connections).then(|| "active"))}
                        onclick={{
                            let active_tab = active_tab.clone();
                            Callback::from(move |_| active_tab.set(DashboardTab::Connections))
                        }}
                    >
                        {"Connections"}
                    </button>
                    <button
                        class={classes!("tab-button", (*active_tab == DashboardTab::Personal).then(|| "active"))}
                        onclick={{
                            let active_tab = active_tab.clone();
                            Callback::from(move |_| active_tab.set(DashboardTab::Personal))
                        }}
                    >
                        {"Personal"}
                    </button>
                </div>
                {
                    match *active_tab {
                        DashboardTab::Connections => html! {
                            <div class="connections-tab">
                                {
                                    if let Some(profile) = (*profile_data).as_ref() {
                                        html! {
                                            <Connect user_id={profile.id} phone_number={profile.phone_number.clone()} estimated_monitoring_cost={profile.estimated_monitoring_cost.clone()}/>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
                                <div class="feature-status">
                                    <p class="feature-suggestion">
                                        {"Have a feature in mind? Email your suggestions to "}
                                        <a href="mailto:rasmus@ahtava.com">{"rasmus@ahtava.com"}</a>
                                    </p>
                                    <h4>{"Tips"}</h4>
                                    <ul>
                                        <li>{"You can ask multiple questions in a single SMS to save money. Note that answers will be less detailed due to SMS character limits. Example: 'did sam altman tweet today and whats the weather?' -> 'Sam Altman hasn't tweeted today. Last tweet was on March 3, a cryptic \"!!!\" image suggesting a major AI development. Weather in Tampere: partly cloudy, 0°C, 82% humidity, wind at 4 m/s.'"}</li>
                                        <li>{"Start your message with 'forget' to make the assistant forget previous conversation context and start fresh. Note that this only applies to that one message - the next message will again remember previous context."}</li>
                                    </ul>
                                </div>
                            </div>
                        },
                        DashboardTab::Personal => html! {
                            <div class="personal-tab">
                                {
                                    if let Some(profile) = (*profile_data).as_ref() {
                                        html! {
                                            <SettingsPage
                                                user_profile={profile.clone()}
                                                on_profile_update={{
                                                    let profile_data = profile_data.clone();
                                                    Callback::from(move |updated_profile| {
                                                        profile_data.set(Some(updated_profile));
                                                    })
                                                }}
                                            />
                                        }
                                    } else {
                                        html! {
                                            <div class="loading-profile">{"Loading profile..."}</div>
                                        }
                                    }
                                }
                            </div>
                        }
                    }
                }
                <footer class="dashboard-footer">
                    <div class="development-links">
                        <p>{"Source code on "}
                            <a href="https://github.com/ahtavarasmus/lightfriend" target="_blank" rel="noopener noreferrer">{"GitHub"}</a>
                        </p>
                        <div class="legal-links">
                            <a href="/terms">{"Terms & Conditions"}</a>
                            {" | "}
                            <a href="/privacy">{"Privacy Policy"}</a>
                        </div>
                    </div>
                </footer>
            </div>
            <style>
                {r#"
                    .status-section {
                        display: flex;
                        align-items: center;
                        justify-content: space-between;
                        margin-bottom: 1.5rem;
                    }
                    .credits-info {
                        width: 100%;
                        margin-bottom: 1rem;
                    }
                    .credits-grid {
                        display: grid;
                        grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
                        gap: 1rem;
                        width: 100%;
                    }
                    .credit-item {
                        background: rgba(30, 144, 255, 0.05);
                        border: 1px solid rgba(30, 144, 255, 0.1);
                        border-radius: 8px;
                        padding: 0.75rem;
                        display: flex;
                        flex-direction: column;
                        align-items: center;
                        gap: 0.25rem;
                        position: relative;
                        cursor: pointer;
                        transition: all 0.3s ease;
                        outline: none;
                    }
                    .credit-item:hover,
                    .credit-item:focus {
                        background: rgba(30, 144, 255, 0.1);
                        border-color: rgba(30, 144, 255, 0.2);
                    }
                    .credit-tooltip {
                        position: absolute;
                        bottom: calc(100% + 10px);
                        left: 50%;
                        transform: translateX(-50%);
                        background: rgba(0, 0, 0, 0.9);
                        color: #fff;
                        padding: 1rem;
                        border-radius: 8px;
                        font-size: 0.80rem;
                        width: max-content;
                        max-width: 350px;
                        z-index: 1000;
                        opacity: 0;
                        visibility: hidden;
                        transition: all 0.3s ease;
                        box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
                        border: 1px solid rgba(30, 144, 255, 0.2);
                        text-align: center;
                    }
                    .credit-tooltip::after {
                        content: '';
                        position: absolute;
                        top: 100%;
                        left: 50%;
                        transform: translateX(-50%);
                        border-width: 8px;
                        border-style: solid;
                        border-color: rgba(0, 0, 0, 0.9) transparent transparent transparent;
                    }
                    .credit-item:hover .credit-tooltip,
                    .credit-item:focus .credit-tooltip {
                        opacity: 1;
                        visibility: visible;
                    }
                    .credit-label {
                        color: #999;
                        font-size: 0.8rem;
                        text-transform: uppercase;
                        letter-spacing: 0.5px;
                    }
                    .credit-value {
                        color: #7EB2FF;
                        font-size: 1.1rem;
                        font-weight: 500;
                    }
                    .reset-timer {
                        display: block;
                        color: #999;
                        font-size: 0.8rem;
                        margin-top: 0.3rem;
                    }
                    .credit-warning {
                        grid-column: 1 / -1;
                        color: #ff4444;
                        font-size: 0.9rem;
                        text-align: center;
                        padding: 0.5rem;
                        background: rgba(255, 68, 68, 0.1);
                        border-radius: 6px;
                        margin-top: 0.5rem;
                    }
                    .phone-selector, .phone-setup {
                        margin: 1.5rem 0;
                    }
                    .preferred-number-display {
                        background: rgba(30, 30, 30, 0.7);
                        border: 1px solid rgba(30, 144, 255, 0.1);
                        border-radius: 8px;
                        padding: 1rem;
                        text-align: center;
                        margin-bottom: 1rem;
                    }
                    .preferred-number-label {
                        display: block;
                        font-size: 1.1rem;
                        font-weight: 500;
                    }
                    .preferred-number-label.configured {
                        color: #7EB2FF;
                    }
                    .preferred-number-label.not-configured {
                        color: #ff4444;
                    }
                    .phone-setup {
                        background: rgba(255, 68, 68, 0.05);
                        border: 1px solid rgba(255, 68, 68, 0.2);
                        border-radius: 8px;
                        padding: 1.5rem;
                        text-align: center;
                    }
                    .phone-setup .field-label {
                        color: #ff4444;
                        font-weight: 500;
                    }
                    .phone-setup .profile-input {
                        margin-bottom: 1rem;
                    }
                    .phone-display {
                        margin: 2rem 0;
                    }
                    .dashboard-container {
                        min-height: 100vh;
                        border-radius: 12px;
                        background: #1a1a1a;
                        padding: 3rem 2rem;
                        width: 100%;
                        max-width: 800px;
                        margin: 4rem auto;
                    }
                    .panel-title {
                        font-size: 2.5rem;
                        background: linear-gradient(45deg, #fff, #7EB2FF);
                        -webkit-background-clip: text;
                        -webkit-text-fill-color: transparent;
                        margin: 0 0 1.5rem 0;
                        text-align: center;
                    }
                    .dashboard-tabs {
                        display: flex;
                        gap: 1rem;
                        margin-bottom: 2rem;
                        border-bottom: 1px solid rgba(30, 144, 255, 0.1);
                        padding-bottom: 1rem;
                        flex-wrap: wrap;
                    }
                    .tab-button {
                        background: transparent;
                        border: none;
                        color: #999;
                        padding: 0.5rem 1rem;
                        cursor: pointer;
                        font-size: 1rem;
                        transition: all 0.3s ease;
                        position: relative;
                        white-space: nowrap;
                        flex: 1;
                        min-width: fit-content;
                    }
                    .tab-button::after {
                        content: '';
                        position: absolute;
                        bottom: -1rem;
                        left: 0;
                        width: 100%;
                        height: 2px;
                        background: transparent;
                        transition: background-color 0.3s ease;
                    }
                    .tab-button.active {
                        color: white;
                    }
                    .tab-button.active::after {
                        background: #1E90FF;
                    }
                    .tab-button:hover {
                        color: #7EB2FF;
                    }
                    .feature-status {
                        margin-top: 3rem;
                        text-align: left;
                        padding: 2rem;
                        background: rgba(30, 30, 30, 0.7);
                        border-radius: 12px;
                        border: 1px solid rgba(30, 144, 255, 0.1);
                        backdrop-filter: blur(10px);
                    }
                    .feature-status h4 {
                        color: #7EB2FF;
                        font-size: 0.9rem;
                        margin: 1rem 0 0.5rem 0;
                    }
                    .feature-status ul {
                        list-style: none;
                        padding: 0;
                        margin: 0 0 1.5rem 0;
                    }
                    .feature-status li {
                        color: #999;
                        font-size: 0.9rem;
                        padding: 0.3rem 0;
                        padding-left: 1.5rem;
                        position: relative;
                    }
                    .feature-status li::before {
                        content: '•';
                        position: absolute;
                        left: 0.5rem;
                        color: #1E90FF;
                    }
                    .feature-suggestion {
                        margin-top: 1rem;
                        color: #999;
                        font-size: 0.9rem;
                    }
                    .feature-suggestion a {
                        color: #1E90FF;
                        text-decoration: none;
                        transition: color 0.3s ease;
                    }
                    .feature-suggestion a:hover {
                        color: #7EB2FF;
                        text-decoration: underline;
                    }
                    .subscriber-promo {
                        margin-top: 1rem;
                        padding: 1rem;
                        background: rgba(30, 144, 255, 0.05);
                        border: 1px solid rgba(30, 144, 255, 0.1);
                        border-radius: 8px;
                        text-align: center;
                    }
                    .subscriber-promo p {
                        color: #fff;
                        margin-bottom: 0.5rem;
                    }
                    .subscriber-promo a {
                        color: #1E90FF;
                        text-decoration: none;
                    }
                    .subscriber-promo a:hover {
                        text-decoration: underline;
                    }
                    .development-links {
                        margin-top: 2rem;
                        font-size: 0.9rem;
                        color: #666;
                    }
                    .development-links p {
                        margin: 0.5rem 0;
                    }
                    .development-links a {
                        color: #007bff;
                        text-decoration: none;
                        position: relative;
                        padding: 0 2px;
                        transition: all 0.3s ease;
                    }
                    .development-links a::after {
                        content: '';
                        position: absolute;
                        width: 100%;
                        height: 1px;
                        bottom: -2px;
                        left: 0;
                        background: linear-gradient(90deg, #1E90FF, #4169E1);
                        transform: scaleX(0);
                        transform-origin: bottom right;
                        transition: transform 0.3s ease;
                    }
                    .development-links a:hover {
                        color: #7EB2FF;
                        text-shadow: 0 0 8px rgba(30, 144, 255, 0.3);
                    }
                    .development-links a:hover::after {
                        transform: scaleX(1);
                        transform-origin: bottom left;
                    }
                    .profile-input {
                        background: rgba(0, 0, 0, 0.2);
                        border: 1px solid rgba(30, 144, 255, 0.2);
                        border-radius: 8px;
                        padding: 0.75rem;
                        color: #ffffff;
                        font-size: 1rem;
                        transition: all 0.3s ease;
                        width: 100%;
                        margin-bottom: 1rem;
                    }
                    .profile-input:focus {
                        outline: none;
                        border-color: rgba(30, 144, 255, 0.5);
                        box-shadow: 0 0 0 2px rgba(30, 144, 255, 0.1);
                    }
                    .edit-button {
                        background: linear-gradient(45deg, #1E90FF, #4169E1);
                        color: white;
                        border: none;
                        padding: 0.75rem 1.5rem;
                        border-radius: 8px;
                        font-size: 1rem;
                        cursor: pointer;
                        transition: all 0.3s ease;
                        margin-top: 0.5rem;
                    }
                    .edit-button:hover {
                        transform: translateY(-2px);
                        box-shadow: 0 4px 20px rgba(30, 144, 255, 0.3);
                    }
                    .edit-button.confirming {
                        background: linear-gradient(45deg, #4CAF50, #45a049);
                    }
                    .message {
                        padding: 0.75rem;
                        border-radius: 8px;
                        margin-bottom: 1rem;
                        text-align: center;
                    }
                    .error-message {
                        background: rgba(255, 68, 68, 0.1);
                        color: #ff4444;
                        border: 1px solid rgba(255, 68, 68, 0.3);
                    }
                    .success-message {
                        background: rgba(76, 175, 80, 0.1);
                        color: #4CAF50;
                        border: 1px solid rgba(76, 175, 80, 0.3);
                    }
                    .profile-field {
                        display: flex;
                        flex-direction: column;
                        gap: 0.5rem;
                        margin-bottom: 1.5rem;
                    }
                    .field-label {
                        font-size: 0.9rem;
                        color: #999;
                        text-transform: uppercase;
                        letter-spacing: 0.5px;
                    }
                    .field-value {
                        color: #fff;
                        font-size: 1rem;
                    }
                    .field-value.not-configured {
                        color: #ff4444;
                    }
                    @media (max-width: 768px) {
                        .status-section {
                            flex-direction: column;
                            align-items: flex-start;
                        }
                        .credit-item {
                            padding: 1rem;
                        }
                        .credit-tooltip {
                            position: fixed;
                            bottom: 20px;
                            left: 50%;
                            transform: translateX(-50%);
                            width: 90%;
                            max-width: 300px;
                            z-index: 1001;
                        }
                        .credit-tooltip::after {
                            display: none;
                        }
                        .dashboard-container {
                            padding: 2rem;
                        }
                        .panel-title {
                            font-size: 1.75rem;
                        }
                    }
                    @media (max-width: 480px) {
                        .dashboard-tabs {
                            gap: 0.5rem;
                            justify-content: center;
                        }
                        .tab-button {
                            padding: 0.5rem 0.75rem;
                            font-size: 0.9rem;
                        }
                    }
                "#}
            </style>
        </>
    }
}
