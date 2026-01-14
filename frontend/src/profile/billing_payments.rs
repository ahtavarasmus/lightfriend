use yew::prelude::*;
use serde::Deserialize;

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
pub fn PaymentMethodButton(_props: &PaymentMethodButtonProps) -> Html {
    html! {
        <div class="payment-method-container">
                {"placeholder"}
        </div>
    }
}
