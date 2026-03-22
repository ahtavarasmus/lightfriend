use super::bridge_connect::{BridgeConnect, SIGNAL_CONFIG};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct SignalProps {
    pub user_id: i32,
    pub sub_tier: Option<String>,
}

#[function_component(SignalConnect)]
pub fn signal_connect(props: &SignalProps) -> Html {
    html! {
        <BridgeConnect
            user_id={props.user_id}
            sub_tier={props.sub_tier.clone()}
            config={SIGNAL_CONFIG}
        />
    }
}
