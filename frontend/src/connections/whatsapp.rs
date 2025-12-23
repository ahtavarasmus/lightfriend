use yew::prelude::*;
use super::bridge_connect::{BridgeConnect, WHATSAPP_CONFIG};

#[derive(Properties, PartialEq)]
pub struct WhatsappProps {
    pub user_id: i32,
    pub sub_tier: Option<String>,
    pub discount: bool,
}

#[function_component(WhatsappConnect)]
pub fn whatsapp_connect(props: &WhatsappProps) -> Html {
    html! {
        <BridgeConnect
            user_id={props.user_id}
            sub_tier={props.sub_tier.clone()}
            discount={props.discount}
            config={WHATSAPP_CONFIG}
        />
    }
}
