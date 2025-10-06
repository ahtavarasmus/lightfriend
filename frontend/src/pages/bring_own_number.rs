use yew::prelude::*;
use crate::Route;
use yew_router::prelude::*;
use web_sys::{MouseEvent, window, Event};
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use serde_json::json;
use crate::config;
use serde::Deserialize;
#[derive(Deserialize, Clone, PartialEq)]
pub struct AvailablePhoneNumbersResponse {
    pub available_phone_numbers: Vec<AvailablePhoneNumber>,
}
#[derive(Deserialize, Clone, PartialEq)]
pub struct AvailablePhoneNumber {
    pub phone_number: String,
    pub friendly_name: String,
    pub address_requirements: String,
    pub capabilities: Capabilities,
    pub iso_country: String,
    pub locality: Option<String>,
    pub region: Option<String>,
    pub postal_code: Option<String>,
    pub lata: Option<String>,
    pub rate_center: Option<String>,
    pub latitude: Option<String>,
    pub longitude: Option<String>,
    #[serde(default)]
    pub beta: bool,
}
#[derive(Deserialize, Clone, PartialEq)]
pub struct Capabilities {
    #[serde(default)]
    pub voice: bool,
    #[serde(default, rename = "SMS", alias = "sms")]
    pub sms: bool,
    #[serde(default, rename = "MMS", alias = "mms")]
    pub mms: bool,
}
#[derive(Deserialize, Clone, PartialEq)]
pub struct AvailableNumbers {
    pub locals: Vec<AvailablePhoneNumber>,
    pub mobiles: Vec<AvailablePhoneNumber>,
}
#[derive(Deserialize, Clone, PartialEq)]
pub struct PhoneNumberCountry {
    pub country: String,
    pub iso_country: String,
    pub phone_number_prices: Vec<PhoneNumberPrice>,
    pub price_unit: String,
    pub url: String,
}
#[derive(Deserialize, Clone, PartialEq)]
pub struct PhoneNumberPrice {
    pub number_type: String,
    pub base_price: String,
    pub current_price: String,
}
#[derive(Deserialize, Clone, PartialEq)]
pub struct MessagingCountry {
    pub country: String,
    pub iso_country: String,
    pub url: String,
    pub price_unit: String,
    pub inbound_sms_prices: Vec<InboundSmsPrice>,
    pub outbound_sms_prices: Vec<OutboundSmsPrice>,
}
#[derive(Deserialize, Clone, PartialEq)]
pub struct InboundSmsPrice {
    pub number_type: String,
    pub current_price: String,
}
#[derive(Deserialize, Clone, PartialEq)]
pub struct OutboundSmsPrice {
    pub carrier: String,
    pub mcc: String,
    pub mnc: String,
    pub prices: Vec<OutboundPrice>,
}
#[derive(Deserialize, Clone, PartialEq)]
pub struct OutboundPrice {
    pub number_type: String,
    pub base_price: String,
    pub current_price: String,
}
#[derive(Deserialize, Clone, PartialEq)]
pub struct VoiceCountry {
    pub country: String,
    pub iso_country: String,
    pub url: String,
    pub inbound_call_prices: Vec<InboundCallPrice>,
    pub outbound_prefix_prices: Vec<OutboundPrefixPrice>,
}
#[derive(Deserialize, Clone, PartialEq)]
pub struct InboundCallPrice {
    pub number_type: String,
    pub base_price: String,
    pub current_price: String,
}
#[derive(Deserialize, Clone, PartialEq)]
pub struct OutboundPrefixPrice {
    pub prefixes: Vec<String>,
    pub base_price: String,
    pub current_price: String,
    pub friendly_name: String,
}
#[derive(Deserialize, Clone, PartialEq)]
pub struct TwilioPrices {
    pub phone_numbers: PhoneNumberCountry,
    pub messaging: MessagingCountry,
    pub voice: VoiceCountry,
}
#[derive(Deserialize, Clone, PartialEq)]
pub struct RegulationsResponse {
    pub results: Vec<Regulation>,
    pub meta: Meta,
}
#[derive(Deserialize, Clone, PartialEq)]
pub struct Meta {
    pub page: i32,
    pub page_size: i32,
    pub first_page_url: String,
    pub previous_page_url: Option<String>,
    pub url: String,
    pub next_page_url: Option<String>,
    pub key: String,
}
#[derive(Deserialize, Clone, PartialEq)]
pub struct Regulation {
    pub sid: String,
    pub friendly_name: String,
    pub iso_country: String,
    pub number_type: String,
    pub end_user_type: String,
    pub requirements: Requirements,
    pub url: String,
}
#[derive(Deserialize, Clone, PartialEq)]
pub struct Requirements {
    pub end_user: Vec<EndUserRequirement>,
    pub supporting_document: Vec<Vec<SupportingDocumentRequirement>>,
}
#[derive(Deserialize, Clone, PartialEq)]
pub struct EndUserRequirement {
    pub name: String,
    #[serde(rename = "type")]
    pub req_type: String,
    pub requirement_name: String,
    pub url: String,
    pub fields: Vec<String>,
    pub detailed_fields: Vec<FieldDetail>,
}
#[derive(Deserialize, Clone, PartialEq)]
pub struct SupportingDocumentRequirement {
    pub name: String,
    #[serde(rename = "type")]
    pub req_type: String,
    pub requirement_name: String,
    pub description: String,
    pub accepted_documents: Vec<AcceptedDocument>,
}
#[derive(Deserialize, Clone, PartialEq)]
pub struct AcceptedDocument {
    pub name: String,
    #[serde(rename = "type")]
    pub doc_type: String,
    pub url: String,
    pub fields: Vec<String>,
    pub detailed_fields: Vec<FieldDetail>,
}
#[derive(Deserialize, Clone, PartialEq)]
pub struct FieldDetail {
    pub machine_name: String,
    pub friendly_name: String,
    pub description: String,
}
#[derive(Deserialize, Clone, PartialEq)]
pub struct TwilioRegulations {
    pub local: Vec<Regulation>,
    pub mobile: Vec<Regulation>,
}
#[derive(Deserialize, Clone, PartialEq)]
pub struct CountryInfoResponse {
    pub available_numbers: AvailableNumbers,
    pub prices: TwilioPrices,
    pub regulations: TwilioRegulations,
}
#[derive(Properties, PartialEq)]
pub struct TwilioHostedInstructionsProps {
    #[prop_or_default]
    pub is_logged_in: bool,
    #[prop_or_default]
    pub sub_tier: Option<String>,
    #[prop_or_default]
    pub twilio_phone: Option<String>,
    #[prop_or_default]
    pub twilio_sid: Option<String>,
    #[prop_or_default]
    pub twilio_token: Option<String>,
    #[prop_or_default]
    pub server_url: Option<String>,
    #[prop_or_default]
    pub messaging_service_sid: Option<String>,
    #[prop_or_default]
    pub textbee_api_key: Option<String>,
    #[prop_or_default]
    pub textbee_device_id: Option<String>,
    #[prop_or_default]
    pub message: String,
    #[prop_or_default]
    pub country: Option<String>,
}
#[derive(Properties, PartialEq)]
struct IntroAndCountryProps {
    selected_country: UseStateHandle<String>,
    on_country_change: Callback<Event>,
    is_loading: UseStateHandle<bool>,
    fetch_error: UseStateHandle<Option<String>>,
    country_info: UseStateHandle<Option<CountryInfoResponse>>,
}
#[function_component(IntroAndCountryComponent)]
fn intro_and_country_component(props: &IntroAndCountryProps) -> Html {
    let selected_country = props.selected_country.clone();
    let on_country_change = props.on_country_change.clone();
    let is_loading = props.is_loading.clone();
    let fetch_error = props.fetch_error.clone();
    let country_info = props.country_info.clone();
    html! {
        <>
            <div class="instruction-block overview-block">
                <div class="instruction-content">
                    <h2>{"SMS and Voice Communication Setup"}</h2>
                    <p>{"Lightfriend uses Twilio to send voice calls and text messages. Users outside the US need to use their own Twilio number, because many countries require local address to be able to buy a phone number or even send messages."}</p>
                    <a href="https://youtu.be/WARw4REp584?si=ghmMYGzTKTLcgss_" class="learn-more-link" target="_blank">{"Don't bother to read? Watch me explain this page on youtube ->"}</a>
                </div>
            </div>
            <div class="instruction-block">
                <div class="instruction-content">
                    <h2>{"Twilio Information"}</h2>
                    <p>{"Select a country to view available phone numbers, costs, and regulations."}</p>
                    <div class="country-selector">
                        <label for="country-select">{"Country: "}</label>
                        <select id="country-select" onchange={on_country_change}>
                            <option value="ae" selected={*selected_country == "ae"}>{"AE"}</option>
                            <option value="ar" selected={*selected_country == "ar"}>{"AR"}</option>
                            <option value="at" selected={*selected_country == "at"}>{"AT"}</option>
                            <option value="au" selected={*selected_country == "au"}>{"AU"}</option>
                            <option value="ba" selected={*selected_country == "ba"}>{"BA"}</option>
                            <option value="bb" selected={*selected_country == "bb"}>{"BB"}</option>
                            <option value="bd" selected={*selected_country == "bd"}>{"BD"}</option>
                            <option value="be" selected={*selected_country == "be"}>{"BE"}</option>
                            <option value="bg" selected={*selected_country == "bg"}>{"BG"}</option>
                            <option value="bh" selected={*selected_country == "bh"}>{"BH"}</option>
                            <option value="bj" selected={*selected_country == "bj"}>{"BJ"}</option>
                            <option value="bo" selected={*selected_country == "bo"}>{"BO"}</option>
                            <option value="br" selected={*selected_country == "br"}>{"BR"}</option>
                            <option value="ca" selected={*selected_country == "ca"}>{"CA"}</option>
                            <option value="us" selected={*selected_country == "us"}>{"US"}</option>
                            <option value="ch" selected={*selected_country == "ch"}>{"CH"}</option>
                            <option value="cl" selected={*selected_country == "cl"}>{"CL"}</option>
                            <option value="co" selected={*selected_country == "co"}>{"CO"}</option>
                            <option value="cr" selected={*selected_country == "cr"}>{"CR"}</option>
                            <option value="cy" selected={*selected_country == "cy"}>{"CY"}</option>
                            <option value="cz" selected={*selected_country == "cz"}>{"CZ"}</option>
                            <option value="de" selected={*selected_country == "de"}>{"DE"}</option>
                            <option value="dk" selected={*selected_country == "dk"}>{"DK"}</option>
                            <option value="do" selected={*selected_country == "do"}>{"DO"}</option>
                            <option value="dz" selected={*selected_country == "dz"}>{"DZ"}</option>
                            <option value="ec" selected={*selected_country == "ec"}>{"EC"}</option>
                            <option value="ee" selected={*selected_country == "ee"}>{"EE"}</option>
                            <option value="eg" selected={*selected_country == "eg"}>{"EG"}</option>
                            <option value="es" selected={*selected_country == "es"}>{"ES"}</option>
                            <option value="fi" selected={*selected_country == "fi"}>{"FI"}</option>
                            <option value="fr" selected={*selected_country == "fr"}>{"FR"}</option>
                            <option value="gb" selected={*selected_country == "gb"}>{"GB"}</option>
                            <option value="gd" selected={*selected_country == "gd"}>{"GD"}</option>
                            <option value="gh" selected={*selected_country == "gh"}>{"GH"}</option>
                            <option value="gr" selected={*selected_country == "gr"}>{"GR"}</option>
                            <option value="gt" selected={*selected_country == "gt"}>{"GT"}</option>
                            <option value="hk" selected={*selected_country == "hk"}>{"HK"}</option>
                            <option value="hr" selected={*selected_country == "hr"}>{"HR"}</option>
                            <option value="hu" selected={*selected_country == "hu"}>{"HU"}</option>
                            <option value="id" selected={*selected_country == "id"}>{"ID"}</option>
                            <option value="ie" selected={*selected_country == "ie"}>{"IE"}</option>
                            <option value="im" selected={*selected_country == "im"}>{"IM"}</option>
                            <option value="in" selected={*selected_country == "in"}>{"IN"}</option>
                            <option value="is" selected={*selected_country == "is"}>{"IS"}</option>
                            <option value="it" selected={*selected_country == "it"}>{"IT"}</option>
                            <option value="jm" selected={*selected_country == "jm"}>{"JM"}</option>
                            <option value="jo" selected={*selected_country == "jo"}>{"JO"}</option>
                            <option value="jp" selected={*selected_country == "jp"}>{"JP"}</option>
                            <option value="ke" selected={*selected_country == "ke"}>{"KE"}</option>
                            <option value="kr" selected={*selected_country == "kr"}>{"KR"}</option>
                            <option value="lk" selected={*selected_country == "lk"}>{"LK"}</option>
                            <option value="lt" selected={*selected_country == "lt"}>{"LT"}</option>
                            <option value="lu" selected={*selected_country == "lu"}>{"LU"}</option>
                            <option value="lv" selected={*selected_country == "lv"}>{"LV"}</option>
                            <option value="md" selected={*selected_country == "md"}>{"MD"}</option>
                            <option value="mg" selected={*selected_country == "mg"}>{"MG"}</option>
                            <option value="ml" selected={*selected_country == "ml"}>{"ML"}</option>
                            <option value="mo" selected={*selected_country == "mo"}>{"MO"}</option>
                            <option value="mu" selected={*selected_country == "mu"}>{"MU"}</option>
                            <option value="mx" selected={*selected_country == "mx"}>{"MX"}</option>
                            <option value="my" selected={*selected_country == "my"}>{"MY"}</option>
                            <option value="na" selected={*selected_country == "na"}>{"NA"}</option>
                            <option value="ng" selected={*selected_country == "ng"}>{"NG"}</option>
                            <option value="ni" selected={*selected_country == "ni"}>{"NI"}</option>
                            <option value="nl" selected={*selected_country == "nl"}>{"NL"}</option>
                            <option value="no" selected={*selected_country == "no"}>{"NO"}</option>
                            <option value="nz" selected={*selected_country == "nz"}>{"NZ"}</option>
                            <option value="pa" selected={*selected_country == "pa"}>{"PA"}</option>
                            <option value="ph" selected={*selected_country == "ph"}>{"PH"}</option>
                            <option value="pl" selected={*selected_country == "pl"}>{"PL"}</option>
                            <option value="pt" selected={*selected_country == "pt"}>{"PT"}</option>
                            <option value="py" selected={*selected_country == "py"}>{"PY"}</option>
                            <option value="qa" selected={*selected_country == "qa"}>{"QA"}</option>
                            <option value="ro" selected={*selected_country == "ro"}>{"RO"}</option>
                            <option value="sa" selected={*selected_country == "sa"}>{"SA"}</option>
                            <option value="se" selected={*selected_country == "se"}>{"SE"}</option>
                            <option value="sg" selected={*selected_country == "sg"}>{"SG"}</option>
                            <option value="si" selected={*selected_country == "si"}>{"SI"}</option>
                            <option value="sk" selected={*selected_country == "sk"}>{"SK"}</option>
                            <option value="sv" selected={*selected_country == "sv"}>{"SV"}</option>
                            <option value="th" selected={*selected_country == "th"}>{"TH"}</option>
                            <option value="tn" selected={*selected_country == "tn"}>{"TN"}</option>
                            <option value="tr" selected={*selected_country == "tr"}>{"TR"}</option>
                            <option value="tw" selected={*selected_country == "tw"}>{"TW"}</option>
                            <option value="ug" selected={*selected_country == "ug"}>{"UG"}</option>
                            <option value="uy" selected={*selected_country == "uy"}>{"UY"}</option>
                            <option value="ve" selected={*selected_country == "ve"}>{"VE"}</option>
                            <option value="vn" selected={*selected_country == "vn"}>{"VN"}</option>
                            <option value="za" selected={*selected_country == "za"}>{"ZA"}</option>
                        </select>
                    </div>
                    { if *is_loading {
                        html! { <p>{"Loading..."}</p> }
                    } else if let Some(err) = &*fetch_error {
                        html! {
                            <div>
                                <p class="error">{err}</p>
                                if !selected_country.is_empty() {
                                    <p>
                                        {"More info from these links or email me rasmus@ahtava.com"}
                                        <a href={format!("https://www.twilio.com/en-us/sms/pricing/{}", *selected_country)} target="_blank">{" Pricing"}</a>
                                        {" and "}
                                        <a href={format!("https://www.twilio.com/en-us/guidelines/{}/regulatory", *selected_country)} target="_blank">{"Regulations"}</a>
                                        {"."}
                                    </p>
                                }
                            </div>
                        }
                    } else if let Some(info) = (*country_info).clone() {
                        let locals = info.available_numbers.locals;
                        let mobiles = info.available_numbers.mobiles;
                        let price_unit = info.prices.phone_numbers.price_unit.clone();
                        let mut local_regs = info.regulations.local.clone();
                        local_regs = local_regs.into_iter().filter(|reg| reg.end_user_type == "individual").collect();
                        let mut mobile_regs = info.regulations.mobile.clone();
                        mobile_regs = mobile_regs.into_iter().filter(|reg| reg.end_user_type == "individual").collect();
                        html! {
                            <div class="country-info">
                                <h3>{"Available Numbers"}</h3>
                                {
                                    if locals.is_empty() && mobiles.is_empty() {
                                        html! { <p>{"No available numbers found for this country."}</p> }
                                    } else {
                                        html! {
                                            <>
                                            <table class="country-table">
                                                <thead>
                                                    <tr>
                                                        <th>{"Number Type"}</th>
                                                        <th>{"Number"}</th>
                                                        <th>{"Address Requirements"}</th>
                                                        <th>{"Capabilities"}</th>
                                                        <th>{format!("Monthly Price ({})", price_unit)}</th>
                                                        <th>{"Inbound SMS Price (per 160 chars)"}</th>
                                                        <th>{"Outbound SMS Price (per 160 chars)"}</th>
                                                        <th>{"Inbound Call Price (per minute)"}</th>
                                                        <th>{"Outbound Call Price (per minute)"}</th>
                                                    </tr>
                                                </thead>
                                                <tbody>
                                                    {
                                                        if !locals.is_empty() {
                                                            let num = locals[0].clone();
                                                            let num_type = "local".to_string();
                                                            let monthly_price = info.prices.phone_numbers.phone_number_prices.iter()
                                                                .find(|p| p.number_type == num_type)
                                                                .map(|p| p.current_price.clone())
                                                                .unwrap_or("N/A".to_string());
                                                            let inbound_sms_price = info.prices.messaging.inbound_sms_prices.iter()
                                                                .find(|inp| inp.number_type == num_type)
                                                                .map(|inp| inp.current_price.clone())
                                                                .unwrap_or("N/A".to_string());
                                                            let outbound_sms_min_price = info.prices.messaging.outbound_sms_prices.iter()
                                                                .flat_map(|out| &out.prices)
                                                                .filter(|pr| pr.number_type == num_type)
                                                                .map(|pr| pr.current_price.parse::<f64>().unwrap_or(f64::MAX))
                                                                .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                                                                .map(|p| p.to_string())
                                                                .unwrap_or("N/A".to_string());
                                                            let inbound_call_price = info.prices.voice.inbound_call_prices.iter()
                                                                .find(|inp| inp.number_type == num_type)
                                                                .map(|inp| inp.current_price.clone())
                                                                .unwrap_or("N/A".to_string());
                                                            let outbound_call_price = info.prices.voice.outbound_prefix_prices.iter()
                                                                .map(|outp| outp.current_price.parse::<f64>().unwrap_or(f64::MAX))
                                                                .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                                                                .map(|p| p.to_string())
                                                                .unwrap_or("N/A".to_string());
                                                            let has_inbound_sms = inbound_sms_price != "N/A" && num.capabilities.sms;
                                                            let has_outbound_sms = outbound_sms_min_price != "N/A" && num.capabilities.sms;
                                                            let mut caps: Vec<String> = Vec::new();
                                                            if num.capabilities.voice { caps.push("Voice".to_string()); }
                                                            if has_inbound_sms && has_outbound_sms {
                                                                caps.push("SMS".to_string());
                                                            } else {
                                                                if has_inbound_sms { caps.push("Inbound SMS".to_string()); }
                                                                if has_outbound_sms { caps.push("Outbound SMS".to_string()); }
                                                            }
                                                            if num.capabilities.mms { caps.push("MMS".to_string()); }
                                                            let caps_str = if caps.is_empty() { "None".to_string() } else { caps.join(", ") };
                                                            html! {
                                                                <tr>
                                                                    <td>{"Local"}</td>
                                                                    <td>{ format!("{} ({})", num.friendly_name, num.phone_number) }</td>
                                                                    <td>{ num.address_requirements }</td>
                                                                    <td>{ caps_str }</td>
                                                                    <td>{ monthly_price }</td>
                                                                    <td>{ inbound_sms_price }</td>
                                                                    <td>{ outbound_sms_min_price }</td>
                                                                    <td>{ inbound_call_price }</td>
                                                                    <td>{ outbound_call_price }</td>
                                                                </tr>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }
                                                    {
                                                        if !mobiles.is_empty() {
                                                            let num = mobiles[0].clone();
                                                            let num_type = "mobile".to_string();
                                                            let monthly_price = info.prices.phone_numbers.phone_number_prices.iter()
                                                                .find(|p| p.number_type == num_type)
                                                                .map(|p| p.current_price.clone())
                                                                .unwrap_or("N/A".to_string());
                                                            let inbound_sms_price = info.prices.messaging.inbound_sms_prices.iter()
                                                                .find(|inp| inp.number_type == num_type)
                                                                .map(|inp| inp.current_price.clone())
                                                                .unwrap_or("N/A".to_string());
                                                            let outbound_sms_min_price = info.prices.messaging.outbound_sms_prices.iter()
                                                                .flat_map(|out| &out.prices)
                                                                .filter(|pr| pr.number_type == num_type)
                                                                .map(|pr| pr.current_price.parse::<f64>().unwrap_or(f64::MAX))
                                                                .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                                                                .map(|p| p.to_string())
                                                                .unwrap_or("N/A".to_string());
                                                            let inbound_call_price = info.prices.voice.inbound_call_prices.iter()
                                                                .find(|inp| inp.number_type == num_type)
                                                                .map(|inp| inp.current_price.clone())
                                                                .unwrap_or("N/A".to_string());
                                                            let outbound_call_price = info.prices.voice.outbound_prefix_prices.iter()
                                                                .map(|outp| outp.current_price.parse::<f64>().unwrap_or(f64::MAX))
                                                                .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                                                                .map(|p| p.to_string())
                                                                .unwrap_or("N/A".to_string());
                                                            let has_inbound_sms = inbound_sms_price != "N/A" && num.capabilities.sms;
                                                            let has_outbound_sms = outbound_sms_min_price != "N/A" && num.capabilities.sms;
                                                            let mut caps: Vec<String> = Vec::new();
                                                            if num.capabilities.voice { caps.push("Voice".to_string()); }
                                                            if has_inbound_sms && has_outbound_sms {
                                                                caps.push("SMS".to_string());
                                                            } else {
                                                                if has_inbound_sms { caps.push("Inbound SMS".to_string()); }
                                                                if has_outbound_sms { caps.push("Outbound SMS".to_string()); }
                                                            }
                                                            if num.capabilities.mms { caps.push("MMS".to_string()); }
                                                            let caps_str = if caps.is_empty() { "None".to_string() } else { caps.join(", ") };
                                                            html! {
                                                                <tr>
                                                                    <td>{"Mobile"}</td>
                                                                    <td>{ format!("{} ({})", num.friendly_name, num.phone_number) }</td>
                                                                    <td>{ num.address_requirements }</td>
                                                                    <td>{ caps_str }</td>
                                                                    <td>{ monthly_price }</td>
                                                                    <td>{ inbound_sms_price }</td>
                                                                    <td>{ outbound_sms_min_price }</td>
                                                                    <td>{ inbound_call_price }</td>
                                                                    <td>{ outbound_call_price }</td>
                                                                </tr>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }
                                                </tbody>
                                            </table>
                                            <p>{"Average usage per month falls around 50-200 SMS queries per month with average cost of 1.5x the outbound SMS cost. Normal voice calling is usually based on the inbound costs, though you can make the agent call you about important notifications which is when the outbound call price applies."}</p>
                                            </>
                                        }
                                    }
                                }
                                <h3>{"Regulations"}</h3>
                                {
                                    if !locals.is_empty() {
                                        html! {
                                            <div>
                                                <h4>{"Local"}</h4>
                                                {
                                                    if local_regs.is_empty() {
                                                        html! { <p>{"No specific regulations found"}</p> }
                                                    } else {
                                                        html! {
                                                            { for local_regs.iter().map(|reg| {
                                                                html! {
                                                                    <div>
                                                                        <p>{ reg.friendly_name.clone() } { " (" } { reg.end_user_type.clone() } { ")" }</p>
                                                                        <ul>
                                                                            { for reg.requirements.end_user.iter().map(|eu| {
                                                                                let fields_joined = eu.fields.iter().fold(String::new(), |acc, f| if acc.is_empty() { f.clone() } else { acc + ", " + f });
                                                                                html! {
                                                                                    <li>
                                                                                        { eu.name.clone() } { " (" } { eu.req_type.clone() } { "): " } { fields_joined }
                                                                                    </li>
                                                                                }
                                                                            }) }
                                                                        </ul>
                                                                        <p>{"Supporting Documents"}</p>
                                                                        { for reg.requirements.supporting_document.iter().flatten().map(|doc| {
                                                                            let accepted_joined = doc.accepted_documents.iter().fold(String::new(), |acc, ad| if acc.is_empty() { ad.name.clone() } else { acc + ", " + &ad.name });
                                                                            html! {
                                                                                <div>
                                                                                    { doc.name.clone() } { ": " } { doc.description.clone() }
                                                                                    { " Accepted: " } { accepted_joined }
                                                                                </div>
                                                                            }
                                                                        }) }
                                                                    </div>
                                                                }
                                                            }) }
                                                        }
                                                    }
                                                }
                                            </div>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
                                {
                                    if !mobiles.is_empty() {
                                        html! {
                                            <div>
                                                <h4>{"Mobile"}</h4>
                                                {
                                                    if mobile_regs.is_empty() {
                                                        html! { <p>{"No specific regulations found"}</p> }
                                                    } else {
                                                        html! {
                                                            { for mobile_regs.iter().map(|reg| {
                                                                html! {
                                                                    <div>
                                                                        <p>{ reg.friendly_name.clone() } { " (" } { reg.end_user_type.clone() } { ")" }</p>
                                                                        <ul>
                                                                            { for reg.requirements.end_user.iter().map(|eu| {
                                                                                let fields_joined = eu.fields.iter().fold(String::new(), |acc, f| if acc.is_empty() { f.clone() } else { acc + ", " + f });
                                                                                html! {
                                                                                    <li>
                                                                                        { eu.name.clone() } { " (" } { eu.req_type.clone() } { "): " } { fields_joined }
                                                                                    </li>
                                                                                }
                                                                            }) }
                                                                        </ul>
                                                                        <p>{"Supporting Documents"}</p>
                                                                        { for reg.requirements.supporting_document.iter().flatten().map(|doc| {
                                                                            let accepted_joined = doc.accepted_documents.iter().fold(String::new(), |acc, ad| if acc.is_empty() { ad.name.clone() } else { acc + ", " + &ad.name });
                                                                            html! {
                                                                                <div>
                                                                                    { doc.name.clone() } { ": " } { doc.description.clone() }
                                                                                    { " Accepted: " } { accepted_joined }
                                                                                </div>
                                                                            }
                                                                        }) }
                                                                    </div>
                                                                }
                                                            }) }
                                                        }
                                                    }
                                                }
                                            </div>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
                                if !selected_country.is_empty() {
                                    <p>
                                        {"Check these more at"}
                                        <a href={format!("https://www.twilio.com/en-us/sms/pricing/{}", *selected_country)} target="_blank">{"Pricing"}</a>
                                        {" and "}
                                        <a href={format!("https://www.twilio.com/en-us/guidelines/{}/regulatory", *selected_country)} target="_blank">{"Regulations"}</a>
                                        {"."}
                                    </p>
                                }
                            </div>
                        }
                    } else {
                        html! { <p>{"Select a country to view information"}</p> }
                    } }
                </div>
            </div>
            <style>
                {r#"
                .instruction-block {
                    display: flex;
                    align-items: center;
                    gap: 4rem;
                    margin-bottom: 4rem;
                    background: rgba(26, 26, 26, 0.85);
                    backdrop-filter: blur(10px);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 12px;
                    padding: 4rem;
                    transition: all 0.3s ease;
                }
                .instruction-block:hover {
                    border-color: rgba(30, 144, 255, 0.3);
                }
                .instruction-content {
                    flex: 1;
                    order: 1;
                }
                .instruction-content h2 {
                    font-size: 2rem;
                    margin-bottom: 1.5rem;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                }
                .learn-more-link {
                    color: #1E90FF;
                    text-decoration: none;
                    font-size: 1.1rem;
                    font-weight: 500;
                    transition: color 0.3s ease;
                }
                .learn-more-link:hover {
                    color: #7EB2FF;
                    text-decoration: underline;
                }
                .country-selector {
                    display: flex;
                    align-items: center;
                    gap: 1rem;
                    margin-bottom: 2rem;
                }
                .country-selector label {
                    color: #7EB2FF;
                    font-size: 1.1rem;
                }
                .country-selector select {
                    padding: 0.75rem;
                    background: rgba(26, 26, 26, 0.5);
                    border: 1px solid rgba(30, 144, 255, 0.3);
                    color: #fff;
                    border-radius: 6px;
                    font-size: 1rem;
                    cursor: pointer;
                    transition: all 0.3s ease;
                }
                .country-selector select:focus {
                    outline: none;
                    border-color: rgba(30, 144, 255, 0.8);
                }
                .country-info h3, .country-info h4, .country-info h5 {
                    color: #7EB2FF;
                }
                .country-info ul {
                    list-style-type: disc;
                    padding-left: 20px;
                }
                .country-info li {
                    color: #999;
                    margin-bottom: 0.5rem;
                }
                .country-table {
                    width: 100%;
                    border-collapse: collapse;
                    margin-bottom: 2rem;
                }
                .country-table th, .country-table td {
                    border: 1px solid rgba(255, 255, 255, 0.1);
                    padding: 0.75rem;
                    text-align: left;
                    color: #999;
                }
                .country-table th {
                    background: rgba(30, 144, 255, 0.1);
                    color: #fff;
                }
                .country-table td {
                    color: #fff;
                }
                .error {
                    color: #f44336;
                }
                "#}
            </style>
        </>
    }
}
#[derive(Properties, PartialEq)]
struct InstructionsProps {
    can_edit: bool,
    phone_number: UseStateHandle<String>,
    on_phone_change: Callback<Event>,
    on_save_phone: Callback<MouseEvent>,
    phone_save_status: UseStateHandle<Option<Result<(), String>>>,
    account_sid: UseStateHandle<String>,
    on_sid_change: Callback<Event>,
    auth_token: UseStateHandle<String>,
    on_token_change: Callback<Event>,
    server_url: UseStateHandle<String>,
    on_server_url_change: Callback<Event>,
    messaging_service_sid: UseStateHandle<String>,
    on_messaging_sid_change: Callback<Event>,
    selected_country: UseStateHandle<String>,
    on_save_creds: Callback<MouseEvent>,
    creds_save_status: UseStateHandle<Option<Result<(), String>>>,
    open_modal: Callback<String>,
    is_phone_valid: bool,
    is_sid_valid: bool,
    is_token_valid: bool,
}
#[function_component(InstructionsComponent)]
fn instructions_component(props: &InstructionsProps) -> Html {
    let open_modal = props.open_modal.clone();
    let phone_number = props.phone_number.clone();
    let on_phone_change = props.on_phone_change.clone();
    let on_save_phone = props.on_save_phone.clone();
    let phone_save_status = props.phone_save_status.clone();
    let account_sid = props.account_sid.clone();
    let on_sid_change = props.on_sid_change.clone();
    let auth_token = props.auth_token.clone();
    let on_token_change = props.on_token_change.clone();
    let server_url = props.server_url.clone();
    let on_server_url_change = props.on_server_url_change.clone();
    let messaging_service_sid = props.messaging_service_sid.clone();
    let on_messaging_sid_change = props.on_messaging_sid_change.clone();
    let selected_country = props.selected_country.clone();
    let on_save_creds = props.on_save_creds.clone();
    let creds_save_status = props.creds_save_status.clone();
    let can_edit = props.can_edit;
    let is_phone_valid = props.is_phone_valid;
    let is_sid_valid = props.is_sid_valid;
    let is_token_valid = props.is_token_valid;
    let is_server_url_valid = {
        let val = &*server_url;
        !val.is_empty() && (val.starts_with("http://") || val.starts_with("https://")) && !val.starts_with("...")
    };
    let is_messaging_sid_valid = {
        if *selected_country == "us" {
            let val = &*messaging_service_sid;
            !val.is_empty() && val.len() == 34 && val.starts_with("MG") && val[2..].chars().all(|c| c.is_ascii_hexdigit()) && !val.starts_with("...")
        } else {
            true
        }
    };
    html! {
        <>
            <div class="instruction-block">
                <div class="instruction-content">
                    <h2>{"Twilio Sign up and Add Funds"}</h2>
                    <ul>
                        <li>{"Go to Twilio's website (twilio.com) and click 'Sign up'"}</li>
                        <li>{"Complete the registration process with your email and other required information"}</li>
                        <li>{"Once registered, you'll need to add funds to your account:"}</li>
                        <li>{"1. Click on 'Admin' in the top right"}</li>
                        <li>{"2. Select 'Account billing' from the dropdown"}</li>
                        <li>{"3. Click 'Add funds' on the new billing page that opens up and input desired amount (minimum usually $20)"}</li>
                        <li>{"After adding funds, your account will be ready to purchase a phone number"}</li>
                    </ul>
                </div>
                <div class="instruction-image">
                    <img
                        src="/assets/billing-twilio.png"
                        alt="Navigating to Twilio Billing Page"
                        loading="lazy"
                        onclick={let open_modal = open_modal.clone(); let src = "/assets/billing-twilio.png".to_string();
                            Callback::from(move |_| open_modal.emit(src.clone()))}
                        style="cursor: pointer;"
                    />
                </div>
            </div>
            <div class="instruction-block">
                <div class="instruction-content">
                    <h2>{"Twilio Buy a Phone Number"}</h2>
                    <ul>
                        <li>{"1. On the Twilio Dashboard, click on the 'Phone Numbers' button in the left sidebar when 'Develop' is selected above."}</li>
                        <li>{"2. Click the 'Buy a number' button under the new sub menu"}</li>
                        <li>{"3. Use the country search box to select your desired country"}</li>
                        <li>{"4. (Optional) Use advanced search options to find specific number types"}</li>
                        <li>{"5. Check the capabilities column to ensure the number supports your needs (Voice, SMS, MMS, etc.)"}</li>
                        <li>{"6. Click the 'Buy' button next to your chosen number and follow the steps"}</li>
                    </ul>
                    <div class="input-field">
                        <label for="phone-number">{"Your Twilio Phone Number:"}</label>
                        <div class="input-with-button">
                            <input
                                type="text"
                                id="phone-number"
                                placeholder="+1234567890"
                                value={(*phone_number).clone()}
                                onchange={on_phone_change.clone()}
                                disabled={!can_edit}
                            />
                            <button
                                class={classes!("save-button", if !is_phone_valid || !can_edit { "invalid" } else { "" })}
                                onclick={on_save_phone.clone()}
                                disabled={!can_edit || !is_phone_valid}
                            >
                                {"Save"}
                            </button>
                            {
                                match &*phone_save_status {
                                    Some(Ok(_)) => html! {
                                        <span class="save-status success">{"✓ Saved"}</span>
                                    },
                                    Some(Err(err)) => html! {
                                        <span class="save-status error">{format!("Error: {}", err)}</span>
                                    },
                                    None => html! {}
                                }
                            }
                        </div>
                    </div>
                </div>
                <div class="instruction-image">
                    <img
                        src="/assets/number-twilio.png"
                        alt="Buy Twilio Phone Number Image"
                        loading="lazy"
                        onclick={let open_modal = open_modal.clone(); let src = "/assets/number-twilio.png".to_string();
                            Callback::from(move |_| open_modal.emit(src.clone()))}
                        style="cursor: pointer;"
                    />
                </div>
            </div>
            <div class="instruction-block">
                <div class="instruction-content">
                    <h2>{"Twilio Finding Credentials"}</h2>
                    <ul>
                        <li>{"1. Click on the 'Account Dashboard' in the left sidebar"}</li>
                        <li>{"2. Find and copy your 'Account SID' from the dashboard"}</li>
                        <li>{"3. Reveal and copy your 'Auth Token' from the dashboard"}</li>
                    </ul>
                    <div class="input-field">
                        <label for="account-sid">{"Your Account SID:"}</label>
                        <div class="input-with-button">
                            <input
                                type="text"
                                id="account-sid"
                                placeholder="ACxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
                                value={(*account_sid).clone()}
                                onchange={on_sid_change.clone()}
                                disabled={!can_edit}
                            />
                        </div>
                    </div>
                    <div class="input-field">
                        <label for="auth-token">{"Your Auth Token:"}</label>
                        <div class="input-with-button">
                            <input
                                type="text"
                                id="auth-token"
                                placeholder="your_auth_token_here"
                                value={(*auth_token).clone()}
                                onchange={on_token_change.clone()}
                                disabled={!can_edit}
                            />
                        </div>
                    </div>
                    <div class="input-field">
                        <label for="server-url">{"Server URL:"}</label>
                        <div class="input-with-button">
                            <input
                                type="url"
                                id="server-url"
                                placeholder="https://your-server.com"
                                value={(*server_url).clone()}
                                onchange={on_server_url_change.clone()}
                                disabled={!can_edit}
                            />
                        </div>
                    </div>
                    { if *selected_country == "us" {
                        html! {
                            <div class="input-field">
                                <label for="messaging-sid">{"Twilio Messaging Service SID (required for US due to A2P):"}</label>
                                <div class="input-with-button">
                                    <input
                                        type="text"
                                        id="messaging-sid"
                                        placeholder="MGxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
                                        value={(*messaging_service_sid).clone()}
                                        onchange={on_messaging_sid_change.clone()}
                                        disabled={!can_edit}
                                    />
                                </div>
                            </div>
                        }
                    } else {
                        html! {}
                    }}
                    <button
                        class={classes!("save-button", if !(is_sid_valid && is_token_valid && is_server_url_valid && is_messaging_sid_valid) || !can_edit { "invalid" } else { "" })}
                        onclick={on_save_creds.clone()}
                        disabled={!can_edit || !(is_sid_valid && is_token_valid && is_server_url_valid && is_messaging_sid_valid)}
                    >
                        {"Save"}
                    </button>
                    {
                        match &*creds_save_status {
                            Some(Ok(_)) => html! {
                                <span class="save-status success">{"✓ Saved"}</span>
                            },
                            Some(Err(err)) => html! {
                                <span class="save-status error">{format!("Error: {}", err)}</span>
                            },
                            None => html! {}
                        }
                    }
                </div>
                <div class="instruction-image">
                    <img
                        src="/assets/creds-twilio.png"
                        alt="Twilio Credentials Dashboard"
                        loading="lazy"
                        onclick={let open_modal = open_modal.clone(); let src = "/assets/creds-twilio.png".to_string();
                            Callback::from(move |_| open_modal.emit(src.clone()))}
                        style="cursor: pointer;"
                    />
                </div>
            </div>
            <style>
                {r#"
                .instruction-block {
                    display: flex;
                    align-items: center;
                    gap: 4rem;
                    margin-bottom: 4rem;
                    background: rgba(26, 26, 26, 0.85);
                    backdrop-filter: blur(10px);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 12px;
                    padding: 4rem;
                    transition: all 0.3s ease;
                }
                .instruction-block:hover {
                    border-color: rgba(30, 144, 255, 0.3);
                }
                .instruction-content {
                    flex: 1;
                    order: 1;
                }
                .instruction-content h2 {
                    font-size: 2rem;
                    margin-bottom: 1.5rem;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                }
                .instruction-content ul {
                    list-style: none;
                    padding: 0;
                }
                .instruction-content li {
                    color: #999;
                    padding: 0.75rem 0;
                    padding-left: 1.5rem;
                    position: relative;
                    line-height: 1.6;
                }
                .instruction-content li::before {
                    content: '•';
                    position: absolute;
                    left: 0.5rem;
                    color: #1E90FF;
                }
                .instruction-content ul ul li::before {
                    content: '◦';
                }
                .instruction-image {
                    flex: 1.2;
                    display: flex;
                    justify-content: center;
                    align-items: center;
                }
                .instruction-image img {
                    max-width: 110%;
                    height: auto;
                    border-radius: 12px;
                    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
                    transition: transform 0.3s ease;
                }
                .instruction-image img:hover {
                    transform: scale(1.02);
                }
                .input-field {
                    margin-top: 1.5rem;
                }
                .input-field label {
                    display: block;
                    margin-bottom: 0.5rem;
                    color: #7EB2FF;
                }
                .input-field input {
                    width: 100%;
                    padding: 0.75rem;
                    border: 1px solid rgba(30, 144, 255, 0.3);
                    border-radius: 6px;
                    background: rgba(26, 26, 26, 0.5);
                    color: #fff;
                    font-size: 1rem;
                    transition: all 0.3s ease;
                }
                .input-field input:focus {
                    outline: none;
                    border-color: rgba(30, 144, 255, 0.8);
                    box-shadow: 0 0 0 2px rgba(30, 144, 255, 0.2);
                }
                .input-field input::placeholder {
                    color: rgba(255, 255, 255, 0.3);
                }
                .input-with-button {
                    display: flex;
                    gap: 0.5rem;
                }
                .input-with-button input {
                    flex: 1;
                }
                .save-button {
                    padding: 0.75rem 1.5rem;
                    background: #1E90FF;
                    color: white;
                    border: none;
                    border-radius: 6px;
                    cursor: pointer;
                    font-size: 1rem;
                    transition: all 0.3s ease;
                }
                .save-button:hover {
                    background: #1976D2;
                }
                .save-button:active {
                    transform: translateY(1px);
                }
                .save-button.invalid {
                    background: #cccccc;
                    color: #666666;
                    cursor: not-allowed;
                }
                .save-button.invalid:hover {
                    background: #cccccc;
                }
                .save-status {
                    margin-left: 1rem;
                    padding: 0.5rem 1rem;
                    border-radius: 4px;
                    font-size: 0.9rem;
                }
                .save-status.success {
                    color: #4CAF50;
                    background: rgba(76, 175, 80, 0.1);
                }
                .save-status.error {
                    color: #f44336;
                    background: rgba(244, 67, 54, 0.1);
                }
                "#}
            </style>
        </>
    }
}
#[derive(Properties, PartialEq)]
struct ModalProps {
    visible: bool,
    selected_image: String,
    on_close: Callback<MouseEvent>,
}
#[function_component(ModalComponent)]
fn modal_component(props: &ModalProps) -> Html {
    let on_close = props.on_close.clone();
    html! {
        <>
            {
                if props.visible {
                    html! {
                        <div class="modal-overlay" onclick={on_close.clone()}>
                            <div class="modal-content" onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                                <img src={props.selected_image.clone()} alt="Large preview" />
                                <button class="modal-close" onclick={on_close}>{"×"}</button>
                            </div>
                        </div>
                    }
                } else {
                    html! {}
                }
            }
            <style>
                {r#"
                .modal-overlay {
                    position: fixed;
                    top: 0;
                    left: 0;
                    width: 100%;
                    height: 100%;
                    background: rgba(0, 0, 0, 0.85);
                    display: flex;
                    justify-content: center;
                    align-items: center;
                    z-index: 1000;
                    backdrop-filter: blur(5px);
                }
                .modal-content {
                    position: relative;
                    max-width: 90%;
                    max-height: 90vh;
                    border-radius: 12px;
                    overflow: hidden;
                    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
                }
                .modal-content img {
                    display: block;
                    max-width: 100%;
                    max-height: 90vh;
                    object-fit: contain;
                }
                .modal-close {
                    position: absolute;
                    top: 10px;
                    right: 10px;
                    width: 40px;
                    height: 40px;
                    border-radius: 50%;
                    background: rgba(0, 0, 0, 0.5);
                    border: 2px solid rgba(255, 255,255, 0.5);
                    color: white;
                    font-size: 24px;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    cursor: pointer;
                    transition: all 0.3s ease;
                }
                .modal-close:hover {
                    background: rgba(0, 0, 0, 0.8);
                    border-color: white;
                }
                "#}
            </style>
        </>
    }
}
#[function_component(TwilioHostedInstructions)]
pub fn twilio_hosted_instructions(props: &TwilioHostedInstructionsProps) -> Html {
    let modal_visible = use_state(|| false);
    let selected_image = use_state(|| String::new());
    let selected_country = use_state(|| "".to_string());
    {
        let selected_country = selected_country.clone();
        let country = props.country.clone();
        use_effect_with_deps(
            move |_| {
                selected_country.set(country.unwrap_or("".to_string()).to_lowercase());
                || ()
            },
            props.country.clone(),
        );
    }
    {
        let selected_country = selected_country.clone();
        let country = props.country.clone();
        use_effect_with_deps(
            move |_| {
                if country.is_none() && selected_country.is_empty() {
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Ok(response) = Request::get("https://ipapi.co/json/").send().await {
                            if let Ok(json) = response.json::<serde_json::Value>().await {
                                if let Some(code) = json.get("country_code").and_then(|c| c.as_str()) {
                                    selected_country.set(code.to_lowercase());
                                }
                            }
                        }
                    });
                }
                || ()
            },
            (),
        );
    }
    let phone_number = use_state(|| props.twilio_phone.clone().unwrap_or_default());
    let account_sid = use_state(|| props.twilio_sid.clone().unwrap_or_default());
    let auth_token = use_state(|| props.twilio_token.clone().unwrap_or_default());
    let server_url = use_state(|| props.server_url.clone().unwrap_or_default());
    let messaging_service_sid = use_state(|| props.messaging_service_sid.clone().unwrap_or_default());
    let textbee_api_key = use_state(|| props.textbee_api_key.clone().unwrap_or_default());
    let textbee_device_id = use_state(|| props.textbee_device_id.clone().unwrap_or_default());
    let phone_save_status = use_state(|| None::<Result<(), String>>);
    let creds_save_status = use_state(|| None::<Result<(), String>>);
    let textbee_save_status = use_state(|| None::<Result<(), String>>);
    {
        let phone_number = phone_number.clone();
        let account_sid = account_sid.clone();
        let auth_token = auth_token.clone();
        let server_url = server_url.clone();
        let messaging_service_sid = messaging_service_sid.clone();
        let textbee_api_key = textbee_api_key.clone();
        let textbee_device_id = textbee_device_id.clone();
        use_effect_with_deps(
            move |(new_phone, new_sid, new_token, new_server_url, new_messaging_sid, new_textbee_key, new_textbee_id)| {
                if let Some(phone) = new_phone {
                    if phone != &*phone_number {
                        phone_number.set(phone.clone());
                    }
                }
                if let Some(sid) = new_sid {
                    if sid != &*account_sid {
                        account_sid.set(sid.clone());
                    }
                }
                if let Some(token) = new_token {
                    if token != &*auth_token {
                        auth_token.set(token.clone());
                    }
                }
                if let Some(url) = new_server_url {
                    if url != &*server_url {
                        server_url.set(url.clone());
                    }
                }
                if let Some(sid) = new_messaging_sid {
                    if sid != &*messaging_service_sid {
                        messaging_service_sid.set(sid.clone());
                    }
                }
                if let Some(key) = new_textbee_key {
                    if key != &*textbee_api_key {
                        textbee_api_key.set(key.clone());
                    }
                }
                if let Some(id) = new_textbee_id {
                    if id != &*textbee_device_id {
                        textbee_device_id.set(id.clone());
                    }
                }
                || {}
            },
            (
                props.twilio_phone.clone(),
                props.twilio_sid.clone(),
                props.twilio_token.clone(),
                props.server_url.clone(),
                props.messaging_service_sid.clone(),
                props.textbee_api_key.clone(),
                props.textbee_device_id.clone(),
            ),
        );
    }
    let on_phone_change = {
        let phone_number = phone_number.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            phone_number.set(input.value());
        })
    };
    let on_sid_change = {
        let account_sid = account_sid.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            account_sid.set(input.value());
        })
    };
    let on_token_change = {
        let auth_token = auth_token.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            auth_token.set(input.value());
        })
    };
    let on_server_url_change = {
        let server_url = server_url.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            server_url.set(input.value());
        })
    };
    let on_messaging_sid_change = {
        let messaging_service_sid = messaging_service_sid.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            messaging_service_sid.set(input.value());
        })
    };
    let on_textbee_key_change = {
        let textbee_api_key = textbee_api_key.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            textbee_api_key.set(input.value());
        })
    };
    let on_textbee_id_change = {
        let textbee_device_id = textbee_device_id.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            textbee_device_id.set(input.value());
        })
    };
    let on_country_change = {
        let selected_country = selected_country.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlSelectElement = e.target_unchecked_into();
            selected_country.set(input.value());
        })
    };
    let on_save_phone = {
        let phone_number = phone_number.clone();
        let phone_save_status = phone_save_status.clone();
        Callback::from(move |_: MouseEvent| {
            let phone_number = phone_number.clone();
            let phone_save_status = phone_save_status.clone();
            let val = (*phone_number).clone();
            if val.is_empty() || !val.starts_with('+') || val.len() < 10 || !val[1..].chars().all(|c| c.is_ascii_digit()) || val.starts_with("...") {
                phone_save_status.set(Some(Err("Invalid phone number format".to_string())));
                return;
            }
            phone_save_status.set(None);
            spawn_local(async move {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|storage| storage.get_item("token").ok())
                    .flatten()
                {
                    let result = Request::post(&format!("{}/api/profile/twilio-phone", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .json(&json!({
                            "twilio_phone": *phone_number
                        }))
                        .unwrap()
                        .send()
                        .await;
                    match result {
                        Ok(response) => {
                            if response.status() == 401 {
                                if let Some(window) = window() {
                                    if let Ok(Some(storage)) = window.local_storage() {
                                        let _ = storage.remove_item("token");
                                    }
                                }
                                phone_save_status.set(Some(Err("Session expired. Please log in again.".to_string())));
                            } else if response.ok() {
                                phone_save_status.set(Some(Ok(())));
                            } else {
                                phone_save_status.set(Some(Err("Failed to save Twilio phone".to_string())));
                            }
                        }
                        Err(_) => {
                            phone_save_status.set(Some(Err("Network error occurred".to_string())));
                        }
                    }
                } else {
                    phone_save_status.set(Some(Err("Please log in to save Twilio phone".to_string())));
                }
            });
        })
    };
    let on_save_creds = {
        let account_sid = account_sid.clone();
        let auth_token = auth_token.clone();
        let server_url = server_url.clone();
        let messaging_service_sid = messaging_service_sid.clone();
        let selected_country = selected_country.clone();
        let creds_save_status = creds_save_status.clone();
        Callback::from(move |_: MouseEvent| {
            let account_sid = account_sid.clone();
            let auth_token = auth_token.clone();
            let server_url = server_url.clone();
            let messaging_service_sid = messaging_service_sid.clone();
            let selected_country = selected_country.clone();
            let creds_save_status = creds_save_status.clone();
            let sid_val = (*account_sid).clone();
            if sid_val.len() != 34 || !sid_val.starts_with("AC") || !sid_val[2..].chars().all(|c| c.is_ascii_hexdigit()) || sid_val.starts_with("...") {
                creds_save_status.set(Some(Err("Invalid Account SID format".to_string())));
                return;
            }
            let token_val = (*auth_token).clone();
            if token_val.len() != 32 || !token_val.chars().all(|c| c.is_ascii_hexdigit()) || token_val.starts_with("...") {
                creds_save_status.set(Some(Err("Invalid Auth Token format".to_string())));
                return;
            }
            let server_url_val = (*server_url).clone();
            if server_url_val.is_empty() || ! (server_url_val.starts_with("http://") || server_url_val.starts_with("https://")) || server_url_val.starts_with("...") {
                creds_save_status.set(Some(Err("Invalid Server URL format".to_string())));
                return;
            }
            let country = (*selected_country).clone();
            if country == "us" {
                let msg_sid_val = (*messaging_service_sid).clone();
                if msg_sid_val.len() != 34 || !msg_sid_val.starts_with("MG") || !msg_sid_val[2..].chars().all(|c| c.is_ascii_hexdigit()) || msg_sid_val.starts_with("...") {
                    creds_save_status.set(Some(Err("Invalid Messaging Service SID format".to_string())));
                    return;
                }
            }
            creds_save_status.set(None);
            spawn_local(async move {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|storage| storage.get_item("token").ok())
                    .flatten()
                {
                    let result = Request::post(&format!("{}/api/profile/twilio-creds", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .json(&json!({
                            "account_sid": *account_sid,
                            "auth_token": *auth_token,
                            "server_url": *server_url,
                            "messaging_service_sid": if *selected_country == "us" { Some((*messaging_service_sid).clone()) } else { None }
                        }))
                        .unwrap()
                        .send()
                        .await;
                    match result {
                        Ok(response) => {
                            if response.status() == 401 {
                                if let Some(window) = window() {
                                    if let Ok(Some(storage)) = window.local_storage() {
                                        let _ = storage.remove_item("token");
                                    }
                                }
                                creds_save_status.set(Some(Err("Session expired. Please log in again.".to_string())));
                            } else if response.ok() {
                                creds_save_status.set(Some(Ok(())));
                            } else {
                                creds_save_status.set(Some(Err("Failed to save Twilio credentials".to_string())));
                            }
                        }
                        Err(_) => {
                            creds_save_status.set(Some(Err("Network error occurred".to_string())));
                        }
                    }
                } else {
                    creds_save_status.set(Some(Err("Please log in to save Twilio credentials".to_string())));
                }
            });
        })
    };
    let on_save_textbee = {
        let textbee_api_key = textbee_api_key.clone();
        let textbee_device_id = textbee_device_id.clone();
        let textbee_save_status = textbee_save_status.clone();
        Callback::from(move |_: MouseEvent| {
            let textbee_api_key = textbee_api_key.clone();
            let textbee_device_id = textbee_device_id.clone();
            let textbee_save_status = textbee_save_status.clone();
            let key_val = (*textbee_api_key).clone();
            if key_val.is_empty() || key_val.starts_with("...") {
                textbee_save_status.set(Some(Err("Invalid API Key".to_string())));
                return;
            }
            let id_val = (*textbee_device_id).clone();
            if id_val.is_empty() || id_val.starts_with("...") {
                textbee_save_status.set(Some(Err("Invalid Device ID".to_string())));
                return;
            }
            textbee_save_status.set(None);
            spawn_local(async move {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|storage| storage.get_item("token").ok())
                    .flatten()
                {
                    let result = Request::post(&format!("{}/api/profile/textbee-creds", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .json(&json!({
                            "textbee_api_key": *textbee_api_key,
                            "textbee_device_id": *textbee_device_id
                        }))
                        .unwrap()
                        .send()
                        .await;
                    match result {
                        Ok(response) => {
                            if response.status() == 401 {
                                if let Some(window) = window() {
                                    if let Ok(Some(storage)) = window.local_storage() {
                                        let _ = storage.remove_item("token");
                                    }
                                }
                                textbee_save_status.set(Some(Err("Session expired. Please log in again.".to_string())));
                            } else if response.ok() {
                                textbee_save_status.set(Some(Ok(())));
                            } else {
                                textbee_save_status.set(Some(Err("Failed to save TextBee credentials".to_string())));
                            }
                        }
                        Err(_) => {
                            textbee_save_status.set(Some(Err("Network error occurred".to_string())));
                        }
                    }
                } else {
                    textbee_save_status.set(Some(Err("Please log in to save TextBee credentials".to_string())));
                }
            });
        })
    };
    let close_modal = {
        let modal_visible = modal_visible.clone();
        Callback::from(move |_: MouseEvent| {
            modal_visible.set(false);
        })
    };
    let open_modal = {
        let modal_visible = modal_visible.clone();
        let selected_image = selected_image.clone();
        Callback::from(move |src: String| {
            selected_image.set(src);
            modal_visible.set(true);
        })
    };
    let can_edit = props.is_logged_in && props.sub_tier.as_deref() == Some("tier 2");
    let is_phone_valid = {
        let val = &*phone_number;
        !val.is_empty() && val.starts_with('+') && val.len() >= 10 && val[1..].chars().all(|c| c.is_ascii_digit()) && !val.starts_with("...")
    };
    let is_sid_valid = {
        let val = &*account_sid;
        val.len() == 34 && val.starts_with("AC") && val[2..].chars().all(|c| c.is_ascii_hexdigit()) && !val.starts_with("...")
    };
    let is_token_valid = {
        let val = &*auth_token;
        val.len() == 32 && val.chars().all(|c| c.is_ascii_hexdigit()) && !val.starts_with("...")
    };
    let is_textbee_key_valid = {
        let val = &*textbee_api_key;
        !val.is_empty() && !val.starts_with("...")
    };
    let is_textbee_id_valid = {
        let val = &*textbee_device_id;
        !val.is_empty() && !val.starts_with("...")
    };
    let country_info = use_state(|| None::<CountryInfoResponse>);
    let fetch_error = use_state(|| None::<String>);
    let is_loading = use_state(|| false);
    {
        let selected_country = selected_country.clone();
        let country_info = country_info.clone();
        let fetch_error = fetch_error.clone();
        let is_loading = is_loading.clone();
        use_effect_with_deps(
            move |country| {
                let country = (*country).clone();
                if !country.is_empty() {
                    is_loading.set(true);
                    fetch_error.set(None);
                    country_info.set(None);
                    spawn_local(async move {
                        let request_body = json!({"country_code": country.to_uppercase()});
                        let response = Request::post(&format!("{}/api/country-info", config::get_backend_url()))
                            .json(&request_body)
                            .unwrap()
                            .send()
                            .await;
                        match response {
                            Ok(resp) if resp.ok() => {
                                match resp.json::<CountryInfoResponse>().await {
                                    Ok(data) => {
                                        country_info.set(Some(data));
                                        fetch_error.set(None);
                                    }
                                    Err(e) => fetch_error.set(Some(e.to_string())),
                                }
                            }
                            _ => fetch_error.set(Some("Failed to fetch country information".to_string())),
                        }
                        is_loading.set(false);
                    });
                }
                || ()
            },
            selected_country,
        );
    }
    html! {
        <div class="instructions-page">
            <div class="instructions-background"></div>
            <section class="instructions-section">
                { if !props.message.is_empty() {
                    html! {
                        <div class="applicable-message">
                            { props.message.clone() }
                        </div>
                    }
                } else {
                    html! {}
                } }
                <IntroAndCountryComponent
                    selected_country={selected_country.clone()}
                    on_country_change={on_country_change.clone()}
                    is_loading={is_loading.clone()}
                    fetch_error={fetch_error.clone()}
                    country_info={country_info.clone()}
                />
                <div class="instruction-block">
                    <div class="instruction-content">
                        <h2>{"Alternative: TextBee for SMS Messaging"}</h2>
                        <p>{"If you have a spare Android phone (version 7.0+) with a secondary phone number lying around, connect it to Lightfriend via TextBee. This lets your AI send and receive texts using your existing phone plan, supporting up to 300 messages per month on the free tier at no extra cost beyond your carrier's standard SMS charges."}</p>
                        <p>{"Note: TextBee is for texting only. If you need phone calls, set up Twilio in addition."}</p>
                        <p>{"Note: TextBee does not support sending images or other media."}</p>
                        <p>{"TextBee also offers a Pro plan ($6.99/month currently) for higher limits (up to 5,000 messages/month) and additional features like multi-device support."}</p>
                        <h3>{"Setup Steps"}</h3>
                        <ul>
                            <li>{"Register for a free account at "}<a href="https://textbee.dev" target="_blank" style="color: #7EB2FF; text-decoration: underline;">{"textbee.dev"}</a>{" using email/password or Google."}</li>
                            <li>{"Download and install the TextBee app on your Android phone from "}<a href="https://dl.textbee.dev" target="_blank" style="color: #7EB2FF; text-decoration: underline;">{"dl.textbee.dev"}</a>{"."}</li>
                            <li>{"Grant SMS permissions in the app."}</li>
                            <li>{"Link the device:"}
                                <ul>
                                    <li>{"Recommended: In the dashboard, click 'Register Device', scan the QR code with the app."}</li>
                                    <li>{"Alternative: Generate an API key in the dashboard, enter it in the app."}</li>
                                </ul>
                            </li>
                            <li>{"Once linked, note your Device ID from the devices list in the dashboard."}</li>
                            <li>{"Generate or use your API Key from the dashboard."}</li>
                        </ul>
                        {
                            if props.is_logged_in && props.sub_tier.as_deref() == Some("tier 2") {
                                html! {
                                    <>
                                        <div class="input-field">
                                            <label for="textbee-device-id">{"Your TextBee Device ID:"}</label>
                                            <div class="input-with-button">
                                                <input
                                                    type="text"
                                                    id="textbee-device-id"
                                                    placeholder="your_device_id_here"
                                                    value={(*textbee_device_id).clone()}
                                                    onchange={on_textbee_id_change.clone()}
                                                />
                                            </div>
                                        </div>
                                        <div class="input-field">
                                            <label for="textbee-api-key">{"Your TextBee API Key:"}</label>
                                            <div class="input-with-button">
                                                <input
                                                    type="text"
                                                    id="textbee-api-key"
                                                    placeholder="your_api_key_here"
                                                    value={(*textbee_api_key).clone()}
                                                    onchange={on_textbee_key_change.clone()}
                                                />
                                            </div>
                                        </div>
                                        <button
                                            class={classes!("save-button", if !(is_textbee_key_valid && is_textbee_id_valid) { "invalid" } else { "" })}
                                            onclick={on_save_textbee.clone()}
                                        >
                                            {"Save TextBee Credentials"}
                                        </button>
                                        {
                                            match &*textbee_save_status {
                                                Some(Ok(_)) => html! {
                                                    <span class="save-status success">{"✓ Saved"}</span>
                                                },
                                                Some(Err(err)) => html! {
                                                    <span class="save-status error">{format!("Error: {}", err)}</span>
                                                },
                                                None => html! {}
                                            }
                                        }
                                    </>
                                }
                            } else {
                                html! {}
                            }
                        }
                    </div>
                </div>
                <InstructionsComponent
                    can_edit={can_edit}
                    phone_number={phone_number.clone()}
                    on_phone_change={on_phone_change.clone()}
                    on_save_phone={on_save_phone.clone()}
                    phone_save_status={phone_save_status.clone()}
                    account_sid={account_sid.clone()}
                    on_sid_change={on_sid_change.clone()}
                    auth_token={auth_token.clone()}
                    on_token_change={on_token_change.clone()}
                    server_url={server_url.clone()}
                    on_server_url_change={on_server_url_change.clone()}
                    messaging_service_sid={messaging_service_sid.clone()}
                    on_messaging_sid_change={on_messaging_sid_change.clone()}
                    selected_country={selected_country.clone()}
                    on_save_creds={on_save_creds.clone()}
                    creds_save_status={creds_save_status.clone()}
                    open_modal={open_modal.clone()}
                    is_phone_valid={is_phone_valid}
                    is_sid_valid={is_sid_valid}
                    is_token_valid={is_token_valid}
                />
                <div class="back-home-container">
                    <Link<Route> to={Route::Home} classes="back-home-button">
                        {"Back to Home"}
                    </Link<Route>>
                </div>
            </section>
            <ModalComponent
                visible={*modal_visible}
                selected_image={(*selected_image).clone()}
                on_close={close_modal.clone()}
            />
            <style>
                {r#"
                .instructions-page {
                    padding-top: 74px;
                    min-height: 100vh;
                    color: #ffffff;
                    position: relative;
                    background: transparent;
                }
                .instructions-background {
                    position: fixed;
                    top: 0;
                    left: 0;
                    width: 100%;
                    height: 100vh;
                    background-image: url('/assets/bicycle_field.webp');
                    background-size: cover;
                    background-position: center;
                    background-repeat: no-repeat;
                    opacity: 1;
                    z-index: -2;
                    pointer-events: none;
                }
                .instructions-background::after {
                    content: '';
                    position: absolute;
                    bottom: 0;
                    left: 0;
                    width: 100%;
                    height: 50%;
                    background: linear-gradient(
                        to bottom,
                        rgba(26, 26, 26, 0) 0%,
                        rgba(26, 26, 26, 1) 100%
                    );
                }
                .instructions-section {
                    max-width: 1200px;
                    margin: 0 auto;
                    padding: 2rem;
                }
                .applicable-message {
                    color: #ffcc00;
                    font-size: 1.2rem;
                    margin-bottom: 2rem;
                    text-align: center;
                    padding: 1rem;
                    background: rgba(255, 204, 0, 0.1);
                    border: 1px solid rgba(255, 204, 0, 0.3);
                    border-radius: 6px;
                }
                .back-home-container {
                    text-align: center;
                    margin-top: 2rem;
                    margin-bottom: 2rem;
                }
                .back-home-button {
                    padding: 0.75rem 1.5rem;
                    background: #1E90FF;
                    color: white;
                    border: none;
                    border-radius: 6px;
                    cursor: pointer;
                    font-size: 1rem;
                    text-decoration: none;
                    display: inline-block;
                    transition: all 0.3s ease;
                }
                .back-home-button:hover {
                    background: #1976D2;
                }
                .back-home-button:active {
                    transform: translateY(1px);
                }
                @media (max-width: 968px) {
                    .instruction-block {
                        flex-direction: column;
                        gap: 2rem;
                    }
                    .instruction-content {
                        order: 1;
                    }
                    .instruction-image {
                        order: 2;
                    }
                    .instructions-section {
                        padding: 1rem;
                    }
                }
                "#}
            </style>
        </div>
    }
}
