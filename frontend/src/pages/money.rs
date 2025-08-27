use yew::prelude::*;
use yew_router::prelude::*;
use crate::Route;
use yew_router::components::Link;
use serde_json::json;
use web_sys::{window, HtmlSelectElement};
use wasm_bindgen_futures;
use serde_json::Value;
use crate::config;
use gloo_net::http::Request;
use serde::Deserialize;
use std::collections::HashMap;
use wasm_bindgen::JsCast;
#[derive(Deserialize, Clone)]
struct UserProfile {
    id: i32,
    email: String,
    sub_tier: Option<String>,
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
    pub selected_addons: Vec<String>,
}
#[function_component(CheckoutButton)]
pub fn checkout_button(props: &CheckoutButtonProps) -> Html {
    let user_id = props.user_id;
    let user_email = props.user_email.clone();
    let subscription_type = props.subscription_type.clone();
    let selected_country = props.selected_country.clone();
    let selected_addons = props.selected_addons.clone();
    let onclick = {
        let user_id = user_id.clone();
        let subscription_type = subscription_type.clone();
        let selected_country = selected_country.clone();
        let selected_addons = selected_addons.clone();
  
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let user_id = user_id.clone();
            let subscription_type = subscription_type.clone();
            let selected_addons = selected_addons.clone();
            let selected_country = selected_country.clone();
      
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
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|storage| storage.get_item("token").ok())
                    .flatten()
                {
                    let endpoint = format!("{}/api/stripe/unified-subscription-checkout/{}", config::get_backend_url(), user_id);
                    let request_body = json!({
                        "subscription_type": match subscription_type.as_str() {
                            "hosted" => "Hosted",
                            "guaranteed" => "Guaranteed",
                            _ => "Hosted" // Default to Hosted if unknown
                        },
                        "addons": selected_addons,
                        "trial_days": if selected_country == "US" || selected_country == "CA" { 7 } else { 0 },
                    });
                    let response = Request::post(&endpoint)
                        .header("Authorization", &format!("Bearer {}", token))
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
                }
            });
        })
    };
    let button_text = "Subscribe";
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
    "#;
    html! {
        <>
            <style>{button_css}</style>
            <button class="iq-button signup-button" {onclick}><b>{button_text}</b></button>
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
    pub is_trial: bool,
    pub user_id: i32,
    pub user_email: String,
    pub is_logged_in: bool,
    pub verified: bool,
    pub sub_tier: Option<String>,
    pub selected_country: String,
    #[prop_or(false)]
    pub coming_soon: bool,
    pub hosted_prices: HashMap<String, f64>,
    #[prop_or_default]
    pub children: Children,
}
#[function_component(PricingCard)]
pub fn pricing_card(props: &PricingCardProps) -> Html {
    let selected_addons = use_state(|| vec![]);
    let addon_total = use_state(|| 0.0);
    let is_european = ["FI", "NL", "UK"].contains(&props.selected_country.as_str());
    let is_giftcard_region = ["US", "CA", "AU"].contains(&props.selected_country.as_str());
    let addon_currency = if is_giftcard_region || props.selected_country == "US" || props.selected_country == "CA" { "$" } else { "€" };
    let available_physical = is_european || is_giftcard_region;
    let dumbphone_id = if is_european { "dumbphone_ship".to_string() } else { "dumbphone_gift".to_string() };
    let dumbphone_name = if is_european { "Dumbphone (Shipped)".to_string() } else { "Amazon Gift Card for Dumbphone".to_string() };
    let dumbphone_price = if is_european { 50.0 } else { 30.0 };
    let dumbphone_description = if is_european {
        "Get a dumbphone shipped to you.".to_string()
    } else {
        "Get $40 Amazon gift card for a dumbphone. ".to_string()
    };
    let ubikey_id = if is_european { "ubikey_ship".to_string() } else { "ubikey_gift".to_string() };
    let ubikey_name = if is_european { "UbiKey Security Key (Shipped)".to_string() } else { "Amazon Gift Card for YubiKey".to_string() };
    let ubikey_price = if is_european { 90.0 } else { 45.0 };
    let ubikey_description = if is_european {
        "Add a UbiKey for enhanced 2FA security (shipped to you).".to_string()
    } else {
        "Get $55 Amazon gift card and link to buy YubiKey 5C NFC: https://www.amazon.com/Yubico-Two-factor-authentication-security-certified/dp/B08DHL1YDL".to_string()
    };
    let addons = vec![
        Addon {
            id: dumbphone_id,
            name: dumbphone_name,
            price: dumbphone_price,
            description: dumbphone_description,
            currency: addon_currency.to_string(),
            available: available_physical,
        },
        Addon {
            id: ubikey_id,
            name: ubikey_name,
            price: ubikey_price,
            description: ubikey_description,
            currency: addon_currency.to_string(),
            available: available_physical,
        },
        Addon {
            id: "cold_turkey".to_string(),
            name: "20% Off Cold Turkey Blocker".to_string(),
            price: 0.0,
            description: "Get 20% discount on Cold Turkey Blocker (code provided after signup).".to_string(),
            currency: addon_currency.to_string(),
            available: true, // Always available, as it's digital
        },
    ];
    let on_addon_change = {
        let selected_addons = selected_addons.clone();
        let addon_total = addon_total.clone();
        let addons = addons.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target().unwrap().dyn_into().unwrap();
            let addon_id = input.id();
            let mut new_addons = (*selected_addons).clone();
            if input.checked() {
                new_addons.push(addon_id.clone());
            } else {
                new_addons.retain(|a| a != &addon_id);
            }
            selected_addons.set(new_addons.clone());
            let new_total: f64 = new_addons.iter().filter_map(|id| {
                addons.iter().find(|a| a.id == *id).map(|a| a.price)
            }).sum();
            addon_total.set(new_total);
        })
    };
    let price_text = if props.subscription_type == "hosted" {
        format!("{}{:.2}", props.currency, props.price / 30.00) // Normal pricing for other plans
    } else {
        format!("{}{:.2}", props.currency, props.price)
    };
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
        } else if props.sub_tier.as_ref() == Some(&effective_tier) {
            html! { <button class="iq-button current-plan" disabled=true><b>{"Current Plan"}</b></button> }
        } else {
            html! {
                <CheckoutButton
                    user_id={props.user_id}
                    user_email={props.user_email.clone()}
                    subscription_type={props.subscription_type.clone()}
                    selected_country={props.selected_country.clone()}
                    selected_addons={(*selected_addons).clone()}
                />
            }
        }
    } else {
        let subscription_type = props.subscription_type.clone();
        let selected_addons = (*selected_addons).clone();
        let onclick = Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let subscription_type = subscription_type.clone();
            let selected_addons = selected_addons.clone();
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    let _ = storage.set_item("selected_plan", &subscription_type);
                    let _ = storage.set_item("selected_addons", &serde_json::to_string(&selected_addons).unwrap_or_default());
                    let _ = window.location().set_href("/register");
                }
            }
        });
        html! { <button onclick={onclick} class="iq-button signup-button"><b>{"Get Started"}</b></button> }
    };
    let image_url = "/assets/hosted-image.png";
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
    .trial-tag {
        position: absolute;
        top: -15px;
        left: 20px;
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
    .header-background {
        position: relative;
        height: 350px;
        background-size: cover;
        background-position: center;
        display: flex;
        align-items: center;
        text-align: center;
        justify-content: center;
        border-top-left-radius: 24px;
        border-top-right-radius: 24px;
    }
    .header-background::before {
        content: '';
        position: absolute;
        top: 0;
        left: 0;
        width: 100%;
        height: 100%;
        background: rgba(0, 0, 0, 0.3);
        border-top-left-radius: 24px;
        border-top-right-radius: 24px;
    }
    .header-background h3 {
        color: #ffffff;
        font-size: 2rem;
        text-shadow: 2px 2px 4px rgba(0, 0, 0, 0.7);
        z-index: 1;
        margin: 0;
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
            padding: 1rem;
        }
        .header-background {
            height: 200px;
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
                } else if props.is_premium {
                    html! { <div class="premium-tag">{"Money Back Guarantee"}</div> }
                } else {
                    html! {}
                }
            }
            {
                if props.is_trial {
                    html! { <div class="trial-tag">{"7-Day Free Trial"}</div> }
                } else {
                    html! {}
                }
            }
            <div class="header-background" style={format!("background-image: url({});", image_url)}>
                <h3>{props.plan_name.clone()}</h3>
            </div>
            <div class="card-content">
                { for props.children.iter() }
                <p class="best-for">{props.best_for.clone()}</p>
                <div class="price">
                    <span class="amount">{price_text}</span>
                    <span class="period">{props.period.clone()}</span>
                    { if props.subscription_type == "hosted" {
                        html! {
                            <p class="billing-note">
                                {if props.is_trial {"7-day free trial, then "} else {""} }{format!("billed monthly at {}{:.2}", props.currency, props.price)}
                            </p>
                        }
                    } else if props.subscription_type == "guaranteed" {
                        html! {
                            <p class="billing-note">
                                {format!("Billed monthly at {}{:.2}", props.currency, props.price)}
                            </p>
                        }
                    } else {
                        html! {}
                    }}
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
                            html! { <li>{"Required: Bring your own number and unlock service in whatever country you're in. Checkout the guide below."}</li> }
                        } else if (props.subscription_type == "hosted" || props.subscription_type == "guaranteed") && ["FI", "NL", "UK", "AU"].contains(&props.selected_country.as_str()) {
                            html! {
                                <>
                                    <li>{"Messages not included - buy credits ahead of time. Credits are used for sending messages, voice calls, notifications, and more."}</li>
                                    <li>{"Get 10€ free credits on signup - buy more when needed."}</li>
                                </>
                            }
                        } else { html! {} }}
                    </ul>
                </div>
                <div class="addons-section">
                    <h4>{"Optional Addons (One-Time at Signup)"}</h4>
                    <ul class="addon-list">
                        { for addons.iter().map(|addon| {
                            let on_addon_change = on_addon_change.clone();
                            html! {
                                <li>
                                    <input type="checkbox" id={addon.id.clone()} onchange={on_addon_change} disabled={!addon.available} />
                                    <label for={addon.id.clone()}>{addon.name.clone()}{ if addon.price > 0.0 { format!(" - {}{:.2}", addon.currency, addon.price) } else { "".to_string() } }</label>
                                    <p class="addon-desc">{addon.description.clone()}</p>
                                </li>
                            }
                        }) }
                    </ul>
                    <p class="addon-total">{"Addon Total: "}{format!("{}{:.2}", addon_currency, *addon_total)}</p>
                </div>
                {
                    if (props.subscription_type == "hosted" || props.subscription_type == "guaranteed") && props.selected_country == "Other" {
                        html! {
                            <div class="learn-more-section">
                                <a href="/bring-own-number" class="learn-more-link">{"How to bring your own number"}</a>
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
        "FI" | "NL" | "UK" | "AU" => "Messages via prepaid credits".to_string(),
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
                <li><i class="fas fa-cloud-sun"></i>{"Weather Search and forecast of the next 6 hours"}</li>
                <li><i class="fas fa-route"></i>{"Step-by-step Directions from Google Maps"}</li>
                <li><i class="fas fa-image"></i>{"Photo Analysis & Translation (US & AUS only)"}</li>
                <li><i class="fas fa-qrcode"></i>{"QR Code Scanning (US & AUS only)"}</li>
                <li><i class="fab fa-whatsapp"></i>{"Send, Fetch and Monitor WhatsApp Messages"}</li>
                <li><i class="fab fa-telegram"></i>{"Send, Fetch and Monitor Telegram Messages"}</li>
                <li><i class="fab fa-signal-messenger"></i>{"Send, Fetch and Monitor Signal Messages"}</li>
                <li><i class="fas fa-envelope"></i>{"Fetch, Send, Reply and Monitor Emails"}</li>
                <li><i class="fas fa-calendar-days"></i>{"Fetch, Create and Monitor Calendar events"}</li>
                <li><i class="fas fa-list-check"></i>{"Fetch and Create Tasks and Ideas"}</li>
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
        margin-bottom: 1rem;
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
    @media (max-width: 968px) {
        .credit-pricing {
            padding: 1.5rem;
            margin: 2rem 1rem;
            max-width: calc(100vw - 2rem);
        }
    }
    "#;
    if country == "Other" {
        html! {
            <div class="credit-pricing">
                <style>{credit_css}</style>
                <h2>{"Messaging Costs"}</h2>
                <p>{"To see prices for your country and how to set up, check our guide page. It has info on rules too."}</p>
                <a href="/bring-own-number">{"See Setup Guide and Prices"}</a>
            </div>
        }
    } else {
        let currency = if country == "US" || country == "CA" { "$" } else { "€" };
        let (msg_cost, voice_sec_cost, noti_msg_cost, noti_call_cost) = match country.as_str() {
            "US" => (0.075, 0.0033, 0.075, 0.15),
            "CA" => (0.075, 0.0033, 0.075, 0.15),
            "FI" => (0.30, 0.005, 0.15, 0.70),
            "NL" => (0.30, 0.005, 0.15, 0.45),
            "UK" => (0.30, 0.005, 0.15, 0.20),
            "AU" => (0.30, 0.005, 0.15, 0.20),
            _ => (0.0, 0.0, 0.0, 0.0),
        };
        let voice_min_cost = voice_sec_cost * 60.0;
        if country != "US" && country != "CA" {
            html! {
                <div class="credit-pricing">
                    <style>{credit_css}</style>
                    <h2>{"Message Costs (Credits)"}</h2>
                    <p>{"Credits work like this: Buy them ahead with money. Use them for texts, calls, and notifications. Buy more when low."}</p>
                    <p>{"It's easy. For example, add $10 in credits. Then send texts until used up."}</p>
                    <p>{"You can set filters for what sends notifications. This way, no surprise costs."}</p>
                    <p>{"Costs for each:"}</p>
                    <ul>
                        <li>{"Text message: "}{currency}{format!("{:.2}", msg_cost)}{" each"}</li>
                        <li>{"Voice call per minute: "}{currency}{format!("{:.2}", voice_min_cost)}{" (or "}{currency}{format!("{:.4}", voice_sec_cost)}{" per second)"}</li>
                        <li>{"Notification text: "}{currency}{format!("{:.2}", noti_msg_cost)}{" each"}</li>
                        <li>{"Notification call: "}{currency}{format!("{:.2}", noti_call_cost)}{" each"}</li>
                        <li>{"Daily summary: "}{currency}{format!("{:.2}", msg_cost)}</li>
                    </ul>
                </div>
            }
        } else {
            html! {}
        }
    }
}
#[function_component(UnifiedPricing)]
pub fn unified_pricing(props: &PricingProps) -> Html {
    let hosted_prices: HashMap<String, f64> = HashMap::from([
        ("US".to_string(), 19.00),
        ("CA".to_string(), 19.00),
        ("FI".to_string(), 19.00),
        ("NL".to_string(), 19.00),
        ("UK".to_string(), 19.00),
        ("AU".to_string(), 19.00),
        ("Other".to_string(), 19.00),
    ]);
    let guaranteed_prices: HashMap<String, f64> = HashMap::from([
        ("US".to_string(), 59.00),
        ("CA".to_string(), 59.00),
        ("FI".to_string(), 59.00),
        ("NL".to_string(), 59.00),
        ("UK".to_string(), 59.00),
        ("AU".to_string(), 59.00),
        ("Other".to_string(), 59.00),
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
                                    { for ["US", "CA", "FI", "NL", "UK", "AU", "Other"]
                                        .iter()
                                        .map(|&c| html! {
                                            <option value={c} selected={props.selected_country == c}>{c}</option>
                                        })
                                    }
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
                <PricingCard
                    plan_name={"Hosted Plan"}
                    best_for={"Full-featured cloud service ready to go. Reclaim 2-4 hours per day* for just"}
                    price={*hosted_total_price}
                    currency={if props.selected_country == "US" || props.selected_country == "CA" { "$" } else { "€" }}
                    period={"/day"}
                    features={hosted_features.clone()}
                    subscription_type={"hosted"}
                    is_popular={true}
                    is_premium={false}
                    is_trial={props.selected_country == "US" || props.selected_country == "CA"}
                    user_id={props.user_id}
                    user_email={props.user_email.clone()}
                    is_logged_in={props.is_logged_in}
                    verified={props.verified}
                    sub_tier={props.sub_tier.clone()}
                    selected_country={props.selected_country.clone()}
                    coming_soon={false}
                    hosted_prices={hosted_prices.clone()}
                />
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
                    is_trial={false}
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
                                    <p>{"Plans bill monthly. Hosted Plan includes everything from phone number to 400 in the US and Canada. No hidden fees, but no refunds — I'm a bootstrapped solo dev."}</p>
                                </details>
                                <details>
                                    <summary>{"What counts as a Message?"}</summary>
                                    <p>{"Voice calls (1 min = 1 Message), text queries (1 query = 1 Message), daily digests (1 digest = 1 Message), priority sender notifications (1 notification = 1/2 Message). Critical monitoring and custom checks are included."}</p>
                                </details>
                                </>
                            }
                        } else if props.selected_country == "FI" || props.selected_country == "NL" || props.selected_country == "AU" || props.selected_country == "UK" {
                            html! {
                                <>
                                <details>
                                    <summary>{"How does billing work?"}</summary>
                                    <p>{"Plans bill monthly. Hosted Plan includes phone number for FI/AU/UK/NL, but messages are bought separately before hand. No hidden fees, but no refunds — I'm a bootstrapped solo dev."}</p>
                                </details>
                                <details>
                                    <summary>{"Can I setup automatic recharge for message credits?"}</summary>
                                    <p>{"Yes! After you purchase them the first time, you can set it recharge the specified amount."}</p>
                               </details>
                                </>
                            }
                        } else {
                            html! {
                                <>
                                <details>
                                    <summary>{"How does billing work?"}</summary>
                                    <p>{"Plans bill monthly. Messages or phone number are not included. Checkout the guide on how to bring your own Number from the pricing card. No hidden fees, but no refunds — I'm a bootstrapped solo dev."}</p>
                                </details>
                                <details>
                                    <summary>{"Can I setup automatic recharge for message credits?"}</summary>
                                    <p>{"Yes! After you purchase them the first time, you can set it recharge the specified amount."}</p>
                                </details>
                                </>
                            }
                        }
                    }
                    <details>
                        <summary>{"Is it available in my country?"}</summary>
                        <p>{"Available globally. US/CA everything is included. FI/UK/AU/NL include a number but message are bought separately. Elsewhere bring your own number (guided setup, costs vary ~€0.05-0.50/text or free if you have extra android phone laying around with a another phone plan). Contact rasmus@ahtava.com for more details."}</p>
                    </details>
                    <details>
                        <summary>{"Why do the plan offering differ per country?"}</summary>
                        <p>{"Different countries have vastly different SMS costs - in the US and Canada, messages cost about 10x less, while in other countries they can cost up to $1 per message. To keep pricing fair and affordable, messages are included only in US/CA plans. For other countries, you can buy message credits as needed, giving you more control over costs."}</p>
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
