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
    pub phone_country: Option<String>,
}
#[derive(Properties, PartialEq, Clone)]
pub struct CheckoutButtonProps {
    pub user_id: i32,
    pub user_email: String,
    pub subscription_type: String,
    pub selected_country: String,
}
#[function_component(CheckoutButton)]
pub fn checkout_button(props: &CheckoutButtonProps) -> Html {
    let user_id = props.user_id;
    let user_email = props.user_email.clone();
    let subscription_type = props.subscription_type.clone();
    let selected_country = props.selected_country.clone();
    let onclick = {
        let user_id = user_id.clone();
        let subscription_type = subscription_type.clone();
        let selected_country = selected_country.clone();
     
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let user_id = user_id.clone();
            let subscription_type = subscription_type.clone();
         
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
                />
            }
        }
    } else {
        let subscription_type = props.subscription_type.clone();
        let onclick = Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let subscription_type = subscription_type.clone();
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    let _ = storage.set_item("selected_plan", &subscription_type);
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
                                {format!("Billed monthly at {}{:.2}", props.currency, props.price)}
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
                <li><i class="fas fa-envelope"></i>{"Fetch and Monitor Emails"}</li>
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
        if country != "US" || country != "CA" {
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

#[function_component(PricingLoggedIn)]
pub fn pricing_logged_in(props: &PricingProps) -> Html {
    let selected_country = use_state(|| "US".to_string());
    let country_name = use_state(|| String::new());
    {
        use_effect_with_deps(
            move |_| {
                if let Some(window) = web_sys::window() {
                    window.scroll_to_with_x_and_y(0.0, 0.0);
                }
                || ()
            },
            (),
        );
    }
    {
        let selected_country_state = selected_country.clone();
        let country_name_state = country_name.clone();
        let is_logged_in = props.is_logged_in;
        use_effect_with_deps(
            {
                let phone_country = props.phone_country.clone();
                let selected_country = selected_country_state.clone();
                let country_name = country_name_state.clone();
                move |_| {
                    if let Some(country) = phone_country {
                        selected_country.set(country);
                        match selected_country.as_str() {
                            "US" => country_name.set("United States".to_string()),
                            "CA" => country_name.set("Canada".to_string()),
                            "FI" => country_name.set("Finland".to_string()),
                            "NL" => country_name.set("Netherlands".to_string()),
                            "UK" => country_name.set("United Kingdom".to_string()),
                            "AU" => country_name.set("Australia".to_string()),
                            _ => country_name.set("Other".to_string()),
                        }
                    }
                    || ()
                }
            },
            (is_logged_in, props.phone_number.clone()),
        );
    }
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
    let on_country_change = {
        let selected_country = selected_country.clone();
        Callback::from(move |e: Event| {
            if let Some(target) = e.target_dyn_into::<HtmlSelectElement>() {
                selected_country.set(target.value());
            }
        })
    };
    let hosted_total_price = hosted_prices.get(&*selected_country).unwrap_or(&0.0);
    let guaranteed_total_price = guaranteed_prices.get(&*selected_country).unwrap_or(&0.0);
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
    let currency_symbol = if *selected_country == "US" || *selected_country == "CA" { "$" } else { "€" };
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
        background-image: url('/assets/human_looking_at_field.webp');
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
                <p>{"Lightfriend makes it possible to seriously switch to a dumbphone, saving you 2-4 hours per day of mindless scrolling.*"}</p>
                <p>{"Save 120 hours/month starting at €19!"}</p>
                {
                    if *selected_country == "Other" {
                        html! {
                            <>
                            <br/>
                            <p class="availability-note" style="color: #ff9494; font-size: 0.9rem; margin-top: 0.5rem;">
                                {format!("Note: Service may be limited or unavailable in {}. ", (*country_name).clone())}
                                {" More info about supported countries can be checked in "}
                                <span class="legal-links">
                                    <a style="color: #1E90FF;" href="/supported-countries">{"Supported Countries"}</a>
                                    {" or by emailing "}
                                    <a style="color: #1E90FF;"
                                       href={format!("mailto:rasmus@ahtava.com?subject=Country%20Availability%20Inquiry%20for%20{}&body=Hey,%0A%0AIs%20the%20service%20available%20in%20{}%3F%0A%0AThanks,%0A",
                                       (*country_name).clone(), (*country_name).clone())}>
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
                    html! {
                        <div class="country-selector">
                            <label for="country">{"Select your country: "}</label>
                            <select id="country" onchange={on_country_change}>
                                { for ["US", "CA", "FI", "NL", "UK", "AU", "Other"]
                                    .iter()
                                    .map(|&c| html! {
                                        <option value={c} selected={*selected_country == c}>{c}</option>
                                    })
                                }
                            </select>
                        </div>
                    }
                } else {
                    html! {}
                }
            }
            <h2 class="section-title">{"Plans"}</h2>
            <div class="pricing-grid">
                <PricingCard
                    plan_name={"Hosted Plan"}
                    best_for={"Full-featured cloud service ready to go."}
                    price={*hosted_total_price}
                    currency={if *selected_country == "US" || *selected_country == "CA" { "$" } else { "€" }}
                    period={"/day"}
                    features={hosted_features.clone()}
                    subscription_type={"hosted"}
                    is_popular={true}
                    is_premium={false}
                    is_trial={false}
                    user_id={props.user_id}
                    user_email={props.user_email.clone()}
                    is_logged_in={props.is_logged_in}
                    verified={props.verified}
                    sub_tier={props.sub_tier.clone()}
                    selected_country={(*selected_country).clone()}
                    coming_soon={false}
                    hosted_prices={hosted_prices.clone()}
                />
                /*
                <PricingCard
                    plan_name={"Guaranteed Plan"}
                    best_for={"Hosted Plan with zero loop holes. Full refund for the first month if not satisfied."}
                    price={*guaranteed_total_price}
                    currency={if *selected_country == "US" || *selected_country == "CA" { "$" } else { "€" }}
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
                    selected_country={(*selected_country).clone()}
                    coming_soon={false}
                    hosted_prices={hosted_prices.clone()}
                />
            */
            </div>
            <FeatureList selected_country={(*selected_country).clone()} />
            <CreditPricing selected_country={(*selected_country).clone()} />
            <div class="pricing-faq">
                <h2>{"Common Questions"}</h2>
                <div class="faq-grid">
                    {
                        if (*selected_country) == "US" || (*selected_country) == "CA" {
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
                        } else if (*selected_country) == "FI" || (*selected_country) == "NL"  || (*selected_country) == "AU" || (*selected_country) == "UK" {
                            html! {
                                <>
                                <details>
                                    <summary>{"How does billing work?"}</summary>
                                    <p>{"Plans bill monthly. Hosted Plan includes phone number for FI/AU/UK, but messages are bought separately before hand. No hidden fees, but no refunds — I'm a bootstrapped solo dev."}</p>
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
                        <p>{"Available globally. US/CA everything is included. FI/UK/AU include a number but message are bought separately. Elsewhere bring your own number (guided setup, costs vary ~€0.05-0.50/text or free if you have extra android phone laying around with a another phone plan). Contact rasmus@ahtava.com for more details."}</p>
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

#[function_component(Pricing)]
pub fn pricing(props: &PricingProps) -> Html {
    let selected_country = use_state(|| "US".to_string());
    let country_name = use_state(|| String::new());
    {
        use_effect_with_deps(
            move |_| {
                if let Some(window) = web_sys::window() {
                    window.scroll_to_with_x_and_y(0.0, 0.0);
                }
                || ()
            },
            (),
        );
    }
    {
        let selected_country_state = selected_country.clone();
        let country_name_state = country_name.clone();
        let is_logged_in = props.is_logged_in;
        use_effect_with_deps(
            {
                let phone_country = props.phone_country.clone();
                let selected_country = selected_country_state.clone();
                let country_name = country_name_state.clone();
                move |_| {
                    if is_logged_in {
                        if let Some(country) = phone_country {
                            selected_country.set(country);
                            match selected_country.as_str() {
                                "US" => country_name.set("United States".to_string()),
                                "CA" => country_name.set("Canada".to_string()),
                                "FI" => country_name.set("Finland".to_string()),
                                "NL" => country_name.set("Netherlands".to_string()),
                                "UK" => country_name.set("United Kingdom".to_string()),
                                "AU" => country_name.set("Australia".to_string()),
                                _ => country_name.set("Other".to_string()),
                            }
                        }
                    } else {
                        let selected_country = selected_country.clone();
                        let country_name = country_name.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            if let Ok(response) = Request::get("https://ipapi.co/json/").send().await {
                                if let Ok(json) = response.json::<Value>().await {
                                    if let Some(code) = json.get("country_code").and_then(|c| c.as_str()) {
                                        let code = code.to_uppercase();
                                        if let Some(name) = json.get("country_name").and_then(|c| c.as_str()) {
                                            country_name.set(name.to_string());
                                        }
                                        if ["US", "CA", "FI", "NL", "UK", "AU"].contains(&code.as_str()) {
                                            selected_country.set(code);
                                        } else {
                                            selected_country.set("Other".to_string());
                                        }
                                    }
                                }
                            }
                        });
                    }
                    || ()
                }
            },
            (is_logged_in, props.phone_number.clone()),
        );
    }
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
    let on_country_change = {
        let selected_country = selected_country.clone();
        Callback::from(move |e: Event| {
            if let Some(target) = e.target_dyn_into::<HtmlSelectElement>() {
                selected_country.set(target.value());
            }
        })
    };
    let hosted_total_price = hosted_prices.get(&*selected_country).unwrap_or(&0.0);
    let guaranteed_total_price = guaranteed_prices.get(&*selected_country).unwrap_or(&0.0);
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
    let currency_symbol = if *selected_country == "US" || *selected_country == "CA" { "$" } else { "€" };
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
        background-image: url('/assets/human_looking_at_field.webp');
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
                <p>{"Lightfriend makes it possible to seriously switch to a dumbphone, saving you 2-4 hours per day of mindless scrolling.*"}</p>
                {
                    if *selected_country == "Other" {
                        html! {
                            <>
                            <br/>
                            <p class="availability-note" style="color: #ff9494; font-size: 0.9rem; margin-top: 0.5rem;">
                                {format!("Note: Service may be limited or unavailable in {}. ", (*country_name).clone())}
                                {" More info about supported countries can be checked in "}
                                <span class="legal-links">
                                    <a style="color: #1E90FF;" href="/supported-countries">{"Supported Countries"}</a>
                                    {" or by emailing "}
                                    <a style="color: #1E90FF;"
                                       href={format!("mailto:rasmus@ahtava.com?subject=Country%20Availability%20Inquiry%20for%20{}&body=Hey,%0A%0AIs%20the%20service%20available%20in%20{}%3F%0A%0AThanks,%0A",
                                       (*country_name).clone(), (*country_name).clone())}>
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
                    html! {
                        <div class="country-selector">
                            <label for="country">{"Select your country: "}</label>
                            <select id="country" onchange={on_country_change}>
                                { for ["US", "CA", "FI", "NL", "UK", "AU", "Other"]
                                    .iter()
                                    .map(|&c| html! {
                                        <option value={c} selected={*selected_country == c}>{c}</option>
                                    })
                                }
                            </select>
                        </div>
                    }
                } else {
                    html! {}
                }
            }
            <h2 class="section-title">{"Plans"}</h2>
            <div class="pricing-grid">
                <PricingCard
                    plan_name={"Hosted Plan"}
                    best_for={"Full-featured cloud service ready to go."}
                    price={*hosted_total_price}
                    currency={if *selected_country == "US" || *selected_country == "CA" { "$" } else { "€" }}
                    period={"/day"}
                    features={hosted_features.clone()}
                    subscription_type={"hosted"}
                    is_popular={true}
                    is_premium={false}
                    is_trial={false}
                    user_id={props.user_id}
                    user_email={props.user_email.clone()}
                    is_logged_in={props.is_logged_in}
                    verified={props.verified}
                    sub_tier={props.sub_tier.clone()}
                    selected_country={(*selected_country).clone()}
                    coming_soon={false}
                    hosted_prices={hosted_prices.clone()}
                />
                /*
                <PricingCard
                    plan_name={"Guaranteed Plan"}
                    best_for={"Hosted Plan with zero loop holes. Full refund for the first month if not satisfied."}
                    price={*guaranteed_total_price}
                    currency={if *selected_country == "US" || *selected_country == "CA" { "$" } else { "€" }}
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
                    selected_country={(*selected_country).clone()}
                    coming_soon={false}
                    hosted_prices={hosted_prices.clone()}
                />
            */
            </div>
            <FeatureList selected_country={(*selected_country).clone()} />
            <CreditPricing selected_country={(*selected_country).clone()} />
            <div class="pricing-faq">
                <h2>{"Common Questions"}</h2>
                <div class="faq-grid">
                    {
                        if (*selected_country) == "US" || (*selected_country) == "CA" {
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
                        } else if (*selected_country) == "FI" || (*selected_country) == "NL" || (*selected_country) == "AU" || (*selected_country) == "UK" {
                            html! {
                                <>
                                <details>
                                    <summary>{"How does billing work?"}</summary>
                                    <p>{"Plans bill monthly. Hosted Plan includes phone number for FI/AU/UK, but messages are bought separately before hand. No hidden fees, but no refunds — I'm a bootstrapped solo dev."}</p>
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
                        <p>{"Available globally. US/CA everything is included. FI/UK/AU include a number but message are bought separately. Elsewhere bring your own number (guided setup, costs vary ~€0.05-0.50/text or free if you have extra android phone laying around with a another phone plan). Contact rasmus@ahtava.com for more details."}</p>
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
            </div>
        </div>
    }
}
