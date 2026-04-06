use crate::utils::api::Api;
use gloo_timers::future::TimeoutFuture;
use serde_json::json;
use wasm_bindgen_futures::spawn_local;
use web_sys::{Event, HtmlInputElement, MouseEvent};
use yew::prelude::*;
#[derive(Properties, PartialEq)]
pub struct EmailProps {
    pub user_id: i32,
    pub sub_tier: Option<String>,
}
#[function_component(EmailConnect)]
pub fn email_connect(props: &EmailProps) -> Html {
    let error = use_state(|| None::<String>);
    let success_message = use_state(|| None::<String>);
    let connected_accounts = use_state(|| Vec::<String>::new());
    let imap_email = use_state(|| String::new());
    let imap_password = use_state(|| String::new());
    let imap_server = use_state(|| String::new());
    let imap_port = use_state(|| String::new());
    let detecting = use_state(|| false);
    let manual_override = use_state(|| false); // true = show provider dropdown instead of auto-detect
    let manual_provider = use_state(|| "gmail".to_string());
    // Detection result: (provider_key, label, imap_server, imap_port, instructions, instructions_url)
    let detected = use_state(|| None::<(String, String, String, u16, Option<String>, Option<String>)>);
    let detected_reset = detected.clone();
    // Provider list for manual override
    let providers = vec![
        ("gmail", "Gmail", "imap.gmail.com", "993"),
        ("icloud", "iCloud", "imap.mail.me.com", "993"),
        ("outlook", "Outlook / Hotmail", "outlook.office365.com", "993"),
        ("yahoo", "Yahoo Mail", "imap.mail.yahoo.com", "993"),
        ("aol", "AOL", "imap.aol.com", "993"),
        ("fastmail", "Fastmail", "imap.fastmail.com", "993"),
        ("zoho", "Zoho Mail", "imap.zoho.com", "993"),
        ("yandex", "Yandex", "imap.yandex.com", "993"),
        ("gmx", "GMX", "imap.gmx.com", "993"),
        ("privateemail", "PrivateEmail (Namecheap)", "mail.privateemail.com", "993"),
        ("godaddy", "GoDaddy", "imap.secureserver.net", "993"),
        ("hostinger", "Hostinger", "imap.hostinger.com", "993"),
        ("custom", "Other / Custom", "", ""),
    ];
    // Check connection status on component mount
    {
        let connected_accounts = connected_accounts.clone();
        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    let request = Api::get("/api/auth/imap/status").send().await;
                    if let Ok(response) = request {
                        if response.ok() {
                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                if let Some(connections) =
                                    data.get("connections").and_then(|v| v.as_array())
                                {
                                    let emails: Vec<String> = connections
                                        .iter()
                                        .filter_map(|c| {
                                            c.get("email")
                                                .and_then(|e| e.as_str())
                                                .map(String::from)
                                        })
                                        .collect();
                                    connected_accounts.set(emails);
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
    // Handlers for input changes
    let onchange_imap_email = {
        let imap_email = imap_email.clone();
        let detected = detected.clone();
        let manual_override = manual_override.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            imap_email.set(input.value());
            // Reset detection and manual override when email changes
            detected.set(None);
            manual_override.set(false);
        })
    };
    let onchange_imap_password = {
        let imap_password = imap_password.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            imap_password.set(input.value());
        })
    };
    let onchange_imap_server = {
        let imap_server = imap_server.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            imap_server.set(input.value());
        })
    };
    let onchange_imap_port = {
        let imap_port = imap_port.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            imap_port.set(input.value());
        })
    };
    // Step 1: Detect provider from email
    let onclick_detect = {
        let imap_email = imap_email.clone();
        let detected = detected.clone();
        let detecting = detecting.clone();
        let imap_server = imap_server.clone();
        let imap_port = imap_port.clone();
        let error = error.clone();
        Callback::from(move |_: MouseEvent| {
            let email = (*imap_email).clone();
            let detected = detected.clone();
            let detecting = detecting.clone();
            let imap_server = imap_server.clone();
            let imap_port = imap_port.clone();
            let error = error.clone();
            if email.is_empty() || !email.contains('@') {
                error.set(Some("Please enter a valid email address.".to_string()));
                return;
            }
            error.set(None);
            detecting.set(true);
            spawn_local(async move {
                let payload = json!({"email": email});
                let request = Api::post("/api/auth/imap/detect")
                    .header("Content-Type", "application/json")
                    .json(&payload)
                    .unwrap();
                match request.send().await {
                    Ok(response) => {
                        detecting.set(false);
                        if response.ok() {
                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                let provider = data.get("provider").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                                let label = data.get("label").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string();
                                let server = data.get("imap_server").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let port = data.get("imap_port").and_then(|v| v.as_u64()).unwrap_or(993) as u16;
                                let instructions = data.get("instructions").and_then(|v| v.as_str()).map(String::from);
                                let instructions_url = data.get("instructions_url").and_then(|v| v.as_str()).map(String::from);
                                imap_server.set(server.clone());
                                imap_port.set(port.to_string());
                                detected.set(Some((provider, label, server, port, instructions, instructions_url)));
                            }
                        } else {
                            error.set(Some("Failed to detect email provider.".to_string()));
                        }
                    }
                    Err(e) => {
                        detecting.set(false);
                        error.set(Some(format!("Network error: {}", e)));
                    }
                }
            });
        })
    };
    let onclick_imap_connect = {
        let imap_email_value = imap_email.clone();
        let imap_password_value = imap_password.clone();
        let imap_server_value = imap_server.clone();
        let imap_port_value = imap_port.clone();
        let connected_accounts = connected_accounts.clone();
        let error = error.clone();
        let success_message = success_message.clone();
        let imap_email_setter = imap_email.clone();
        let imap_password_setter = imap_password.clone();
        let detected_setter = detected.clone();
        Callback::from(move |_: MouseEvent| {
            let email = (*imap_email_value).clone();
            let password = (*imap_password_value).clone();
            let server = (*imap_server_value).clone();
            let port = (*imap_port_value).clone();
            let connected_accounts = connected_accounts.clone();
            let error = error.clone();
            let success_message = success_message.clone();
            let imap_email_setter = imap_email_setter.clone();
            let imap_password_setter = imap_password_setter.clone();
            let detected_setter = detected_setter.clone();
            spawn_local(async move {
                let mut payload = json!({
                    "email": email,
                    "password": password,
                });
                if !server.is_empty() && !port.is_empty() {
                    payload["imap_server"] = json!(server);
                    payload["imap_port"] = json!(port.parse::<u16>().unwrap_or(993));
                }
                let request = Api::post("/api/auth/imap/login")
                    .header("Content-Type", "application/json")
                    .json(&payload)
                    .unwrap();
                match request.send().await {
                    Ok(response) => {
                        if response.ok() {
                            // Add to connected accounts list
                            let mut accounts = (*connected_accounts).clone();
                            if !accounts.contains(&email) {
                                accounts.push(email);
                            }
                            connected_accounts.set(accounts);
                            imap_email_setter.set(String::new());
                            imap_password_setter.set(String::new());
                            detected_setter.set(None);
                            error.set(None);
                            success_message.set(Some("Email connected successfully!".to_string()));
                            let success_message_clone = success_message.clone();
                            spawn_local(async move {
                                TimeoutFuture::new(3_000).await;
                                success_message_clone.set(None);
                            });
                        } else {
                            if let Ok(error_data) = response.json::<serde_json::Value>().await {
                                if let Some(error_msg) =
                                    error_data.get("error").and_then(|e| e.as_str())
                                {
                                    error.set(Some(error_msg.to_string()));
                                } else {
                                    error.set(Some(format!(
                                        "Failed to connect: {}",
                                        response.status()
                                    )));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error: {}", e)));
                    }
                }
            });
        })
    };
    html! {
        <div class="service-item">
            if let Some(msg) = (*success_message).as_ref() {
                <div class="success-banner">
                    {msg}
                </div>
            }
            <div class="service-header">
                <div class="service-name">
                    <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 512 512'%3E%3Cpath fill='%234285f4' d='M48 64C21.5 64 0 85.5 0 112c0 15.1 7.1 29.3 19.2 38.4L236.8 313.6c11.4 8.5 27 8.5 38.4 0L492.8 150.4c12.1-9.1 19.2-23.3 19.2-38.4c0-26.5-21.5-48-48-48H48zM0 176V384c0 35.3 28.7 64 64 64H448c35.3 0 64-28.7 64-64V176L294.4 339.2c-22.8 17.1-54 17.1-76.8 0L0 176z'/%3E%3C/svg%3E" alt="IMAP" width="24" height="24"/>
                    {"Email"}
                </div>
                <button class="info-button" onclick={Callback::from(|_| {
                    if let Some(element) = web_sys::window()
                        .and_then(|w| w.document())
                        .and_then(|d| d.get_element_by_id("email-info"))
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
                if !connected_accounts.is_empty() {
                    <div class="service-status-container">
                        <span class="service-status">
                            {format!("{} connected", connected_accounts.len())}
                        </span>
                    </div>
                }
            </div>
            <div id="email-info" class="info-section" style="display: none">
                <h4>{"How It Works"}</h4>
                <div class="info-subsection">
                    <h5>{"SMS and Voice Call Tools"}</h5>
                    <ul>
                        <li>{"Fetch specific number of Email Previews: Fetches a given number of latest emails previews from your inbox."}</li>
                        <li>{"Search specific Email: Searches for specific email based on a given query(sender, subject, content, time, etc)."}</li>
                        <li>{"Send Email: Give recipient name email address, subject and content and lightfriend send the email. Email will only be sent 60 seconds later so if you or assistant made a mistake just type 'cancel' with sms or say 'cancel the message' with voice calls."}</li>
                        <li>{"Reply to an Email: If you have asked about your emails or lightfriend is making a notification call about a specific email, then you can ask to reply to specific one by mentioning your reply content and which email to send reply to. Reply will only be sent 60 seconds later so if you or assistant made a mistake just type 'cancel' with sms or say 'cancel the message' with voice calls."}</li>
                    </ul>
                </div>
                <div class="info-subsection">
                    <h5>{"Provider Support"}</h5>
                    <p class="provider-note">{"We support 15+ email providers. Most require an App Password for security:"}</p>
                    <ul>
                        <li>{"Gmail: "}<a class="nice-link" href="https://myaccount.google.com/apppasswords" target="_blank">{"Create App Password"}</a>{" (requires 2FA)"}</li>
                        <li>{"iCloud: "}<a class="nice-link" href="https://appleid.apple.com/account/manage" target="_blank">{"Create App-Specific Password"}</a>{" (Sign-In and Security > App-Specific Passwords)"}</li>
                        <li>{"Yahoo: "}<a class="nice-link" href="https://login.yahoo.com/myaccount/security/app-password" target="_blank">{"Create App Password"}</a>{" (requires 2FA)"}</li>
                        <li>{"AOL: "}<a class="nice-link" href="https://login.aol.com/myaccount/security/app-password" target="_blank">{"Create App Password"}</a>{" (requires 2FA)"}</li>
                        <li>{"Outlook/Hotmail: Use regular password, or "}<a class="nice-link" href="https://account.microsoft.com/security" target="_blank">{"app password"}</a>{" if 2FA enabled"}</li>
                        <li>{"Zoho, Fastmail, GMX, Yandex: Use regular password or app password if available"}</li>
                        <li>{"Hosting providers (GoDaddy, Bluehost, etc.): Use your email password"}</li>
                    </ul>
                </div>
                <div class="info-subsection security-notice">
                    <h5>{"Security & Privacy"}</h5>
                    <p>{"Your email security is our top priority. Here's how we protect your data:"}</p>
                    <ul>
                        <li>{"Secure IMAP Connection: All email communications use TLS-encrypted IMAP connections (port 993)"}</li>
                        <li>{"Credentials Protection: Your email credentials are encrypted and stored securely"}</li>
                        <li>{"Limited Access: We only access emails when you specifically request them"}</li>
                        <li>{"No Email Storage: We don't store your emails - we fetch them on demand when you need them"}</li>
                    </ul>
                    <p class="security-recommendation">{"Note: We recommend using App Passwords when available (Gmail, iCloud, Yahoo, AOL, etc.) instead of your main account password. This provides an extra layer of security and control over access."}</p>
                </div>
            </div>
            <p class="service-description">
                {"Connect your email account using IMAP to access your emails through SMS or voice calls."}
            </p>
            if props.sub_tier.is_some() {
                // List connected accounts with per-account disconnect
                if !connected_accounts.is_empty() {
                    <div class="connected-accounts-list">
                        { for connected_accounts.iter().map(|account_email| {
                            let email_for_display = account_email.clone();
                            let email_for_disconnect = account_email.clone();
                            let connected_accounts_clone = connected_accounts.clone();
                            let error_clone = error.clone();
                            html! {
                                <div class="connected-account-row">
                                    <span class="connected-account-email">{&email_for_display}</span>
                                    <button
                                        class="disconnect-button-small"
                                        onclick={Callback::from(move |_: MouseEvent| {
                                            let email = email_for_disconnect.clone();
                                            let connected_accounts = connected_accounts_clone.clone();
                                            let error = error_clone.clone();
                                            spawn_local(async move {
                                                let payload = json!({"email": email});
                                                let request = Api::delete("/api/auth/imap/disconnect")
                                                    .header("Content-Type", "application/json")
                                                    .json(&payload)
                                                    .unwrap()
                                                    .send()
                                                    .await;
                                                match request {
                                                    Ok(response) => {
                                                        if response.ok() {
                                                            let mut accounts = (*connected_accounts).clone();
                                                            accounts.retain(|e| e != &email);
                                                            connected_accounts.set(accounts);
                                                            error.set(None);
                                                        } else {
                                                            if let Ok(error_data) = response.json::<serde_json::Value>().await {
                                                                error.set(Some(
                                                                    error_data.get("error")
                                                                        .and_then(|e| e.as_str())
                                                                        .unwrap_or("Failed to disconnect")
                                                                        .to_string()
                                                                ));
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        error.set(Some(format!("Network error: {}", e)));
                                                    }
                                                }
                                            });
                                        })}
                                    >
                                        {"Disconnect"}
                                    </button>
                                </div>
                            }
                        })}
                    </div>
                }
                // Two-step connect flow
                if detected.is_none() {
                    // Step 1: Enter email and detect provider
                    <div class="imap-form" style="display: flex; flex-wrap: wrap; gap: 10px; align-items: center; margin-top: 10px;">
                        <input
                            type="email"
                            placeholder="Email address"
                            value={(*imap_email).clone()}
                            onchange={onchange_imap_email}
                            style="flex: 2 1 250px; padding: 8px; border-radius: 4px; background-color: #2a2a2a; color: #ccc; border: 1px solid #444;"
                        />
                        <button
                            onclick={onclick_detect}
                            class="connect-button"
                            disabled={*detecting}
                            style="padding: 8px 16px; background-color: #3b82f6; color: white; border: none; border-radius: 4px; cursor: pointer;"
                        >
                            {if *detecting { "Detecting..." } else { "Next" }}
                        </button>
                    </div>
                } else {
                    // Step 2: Show detected provider + password input
                    {
                        if let Some((ref provider, ref label, ref _server, _port, ref instructions, ref instructions_url)) = *detected {
                            html! {
                                <>
                                    if !*manual_override {
                                        // Show auto-detected provider
                                        <div class="provider-detected" style="margin-top: 10px; padding: 10px 14px; background: rgba(76, 175, 80, 0.1); border: 1px solid rgba(76, 175, 80, 0.25); border-radius: 8px;">
                                            <span style="color: #4CAF50; font-weight: 500;">
                                                {format!("Detected: {}", label)}
                                            </span>
                                            <span style="color: #888; margin-left: 8px; font-size: 0.9rem;">
                                                {format!("({})", (*imap_email).clone())}
                                            </span>
                                            <button
                                                onclick={{
                                                    let manual_override = manual_override.clone();
                                                    Callback::from(move |_: MouseEvent| manual_override.set(true))
                                                }}
                                                style="float: right; background: none; border: none; color: #888; cursor: pointer; font-size: 0.85rem;"
                                            >
                                                {"Change"}
                                            </button>
                                        </div>
                                        if let Some(instr) = instructions {
                                            <p class="provider-hint">
                                                {instr.clone()}
                                                {" "}
                                                if let Some(url) = instructions_url {
                                                    <a class="nice-link" href={url.clone()} target="_blank">{"Get App Password"}</a>
                                                }
                                            </p>
                                        }
                                        if provider == "unknown" {
                                            <p class="provider-hint">
                                                {"Provider not detected. Enter your IMAP server details, or click Change to select your provider."}
                                            </p>
                                            <div class="imap-form" style="display: flex; flex-wrap: wrap; gap: 10px; align-items: center; margin-top: 6px;">
                                                <input
                                                    type="text"
                                                    placeholder="IMAP Server (e.g., imap.example.com)"
                                                    value={(*imap_server).clone()}
                                                    onchange={onchange_imap_server.clone()}
                                                    style="flex: 2 1 200px; padding: 8px; border-radius: 4px; background-color: #2a2a2a; color: #ccc; border: 1px solid #444;"
                                                />
                                                <input
                                                    type="number"
                                                    placeholder="Port (993)"
                                                    value={(*imap_port).clone()}
                                                    onchange={onchange_imap_port.clone()}
                                                    style="flex: 1 1 80px; padding: 8px; border-radius: 4px; background-color: #2a2a2a; color: #ccc; border: 1px solid #444;"
                                                />
                                            </div>
                                        }
                                    } else {
                                        // Manual provider override
                                        <div style="margin-top: 10px;">
                                            <div style="display: flex; align-items: center; gap: 8px; margin-bottom: 8px;">
                                                <span style="color: #888; font-size: 0.9rem;">
                                                    {format!("({})", (*imap_email).clone())}
                                                </span>
                                                <button
                                                    onclick={{
                                                        let manual_override = manual_override.clone();
                                                        Callback::from(move |_: MouseEvent| manual_override.set(false))
                                                    }}
                                                    style="background: none; border: none; color: #3b82f6; cursor: pointer; font-size: 0.85rem;"
                                                >
                                                    {"Use auto-detected"}
                                                </button>
                                            </div>
                                            <div class="imap-form" style="display: flex; flex-wrap: wrap; gap: 10px; align-items: center;">
                                                <select
                                                    onchange={{
                                                        let manual_provider = manual_provider.clone();
                                                        let imap_server = imap_server.clone();
                                                        let imap_port = imap_port.clone();
                                                        let providers = providers.clone();
                                                        Callback::from(move |e: Event| {
                                                            let select: web_sys::HtmlSelectElement = e.target_unchecked_into();
                                                            let value = select.value();
                                                            manual_provider.set(value.clone());
                                                            if let Some((_, _, server, port)) = providers.iter().find(|(id, _, _, _)| *id == value) {
                                                                imap_server.set(server.to_string());
                                                                imap_port.set(port.to_string());
                                                            } else {
                                                                imap_server.set(String::new());
                                                                imap_port.set(String::new());
                                                            }
                                                        })
                                                    }}
                                                    style="flex: 1 1 150px; padding: 8px; border-radius: 4px; background-color: #2a2a2a; color: #ccc; border: 1px solid #444; appearance: none;"
                                                >
                                                    { for providers.iter().map(|(id, name, _, _)| {
                                                        html! {
                                                            <option value={id.to_string()} selected={*manual_provider == *id}>
                                                                {name}
                                                            </option>
                                                        }
                                                    })}
                                                </select>
                                            </div>
                                            if *manual_provider == "custom" {
                                                <div class="imap-form" style="display: flex; flex-wrap: wrap; gap: 10px; align-items: center; margin-top: 6px;">
                                                    <input
                                                        type="text"
                                                        placeholder="IMAP Server (e.g., imap.example.com)"
                                                        value={(*imap_server).clone()}
                                                        onchange={onchange_imap_server.clone()}
                                                        style="flex: 2 1 200px; padding: 8px; border-radius: 4px; background-color: #2a2a2a; color: #ccc; border: 1px solid #444;"
                                                    />
                                                    <input
                                                        type="number"
                                                        placeholder="Port (993)"
                                                        value={(*imap_port).clone()}
                                                        onchange={onchange_imap_port.clone()}
                                                        style="flex: 1 1 80px; padding: 8px; border-radius: 4px; background-color: #2a2a2a; color: #ccc; border: 1px solid #444;"
                                                    />
                                                </div>
                                            }
                                        </div>
                                    }
                                    <div class="imap-form" style="display: flex; flex-wrap: wrap; gap: 10px; align-items: center; margin-top: 8px;">
                                        <input
                                            type="password"
                                            placeholder="Password or App Password"
                                            value={(*imap_password).clone()}
                                            onchange={onchange_imap_password}
                                            style="flex: 2 1 250px; padding: 8px; border-radius: 4px; background-color: #2a2a2a; color: #ccc; border: 1px solid #444;"
                                        />
                                        <button
                                            onclick={onclick_imap_connect}
                                            class="connect-button"
                                            style="padding: 8px 16px; background-color: #3b82f6; color: white; border: none; border-radius: 4px; cursor: pointer;"
                                        >
                                            {"Connect"}
                                        </button>
                                    </div>
                                </>
                            }
                        } else {
                            html! {}
                        }
                    }
                }
            if let Some(err) = (*error).as_ref() {
                <div class="error-message">
                    {err}
                </div>
            }
        } else {
            <div class="upgrade-prompt">
                <div class="upgrade-content">
                    <h3>{"Upgrade to Enable Email Integration"}</h3>
                    <a href="/pricing" class="upgrade-button">
                        {"View Pricing Plans"}
                    </a>
                </div>
            </div>
        }
            <style>

                {r#"
                    .connected-accounts-list {
                        display: flex;
                        flex-direction: column;
                        gap: 8px;
                        margin: 10px 0;
                    }
                    .connected-account-row {
                        display: flex;
                        align-items: center;
                        justify-content: space-between;
                        padding: 8px 12px;
                        background: rgba(0, 136, 204, 0.08);
                        border: 1px solid rgba(0, 136, 204, 0.15);
                        border-radius: 8px;
                    }
                    .connected-account-email {
                        color: #ccc;
                        font-size: 0.95rem;
                    }
                    .disconnect-button-small {
                        background: transparent;
                        border: 1px solid rgba(255, 99, 71, 0.3);
                        color: #FF6347;
                        padding: 4px 12px;
                        border-radius: 6px;
                        cursor: pointer;
                        font-size: 0.85rem;
                        transition: all 0.3s ease;
                    }
                    .disconnect-button-small:hover {
                        background: rgba(255, 99, 71, 0.1);
                        border-color: rgba(255, 99, 71, 0.5);
                    }
                    .service-item {
                        background: rgba(0, 0, 0, 0.2);
                        border: 1px solid rgba(0, 136, 204, 0.2);
                        border-radius: 12px;
                        width: 100%;
                        padding: 1.5rem;
                        margin: 1rem 0;
                        transition: all 0.3s ease;
                        color: #fff;
                    }
                    .service-item:hover {
                        transform: translateY(-2px);
                        border-color: rgba(0, 136, 204, 0.4);
                        box-shadow: 0 4px 20px rgba(0, 136, 204, 0.1);
                    }
                    .service-header {
                        display: flex;
                        align-items: center;
                        gap: 1rem;
                        flex-wrap: wrap;
                    }
                    .service-name {
                        flex: 1;
                        min-width: 150px;
                    }
                    .service-status {
                        white-space: nowrap;
                    }
                    .service-name {
                        display: flex;
                        align-items: center;
                        gap: 0.5rem;
                    }
                    .service-name img {
                        width: 24px !important;
                        height: 24px !important;
                    }
                    .service-status {
                        color: #4CAF50;
                        font-weight: 500;
                    }
                    .provider-hint {
                        background: rgba(0, 136, 204, 0.1);
                        border: 1px solid rgba(0, 136, 204, 0.2);
                        border-radius: 8px;
                        padding: 0.8rem 1rem;
                        margin: 0.5rem 0 1rem 0;
                        color: #CCC;
                        font-size: 0.9rem;
                    }
                    .provider-hint a {
                        color: #0088cc;
                    }
                    .info-button {
                        background: none;
                        border: none;
                        color: #0088cc;
                        font-size: 1.2rem;
                        cursor: pointer;
                        padding: 0.5rem;
                        border-radius: 50%;
                        width: 2rem;
                        height: 2rem;
                        display: flex;
                        align-items: center;
                        justify-content: center;
                        transition: all 0.3s ease;
                        margin-left: auto;
                    }
                    .info-button:hover {
                        background: rgba(0, 136, 204, 0.1);
                        transform: scale(1.1);
                    }
                    .auth-form-container {
                        margin: 1.5rem 0;
                    }
                    .auth-instructions {
                        color: #CCC;
                        margin-bottom: 1rem;
                        padding-left: 1.5rem;
                    }
                    .auth-instructions li {
                        margin-bottom: 0.5rem;
                    }
                    .curl-textarea {
                        width: 100%;
                        height: 150px;
                        background: rgba(0, 0, 0, 0.3);
                        border: 1px solid rgba(255, 255, 255, 0.1);
                        border-radius: 8px;
                        color: #fff;
                        padding: 1rem;
                        font-family: monospace;
                        margin-bottom: 1rem;
                    }
                    .submit-button {
                        background: #0088cc;
                        color: white;
                        border: none;
                        padding: 0.8rem 1.5rem;
                        border-radius: 8px;
                        cursor: pointer;
                        transition: all 0.3s ease;
                        width: 100%;
                    }
                    .submit-button:hover {
                        background: #0077b3;
                        transform: translateY(-2px);
                        box-shadow: 0 4px 12px rgba(0, 136, 204, 0.3);
                    }
                    .auth-note {
                        color: #999;
                        font-size: 0.9rem;
                        margin-top: 1rem;
                    }
                    .loading-container {
                        text-align: center;
                        margin: 2rem 0;
                    }
                    .loading-spinner {
                        display: inline-block;
                        width: 40px;
                        height: 40px;
                        border: 4px solid rgba(0, 136, 204, 0.1);
                        border-radius: 50%;
                        border-top-color: #0088cc;
                        animation: spin 1s ease-in-out infinite;
                        margin: 1rem auto;
                    }
                    .button-group {
                        display: flex;
                        flex-direction: column;
                        gap: 1rem;
                        margin-bottom: 1rem;
                    }
                    @media (min-width: 768px) {
                        .button-group {
                            flex-direction: row;
                        }
                    }
                    .resync-button {
                        background: linear-gradient(45deg, #0088cc, #0099dd);
                        color: white;
                        border: none;
                        padding: 0.8rem 1.5rem;
                        border-radius: 8px;
                        cursor: pointer;
                        transition: all 0.3s ease;
                        flex: 1;
                    }
                    .resync-button:hover {
                        transform: translateY(-2px);
                        box-shadow: 0 4px 20px rgba(0, 136, 204, 0.3);
                    }
                    .connect-button, .disconnect-button {
                        background: #0088cc;
                        color: white;
                        border: none;
                        padding: 0.8rem 1.5rem;
                        border-radius: 8px;
                        cursor: pointer;
                        transition: all 0.3s ease;
                        margin-top: 1rem;
                    }
                    .disconnect-button {
                        background: transparent;
                        border: 1px solid rgba(255, 99, 71, 0.3);
                        color: #FF6347;
                    }
                    .disconnect-button:hover {
                        background: rgba(255, 99, 71, 0.1);
                        border-color: rgba(255, 99, 71, 0.5);
                        transform: translateY(-2px);
                    }
                    .connect-button:hover {
                        background: #0077b3;
                        transform: translateY(-2px);
                        box-shadow: 0 4px 12px rgba(0, 136, 204, 0.3);
                    }
                    .error-message {
                        color: #FF4B4B;
                        background: rgba(255, 75, 75, 0.1);
                        border: 1px solid rgba(255, 75, 75, 0.2);
                        border-radius: 8px;
                        padding: 1rem;
                        margin-top: 1rem;
                    }
                    .success-banner {
                        color: #4CAF50;
                        background: rgba(76, 175, 80, 0.1);
                        border: 1px solid rgba(76, 175, 80, 0.3);
                        border-radius: 8px;
                        padding: 1rem;
                        margin-bottom: 1rem;
                        text-align: center;
                        font-weight: 500;
                    }
                    .sync-indicator {
                        display: flex;
                        align-items: center;
                        background: rgba(0, 136, 204, 0.1);
                        border-radius: 8px;
                        padding: 1rem;
                        margin-bottom: 1rem;
                        color: #0088cc;
                    }
                    .sync-spinner {
                        display: inline-block;
                        width: 20px;
                        height: 20px;
                        border: 3px solid rgba(0, 136, 204, 0.1);
                        border-radius: 50%;
                        border-top-color: #0088cc;
                        animation: spin 1s ease-in-out infinite;
                        margin-right: 10px;
                    }
                    .test-button {
                        background: linear-gradient(45deg, #4CAF50, #45a049);
                        color: white;
                        border: none;
                        width: 100%;
                        padding: 1rem;
                        border-radius: 8px;
                        font-size: 1rem;
                        cursor: pointer;
                        transition: all 0.3s ease;
                        margin-top: 1rem;
                    }
                    .test-button:hover {
                        transform: translateY(-2px);
                        box-shadow: 0 4px 20px rgba(76, 175, 80, 0.3);
                    }
                    .test-send-button {
                        background: linear-gradient(45deg, #FF8C00, #FFA500);
                        margin-top: 0.5rem;
                    }
                    .test-send-button:hover {
                        box-shadow: 0 4px 20px rgba(255, 140, 0, 0.3);
                    }
                    .test-search-button {
                        background: linear-gradient(45deg, #9C27B0, #BA68C8);
                        margin-top: 0.5rem;
                    }
                    .test-search-button:hover {
                        box-shadow: 0 4px 20px rgba(156, 39, 176, 0.3);
                    }
                    .upgrade-prompt {
                        background: rgba(0, 136, 204, 0.05);
                        border: 1px solid rgba(0, 136, 204, 0.1);
                        border-radius: 12px;
                        padding: 1.8rem;
                        text-align: center;
                        margin: 0.8rem 0;
                    }
                    .upgrade-content h3 {
                        color: #0088cc;
                        margin-bottom: 1rem;
                        font-size: 1.2rem;
                    }
                    .upgrade-button {
                        display: inline-block;
                        background: #0088cc;
                        color: white;
                        text-decoration: none;
                        padding: 1rem 2rem;
                        border-radius: 8px;
                        font-weight: bold;
                        transition: all 0.3s ease;
                        margin-top: 1rem;
                    }
                    .upgrade-button:hover {
                        background: #0077b3;
                        transform: translateY(-2px);
                        box-shadow: 0 4px 12px rgba(0, 136, 204, 0.3);
                    }
                    @keyframes spin {
                        to { transform: rotate(360deg); }
                    }
                    .security-notice {
                        background: rgba(0, 136, 204, 0.1);
                        padding: 1.2rem;
                        border-radius: 8px;
                        border: 1px solid rgba(0, 136, 204, 0.2);
                    }
                    .security-notice p {
                        margin: 0 0 1rem 0;
                        color: #CCC;
                    }
                    .security-recommendation {
                        font-style: italic;
                        color: #999 !important;
                        margin-top: 1rem !important;
                        font-size: 0.9rem;
                        padding-top: 1rem;
                        border-top: 1px solid rgba(0, 136, 204, 0.1);
                    }
                    .modal-overlay {
                        position: fixed;
                        top: 0;
                        left: 0;
                        right: 0;
                        bottom: 0;
                        background: rgba(0, 0, 0, 0.85);
                        display: flex;
                        justify-content: center;
                        align-items: center;
                        z-index: 1000;
                    }
                    .modal-content {
                        background: #1a1a1a;
                        border: 1px solid rgba(0, 136, 204, 0.2);
                        border-radius: 12px;
                        padding: 2rem;
                        max-width: 500px;
                        width: 90%;
                        box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
                    }
                    .modal-content h3 {
                        color: #FF6347;
                        margin-bottom: 1rem;
                    }
                    .modal-content p {
                        color: #CCC;
                        margin-bottom: 1rem;
                    }
                    .modal-content ul {
                        margin-bottom: 2rem;
                        padding-left: 1.5rem;
                    }
                    .modal-content li {
                        color: #999;
                        margin-bottom: 0.5rem;
                    }
                    .modal-buttons {
                        display: flex;
                        gap: 1rem;
                        justify-content: flex-end;
                    }
                    .cancel-button {
                        background: transparent;
                        border: 1px solid rgba(204, 204, 204, 0.3);
                        color: #CCC;
                        padding: 0.8rem 1.5rem;
                        border-radius: 8px;
                        cursor: pointer;
                        transition: all 0.3s ease;
                    }
                    .cancel-button:hover {
                        background: rgba(204, 204, 204, 0.1);
                        transform: translateY(-2px);
                    }
                    .confirm-disconnect-button {
                        background: linear-gradient(45deg, #FF6347, #FF4500);
                        color: white;
                        border: none;
                        padding: 0.8rem 1.5rem;
                        border-radius: 8px;
                        cursor: pointer;
                        transition: all 0.3s ease;
                    }
                    .confirm-disconnect-button:hover {
                        transform: translateY(-2px);
                        box-shadow: 0 4px 12px rgba(255, 99, 71, 0.3);
                    }
                    .button-spinner {
                        display: inline-block;
                        width: 16px;
                        height: 16px;
                        border: 2px solid rgba(255, 255, 255, 0.3);
                        border-radius: 50%;
                        border-top-color: #fff;
                        animation: spin 1s ease-in-out infinite;
                        margin-right: 8px;
                        vertical-align: middle;
                    }
                    .disconnecting-message {
                        color: #0088cc;
                        margin: 1rem 0;
                        font-weight: bold;
                    }
                "#}
            </style>
        </div>
    }
}
