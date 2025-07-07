use yew::prelude::*;
use yew_router::prelude::*;
use crate::Route;
use yew_router::components::Link;
use serde_json::json;
use web_sys::window;
use wasm_bindgen_futures;
use serde_json::Value;
use crate::config;
use gloo_net::http::Request;
use serde::Deserialize;
use std::collections::HashMap;
use wasm_bindgen::JsCast;
use web_sys::HtmlSelectElement;

#[derive(Deserialize, Clone)]
struct UserProfile {
    id: i32,
    email: String,
    sub_tier: Option<String>,
    phone_number: Option<String>,
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
    #[prop_or(1)]
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
            
            // For Sentinel Plan and "Other" country, show confirmation dialog
            if subscription_type != "basic" && selected_country == "Other" {
                if let Some(window) = web_sys::window() {
                    if !window.confirm_with_message(
                        "Have you contacted us to get a coupon code for your country? The base price shown is just a placeholder and the actual price will be set using a coupon code based on your country's pricing. Click OK if you have a coupon code, or Cancel to contact us first."
                    ).unwrap_or(false) {
                        // If user clicks Cancel, redirect to email
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
                    let endpoint = if subscription_type == "basic" {
                        format!("{}/api/stripe/basic-subscription-checkout/{}", config::get_backend_url(), user_id)
                    } else if subscription_type == "oracle" {
                        format!("{}/api/stripe/oracle-subscription-checkout/{}", config::get_backend_url(), user_id)
                    } else {
                        format!("{}/api/stripe/subscription-checkout/{}", config::get_backend_url(), user_id)
                    };

                    // Create request body with top-ups and digests
                    let request_body = if selected_topups > 0 {
                        json!({
                            "selected_topups": selected_topups
                        })
                    } else {
                        json!({})
                    };

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




#[function_component(Pricing)]
pub fn pricing(props: &PricingProps) -> Html {
    // Add this function at the top of your file, after the imports
    fn get_country_from_phone(phone_number: &str) -> String {
        // Remove any non-digit characters for analysis
        let digits: String = phone_number.chars().filter(|c| c.is_digit(10)).collect();
        
        // Check country codes
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

    // Scroll to top only on initial mount
    {
        use_effect_with_deps(
            move |_| {
                if let Some(window) = web_sys::window() {
                    window.scroll_to_with_x_and_y(0.0, 0.0);
                }
                || ()
            },
            (), // Empty dependencies array means this effect runs only once on mount
        );
    }

    
    // Set country based on user's phone number if logged in, otherwise detect from IP
    {
        // Keep the originals alive for the rest of the component
        let selected_country_state = selected_country.clone();
        let country_name_state     = country_name.clone();

        let is_logged_in = props.is_logged_in;

        use_effect_with_deps(
            {
                // Capture **clones** inside the closure so nothing else is moved away.
                let user_phone       = props.phone_number.clone();
                let selected_country = selected_country_state.clone();
                let country_name     = country_name_state.clone();

                move |_| {
                    if is_logged_in {
                        // Logged-in user → use phone number
                        if let Some(phone) = &user_phone {
                            let country = get_country_from_phone(phone);
                            selected_country.set(country);

                            match selected_country.as_str() {
                                "US" => country_name.set("United States".to_string()),
                                "FI" => country_name.set("Finland".to_string()),
                                "UK" => country_name.set("United Kingdom".to_string()),
                                "AU" => country_name.set("Australia".to_string()),
                                _    => country_name.set("Other".to_string()),
                            }
                        }
                    } else {
                        // Not logged in → fall back to IP lookup (async)
                        let selected_country = selected_country.clone();
                        let country_name     = country_name.clone();

                        wasm_bindgen_futures::spawn_local(async move {
                            if let Ok(response) = Request::get("https://ipapi.co/json/").send().await {
                                if let Ok(json) = response.json::<Value>().await {
                                    if let Some(code) = json.get("country_code").and_then(|c| c.as_str()) {
                                        let code = code.to_uppercase();

                                        if let Some(name) =
                                            json.get("country_name").and_then(|c| c.as_str())
                                        {
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
            (
                is_logged_in,
                // Dependency tuple gets its own fresh clone
                props.phone_number.clone(),
            ),
        );
    }

    

    

    let sentinel_prices: HashMap<String, f64> = HashMap::from([
        ("US".to_string(), 29.00),
        ("FI".to_string(), 29.00),
        ("UK".to_string(), 29.00),
        ("AU".to_string(), 29.00),
        ("Other".to_string(), 29.00),
    ]);

    let oracle_prices: HashMap<String, f64> = HashMap::from([
        ("US".to_string(), 19.00),
        ("FI".to_string(), 19.00),
        ("UK".to_string(), 19.00),
        ("AU".to_string(), 19.00),
        ("Other".to_string(), 19.00),
    ]);

    let basic_prices: HashMap<String, f64> = HashMap::from([
        ("US".to_string(), 8.00),
        ("FI".to_string(), 15.00),
        ("UK".to_string(), 15.00),
        ("AU".to_string(), 15.00),
        ("Other".to_string(), 15.00),
    ]);

    let credit_rates: HashMap<String, f64> = HashMap::from([
        ("US".to_string(), 0.15), // cost per additional message/question
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

    // State for Oracle plan top-ups
    let oracle_selected_topups = use_state(|| 0);

    let base_price = oracle_prices.get(&*selected_country).unwrap_or(&0.0);

    let topup_price = if *selected_country == "US" { 
        0.0 
    } else { 
        *oracle_selected_topups as f64 * 5.0 
    };
    let total_price = base_price + topup_price;

    let selected_topups_for_checkout = *oracle_selected_topups;

    // State for Sentinel plan extras
    let sentinel_selected_topups = use_state(|| 0);
    let sentinel_selected_digests = use_state(|| 0); 

    let sentinel_base_price = sentinel_prices.get(&*selected_country).unwrap_or(&0.0);
    let sentinel_topup_price = if *selected_country == "US" { 
        0.0 
    } else { 
        *sentinel_selected_topups as f64 * 5.0 
    };
    let sentinel_total_price = sentinel_base_price + sentinel_topup_price;

    let sentinel_topups_for_checkout = *sentinel_selected_topups;
    let sentinel_digests_for_checkout = *sentinel_selected_digests;

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

            /* Country selector ‒ hide if user is logged in */
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
                    html! {}   // nothing rendered when logged in
                }
            }

            
            <div class="pricing-grid">

                <div class="pricing-card subscription">
                    <div class="card-header">
                        <h3>{"Basic Plan"}</h3>
                        <p class="best-for">{"Essential AI tools for weather queries and internet searches."}</p>
                        
                        <div class="price">
                            {
                                if *selected_country == "Other" {
                                    html! {
                                        <>
                                            <span class="amount">{format!("from €{:.2}", basic_prices.get(&*selected_country).unwrap_or(&0.0))}</span>
                                            <span class="period">{"/month"}</span>
                                        </>
                                    }
                                } else if *selected_country == "US" {
                                    html! {
                                        <>
                                            <span class="amount">{format!("${:.2}", basic_prices.get(&*selected_country).unwrap_or(&0.0))}</span>
                                            <span class="period">{"/month"}</span>
                                        </>
                                    }
                                } else {
                                    html! {
                                        <>
                                            <span class="amount">{format!("€{:.2}", basic_prices.get(&*selected_country).unwrap_or(&0.0))}</span>
                                            <span class="period">{"/month"}</span>
                                        </>
                                    }
                                }
                            }
                        </div>
                        <div class="includes">
                            <p>{"Subscription includes:"}</p>
                            <ul class="quota-list">
                                <li>{"🔍 Internet Search (Perplexity)"}</li>
                                <li>{"☀️ Weather Search"}</li>
                                <li>{"📱 40 Messages per month for:"}</li>
                                <li class="sub-item">{"   • Voice calls (1 min = 1 Message)"}</li>
                                <li class="sub-item">{"   • Text queries to Lightfriend (1 message = 1 Message)"}</li>
                                <li>{"💳 Additional credits for more messages"}</li>
                            </ul>
                        </div>
                    </div>
                    {
                        if props.is_logged_in {
                            if !props.verified {
                                let onclick = {
                                    Callback::from(move |e: MouseEvent| {
                                        e.prevent_default();
                                        if let Some(window) = web_sys::window() {
                                            let _ = window.location().set_href("/verify");
                                        }
                                    })
                                };
                                html! {
                                    <button class="iq-button verify-required" {onclick}>
                                        <b>{"Verify Account to Subscribe"}</b>
                                    </button>
                                }
                            } else if props.sub_tier.as_ref().is_none() {
                                html! {
                                    <CheckoutButton 
                                        user_id={props.user_id} 
                                        user_email={props.user_email.clone()} 
                                        subscription_type="basic"
                                        selected_country={(*selected_country).clone()}
                                    />
                                }
                            } else if props.sub_tier.as_ref().unwrap() == &"tier 1".to_string() {
                                html! {
                                    <button class="iq-button current-plan" disabled=true><b>{"Current Plan"}</b></button>
                                }
                            } else {
                                html! {
                                    <CheckoutButton 
                                        user_id={props.user_id} 
                                        user_email={props.user_email.clone()} 
                                        subscription_type="basic"
                                        selected_country={(*selected_country).clone()}
                                    />
                                }
                            }
                        } else {
                            let onclick = {
                                Callback::from(move |e: MouseEvent| {
                                    e.prevent_default();
                                    if let Some(window) = web_sys::window() {
                                        if let Ok(Some(storage)) = window.local_storage() {
                                            let _ = storage.set_item("selected_plan", "basic");
                                            let _ = window.location().set_href("/register");
                                        }
                                    }
                                })
                            };
                            html! {
                                <button onclick={onclick} class="iq-button signup-button"><b>{"Get Started"}</b></button>
                            }
                        }
                    }
                </div>

                <div class="pricing-card subscription popular">
                    <div class="popular-tag">{"Most Popular"}</div>
                    <div class="card-header">
                        <h3>{"Oracle Plan"}</h3>
                        <p class="best-for">{"Answers plus integrations — no monitoring."}</p>
                        <div class="price">
                            {
                                
                                if *selected_country == "Other" {
                                    html! {
                                        <>
                                            <span class="amount">{format!("from €{:.2}", total_price)}</span>
                                            <span class="period">{"/month"}</span>
                                        </>
                                    }
                                } else if *selected_country == "US" {
                                    html! {
                                        <>
                                            <span class="amount">{format!("${:.2}", total_price)}</span>
                                            <span class="period">{"/month"}</span>
                                        </>
                                    }
                                } else {
                                    html! {
                                        <>
                                            <span class="amount">{format!("€{:.2}", total_price)}</span>
                                            <span class="period">{"/month"}</span>
                                        </>
                                    }
                                }
                            }
                        </div>
                        <div class="includes">
                            <p>{"Subscription includes:"}</p>
                            <ul class="quota-list">
                                <li>{"💬 WhatsApp Integration"}</li>
                                <li>{"📧 Email Integration"}</li>
                                <li>{"📅 Calendar Integration"}</li>
                                <li>{"✅ Task Management"}</li>
                                {
                                    if *selected_country == "US" {
                                        html! { <li>{"📱 120 Messages per month for:"}</li> }
                                    } else {
                                        html! { <li>{"📱 40 Messages per month for:"}</li> }
                                    }
                                }
                                <li class="sub-item">{"   • Voice calls (1 min = 1 message)"}</li>
                                <li class="sub-item">{"   • Text queries to Lightfriend"}</li>
                                <li>{"💳 Additional credits for more messages"}</li>
                                <li>{"✨ Everything in Basic Plan included"}</li>
                            </ul>
                        </div>
                        
                        {
                            // Show top-up selector for non-US countries
                            if *selected_country != "US" {
                                let selected_topups_clone = oracle_selected_topups.clone();
                                let on_topup_change = {
                                    let selected_topups = oracle_selected_topups.clone();
                                    Callback::from(move |e: Event| {
                                        if let Some(target) = e.target_dyn_into::<HtmlSelectElement>() {
                                            if let Ok(value) = target.value().parse::<i32>() {
                                                selected_topups.set(value);
                                            }
                                        }
                                    })
                                };
                                
                                html! {
                                    <div class="oracle-topup-integrated">
                                        <div class="topup-section">
                                            <h4>{"Add Message Packs"}</h4>
                                            <p class="topup-description">{"Each pack adds 20 messages for €5/month"}</p>
                                            
                                            <div class="topup-selector-inline">
                                                <div class="quantity-selector-inline">
                                                    <label for="oracle-topups">{"Message Packs:"}</label>
                                                    <select id="oracle-topups" onchange={on_topup_change}>
                                                        { for (0..=10).map(|i| html! {
                                                            <option value={i.to_string()} selected={*selected_topups_clone == i}>
                                                                {if i == 0 { "None".to_string() } else { format!("{} pack{}", i, if i == 1 { "" } else { "s" }) }}
                                                            </option>
                                                        })}
                                                    </select>
                                                </div>
                                                
                                                {
                                                    if *selected_topups_clone > 0 {
                                                        html! {
                                                            <div class="topup-summary-inline">
                                                                <div class="summary-item">
                                                                    <span class="summary-label">{"Extra Messages:"}</span>
                                                                    <span class="summary-value">{format!("{}", *selected_topups_clone * 20)}</span>
                                                                </div>
                                                                <div class="summary-item">
                                                                    <span class="summary-label">{"Extra Cost:"}</span>
                                                                    <span class="summary-value">{format!("€{}/month", *selected_topups_clone * 5)}</span>
                                                                </div>
                                                            </div>
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                }
                                            </div>
                                        </div>
                                    </div>
                                }
                            } else {
                                html! {}
                            }
                        }
                    </div>
                    {
                        if props.is_logged_in {
                            if !props.verified {
                                let onclick = {
                                    Callback::from(move |e: MouseEvent| {
                                        e.prevent_default();
                                        if let Some(window) = web_sys::window() {
                                            let _ = window.location().set_href("/verify");
                                        }
                                    })
                                };
                                html! {
                                    <button class="iq-button verify-required" {onclick}>
                                        <b>{"Verify Account to Subscribe"}</b>
                                    </button>
                                }
                            } else if props.sub_tier.as_ref().is_none() {
                                html! {
                                    <CheckoutButton 
                                        user_id={props.user_id} 
                                        user_email={props.user_email.clone()} 
                                        subscription_type="oracle"
                                        selected_country={(*selected_country).clone()}
                                        selected_topups={selected_topups_for_checkout}
                                    />
                                }
                            } else if props.sub_tier.as_ref().unwrap() == &"tier 1.5".to_string() {
                                html! {
                                    <button class="iq-button current-plan" disabled=true><b>{"Current Plan"}</b></button>
                                }
                            } else {
                                html! {
                                    <CheckoutButton 
                                        user_id={props.user_id} 
                                        user_email={props.user_email.clone()} 
                                        subscription_type="oracle"
                                        selected_country={(*selected_country).clone()}
                                        selected_topups={selected_topups_for_checkout}
                                    />
                                }
                            }
                        } else {
                            let onclick = {
                                Callback::from(move |e: MouseEvent| {
                                    e.prevent_default();
                                    if let Some(window) = web_sys::window() {
                                        if let Ok(Some(storage)) = window.local_storage() {
                                            let _ = storage.set_item("selected_plan", "oracle");
                                            let _ = window.location().set_href("/register");
                                        }
                                    }
                                })
                            };
                            html! {
                                <button onclick={onclick} class="iq-button signup-button"><b>{"Get Started"}</b></button>
                            }
                        }
                    }
                </div>

                <div class="pricing-card subscription premium">
                    <div class="premium-tag">
                        {
                            if ["FI", "UK"].contains(&(*selected_country).as_str()) {
                                "EU-Hosted 24/7 Monitoring"
                            } else {
                                "All-Inclusive Monitoring"
                            }
                        }
                    </div>
                    <div class="card-header">
                        <h3>{"Sentinel Plan"}</h3>
                        <p class="best-for">{"24/7 AI monitoring and alerts for peace of mind."}</p>
                        <div class="price">
                            {
                                if *selected_country == "Other" {
                                    html! {
                                        <>
                                            <span class="amount">{format!("from €{:.2}", sentinel_total_price)}</span>
                                            <span class="period">{"/month"}</span>
                                        </>
                                    }
                                } else if *selected_country == "US" {
                                    html! {
                                        <>
                                            <span class="amount">{format!("${:.2}", sentinel_total_price)}</span>
                                            <span class="period">{"/month"}</span>
                                        </>
                                    }
                                } else {
                                    html! {
                                        <>
                                            <span class="amount">{format!("€{:.2}", sentinel_total_price)}</span>
                                            <span class="period">{"/month"}</span>
                                        </>
                                    }
                                }
                            }
                        </div>
                        <div class="includes">
                            <p>{"Subscription includes:"}</p>
                            <ul class="quota-list">
                                <li>{"🔔 24/7 monitoring for critical messages (included)"}</li>
                                <li>{"⚡ Set temporary monitoring for specific messages (like package delivery)"}</li>
                                <li>{"⭐ Priority sender notifications"}</li>
                                <li>{"📊 Daily digest summaries (up to 3 per day)"}</li>
                                {
                                    if *selected_country == "US" {
                                        html! { <li>{"📱 200 Messages per month for:"}</li> }
                                    } else {
                                        html! { <li>{"📱 40 Messages per month for:"}</li> }
                                    }
                                }
                                <li class="sub-item">{"   • Daily digests"}</li>
                                <li class="sub-item">{"   • Voice calls (1 min = 1 message)"}</li>
                                <li class="sub-item">{"   • Text queries to Lightfriend"}</li>
                                <li class="sub-item">{"   • Priority sender notifications"}</li>
                                <li>{"💳 Additional credits for more messages"}</li>
                                <li>{"✨ Everything in Oracle Plan included"}</li>
                            </ul>
                        </div>
                        
                        {
                            // Seamless extras selection within Sentinel plan card
                            html! {
                                <div class="sentinel-extras-integrated">
                                    {
                                        // Show top-up selector for non-US countries
                                        if *selected_country != "US" {
                                            let selected_topups_clone = sentinel_selected_topups.clone();
                                            let on_topup_change = {
                                                let selected_topups = sentinel_selected_topups.clone();
                                                Callback::from(move |e: Event| {
                                                    if let Some(target) = e.target_dyn_into::<HtmlSelectElement>() {
                                                        if let Ok(value) = target.value().parse::<i32>() {
                                                            selected_topups.set(value);
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
                                                            <label for="sentinel-topups">{"Message Packs:"}</label>
                                                            <select id="sentinel-topups" onchange={on_topup_change}>
                                                                { for (0..=10).map(|i| html! {
                                                                    <option value={i.to_string()} selected={*selected_topups_clone == i}>
                                                                        {if i == 0 { "None".to_string() } else { format!("{} pack{}", i, if i == 1 { "" } else { "s" }) }}
                                                                    </option>
                                                                })}
                                                            </select>
                                                        </div>
                                                        
                                                        {
                                                            if *selected_topups_clone > 0 {
                                                                html! {
                                                                    <div class="extras-summary-inline">
                                                                        <div class="summary-item">
                                                                            <span class="summary-label">{"Extra Messages:"}</span>
                                                                            <span class="summary-value">{format!("{}", *selected_topups_clone * 20)}</span>
                                                                        </div>
                                                                        <div class="summary-item">
                                                                            <span class="summary-label">{"Extra Cost:"}</span>
                                                                            <span class="summary-value">{format!("€{}/month", *selected_topups_clone * 5)}</span>
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
                                        } else {
                                            html! {}
                                        }
                                    }
                                </div>
                            }
                        }
                    </div>
                    {
                        if props.is_logged_in {
                            if !props.verified {
                                let onclick = {
                                    Callback::from(move |e: MouseEvent| {
                                        e.prevent_default();
                                        if let Some(window) = web_sys::window() {
                                            let _ = window.location().set_href("/verify");
                                        }
                                    })
                                };
                                html! {
                                    <button class="iq-button verify-required" {onclick}>
                                        <b>{"Verify Account to Subscribe"}</b>
                                    </button>
                                }
                            } else if props.sub_tier.as_ref().is_none() {
                                html! {
                                    <CheckoutButton 
                                        user_id={props.user_id} 
                                        user_email={props.user_email.clone()} 
                                        subscription_type="sentinel_plan"
                                        selected_country={(*selected_country).clone()}
                                        selected_topups={sentinel_topups_for_checkout}
                                        selected_digests={sentinel_digests_for_checkout}
                                    />
                                }
                            } else if props.sub_tier.as_ref().unwrap() == &"tier 2".to_string() {
                                html! {
                                    <button class="iq-button current-plan" disabled=true><b>{"Current Plan"}</b></button>
                                }
                            } else {
                                html! {
                                    <CheckoutButton 
                                        user_id={props.user_id} 
                                        user_email={props.user_email.clone()} 
                                        subscription_type="sentinel_plan"
                                        selected_country={(*selected_country).clone()}
                                        selected_topups={sentinel_topups_for_checkout}
                                        selected_digests={sentinel_digests_for_checkout}
                                    />
                                }
                            }
                        } else {
                            let onclick = {
                                Callback::from(move |e: MouseEvent| {
                                    e.prevent_default();
                                    if let Some(window) = web_sys::window() {
                                        if let Ok(Some(storage)) = window.local_storage() {
                                            let _ = storage.set_item("selected_plan", "sentinel_plan");
                                            let _ = window.location().set_href("/register");
                                        }
                                    }
                                })
                            };
                            html! {
                                <button onclick={onclick} class="iq-button signup-button"><b>{"Get Started"}</b></button>
                            }
                        }
                    }
                </div>
            </div>


            <div class="topup-pricing">
                <h2>{format!("Overage Rates for {}", *selected_country)}</h2>
                <p>{"When you exceed your quota, these rates apply. Enable auto-top-up to automatically add credits when you run low. Unused credits carry over indefinitely. These are can used to answer user initiated questions, send notifications from priority senders and daily digests."}</p>
                <div class="topup-packages">
                    <div class="pricing-card main">
                        <div class="card-header">
                            <div class="package-row">
                                <h3>{"Additional Messages:"}</h3>
                                <div class="price">
                                    {
                                        if *selected_country == "US" {
                                            html! {
                                                <>
                                                    <span class="amount">{format!("${:.2}", credit_rates.get(&*selected_country).unwrap_or(&0.0))}</span>
                                                </>
                                            }
                                        } else {
                                            html! {
                                                <>
                                                    <span class="amount">{format!("€{:.2}", credit_rates.get(&*selected_country).unwrap_or(&0.0))}</span>
                                                </>
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
                                                    <span class="amount">{format!("${:.2}", credit_rates.get(&*selected_country).unwrap_or(&0.0)/&3.0)}</span>
                                                    <span class="period">{" per notification"}</span>
                                                </>
                                            }
                                        } else {
                                            html! {
                                                <>
                                                    <span class="amount">{format!("€{:.2}", credit_rates.get(&*selected_country).unwrap_or(&0.0)/&2.0)}</span>
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

            <div class="feature-comparison">
                <h2>{"Included Features"}</h2>
                <table>
                    <thead>
                        <tr>
                            <th>{"Feature"}</th>
                            <th>{"Basic Plan"}</th>
                            <th>{"Oracle Plan"}</th>
                            <th>{"Sentinel Plan"}</th>
                        </tr>
                    </thead>
                    <tbody>
                        <tr>
                            <td>{"Voice calling and SMS interface"}</td>
                            <td>{"✅"}</td>
                            <td>{"✅"}</td>
                            <td>{"✅"}</td>
                        </tr>
                        <tr>
                            <td>{
                                if *selected_country == "US" {
                                    "Base Messages (40/120/200 per month respectively)"
                                } else {
                                    "Base Messages (40/40/40 per month respectively)"
                                }
                            }</td>
                            <td>{"✅"}</td>
                            <td>{"✅"}</td>
                            <td>{"✅"}</td>
                        </tr>
                        <tr>
                            <td>{"Buy Additional Messages"}</td>
                            <td>{"✅"}</td>
                            <td>{"✅"}</td>
                            <td>{"✅"}</td>
                        </tr>
                        <tr>
                            <td>{"Perplexity AI Web Search"}</td>
                            <td>{"✅"}</td>
                            <td>{"✅"}</td>
                            <td>{"✅"}</td>
                        </tr>
                        <tr>
                            <td>{"Weather Search and forecast of the next 6 hours"}</td>
                            <td>{"✅"}</td>
                            <td>{"✅"}</td>
                            <td>{"✅"}</td>
                        </tr>
                        <tr>
                            <td>{"Photo Analysis & Translation (US & AUS only)"}</td>
                            <td>{"✅"}</td>
                            <td>{"✅"}</td>
                            <td>{"✅"}</td>
                        </tr>
                        <tr>
                            <td>{"QR Code Scanning (US & AUS only)"}</td>
                            <td>{"✅"}</td>
                            <td>{"✅"}</td>
                            <td>{"✅"}</td>
                        </tr>
                        <tr>
                            <td>{"WhatsApp Integration"}</td>
                            <td>{"❌"}</td>
                            <td>{"✅"}</td>
                            <td>{"✅"}</td>
                        </tr>
                        <tr>
                            <td>{"Email Integration"}</td>
                            <td>{"❌"}</td>
                            <td>{"✅"}</td>
                            <td>{"✅"}</td>
                        </tr>
                        <tr>
                            <td>{"Calendar Integration"}</td>
                            <td>{"❌"}</td>
                            <td>{"✅"}</td>
                            <td>{"✅"}</td>
                        </tr>
                        <tr>
                            <td>{"Task Management"}</td>
                            <td>{"❌"}</td>
                            <td>{"✅"}</td>
                            <td>{"✅"}</td>
                        </tr>
                        <tr>
                            <td>{"24/7 Critical Message Monitoring"}</td>
                            <td>{"❌"}</td>
                            <td>{"❌"}</td>
                            <td>{"✅"}</td>
                        </tr>
                        <tr>
                            <td>{"Morning, Day and Evening Digests"}</td>
                            <td>{"❌"}</td>
                            <td>{"❌"}</td>
                            <td>{"✅"}</td>
                        </tr>
                        <tr>
                            <td>{"Custom Waiting Checks"}</td>
                            <td>{"❌"}</td>
                            <td>{"❌"}</td>
                            <td>{"✅"}</td>
                        </tr>
                        <tr>
                            <td>{"Priority Sender Notifications"}</td>
                            <td>{"❌"}</td>
                            <td>{"❌"}</td>
                            <td>{"✅"}</td>
                        </tr>
                        <tr>
                            <td>{"Priority Support"}</td>
                            <td>{"✅"}</td>
                            <td>{"✅"}</td>
                            <td>{"✅"}</td>
                        </tr>
                    </tbody>
                </table>
            </div>

            <div class="phone-number-options">
                <div class="phone-number-section">
                    <h2>{"Phone Number Options"}</h2>
                    <div class="options-grid">
                        <div class="option-card">
                            <h3>{"Request New Number"}</h3>
                            <p>{"Need a phone number? We can provide numbers in select countries like US, Finland, UK, and Australia. Due to regulatory restrictions, we cannot provide numbers in many countries including Germany, India, most African countries, and parts of Asia. If your country isn't listed in the pricing above, contact us to check availability."}</p>
                            <a href={format!("mailto:rasmus@ahtava.com?subject=Country%20Availability%20Inquiry%20for%20{}&body=Hey,%0A%0AIs%20the%20service%20available%20in%20{}%3F%0A%0AThanks,%0A",(*country_name).clone(), (*country_name).clone())}>
                                <button class="iq-button signup-button">{"Check Number Availability"}</button>
                            </a>
                        </div>
                        <div class="option-card">
                            <h3>{"Bring Your Own Number"}</h3>
                            <p>{"Use your own Twilio number to get 50% off any plan and enable service in ANY country. Perfect for regions where we can't directly provide numbers (like Germany, India, African countries). This option lets you use our service worldwide while managing your own number through Twilio."}</p>
                            <a href={format!("mailto:rasmus@ahtava.com?subject=Bring%20my%20own%20Twilio%20number%20from%20{}&body=Hey,%0A%0AI'm%20interested%20in%20bringing%20my%20own%20Twilio%20number%20from%20{}.%0A%0AThanks,%0A", (*selected_country).clone(), (*selected_country).clone())}>
                                <button class="iq-button signup-button">{"Contact Us to Set Up"}</button>
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
                        <p>{"Messages can be used for voice calls (1 minute = 1 Message) or text queries (1 query = 1 Message). On the Sentinel Plan they can also be used for receiving daily digests (1 digest = 1 Message) or notifications from priority senders (1 notification = 1/2 Message). Critical message monitoring and custom waiting checks (Sentinel Plan only) don't count against your quota."}</p>
                    </details>
                    <details>
                        <summary>{"How do credits work?"}</summary>
                        <p>{"Credits can be used for additional messages beyond your monthly limit. Enable auto-top-up to automatically purchase credits when you run low. Unused credits never expire."}</p>
                    </details>
                    <details>
                        <summary>{"How does automatic monitoring work?"}</summary>
                        <p>{"The AI continuously monitors your email, messages, and calendar, providing three daily digest summaries (morning, day, evening). Critical messages are flagged immediately and sent to you if enabled. You can set up to 5 custom waiting checks to monitor for specific types of messages or emails, and designate priority senders whose messages will always be notified about."}</p>
                    </details>
                    <details>
                        <summary>{"Is the service available in my country?"}</summary>
                        <p>{"Service availability and features vary by country. The Basic Plan may be limited or unavailable in countries where we can't provide local phone numbers (including many parts of Asia, Africa, and some European countries). For full service availability, you can either: 1) Request a new number availability for your country, or 2) Bring your own Twilio number to enable service worldwide and get 50% off any plan. Contact us to check availability for your specific location."}</p>
                    </details>
                </div>
            </div>

            <div class="footnotes">
                <p class="footnote">{"* Gen Z spends 4-7 hours daily on phones, often regretting 60% of social media time. "}<a href="https://explodingtopics.com/blog/smartphone-usage-stats" target="_blank" rel="noopener noreferrer">{"Read the study"}</a></p>
                <p class="footnote">{"The dumbphone is sold separately and is not included in any plan."}</p>
                <p class="footnote">{"All data processed & stored in EU-based servers compliant with GDPR."}</p>
            </div>

            <div class="legal-links">
                <Link<Route> to={Route::Terms}>{"Terms & Conditions"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Privacy}>{"Privacy Policy"}</Link<Route>>
            </div>

            <style>
                {r#"
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

                .pricing-grid {
                    display: grid;
                    grid-template-columns: repeat(3, 1fr);
                    gap: 2rem;
                    max-width: 1400px;
                    margin: 4rem auto;
                }

                .pricing-card {
                    background: rgba(30, 30, 30, 0.8);
                    border: 1px solid rgba(30, 144, 255, 0.15);
                    border-radius: 24px;
                    padding: 2.5rem;
                    position: relative;
                    transition: transform 0.3s ease, box-shadow 0.3s ease;
                    backdrop-filter: blur(10px);
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
                    left: 24px;
                    background: linear-gradient(45deg, #FFD700, #FFA500);
                    color: white;
                    padding: 0.5rem 1rem;
                    border-radius: 20px;
                    font-size: 0.9rem;
                    font-weight: 500;
                }

                .card-header h3 {
                    color: #7EB2FF;
                    font-size: 2rem;
                    margin-bottom: 1rem;
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

                .includes p {
                    color: #7EB2FF;
                    font-size: 1.1rem;
                    margin-bottom: 1rem;
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

                .feature-comparison {
                    max-width: 1000px;
                    margin: 4rem auto;
                    background: rgba(30, 30, 30, 0.8);
                    border: 1px solid rgba(30, 144, 255, 0.15);
                    border-radius: 24px;
                    padding: 2.5rem;
                    backdrop-filter: blur(10px);
                    overflow-x: auto; /* Enable horizontal scrolling */
                    -webkit-overflow-scrolling: touch; /* Smooth scrolling on iOS */
                }

                .feature-comparison h2 {
                    color: #7EB2FF;
                    font-size: 2rem;
                    margin-bottom: 2rem;
                    text-align: center;
                }

                .feature-comparison table {
                    width: 100%;
                    border-collapse: collapse;
                    margin-top: 2rem;
                }

                .feature-comparison th, 
                .feature-comparison td {
                    padding: 1rem;
                    text-align: center;
                    border: 1px solid rgba(30, 144, 255, 0.1);
                }

                .feature-comparison th {
                    background: rgba(30, 144, 255, 0.1);
                    color: #7EB2FF;
                    font-weight: 600;
                }

                .feature-comparison td {
                    color: #e0e0e0;
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
                    grid-template-columns: repeat(2, 1fr);
                    gap: 2rem;
                    margin-top: 2rem;
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

                .details {
                    background: rgba(30, 30, 30, 0.8);
                    border: 1px solid rgba(30, 144, 255, 0.15);
                    border-radius: 12px;
                    padding: 1.5rem;
                    transition: all 0.3s ease;
                }

                .details:hover {
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
                    font-size: 1.1rem;
                    cursor: pointer;
                    transition: all 0.3s ease;
                    border: 1px solid rgba(255, 255, 255, 0.1);
                    width: 100%;
                    margin-top: 2rem;
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


                .oracle-topup-integrated {
                    margin-top: 2rem;
                    padding: 1.5rem;
                    background: rgba(20, 20, 20, 0.6);
                    border: 1px solid rgba(30, 144, 255, 0.2);
                    border-radius: 16px;
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

                .topup-section h4 {
                    color: #7EB2FF;
                    font-size: 1.3rem;
                    margin-bottom: 0.5rem;
                    text-align: center;
                }

                .topup-description {
                    color: #b0b0b0;
                    font-size: 0.95rem;
                    text-align: center;
                    margin-bottom: 1.5rem;
                }

                .topup-selector-inline {
                    display: flex;
                    flex-direction: column;
                    gap: 1rem;
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

                .topup-summary-inline {
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    padding: 1rem;
                    background: rgba(30, 144, 255, 0.1);
                    border-radius: 8px;
                    margin-top: 0.5rem;
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

                @media (max-width: 768px) {
                    .topup-controls {
                        grid-template-columns: 1fr;
                        gap: 1rem;
                    }
                    
                    .oracle-topup-selector {
                        margin: 2rem auto;
                    }
                    
                    .topup-card {
                        padding: 1.5rem;
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

                    .topup-summary-inline {
                        flex-direction: column;
                        gap: 1rem;
                    }
                }

                @media (max-width: 968px) {
                    .pricing-grid {
                        grid-template-columns: 1fr;
                    }

                    .pricing-header h1 {
                        font-size: 2.5rem;
                    }

                    .pricing-panel {
                        padding: 4rem 1rem;
                    }

                    .feature-comparison {
                        padding: 1.5rem;
                        margin: 2rem 1rem;
                        max-width: calc(100vw - 2rem); /* Ensure it stays within viewport */
                    }

                    .feature-comparison table {
                        min-width: 600px; /* Minimum width to maintain readability */
                    }
                }
                "#}
            </style>
        </div>
    }
}
