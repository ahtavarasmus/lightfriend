use crate::utils::api::Api;
use serde::Deserialize;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = window)]
    fn open(url: &str, target: &str, features: &str);
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct PricingTableConfig {
    pricing_table_id: String,
    publishable_key: String,
    customer_session_client_secret: Option<String>,
}

#[derive(Properties, PartialEq)]
pub struct StripePricingTableProps {
    #[prop_or_default]
    pub user_id: Option<i32>,
    #[prop_or_default]
    pub customer_email: Option<String>,
}

#[function_component(StripePricingTable)]
pub fn stripe_pricing_table(props: &StripePricingTableProps) -> Html {
    let config = use_state(|| None::<PricingTableConfig>);
    let error = use_state(|| None::<String>);

    {
        use_effect_with_deps(
            move |_| {
                let _ = js_sys::eval(
                    r#"
                    if (!document.querySelector('script[src="https://js.stripe.com/v3/pricing-table.js"]')) {
                        const script = document.createElement('script');
                        script.async = true;
                        script.src = 'https://js.stripe.com/v3/pricing-table.js';
                        document.head.appendChild(script);
                    }
                    "#,
                );
                || ()
            },
            (),
        );
    }

    {
        let config = config.clone();
        let error = error.clone();
        use_effect_with_deps(
            move |user_id| {
                let config = config.clone();
                let error = error.clone();
                let user_id = *user_id;
                spawn_local(async move {
                    error.set(None);
                    let endpoint = user_id
                        .map(|id| format!("/api/stripe/pricing-table-session/{}", id))
                        .unwrap_or_else(|| "/api/stripe/pricing-table-config".to_string());
                    match Api::get(&endpoint).send().await {
                        Ok(response) if response.ok() => {
                            match response.json::<PricingTableConfig>().await {
                                Ok(data) => config.set(Some(data)),
                                Err(_) => error.set(Some(
                                    "Failed to read Stripe pricing configuration".to_string(),
                                )),
                            }
                        }
                        Ok(_) => {
                            error.set(Some("Stripe pricing is not configured yet".to_string()))
                        }
                        Err(_) => error.set(Some("Could not load Stripe pricing".to_string())),
                    }
                });
                || ()
            },
            props.user_id,
        );
    }

    html! {
        <div class="stripe-pricing-table-wrap">
            {
                if let Some(config) = (*config).as_ref() {
                    let client_reference_id = props.user_id.map(|id| format!("user_{}", id));
                    let customer_session_client_secret = config
                        .customer_session_client_secret
                        .as_ref()
                        .filter(|secret| !secret.trim().is_empty());
                    let customer_email = props
                        .customer_email
                        .as_ref()
                        .filter(|email| !email.trim().is_empty())
                        .cloned();

                    match (
                        customer_session_client_secret,
                        customer_email.as_ref(),
                        client_reference_id.as_ref(),
                    ) {
                        (Some(client_secret), _, Some(reference_id)) => html! {
                            <stripe-pricing-table
                                pricing-table-id={config.pricing_table_id.clone()}
                                publishable-key={config.publishable_key.clone()}
                                customer-session-client-secret={client_secret.clone()}
                                client-reference-id={reference_id.clone()}
                            />
                        },
                        (Some(client_secret), _, None) => html! {
                            <stripe-pricing-table
                                pricing-table-id={config.pricing_table_id.clone()}
                                publishable-key={config.publishable_key.clone()}
                                customer-session-client-secret={client_secret.clone()}
                            />
                        },
                        (None, Some(email), Some(reference_id)) => html! {
                            <stripe-pricing-table
                                pricing-table-id={config.pricing_table_id.clone()}
                                publishable-key={config.publishable_key.clone()}
                                customer-email={email.clone()}
                                client-reference-id={reference_id.clone()}
                            />
                        },
                        (None, Some(email), None) => html! {
                            <stripe-pricing-table
                                pricing-table-id={config.pricing_table_id.clone()}
                                publishable-key={config.publishable_key.clone()}
                                customer-email={email.clone()}
                            />
                        },
                        (None, None, Some(reference_id)) => html! {
                            <stripe-pricing-table
                                pricing-table-id={config.pricing_table_id.clone()}
                                publishable-key={config.publishable_key.clone()}
                                client-reference-id={reference_id.clone()}
                            />
                        },
                        (None, None, None) => html! {
                            <stripe-pricing-table
                                pricing-table-id={config.pricing_table_id.clone()}
                                publishable-key={config.publishable_key.clone()}
                            />
                        },
                    }
                } else if let Some(message) = (*error).as_ref() {
                    html! { <div class="stripe-pricing-error">{message}</div> }
                } else {
                    html! { <div class="stripe-pricing-loading">{"Loading plans..."}</div> }
                }
            }
        </div>
    }
}
