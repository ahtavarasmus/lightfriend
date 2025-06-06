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
}

#[derive(Properties, PartialEq, Clone)]
pub struct CheckoutButtonProps {
    pub user_id: i32,
    pub user_email: String,
    pub subscription_type: String,
}

#[function_component(CheckoutButton)]
pub fn checkout_button(props: &CheckoutButtonProps) -> Html {
    let user_id = props.user_id;
    let user_email = props.user_email.clone();
    let subscription_type = props.subscription_type.clone();

    let onclick = {
        let user_id = user_id.clone();
        let subscription_type = subscription_type.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let user_id = user_id.clone();
            let subscription_type = subscription_type.clone();
            
            wasm_bindgen_futures::spawn_local(async move {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|storage| storage.get_item("token").ok())
                    .flatten()
                {
                    let endpoint = if subscription_type == "hard_mode" {
                        format!("{}/api/stripe/hard-mode-subscription-checkout/{}", config::get_backend_url(), user_id)
                    } else {
                        format!("{}/api/stripe/subscription-checkout/{}", config::get_backend_url(), user_id)
                    };

                    let response = Request::post(&endpoint)
                        .header("Authorization", &format!("Bearer {}", token))
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
    let selected_country = use_state(|| "US".to_string());

    let basic_prices: HashMap<String, f64> = HashMap::from([
        ("US".to_string(), 8.00),
        ("FI".to_string(), 15.00),
        ("UK".to_string(), 15.00),
        ("AU".to_string(), 15.00),
        ("IL".to_string(), 35.00),
    ]);

    let premium_prices: HashMap<String, f64> = HashMap::from([
        ("US".to_string(), 15.00),
        ("FI".to_string(), 30.00),
        ("UK".to_string(), 30.00),
        ("AU".to_string(), 30.00),
        ("IL".to_string(), 65.00),
    ]);

    let voice_overage: HashMap<String, f64> = HashMap::from([
        ("US".to_string(), 0.20),
        ("FI".to_string(), 0.25),
        ("UK".to_string(), 0.25),
        ("AU".to_string(), 0.25),
        ("IL".to_string(), 0.20),
    ]);

    let message_overage: HashMap<String, f64> = HashMap::from([
        ("US".to_string(), 0.10),
        ("FI".to_string(), 0.30),
        ("UK".to_string(), 0.30),
        ("AU".to_string(), 0.40),
        ("IL".to_string(), 0.90),
    ]);

    let on_country_change = {
        let selected_country = selected_country.clone();
        Callback::from(move |e: Event| {
            if let Some(target) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                selected_country.set(target.value());
            }
        })
    };

    html! {
        <div class="pricing-container">
            <div class="pricing-header">
                <h1>{"Your Personal AI Assistant"}</h1>
                <p>{"Buy back over 100 hours every month*. Dumbphone sold separately."}</p>
            </div>

            <div class="country-selector">
                <label for="country">{"Select your country: "}</label>
                <select id="country" onchange={on_country_change}>
                    { for ["US", "FI", "UK", "AU", "IL"].iter().map(|&c| html! { <option value={c} selected={*selected_country == c}>{c}</option> }) }
                </select>
            </div>

            <div class="pricing-grid">
                <div class="pricing-card subscription basic">
                    <div class="card-header">
                        <h3>{"Hard Mode"}</h3>
                        <div class="price">
                            <span class="amount">{format!("€{:.2}", basic_prices.get(&*selected_country).unwrap_or(&0.0))}</span>
                            <span class="period">{"/month"}</span>
                        </div>
                        <div class="includes">
                            <p>{"Subscription includes:"}</p>
                            <ul class="quota-list">
                                <li>{"📞 40-unit quota (1 message = 1 minute)"}</li>
                            </ul>
                        </div>
                    </div>
                    {
                        if props.is_logged_in {
                            if props.sub_tier.is_none() {
                                html! {
                                    <CheckoutButton 
                                        user_id={props.user_id} 
                                        user_email={props.user_email.clone()} 
                                        subscription_type="hard_mode"
                                    />
                                }
                            } else {
                                html! {
                                    <button class="iq-button disabled" disabled=true><b>{"Already subscribed"}</b></button>
                                }
                            }
                        } else {
                            html! {
                                <Link<Route> to={Route::Register} classes="forward-link signup-link">
                                    <button class="iq-button signup-button"><b>{"Get Started"}</b></button>
                                </Link<Route>>
                            }

                        }
                    }
                </div>

                <div class="pricing-card subscription premium">
                    <div class="popular-tag">{"Most Popular"}</div>
                    <div class="card-header">
                        <h3>{"Escape Plan"}</h3>
                        <div class="price">
                            <span class="amount">{format!("€{:.2}", premium_prices.get(&*selected_country).unwrap_or(&0.0))}</span>
                            <span class="period">{"/month"}</span>
                        </div>
                        <div class="includes">
                            <p>{"Subscription includes:"}</p>
                            <ul class="quota-list">
                                <li>{"📞 40-unit quota (1 message = 1 minute)"}</li>
                                <li>{"🎯 Up to 60 filtered notifications/month"}</li>
                            </ul>
                        </div>
                    </div>
                    {
                        if props.is_logged_in {
                            if props.sub_tier.is_none() {
                                html! {
                                    <CheckoutButton 
                                        user_id={props.user_id} 
                                        user_email={props.user_email.clone()} 
                                        subscription_type="world"
                                    />
                                }
                            } else {
                                html! {
                                    <button class="iq-button disabled" disabled=true><b>{"Already subscribed"}</b></button>
                                }
                            }
                        } else {
                            html! {
                                <Link<Route> to={Route::Register} classes="forward-link signup-link">
                                    <button class="iq-button signup-button pro-signup"><b>{"Buy Back Your Time"}</b></button>
                                </Link<Route>>
                            }

                        }
                    }
                </div>
            </div>

            <div class="feature-comparison">
                <h2>{"Feature Comparison"}</h2>
                <table>
                    <thead>
                        <tr>
                            <th>{"Feature"}</th>
                            <th>{"Hard Mode"}</th>
                            <th>{"Escape Plan"}</th>
                        </tr>
                    </thead>
                    <tbody>
                        <tr>
                            <td>{"Search Internet with Perplexity"}</td>
                            <td>{"✅"}</td>
                            <td>{"✅"}</td>
                        </tr>
                        <tr>
                            <td>{"Fetch Current Weather"}</td>
                            <td>{"✅"}</td>
                            <td>{"✅"}</td>
                        </tr>
                        <tr>
                            <td>{"Fetch & Send WhatsApp Messages"}</td>
                            <td>{"❌"}</td>
                            <td>{"✅"}</td>
                        </tr>
                        <tr>
                            <td>{"Email: Fetch + Notifications"}</td>
                            <td>{"❌"}</td>
                            <td>{"✅"}</td>
                        </tr>
                        <tr>
                            <td>{"Calendar: Fetch & Create Events"}</td>
                            <td>{"❌"}</td>
                            <td>{"✅"}</td>
                        </tr>
                        <tr>
                            <td>{"Tasks: Fetch & Create"}</td>
                            <td>{"❌"}</td>
                            <td>{"✅"}</td>
                        </tr>
                        <tr>
                            <td>{"Recognize Songs with Shazam"}</td>
                            <td>{"❌"}</td>
                            <td>{"✅"}</td>
                        </tr>
                        <tr>
                            <td>{"24/7 Automated Monitoring"}</td>
                            <td>{"❌"}</td>
                            <td>{"✅"}</td>
                        </tr>
                        <tr>
                            <td>{"Filtered Notifications"}</td>
                            <td>{"❌"}</td>
                            <td>{"Up to 60/month"}</td>
                        </tr>
                        <tr>
                            <td>{"Priority Support"}</td>
                            <td>{"✅"}</td>
                            <td>{"✅"}</td>
                        </tr>
                    </tbody>
                </table>
            </div>

            <div class="usage-pricing">
                <h2>{format!("Overage Rates for {}", *selected_country)}</h2>
                <p>{"After your monthly quota, these rates apply for additional usage."}</p>
                <div class="usage-grid">
                    <div class="pricing-card main">
                        <div class="card-header">
                            <h3>{"Additional Voice Calls"}</h3>
                            <div class="price">
                                <span class="amount">{format!("€{:.2}", voice_overage.get(&*selected_country).unwrap_or(&0.0))}</span>
                                <span class="period">{"/minute"}</span>
                            </div>
                        </div>
                    </div>
                    <div class="pricing-card main">
                        <div class="card-header">
                            <h3>{"Additional Messages"}</h3>
                            <div class="price">
                                <span class="amount">{format!("€{:.2}", message_overage.get(&*selected_country).unwrap_or(&0.0))}</span>
                                <span class="period">{"/message"}</span>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            <div class="byo-twilio">
                <h3>{"Bring Your Own Twilio Number"}</h3>
                <p>{"Get 50% off any plan by using your own Twilio number. You’ll pay for messages directly to Twilio."}</p>
                <a href="mailto:rasmus@ahtava.com?subject=Bring Your Own Twilio Number">
                    <button class="iq-button signup-button">{"Contact Us to Set Up"}</button>
                </a>
            </div>

            <div class="pricing-faq">
                <h2>{"Common Questions"}</h2>
                <div class="faq-grid">
                    <details>
                        <summary>{"How does billing work?"}</summary>
                        <p>{"Pricing varies by country. Both plans include 40 units (messages or voice minutes). Escape Plan includes up to 60 filtered notifications per month and gives your agent more tools to work with. Overage rates apply after the quota, varying by country. Enable automatic top-up for uninterrupted service. No hidden fees or commitments."}</p>
                    </details>
                    <details>
                        <summary>{"What counts as a message/minute?"}</summary>
                        <p>{"Voice calls are counted by seconds when you call. Messages are counted per query, with free replies if the AI needs clarification."}</p>
                    </details>
                    <details>
                        <summary>{"How does automatic monitoring work?"}</summary>
                        <p>{"The AI monitors your email/calendar every minute, notifying you of important emails/events based on priority and custom criteria."}</p>
                    </details>
                </div>
            </div>

            <div class="footnotes">
                <p class="footnote">{"* Gen Z spends 4-7 hours daily on phones, often regretting 60% of social media time. "}<a href="https://explodingtopics.com/blog/smartphone-usage-stats" target="_blank" rel="noopener noreferrer">{"Read the study"}</a></p>
            </div>

            <div class="legal-links">
                <Link<Route> to={Route::Terms}>{"Terms & Conditions"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Privacy}>{"Privacy Policy"}</Link<Route>>
            </div>

<style>
    {r#"
    .pricing-container {
        position: relative;
        min-height: 100vh;
        padding: 6rem 2rem;
        color: #ffffff;
        z-index: 1;
        overflow: hidden;
    }

    .pricing-container::before {
        content: '';
        position: fixed;
        top: 0;
        left: 0;
        width: 100%;
        height: 100vh;
        background-image: url('/assets/human_looking_at_field.png');
        background-size: cover;
        background-position: center;
        background-repeat: no-repeat;
        opacity: 0.8;
        z-index: -2;
        pointer-events: none;
    }

    .pricing-container::after {
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
        grid-template-columns: repeat(2, 1fr);
        gap: 2rem;
        max-width: 1200px;
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
        background: rgba(30, 30, 30, 0.85);
        border: 1px solid rgba(30, 144, 255, 0.25);
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

    .card-header h3 {
        color: #7EB2FF;
        font-size: 2rem;
        margin-bottom: 1rem;
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

    .feature-comparison {
        max-width: 1000px;
        margin: 4rem auto;
        background: rgba(30, 30, 30, 0.8);
        border: 1px solid rgba(30, 144, 255, 0.15);
        border-radius: 24px;
        padding: 2.5rem;
        backdrop-filter: blur(10px);
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

    .usage-pricing {
        max-width: 1000px;
        margin: 4rem auto;
        text-align: center;
    }

    .usage-pricing h2 {
        color: #7EB2FF;
        font-size: 2rem;
        margin-bottom: 1rem;
    }

    .usage-pricing p {
        color: #999;
        margin-bottom: 2rem;
    }

    .usage-grid {
        display: grid;
        grid-template-columns: repeat(2, 1fr);
        gap: 2rem;
    }

    .pricing-card.main {
        background: rgba(30, 30, 30, 0.8);
        border: 1px solid rgba(30, 144, 255, 0.15);
        padding: 2rem;
    }

    .byo-twilio {
        max-width: 800px;
        margin: 4rem auto;
        text-align: center;
        background: rgba(30, 30, 30, 0.8);
        border: 1px solid rgba(30, 144, 255, 0.15);
        border-radius: 24px;
        padding: 2.5rem;
        backdrop-filter: blur(10px);
    }

    .byo-twilio h3 {
        color: #7EB2FF;
        font-size: 2rem;
        margin-bottom: 1rem;
    }

    .byo-twilio p {
        color: #e0e0e0;
        margin-bottom: 2rem;
        font-size: 1.1rem;
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

    @media (max-width: 968px) {
        .pricing-grid, .usage-grid {
            grid-template-columns: 1fr;
        }

        .pricing-header h1 {
            font-size: 2.5rem;
        }

        .pricing-container {
            padding: 4rem 1rem;
        }

        .feature-comparison {
            padding: 1.5rem;
            margin: 2rem auto;
        }

        .byo-twilio {
            padding: 1.5rem;
            margin: 2rem auto;
        }
    }
                .country-selector {
                    text-align: center;
                    margin: 2rem auto;
                    max-width: 400px;
                    background: rgba(30, 30, 30, 0.7);
                    padding: 1.5rem;
                    border-radius: 16px;
                    border: 1px solid rgba(30, 144, 255, 0.15);
                }
                .country-selector label {
                    color: #7EB2FF;
                    margin-right: 1rem;
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
                    margin: 0 auto;
                    display: block;
                    min-width: 120px;
                    text-align: center;
                    text-align-last: center;
                    -webkit-text-align-last: center;
                    -moz-text-align-last: center;
                }
                .pricing-grid {
                    display: grid;
                    grid-template-columns: repeat(2, 1fr);
                    gap: 2rem;
                    max-width: 1200px;
                    margin: 4rem auto;
                }
                .feature-comparison {
                    margin: 4rem auto;
                    max-width: 800px;
                    text-align: center;
                }
                .feature-comparison h2 {
                    color: #7EB2FF;
                    margin-bottom: 2rem;
                }
                .feature-comparison table {
                    width: 100%;
                    border-collapse: collapse;
                    background: rgba(30, 30, 30, 0.7);
                    border-radius: 8px;
                    overflow: hidden;
                }
                .feature-comparison th, .feature-comparison td {
                    padding: 1rem;
                    border: 1px solid rgba(30, 144, 255, 0.1);
                }
                .feature-comparison th {
                    background: rgba(30, 144, 255, 0.2);
                    color: #fff;
                }
                .feature-comparison td {
                    color: #999;
                }
                .byo-twilio {
                    margin: 4rem auto;
                    max-width: 600px;
                    padding: 2rem;
                    background: rgba(30, 30, 30, 0.7);
                    border-radius: 16px;
                    text-align: center;
                    border: 1px solid rgba(30, 144, 255, 0.1);
                }
                .byo-twilio h3 {
                    color: #7EB2FF;
                    margin-bottom: 1rem;
                }
                .byo-twilio p {
                    color: #999;
                    margin-bottom: 1.5rem;
                }
                details {
                    background: rgba(30, 30, 30, 0.7);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 8px;
                    margin-bottom: 1rem;
                    padding: 1rem;
                }
                summary {
                    color: #7EB2FF;
                    cursor: pointer;
                    font-size: 1.2rem;
                }
                details p {
                    color: #999;
                    margin-top: 1rem;
                }
                @media (max-width: 968px) {
                    .pricing-grid {
                        grid-template-columns: 1fr;
                    }
                    .usage-grid {
                        grid-template-columns: 1fr;
                    }
                }
                /* Existing styles retained and adjusted where necessary */
                .signup-button {
                    width: 100%;
                    background: linear-gradient(45deg, #1E90FF, #4169E1);
                    border: none;
                    color: white;
                    padding: 1rem 2rem;
                    border-radius: 8px;
                    font-size: 1.1rem;
                    cursor: pointer;
                    transition: all 0.3s ease;
                }
                .signup-button:hover {
                    background: linear-gradient(45deg, #4169E1, #1E90FF);
                    box-shadow: 0 4px 15px rgba(30, 144, 255, 0.4);
                }
                .pricing-card {
                    background: rgba(30, 30, 30, 0.7);
                    border: 1px solid rgba(30, 144, 255, 0.3);
                    border-radius: 16px;
                    padding: 2rem;
                    transition: all 0.3s ease-out;
                }
                .pricing-card:hover {
                    border-color: rgba(30, 144, 255, 0.4);
                    box-shadow: 0 15px 40px rgba(30, 144, 255, 0.15);
                }
                .card-header {
                    text-align: center;
                    margin-bottom: 2rem;
                }
                .price .amount {
                    font-size: 2rem;
                    color: #fff;
                }
                .price .period {
                    color: #999;
                    font-size: 1rem;
                }
                .popular-tag {
                    position: absolute;
                    top: 1rem;
                    right: 1rem;
                    background: linear-gradient(45deg, #1E90FF, #4169E1);
                    color: white;
                    padding: 0.5rem 1rem;
                    border-radius: 20px;
                }
                "#}
            </style>
        </div>
    }
}
