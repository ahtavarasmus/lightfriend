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
        <button class="iq-button signup-button pro-signup" href="#" {onclick}><b>{"Subscribe Now!"}</b></button>
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
                <h1>{"Your Personal AI Assistant"}</h1>
                <p>{"Buy 135 hours of your life for 25 euros"}</p>
            </div>

            <div class="pricing-grid single">
                <div class="pricing-card subscription">
                    <div class="popular-tag">{"All-Inclusive"}</div>
                    <div class="card-header">
                        <h3>{"Subscription"}</h3>
                        <div class="price">
                            <span class="amount">{"€25.00"}</span>
                            <span class="period">{"/month"}</span>
                        </div>
                        <div class="includes">
                            <p>{"Subscription includes:"}</p>
                            <ul class="quota-list">
                                <li>{"⏰ 4.5 hours of extra time per day"}</li>
                                <li>{"📞 100-unit quota (1 message = 1 minute)"}</li>
                                <li>{"🎯 up to 100 filtered notifications"}</li>
                            </ul>
                        </div>
                    </div>
                    <ul>
                        <li><img src="/assets/whatsapplogo.png" alt="WhatsApp" class="feature-logo" />{"WhatsApp integration"}</li>
                        <li>{"📧 Email access & monitoring"}</li>
                        <li>{"📅 Calendar management"}</li>
                        <li>{"✅ Task management"}</li>
                        <li><img src="/assets/perplexitylogo.png" alt="Perplexity" class="feature-logo" />{" Perplexity AI search"}</li>
                        <li>{"🎵 Shazam song recognition"}</li>
                        <li>{"☀️ Weather updates"}</li>
                        <li>{"🔄 24/7 automated monitoring"}</li>
                        <li>{"🚀 Priority support"}</li>
                    </ul>
                    {
                        if props.is_logged_in && props.sub_tier.is_none() {
                            html! {
                                <CheckoutButton user_id={props.user_id} user_email={props.user_email.clone()} />
                            }
                        } else if !props.is_logged_in {
                            html! {
                                <Link<Route> to={Route::Register} classes="forward-link signup-link">
                                    <button class="iq-button signup-button pro-signup"><b>{"Buy Back Your Time Now"}</b></button>
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
                <h2>{"Overage Rates"}</h2>
                <p>{"After your monthly quota is used, these rates apply for additional usage"}</p>
                
                <div class="usage-grid">
                    <div class="pricing-card main">
                    <div class="card-header">
                        <h3>{"Additional Voice Calls"}</h3>
                        <div class="price">
                            <span class="amount">{"€0.20"}</span>
                            <span class="period">{"/minute"}</span>
                        </div>
                    </div>
                </div>


                <div class="pricing-card main">
                    <div class="card-header">
                        <h3>{"Additional Messages"}</h3>
                        <div class="price-container">
                            <div class="price">
                                <span class="amount">{"€0.20"}</span>
                                <span class="period">{"/message"}</span>
                            </div>
                        </div>
                    </div>
                    </div>

                </div>
            </div>


            <div class="pricing-faq">
                <h2>{"Common Questions"}</h2>
                <div class="faq-grid">
                    <div class="faq-item">
                        <h3>{"How does billing work?"}</h3>
                        <p>{"The Premium Plan is billed monthly at €25 and includes 60 messages or 60 minutes of voice calls. You can use these credits flexibly - for example, 30 messages and 30 minutes of calls. After your monthly quota is used, additional usage is billed at the pay-as-you-go rates. You can optionally enable automatic top-up to ensure uninterrupted service. No hidden fees or long-term commitments."}</p>

                    </div>
                    
                    <div class="faq-item">
                        <h3>{"When will I get charged?"}</h3>
                        <p>{"For voice calls, you are charged by the seconds when you initiate the call to lightfriend. The AI's responses during the call are included in this time."}</p>
                        <p>{"For SMS messages, you are charged for each complete query. If the AI needs more information to properly answer your query, you'll see '(free reply)' at the end of its message. Your response to such messages is free since the AI needed clarification to complete your original request, otherwise messages are charged."}</p>
                        <p>{"For example:"}</p>
                        <ul class="example-list">
                            <li>{"You: What's the weather like?"}</li>
                            <li>{"AI: In which city? (free reply)"}</li>
                            <li>{"You: In Helsinki"}</li>
                            <li>{"AI: The weather in Helsinki is... (this completes the interaction, you are charged once for the first message)"}</li>
                        </ul>
                    </div>
                    <div class="faq-item">
                        <h3>{"How does automatic email monitoring work?"}</h3>
                        <p>{"Our AI continuously monitors your email every minute, analyzing new messages using advanced criteria including priority senders, custom keywords, and waiting checks. It evaluates each email's importance on a scale of 0-10, considering factors like urgency indicators, sender importance, and content significance. You'll only be notified of truly important messages."}</p>
                    </div>

                    <div class="faq-item">
                        <h3>{"What are waiting checks?"}</h3>
                        <p>{"Waiting checks are temporary filters you can set up to watch for specific emails. For example, if you're expecting an important reply, you can create a waiting check. Once the matching email arrives, you'll be notified, and if configured, the check can automatically remove itself after finding the match."}</p>
                    </div>

                    <div class="faq-item">
                        <h3>{"What email services are supported?"}</h3>
                        <p>{"Currently, we support IMAP email monitoring, meaning all major email providers work."}</p>
                    </div>

                    <div class="faq-item">
                        <h3>{"What about refunds?"}</h3>
                        <p>{"Due to the pay-as-you-go nature of our service, we don't offer refunds."}</p>
                    </div>

                    <div class="faq-item">
                        <h3>{""}</h3>
                        <p>{""}</p>
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

                .pricing-grid.single {
                    grid-template-columns: 1fr;
                    max-width: 600px;
                    margin: 4rem auto;
                }

                .quota-list {
                    list-style: none;
                    padding: 0;
                    margin: 1rem 0;
                    font-size: 1rem;
                    color: #7EB2FF;
                }

                .quota-list li {
                    padding: 0.3rem 0;
                    text-align: center;
                }

                .includes {
                    margin-top: 1.5rem;
                    padding-top: 1.5rem;
                    border-top: 1px solid rgba(30, 144, 255, 0.1);
                }

                .includes p {
                    color: #999;
                    font-size: 1rem;
                    margin-bottom: 0.5rem;
                }

                .includes .value-prop {
                    color: #7EB2FF;
                    font-size: 1.1rem;
                    margin-top: 1rem;
                    font-weight: bold;
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
                .faq-grid {
                    display: flex;
                    flex-direction: column;
                    gap: 3rem;
                    max-width: 800px;
                    margin: 0 auto;
                }

                .faq-item {
                    border-bottom: 1px solid rgba(30, 144, 255, 0.1);
                    padding-bottom: 3rem;
                }

                .faq-item:last-child {
                    border-bottom: none;
                }

                .faq-item h3 {
                    color: #fff;
                    font-size: 1.8rem;
                    margin-bottom: 1.5rem;
                    font-weight: 600;
                }

                .faq-item p {
                    color: #999;
                    line-height: 1.8;
                    font-size: 1.1rem;
                    margin-bottom: 1rem;
                }

                .faq-item .example-list {
                    list-style: none;
                    padding: 1rem 1.5rem;
                    margin: 1rem 0;
                    background: rgba(30, 30, 30, 0.5);
                    border-left: 3px solid #1E90FF;
                    border-radius: 4px;
                }

                .faq-item .example-list li {
                    color: #999;
                    padding: 0.5rem 0;
                    font-family: monospace;
                    font-size: 0.95rem;
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
                    display: flex;
                    flex-direction: column;
                    gap: 2.5rem;
                    max-width: 800px;
                    margin: 0 auto;
                }


                .faq-item {
                    background: rgba(30, 30, 30, 0.7);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 16px;
                    padding: 2.5rem;
                    backdrop-filter: blur(10px);
                    transition: all 0.3s ease;
                }

                .faq-item:hover {

                    border-color: rgba(30, 144, 255, 0.3);
                    box-shadow: 0 8px 32px rgba(30, 144, 255, 0.1);
                }


                .faq-item h3 {
                    color: #7EB2FF;
                    font-size: 1.4rem;
                    margin-bottom: 1.5rem;
                    letter-spacing: 0.5px;
                }

                .faq-item p {
                    color: #999;
                    line-height: 1.8;
                    font-size: 1.1rem;
                    margin-bottom: 1rem;
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
