use yew::prelude::*;
use yew_router::prelude::Link;
use crate::Route;
use serde_json::json;
use web_sys::window;
use wasm_bindgen_futures;
use serde_json::Value;
use serde::Deserialize;
use std::collections::HashMap;
use crate::utils::api::Api;

/// Check if a country is notification-only (receives SMS from US number, dynamic pricing)
fn is_notification_only_country(country: &str) -> bool {
    matches!(country, "DE" | "FR" | "ES" | "IT" | "PT" | "BE" | "AT" | "CH" | "PL" | "CZ" | "SE" | "DK" | "NO" | "IE" | "NZ")
}

/// Check if country has local number (can receive inbound calls)
fn is_local_number_country(country: &str) -> bool {
    matches!(country, "FI" | "NL" | "GB" | "AU")
}

/// Message equivalent display that fetches real pricing from backend
#[derive(Properties, PartialEq)]
pub struct MessageEquivalentProps {
    pub plan_messages: i32,  // 50 or 150 (the "messages" in the plan)
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
        use_effect_with_deps(move |country| {
            let country = country.clone();
            // Set loading state immediately when country changes
            loading.set(true);
            wasm_bindgen_futures::spawn_local(async move {
                let response = Api::get(&format!("/api/pricing/byot/{}", country)).send().await;
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
        }, country);
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
            let voice_calls = p.costs.voice_outbound_per_min
                .map(|v| (total_credits / v).floor() as i32);

            // Inbound voice (if local number country)
            let inbound_mins = if is_local {
                p.costs.voice_inbound_per_min.map(|v| (total_credits / v).floor() as i32)
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
        background: rgba(30, 144, 255, 0.1);
        border: 1px solid rgba(30, 144, 255, 0.3);
        border-radius: 8px;
        transition: all 0.2s ease;
        text-align: center;
        user-select: none;
    }
    .message-equivalent:hover {
        background: rgba(30, 144, 255, 0.2);
        border-color: rgba(30, 144, 255, 0.5);
    }
    .message-equivalent .value {
        font-size: 1.1rem;
        font-weight: 600;
        color: #7EB2FF;
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
        views.get(*current_view).cloned().unwrap_or_else(|| format!("{} messages", props.plan_messages))
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
        use_effect_with_deps(move |country| {
            let country = country.clone();
            wasm_bindgen_futures::spawn_local(async move {
                loading.set(true);
                error.set(None);
                let response = Api::get(&format!("/api/pricing/byot/{}", country)).send().await;
                match response {
                    Ok(resp) if resp.ok() => {
                        match resp.json::<ByotPricingResponse>().await {
                            Ok(data) => pricing.set(Some(data)),
                            Err(_) => error.set(Some("Failed to parse pricing".to_string())),
                        }
                    }
                    Ok(_) => error.set(Some("Country not supported".to_string())),
                    Err(e) => error.set(Some(format!("Failed to fetch: {}", e))),
                }
                loading.set(false);
            });
            || ()
        }, country);
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
        background: rgba(30, 144, 255, 0.05);
        border: 1px solid rgba(30, 144, 255, 0.2);
        border-radius: 12px;
        padding: 1rem;
        margin: 1rem 0;
    }
    .byot-pricing h4 {
        color: #7EB2FF;
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
        color: #7EB2FF;
        font-weight: 500;
    }
    .byot-monthly {
        text-align: center;
        padding: 0.5rem;
        background: rgba(30, 144, 255, 0.1);
        border-radius: 8px;
        margin-bottom: 0.75rem;
    }
    .byot-monthly .value {
        font-size: 1.1rem;
        color: #7EB2FF;
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
        border: 1px solid rgba(30, 144, 255, 0.3);
        border-radius: 6px;
        color: #e0e0e0;
        font-size: 0.85rem;
        cursor: pointer;
    }
    .byot-country-selector select:focus {
        outline: none;
        border-color: rgba(30, 144, 255, 0.6);
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
                            <option value="MY" selected={current_country == "MY"}>{"Malaysia"}</option>
                            <option value="TH" selected={current_country == "TH"}>{"Thailand"}</option>
                            <option value="PH" selected={current_country == "PH"}>{"Philippines"}</option>
                            <option value="ID" selected={current_country == "ID"}>{"Indonesia"}</option>
                            <option value="VN" selected={current_country == "VN"}>{"Vietnam"}</option>
                            <option value="IN" selected={current_country == "IN"}>{"India"}</option>
                            <option value="PK" selected={current_country == "PK"}>{"Pakistan"}</option>
                            <option value="BD" selected={current_country == "BD"}>{"Bangladesh"}</option>
                            <option value="LK" selected={current_country == "LK"}>{"Sri Lanka"}</option>
                        </optgroup>
                        <optgroup label="Americas">
                            <option value="MX" selected={current_country == "MX"}>{"Mexico"}</option>
                            <option value="BR" selected={current_country == "BR"}>{"Brazil"}</option>
                            <option value="AR" selected={current_country == "AR"}>{"Argentina"}</option>
                            <option value="CL" selected={current_country == "CL"}>{"Chile"}</option>
                            <option value="CO" selected={current_country == "CO"}>{"Colombia"}</option>
                            <option value="PE" selected={current_country == "PE"}>{"Peru"}</option>
                            <option value="VE" selected={current_country == "VE"}>{"Venezuela"}</option>
                            <option value="EC" selected={current_country == "EC"}>{"Ecuador"}</option>
                            <option value="CR" selected={current_country == "CR"}>{"Costa Rica"}</option>
                            <option value="PA" selected={current_country == "PA"}>{"Panama"}</option>
                            <option value="PR" selected={current_country == "PR"}>{"Puerto Rico"}</option>
                        </optgroup>
                        <optgroup label="Middle East">
                            <option value="IL" selected={current_country == "IL"}>{"Israel"}</option>
                            <option value="AE" selected={current_country == "AE"}>{"UAE"}</option>
                            <option value="SA" selected={current_country == "SA"}>{"Saudi Arabia"}</option>
                            <option value="QA" selected={current_country == "QA"}>{"Qatar"}</option>
                            <option value="KW" selected={current_country == "KW"}>{"Kuwait"}</option>
                            <option value="BH" selected={current_country == "BH"}>{"Bahrain"}</option>
                            <option value="OM" selected={current_country == "OM"}>{"Oman"}</option>
                            <option value="JO" selected={current_country == "JO"}>{"Jordan"}</option>
                            <option value="TR" selected={current_country == "TR"}>{"Turkey"}</option>
                        </optgroup>
                        <optgroup label="Africa">
                            <option value="ZA" selected={current_country == "ZA"}>{"South Africa"}</option>
                            <option value="NG" selected={current_country == "NG"}>{"Nigeria"}</option>
                            <option value="KE" selected={current_country == "KE"}>{"Kenya"}</option>
                            <option value="EG" selected={current_country == "EG"}>{"Egypt"}</option>
                            <option value="GH" selected={current_country == "GH"}>{"Ghana"}</option>
                            <option value="MA" selected={current_country == "MA"}>{"Morocco"}</option>
                            <option value="TN" selected={current_country == "TN"}>{"Tunisia"}</option>
                            <option value="TZ" selected={current_country == "TZ"}>{"Tanzania"}</option>
                            <option value="UG" selected={current_country == "UG"}>{"Uganda"}</option>
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

#[derive(Clone, PartialEq, Deserialize)]
struct CountryPricing {
    country_code: String,
    country_name: String,
    sms_price: f32,
    voice_price: f32,
}

#[derive(Clone, PartialEq, Deserialize)]
struct NotificationPricingResponse {
    countries: Vec<CountryPricing>,
    formula_note: String,
}

#[function_component(NotificationPricingTable)]
fn notification_pricing_table() -> Html {
    let pricing = use_state(|| None::<NotificationPricingResponse>);
    let loading = use_state(|| true);

    {
        let pricing = pricing.clone();
        let loading = loading.clone();
        use_effect_with_deps(move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                let response = Api::get("/api/pricing/notification-only").send().await;
                if let Ok(resp) = response {
                    if let Ok(data) = resp.json::<NotificationPricingResponse>().await {
                        pricing.set(Some(data));
                    }
                }
                loading.set(false);
            });
            || ()
        }, ());
    }

    let table_css = r#"
    .notification-pricing-table {
        margin-top: 1.5rem;
    }
    .notification-pricing-table h3 {
        color: #7EB2FF;
        font-size: 1.3rem;
        margin-bottom: 1rem;
    }
    .notification-pricing-table table {
        width: 100%;
        border-collapse: collapse;
        margin: 1rem 0;
    }
    .notification-pricing-table th,
    .notification-pricing-table td {
        padding: 0.75rem;
        text-align: left;
        border-bottom: 1px solid rgba(30, 144, 255, 0.15);
    }
    .notification-pricing-table th {
        color: #7EB2FF;
        font-weight: 600;
    }
    .notification-pricing-table td {
        color: #e0e0e0;
    }
    .notification-pricing-table tr:hover {
        background: rgba(30, 144, 255, 0.05);
    }
    .formula-note {
        color: #999;
        font-size: 0.85rem;
        margin-top: 1rem;
        font-style: italic;
    }
    "#;

    html! {
        <div class="notification-pricing-table">
            <style>{table_css}</style>
            <h3>{"Country Pricing"}</h3>
            if *loading {
                <p>{"Loading prices..."}</p>
            } else if let Some(data) = (*pricing).as_ref() {
                <table>
                    <thead>
                        <tr>
                            <th>{"Country"}</th>
                            <th>{"SMS Price"}</th>
                            <th>{"Voice (per min)"}</th>
                        </tr>
                    </thead>
                    <tbody>
                        { for data.countries.iter().map(|c| html! {
                            <tr>
                                <td>{&c.country_name}</td>
                                <td>{format!("€{:.2}", c.sms_price)}</td>
                                <td>{format!("€{:.2}", c.voice_price)}</td>
                            </tr>
                        })}
                    </tbody>
                </table>
                <p class="formula-note">{&data.formula_note}</p>
            } else {
                <p>{"Unable to load pricing"}</p>
            }
        </div>
    }
}

#[derive(Deserialize, Clone)]
struct UserProfile {
    id: i32,
    email: String,
    sub_tier: Option<String>,
    plan_type: Option<String>,
    phone_number: Option<String>,
    verified: bool,
    phone_number_country: Option<String>,
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
    pub verified: bool,
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
    let user_email = props.user_email.clone();
    let subscription_type = props.subscription_type.clone();
    let selected_country = props.selected_country.clone();
    let plan_type = props.plan_type.clone();

    // Check if subscriptions are blocked
    let subscriptions_blocked = false;

    let onclick = {
        let user_id = user_id.clone();
        let subscription_type = subscription_type.clone();
        let selected_country = selected_country.clone();
        let plan_type = plan_type.clone();

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let user_id = user_id.clone();
            let subscription_type = subscription_type.clone();
            let selected_country = selected_country.clone();
            let plan_type = plan_type.clone();

            if subscription_type != "basic" && subscription_type != "oracle" && selected_country == "Other" {
                if let Some(window) = web_sys::window() {
                    if !window.confirm_with_message(
                        "Have you contacted us to make sure the service is available in your country?"
                    ).unwrap_or(false) {
                        let email_url = "mailto:rasmus@ahtava.com";
                        let _ = window.location().set_href(email_url);
                        return;
                    }
                }
            }

            wasm_bindgen_futures::spawn_local(async move {
                let endpoint = format!("/api/stripe/unified-subscription-checkout/{}", user_id);
                let mut request_body = json!({
                    "subscription_type": match subscription_type.as_str() {
                        "hosted" => "Hosted",
                        "guaranteed" => "Guaranteed",
                        _ => "Hosted" // Default to Hosted if unknown
                    },
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
        background: linear-gradient(45deg, #1E90FF, #4169E1);
        color: white;
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
        transform: translateY(-2px);
        box-shadow: 0 4px 20px rgba(30, 144, 255, 0.3);
        background: linear-gradient(45deg, #4169E1, #1E90FF);
    }
    .iq-button.disabled {
        background: rgba(30, 30, 30, 0.5);
        cursor: not-allowed;
        border: 1px solid rgba(255, 255, 255, 0.1);
        opacity: 0.6;
    }
    .iq-button.disabled:hover {
        transform: none;
        box-shadow: none;
        background: rgba(30, 30, 30, 0.5);
    }
    .iq-button.current-plan {
        background: rgba(30, 144, 255, 0.3);
        border: 1px solid rgba(30, 144, 255, 0.5);
        cursor: default;
    }
    .iq-button.current-plan:hover {
        transform: none;
        box-shadow: none;
        background: rgba(30, 144, 255, 0.3);
    }
    .iq-button.coming-soon {
        background: rgba(255, 165, 0, 0.3);
        border: 1px solid rgba(255, 165, 0, 0.5);
        cursor: default;
    }
    .iq-button.coming-soon:hover {
        transform: none;
        box-shadow: none;
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
                                    error.set(Some("Failed to parse checkout response".to_string()));
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
        border: 2px solid rgba(30, 144, 255, 0.5);
        border-radius: 4px;
        background: rgba(30, 30, 30, 0.7);
        cursor: pointer;
        position: relative;
        margin-top: 2px;
        transition: all 0.2s ease;
    }
    .terms-checkbox-container input[type="checkbox"]:checked {
        background: #1E90FF;
        border-color: #1E90FF;
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
        border-color: #1E90FF;
    }
    .terms-checkbox-container a {
        color: #1E90FF;
        text-decoration: none;
    }
    .terms-checkbox-container a:hover {
        color: #7EB2FF;
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

#[derive(Clone, PartialEq)]
pub struct Addon {
    pub id: String,
    pub name: String,
    pub price: f64,
    pub description: String,
    pub currency: String,
    pub available: bool,
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
    pub verified: bool,
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
        if !props.verified {
            let onclick = Callback::from(|e: MouseEvent| {
                e.prevent_default();
                if let Some(window) = web_sys::window() {
                    let _ = window.location().set_href("/verify");
                }
            });
            html! { <button class="iq-button verify-required" onclick={onclick}><b>{"Verify Account to Subscribe"}</b></button> }
        } else if props.sub_tier.as_ref() == Some(&effective_tier)
            && (props.plan_type == props.user_plan_type
                || (props.user_plan_type.is_none() && props.selected_country != "Other")) {
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
        color: #1E90FF;
        text-decoration: none;
        font-size: 1.1rem;
        font-weight: 500;
        transition: color 0.3s ease;
    }
    .learn-more-link:hover {
        color: #7EB2FF;
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
        color: #7EB2FF;
        text-decoration: underline;
    }
    .pricing-card {
        flex: 1;
        min-width: 0;
        max-width: 100%;
        background: rgba(30, 30, 30, 0.8);
        border: 1px solid rgba(30, 144, 255, 0.15);
        border-radius: 24px;
        position: relative;
        transition: transform 0.3s ease, box-shadow 0.3s ease;
        backdrop-filter: blur(10px);
        box-sizing: border-box;
        display: flex;
        flex-direction: column;
        padding: 0;
        width: 100%;
    }
    .pricing-card:hover {
        transform: translateY(-5px);
        box-shadow: 0 8px 32px rgba(30, 144, 255, 0.2);
        border-color: rgba(30, 144, 255, 0.4);
    }
    .pricing-card.popular {
        background: linear-gradient(180deg, rgba(30, 144, 255, 0.1), rgba(30, 30, 30, 0.9));
        border: 2px solid #1E90FF;
        box-shadow: 0 4px 16px rgba(30, 144, 255, 0.3);
    }
    .pricing-card.popular:hover {
        box-shadow: 0 8px 32px rgba(30, 144, 255, 0.4);
    }
    .pricing-card.premium {
        background: rgba(40, 40, 40, 0.85);
        border: 2px solid rgba(255, 215, 0, 0.3);
    }
    .pricing-card.premium:hover {
        box-shadow: 0 8px 32px rgba(255, 215, 0, 0.3);
    }
    .popular-tag {
        position: absolute;
        top: -15px;
        right: 20px;
        background: linear-gradient(45deg, #1E90FF, #4169E1);
        color: white;
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
        padding: 1.5rem 2rem;
        text-align: center;
        border-bottom: 1px solid rgba(30, 144, 255, 0.15);
    }
    .card-header h3 {
        color: #7EB2FF;
        font-size: 1.8rem;
        margin: 0;
        font-weight: 700;
    }
    .card-content {
        padding: 1.5rem 2.5rem 2.5rem;
        flex-grow: 1;
        display: flex;
        flex-direction: column;
    }
    .best-for {
        color: #e0e0e0;
        font-size: 1.1rem;
        margin-top: 0.5rem;
        margin-bottom: 1.5rem;
        font-style: italic;
        text-align: center;
    }
    .price {
        margin: 1.5rem 0;
        text-align: center;
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 0.5rem;
    }
    .price .amount {
        font-size: 3.5rem;
        color: #fff;
        font-weight: 800;
        background: linear-gradient(45deg, #1E90FF, #7EB2FF);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
        line-height: 1;
    }
    .price .period {
        color: #999;
        font-size: 1.2rem;
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
        background: rgba(30, 144, 255, 0.1);
        border-radius: 8px;
        padding: 0.5rem;
    }
    .us-deal-text {
        color: #FFD700;
        font-size: 0.95rem;
        font-weight: 500;
    }
    .includes {
        margin-top: 2rem;
    }
    .quota-list {
        list-style: none;
        padding: 0;
        margin: 0;
    }
    .quota-list li {
        color: #e0e0e0;
        padding: 0.5rem 0;
        font-size: 1.1rem;
    }
    .quota-list li.sub-item {
        padding-left: 2rem;
        font-size: 1rem;
        color: #b0b0b0;
        position: relative;
    }
    .quota-list li.sub-item::before {
        content: "→";
        position: absolute;
        left: 1rem;
        color: #7EB2FF;
    }
    .iq-button {
        background: linear-gradient(45deg, #1E90FF, #4169E1);
        color: white;
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
        transform: translateY(-2px);
        box-shadow: 0 4px 20px rgba(30, 144, 255, 0.3);
        background: linear-gradient(45deg, #4169E1, #1E90FF);
    }
    .iq-button.disabled {
        background: rgba(30, 30, 30, 0.5);
        cursor: not-allowed;
        border: 1px solid rgba(255, 255, 255, 0.1);
    }
    .iq-button.disabled:hover {
        transform: none;
        box-shadow: none;
    }
    .iq-button.current-plan {
        background: rgba(30, 144, 255, 0.3);
        border: 1px solid rgba(30, 144, 255, 0.5);
        cursor: default;
    }
    .iq-button.current-plan:hover {
        transform: none;
        box-shadow: none;
        background: rgba(30, 144, 255, 0.3);
    }
    .iq-button.coming-soon {
        background: rgba(255, 165, 0, 0.3);
        border: 1px solid rgba(255, 165, 0, 0.5);
        cursor: default;
    }
    .iq-button.coming-soon:hover {
        transform: none;
        box-shadow: none;
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
    @media (max-width: 968px) {
        .pricing-card {
            min-width: 0;
            width: 100%;
            padding: 0;
        }
        .card-content {
            padding: 1rem;
        }
        .price .amount {
            font-size: 2.5rem;
        }
    }
    @media (min-width: 969px) {
        .pricing-card {
            flex: 0 1 calc(50% - 1rem);
        }
    }
.learn-more-section {
    text-align: center;
    margin-top: 1.5rem;
    margin-bottom: 1rem;
}
.learn-more-link {
    color: #1E90FF;
    text-decoration: none;
    font-size: 1.1rem;
    font-weight: 500;
    transition: color 0.3s ease;
}
.learn-more-link:hover {
    color: #7EB2FF;
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
                        { if (props.subscription_type == "hosted" || props.subscription_type == "guaranteed") && props.selected_country == "Other" {
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
#[function_component(FeatureList)]
pub fn feature_list(props: &FeatureListProps) -> Html {
    let base_messages_text: String = match props.selected_country.as_str() {
        "US" => "400 Messages per month included".to_string(),
        "CA" => "400 Messages per month included".to_string(),
        "FI" | "NL" | "GB" | "AU" => "Messages via prepaid credits".to_string(),
        c if is_notification_only_country(c) => "Notification service via US number - prepaid credits".to_string(),
        _ => "Bring your own Twilio for messages (pay Twilio directly)".to_string(),
    };
    let feature_css = r#"
    .feature-list {
        max-width: 1000px;
        margin: 4rem auto;
        background: rgba(30, 30, 30, 0.8);
        border: 1px solid rgba(30, 144, 255, 0.15);
        border-radius: 24px;
        padding: 2.5rem;
        backdrop-filter: blur(10px);
    }
    .feature-list h2 {
        color: #7EB2FF;
        font-size: 2rem;
        margin-bottom: 2rem;
        text-align: center;
    }
    .feature-list ul {
        list-style-type: none;
        padding: 0;
    }
    .feature-list li {
        color: #e0e0e0;
        padding: 0.5rem 0;
        font-size: 1.1rem;
        display: flex;
        align-items: center;
    }
    .feature-list li i {
        margin-right: 1rem;
        color: #7EB2FF;
        width: 1.2em; /* Fixed width for alignment */
        text-align: center;
    }
    @media (max-width: 968px) {
        .feature-list {
            padding: 1.5rem;
            margin: 2rem 1rem;
            max-width: calc(100vw - 2rem);
        }
    }
    "#;
    html! {
        <div class="feature-list">
            <style>{feature_css}</style>
            <h2>{"Included in All Plans"}</h2>
            <ul>
                <li><i class="fas fa-phone"></i>{"Voice calling and SMS interface"}</li>
                <li><i class="fas fa-comments"></i>{base_messages_text}</li>
                <li><i class="fas fa-search"></i>{"Perplexity AI Web Search"}</li>
                <li><i class="fas fa-cloud-sun"></i>{"Weather Search and forecast up to 7 days"}</li>
                <li><i class="fas fa-route"></i>{"Step-by-step Directions from Google Maps"}</li>
                <li><i class="fas fa-image"></i>{"Photo Analysis & Translation (US & AUS only)"}</li>
                <li><i class="fas fa-qrcode"></i>{"QR Code Scanning (US & AUS only)"}</li>
                <li><i class="fab fa-whatsapp"></i>{"Send, Fetch and Monitor WhatsApp Messages"}</li>
                <li><i class="fab fa-telegram"></i>{"Send, Fetch and Monitor Telegram Messages"}</li>
                <li><i class="fab fa-signal-messenger"></i>{"Send, Fetch and Monitor Signal Messages"}</li>
                <li><i class="fas fa-envelope"></i>{"Fetch, Send, Reply and Monitor Emails"}</li>
                <li><i class="fas fa-calendar-days"></i>{"Fetch, Create and Monitor Calendar events"}</li>
                <li><i class="fas fa-list-check"></i>{"Fetch and Create Tasks and Ideas"}</li>
                <li><i class="fas fa-car"></i>{"Tesla Vehicle Control (lock, unlock, climate, start, battery)"}</li>
                <li><i class="fas fa-eye"></i>{"24/7 Critical Message Monitoring"}</li>
                <li><i class="fas fa-newspaper"></i>{"Morning, Day and Evening Digests"}</li>
                <li><i class="fas fa-clock"></i>{"Custom Waiting Checks Specific Content"}</li>
                <li><i class="fas fa-bell"></i>{"Priority Sender Notifications"}</li>
                <li><i class="fas fa-rocket"></i>{"All Future Features Included"}</li>
                <li><i class="fas fa-headset"></i>{"Priority Support"}</li>
            </ul>
        </div>
    }
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
        use_effect_with_deps(move |country| {
            let country = country.clone();
            // Skip fetch for US/CA/Other
            if country != "US" && country != "CA" && country != "Other" {
                let pricing = pricing.clone();
                let loading = loading.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    loading.set(true);
                    let response = Api::get(&format!("/api/pricing/byot/{}", country)).send().await;
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
        }, country);
    }

    let credit_css = r#"
    .credit-pricing {
        max-width: 1000px;
        margin: 4rem auto;
        background: rgba(30, 30, 30, 0.8);
        border: 1px solid rgba(30, 144, 255, 0.15);
        border-radius: 24px;
        padding: 2.5rem;
        backdrop-filter: blur(10px);
        text-align: center;
    }
    .credit-pricing h2 {
        color: #7EB2FF;
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
        color: #1E90FF;
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
        let voice_out_price = p.costs.voice_outbound_per_min.map(|v| v * OVERAGE_MULTIPLIER);
        let voice_in_price = p.costs.voice_inbound_per_min.map(|v| v * OVERAGE_MULTIPLIER);

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
    // Monitor plan prices (€29 for euro countries, $29 for US/CA)
    let hosted_prices: HashMap<String, f64> = HashMap::from([
        ("US".to_string(), 29.00),
        ("CA".to_string(), 29.00),
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
        ("Other".to_string(), 19.00),  // BYOT plan stays at €19
    ]);
    // Digest plan prices (€49 for euro countries)
    let guaranteed_prices: HashMap<String, f64> = HashMap::from([
        ("US".to_string(), 59.00),
        ("CA".to_string(), 59.00),
        ("FI".to_string(), 49.00),
        ("NL".to_string(), 49.00),
        ("GB".to_string(), 49.00),
        ("AU".to_string(), 49.00),
        ("Other".to_string(), 49.00),
    ]);
    let hosted_total_price = hosted_prices.get(&props.selected_country).unwrap_or(&0.0);
    let guaranteed_total_price = guaranteed_prices.get(&props.selected_country).unwrap_or(&0.0);
    let hosted_features = vec![
        Feature {
            text: "Fully managed service hosted in EU".to_string(),
            sub_items: vec![],
        },
        Feature {
            text: "Simple setup, connect apps and go".to_string(),
            sub_items: vec![],
        },
        Feature {
            text: "Secure no-logging policy".to_string(),
            sub_items: vec![],
        },
        Feature {
            text: "All future updates, security, and priority support".to_string(),
            sub_items: vec![],
        },
    ];
    let guaranteed_features = vec![
        Feature {
            text: "Full Hosted Plan".to_string(),
            sub_items: vec![],
        },
        Feature {
            text: "Password Vault & Cheating Checker".to_string(),
            sub_items: vec!["Lightfriend password vault for app blockers and physical lock boxes, 60-min relock window or permanent downgrade.".to_string()],
        },
        Feature {
            text: "Free Cold Turkey Blocker Pro".to_string(),
            sub_items: vec!["Block computer temptations with no escape hatch.".to_string()],
        },
        Feature {
            text: "Optional Signup Bonuses".to_string(),
            sub_items: vec![
                "If needed: $20 for $40 Amazon gift card (for a dumbphone if you don't have one).".to_string(),
                "For smartphone locking: $10 for $20 Amazon gift card (for a smartphone lock box you can close with a password).".to_string(),
            ],
        },
    ];
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
        flex-wrap: wrap;
        gap: 2rem;
        justify-content: center;
        max-width: 1200px;
        margin: 2rem auto;
    }
    .hosted-plans-section, .self-hosted-plans-section {
        margin: 4rem auto;
        max-width: 1200px;
    }
    .section-title {
        text-align: center;
        color: #7EB2FF;
        font-size: 2.5rem;
        margin-bottom: 2rem;
    }
    .pricing-panel {
        position: relative;
        min-height: 100vh;
        padding: 6rem 2rem;
        color: #ffffff;
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
        background-image: url('/assets/rain.gif');
        background-size: cover;
        background-position: center;
        background-repeat: no-repeat;
        opacity: 0.8;
        z-index: -2;
        pointer-events: none;
    }
    .pricing-panel::after {
        content: '';
        position: fixed;
        top: 0;
        left: 0;
        width: 100%;
        height: 100vh;
        background: linear-gradient(
            to bottom,
            rgba(26, 26, 26, 0.75) 0%,
            rgba(26, 26, 26, 0.9) 100%
        );
        z-index: -1;
        pointer-events: none;
    }
    .pricing-header {
        text-align: center;
        margin-bottom: 4rem;
    }
    .pricing-header h1 {
        font-size: 3.5rem;
        margin-bottom: 1.5rem;
        background: linear-gradient(45deg, #fff, #7EB2FF);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
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
        border: 1px solid rgba(30, 144, 255, 0.15);
        max-width: 400px;
        margin: 2rem auto;
    }
    .country-selector label {
        color: #7EB2FF;
        margin-right: 1rem;
        font-size: 1.1rem;
    }
    .country-selector select {
        padding: 0.8rem;
        font-size: 1rem;
        border-radius: 8px;
        border: 1px solid rgba(30, 144, 255, 0.3);
        background: rgba(30, 30, 30, 0.9);
        color: #fff;
        cursor: pointer;
        transition: all 0.3s ease;
    }
    .country-selector select:hover {
        border-color: rgba(30, 144, 255, 0.5);
    }
    .pricing-faq {
        max-width: 800px;
        margin: 4rem auto;
    }
    .pricing-faq h2 {
        color: #7EB2FF;
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
        border: 1px solid rgba(30, 144, 255, 0.15);
        border-radius: 12px;
        padding: 1.5rem;
        transition: all 0.3s ease;
    }
    details:hover {
        border-color: rgba(30, 144, 255, 0.3);
    }
    summary {
        color: #7EB2FF;
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
        color: #7EB2FF;
        text-decoration: none;
        transition: color 0.3s ease;
    }
    .footnote a:hover {
        color: #1E90FF;
    }
    .github-link {
        color: #7EB2FF;
        font-size: 0.9rem;
        text-decoration: none;
        transition: color 0.3s ease;
    }
    .github-link:hover {
        color: #1E90FF;
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
        color: #7EB2FF;
    }
    .topup-pricing {
        max-width: 1000px;
        margin: 4rem auto;
        text-align: center;
    }
    .topup-pricing h2 {
        color: #7EB2FF;
        font-size: 2rem;
        margin-bottom: 1rem;
    }
    .topup-pricing p {
        color: #999;
        margin-bottom: 2rem;
    }
    .pricing-card.main {
        background: rgba(30, 30, 30, 0.8);
        border: 1px solid rgba(30, 144, 255, 0.15);
        padding: 2rem;
        min-width: 400px;
    }
    .package-row {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 1rem 0;
        border-bottom: 1px solid rgba(30, 144, 255, 0.15);
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
        color: #7EB2FF;
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
        border: 1px solid rgba(30, 144, 255, 0.15);
        border-radius: 24px;
        padding: 2.5rem;
        backdrop-filter: blur(10px);
        transition: transform 0.3s ease, box-shadow 0.3s ease;
    }
    .option-card:hover {
        transform: translateY(-5px);
        box-shadow: 0 8px 32px rgba(30, 144, 255, 0.15);
        border-color: rgba(30, 144, 255, 0.3);
    }
    .option-card h3 {
        color: #7EB2FF;
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
        border: 1px solid rgba(30, 144, 255, 0.15);
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
        color: #7EB2FF;
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
        background: rgba(30, 144, 255, 0.1);
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
        color: #7EB2FF;
        font-size: 1rem;
        font-weight: 500;
        min-width: 120px;
    }
    .quantity-selector-inline select {
        padding: 0.6rem 1rem;
        font-size: 0.95rem;
        border-radius: 8px;
        border: 1px solid rgba(30, 144, 255, 0.3);
        background: rgba(30, 30, 30, 0.9);
        color: #fff;
        cursor: pointer;
        transition: all 0.3s ease;
        min-width: 140px;
    }
    .quantity-selector-inline select:hover {
        border-color: rgba(30, 144, 255, 0.5);
    }
    .summary-item {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 0.25rem;
    }
    .summary-label {
        color: #7EB2FF;
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
        border: 1px solid rgba(30, 144, 255, 0.15);
        border-radius: 24px;
        padding: 2rem;
        backdrop-filter: blur(10px);
    }
    .time-value-section h2 {
        color: #7EB2FF;
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
                                    <a style="color: #1E90FF;" href="/supported-countries">{"Supported Countries"}</a>
                                    {" or by emailing "}
                                    <a style="color: #1E90FF;"
                                       href={format!("mailto:rasmus@ahtava.com?subject=Country%20Availability%20Inquiry%20for%20{}&body=Hey,%0A%0AIs%20the%20service%20available%20in%20{}%3F%0A%0AThanks,%0A",
                                       props.country_name.clone(), props.country_name.clone())}>
                                        {"rasmus@ahtava.com"}
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
                if !props.is_logged_in {
                    if let Some(on_change) = props.on_country_change.clone() {
                        html! {
                            <div class="country-selector">
                                <label for="country">{"Select your country: "}</label>
                                <select id="country" onchange={on_change}>
                                    <optgroup label="Full Service (local number)">
                                        <option value="US" selected={props.selected_country == "US"}>{"United States"}</option>
                                        <option value="CA" selected={props.selected_country == "CA"}>{"Canada"}</option>
                                        <option value="FI" selected={props.selected_country == "FI"}>{"Finland"}</option>
                                        <option value="NL" selected={props.selected_country == "NL"}>{"Netherlands"}</option>
                                        <option value="GB" selected={props.selected_country == "GB"}>{"United Kingdom"}</option>
                                        <option value="AU" selected={props.selected_country == "AU"}>{"Australia"}</option>
                                    </optgroup>
                                    <optgroup label="Notification Only (US number)">
                                        <option value="DE" selected={props.selected_country == "DE"}>{"Germany"}</option>
                                        <option value="FR" selected={props.selected_country == "FR"}>{"France"}</option>
                                        <option value="ES" selected={props.selected_country == "ES"}>{"Spain"}</option>
                                        <option value="IT" selected={props.selected_country == "IT"}>{"Italy"}</option>
                                        <option value="PT" selected={props.selected_country == "PT"}>{"Portugal"}</option>
                                        <option value="BE" selected={props.selected_country == "BE"}>{"Belgium"}</option>
                                        <option value="AT" selected={props.selected_country == "AT"}>{"Austria"}</option>
                                        <option value="CH" selected={props.selected_country == "CH"}>{"Switzerland"}</option>
                                        <option value="PL" selected={props.selected_country == "PL"}>{"Poland"}</option>
                                        <option value="CZ" selected={props.selected_country == "CZ"}>{"Czech Republic"}</option>
                                        <option value="SE" selected={props.selected_country == "SE"}>{"Sweden"}</option>
                                        <option value="DK" selected={props.selected_country == "DK"}>{"Denmark"}</option>
                                        <option value="NO" selected={props.selected_country == "NO"}>{"Norway"}</option>
                                        <option value="IE" selected={props.selected_country == "IE"}>{"Ireland"}</option>
                                        <option value="NZ" selected={props.selected_country == "NZ"}>{"New Zealand"}</option>
                                    </optgroup>
                                    <optgroup label="Other">
                                        <option value="Other" selected={props.selected_country == "Other"}>{"Other (bring your own number)"}</option>
                                    </optgroup>
                                </select>
                            </div>
                        }
                    } else {
                        html! {}
                    }
                } else {
                    html! {}
                }
            }
            <h2 class="section-title">{"Plans"}</h2>
            <div class="pricing-grid">
                {
                    if props.selected_country == "US" || props.selected_country == "CA" {
                        // US/CA: Show single Hosted Plan with messages included
                        html! {
                            <PricingCard
                                plan_name={"Hosted Plan"}
                                best_for={"Full-featured cloud service ready to go. Reclaim 2-4 hours per day* for just"}
                                price={*hosted_total_price}
                                currency={"$"}
                                period={"/month"}
                                features={hosted_features.clone()}
                                subscription_type={"hosted"}
                                is_popular={true}
                                is_premium={false}
                                user_id={props.user_id}
                                user_email={props.user_email.clone()}
                                is_logged_in={props.is_logged_in}
                                verified={props.verified}
                                sub_tier={props.sub_tier.clone()}
                                user_plan_type={props.user_plan_type.clone()}
                                selected_country={props.selected_country.clone()}
                                coming_soon={false}
                                plan_type={Some("monitor".to_string())}
                                hosted_prices={hosted_prices.clone()}
                            />
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
                                verified={props.verified}
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
                        // Euro countries: Show Monitor and Digest plans
                        // Both plans have ALL features - they only differ in prepaid message count
                        let is_local_number_country = matches!(props.selected_country.as_str(), "FI" | "NL" | "GB" | "AU");
                        let is_notification_only = is_notification_only_country(&props.selected_country);

                        let base_features: Vec<Feature> = if is_local_number_country {
                            vec![
                                Feature { text: "Local phone number included".to_string(), sub_items: vec![] },
                                Feature { text: "All features included".to_string(), sub_items: vec![] },
                            ]
                        } else {
                            // Notification-only countries
                            vec![
                                Feature { text: "Notifications from US number".to_string(), sub_items: vec!["Texting back may cost extra".to_string()] },
                                Feature { text: "All features included".to_string(), sub_items: vec![] },
                            ]
                        };

                        let mut monitor_features = base_features.clone();
                        monitor_features.push(Feature { text: "Go off-grid knowing critical alerts reach you".to_string(), sub_items: vec![] });
                        monitor_features.push(Feature { text: "Fixed monthly quota (no overage credits)".to_string(), sub_items: vec![] });

                        let mut digest_features = base_features;
                        digest_features.push(Feature { text: "Eliminates FOMO - stay caught up without checking".to_string(), sub_items: vec![] });
                        digest_features.push(Feature { text: "Can purchase overage credits if needed".to_string(), sub_items: vec![] });

                        let byot_features = vec![
                            Feature { text: "Bring your own Twilio number".to_string(), sub_items: vec![] },
                            Feature { text: "All features included".to_string(), sub_items: vec![] },
                            Feature { text: "No message limits - pay Twilio directly".to_string(), sub_items: vec![] },
                        ];

                        let selected_country_clone = props.selected_country.clone();
                        let selected_country_clone2 = props.selected_country.clone();

                        html! {
                            <>
                                <PricingCard
                                    plan_name={"Monitor Plan"}
                                    best_for={"Peace of mind. Get alerted to what matters, ignore the rest."}
                                    price={29.0}
                                    currency={"€"}
                                    period={"/month"}
                                    features={monitor_features}
                                    subscription_type={"hosted"}
                                    is_popular={false}
                                    is_premium={false}
                                    user_id={props.user_id}
                                    user_email={props.user_email.clone()}
                                    is_logged_in={props.is_logged_in}
                                    verified={props.verified}
                                    sub_tier={props.sub_tier.clone()}
                                    user_plan_type={props.user_plan_type.clone()}
                                    selected_country={props.selected_country.clone()}
                                    coming_soon={false}
                                    hosted_prices={hosted_prices.clone()}
                                    plan_type={Some("monitor".to_string())}
                                >
                                    <MessageEquivalentDisplay
                                        plan_messages={30}
                                        country={selected_country_clone}
                                    />
                                </PricingCard>
                                <PricingCard
                                    plan_name={"Digest Plan"}
                                    best_for={"Stay informed without the urge to check. Regular updates keep you ahead."}
                                    price={49.0}
                                    currency={"€"}
                                    period={"/month"}
                                    features={digest_features}
                                    subscription_type={"hosted"}
                                    is_popular={true}
                                    is_premium={false}
                                    user_id={props.user_id}
                                    user_email={props.user_email.clone()}
                                    is_logged_in={props.is_logged_in}
                                    verified={props.verified}
                                    sub_tier={props.sub_tier.clone()}
                                    user_plan_type={props.user_plan_type.clone()}
                                    selected_country={props.selected_country.clone()}
                                    coming_soon={false}
                                    hosted_prices={hosted_prices.clone()}
                                    plan_type={Some("digest".to_string())}
                                >
                                    <MessageEquivalentDisplay
                                        plan_messages={120}
                                        country={selected_country_clone2}
                                    />
                                </PricingCard>
                                // Show BYOT option for notification-only countries (they may want their own local number)
                                if is_notification_only {
                                    <PricingCard
                                        plan_name={"BYOT Plan"}
                                        best_for={"Want a local number? Bring Your Own Twilio for two-way messaging."}
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
                                        verified={props.verified}
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
                /*
                <PricingCard
                    plan_name={"Guaranteed Plan"}
                    best_for={"Hosted Plan with zero loop holes. Full refund for the first month if not satisfied."}
                    price={*guaranteed_total_price}
                    currency={if props.selected_country == "US" || props.selected_country == "CA" { "$" } else { "€" }}
                    period={"/month"}
                    features={guaranteed_features.clone()}
                    subscription_type={"guaranteed"}
                    is_popular={false}
                    is_premium={true}
                    user_id={props.user_id}
                    user_email={props.user_email.clone()}
                    is_logged_in={props.is_logged_in}
                    verified={props.verified}
                    sub_tier={props.sub_tier.clone()}
                    selected_country={props.selected_country.clone()}
                    coming_soon={false}
                    hosted_prices={hosted_prices.clone()}
                />
            */
            </div>
            <FeatureList selected_country={props.selected_country.clone()} />
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
                                    <p>{"Plans bill monthly. Hosted Plan includes everything from phone number to 400 messages per month in the US and Canada. No hidden fees. Not satisfied? Request a full refund within 7 days if you've used less than 30% of your credits."}</p>
                                </details>
                                <details>
                                    <summary>{"What counts as a Message?"}</summary>
                                    <p>{"Voice calls (1 min = 1 Message), text queries (1 query = 1 Message), daily digests (1 digest = 1 Message), priority sender notifications (1 notification = 1/2 Message)."}</p>
                                </details>
                                </>
                            }
                        } else if props.selected_country == "FI" || props.selected_country == "NL" || props.selected_country == "AU" || props.selected_country == "GB" {
                            html! {
                                <>
                                <details>
                                    <summary>{"How does billing work?"}</summary>
                                    <p>{"Plans bill monthly. Monitor (€29) includes 40 credits. Digest (€49) includes 120 credits. Phone number included. No hidden fees. Not satisfied? Request a full refund within 7 days if you've used less than 30% of your credits."}</p>
                                </details>
                                <details>
                                    <summary>{"What's the difference between Monitor and Digest?"}</summary>
                                    <p>{"Monitor is for critical notifications only - you get 40 credits and cannot buy more. Digest includes 120 credits with the ability to purchase additional credits when needed, perfect for daily summaries and regular use."}</p>
                                </details>
                                <details>
                                    <summary>{"How do credits work?"}</summary>
                                    <p>{"Different message types cost different amounts: notifications cost less than responses, which cost less than digests. The exact pricing is shown above based on your country's SMS rates. Digest plan users can purchase additional credits when needed."}</p>
                                </details>
                                </>
                            }
                        } else if is_notification_only_country(&props.selected_country) {
                            html! {
                                <>
                                <details>
                                    <summary>{"How does billing work?"}</summary>
                                    <p>{"Plans bill monthly. Monitor (€29) includes 40 credits. Digest (€49) includes 120 credits. Messages sent from a US number. No hidden fees. Not satisfied? Request a full refund within 7 days if you've used less than 30% of your credits."}</p>
                                </details>
                                <details>
                                    <summary>{"What's the difference between Monitor and Digest?"}</summary>
                                    <p>{"Monitor is for critical notifications only - you get 40 credits and cannot buy more. Digest includes 120 credits with the ability to purchase additional credits when needed, perfect for daily summaries and regular use."}</p>
                                </details>
                                <details>
                                    <summary>{"How do credits work?"}</summary>
                                    <p>{"Different message types cost different amounts: notifications cost less than digests. Voice calls also use credits. The exact pricing is shown above based on your country's SMS rates. Digest plan users can purchase additional credits when needed."}</p>
                                </details>
                                <details>
                                    <summary>{"Can I bring my own phone number?"}</summary>
                                    <p>{"Yes! If you want a local number for two-way messaging, you can use the BYOT (Bring Your Own Twilio) plan. This lets you set up your own Twilio account and pay messaging costs directly to them."}</p>
                                </details>
                                </>
                            }
                        } else {
                            html! {
                                <>
                                <details>
                                    <summary>{"How does billing work?"}</summary>
                                    <p>{"Plans bill monthly. Use the BYOT (Bring Your Own Twilio) plan to set up your own number and pay messaging costs directly to Twilio. No hidden fees. Not satisfied? Request a full refund within 7 days if you've used less than 30% of your credits."}</p>
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
                <Link<Route> to={Route::Changelog}>{"Updates"}</Link<Route>>
            </div>
        </div>
    }
}
