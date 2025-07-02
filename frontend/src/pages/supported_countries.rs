use yew::prelude::*;
use std::collections::HashMap;
use wasm_bindgen_futures;
use web_sys::{Request, RequestInit, window, Response};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use serde_json::Value;

#[derive(Clone, PartialEq)]
enum FeatureStatus {
    Available,
    Unavailable,
    Unsure,
}

#[derive(Clone, PartialEq)]
struct CountryFeatureAvailability {
    lightfriend_can_provide_local_number: FeatureStatus,
    user_can_bring_their_own_number: FeatureStatus,
    local_number: FeatureStatus,
    inbound_calling: FeatureStatus,
    outbound_calling: FeatureStatus,
    inbound_sms: FeatureStatus,
    outbound_sms: FeatureStatus,
    mms_messages: FeatureStatus,
    notes: Option<String>,
}

#[function_component(SupportedCountries)]
pub fn supported_countries() -> Html {
    let countries = use_state(|| {
        let mut map: HashMap<String, CountryFeatureAvailability> = HashMap::new();
        let mut country_order: Vec<String> = Vec::new();
        map.insert("United States".to_string(), CountryFeatureAvailability {
            lightfriend_can_provide_local_number: FeatureStatus::Available,
            user_can_bring_their_own_number: FeatureStatus::Unavailable,
            local_number: FeatureStatus::Available,
            inbound_calling: FeatureStatus::Available,
            outbound_calling: FeatureStatus::Available,
            inbound_sms: FeatureStatus::Available,
            outbound_sms: FeatureStatus::Available,
            mms_messages: FeatureStatus::Available,
            notes: Some("Full support including MMS".to_string()),
        });
        map.insert("United Kingdom".to_string(), CountryFeatureAvailability {
            lightfriend_can_provide_local_number: FeatureStatus::Available,
            user_can_bring_their_own_number: FeatureStatus::Unavailable,
            local_number: FeatureStatus::Available,
            inbound_calling: FeatureStatus::Available,
            outbound_calling: FeatureStatus::Available,
            inbound_sms: FeatureStatus::Available,
            outbound_sms: FeatureStatus::Available,
            mms_messages: FeatureStatus::Unavailable,
            notes: Some("MMS messages are only not supported in Europe".to_string()),
        });
        map.insert("Finland".to_string(), CountryFeatureAvailability {
            lightfriend_can_provide_local_number: FeatureStatus::Available,
            user_can_bring_their_own_number: FeatureStatus::Unavailable,
            local_number: FeatureStatus::Available,
            inbound_calling: FeatureStatus::Available,
            outbound_calling: FeatureStatus::Available,
            inbound_sms: FeatureStatus::Available,
            outbound_sms: FeatureStatus::Available,
            mms_messages: FeatureStatus::Unavailable,
            notes: Some("MMS messages are not supported in Europe".to_string()),
        });
        map.insert("Australia".to_string(), CountryFeatureAvailability {
            lightfriend_can_provide_local_number: FeatureStatus::Available,
            user_can_bring_their_own_number: FeatureStatus::Unavailable,
            local_number: FeatureStatus::Available,
            inbound_calling: FeatureStatus::Available,
            outbound_calling: FeatureStatus::Available,
            inbound_sms: FeatureStatus::Available,
            outbound_sms: FeatureStatus::Available,
            mms_messages: FeatureStatus::Available,
            notes: Some("Full support including MMS".to_string()),
        });
        map.insert("Denmark".to_string(), CountryFeatureAvailability {
            lightfriend_can_provide_local_number: FeatureStatus::Available,
            user_can_bring_their_own_number: FeatureStatus::Available,
            local_number: FeatureStatus::Unavailable,
            inbound_calling: FeatureStatus::Available,
            outbound_calling: FeatureStatus::Available,
            inbound_sms: FeatureStatus::Available,
            outbound_sms: FeatureStatus::Available,
            mms_messages: FeatureStatus::Unavailable,
            notes: Some("Lightfriend can provide a local number in case there is enough interest. Number causes 15e/month extra to the normal european cost.".to_string()),
        });
        map.insert("United Arab Emirates".to_string(), CountryFeatureAvailability {
            lightfriend_can_provide_local_number: FeatureStatus::Unavailable,
            user_can_bring_their_own_number: FeatureStatus::Unavailable,
            local_number: FeatureStatus::Unavailable,
            inbound_calling: FeatureStatus::Available,
            outbound_calling: FeatureStatus::Available,
            inbound_sms: FeatureStatus::Available,
            outbound_sms: FeatureStatus::Unavailable,
            mms_messages: FeatureStatus::Unavailable,
            notes: Some("Registering a number to work in UAE costs 225e + 115e/month which has not been feasible yet(https://help.twilio.com/articles/223133767-International-support-for-Alphanumeric-Sender-ID). Local number cannot be bought. Indonesian number can be bought(it is cheaper to call from UAE than other options), but it costs 25e/month extra. It is possible to send lightfriend messages with sms even though lightfriend cannot respond - could be useful for sending whatsapp messages and other features that are for creating something. Voice calling works normally.".to_string()),
        });

        map.insert("Israel".to_string(), CountryFeatureAvailability {
            lightfriend_can_provide_local_number: FeatureStatus::Available,
            user_can_bring_their_own_number: FeatureStatus::Available,
            local_number: FeatureStatus::Unavailable,
            inbound_calling: FeatureStatus::Available,
            outbound_calling: FeatureStatus::Available,
            inbound_sms: FeatureStatus::Available,
            outbound_sms: FeatureStatus::Available,
            mms_messages: FeatureStatus::Unavailable,
            notes: Some("Lightfriend can provide a local number if there is enough interest. Number costs 15e/month. Messaging is very expensive with message segment costing 0.25€ so normal response can be up to 1€ per message. So monthly price would have to around 120€/month for the Monitoring Plan and 40€/month for the Basic Plan. If you bring your own number, Monitoring Plan can be provided with 30€/month and Basic Plan 10€ per month.".to_string()),
        });

        map.insert("Germany".to_string(), CountryFeatureAvailability {
            lightfriend_can_provide_local_number: FeatureStatus::Unavailable,
            user_can_bring_their_own_number: FeatureStatus::Available,
            local_number: FeatureStatus::Unavailable,
            inbound_calling: FeatureStatus::Available,
            outbound_calling: FeatureStatus::Available,
            inbound_sms: FeatureStatus::Available,
            outbound_sms: FeatureStatus::Available,
            mms_messages: FeatureStatus::Unavailable,
            notes: Some("Germany requires local business address to be able to buy german number so lightfriend cannot provide it. User can buy their own number from Twilio and use it with lightfriend though. Otherwise lightfriend can be used by calling UK number which is usually the cheapest option for many phone plans in Germany.".to_string()),
        });

        map.insert("Other Countries".to_string(), CountryFeatureAvailability {
            lightfriend_can_provide_local_number: FeatureStatus::Unsure,
            user_can_bring_their_own_number: FeatureStatus::Unsure,
            local_number: FeatureStatus::Unsure,
            inbound_calling: FeatureStatus::Unsure,
            outbound_calling: FeatureStatus::Unsure,
            inbound_sms: FeatureStatus::Unsure,
            outbound_sms: FeatureStatus::Unsure,
            mms_messages: FeatureStatus::Unsure,
            notes: Some("Contact rasmus@ahtava.com for the availability in countries that were not mentioned here:)".to_string()),
        });
        map
    });

    let selected_country = use_state(|| "United States".to_string());
    let detected_country_name = use_state(|| None::<String>);

    // Detect user's country on component mount
    {
        let selected_country = selected_country.clone();
        let detected_country_name = detected_country_name.clone();
        let countries = countries.clone();
        
        use_effect_with_deps(
            move |_| {
                wasm_bindgen_futures::spawn_local(async move {
                    let mut opts = RequestInit::new();
                    opts.method("GET");
                    
                    let request = Request::new_with_str_and_init(
                        "https://ipapi.co/json/",
                        &opts,
                    ).unwrap();
                    
                    let window = window().unwrap();
                    if let Ok(resp_value) = JsFuture::from(window.fetch_with_request(&request)).await
                    {
                        let response: Response = resp_value.dyn_into().unwrap();
                        if let Ok(json_value) = JsFuture::from(response.json().unwrap()).await {
                            let json: Value = serde_wasm_bindgen::from_value(json_value).unwrap();
                            if let Some(country) = json.get("country_name").and_then(|c| c.as_str()) {
                                let country_str = country.to_string();
                                detected_country_name.set(Some(country_str.clone()));
                                
                                // Try to find a matching country ignoring case
                                let found_country = ["United States", "United Kingdom", "Finland", "Australia", "Denmark", "United Arab Emirates", "Israel", "Germany"]
                                    .iter()
                                    .find(|&&k| k.to_lowercase() == country_str.to_lowercase());
                                
                                if let Some(&found_country) = found_country {
                                    selected_country.set(found_country.to_string());
                                } else {
                                    // If country is not in our list, show "Other Countries"
                                    selected_country.set("Other Countries".to_string());
                                }
                            }
                        }
                    }
                });
                || ()
            },
            (),
        );
    }

    let on_country_change = {
        let selected_country = selected_country.clone();
        Callback::from(move |e: Event| {
            let target = e.target_dyn_into::<web_sys::HtmlSelectElement>().unwrap();
            selected_country.set(target.value());
        })
    };

    let current_data = countries.get(&*selected_country).cloned();

    html! {
        <div class="supported-countries-page">
            <div class="header">
                <h1>{"Service Availability by Country"}</h1>
                <p>{"Select a country to view feature availability."}</p>
                <select 
                    onchange={on_country_change} 
                    value={(*selected_country).clone()} 
                    class="country-select"
                >
                    {for ["United States", "United Kingdom", "Finland", "Australia", "Denmark", "United Arab Emirates", "Israel", "Germany", "Other Countries"].iter().map(|country| {
                        if country == &"Other Countries" && detected_country_name.is_some() {
                            let detected = detected_country_name.as_ref().unwrap();
                            let is_country_listed = countries.keys().any(|k| k.to_lowercase() == detected.to_lowercase());
                            
                            if !is_country_listed {
                                html! { 
                                    <option value={country.clone()}>
                                        {format!("Other Countries (including {})", detected)}
                                    </option> 
                                }
                            } else {
                                html! { <option value={country.clone()}>{country}</option> }
                            }
                        } else {
                            html! { <option value={country.clone()}>{country}</option> }
                        }
                    })}
                </select>
            </div>

            {
                if let Some(data) = current_data {
                    html! {
                        <table class="feature-table">
                            <thead>
                                <tr>
                                    <th>{"Feature"}</th>
                                    <th>{"Available"}</th>
                                </tr>
                            </thead>
                            <tbody>
                                <tr>
                                    <td>{"Lightfriend can provide local number"}</td>
                                    <td>{match data.lightfriend_can_provide_local_number {
                                        FeatureStatus::Available => "✅",
                                        FeatureStatus::Unavailable => "❌",
                                        FeatureStatus::Unsure => "❓",
                                    }}</td>
                                </tr>
                                <tr>
                                    <td>{"Local number available"}</td>
                                    <td>{match data.local_number {
                                        FeatureStatus::Available => "✅",
                                        FeatureStatus::Unavailable => "❌",
                                        FeatureStatus::Unsure => "❓",
                                    }}</td>
                                </tr>
                                <tr>
                                    <td>{"Inbound Calling"}</td>
                                    <td>{match data.inbound_calling {
                                        FeatureStatus::Available => "✅",
                                        FeatureStatus::Unavailable => "❌",
                                        FeatureStatus::Unsure => "❓",
                                    }}</td>
                                </tr>
                                <tr>
                                    <td>{"Outbound Calling"}</td>
                                    <td>{match data.outbound_calling {
                                        FeatureStatus::Available => "✅",
                                        FeatureStatus::Unavailable => "❌",
                                        FeatureStatus::Unsure => "❓",
                                    }}</td>
                                </tr>
                                <tr>
                                    <td>{"Inbound SMS"}</td>
                                    <td>{match data.inbound_sms {
                                        FeatureStatus::Available => "✅",
                                        FeatureStatus::Unavailable => "❌",
                                        FeatureStatus::Unsure => "❓",
                                    }}</td>
                                </tr>
                                <tr>
                                    <td>{"Outbound SMS"}</td>
                                    <td>{match data.outbound_sms {
                                        FeatureStatus::Available => "✅",
                                        FeatureStatus::Unavailable => "❌",
                                        FeatureStatus::Unsure => "❓",
                                    }}</td>
                                </tr>
                                <tr>
                                    <td>{"Send Photos (MMS)"}</td>
                                    <td>{match data.mms_messages {
                                        FeatureStatus::Available => "✅",
                                        FeatureStatus::Unavailable => "❌",
                                        FeatureStatus::Unsure => "❓",
                                    }}</td>
                                </tr>
                                <tr>
                                    <td>{"Notes"}</td>
                                    <td>{data.notes.unwrap_or("-".to_string())}</td>
                                </tr>
                            </tbody>
                        </table>
                    }
                } else {
                    html! { <p>{"No data available for selected country."}</p> }
                }
            }

            <style>
                {r#"
                    .supported-countries-page {
                        padding: 2rem;
                        background: rgba(26, 26, 26, 0.85);
                        color: white;
                        max-width: 800px;
                        margin: auto;
                        margin-top: 5rem;
                        border-radius: 12px;
                        border: 1px solid rgba(30, 144, 255, 0.2);
                    }

                    .header {
                        text-align: center;
                        margin-bottom: 2rem;
                    }

                    select {
                        padding: 0.5rem;
                        margin-top: 1rem;
                        font-size: 1rem;
                        border-radius: 6px;
                        background-color: #111;
                        color: #fff;
                        border: 1px solid #444;
                    }

                    .feature-table {
                        width: 100%;
                        border-collapse: collapse;
                        margin-top: 2rem;
                    }

                    .feature-table th,
                    .feature-table td {
                        padding: 1rem;
                        text-align: left;
                        border-bottom: 1px solid #444;
                    }

                    .feature-table th {
                        color: #7EB2FF;
                        font-size: 1.1rem;
                    }

                    .feature-table td {
                        font-size: 1rem;
                    }
                "#}
            </style>
        </div>
    }
}

