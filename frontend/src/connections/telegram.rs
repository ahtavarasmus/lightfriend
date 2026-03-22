use yew::prelude::*;
use super::bridge_connect::{BridgeConnect, TELEGRAM_CONFIG};

#[derive(Properties, PartialEq)]
pub struct TelegramProps {
    pub user_id: i32,
    pub sub_tier: Option<String>,
}

#[function_component(TelegramConnect)]
pub fn telegram_connect(props: &TelegramProps) -> Html {
    html! {
        <BridgeConnect
            user_id={props.user_id}
            sub_tier={props.sub_tier.clone()}
            config={TELEGRAM_CONFIG}
        />
    }
}
