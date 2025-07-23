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
}

#[derive(Properties, PartialEq, Clone)]
pub struct CheckoutButtonProps {
    pub user_id: i32,
    pub user_email: String,
    pub subscription_type: String,
    pub selected_country: String,
    #[prop_or(0)]
    pub selected_topups: i32,
    #[prop_or(0)]
    pub selected_digests: i32,
}

#[function_component(CheckoutButton)]
pub fn checkout_button(props: &CheckoutButtonProps) -> Html {
    let user_id = props.user_id;
    let user_email = props.user_email.clone();
    let subscription_type = props.subscription_type.clone();
    let selected_country = props.selected_country.clone();
    let selected_topups = props.selected_topups;
    let selected_digests = props.selected_digests;

    let onclick = {
        let user_id = user_id.clone();
        let subscription_type = subscription_type.clone();
        let selected_country = selected_country.clone();
        let selected_topups = selected_topups.clone();
        let selected_digests = selected_digests.clone();
        
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let user_id = user_id.clone();
            let subscription_type = subscription_type.clone();
            let selected_topups = selected_topups.clone();
            
            if subscription_type != "basic" && subscription_type != "oracle" && selected_country == "Other" {
                if let Some(window) = web_sys::window() {
                    if !window.confirm_with_message(
                        "Have you contacted us to get a coupon code for your country? The base price shown is just a placeholder and the actual price will be set using a coupon code based on your country's pricing. Click OK if you have a coupon code, or Cancel to contact us first."
                    ).unwrap_or(false) {
                        let email_url = "mailto:rasmus@ahtava.com?subject=Monitoring%20Plan%20Pricing%20Inquiry&body=Hey,%0A%0AI'm%20interested%20in%20the%20Monitoring%20Plan.%20Could%20you%20please%20provide%20me%20with%20the%20correct%20pricing%20and%20coupon%20code%20for%20my%20country?%0A%0AThanks!";
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
                            "self_hosting" => "SelfHosting",
                            _ => "Hosted"  // Default to Hosted if unknown
                        },
                        "selected_topups": selected_topups,
                        "selected_digests": selected_digests
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

    html! {
        <button class="iq-button signup-button" {onclick}><b>{button_text}</b></button>
    }
}

#[derive(Properties, PartialEq)]
pub struct PricingCardProps {
    pub plan_name: String,
    pub best_for: String,
    pub price: f64,
    pub currency: String,
    pub features: Vec<Feature>,
    pub subscription_type: String,
    pub is_popular: bool,
    pub is_premium: bool,
    pub is_self_hosting: bool,
    pub user_id: i32,
    pub user_email: String,
    pub is_logged_in: bool,
    pub verified: bool,
    pub sub_tier: Option<String>,
    pub selected_country: String,
    pub show_topup_selector: bool,
    #[prop_or(0)]
    pub selected_topups: i32,
    pub on_topup_change: Option<Callback<i32>>,
    #[prop_or(0)]
    pub selected_digests: i32,
    #[prop_or(false)]
    pub coming_soon: bool,
}

#[function_component(PricingCard)]
pub fn pricing_card(props: &PricingCardProps) -> Html {
    let price_text = if props.price == 0.0 {
        "Free".to_string()
    } else if props.selected_country == "Other" {
        format!("from {}{:.2}", props.currency, props.price)
    } else {
        format!("{}{:.2}", props.currency, props.price)
    };

    let effective_tier = if props.subscription_type == "hosted" {
        "tier 2".to_string()
    } else {
        props.subscription_type.clone()
    };

    let button = if props.subscription_type == "diy" {
        html! { 
            <a href="https://github.com/ahtavarasmus/lightfriend" target="_blank" rel="noopener noreferrer" class="iq-button signup-button"><b>{"Get Started on GitHub"}</b></a> 
        }
    } else if props.coming_soon {
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
                    selected_topups={props.selected_topups}
                    selected_digests={props.selected_digests}
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

    let image_url = match props.subscription_type.as_str() {
        "diy" => "/assets/diy-tier-image.png",
        "self_hosting" => "/assets/easy-self-host-image.png",
        "hosted" => "/assets/hosted-tier-image.png",
        _ => "",
    };

    html! {
        <div class={classes!("pricing-card", "subscription",
            if props.is_popular { "popular" } else { "" },
            if props.is_premium { "premium" } else { "" },
            if props.is_self_hosting { "self-hosting" } else { "" })}>
            {
                if props.is_popular {
                    html! { <div class="popular-tag">{"Most Popular"}</div> }
                } else if props.is_premium {
                    html! { <div class="premium-tag">{"Simplest"}</div> }
                } else {
                    html! {}
                }
            }
            <div class="header-background" style={format!("background-image: url({});", image_url)}>
                <h3>{props.plan_name.clone()}</h3>
            </div>
            <div class="card-content">
                <p class="best-for">{props.best_for.clone()}</p>
                <div class="price">
                    <span class="amount">{price_text}</span>
                    { if props.price > 0.0 { html! { <span class="period">{"/month"}</span> } } else { html! {} } }
                </div>
                <div class="includes">
                    <ul class="quota-list">
                        { for props.features.iter().flat_map(|feature| {
                            let main_item = html! { <li>{feature.text.clone()}</li> };
                            let sub_items = feature.sub_items.iter().map(|sub| html! { <li class="sub-item">{sub}</li> }).collect::<Vec<_>>();
                            vec![main_item].into_iter().chain(sub_items.into_iter())
                        }) }
                    </ul>
                </div>
                {
                    if props.is_self_hosting {
                        html! {
                            <div class="learn-more-section">
                                <a href="/host-instructions" class="learn-more-link">{"Learn how self-hosting works"}</a>
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
pub struct TopupSelectorProps {
    pub id: String,
    pub selected_topups: i32,
    pub on_topup_change: Callback<i32>,
}

#[function_component(TopupSelector)]
pub fn topup_selector(props: &TopupSelectorProps) -> Html {
    let on_change = {
        let on_topup_change = props.on_topup_change.clone();
        Callback::from(move |e: Event| {
            if let Some(target) = e.target_dyn_into::<HtmlSelectElement>() {
                if let Ok(value) = target.value().parse::<i32>() {
                    on_topup_change.emit(value);
                }
            }
        })
    };

    html! {
        <div class="extras-section">
            <h4>{"Add Message Packs"}</h4>
            <p class="extras-description">{"Each pack adds 20 Messages for €5/month"}</p>
            <div class="extras-selector-inline">
                <div class="quantity-selector-inline">
                    <label for={props.id.clone()}>{"Message Packs:"}</label>
                    <select id={props.id.clone()} onchange={on_change}>
                        { for (0..=10).map(|i| html! {
                            <option value={i.to_string()} selected={props.selected_topups == i}>
                                {if i == 0 { "None".to_string() } else { format!("{} pack{}", i, if i == 1 { "" } else { "s" }) }}
                            </option>
                        }) }
                    </select>
                </div>
                {
                    if props.selected_topups > 0 {
                        html! {
                            <div class="extras-summary-inline">
                                <div class="summary-item">
                                    <span class="summary-label">{"Extra Messages:"}</span>
                                    <span class="summary-value">{format!("{}", props.selected_topups * 20)}</span>
                                </div>
                                <div class="summary-item">
                                    <span class="summary-label">{"Extra Cost:"}</span>
                                    <span class="summary-value">{format!("€{}/month", props.selected_topups * 5)}</span>
                                </div>
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
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
    let mut base_messages_text: String = "400 Messages per month (Hosted) or connect your own Twilio (Self-Hosting)".to_string();
    if props.selected_country != "US".to_string() {
        base_messages_text = "200 Messages per month (Hosted) or connect your own Twilio (Self-Hosting)".to_string();
    }

    html! {
        <div class="feature-list">
            <h2>{"Included in All Plans"}</h2>
            <ul>
                <li>{"Voice calling and SMS interface"}</li>
                <li>{base_messages_text}</li>
                <li>{"Buy Additional Messages (via Lightfriend or Twilio)"}</li>
                <li>{"Perplexity AI Web Search"}</li>
                <li>{"Weather Search and forecast of the next 6 hours"}</li>
                <li>{"Photo Analysis & Translation (US & AUS only)"}</li>
                <li>{"QR Code Scanning (US & AUS only)"}</li>
                <li>{"Send, Fetch and Monitor WhatsApp Messages"}</li>
                <li>{"Fetch and Monitor Emails"}</li>
                <li>{"Fetch, Create and Monitor Calendar events"}</li>
                <li>{"Fetch and Create Tasks and Ideas"}</li>
                <li>{"24/7 Critical Message Monitoring"}</li>
                <li>{"Morning, Day and Evening Digests"}</li>
                <li>{"Custom Waiting Checks"}</li>
                <li>{"Priority Sender Notifications"}</li>
                <li>{"All Future Features Included"}</li>
                <li>{"Priority Support (for paid plans)"}</li>
            </ul>
        </div>
    }
}

#[function_component(Pricing)]
pub fn pricing(props: &PricingProps) -> Html {
    fn get_country_from_phone(phone_number: &str) -> String {
        let digits: String = phone_number.chars().filter(|c| c.is_digit(10)).collect();
        if digits.starts_with("1") {
            "US".to_string()
        } else if digits.starts_with("358") {
            "FI".to_string()
        } else if digits.starts_with("44") {
            "UK".to_string()
        } else if digits.starts_with("61") {
            "AU".to_string()
        } else {
            "Other".to_string()
        }
    }
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
                let user_phone = props.phone_number.clone();
                let selected_country = selected_country_state.clone();
                let country_name = country_name_state.clone();

                move |_| {
                    if is_logged_in {
                        if let Some(phone) = &user_phone {
                            let country = get_country_from_phone(phone);
                            selected_country.set(country);
                            match selected_country.as_str() {
                                "US" => country_name.set("United States".to_string()),
                                "FI" => country_name.set("Finland".to_string()),
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
                                        if ["US", "FI", "UK", "AU"].contains(&code.as_str()) {
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
        ("US".to_string(), 49.00),
        ("FI".to_string(), 79.00),
        ("UK".to_string(), 79.00),
        ("AU".to_string(), 79.00),
        ("Other".to_string(), 79.00),
    ]);

    let self_hosting_prices: HashMap<String, f64> = HashMap::from([
        ("US".to_string(), 15.00),
        ("FI".to_string(), 15.00),
        ("UK".to_string(), 15.00),
        ("AU".to_string(), 15.00),
        ("Other".to_string(), 15.00),
    ]);

    let credit_rates: HashMap<String, f64> = HashMap::from([
        ("US".to_string(), 0.15),
        ("FI".to_string(), 0.30),
        ("UK".to_string(), 0.30),
        ("AU".to_string(), 0.30),
        ("Other".to_string(), 0.30),
    ]);

    let on_country_change = {
        let selected_country = selected_country.clone();
        Callback::from(move |e: Event| {
            if let Some(target) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                selected_country.set(target.value());
            }
        })
    };

    let hosted_selected_topups = use_state(|| 0);
    let hosted_selected_digests = use_state(|| 0);
    let hosted_base_price = hosted_prices.get(&*selected_country).unwrap_or(&0.0);
    let hosted_topup_price = if *selected_country == "US" {
        0.0
    } else {
        *hosted_selected_topups as f64 * 5.0
    };
    let hosted_total_price = hosted_base_price + hosted_topup_price;

    let self_hosting_selected_topups = use_state(|| 0);
    let self_hosting_selected_digests = use_state(|| 0);
    let self_hosting_base_price = self_hosting_prices.get(&*selected_country).unwrap_or(&0.0);
    let self_hosting_topup_price = if *selected_country == "US" {
        0.0
    } else {
        *self_hosting_selected_topups as f64 * 5.0
    };
    let self_hosting_total_price = self_hosting_base_price + self_hosting_topup_price;

    let diy_features = vec![
        Feature {
            text: "For tech-savvy users comfortable with server setup, security, and maintenance".to_string(),
            sub_items: vec![],
        },
        Feature {
            text: "Manual updates and configuration".to_string(),
            sub_items: vec![],
        },
    ];

    let self_hosting_features = vec![
        Feature {
            text: "User-friendly setup with no coding required".to_string(),
            sub_items: vec![],
        },
        Feature {
            text: "Easy to follow instructions and our support".to_string(),
            sub_items: vec![],
        },
        Feature {
            text: "Automatic updates and built-in security".to_string(),
            sub_items: vec![],
        },
        Feature {
            text: "Available worldwide, even where hosted service isn't".to_string(),
            sub_items: vec![],
        },
    ];

    let hosted_features = vec![
        Feature {
            text: "Fully managed ready to go service hosted in EU".to_string(),
            sub_items: vec![],
        },
        Feature {
            text: "Includes phone numbers, servers, and everything else".to_string(),
            sub_items: vec![],
        },
        Feature {
            text: "Automatic updates, security and priority support".to_string(),
            sub_items: vec![],
        },
        Feature {
            text: "Simply connect your apps to get started".to_string(),
            sub_items: vec![],
        },
    ];

    let currency_symbol = if *selected_country == "US" { "$" } else { "€" };

    html! {
        <div class="pricing-panel">
            <div class="pricing-header">
                <h1>{"Invest in Your Peace of Mind"}</h1>
                <p>{"Reduce anxiety, sleep better, and live with clarity without the constant pull of your smartphone."}</p>
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
                                {". Contact to ask for availability and you get a coupon code to use for the subscription to set the correct price."}
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
                                { for ["US", "FI", "UK", "AU", "Other"]
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

            <div class="pricing-grid">
                <PricingCard
                    plan_name="DIY Hosting"
                    best_for="For tech-savvy users who enjoy hands-on server management and tinkering"
                    price={0.0}
                    currency={"$"}
                    features={diy_features}
                    subscription_type="diy"
                    is_popular=false
                    is_premium=false
                    is_self_hosting=false
                    user_id={props.user_id}
                    user_email={props.user_email.clone()}
                    is_logged_in={props.is_logged_in}
                    verified={props.verified}
                    sub_tier={props.sub_tier.clone()}
                    selected_country={(*selected_country).clone()}
                    show_topup_selector=false
                    selected_topups=0
                    on_topup_change={None::<Callback<i32>>}
                    selected_digests=0
                    coming_soon={false}
                />
                <PricingCard
                    plan_name="Easy Self-Hosting Plan"
                    best_for="Self-Hosted setup for non-technical users with automatic management."
                    price={self_hosting_total_price}
                    currency={if *selected_country == "US" { "$" } else { "€" }}
                    features={self_hosting_features}
                    subscription_type="self_hosting"
                    is_popular=true
                    is_premium=false
                    is_self_hosting=true
                    user_id={props.user_id}
                    user_email={props.user_email.clone()}
                    is_logged_in={props.is_logged_in}
                    verified={props.verified}
                    sub_tier={props.sub_tier.clone()}
                    selected_country={(*selected_country).clone()}
                    show_topup_selector={false}
                    selected_topups=0
                    on_topup_change={None::<Callback<i32>>}
                    selected_digests=0
                    coming_soon={true}
                />
                <PricingCard
                    plan_name="Hosted Plan"
                    best_for="Full-featured cloud service ready to go."
                    price={hosted_total_price}
                    currency={if *selected_country == "US" { "$" } else { "€" }}
                    features={hosted_features}
                    subscription_type="hosted"
                    is_popular=false
                    is_premium=true
                    is_self_hosting=false
                    user_id={props.user_id}
                    user_email={props.user_email.clone()}
                    is_logged_in={props.is_logged_in}
                    verified={props.verified}
                    sub_tier={props.sub_tier.clone()}
                    selected_country={(*selected_country).clone()}
                    show_topup_selector={*selected_country != "US"}
                    selected_topups={*hosted_selected_topups}
                    on_topup_change={if *selected_country != "US" { Some(Callback::from({
                        let hosted_selected_topups = hosted_selected_topups.clone();
                        move |value: i32| hosted_selected_topups.set(value)
                    })) } else { None }}
                    selected_digests={*hosted_selected_digests}
                    coming_soon={false}
                />
            </div>

            <div class="topup-pricing">
                <h2>{format!("Overage Rates for {}", *selected_country)}</h2>
                <p>{"When you exceed your quota on the Hosted Plan, these rates apply. Enable auto-top-up to automatically add credits when you run low. Unused credits carry over indefinitely. These are can used to answer user initiated questions, send notifications from priority senders and daily digests. On the Easy Self-Hosting Plan credits are bought directly from twilio."}</p>
                <div class="topup-packages">
                    <div class="pricing-card main">
                        <div class="card-header">
                            <div class="package-row">
                                <h3>{"Additional Messages:"}</h3>
                                <div class="price">
                                    {
                                        if *selected_country == "US" {
                                            html! {
                                                <span class="amount">{format!("${:.2}", credit_rates.get(&*selected_country).unwrap_or(&0.0))}</span>
                                            }
                                        } else {
                                            html! {
                                                <span class="amount">{format!("€{:.2}", credit_rates.get(&*selected_country).unwrap_or(&0.0))}</span>
                                            }
                                        }
                                    }
                                </div>
                            </div>
                            <div class="package-row">
                                <h3>{"Additional Priority Sender Notifications:"}</h3>
                                <div class="price">
                                    {
                                        if *selected_country == "US" {
                                            html! {
                                                <>
                                                    <span class="amount">{format!("${:.2}", credit_rates.get(&*selected_country).unwrap_or(&0.0)/3.0)}</span>
                                                    <span class="period">{" per notification"}</span>
                                                </>
                                            }
                                        } else {
                                            html! {
                                                <>
                                                    <span class="amount">{format!("€{:.2}", credit_rates.get(&*selected_country).unwrap_or(&0.0)/2.0)}</span>
                                                    <span class="period">{" per notification"}</span>
                                                </>
                                            }
                                        }
                                    }
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
                {
                    if props.is_logged_in {
                        html! {
                            <div class="topup-toggle">
                                <p>{"Choose your auto-top-up package size in your account billing."}</p>
                                <button class="iq-button signup-button" onclick={Callback::from(move |e: MouseEvent| {
                                    e.prevent_default();
                                    if let Some(window) = web_sys::window() {
                                        let _ = window.location().set_href("/billing");
                                    }
                                })}><b>{"Go to Billing"}</b></button>
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
            </div>

            <FeatureList selected_country={(*selected_country).clone()} />

            <div class="phone-number-options">
                <div class="phone-number-section">
                    <h2>{"Phone Number Options"}</h2>
                    <div class="options-grid">
                        <div class="option-card">
                            <h3>{"Request New Number"}</h3>
                            <p>{"Need a phone number? We can provide numbers in select countries like US, Finland, UK, and Australia. Due to regulatory restrictions, we cannot provide numbers in many countries including Germany, India, most African countries, and parts of Asia. If your country isn't listed in the pricing above, contact us to check availability or consider the Easy Self-Hosting Plan where you can connect your own Twilio account."}</p>
                            <a href={format!("mailto:rasmus@ahtava.com?subject=Country%20Availability%20Inquiry%20for%20{}&body=Hey,%0A%0AIs%20the%20service%20available%20in%20{}%3F%0A%0AThanks,%0A",(*country_name).clone(), (*country_name).clone())}>
                                <button class="iq-button signup-button">{"Check Number Availability"}</button>
                            </a>
                        </div>
                    </div>
                </div>
            </div>

            <div class="pricing-faq">
                <h2>{"Common Questions"}</h2>
                <div class="faq-grid">
                    <details>
                        <summary>{"How does billing work?"}</summary>
                        <p>{"All plans are billed monthly and include a certain number of Messages per month. Additional Messages can be purchased using credits. Unused credits carry over indefinitely. You retain subscription benefits until the next normal billing period end even if you unsubscribed immediately. No hidden fees or commitments. Note that due to high cost of running the service, no free tiers or refunds can be offered (Lightfriend is a bootstrapped startup)."}</p>
                    </details>
                    <details>
                        <summary>{"What counts as a Message?"}</summary>
                        <p>{"Messages can be used for voice calls (1 minute = 1 Message) or text queries (1 query = 1 Message). They can also be used for receiving daily digests (1 digest = 1 Message) or notifications from priority senders (1 notification = 1/2 Message). Critical message monitoring and custom waiting checks don't count against your quota."}</p>
                    </details>
                    <details>
                        <summary>{"How do credits work?"}</summary>
                        <p>{"For Hosted Plan, credits can be used for additional messages beyond your monthly limit. Enable auto-top-up to automatically purchase credits when you run low. Unused credits never expire."}</p>
                    </details>
                    <details>
                        <summary>{"How does automatic monitoring work?"}</summary>
                        <p>{"The AI continuously monitors your email, messages, and calendar, providing three daily digest summaries (morning, day, evening). Critical messages are flagged immediately and sent to you if enabled. You can set up to 5 custom waiting checks to monitor for specific types of messages or emails, and designate priority senders whose messages will always be notified about."}</p>
                    </details>
                    <details>
                        <summary>{"What's the difference between the plans in terms of setup and ease of use?"}</summary>
                        <p>{"The Hosted Plan is the easiest - no setup required, just connect your apps and start using. The Easy Self-Hosting Plan requires minimal effort: we provide a pre-configured VPS with one-click installation via Cloudron, automatic updates, and guided setup - no coding needed. The DIY Hosting is for advanced users: it's free but requires manual server setup, configuration, security management, and updates via GitHub."}</p>
                    </details>
                    <details>
                        <summary>{"Who should choose the Hosted Plan?"}</summary>
                        <p>{"If you want the simplest experience with zero technical setup, the Hosted Plan is ideal. Everything is managed for you, including phone numbers and servers - just subscribe and connect your accounts to get started immediately."}</p>
                    </details>
                    <details>
                        <summary>{"Who is the Easy Self-Hosting Plan for?"}</summary>
                        <p>{"This plan is perfect for non-technical users who want more control without the hassle. It offers user-friendly setup on your own server with automatic management, updates, and security - available worldwide and easy to maintain."}</p>
                    </details>
                    <details>
                        <summary>{"Is DIY Hosting suitable for beginners?"}</summary>
                        <p>{"DIY Hosting is best for tech-savvy users comfortable with server management, coding, and manual updates. If you're new to this, we recommend starting with the Hosted or Easy Self-Hosting Plans for a smoother experience."}</p>
                    </details>
                    <details>
                        <summary>{"Why charge monthly for the Easy Self-Hosting Plan?"}</summary>
                        <p>{"The fee covers the simplified setup (no coding needed), managed Cloudron environment, automatic updates, subdomain, and ongoing support. You control your own server, but we handle the heavy lifting to make it effortless."}</p>
                    </details>
                    <details>
                        <summary>{"Can I self-host for free?"}</summary>
                        <p>{"Yes, the core code is open-source on GitHub for developers comfortable with manual setup. For non-technical users, we recommend the Easy Self-Hosting Plan, which includes a pre-configured VPS, one-click Cloudron install, automatic updates, and priority support."}</p>
                    </details>
                    <details>
                        <summary>{"Is the service available in my country?"}</summary>
                        <p>{"Service availability and features vary by country. The Hosted Plan may be limited or unavailable in countries where we can't provide local phone numbers (including many parts of Asia, Africa, and some European countries). The Easy Self-Hosting Plan is available worldwide as it runs on your own server and you can connect your own Twilio account. Contact us to check availability for your specific location."}</p>
                    </details>
                </div>
            </div>

            <div class="footnotes">
                <p class="footnote">{"* Gen Z spends 4-7 hours daily on phones, often regretting 60% of social media time. "}<a href="https://explodingtopics.com/blog/smartphone-usage-stats" target="_blank" rel="noopener noreferrer">{"Read the study"}</a><grok-card data-id="badfd9" data-type="citation_card"></grok-card></p>
                <p class="footnote">{"The dumbphone is sold separately and is not included in any plan."}</p>
                <p class="footnote">{"For developers: Check out the open-source repo on GitHub if you'd like to self-host from source (requires technical setup)."}</p>
                <a href="https://github.com/ahtavarasmus/lightfriend" target="_blank" rel="noopener noreferrer" class="github-link">{"View GitHub Repo"}</a>
            </div>

            <div class="legal-links">
                <Link<Route> to={Route::Terms}>{"Terms & Conditions"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Privacy}>{"Privacy Policy"}</Link<Route>>
            </div>

            <style>
                {r#"
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
                .pricing-grid {
                    display: grid;
                    grid-template-columns: 1fr;
                    gap: 4rem;
                    max-width: 1200px;
                    margin: 4rem auto;
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

                .pricing-card {
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
                    overflow: hidden;
                }

                .pricing-card:hover {
                    transform: translateY(-5px);
                    box-shadow: 0 8px 32px rgba(30, 144, 255, 0.15);
                    border-color: rgba(30, 144, 255, 0.3);
                }

                .pricing-card.premium {
                    background: rgba(40, 40, 40, 0.85);
                    border: 2px solid rgba(255, 215, 0, 0.3);
                }

                .pricing-card.premium:hover {
                    box-shadow: 0 8px 32px rgba(255, 215, 0, 0.2);
                    border-color: rgba(255, 215, 0, 0.5);
                }

                .pricing-card.self-hosting {
                    background: rgba(40, 40, 40, 0.85);
                    border: 2px solid rgba(0, 255, 255, 0.3);
                }

                .pricing-card.self-hosting:hover {
                    box-shadow: 0 8px 32px rgba(0, 255, 255, 0.2);
                    border-color: rgba(0, 255, 255, 0.5);
                }

                .popular-tag {
                    position: absolute;
                    top: -12px;
                    right: 24px;
                    background: linear-gradient(45deg, #1E90FF, #4169E1);
                    color: white;
                    padding: 0.5rem 1rem;
                    border-radius: 20px;
                    font-size: 0.9rem;
                    font-weight: 500;
                }

                .premium-tag {
                    position: absolute;
                    top: -12px;
                    right: 24px;
                    background: linear-gradient(45deg, #FFD700, #FFA500);
                    color: white;
                    padding: 0.5rem 1rem;
                    border-radius: 20px;
                    font-size: 0.9rem;
                    font-weight: 500;
                }

                .header-background {
                    position: relative;
                    height: 200px;
                    background-size: cover;
                    background-position: center;
                    display: flex;
                    align-items: center;
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
                }

                .price {
                    margin: 1.5rem 0;
                    text-align: center;
                }

                .price .amount {
                    font-size: 3rem;
                    color: #fff;
                    font-weight: 700;
                }

                .price .period {
                    color: #999;
                    font-size: 1.1rem;
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
                    position: absolute;
                    left: 1rem;
                    color: #7EB2FF;
                }

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
                    position: relative;
                    padding-left: 2rem;
                }

                .feature-list li::before {
                    content: "✅";
                    position: absolute;
                    left: 0;
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

                .topup-packages {
                    max-width: 600px;
                    margin: 2rem auto;
                }

                .pricing-card.main {
                    background: rgba(30, 30, 30, 0.8);
                    border: 1px solid rgba(30, 144, 255, 0.15);
                    padding: 2rem;
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

                @media (max-width: 968px) {
                    .options-grid {
                        grid-template-columns: 1fr;
                    }
                    
                    .topup-packages {
                        padding: 0 1rem;
                    }
                    
                    .package-row {
                        flex-direction: column;
                        text-align: center;
                        gap: 0.5rem;
                    }
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

                    .feature-list {
                        padding: 1.5rem;
                        margin: 2rem 1rem;
                        max-width: calc(100vw - 2rem);
                    }
                }

                @media (max-width: 768px) {
                    .topup-controls {
                        grid-template-columns: 1fr;
                        gap: 1rem;
                    }
                    
                    .quantity-selector-inline {
                        flex-direction: column;
                        align-items: center;
                        gap: 0.5rem;
                    }

                    .quantity-selector-inline label {
                        min-width: auto;
                        text-align: center;
                    }

                    .extras-summary-inline {
                        flex-direction: column;
                        gap: 1rem;
                    }
                }
                "#}
            </style>
        </div>
    }
}
