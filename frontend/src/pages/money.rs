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

#[derive(Deserialize, Clone)]
struct UserProfile {
    id: i32,
    email: String,
    sub_tier: Option<String>,
    phone_number: Option<String>,
}

#[derive(Properties, PartialEq, Clone)]
pub struct CheckoutButtonProps {
    pub user_id: i32,
    pub user_email: String,
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


#[function_component(CheckoutButton)]
pub fn checkout_button(props: &CheckoutButtonProps) -> Html {
    let user_id = props.user_id;
    let user_email = props.user_email.clone();

    let onclick = {
        let user_id = user_id.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let user_id = user_id.clone();
            
            // Create subscription checkout session
            wasm_bindgen_futures::spawn_local(async move {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|storage| storage.get_item("token").ok())
                    .flatten()
                {
                    let response = Request::post(&format!("{}/api/stripe/subscription-checkout/{}", config::get_backend_url(), user_id))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await;

                    match response {
                        Ok(resp) => {
                            if let Ok(json) = resp.json::<Value>().await {
                                if let Some(url) = json.get("url").and_then(|u| u.as_str()) {
                                    // Redirect to Stripe Checkout
                                    if let Some(window) = window() {
                                        let _ = window.location().set_href(url);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            // Handle error (you might want to show an error message to the user)
                        }
                    }
                }
            });
        })
    };

    html! {
        <button class="iq-button signup-button pro-signup" href="#" {onclick}><b>{"Subscribe Now with 3 Day free trial!"}</b></button>
    }
}


#[function_component(PricingWrapper)]
pub fn pricing_wrapper() -> Html {
    let profile_data = use_state(|| None::<UserProfile>);
    
    {
        let profile_data = profile_data.clone();
        
        use_effect_with_deps(move |_| {
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
                            if let Ok(profile) = response.json::<UserProfile>().await {
                                profile_data.set(Some(profile));
                            }
                        }
                        Err(_) => {}
                    }
                }
            });
            
            || ()
        }, ());
    }

    if let Some(profile) = (*profile_data).as_ref() {
        html! {
            <Pricing
                user_id={profile.id}
                user_email={profile.email.clone()}
                sub_tier={profile.sub_tier.clone()}
                is_logged_in={true}
                phone_number={profile.phone_number.clone()}
            />
        }
    } else {
        html! {
            <Pricing
                user_id={0}
                user_email={"".to_string()}
                sub_tier={None::<String>}
                is_logged_in={true}
                phone_number={None::<String>}
            />
        }
    }
}

#[function_component(Pricing)]
pub fn pricing(props: &PricingProps) -> Html {
    html! {
        <div class="pricing-container">
            <div class="pricing-header">
                <h1>{"Keep it Simple or Full Capability?"}</h1>
            </div>

            <div class="pricing-grid">
                <div class="pricing-card free">
                    <div class="card-header">
                        <h3>{"Free Plan"}</h3>
                        <div class="price">
                            <span class="amount">{"€0"}</span>
                            <span class="period">{"/forever"}</span>
                        </div>
                    </div>
                    <ul>
                        <li><img src="/assets/perplexitylogo.png" alt="Perplexity" class="feature-logo" />{" Perplexity AI search"}</li>
                        <li>{"☀️ Ask about the Weather"}</li>
                        <li>{"🤖 Basic AI assistance"}</li>
                    </ul>
                    {
                        if !props.is_logged_in {
                            html! {
                                <Link<Route> to={Route::Register} classes="forward-link signup-link">
                                    <button class="iq-button signup-button pro-signup"><b>{"Sign up now"}</b></button>
                                </Link<Route>>
                            }
                        } else {
                            html! {
                                <button class="iq-button disabled" disabled=true><b>{"Current Plan"}</b></button>
                            }
                        }
                    }
                </div>

                <div class="pricing-card subscription">
                    <div class="popular-tag">{"Most Popular"}</div>
                    <div class="card-header">
                        <h3>{"Pro Plan"}</h3>
                        <div class="price">
                            <span class="amount">{"€10.00"}</span>
                            <span class="period">{"/month"}</span>
                        </div>
                    </div>
                    <ul>
                        <li><img src="/assets/whatsapplogo.png" alt="WhatsApp" class="feature-logo" />{" WhatsApp integration"}</li>
                        <li>{"📧 Email access"}</li>
                        <li>{"📅 Calendar management"}</li>
                        <li>{"✅ Task management"}</li>
                        <li>{"🎵 Shazam song recognition"}</li>
                        <li><img src="/assets/perplexitylogo.png" alt="Perplexity" class="feature-logo" />{" Perplexity AI search"}</li>
                        <li>{"☀️ Ask about the Weather"}</li>
                        <li>{"🔄 24/7 automated monitoring (optional)"}</li>
                        <li>{"📱 Up to 150 proactive notifications/month"}</li>
                    </ul>
                    {
                        if props.is_logged_in && props.sub_tier.is_none() {
                            html! {
                                <CheckoutButton user_id={props.user_id} user_email={props.user_email.clone()} />
                            }
                        } else if !props.is_logged_in {
                            html! {
                                <Link<Route> to={Route::Register} classes="forward-link signup-link">
                                    <button class="iq-button signup-button pro-signup"><b>{"Sign up now"}</b></button>
                                </Link<Route>>
                            }
                        } else {
                            html! {
                                <button class="iq-button disabled" disabled=true><b>{"Already subscribed"}</b></button>
                            }
                        }
                    }
                </div>
            </div>

            <div class="usage-pricing">
                <h2>{"Talk & Text When You Need It"}</h2>
                <p>{"Pay only for what you use with our flexible usage-based pricing"}</p>
                
                <div class="usage-grid">
                    <div class="pricing-card main">
                        <div class="card-header">
                            <h3>{"Voice Calls"}</h3>
                            <div class="price">
                                <span class="amount">{"€0.20"}</span>
                                <span class="period">{"/minute"}</span>
                            </div>
                        </div>
                        <ul>
                            <li>{"High-quality AI assistant voice calls"}</li>
                            <li>{"Charged only when you take initiative"}</li>
                            <li>{"24/7 availability"}</li>
                        </ul>
                    </div>


                    <div class="pricing-card main">
                        <div class="card-header">
                            <h3>{"SMS Messages"}</h3>
                            <div class="price-container">
                                <div class="us-price">
                                    <div class="price">
                                        <span class="region">{"US: "}</span>
                                        <span class="amount">{"€0.10"}</span>
                                        <span class="period">{"/message"}</span>
                                    </div>
                                </div>
                                <div class="price">
                                    <span class="region">{"International: "}</span>
                                    <span class="amount">{"€0.20"}</span>
                                    <span class="period">{"/message"}</span>
                                </div>
                            </div>
                        </div>
                        <ul>
                            <li>{"AI assistant sms responses"}</li>
                            <li>{"Charged only when you take initiative"}</li>
                            <li>{"24/7 availability"}</li>
                        </ul>
                    </div>

                </div>
            </div>


            <div class="pricing-faq">
                <h2>{"Common Questions"}</h2>
                <div class="faq-grid">
                    <div class="faq-item">
                        <h3>{"How does billing work?"}</h3>
                        <p>{"You’ll purchase credits in advance to use for voice calls and SMS messages. You can optionally enable automatic top-up to recharge your account with additional credits whenever your balance runs low, ensuring uninterrupted service. No minimum fees or hidden charges. Proactive messaging subscription is paid on montly basis and gives 150 notification montly quota."}</p>
                    </div>
                    
                    <div class="faq-item">
                        <h3>{"What counts as a message or call?"}</h3>
                        <p>{"You are charged for all messages and calls you initiate. For example texting 'shazam' to lightfriend, you will only be charged for the 'shazam' message and not the listening call."}</p>
                    </div>

                    <div class="faq-item">
                        <h3>{"How does proactive email monitoring work?"}</h3>
                        <p>{"Our AI continuously monitors your email every minute, analyzing new messages using advanced criteria including priority senders, custom keywords, and waiting checks. It evaluates each email's importance on a scale of 0-10, considering factors like urgency indicators, sender importance, and content significance. You'll only be notified of truly important messages."}</p>
                    </div>

                    <div class="faq-item">
                        <h3>{"What are waiting checks?"}</h3>
                        <p>{"Waiting checks are temporary filters you can set up to watch for specific emails. For example, if you're expecting an important reply, you can create a waiting check. Once the matching email arrives, you'll be notified, and if configured, the check can automatically remove itself after finding the match."}</p>
                    </div>

                    <div class="faq-item">
                        <h3>{"How does the AI determine email importance?"}</h3>
                        <p>{"The AI uses a comprehensive evaluation process that checks for: urgency indicators (words like 'urgent', 'deadline'), sender importance (managers, clients), content significance (action items, meetings), context (CC'd stakeholders, business hours), and personal impact. You can also set your own importance threshold (default is 7/10) for notifications."}</p>
                    </div>

                    <div class="faq-item">
                        <h3>{"What about message quotas?"}</h3>
                        <p>{"The subscription includes up to 150 proactive notifications per month. You'll receive a notice when you're on your last notification, and your quota automatically resets at the start of each month. This helps manage costs while ensuring you don't miss truly important messages."}</p>
                    </div>

                    <div class="faq-item">
                        <h3>{"What email services are supported?"}</h3>
                        <p>{"Currently, we support IMAP email monitoring, meaning all major email providers work."}</p>
                    </div>



                    <div class="faq-item">
                        <h3>{"What about refunds?"}</h3>
                        <p>{"Due to the pay-as-you-go nature of our service, we don't offer refunds. You're only charged for services you actually use."}</p>
                    </div>
                </div>
            </div>

            <div class="legal-links">
                <Link<Route> to={Route::Terms}>{"Terms & Conditions"}</Link<Route>>
                {" | "}
                <Link<Route> to={Route::Privacy}>{"Privacy Policy"}</Link<Route>>
            </div>

            <style>
                {r#"
                .signup-link {
                    text-decoration: none;
                    display: block;
                    width: 100%;
                }

                .signup-button {
                    width: 100%;
                    background: linear-gradient(45deg, #1E90FF, #4169E1);
                    border: none;
                    color: white;
                    padding: 0.8rem 1.5rem;
                    border-radius: 8px;
                    font-size: 1rem;
                    cursor: pointer;
                    transition: all 0.3s ease;
                    text-transform: uppercase;
                    letter-spacing: 1px;
                }

                .signup-button:hover {
                    transform: translateY(-2px);
                    box-shadow: 0 4px 15px rgba(30, 144, 255, 0.4);
                    background: linear-gradient(45deg, #4169E1, #1E90FF);
                }

                .signup-button:active {
                    transform: translateY(1px);
                }

                .signup-button.pro-signup {
                    padding: 1rem 2rem;
                    font-size: 1.1rem;
                    background: linear-gradient(45deg, #4169E1, #1E90FF);
                    box-shadow: 0 4px 15px rgba(30, 144, 255, 0.2);
                }

                .signup-button.pro-signup:hover {
                    background: linear-gradient(45deg, #1E90FF, #4169E1);
                    box-shadow: 0 4px 20px rgba(30, 144, 255, 0.5);
                    transform: translateY(-3px);
                }



                .pricing-card.main {
                    background: rgba(30, 30, 30, 0.7);
                }

                .pricing-card.free {
                    background: rgba(30, 30, 30, 0.8);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    position: relative;
                }

                .pricing-card.free:hover {
                    border-color: rgba(30, 144, 255, 0.3);
                    box-shadow: 0 8px 32px rgba(30, 144, 255, 0.15);
                }

                .pricing-card.subscription {
                    background: rgba(30, 144, 255, 0.15);
                    border: 1px solid rgba(30, 144, 255, 0.3);
                    transform: scale(1.05);
                    position: relative;
                    overflow: hidden;
                }

                .pricing-card.subscription:hover {
                    transform: scale(1.08);
                    border-color: rgba(30, 144, 255, 0.5);
                    box-shadow: 0 8px 32px rgba(30, 144, 255, 0.25);
                    background: rgba(30, 144, 255, 0.2);
                }

                .popular-tag {
                    position: absolute;
                    top: 1rem;
                    right: 1rem;
                    background: linear-gradient(45deg, #1E90FF, #4169E1);
                    color: white;
                    padding: 0.5rem 1rem;
                    border-radius: 20px;
                    font-size: 0.9rem;
                    font-weight: 600;
                }

                .feature-logo {
                    height: 1em;
                    width: auto;
                    vertical-align: middle;
                    margin-right: 0.2em;
                }

                .pricing-container {
                    margin: 2rem auto;
                    padding: 6rem 6rem;
                    min-height: 100vh;
                    background: #1a1a1a;
                }

                .pricing-header {
                    text-align: center;
                    margin-bottom: 4rem;
                }

                .pricing-header h1 {
                    font-size: 3.5rem;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                    margin-bottom: 1.5rem;
                }

                .pricing-header p {
                    color: #999;
                    font-size: 1.2rem;
                    max-width: 600px;
                    margin: 0 auto;
                }

                .pricing-grid {
                    display: grid;
                    grid-template-columns: repeat(2, 1fr);
                    gap: 2rem;
                    margin: 4rem 0;
                }

                .usage-pricing {
                    margin: 6rem 0;
                    text-align: center;
                }

                .usage-pricing h2 {
                    font-size: 2.5rem;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                    margin-bottom: 1rem;
                }

                .usage-pricing p {
                    color: #999;
                    font-size: 1.2rem;
                    margin-bottom: 3rem;
                }

                .usage-grid {
                    display: grid;
                    grid-template-columns: repeat(2, 1fr);
                    gap: 2rem;
                }

                .pricing-card {
                    background: rgba(30, 30, 30, 0.7);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 16px;
                    padding: 2rem;
                    transition: all 0.3s ease;
                    backdrop-filter: blur(10px);
                }

                .pricing-card:hover {
                    transform: translateY(-5px);
                    box-shadow: 0 8px 32px rgba(30, 144, 255, 0.15);
                    border-color: rgba(30, 144, 255, 0.4);
                }

                .iq-button.disabled {
                    opacity: 0.8;
                    cursor: not-allowed;
                    background: linear-gradient(45deg, #666, #888);
                    border: none;
                    color: white;
                    padding: 0.8rem 1.5rem;
                    border-radius: 8px;
                    font-size: 1rem;
                    text-transform: uppercase;
                    letter-spacing: 1px;
                    transition: all 0.3s ease;
                    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
                }

                .iq-button.disabled:hover {
                    transform: none;
                    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
                    background: linear-gradient(45deg, #666, #888);
                }

                .pricing-card.subscription .iq-button.disabled {
                    padding: 1rem 2rem;
                    font-size: 1.1rem;
                    background: linear-gradient(45deg, #555, #777);
                }

                .card-header {
                    text-align: center;
                    margin-bottom: 2rem;
                    padding-bottom: 2rem;
                    border-bottom: 1px solid rgba(30, 144, 255, 0.1);
                }

                .card-header h3 {
                    color: #7EB2FF;
                    font-size: 1.5rem;
                    margin-bottom: 1rem;
                }

                .price-container {
                    display: flex;
                    flex-direction: column;
                    gap: 1rem;
                    align-items: center;
                }

                .price {
                    display: flex;
                    align-items: baseline;
                    justify-content: center;
                    gap: 0.25rem;
                }

                .price .region {
                    color: #7EB2FF;
                    font-size: 1rem;
                }

                .price .amount {
                    font-size: 2rem;
                    color: #fff;
                    font-weight: 600;
                }

                .price .period {
                    color: #999;
                    font-size: 1rem;
                }

                .price .region-note {
                    color: #7EB2FF;
                    font-size: 0.8rem;
                    margin-top: 0.5rem;
                    opacity: 0.8;
                }

                .us-price {
                    border-bottom: 1px solid rgba(30, 144, 255, 0.1);
                    padding-bottom: 0.5rem;
                }

                @media (max-width: 768px) {
                    .price .amount {
                        font-size: 1.75rem;
                    }
                    
                    .price .region {
                        font-size: 0.9rem;
                    }
                }

                .pricing-card ul {
                    list-style: none;
                    padding: 0;
                    margin: 0;
                }

                .pricing-card li {
                    color: #fff;
                    padding: 1rem 0;
                    display: flex;
                    align-items: center;
                    gap: 0.75rem;
                    font-size: 1.1rem;
                }

                .pricing-card li img {
                    margin-right: 0.5rem;
                }

                .pricing-card.subscription li {
                    border-bottom: 1px solid rgba(30, 144, 255, 0.1);
                }

                .pricing-card.subscription li:last-child {
                    border-bottom: none;
                }

                .pricing-card.free li {
                    border-bottom: 1px solid rgba(30, 144, 255, 0.1);
                }

                .pricing-card.free li:last-child {
                    border-bottom: none;
                }

                .pricing-faq {
                    margin-top: 6rem;
                    padding-top: 6rem;
                    border-top: 1px solid rgba(30, 144, 255, 0.1);
                }

                .pricing-faq h2 {
                    text-align: center;
                    font-size: 2.5rem;
                    color: #7EB2FF;
                    margin-bottom: 3rem;
                }

                .faq-grid {
                    display: grid;
                    grid-template-columns: repeat(auto-fit, minmax(350px, 1fr));
                    gap: 2.5rem;
                    max-width: 1400px;
                    margin: 0 auto;
                }
                }

                .faq-item {
                    background: rgba(30, 30, 30, 0.7);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 16px;
                    padding: 2rem;
                    backdrop-filter: blur(10px);
                    transition: all 0.3s ease;
                }

                .faq-item:hover {
                    transform: translateY(-5px);
                    border-color: rgba(30, 144, 255, 0.3);
                    box-shadow: 0 8px 32px rgba(30, 144, 255, 0.1);
                }
                }

                .faq-item h3 {
                    color: #7EB2FF;
                    font-size: 1.2rem;
                    margin-bottom: 1rem;
                }

                .faq-item p {
                    color: #999;
                    line-height: 1.6;
                }

                @media (max-width: 968px) {
                    .pricing-grid {
                        grid-template-columns: 1fr;
                    }

                    .usage-grid {
                        grid-template-columns: 1fr;
                    }
                    
                    .pricing-header h1 {
                        font-size: 2.5rem;
                    }
                }

                @media (max-width: 768px) {
                    .pricing-grid {
                        grid-template-columns: 1fr;
                    }
                    
                    .pricing-header h1 {
                        font-size: 2rem;
                    }
                    
                    .pricing-container {
                        padding: 4rem 1rem;
                    }
                    
                    .faq-grid {
                        grid-template-columns: 1fr;
                    }
                }
                "#}
            </style>
        </div>
    }
}
