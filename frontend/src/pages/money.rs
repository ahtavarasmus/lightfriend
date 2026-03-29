use crate::utils::api::Api;
use crate::Route;
use serde::Deserialize;
use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;
use wasm_bindgen_futures;
use web_sys::window;
use yew::prelude::*;
use yew_router::prelude::Link;

/// Check if a country is notification-only (receives SMS from US number, dynamic pricing)
/// Any country that's not a local-number country is notification-only
fn is_notification_only_country(country: &str) -> bool {
    // Local number countries have dedicated Twilio numbers
    !matches!(country, "US" | "CA" | "FI" | "NL" | "GB" | "AU")
}

/// Check if country has local number (can receive inbound calls)
fn is_local_number_country(country: &str) -> bool {
    matches!(country, "FI" | "NL" | "GB" | "AU")
}

/// Message equivalent display that fetches real pricing from backend
#[derive(Properties, PartialEq)]
pub struct MessageEquivalentProps {
    pub plan_messages: i32, // 50 or 150 (the "messages" in the plan)
    pub country: String,
}

#[function_component(MessageEquivalentDisplay)]
pub fn message_equivalent_display(props: &MessageEquivalentProps) -> Html {
    let current_view = use_state(|| 0usize);
    let pricing = use_state(|| None::<ByotPricingResponse>);
    let loading = use_state(|| true);

    // Fetch pricing for this country
    {
        let pricing = pricing.clone();
        let loading = loading.clone();
        let country = props.country.clone();
        use_effect_with_deps(
            move |country| {
                let country = country.clone();
                // Set loading state immediately when country changes
                loading.set(true);
                wasm_bindgen_futures::spawn_local(async move {
                    let response = Api::get(&format!("/api/pricing/byot/{}", country))
                        .send()
                        .await;
                    if let Ok(resp) = response {
                        if resp.ok() {
                            if let Ok(data) = resp.json::<ByotPricingResponse>().await {
                                pricing.set(Some(data));
                            }
                        }
                    }
                    loading.set(false);
                });
                || ()
            },
            country,
        );
    }

    let is_local = is_local_number_country(&props.country);
    let is_notification_only = is_notification_only_country(&props.country);

    // Calculate equivalents based on actual pricing
    let views: Vec<String> = if let Some(ref p) = *pricing {
        if let Some(sms_price) = p.costs.sms_per_segment {
            // Credits = 3 × sms_price × plan_messages
            let total_credits = 3.0 * sms_price * (props.plan_messages as f32);

            // Normal response = 3 segments
            let normal_responses = (total_credits / (3.0 * sms_price)).floor() as i32;

            // Notification = 1.5 segments
            let notifications = (total_credits / (1.5 * sms_price)).floor() as i32;

            // Digest = 3 segments (same as response)
            let digests = (total_credits / (3.0 * sms_price)).floor() as i32;

            // Voice outbound (if available)
            let voice_calls = p
                .costs
                .voice_outbound_per_min
                .map(|v| (total_credits / v).floor() as i32);

            // Inbound voice (if local number country)
            let inbound_mins = if is_local {
                p.costs
                    .voice_inbound_per_min
                    .map(|v| (total_credits / v).floor() as i32)
            } else {
                None
            };

            let mut v = Vec::new();

            // Only show responses for countries that can reply (not notification-only)
            if !is_notification_only {
                v.push(format!("{} responses", normal_responses));
            }

            v.push(format!("{} notifications", notifications));
            v.push(format!("{} digests", digests));

            if let Some(vc) = voice_calls {
                v.push(format!("{} voice mins out", vc));
            }

            if let Some(im) = inbound_mins {
                v.push(format!("{} voice mins in", im));
            }

            v
        } else {
            vec![format!("{} messages", props.plan_messages)]
        }
    } else {
        vec![format!("{} messages", props.plan_messages)]
    };

    let total_views = views.len();
    let is_loading = *loading;

    let onclick = {
        let current_view = current_view.clone();
        let loading = loading.clone();
        Callback::from(move |_: MouseEvent| {
            // Only cycle if not loading
            if !*loading {
                current_view.set((*current_view + 1) % total_views);
            }
        })
    };

    let css = r#"
    .message-equivalent {
        cursor: pointer;
        padding: 0.5rem 0.75rem;
        background: rgba(255, 255, 255, 0.08);
        border: 1px solid rgba(255, 255, 255, 0.2);
        border-radius: 8px;
        transition: all 0.2s ease;
        text-align: center;
        user-select: none;
    }
    .message-equivalent:hover {
        background: rgba(255, 255, 255, 0.15);
        border-color: rgba(255, 255, 255, 0.3);
    }
    .message-equivalent .value {
        font-size: 1.1rem;
        font-weight: 600;
        color: rgba(255, 255, 255, 0.7);
    }
    .message-equivalent .hint {
        font-size: 0.75rem;
        color: #888;
        margin-top: 0.25rem;
    }
    .message-equivalent.loading .value {
        color: #888;
    }
    .message-equivalent.loading {
        cursor: default;
    }
    "#;

    let display_value = if is_loading {
        format!("{} messages", props.plan_messages)
    } else {
        views
            .get(*current_view)
            .cloned()
            .unwrap_or_else(|| format!("{} messages", props.plan_messages))
    };

    let hint_text = if is_loading {
        "loading pricing..."
    } else {
        "tap to see equivalents"
    };

    html! {
        <>
            <style>{css}</style>
            <div class={classes!("message-equivalent", if is_loading { "loading" } else { "" })} onclick={onclick}>
                <div class="value">{display_value}</div>
                <div class="hint">{hint_text}</div>
            </div>
        </>
    }
}

/// BYOT pricing response from backend
#[derive(Clone, PartialEq, Deserialize)]
struct ByotPricingResponse {
    country_code: String,
    country_name: String,
    has_local_numbers: bool,
    monthly_number_cost: Option<f32>,
    costs: ByotMessageCosts,
}

#[derive(Clone, PartialEq, Deserialize)]
struct ByotMessageCosts {
    sms_per_segment: Option<f32>,
    notification: Option<f32>,
    normal_response: Option<f32>,
    digest: Option<f32>,
    voice_outbound_per_min: Option<f32>,
    voice_inbound_per_min: Option<f32>,
}

#[function_component(ByotPricingDisplay)]
pub fn byot_pricing_display() -> Html {
    let selected_country = use_state(|| "DE".to_string());
    let pricing = use_state(|| None::<ByotPricingResponse>);
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);

    {
        let pricing = pricing.clone();
        let loading = loading.clone();
        let error = error.clone();
        let country = (*selected_country).clone();
        use_effect_with_deps(
            move |country| {
                let country = country.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    loading.set(true);
                    error.set(None);
                    let response = Api::get(&format!("/api/pricing/byot/{}", country))
                        .send()
                        .await;
                    match response {
                        Ok(resp) if resp.ok() => match resp.json::<ByotPricingResponse>().await {
                            Ok(data) => pricing.set(Some(data)),
                            Err(_) => error.set(Some("Failed to parse pricing".to_string())),
                        },
                        Ok(_) => error.set(Some("Country not supported".to_string())),
                        Err(e) => error.set(Some(format!("Failed to fetch: {}", e))),
                    }
                    loading.set(false);
                });
                || ()
            },
            country,
        );
    }

    let on_country_change = {
        let selected_country = selected_country.clone();
        Callback::from(move |e: Event| {
            let target: web_sys::HtmlSelectElement = e.target_unchecked_into();
            selected_country.set(target.value());
        })
    };

    let css = r#"
    .byot-pricing {
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.15);
        border-radius: 12px;
        padding: 1rem;
        margin: 1rem 0;
    }
    .byot-pricing h4 {
        color: rgba(255, 255, 255, 0.7);
        font-size: 1rem;
        margin: 0 0 0.75rem 0;
        text-align: center;
    }
    .byot-pricing-grid {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 0.5rem;
    }
    .byot-pricing-item {
        display: flex;
        justify-content: space-between;
        padding: 0.25rem 0;
        font-size: 0.85rem;
    }
    .byot-pricing-item .label {
        color: #b0b0b0;
    }
    .byot-pricing-item .value {
        color: rgba(255, 255, 255, 0.7);
        font-weight: 500;
    }
    .byot-monthly {
        text-align: center;
        padding: 0.5rem;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 8px;
        margin-bottom: 0.75rem;
    }
    .byot-monthly .value {
        font-size: 1.1rem;
        color: rgba(255, 255, 255, 0.7);
        font-weight: 600;
    }
    .byot-monthly .label {
        font-size: 0.75rem;
        color: #888;
    }
    .byot-loading, .byot-error {
        text-align: center;
        padding: 1rem;
        color: #888;
    }
    .byot-error {
        color: #ff6b6b;
    }
    .byot-country-selector {
        margin-bottom: 0.75rem;
    }
    .byot-country-selector select {
        width: 100%;
        padding: 0.5rem;
        background: rgba(30, 30, 30, 0.9);
        border: 1px solid rgba(255, 255, 255, 0.2);
        border-radius: 6px;
        color: #e0e0e0;
        font-size: 0.85rem;
        cursor: pointer;
    }
    .byot-country-selector select:focus {
        outline: none;
        border-color: rgba(255, 255, 255, 0.35);
    }
    "#;

    let current_country = (*selected_country).clone();

    html! {
        <>
            <style>{css}</style>
            <div class="byot-pricing">
                <h4>{"Twilio Pricing Preview"}</h4>
                <div class="byot-country-selector">
                    <select onchange={on_country_change} value={current_country.clone()}>
                        <optgroup label="Europe">
                            <option value="DE" selected={current_country == "DE"}>{"Germany"}</option>
                            <option value="FR" selected={current_country == "FR"}>{"France"}</option>
                            <option value="ES" selected={current_country == "ES"}>{"Spain"}</option>
                            <option value="IT" selected={current_country == "IT"}>{"Italy"}</option>
                            <option value="BE" selected={current_country == "BE"}>{"Belgium"}</option>
                            <option value="AT" selected={current_country == "AT"}>{"Austria"}</option>
                            <option value="CH" selected={current_country == "CH"}>{"Switzerland"}</option>
                            <option value="SE" selected={current_country == "SE"}>{"Sweden"}</option>
                            <option value="NO" selected={current_country == "NO"}>{"Norway"}</option>
                            <option value="DK" selected={current_country == "DK"}>{"Denmark"}</option>
                            <option value="IE" selected={current_country == "IE"}>{"Ireland"}</option>
                            <option value="PT" selected={current_country == "PT"}>{"Portugal"}</option>
                            <option value="PL" selected={current_country == "PL"}>{"Poland"}</option>
                            <option value="CZ" selected={current_country == "CZ"}>{"Czech Republic"}</option>
                            <option value="GR" selected={current_country == "GR"}>{"Greece"}</option>
                            <option value="HU" selected={current_country == "HU"}>{"Hungary"}</option>
                            <option value="RO" selected={current_country == "RO"}>{"Romania"}</option>
                            <option value="SK" selected={current_country == "SK"}>{"Slovakia"}</option>
                            <option value="BG" selected={current_country == "BG"}>{"Bulgaria"}</option>
                            <option value="HR" selected={current_country == "HR"}>{"Croatia"}</option>
                            <option value="SI" selected={current_country == "SI"}>{"Slovenia"}</option>
                            <option value="LT" selected={current_country == "LT"}>{"Lithuania"}</option>
                            <option value="LV" selected={current_country == "LV"}>{"Latvia"}</option>
                            <option value="EE" selected={current_country == "EE"}>{"Estonia"}</option>
                            <option value="LU" selected={current_country == "LU"}>{"Luxembourg"}</option>
                            <option value="MT" selected={current_country == "MT"}>{"Malta"}</option>
                            <option value="CY" selected={current_country == "CY"}>{"Cyprus"}</option>
                            <option value="IS" selected={current_country == "IS"}>{"Iceland"}</option>
                        </optgroup>
                        <optgroup label="Asia-Pacific">
                            <option value="NZ" selected={current_country == "NZ"}>{"New Zealand"}</option>
                            <option value="JP" selected={current_country == "JP"}>{"Japan"}</option>
                            <option value="KR" selected={current_country == "KR"}>{"South Korea"}</option>
                            <option value="SG" selected={current_country == "SG"}>{"Singapore"}</option>
                            <option value="HK" selected={current_country == "HK"}>{"Hong Kong"}</option>
                            <option value="TW" selected={current_country == "TW"}>{"Taiwan"}</option>
                        </optgroup>
                        <optgroup label="Middle East">
                            <option value="IL" selected={current_country == "IL"}>{"Israel"}</option>
                        </optgroup>
                    </select>
                </div>
                if *loading {
                    <div class="byot-loading">{"Loading..."}</div>
                } else if let Some(err) = (*error).as_ref() {
                    <div class="byot-error">{err}</div>
                } else if let Some(data) = (*pricing).as_ref() {
                    <>
                        if let Some(monthly) = data.monthly_number_cost {
                            <div class="byot-monthly">
                                <div class="value">{format!("${:.2}/mo", monthly)}</div>
                                <div class="label">{"Phone number"}</div>
                            </div>
                        }
                        <div class="byot-pricing-grid">
                            if let Some(p) = data.costs.notification {
                                <div class="byot-pricing-item">
                                    <span class="label">{"Notification"}</span>
                                    <span class="value">{format!("${:.3}", p)}</span>
                                </div>
                            }
                            if let Some(p) = data.costs.normal_response {
                                <div class="byot-pricing-item">
                                    <span class="label">{"Response"}</span>
                                    <span class="value">{format!("${:.3}", p)}</span>
                                </div>
                            }
                            if let Some(p) = data.costs.digest {
                                <div class="byot-pricing-item">
                                    <span class="label">{"Digest"}</span>
                                    <span class="value">{format!("${:.3}", p)}</span>
                                </div>
                            }
                            if let Some(p) = data.costs.voice_outbound_per_min {
                                <div class="byot-pricing-item">
                                    <span class="label">{"Voice/min"}</span>
                                    <span class="value">{format!("${:.3}", p)}</span>
                                </div>
                            }
                        </div>
                        if !data.has_local_numbers {
                            <p style="font-size: 0.75rem; color: #888; margin-top: 0.5rem; text-align: center;">
                                {"No local number in this country"}
                            </p>
                        }
                    </>
                }
            </div>
        </>
    }
}

#[derive(Clone, PartialEq)]
pub struct Feature {
    pub text: String,
    pub sub_items: Vec<String>,
}
#[derive(Properties, PartialEq)]
pub struct PricingProps {
    #[prop_or_default]
    pub user_id: i32,
    #[prop_or_default]
    pub user_email: String,
    #[prop_or_default]
    pub sub_tier: Option<String>,
    #[prop_or_default]
    pub user_plan_type: Option<String>,
    #[prop_or_default]
    pub is_logged_in: bool,
    #[prop_or_default]
    pub phone_number: Option<String>,
    #[prop_or_default]
    pub selected_country: String,
    #[prop_or_default]
    pub country_name: String,
    #[prop_or_default]
    pub on_country_change: Option<Callback<Event>>,
}
#[derive(Properties, PartialEq, Clone)]
pub struct CheckoutButtonProps {
    pub user_id: i32,
    pub user_email: String,
    pub subscription_type: String,
    pub selected_country: String,
    #[prop_or_default]
    pub plan_type: Option<String>,
}
#[function_component(CheckoutButton)]
pub fn checkout_button(props: &CheckoutButtonProps) -> Html {
    let user_id = props.user_id;
    let selected_country = props.selected_country.clone();
    let plan_type = props.plan_type.clone();

    // Check if subscriptions are blocked
    let subscriptions_blocked = false;

    let onclick = {
        let user_id = user_id.clone();
        let selected_country = selected_country.clone();
        let plan_type = plan_type.clone();

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let user_id = user_id.clone();
            let selected_country = selected_country.clone();
            let plan_type = plan_type.clone();

            // BYOT plan warning - triggers for "Other" country or explicit BYOT plan selection
            if selected_country == "Other" || plan_type.as_deref() == Some("byot") {
                if let Some(window) = web_sys::window() {
                    if !window.confirm_with_message(
                        "BYOT Plan Notice:\n\nBefore subscribing, please verify you can purchase a Twilio phone number for your country.\n\n- Some countries only allow business accounts\n- Number availability varies by region\n- Check Twilio's phone number search first\n\nContinue to checkout?"
                    ).unwrap_or(false) {
                        return;
                    }
                }
            }

            wasm_bindgen_futures::spawn_local(async move {
                let endpoint = format!("/api/stripe/unified-subscription-checkout/{}", user_id);
                let mut request_body = json!({
                    "subscription_type": "Hosted",
                });
                if let Some(pt) = plan_type {
                    request_body["plan_type"] = json!(pt);
                }
                let response = Api::post(&endpoint)
                    .header("Content-Type", "application/json")
                    .body(request_body.to_string())
                    .send()
                    .await;
                match response {
                    Ok(resp) => {
                        if let Ok(json) = resp.json::<Value>().await {
                            if let Some(url) = json.get("url").and_then(|u| u.as_str()) {
                                if let Some(window) = window() {
                                    let _ = window.location().set_href(url);
                                }
                            }
                        }
                    }
                    Err(_) => {}
                }
            });
        })
    };
    let button_text = if subscriptions_blocked {
        "Temporarily Unavailable"
    } else {
        "Subscribe"
    };
    let button_css = r#"
    .iq-button {
        background: linear-gradient(135deg, #d4d4d4, #a8a8a8 30%, #e8e8e8 50%, #a8a8a8 70%, #c0c0c0);
        color: #1a1a2e;
        border: none;
        padding: 1rem 2rem;
        border-radius: 8px;
        font-size: 1rem;
        cursor: pointer;
        transition: all 0.3s ease;
        border: 1px solid rgba(255, 255, 255, 0.1);
        width: 100%;
        margin-top: 2rem;
        text-decoration: none;
    }
    .iq-button:hover {
        background: linear-gradient(135deg, #e0e0e0, #b8b8b8 30%, #f0f0f0 50%, #b8b8b8 70%, #d0d0d0);
    }
    .iq-button.disabled {
        background: rgba(30, 30, 30, 0.5);
        cursor: not-allowed;
        border: 1px solid rgba(255, 255, 255, 0.1);
        opacity: 0.6;
    }
    .iq-button.disabled:hover {
        background: rgba(30, 30, 30, 0.5);
    }
    .iq-button.current-plan {
        background: rgba(255, 255, 255, 0.2);
        border: 1px solid rgba(255, 255, 255, 0.3);
        cursor: default;
    }
    .iq-button.current-plan:hover {
        background: rgba(255, 255, 255, 0.2);
    }
    .iq-button.coming-soon {
        background: rgba(255, 165, 0, 0.3);
        border: 1px solid rgba(255, 165, 0, 0.5);
        cursor: default;
    }
    .iq-button.coming-soon:hover {
    }
    "#;
    html! {
        <>
            <style>{button_css}</style>
            if subscriptions_blocked {
                <button class="iq-button disabled" disabled=true><b>{button_text}</b></button>
            } else {
                <button class="iq-button signup-button" {onclick}><b>{button_text}</b></button>
            }
        </>
    }
}

#[derive(Deserialize)]
struct GuestCheckoutResponse {
    url: String,
}

#[derive(Properties, PartialEq)]
pub struct GuestCheckoutButtonProps {
    pub subscription_type: String,
    pub selected_country: String,
    #[prop_or_default]
    pub plan_type: Option<String>,
}

#[function_component(GuestCheckoutButton)]
pub fn guest_checkout_button(props: &GuestCheckoutButtonProps) -> Html {
    let loading = use_state(|| false);
    let error = use_state(|| None::<String>);
    let terms_accepted = use_state(|| false);
    let subscription_type = props.subscription_type.clone();
    let selected_country = props.selected_country.clone();
    let plan_type = props.plan_type.clone();

    let onclick = {
        let loading = loading.clone();
        let error = error.clone();
        let terms_accepted = terms_accepted.clone();
        let subscription_type = subscription_type.clone();
        let selected_country = selected_country.clone();
        let plan_type = plan_type.clone();

        Callback::from(move |e: web_sys::MouseEvent| {
            e.prevent_default();

            if !*terms_accepted {
                error.set(Some("Please accept the terms and conditions".to_string()));
                return;
            }

            // BYOT plan warning - triggers for "Other" country or explicit BYOT plan selection
            if selected_country == "Other" || plan_type.as_deref() == Some("byot") {
                if let Some(window) = web_sys::window() {
                    if !window.confirm_with_message(
                        "BYOT Plan Notice:\n\nBefore subscribing, please verify you can purchase a Twilio phone number for your country.\n\n- Some countries only allow business accounts\n- Number availability varies by region\n- Check Twilio's phone number search first\n\nContinue to checkout?"
                    ).unwrap_or(false) {
                        return;
                    }
                }
            }

            let loading = loading.clone();
            let error = error.clone();
            let subscription_type = subscription_type.clone();
            let selected_country = selected_country.clone();
            let plan_type = plan_type.clone();

            loading.set(true);
            error.set(None);

            wasm_bindgen_futures::spawn_local(async move {
                let mut body = json!({
                    "subscription_type": match subscription_type.as_str() {
                        "hosted" => "Hosted",
                        _ => &subscription_type
                    },
                    "selected_country": selected_country
                });
                if let Some(pt) = plan_type {
                    body["plan_type"] = json!(pt);
                }
                match Api::post("/api/stripe/guest-checkout")
                    .json(&body)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.ok() {
                            match response.json::<GuestCheckoutResponse>().await {
                                Ok(checkout_response) => {
                                    if let Some(window) = window() {
                                        let _ = window.location().set_href(&checkout_response.url);
                                    }
                                }
                                Err(_) => {
                                    loading.set(false);
                                    error
                                        .set(Some("Failed to parse checkout response".to_string()));
                                }
                            }
                        } else {
                            loading.set(false);
                            error.set(Some("Failed to create checkout session".to_string()));
                        }
                    }
                    Err(e) => {
                        loading.set(false);
                        error.set(Some(format!("Request failed: {}", e)));
                    }
                }
            });
        })
    };

    let on_terms_change = {
        let terms_accepted = terms_accepted.clone();
        let error = error.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            terms_accepted.set(input.checked());
            if input.checked() {
                error.set(None);
            }
        })
    };

    let button_text = if *loading {
        "Loading..."
    } else {
        "Get Started"
    };

    let checkbox_css = r#"
    .terms-checkbox-container {
        margin: 1rem 0;
        padding: 0 0.5rem;
    }
    .terms-checkbox-container label {
        display: flex;
        align-items: flex-start;
        gap: 10px;
        cursor: pointer;
        font-size: 0.8rem;
        color: rgba(255, 255, 255, 0.7);
        line-height: 1.4;
    }
    .terms-checkbox-container input[type="checkbox"] {
        appearance: none;
        -webkit-appearance: none;
        width: 18px;
        height: 18px;
        min-width: 18px;
        border: 2px solid rgba(255, 255, 255, 0.3);
        border-radius: 4px;
        background: rgba(30, 30, 30, 0.7);
        cursor: pointer;
        position: relative;
        margin-top: 2px;
        transition: all 0.2s ease;
    }
    .terms-checkbox-container input[type="checkbox"]:checked {
        background: rgba(255, 255, 255, 0.2);
        border-color: rgba(255, 255, 255, 0.8);
    }
    .terms-checkbox-container input[type="checkbox"]:checked::after {
        content: "✓";
        position: absolute;
        color: white;
        font-size: 12px;
        left: 2px;
        top: -1px;
    }
    .terms-checkbox-container input[type="checkbox"]:hover {
        border-color: rgba(255, 255, 255, 0.8);
    }
    .terms-checkbox-container a {
        color: rgba(255, 255, 255, 0.8);
        text-decoration: none;
    }
    .terms-checkbox-container a:hover {
        color: rgba(255, 255, 255, 0.7);
        text-decoration: underline;
    }
    "#;

    html! {
        <>
            <style>{checkbox_css}</style>
            <div class="terms-checkbox-container">
                <label>
                    <input
                        type="checkbox"
                        checked={*terms_accepted}
                        onchange={on_terms_change}
                    />
                    <span>
                        {"By signing up you agree to our "}
                        <a href="/terms" target="_blank">{"terms of service"}</a>
                        {" and "}
                        <a href="/privacy" target="_blank">{"privacy policy"}</a>
                        {" and consent to receive automated SMS messages from Lightfriend. Message and data rates may apply. Message frequency varies. Reply STOP to opt out."}
                    </span>
                </label>
            </div>
            if let Some(err) = (*error).as_ref() {
                <div style="color: #ff6b6b; font-size: 0.85rem; margin-bottom: 0.5rem; text-align: center;">
                    {err}
                </div>
            }
            <button
                class={classes!("iq-button", "signup-button", if !*terms_accepted { "disabled" } else { "" })}
                onclick={onclick}
                disabled={*loading || !*terms_accepted}
                style={if !*terms_accepted { "opacity: 0.6; cursor: not-allowed;" } else { "" }}
            >
                <b>{button_text}</b>
            </button>
        </>
    }
}

#[derive(Properties, PartialEq)]
pub struct PricingCardProps {
    pub plan_name: String,
    pub best_for: String,
    pub price: f64,
    pub currency: String,
    pub period: String,
    pub features: Vec<Feature>,
    pub subscription_type: String,
    pub is_popular: bool,
    pub is_premium: bool,
    pub user_id: i32,
    pub user_email: String,
    pub is_logged_in: bool,
    pub sub_tier: Option<String>,
    #[prop_or_default]
    pub user_plan_type: Option<String>,
    pub selected_country: String,
    #[prop_or(false)]
    pub coming_soon: bool,
    pub hosted_prices: HashMap<String, f64>,
    #[prop_or_default]
    pub plan_type: Option<String>,
    #[prop_or_default]
    pub children: Children,
}
#[function_component(PricingCard)]
pub fn pricing_card(props: &PricingCardProps) -> Html {
    // Always show monthly price
    let price_text = format!("{}{:.0}", props.currency, props.price);
    let effective_tier = if props.subscription_type == "hosted" {
        "tier 2".to_string()
    } else {
        props.subscription_type.clone()
    };
    let button = if props.coming_soon {
        html! { <button class="iq-button coming-soon" disabled=true><b>{"Coming Soon"}</b></button> }
    } else if props.is_logged_in {
        if props.sub_tier.as_ref() == Some(&effective_tier)
            && (props.plan_type == props.user_plan_type
                || (props.user_plan_type.is_none() && props.selected_country != "Other"))
        {
            // Show "Current Plan" if tier matches AND either:
            // - plan_type matches exactly, OR
            // - user has no plan_type (legacy US/CA users) and not on BYOT
            html! { <button class="iq-button current-plan" disabled=true><b>{"Current Plan"}</b></button> }
        } else {
            html! {
                <CheckoutButton
                    user_id={props.user_id}
                    user_email={props.user_email.clone()}
                    subscription_type={props.subscription_type.clone()}
                    selected_country={props.selected_country.clone()}
                    plan_type={props.plan_type.clone()}
                />
            }
        }
    } else {
        html! {
            <GuestCheckoutButton
                subscription_type={props.subscription_type.clone()}
                selected_country={props.selected_country.clone()}
                plan_type={props.plan_type.clone()}
            />
        }
    };
    let card_css = r#"
    .learn-more-section {
        text-align: center;
        margin-top: 1.5rem;
        margin-bottom: 1rem;
    }
    .learn-more-link {
        color: rgba(255, 255, 255, 0.8);
        text-decoration: none;
        font-size: 1.1rem;
        font-weight: 500;
        transition: color 0.3s ease;
    }
    .learn-more-link:hover {
        color: rgba(255, 255, 255, 0.7);
        text-decoration: underline;
    }
    .promo-tag {
        position: absolute;
        top: -15px;
        right: 20px;
        background: linear-gradient(45deg, #00FFFF, #00CED1);
        color: white;
        padding: 0.5rem 1rem;
        border-radius: 20px;
        font-size: 0.9rem;
        font-weight: 500;
        z-index: 4;
    }
    .signup-notification-section {
        text-align: center;
        margin: 1rem 0;
    }
    .signup-notification-link {
        color: #00FFFF;
        text-decoration: none;
        font-size: 1rem;
        font-weight: 500;
        transition: color 0.3s ease;
    }
    .signup-notification-link:hover {
        color: rgba(255, 255, 255, 0.7);
        text-decoration: underline;
    }
    .pricing-card {
        flex: 1 1 350px;
        min-width: 300px;
        max-width: 450px;
        background: rgba(30, 30, 30, 0.8);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 12px;
        position: relative;
        transition: border-color 0.3s ease;
        backdrop-filter: none;
        box-sizing: border-box;
        display: flex;
        flex-direction: column;
        padding: 0;
        width: 100%;
    }
    .pricing-card:hover {
        border-color: rgba(255, 255, 255, 0.25);
    }
    .pricing-card.popular {
        background: rgba(30, 30, 30, 0.6);
        border: 2px solid rgba(255, 255, 255, 0.3);
    }
    .pricing-card.popular:hover {
        border-color: rgba(255, 255, 255, 0.45);
    }
    .pricing-card.premium {
        background: rgba(40, 40, 40, 0.85);
        border: 2px solid rgba(255, 215, 0, 0.3);
    }
    .pricing-card.premium:hover {
        border-color: rgba(255, 215, 0, 0.5);
    }
    .popular-tag {
        position: absolute;
        top: -15px;
        right: 20px;
        background: linear-gradient(135deg, #d4d4d4, #a8a8a8 30%, #e8e8e8 50%, #a8a8a8 70%, #c0c0c0);
        color: #1a1a2e;
        padding: 0.5rem 1rem;
        border-radius: 20px;
        font-size: 0.9rem;
        font-weight: 500;
        z-index: 4;
    }
    .premium-tag {
        position: absolute;
        top: -15px;
        right: 20px;
        background: linear-gradient(45deg, #FFD700, #FFA500);
        color: white;
        padding: 0.5rem 1rem;
        border-radius: 20px;
        font-size: 0.9rem;
        font-weight: 500;
        z-index: 4;
    }
    .card-header {
        padding: 0.75rem 1rem;
        text-align: center;
        border-bottom: 1px solid rgba(255, 255, 255, 0.12);
    }
    .card-header h3 {
        color: rgba(255, 255, 255, 0.7);
        font-size: 1.1rem;
        margin: 0;
        font-weight: 700;
    }
    .card-content {
        padding: 0.75rem 1rem 1rem;
        flex-grow: 1;
        display: flex;
        flex-direction: column;
    }
    .best-for {
        color: #e0e0e0;
        font-size: 0.8rem;
        margin-top: 0.25rem;
        margin-bottom: 0.5rem;
        font-style: italic;
        text-align: center;
    }
    .price {
        margin: 0.5rem 0;
        text-align: center;
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 0.15rem;
    }
    .price .amount {
        font-size: 2rem;
        color: #fff;
        font-weight: 800;
        line-height: 1;
    }
    .price .period {
        color: #999;
        font-size: 0.85rem;
        margin-left: 0.5rem;
    }
    .billing-note {
        color: #b0b0b0;
        font-size: 0.95rem;
        margin-top: 0.5rem;
        text-align: center;
    }
    .us-deal-section {
        margin: 1rem 0;
        text-align: center;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 8px;
        padding: 0.5rem;
    }
    .us-deal-text {
        color: #FFD700;
        font-size: 0.95rem;
        font-weight: 500;
    }
    .includes {
        margin-top: 0.75rem;
    }
    .quota-list {
        list-style: none;
        padding: 0;
        margin: 0;
    }
    .quota-list li {
        color: #e0e0e0;
        padding: 0.3rem 0;
        font-size: 0.85rem;
    }
    .quota-list li.sub-item {
        padding-left: 1.5rem;
        font-size: 0.8rem;
        color: #b0b0b0;
        position: relative;
    }
    .quota-list li.sub-item::before {
        content: "→";
        position: absolute;
        left: 1rem;
        color: rgba(255, 255, 255, 0.7);
    }
    .iq-button {
        background: linear-gradient(135deg, #d4d4d4, #a8a8a8 30%, #e8e8e8 50%, #a8a8a8 70%, #c0c0c0);
        color: #1a1a2e;
        border: none;
        padding: 1rem 2rem;
        border-radius: 8px;
        font-size: 1rem;
        cursor: pointer;
        transition: all 0.3s ease;
        border: 1px solid rgba(255, 255, 255, 0.1);
        width: 100%;
        margin-top: 2rem;
        text-decoration: none;
    }
    .iq-button:hover {
        background: linear-gradient(135deg, #e0e0e0, #b8b8b8 30%, #f0f0f0 50%, #b8b8b8 70%, #d0d0d0);
    }
    .iq-button.disabled {
        background: rgba(30, 30, 30, 0.5);
        cursor: not-allowed;
        border: 1px solid rgba(255, 255, 255, 0.1);
    }
    .iq-button.disabled:hover {
    }
    .iq-button.current-plan {
        background: rgba(255, 255, 255, 0.2);
        border: 1px solid rgba(255, 255, 255, 0.3);
        cursor: default;
    }
    .iq-button.current-plan:hover {
        background: rgba(255, 255, 255, 0.2);
    }
    .iq-button.coming-soon {
        background: rgba(255, 165, 0, 0.3);
        border: 1px solid rgba(255, 165, 0, 0.5);
        cursor: default;
    }
    .iq-button.coming-soon:hover {
    }
    .addons-section {
        margin-top: 1.5rem;
        border-top: 1px solid rgba(255,255,255,0.1);
        padding-top: 1rem;
    }
    .addon-list {
        list-style: none;
        padding: 0;
    }
    .addon-list li {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        color: #e0e0e0;
        padding: 0.5rem 0;
    }
    .addon-desc {
        font-size: 0.9rem;
        color: #b0b0b0;
        margin-left: 1.5rem;
    }
    .addon-total {
        font-weight: bold;
        margin-top: 1rem;
        text-align: right;
        color: #e0e0e0;
    }
    @media (max-width: 640px) {
        .pricing-grid {
            flex-wrap: wrap;
        }
        .pricing-card {
            min-width: 0;
            max-width: 100%;
            width: 100%;
            padding: 0;
        }
        .card-content {
            padding: 0.75rem;
        }
        .price .amount {
            font-size: 1.8rem;
        }
    }
.learn-more-section {
    text-align: center;
    margin-top: 1.5rem;
    margin-bottom: 1rem;
}
.learn-more-link {
    color: rgba(255, 255, 255, 0.8);
    text-decoration: none;
    font-size: 1.1rem;
    font-weight: 500;
    transition: color 0.3s ease;
}
.learn-more-link:hover {
    color: rgba(255, 255, 255, 0.7);
    text-decoration: underline;
}
    "#;
    html! {
        <div class={classes!("pricing-card", "subscription",
            if props.is_popular { "popular" } else { "" },
            if props.is_premium { "premium" } else { "" })}>
            <style>{card_css}</style>
            {
                if props.is_popular {
                    html! { <div class="popular-tag">{"Most Popular"}</div> }
                } else {
                    html! {}
                }
            }
            <div class="card-header">
                <h3>{props.plan_name.clone()}</h3>
            </div>
            <div class="card-content">
                { for props.children.iter() }
                <p class="best-for">{props.best_for.clone()}</p>
                <div class="price">
                    <span class="amount">{price_text}</span>
                    <span class="period">{props.period.clone()}</span>
                </div>
                    <div class="learn-more-section">
                        <a href="/how-to-switch-to-dumbphone" class="learn-more-link">{"How to switch to a dumbphone and what you'll need"}</a>
                    </div>
                <div class="includes">
                    <ul class="quota-list">
                        { for props.features.iter().flat_map(|feature| {
                            let main_item = html! { <li>{feature.text.clone()}</li> };
                            let sub_items = feature.sub_items.iter().map(|sub| html! { <li class="sub-item">{sub}</li> }).collect::<Vec<_>>();
                            vec![main_item].into_iter().chain(sub_items.into_iter())
                        }) }
                        { if props.subscription_type == "hosted" && props.selected_country == "Other" {
                            html! { <li>{"Bring your own number. See the guide below."}</li> }
                        } else { html! {} }}
                    </ul>
                </div>
                {
                    // Show BYOT guide link for BYOT plans or "Other" countries
                    if props.plan_type.as_deref() == Some("byot") || props.selected_country == "Other" {
                        html! {
                            <div class="learn-more-section">
                                <a href="/bring-own-number" class="learn-more-link">{"Check Twilio availability for your country"}</a>
                            </div>
                            }
                    } else {
                        html! {}
                    }
                }
                {button}
            </div>
        </div>
    }
}
#[derive(Properties, PartialEq)]
pub struct FeatureListProps {
    pub selected_country: String,
}
#[function_component(CreditPricing)]
pub fn credit_pricing(props: &FeatureListProps) -> Html {
    let country = &props.selected_country;
    let pricing = use_state(|| None::<ByotPricingResponse>);
    let loading = use_state(|| true);

    // Fetch pricing for this country
    {
        let pricing = pricing.clone();
        let loading = loading.clone();
        let country = country.clone();
        use_effect_with_deps(
            move |country| {
                let country = country.clone();
                // Skip fetch for US/CA/Other
                if country != "US" && country != "CA" && country != "Other" {
                    let pricing = pricing.clone();
                    let loading = loading.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        loading.set(true);
                        let response = Api::get(&format!("/api/pricing/byot/{}", country))
                            .send()
                            .await;
                        if let Ok(resp) = response {
                            if resp.ok() {
                                if let Ok(data) = resp.json::<ByotPricingResponse>().await {
                                    pricing.set(Some(data));
                                }
                            }
                        }
                        loading.set(false);
                    });
                } else {
                    loading.set(false);
                }
                || ()
            },
            country,
        );
    }

    let credit_css = r#"
    .credit-pricing {
        max-width: 1000px;
        margin: 4rem auto;
        background: rgba(30, 30, 30, 0.8);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 24px;
        padding: 2.5rem;
        backdrop-filter: none;
        text-align: center;
    }
    .credit-pricing h2 {
        color: rgba(255, 255, 255, 0.7);
        font-size: 2rem;
        margin-bottom: 0.5rem;
    }
    .credit-pricing .subtitle {
        color: #888;
        font-size: 0.95rem;
        margin-bottom: 1.5rem;
    }
    .credit-pricing p {
        color: #e0e0e0;
        font-size: 1.1rem;
        margin-bottom: 1rem;
    }
    .credit-pricing ul {
        list-style-type: none;
        padding: 0;
        margin: 1rem 0;
    }
    .credit-pricing li {
        color: #e0e0e0;
        padding: 0.5rem 0;
        font-size: 1.1rem;
    }
    .credit-pricing a {
        color: rgba(255, 255, 255, 0.8);
        text-decoration: none;
    }
    .credit-pricing a:hover {
        text-decoration: underline;
    }
    .credit-pricing .loading {
        color: #888;
        font-style: italic;
    }
    @media (max-width: 968px) {
        .credit-pricing {
            padding: 1.5rem;
            margin: 2rem 1rem;
            max-width: calc(100vw - 2rem);
        }
    }
    "#;

    // VAT multiplier for overage credits
    const OVERAGE_MULTIPLIER: f32 = 1.3;

    if country == "Other" {
        html! {
            <div class="credit-pricing">
                <style>{credit_css}</style>
                <h2>{"Messaging Costs"}</h2>
                <p>{"To see prices for your country and how to set up, check our guide page. It has info on rules too."}</p>
                <a href="/bring-own-number">{"See Setup Guide and Prices"}</a>
            </div>
        }
    } else if country == "US" || country == "CA" {
        // US/CA has messages included, no overage section needed
        html! {}
    } else if *loading {
        html! {
            <div class="credit-pricing">
                <style>{credit_css}</style>
                <h2>{"Overage Credits"}</h2>
                <p class="subtitle">{"Available on Digest plan only"}</p>
                <p class="loading">{"Loading pricing..."}</p>
            </div>
        }
    } else if let Some(ref p) = *pricing {
        // Calculate overage prices with 1.3x VAT multiplier
        let notification_price = p.costs.notification.map(|n| n * OVERAGE_MULTIPLIER);
        let response_price = p.costs.normal_response.map(|r| r * OVERAGE_MULTIPLIER);
        let digest_price = p.costs.digest.map(|d| d * OVERAGE_MULTIPLIER);
        let voice_out_price = p
            .costs
            .voice_outbound_per_min
            .map(|v| v * OVERAGE_MULTIPLIER);
        let voice_in_price = p
            .costs
            .voice_inbound_per_min
            .map(|v| v * OVERAGE_MULTIPLIER);

        let is_local = is_local_number_country(country);
        let is_notification_only = is_notification_only_country(country);

        html! {
            <div class="credit-pricing">
                <style>{credit_css}</style>
                <h2>{"Overage Credits"}</h2>
                <p class="subtitle">{"Available on Digest plan only"}</p>
                <p>{"Digest plan includes prepaid credits each month. If you use them all, you can purchase more at these rates:"}</p>
                <ul>
                    if let Some(noti) = notification_price {
                        <li>{format!("Notification: €{:.2} each", noti)}</li>
                    }
                    if let Some(resp) = response_price {
                        if is_notification_only {
                            <li>{format!("Response message: €{:.2} each (pricing basis)", resp)}</li>
                        } else {
                            <li>{format!("Response message: €{:.2} each", resp)}</li>
                        }
                    }
                    if let Some(digest) = digest_price {
                        <li>{format!("Daily digest: €{:.2} each", digest)}</li>
                    }
                    if let Some(voice_out) = voice_out_price {
                        <li>{format!("Voice call (outbound): €{:.2}/min", voice_out)}</li>
                    }
                    if is_local {
                        if let Some(voice_in) = voice_in_price {
                            <li>{format!("Voice call (inbound): €{:.2}/min", voice_in)}</li>
                        }
                    }
                </ul>
            </div>
        }
    } else {
        // Fallback if pricing fetch failed
        html! {
            <div class="credit-pricing">
                <style>{credit_css}</style>
                <h2>{"Overage Credits"}</h2>
                <p class="subtitle">{"Available on Digest plan only"}</p>
                <p>{"Digest plan includes prepaid credits each month. Exact overage pricing varies by country."}</p>
            </div>
        }
    }
}
#[function_component(UnifiedPricing)]
pub fn unified_pricing(props: &PricingProps) -> Html {
    // Assistant plan prices (lower tier)
    let hosted_prices: HashMap<String, f64> = HashMap::from([
        ("US".to_string(), 19.00),
        ("CA".to_string(), 19.00),
        ("FI".to_string(), 29.00),
        ("NL".to_string(), 29.00),
        ("GB".to_string(), 29.00),
        ("AU".to_string(), 29.00),
        // Notification-only countries
        ("DE".to_string(), 29.00),
        ("FR".to_string(), 29.00),
        ("ES".to_string(), 29.00),
        ("IT".to_string(), 29.00),
        ("PT".to_string(), 29.00),
        ("BE".to_string(), 29.00),
        ("AT".to_string(), 29.00),
        ("CH".to_string(), 29.00),
        ("PL".to_string(), 29.00),
        ("CZ".to_string(), 29.00),
        ("SE".to_string(), 29.00),
        ("DK".to_string(), 29.00),
        ("NO".to_string(), 29.00),
        ("IE".to_string(), 29.00),
        ("NZ".to_string(), 29.00),
        ("GR".to_string(), 29.00),
        ("HU".to_string(), 29.00),
        ("RO".to_string(), 29.00),
        ("SK".to_string(), 29.00),
        ("BG".to_string(), 29.00),
        ("HR".to_string(), 29.00),
        ("SI".to_string(), 29.00),
        ("LT".to_string(), 29.00),
        ("LV".to_string(), 29.00),
        ("EE".to_string(), 29.00),
        ("LU".to_string(), 29.00),
        ("MT".to_string(), 29.00),
        ("CY".to_string(), 29.00),
        ("IS".to_string(), 29.00),
        ("JP".to_string(), 29.00),
        ("KR".to_string(), 29.00),
        ("SG".to_string(), 29.00),
        ("HK".to_string(), 29.00),
        ("TW".to_string(), 29.00),
        ("IL".to_string(), 29.00),
        ("Other".to_string(), 19.00), // BYOT plan stays at 19 EUR
    ]);
    let _hosted_total_price = hosted_prices.get(&props.selected_country).unwrap_or(&0.0);
    let pricing_css = r#"
    .subscription-blocked-notice {
        max-width: 800px;
        margin: 2rem auto;
        padding: 2rem;
        background: rgba(255, 165, 0, 0.15);
        border: 2px solid rgba(255, 165, 0, 0.5);
        border-radius: 16px;
        text-align: center;
    }
    .subscription-blocked-notice h3 {
        color: #FFA500;
        font-size: 1.5rem;
        margin-bottom: 1rem;
    }
    .subscription-blocked-notice p {
        color: #e0e0e0;
        font-size: 1.1rem;
        line-height: 1.6;
        margin-bottom: 0.5rem;
    }
    .pricing-grid {
        display: flex;
        flex-wrap: nowrap;
        gap: 1rem;
        justify-content: center;
        max-width: 1000px;
        margin: 2rem auto;
    }
    .hosted-plans-section, .self-hosted-plans-section {
        margin: 4rem auto;
        max-width: 1200px;
    }
    .section-title {
        text-align: center;
        color: rgba(255, 255, 255, 0.7);
        font-size: 2.5rem;
        margin-bottom: 2rem;
    }
    .pricing-panel {
        position: relative;
        min-height: 100vh;
        padding: 6rem 2rem;
        color: #ffffff;
        background: #0d0d0d;
        z-index: 1;
        overflow: hidden;
    }
    .pricing-panel::before {
        content: '';
        position: fixed;
        top: 0;
        left: 0;
        width: 100%;
        height: 100vh;
        background-image: url('/assets/stars-bg.jpg');
        background-size: cover;
        background-position: center;
        background-repeat: no-repeat;
        opacity: 0.3;
        z-index: -2;
        pointer-events: none;
    }
    .pricing-header {
        text-align: center;
        margin-bottom: 4rem;
    }
    .pricing-header h1 {
        font-size: 3.5rem;
        margin-bottom: 1.5rem;
        color: #fff;
        font-weight: 700;
    }
    .pricing-header p {
        color: #999;
        font-size: 1.2rem;
        max-width: 600px;
        margin: 0 auto;
    }
    .country-selector {
        text-align: center;
        margin: 2rem 0;
        background: rgba(30, 30, 30, 0.7);
        padding: 1.5rem;
        border-radius: 16px;
        border: 1px solid rgba(255, 255, 255, 0.12);
        max-width: 400px;
        margin: 2rem auto;
    }
    .country-selector label {
        color: rgba(255, 255, 255, 0.7);
        margin-right: 1rem;
        font-size: 1.1rem;
    }
    .country-selector select {
        padding: 0.8rem;
        font-size: 1rem;
        border-radius: 8px;
        border: 1px solid rgba(255, 255, 255, 0.2);
        background: rgba(30, 30, 30, 0.9);
        color: #fff;
        cursor: pointer;
        transition: all 0.3s ease;
    }
    .country-selector select:hover {
        border-color: rgba(255, 255, 255, 0.3);
    }
    .pricing-faq {
        max-width: 800px;
        margin: 4rem auto;
    }
    .pricing-faq h2 {
        color: rgba(255, 255, 255, 0.7);
        font-size: 2rem;
        margin-bottom: 2rem;
        text-align: center;
    }
    .faq-grid {
        display: grid;
        gap: 1rem;
    }
    details {
        background: rgba(30, 30, 30, 0.8);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 12px;
        padding: 1.5rem;
        transition: all 0.3s ease;
    }
    details:hover {
        border-color: rgba(255, 255, 255, 0.2);
    }
    summary {
        color: rgba(255, 255, 255, 0.7);
        font-size: 1.1rem;
        cursor: pointer;
        padding: 0.5rem 0;
    }
    details p {
        color: #e0e0e0;
        margin-top: 1rem;
        line-height: 1.6;
        padding: 0.5rem 0;
    }
    .footnotes {
        max-width: 800px;
        margin: 3rem auto;
        text-align: center;
    }
    .footnote {
        color: #999;
        font-size: 0.9rem;
    }
    .footnote a {
        color: rgba(255, 255, 255, 0.7);
        text-decoration: none;
        transition: color 0.3s ease;
    }
    .footnote a:hover {
        color: rgba(255, 255, 255, 0.8);
    }
    .github-link {
        color: rgba(255, 255, 255, 0.7);
        font-size: 0.9rem;
        text-decoration: none;
        transition: color 0.3s ease;
    }
    .github-link:hover {
        color: rgba(255, 255, 255, 0.8);
    }
    .legal-links {
        text-align: center;
        margin-top: 2rem;
    }
    .legal-links a {
        color: #999;
        text-decoration: none;
        transition: color 0.3s ease;
    }
    .legal-links a:hover {
        color: rgba(255, 255, 255, 0.7);
    }
    .topup-pricing {
        max-width: 1000px;
        margin: 4rem auto;
        text-align: center;
    }
    .topup-pricing h2 {
        color: rgba(255, 255, 255, 0.7);
        font-size: 2rem;
        margin-bottom: 1rem;
    }
    .topup-pricing p {
        color: #999;
        margin-bottom: 2rem;
    }
    .pricing-card.main {
        background: rgba(30, 30, 30, 0.8);
        border: 1px solid rgba(255, 255, 255, 0.12);
        padding: 2rem;
        min-width: 400px;
    }
    .package-row {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 1rem 0;
        border-bottom: 1px solid rgba(255, 255, 255, 0.12);
    }
    .package-row:last-child {
        border-bottom: none;
    }
    .package-row h3 {
        font-size: 1.2rem;
        margin: 0;
    }
    .package-row .price {
        margin: 0;
    }
    .topup-packages {
        max-width: 600px;
        margin: 2rem auto;
        align-items: center;
        display: flex;
        justify-content: center;
    }
    .package-row .price .amount {
        font-size: 1.5rem;
    }
    .topup-toggle {
        margin-top: 2rem;
        text-align: center;
    }
    .topup-toggle p {
        color: #999;
        margin-bottom: 1rem;
    }
    .phone-number-options {
        max-width: 1200px;
        margin: 4rem auto;
    }
    .phone-number-section {
        text-align: center;
        padding: 2.5rem;
    }
    .phone-number-section h2 {
        color: rgba(255, 255, 255, 0.7);
        font-size: 2.5rem;
        margin-bottom: 2rem;
    }
    .options-grid {
        display: grid;
        grid-template-columns: 1fr;
        gap: 2rem;
        margin-top: 2rem;
        max-width: 600px;
        margin: 2rem auto;
    }
    .option-card {
        background: rgba(30, 30, 30, 0.8);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 24px;
        padding: 2.5rem;
        backdrop-filter: none;
        transition: border-color 0.3s ease;
    }
    .option-card:hover {
        border-color: rgba(255, 255, 255, 0.2);
    }
    .option-card h3 {
        color: rgba(255, 255, 255, 0.7);
        font-size: 1.8rem;
        margin-bottom: 1rem;
    }
    .option-card p {
        color: #e0e0e0;
        margin-bottom: 2rem;
        font-size: 1.1rem;
        line-height: 1.6;
    }
    .sentinel-extras-integrated {
        margin: 2rem auto;
        padding: 2rem;
        background: rgba(30, 30, 30, 0.7);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 16px;
        max-width: 600px;
    }
    .extras-section {
        margin-bottom: 2rem;
    }
    .extras-section:last-child {
        margin-bottom: 0;
    }
    .extras-section h4 {
        color: rgba(255, 255, 255, 0.7);
        font-size: 1.3rem;
        margin-bottom: 0.5rem;
        text-align: center;
    }
    .extras-description {
        color: #b0b0b0;
        font-size: 0.95rem;
        text-align: center;
        margin-bottom: 1.5rem;
    }
    .extras-selector-inline {
        display: flex;
        flex-direction: column;
        gap: 1rem;
    }
    .extras-summary-inline {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 1rem;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 8px;
        margin-top: 0.5rem;
    }
    .quantity-selector-inline {
        display: flex;
        align-items: center;
        gap: 1rem;
        justify-content: center;
    }
    .quantity-selector-inline label {
        color: rgba(255, 255, 255, 0.7);
        font-size: 1rem;
        font-weight: 500;
        min-width: 120px;
    }
    .quantity-selector-inline select {
        padding: 0.6rem 1rem;
        font-size: 0.95rem;
        border-radius: 8px;
        border: 1px solid rgba(255, 255, 255, 0.2);
        background: rgba(30, 30, 30, 0.9);
        color: #fff;
        cursor: pointer;
        transition: all 0.3s ease;
        min-width: 140px;
    }
    .quantity-selector-inline select:hover {
        border-color: rgba(255, 255, 255, 0.3);
    }
    .summary-item {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 0.25rem;
    }
    .summary-label {
        color: rgba(255, 255, 255, 0.7);
        font-size: 0.9rem;
        font-weight: 500;
    }
    .summary-value {
        color: #fff;
        font-size: 1rem;
        font-weight: 600;
    }
    .time-value-section {
        max-width: 800px;
        margin: 2rem auto;
        text-align: center;
        background: rgba(30, 30, 30, 0.8);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 24px;
        padding: 2rem;
        backdrop-filter: none;
    }
    .time-value-section h2 {
        color: rgba(255, 255, 255, 0.7);
        font-size: 2rem;
        margin-bottom: 1rem;
    }
    .time-value-section p {
        color: #e0e0e0;
        font-size: 1.1rem;
        margin-bottom: 1rem;
    }
    @media (max-width: 968px) {
        .pricing-header h1 {
            font-size: 2.5rem;
        }
        .pricing-panel {
            padding: 4rem 1rem;
        }
        .pricing-grid {
            flex-direction: column;
        }
    }
    "#;
    html! {
        <div class="pricing-panel">
            <style>{pricing_css}</style>
            <div class="pricing-header">
                <h1>{"Invest in Your Peace of Mind"}</h1>
                <p>{"Lightfriend makes it possible to seriously switch to a dumbphone, saving you 2-4 hours per day of mindless scrolling*"}</p>
                {
                    if props.selected_country == "Other" {
                        html! {
                            <>
                            <br/>
                            <p class="availability-note" style="color: #ff9494; font-size: 0.9rem; margin-top: 0.5rem;">
                                {format!("Note: Service may be limited or unavailable in {}. ", props.country_name.clone())}
                                {" More info about supported countries can be checked in "}
                                <span class="legal-links">
                                    <a style="color: rgba(255, 255, 255, 0.8);" href="/supported-countries">{"Supported Countries"}</a>
                                    {" or by emailing "}
                                    <a style="color: rgba(255, 255, 255, 0.8);"
                                       href={format!("mailto:rasmus@lightfriend.ai?subject=Country%20Availability%20Inquiry%20for%20{}&body=Hey,%0A%0AIs%20the%20service%20available%20in%20{}%3F%0A%0AThanks,%0A",
                                       props.country_name.clone(), props.country_name.clone())}>
                                        {"rasmus@lightfriend.ai"}
                                    </a>
                                </span>
                                {". Contact to ask for availability"}
                            </p>
                            </>
                        }
                    } else {
                        html! {}
                    }
                }
            </div>

            {
                if !props.is_logged_in && !props.country_name.is_empty() {
                    html! {
                        <p class="detected-country" style="font-size: 0.85rem; color: #888; margin-bottom: 1.5rem;">
                            {format!("Showing prices for {}. ", props.country_name.clone())}
                            <a href="mailto:rasmus@lightfriend.ai" style="color: #7EB2FF; text-decoration: none;">{"Not your country?"}</a>
                        </p>
                    }
                } else {
                    html! {}
                }
            }
            <h2 class="section-title">{"Plans"}</h2>
            <div class="pricing-grid">
                {
                    if props.selected_country == "US" || props.selected_country == "CA" {
                        // US/CA: Show Assistant and Autopilot plans
                        let assistant_features = vec![
                            Feature { text: "$25/month in messaging credits - more than enough for most users".to_string(), sub_items: vec![] },
                            Feature { text: "Reminders and scheduled items".to_string(), sub_items: vec![] },
                            Feature { text: "Contact profiles with all/digest modes".to_string(), sub_items: vec![] },
                            Feature { text: "Daily digests".to_string(), sub_items: vec![] },
                            Feature { text: "Manual item tracking".to_string(), sub_items: vec![] },
                            Feature { text: "Need more? Buy overage credits anytime".to_string(), sub_items: vec![] },
                        ];
                        let autopilot_features = vec![
                            Feature { text: "$25/month in messaging credits - more than enough for most users".to_string(), sub_items: vec![] },
                            Feature { text: "Everything in Assistant, plus:".to_string(), sub_items: vec![] },
                            Feature { text: "Automatic message analysis".to_string(), sub_items: vec![] },
                            Feature { text: "Background monitoring".to_string(), sub_items: vec![] },
                            Feature { text: "Critical alerts filtering".to_string(), sub_items: vec![] },
                            Feature { text: "Auto item creation from messages".to_string(), sub_items: vec![] },
                        ];
                        html! {
                            <>
                                <PricingCard
                                    plan_name={"Assistant Plan"}
                                    best_for={"Manual control. Set reminders, track items, get daily digests."}
                                    price={19.0}
                                    currency={"$"}
                                    period={"/month"}
                                    features={assistant_features}
                                    subscription_type={"hosted"}
                                    is_popular={false}
                                    is_premium={false}
                                    user_id={props.user_id}
                                    user_email={props.user_email.clone()}
                                    is_logged_in={props.is_logged_in}
                                    sub_tier={props.sub_tier.clone()}
                                    user_plan_type={props.user_plan_type.clone()}
                                    selected_country={props.selected_country.clone()}
                                    coming_soon={false}
                                    plan_type={Some("assistant".to_string())}
                                    hosted_prices={hosted_prices.clone()}
                                />
                                <PricingCard
                                    plan_name={"Autopilot Plan"}
                                    best_for={"Automatic intelligence. Lightfriend processes your messages for you."}
                                    price={29.0}
                                    currency={"$"}
                                    period={"/month"}
                                    features={autopilot_features}
                                    subscription_type={"hosted"}
                                    is_popular={true}
                                    is_premium={false}
                                    user_id={props.user_id}
                                    user_email={props.user_email.clone()}
                                    is_logged_in={props.is_logged_in}
                                    sub_tier={props.sub_tier.clone()}
                                    user_plan_type={props.user_plan_type.clone()}
                                    selected_country={props.selected_country.clone()}
                                    coming_soon={false}
                                    plan_type={Some("autopilot".to_string())}
                                    hosted_prices={hosted_prices.clone()}
                                />
                            </>
                        }
                    } else if props.selected_country == "Other" {
                        // Other countries: BYOT Plan only (bring your own Twilio)
                        let byot_features = vec![
                            Feature { text: "Bring your own Twilio number".to_string(), sub_items: vec![] },
                            Feature { text: "All features included".to_string(), sub_items: vec![] },
                            Feature { text: "No message limits - pay Twilio directly".to_string(), sub_items: vec![] },
                        ];
                        html! {
                            <PricingCard
                                plan_name={"BYOT Plan"}
                                best_for={"Bring Your Own Twilio. Full service, you handle messaging costs."}
                                price={19.0}
                                currency={"€"}
                                period={"/month"}
                                features={byot_features}
                                subscription_type={"hosted"}
                                is_popular={true}
                                is_premium={false}
                                user_id={props.user_id}
                                user_email={props.user_email.clone()}
                                is_logged_in={props.is_logged_in}

                                sub_tier={props.sub_tier.clone()}
                                user_plan_type={props.user_plan_type.clone()}
                                selected_country={props.selected_country.clone()}
                                coming_soon={false}
                                hosted_prices={hosted_prices.clone()}
                                plan_type={Some("byot".to_string())}
                            >
                                <ByotPricingDisplay />
                            </PricingCard>
                        }
                    } else {
                        // Euro countries: Show Assistant and Autopilot plans
                        let is_notification_only = is_notification_only_country(&props.selected_country);

                        let assistant_features = vec![
                            Feature { text: "$25/month in messaging credits - more than enough for most users".to_string(), sub_items: vec![] },
                            Feature { text: "Reminders and scheduled items".to_string(), sub_items: vec![] },
                            Feature { text: "Contact profiles with all/digest modes".to_string(), sub_items: vec![] },
                            Feature { text: "Daily digests".to_string(), sub_items: vec![] },
                            Feature { text: "Manual item tracking".to_string(), sub_items: vec![] },
                            Feature { text: "Need more? Buy overage credits anytime".to_string(), sub_items: vec![] },
                        ];
                        let autopilot_features = vec![
                            Feature { text: "$25/month in messaging credits - more than enough for most users".to_string(), sub_items: vec![] },
                            Feature { text: "Everything in Assistant, plus:".to_string(), sub_items: vec![] },
                            Feature { text: "Automatic message analysis".to_string(), sub_items: vec![] },
                            Feature { text: "Background monitoring".to_string(), sub_items: vec![] },
                            Feature { text: "Critical alerts filtering".to_string(), sub_items: vec![] },
                            Feature { text: "Auto item creation from messages".to_string(), sub_items: vec![] },
                        ];

                        let byot_features = vec![
                            Feature { text: "Bring your own Twilio number".to_string(), sub_items: vec![] },
                            Feature { text: "All Autopilot features included".to_string(), sub_items: vec![] },
                            Feature { text: "No message limits - pay Twilio directly".to_string(), sub_items: vec![] },
                        ];

                        html! {
                            <>
                                <PricingCard
                                    plan_name={"Assistant Plan"}
                                    best_for={"Manual control. Set reminders, track items, get daily digests."}
                                    price={29.0}
                                    currency={"€"}
                                    period={"/month"}
                                    features={assistant_features}
                                    subscription_type={"hosted"}
                                    is_popular={false}
                                    is_premium={false}
                                    user_id={props.user_id}
                                    user_email={props.user_email.clone()}
                                    is_logged_in={props.is_logged_in}

                                    sub_tier={props.sub_tier.clone()}
                                    user_plan_type={props.user_plan_type.clone()}
                                    selected_country={props.selected_country.clone()}
                                    coming_soon={false}
                                    hosted_prices={hosted_prices.clone()}
                                    plan_type={Some("assistant".to_string())}
                                />
                                <PricingCard
                                    plan_name={"Autopilot Plan"}
                                    best_for={"Automatic intelligence. Lightfriend processes your messages for you."}
                                    price={49.0}
                                    currency={"€"}
                                    period={"/month"}
                                    features={autopilot_features}
                                    subscription_type={"hosted"}
                                    is_popular={true}
                                    is_premium={false}
                                    user_id={props.user_id}
                                    user_email={props.user_email.clone()}
                                    is_logged_in={props.is_logged_in}

                                    sub_tier={props.sub_tier.clone()}
                                    user_plan_type={props.user_plan_type.clone()}
                                    selected_country={props.selected_country.clone()}
                                    coming_soon={false}
                                    hosted_prices={hosted_prices.clone()}
                                    plan_type={Some("autopilot".to_string())}
                                />
                                // Show BYOT option for notification-only countries (they may want their own local number)
                                if is_notification_only {
                                    <PricingCard
                                        plan_name={"BYOT Plan"}
                                        best_for={"Want a local number? Verify Twilio availability for your country first."}
                                        price={19.0}
                                        currency={"€"}
                                        period={"/month"}
                                        features={byot_features}
                                        subscription_type={"hosted"}
                                        is_popular={false}
                                        is_premium={false}
                                        user_id={props.user_id}
                                        user_email={props.user_email.clone()}
                                        is_logged_in={props.is_logged_in}

                                        sub_tier={props.sub_tier.clone()}
                                        user_plan_type={props.user_plan_type.clone()}
                                        selected_country={props.selected_country.clone()}
                                        coming_soon={false}
                                        hosted_prices={hosted_prices.clone()}
                                        plan_type={Some("byot".to_string())}
                                    >
                                        <ByotPricingDisplay />
                                    </PricingCard>
                                }
                            </>
                        }
                    }
                }
            </div>
            <CreditPricing selected_country={props.selected_country.clone()} />
            <div class="pricing-faq">
                <h2>{"Common Questions"}</h2>
                <div class="faq-grid">
                    {
                        if props.selected_country == "US" || props.selected_country == "CA" {
                            html! {
                                <>
                                <details>
                                    <summary>{"How does billing work?"}</summary>
                                    <p>{"Plans bill monthly. Both plans include the same generous message allowance. No hidden fees."}</p>
                                </details>
                                <details>
                                    <summary>{"What's the difference between Assistant and Autopilot?"}</summary>
                                    <p>{"Assistant gives you manual control: reminders, contact profiles, daily digests. Autopilot adds automatic intelligence: Lightfriend reads your incoming messages, filters by urgency, creates items automatically, and monitors for updates."}</p>
                                </details>
                                </>
                            }
                        } else if props.selected_country == "FI" || props.selected_country == "NL" || props.selected_country == "AU" || props.selected_country == "GB" {
                            html! {
                                <>
                                <details>
                                    <summary>{"How does billing work?"}</summary>
                                    <p>{"Plans bill monthly. Assistant (29 EUR) and Autopilot (49 EUR) both include the same message credits. Phone number included. No hidden fees."}</p>
                                </details>
                                <details>
                                    <summary>{"What's the difference between Assistant and Autopilot?"}</summary>
                                    <p>{"Assistant gives you manual control: reminders, contact profiles, daily digests. Autopilot adds automatic intelligence: Lightfriend reads your incoming messages, filters by urgency, creates items automatically, and monitors for updates."}</p>
                                </details>
                                <details>
                                    <summary>{"How do credits work?"}</summary>
                                    <p>{"Different message types cost different amounts based on your country's SMS rates. Both plans can purchase additional credits when needed."}</p>
                                </details>
                                </>
                            }
                        } else if is_notification_only_country(&props.selected_country) {
                            html! {
                                <>
                                <details>
                                    <summary>{"How does billing work?"}</summary>
                                    <p>{"Plans bill monthly. Assistant (29 EUR) and Autopilot (49 EUR) both include the same message credits. Messages sent from a US number. No hidden fees."}</p>
                                </details>
                                <details>
                                    <summary>{"What's the difference between Assistant and Autopilot?"}</summary>
                                    <p>{"Assistant gives you manual control: reminders, contact profiles, daily digests. Autopilot adds automatic intelligence: Lightfriend reads your incoming messages, filters by urgency, creates items automatically, and monitors for updates."}</p>
                                </details>
                                <details>
                                    <summary>{"How do credits work?"}</summary>
                                    <p>{"Different message types cost different amounts based on your country's SMS rates. Both plans can purchase additional credits when needed."}</p>
                                </details>
                                <details>
                                    <summary>{"Can I bring my own phone number?"}</summary>
                                    <p>{"Yes! If you want a local number for two-way messaging, you can use the BYOT (Bring Your Own Twilio) plan. This lets you set up your own Twilio account and pay messaging costs directly to them."}</p>
                                </details>
                                <details>
                                    <summary>{"Do I need to text back?"}</summary>
                                    <p>{"Rarely! Lightfriend is proactive-first - it monitors your emails and messages, then notifies you about what matters. You can ask questions via web chat or voice calls from the dashboard anytime."}</p>
                                </details>
                                </>
                            }
                        } else {
                            html! {
                                <>
                                <details>
                                    <summary>{"How does billing work?"}</summary>
                                    <p>{"Plans bill monthly. Use the BYOT (Bring Your Own Twilio) plan to set up your own number and pay messaging costs directly to Twilio. No hidden fees."}</p>
                                </details>
                                <details>
                                    <summary>{"What is BYOT?"}</summary>
                                    <p>{"Bring Your Own Twilio lets you create a Twilio account, get a phone number for your country, and connect it to Lightfriend. You pay Twilio directly for messaging - we provide the AI assistant service."}</p>
                                </details>
                                </>
                            }
                        }
                    }
                    <details>
                        <summary>{"Is it available in my country?"}</summary>
                        <p>{"Available globally. US/CA has everything included. FI/GB/AU/NL include a local number. Other European countries receive messages from a US number. For other countries, you can use BYOT to bring your own number."}</p>
                    </details>
                    <details>
                        <summary>{"Why do plan offerings differ per country?"}</summary>
                        <p>{"SMS costs vary dramatically by country - US/Canada is about 10x cheaper than Europe. We structure plans to keep pricing fair: US/CA gets messages included, while other countries get credits that account for local SMS rates."}</p>
                    </details>
                </div>
            </div>
            <div class="footnotes">
                <p class="footnote">{"* Gen Z spends 4-7 hours daily on phones, often regretting 60% of social media time. "}<a href="https://explodingtopics.com/blog/smartphone-usage-stats" target="_blank" rel="noopener noreferrer">{"Read the study"}</a><grok-card data-id="badfd9" data-type="citation_card"></grok-card></p>
                <p class="footnote">{"The dumbphone is sold separately and is not included in the Hosted Plan."}</p>
                <p class="footnote">{"For developers: Check out the open-source repo on GitHub if you'd like to self-host from source (requires technical setup)."}</p>
                <a href="https://github.com/ahtavarasmus/lightfriend" target="_blank" rel="noopener noreferrer" class="github-link">{"View GitHub Repo"}</a>
            </div>
            <div class="legal-links">
                <Link<Route> to={Route::Terms}>{"Terms & Conditions"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Privacy}>{"Privacy Policy"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Trustless}>{"Verifiably Private"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Changelog}>{"Updates"}</Link<Route>>
            </div>
        </div>
    }
}
