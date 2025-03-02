use yew::prelude::*;
use web_sys::window;
use gloo_net::http::Request;
use serde::Deserialize;
use wasm_bindgen_futures::spawn_local;
use gloo_timers::future::TimeoutFuture;

#[derive(Properties, PartialEq, Clone)]
pub struct PaymentMethodButtonProps {
    pub user_id: i32, // User ID for the Stripe customer
}

#[derive(Deserialize, Clone, PartialEq)]
pub struct ApiResponse {
    pub success: bool,
    pub message: String,
}

#[function_component]
pub fn PaymentMethodButton(props: &PaymentMethodButtonProps) -> Html {
    html! {
        <div class="payment-method-container">
                {"placeholder"}
        </div>
    }
}
