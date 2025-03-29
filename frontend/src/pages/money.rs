use yew::prelude::*;
use yew_router::prelude::*;
use crate::Route;
use serde_json::json;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = window)]
    fn openCheckout(items: JsValue, customer: JsValue, passthrough: JsValue);
}

#[derive(Properties, PartialEq, Clone)]
pub struct CheckoutButtonProps {
    pub user_id: i32,
    pub user_email: String,
}

#[function_component(CheckoutButton)]
pub fn checkout_button(props: &CheckoutButtonProps) -> Html {
    let user_id = props.user_id;
    let user_email = props.user_email.clone();
    let onclick = Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        // zero dollar subscription
        let items = json!([{
            "priceId": "pri_01jmqk1r39nk4h7bbr10jbatsz",
            "quantity": 1,
        }]);
        let customer_info = json!({
            "email": user_email,
        });
        let passthrough = json!({
            "user_id": user_id
        });
        openCheckout(
            serde_wasm_bindgen::to_value(&items).unwrap(),
            serde_wasm_bindgen::to_value(&customer_info).unwrap(),
            serde_wasm_bindgen::to_value(&passthrough).unwrap(),
        );
    });

    html! {
        <button class="iq-button" href="#" {onclick}><b>{"Sign up now"}</b></button>
    }
}


#[function_component(Pricing)]
pub fn pricing() -> Html {
    html! {
        <div class="pricing-container">
            <div class="pricing-header">
                <h1>{"Simple, Usage-Based Pricing"}</h1>
                <p>{"Pay only for what you use. No subscriptions, no commitments."}</p>
            </div>

            <div class="pricing-grid">
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
                        <li>{"All the tools available"}</li>
                        // TODO remove until i can give choice for this <li>{"No logging, fully private"}</li>
                        <li>{"24/7 availability"}</li>
                    </ul>
                </div>

                <div class="pricing-card main">
                    <div class="card-header">
                        <h3>{"SMS Messages"}</h3>
                        <div class="price-container">
                            <div class="price us-price">
                                <span class="region">{"US/Canada: "}</span>
                                <span class="amount">{"€0.10"}</span>
                                <span class="period">{"/message"}</span>
                            </div>
                            <div class="price other-price">
                                <span class="region">{"Other regions: "}</span>
                                <span class="amount">{"€0.15"}</span>
                                <span class="period">{"/message"}</span>
                            </div>
                        </div>
                    </div>
                    <ul>
                        <li>{"AI assistant chat responses"}</li>
                        <li>{"All the tools available"}</li>
                        // TODO remove until i can give choice for this <li>{"Message history, only on your device locally"}</li>
                        <li>{"24/7 availability"}</li>
                    </ul>
                </div>

                <div class="pricing-card features">
                    <div class="card-header">
                        <h3>{"Included Features"}</h3>
                    </div>
                    <ul>
                        <li>{"Smart AI Assistant"}</li>
                        <li>{"Perplexity Search"}</li>
                        <li>{"Weather"}</li>
                        <li>{"Shazam"}</li>
                        <li>{"Calendar Integration"}</li>
                        <li>{"Email Access"}</li>
                        <li>{"WhatsApp Integration(coming)"}</li>
                        <li>{"Telegram Integration(coming"}</li>
                        <li>{"24/7 Availability"}</li>
                    </ul>
                </div>
                </div>

            <div class="pricing-faq">
                <h2>{"Common Questions"}</h2>
                <div class="faq-grid">
                    <div class="faq-item">
                        <h3>{"How does billing work?"}</h3>
                        <p>{"You’ll purchase credits in advance to use for voice calls and SMS messages. You can optionally enable automatic top-up to recharge your account with additional credits whenever your balance runs low, ensuring uninterrupted service. No minimum fees or hidden charges."}</p>
                    </div>
                    
                    <div class="faq-item">
                        <h3>{"What counts as a message or call?"}</h3>
                        <p>{"You are charged for all messages and calls you initiate. For example texting 'shazam' to lightfriend, you will only be charged for the 'shazam' message and not the listening call."}</p>
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
        </div>
    }
}
