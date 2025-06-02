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
                <p>{"Buy back over 100 hours every month*. Dumbphone sold separately."}</p>
            </div>

            <div class="pricing-grid">
                /* {Hard Mode tier temporarily commented out
                <div class="pricing-card subscription basic">
                    <div class="card-header">
                        <h3>{"Hard Mode"}</h3>
                        <div class="price-container">
                            {
                                html! {
                                    <>
                                        <div class="price">
                                            <span class="amount">{"€7.50"}</span>
                                            <span class="period">{"/month"}</span>
                                        </div>
                                    </>
                                }
                            }
                        </div>
                        <div class="includes">
                            <p>{"Subscription includes:"}</p>
                            <ul class="quota-list">
                                <li>{"📞 30-unit quota (1 message = 1 minute)"}</li>
                                <li class="unavailable">{"🎯 filtered notifications"}</li>
                            </ul>
                        </div>
                    </div>
                    <ul>
                        <li><img src="/assets/perplexitylogo.png" alt="Perplexity" class="feature-logo" />{"Search Internet with Perplexity"}</li>
                        <li>{"☀️ Fetch Current Weather"}</li>
                        <li class="unavailable"><img src="/assets/whatsapplogo.png" alt="WhatsApp" class="feature-logo" />{"Fetch & Send WhatsApp Messages"}</li>
                        <li class="unavailable">{"📧 Email: Fetch Emails + Receive Notifications"}</li>
                        <li class="unavailable">{"📅 Calendar: Fetch & Create Events + Receive Notifications"}</li>
                        <li class="unavailable">{"✅ Tasks: Fetch & Create Tasks"}</li>
                        <li class="unavailable">{"🎵 Recognize Songs with Shazam"}</li>
                        <li class="unavailable">{"🔄 24/7 automated monitoring"}</li>
                        <li class="unavailable">{"🚀 Priority support"}</li>
                    </ul>
                    {
                        if props.is_logged_in && props.sub_tier.is_none() {
                            html! {
                                <CheckoutButton user_id={props.user_id} user_email={props.user_email.clone()} />
                            }
                        } else if !props.is_logged_in {
                            html! {
                                <Link<Route> to={Route::Register} classes="forward-link signup-link">
                                    <button class="iq-button signup-button"><b>{"Get Started"}</b></button>
                                </Link<Route>>
                            }
                        } else {
                            html! {
                                <button class="iq-button disabled" disabled=true><b>{"Already subscribed"}</b></button>
                            }
                        }
                    }
                </div>
                }*/

                <div class="pricing-card subscription premium">
                    <div class="popular-tag">{"All-Inclusive"}</div>
                    <div class="popular-tag">{"All-Inclusive"}</div>
                    <div class="card-header">
                        <h3>{"Escape Plan"}</h3>
                        <div class="price-container">
                            {
                                if !props.is_logged_in {
                                    html! {
                                        <>
                                            <div class="price">
                                                <span class="amount">{"€20.00"}</span>
                                                <span class="period">{"/month"}</span>
                                            </div>
                                        </>
                                    }
                                } else {
                                    let is_us = props.phone_number.as_ref()
                                        .map(|num| num.starts_with("+1"))
                                        .unwrap_or(false);
                                    
                                    html! {
                                        <div class="price">
                                            <span class="amount">{if is_us { "€15.00" } else { "€20.00" }}</span>
                                            <span class="period">{"/month"}</span>
                                        </div>
                                    }
                                }
                            }
                        </div>
                        <div class="includes">
                            <p>{"Subscription includes:"}</p>
                            <ul class="quota-list">
                                <li>{"📞 100-unit quota (1 message = 1 minute)"}</li>
                                <li>{"🎯 up to 100 filtered notifications"}</li>
                            </ul>
                        </div>
                    </div>
                    <ul>
                        <li><img src="/assets/perplexitylogo.png" alt="Perplexity" class="feature-logo" />{"Search Internet with Perplexity"}</li>
                        <li>{"☀️ Fetch Current Weather"}</li>
                        <li><img src="/assets/whatsapplogo.png" alt="WhatsApp" class="feature-logo" />{"Fetch & Send WhatsApp Messages"}</li>
                        <li>{"📧 Email: Fetch Emails + Receive Notifications"}</li>
                        <li>{"📅 Calendar: Fetch & Create Events + Receive Notifications"}</li>
                        <li>{"✅ Tasks: Fetch & Create Tasks"}</li>
                        <li>{"🎵 Recognize Songs with Shazam"}</li>
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
                <p>{"After your monthly quota is used, these rates apply for additional usage. You have to buy these in advance or setup automatic recharge if you want to prepare to never run out."}</p>
                
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
                        <p>{"The Premium Plan is billed monthly at €15 for US customers and €20 for international customers. The plan includes 100 messages or 100 minutes of voice calls. You can use these credits flexibly - for example, 50 messages and 50 minutes of calls. After your monthly quota is used, additional usage is billed at the pay-as-you-go rates. You can optionally enable automatic top-up to ensure uninterrupted service. Plan also includes 100 proactive messages which can be customized to your liking. No hidden fees or long-term commitments."}</p>

                    </div>
                    
                    <div class="faq-item">
                        <h3>{"What counts as a message/minute?"}</h3>
                        <p>{"Voice calls are counted by the seconds when you initiate the call to lightfriend. The AI's responses during the call are included in this time."}</p>
                        <p>{"Message is couted for each complete query you initiate. If the AI needs more information to properly answer your query, you'll see '(free reply)' at the end of its message. Your response to such messages is free since the AI needed clarification to complete your original request, otherwise messages are counted."}</p>
                        <p>{"For example:"}</p>
                        <ul class="example-list">
                            <li>{"You: What's the weather like?"}</li>
                            <li>{"AI: In which city? (free reply)"}</li>
                            <li>{"You: In Helsinki"}</li>
                            <li>{"AI: The weather in Helsinki is... (this completes the interaction and only one message is counted)"}</li>
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
                        <p>{"Due to the high cost of running the service, we don't offer refunds."}</p>
                    </div>

                </div>
            </div>

            <div class="footnotes">
                <p class="footnote">{"* Gen Z spends an average of 4 to 7 hours a day on their phones, often with up to 60% of that time on social media sessions they later regret, wishing they'd used those hours for something more meaningful. "}<a href="https://explodingtopics.com/blog/smartphone-usage-stats" target="_blank" rel="noopener noreferrer">{"Read the study"}</a></p>
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
                    padding: 1rem 2rem;
                    border-radius: 8px;
                    font-size: 1.1rem;
                    cursor: pointer;
                    transition: all 1s cubic-bezier(0.4, 0, 0.2, 1);
                    text-transform: uppercase;
                    letter-spacing: 1px;
                    position: relative;
                    overflow: hidden;
                }



                .signup-button::before {
                    content: '';
                    position: absolute;
                    top: 0;
                    left: 0;
                    width: 100%;
                    height: 100%;
                    background: linear-gradient(
                        45deg,
                        transparent,
                        rgba(255, 255, 255, 0.1),
                        transparent
                    );
                    transform: translateX(-100%);
                    transition: transform 1.5s cubic-bezier(0.4, 0, 0.2, 1);
                }

.signup-button:hover::before {
    transform: translateX(100%);
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

                .footnotes {
                    margin: 4rem auto;
                    max-width: 800px;
                    padding: 2rem;
                    background: rgba(30, 30, 30, 0.7);
                    border-radius: 16px;
                    border: 1px solid rgba(30, 144, 255, 0.1);
                }

                .footnote {
                    color: #999;
                    font-size: 0.9rem;
                    line-height: 1.6;
                    margin-bottom: 1rem;
                }

                .footnote:last-child {
                    margin-bottom: 0;
                }

                .footnote a {
                    color: #7EB2FF;
                    text-decoration: none;
                    transition: color 0.3s ease;
                }

                .footnote a:hover {
                    color: #fff;
                    text-decoration: underline;
                }

.pricing-card.free {
    background: rgba(30, 30, 30, 0.8);
    border: 1px solid rgba(30, 144, 255, 0.1);
    position: relative;
    transition: all 0.3s ease-out;
}

                .pricing-card.free:hover {
                    border-color: rgba(30, 144, 255, 0.3);
                    box-shadow: 0 10px 30px rgba(30, 144, 255, 0.1);
                }

.pricing-card.subscription {
    background: rgba(30, 30, 30, 0.7);
    border: 1px solid rgba(30, 144, 255, 0.3);
    transform: scale(1.05);
    position: relative;
    overflow: hidden;
    backdrop-filter: blur(10px);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
    transition: all 0.3s ease-out;
}

.pricing-card.subscription::before {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 100%;
    background: linear-gradient(
        180deg,
        rgba(30, 144, 255, 0.15) 0%,
        rgba(30, 144, 255, 0.05) 100%
    );
    z-index: -1;
    transition: opacity 0.3s ease-out;
}

.pricing-card.subscription:hover {
    border-color: rgba(30, 144, 255, 0.4);
    box-shadow: 0 15px 40px rgba(30, 144, 255, 0.15);
}

.pricing-card.subscription:hover::before {
    opacity: 0.8;
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
    margin: 0 auto;
    padding: 6rem 2rem;
    min-height: 100vh;
    background: transparent;
    position: relative;
    overflow: hidden;
    z-index: 1;
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
    opacity: 1;
    z-index: -1;
}

.pricing-container::after {
    content: '';
    position: fixed;
    bottom: 0;
    left: 0;
    width: 100%;
    height: 50%;
    background: linear-gradient(to bottom, 
        rgba(26, 26, 26, 0) 0%,
        rgba(26, 26, 26, 1) 100%
    );
    z-index: -1;
}

@keyframes float {
    0% {
        transform: translate(0, 0);
    }
    100% {
        transform: translate(20px, 20px);
    }
}

                .pricing-header {
                    text-align: center;
                    color: rgba(0, 0, 0, 0.8);
                    margin-bottom: 4rem;
                }

                .pricing-header h1 {
                    font-size: 3.4rem;
                    line-height: 1.1;
                    font-weight: 400;
                    max-width: 400px;
                    font-family: 'Cormorant Garamond', serif;
                    letter-spacing: 0.02em;
                    text-align: center;
                    margin: 0 auto 20px;
                    text-shadow: 0 2px 4px rgba(255, 255, 255, 0.3);
                    font-style: italic;
                    transform: translateZ(0);
                    animation: whisperIn 1.5s ease-out forwards;
                }

@keyframes whisperIn {
    0% {
        opacity: 0;
        transform: translateY(20px);
    }
    100% {
        opacity: 1;
        transform: translateY(0);
    }
}

                .pricing-header p {
                    color: rgba(0, 0, 0, 0.8);
                    font-size: 1.2rem;
                    max-width: 600px;
                    margin: 0 auto;
                    text-shadow: 0 1px 2px rgba(255, 255, 255, 0.3);
                    font-weight: 500;
                }

.pricing-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 2rem;
    margin: 4rem 0;
    position: relative;
    z-index: 2;
}

                .pricing-grid {
                    grid-template-columns: 1fr;
                    max-width: 600px;
                    margin: 4rem auto;
                    gap: 2rem;
                }

                .pricing-card.basic {
                    transform: none;
                }

                .pricing-card.premium {
                    transform: scale(1.05);
                }

                @media (max-width: 968px) {
                    .pricing-grid {
                        grid-template-columns: 1fr;
                        max-width: 600px;
                    }
                    
                    .pricing-card.basic,
                    .pricing-card.premium {
                        transform: none;
                    }
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
    font-size: 3rem;
    background: linear-gradient(45deg, #fff, #7EB2FF);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    margin-bottom: 1rem;
    font-weight: 700;
    text-shadow: 0 0 20px rgba(30, 144, 255, 0.2);
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
    transition: all 0.3s ease-out;
    backdrop-filter: blur(10px);
    position: relative;
}

                .pricing-card:hover {
                    border-color: rgba(30, 144, 255, 0.3);
                    box-shadow: 0 10px 30px rgba(30, 144, 255, 0.1);
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

                .pricing-card li.unavailable {
                    color: rgba(255, 255, 255, 0.4);
                    text-decoration: line-through;
                    opacity: 0.7;
                }

                .pricing-card li.unavailable img {
                    opacity: 0.4;
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
    position: relative;
    z-index: 2;
}

.pricing-faq h2 {
    text-align: center;
    font-size: 3rem;
    background: linear-gradient(45deg, #fff, #7EB2FF);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    margin-bottom: 3rem;
    font-weight: 700;
    text-shadow: 0 0 20px rgba(30, 144, 255, 0.2);
}

.faq-grid {
    display: flex;
    flex-direction: column;
    gap: 2.5rem;
    max-width: 800px;
    margin: 0 auto;
    position: relative;
    z-index: 2;
    padding: 2rem;
    border-radius: 16px;
}


.faq-item {
    background: rgba(30, 30, 30, 0.7);
    border: 1px solid rgba(30, 144, 255, 0.1);
    border-radius: 16px;
    padding: 2.5rem;
    backdrop-filter: blur(10px);
    transition: transform 1.5s cubic-bezier(0.4, 0, 0.2, 1), border-color 1.5s ease, box-shadow 1.5s ease;
    position: relative;
    overflow: hidden;
}

.faq-item::before {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 100%;
    background: linear-gradient(
        180deg,
        rgba(30, 144, 255, 0.05) 0%,
        transparent 100%
    );
    z-index: -1;

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
