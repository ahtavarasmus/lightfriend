use reqwest::Client;
use std::env;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::sync::Arc;
use axum::{
    extract::{Json, State},
    http::StatusCode,
};
use crate::AppState; 
use serde_json::{json, Value};

#[derive(Deserialize, Debug)]
pub struct AvailablePhoneNumbersResponse {
    #[serde(default)]
    pub available_phone_numbers: Vec<AvailablePhoneNumber>,
}

#[derive(Deserialize, Debug, Serialize, Clone)]
pub struct AvailablePhoneNumber {
    pub phone_number: String,
    pub friendly_name: String,
    pub address_requirements: String,
    pub capabilities: Capabilities,
    pub iso_country: String,
    #[serde(default)]
    pub locality: Option<String>,
    #[serde(default)]
    pub beta: Option<bool>,
    #[serde(default)]
    pub lata: Option<String>,
    #[serde(default)]
    pub latitude: Option<String>,
    #[serde(default)]
    pub longitude: Option<String>,
    #[serde(default)]
    pub postal_code: Option<String>,
    #[serde(default)]
    pub rate_center: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
}

#[derive(Deserialize, Debug, Serialize, Clone)]
pub struct Capabilities {
    #[serde(default)]
    pub voice: bool,
    #[serde(default, rename = "SMS", alias = "sms")]  // Handles both cases
    pub sms: bool,
    #[serde(default, rename = "MMS", alias = "mms")]  // Handles both cases
    pub mms: bool,
    #[serde(default, rename = "fax", alias = "FAX")]  // Optional, handles potential variants
    pub fax: bool,
}

#[derive(Debug, Serialize)]
pub struct AvailableNumbers {
    pub locals: Vec<AvailablePhoneNumber>,
    pub mobiles: Vec<AvailablePhoneNumber>,
}

#[derive(Deserialize, Serialize)]
pub struct PhoneNumberCountry {
    pub country: String,
    pub iso_country: String,
    pub phone_number_prices: Vec<PhoneNumberPrice>,
    pub price_unit: String,
    pub url: String,
}

#[derive(Deserialize, Serialize)]
pub struct PhoneNumberPrice {
    pub number_type: String,
    pub base_price: String,
    pub current_price: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct MessagingCountry {
    pub country: String,
    pub iso_country: String,
    pub url: String,
    pub price_unit: String,
    pub inbound_sms_prices: Vec<InboundSmsPrice>,
    pub outbound_sms_prices: Vec<OutboundSmsPrice>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct InboundSmsPrice {
    pub number_type: String,
    pub current_price: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct OutboundSmsPrice {
    pub carrier: String,
    pub mcc: String,
    pub mnc: String,
    pub prices: Vec<OutboundPrice>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct OutboundPrice {
    pub number_type: String,
    pub base_price: String,
    pub current_price: String,
}

#[derive(Deserialize, Serialize)]
pub struct Price {
    pub base_price: String,
    pub current_price: String,
}

#[derive(Deserialize, Serialize)]
pub struct VoiceCountry {
    pub country: String,
    pub iso_country: String,
    pub url: String,
    pub inbound_call_prices: Vec<InboundCallPrice>,
    pub outbound_prefix_prices: Vec<OutboundPrefixPrice>,
}

#[derive(Deserialize, Serialize)]
pub struct InboundCallPrice {
    pub number_type: String,
    pub base_price: String,
    pub current_price: String,
}

#[derive(Deserialize, Serialize)]
pub struct OutboundPrefixPrice {
    pub prefixes: Vec<String>,
    pub base_price: String,
    pub current_price: String,
    pub friendly_name: String,
}

#[derive(Serialize)]
pub struct TwilioPrices {
    pub phone_numbers: PhoneNumberCountry,
    pub messaging: MessagingCountry,
    pub voice: VoiceCountry,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct RegulationsResponse {
    pub results: Vec<Regulation>,
    pub meta: Meta,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Meta {
    pub page: i32,
    pub page_size: i32,
    pub first_page_url: String,
    pub previous_page_url: Option<String>,
    pub url: String,
    pub next_page_url: Option<String>,
    pub key: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Regulation {
    pub sid: String,
    pub friendly_name: String,
    pub iso_country: String,
    pub number_type: String,
    pub end_user_type: String,
    pub requirements: Requirements,
    pub url: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Requirements {
    pub end_user: Vec<EndUserRequirement>,
    pub supporting_document: Vec<Vec<SupportingDocumentRequirement>>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct EndUserRequirement {
    pub name: String,
    #[serde(rename = "type")]
    pub req_type: String,
    pub requirement_name: String,
    pub url: String,
    pub fields: Vec<String>,
    pub detailed_fields: Vec<FieldDetail>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SupportingDocumentRequirement {
    pub name: String,
    #[serde(rename = "type")]
    pub req_type: String,
    pub requirement_name: String,
    pub description: String,
    pub accepted_documents: Vec<AcceptedDocument>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct AcceptedDocument {
    pub name: String,
    #[serde(rename = "type")]
    pub doc_type: String,
    pub url: String,
    pub fields: Vec<String>,
    pub detailed_fields: Vec<FieldDetail>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct FieldDetail {
    pub machine_name: String,
    pub friendly_name: String,
    pub description: String,
}

#[derive(Serialize)]
pub struct TwilioRegulations {
    pub local: Vec<Regulation>,
    pub mobile: Vec<Regulation>,
}

#[derive(Deserialize)]
pub struct CountryRequest {
    pub country_code: String,
}

#[derive(Serialize)]
pub struct CountryInfoResponse {
    pub available_numbers: AvailableNumbers,
    pub prices: TwilioPrices,
    pub regulations: TwilioRegulations,
}

pub async fn get_country_info(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<CountryRequest>,
) -> Result<Json<CountryInfoResponse>, (StatusCode, Json<Value>)> {
    println!("Starting get_country_info with country_code: {}", req.country_code);

    let account_sid = match env::var("TWILIO_ACCOUNT_SID") {
        Ok(sid) => {
            println!("Successfully retrieved TWILIO_ACCOUNT_SID");
            sid
        },
        Err(e) => {
            println!("Error retrieving TWILIO_ACCOUNT_SID: {}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Missing TWILIO_ACCOUNT_SID"}))))
        },
    };

    let auth_token = match env::var("TWILIO_AUTH_TOKEN") {
        Ok(token) => {
            println!("Successfully parsed local numbers response");
            token
        },
        Err(e) => {
            println!("Error retrieving TWILIO_AUTH_TOKEN: {}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Missing TWILIO_AUTH_TOKEN"}))))
        },
    };

    let client = Client::new();
    println!("Created new HTTP client");



    // Fetch phone number prices
    println!("Fetching phone number prices for country: {}", req.country_code);
    let phone_url = format!("https://pricing.twilio.com/v1/PhoneNumbers/Countries/{}", req.country_code);
    println!("Phone prices URL: {}", phone_url);

    let phone_send = client
        .get(&phone_url)
        .basic_auth(&account_sid, Some(&auth_token))
        .send()
        .await;

    let phone_send = match phone_send {
        Ok(resp) => {
            println!("Successfully sent request for phone number prices");
            resp
        },
        Err(e) => {
            println!("Failed to send request for phone number prices: {}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to send request for phone number prices: {}", e)}))))
        },
    };

    println!("Parsing phone number prices response");
    let phone_numbers: PhoneNumberCountry = match phone_send.json().await {
        Ok(json) => {
            println!("Successfully parsed phone number prices");
            json
        },
        Err(e) => {
            println!("Failed to parse phone number prices: {}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to parse phone number prices: {}", e)}))))
        },
    };

    let mut type_prices: Vec<(String, f64)> = phone_numbers.phone_number_prices.iter()
        .filter_map(|p| {
            if p.number_type == "local" || p.number_type == "mobile" {
                p.current_price.parse::<f64>().ok().map(|pr| (p.number_type.clone(), pr))
            } else {
                None
            }
        })
        .collect();
    type_prices.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let mut best_number: Option<AvailablePhoneNumber> = None;
    let mut best_type: String = String::new();
    for (num_type, _) in type_prices {
        let url_type = match num_type.as_str() {
            "local" => Some("Local"),
            "mobile" => Some("Mobile"),
            _ => None,
        };
        if let Some(u_type) = url_type {
            let url = format!(
                "https://api.twilio.com/2010-04-01/Accounts/{}/AvailablePhoneNumbers/{}/{}.json",
                account_sid, req.country_code.to_uppercase(), u_type
            );
            let resp = match client
                .get(&url)
                .basic_auth(&account_sid, Some(&auth_token))
                .query(&[("pageSize", "20")])
                .send()
                .await {
                    Ok(r) => r,
                    Err(e) => {
                        println!("Failed to fetch {} numbers: {}", num_type, e);
                        continue;
                    }
                };
            if !resp.status().is_success() {
                let err_text = resp.text().await.unwrap_or_default();
                println!("Twilio API error for {} numbers: {}", num_type, err_text);
                continue;
            }
            let avail_resp = match resp.json::<AvailablePhoneNumbersResponse>().await {
                Ok(json) => json,
                Err(e) => {
                    println!("Failed to parse {} numbers: {}", num_type, e);
                    continue;
                }
            };
            let mut candidates = avail_resp.available_phone_numbers
                .into_iter()
                .filter(|n| n.capabilities.sms || n.capabilities.voice)
                .collect::<Vec<_>>();
            if candidates.is_empty() {
                continue;
            }
            candidates.sort_by_key(|n| {
                -(if n.capabilities.sms && n.capabilities.voice { 3 } else if n.capabilities.sms && n.capabilities.mms { 2 } else if n.capabilities.sms || n.capabilities.voice { 1 } else { 0 })
            });
            best_number = Some(candidates[0].clone());
            best_type = num_type;
            break;
        }
    }
    let locals = if best_type == "local" { best_number.clone().map(|n| vec![n]).unwrap_or_default() } else { vec![] };
    let mobiles = if best_type == "mobile" { best_number.clone().map(|n| vec![n]).unwrap_or_default() } else { vec![] };
    let available_numbers = AvailableNumbers { locals, mobiles };
    println!("Selected best number: type={}, count={}", best_type, if best_number.is_some() { 1 } else { 0 });

    // Fetch messaging prices
    println!("Fetching messaging prices for country: {}", req.country_code);
    let messaging_url = format!("https://pricing.twilio.com/v1/Messaging/Countries/{}", req.country_code);
    println!("Messaging prices URL: {}", messaging_url);

    let messaging_send = client
        .get(&messaging_url)
        .basic_auth(&account_sid, Some(&auth_token))
        .send()
        .await;

    let messaging_send = match messaging_send {
        Ok(resp) => {
            println!("Successfully sent request for messaging prices");
            resp
        },
        Err(e) => {
            println!("Failed to send request for messaging prices: {}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to send request for messaging prices: {}", e)}))))
        },
    };

    println!("Parsing messaging prices response");

    let text = match messaging_send.text().await {
        Ok(t) => t,
        Err(e) => {
            println!("Failed to read messaging response body: {}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to read messaging response body: {}", e)}))));
        },
    };
    let value = match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(v) => v,
        Err(e) => {
            println!("Failed to parse messaging as Value: {}", e);
            println!("Raw messaging response body: {}", text);
            json!({})
        }
    };
    let country = value["country"].as_str().unwrap_or(&req.country_code).to_string();
    let iso_country = value["iso_country"].as_str().unwrap_or(&req.country_code).to_string();
    let url = value["url"].as_str().unwrap_or("").to_string();
    let price_unit = value["price_unit"].as_str().unwrap_or("USD").to_string();
    let inbound_sms_prices = value["inbound_sms_prices"].as_array().unwrap_or(&vec![]).iter().map(|item| {
        let number_type = item["number_type"].as_str().unwrap_or("").to_string();
        let current_price = item["prices"].as_array().and_then(|arr| arr.get(0)).and_then(|p| p["current_price"].as_str()).unwrap_or("0.00").to_string();
        InboundSmsPrice { number_type, current_price }
    }).collect::<Vec<_>>();
    let outbound_sms_prices: Vec<OutboundSmsPrice> = match serde_json::from_value(value["outbound_sms_prices"].clone()) {
        Ok(o) => o,
        Err(e) => {
            println!("Failed to parse outbound_sms_prices: {}", e);
            vec![]
        }
    };
    let messaging = MessagingCountry {
        country,
        iso_country,
        url,
        price_unit,
        inbound_sms_prices,
        outbound_sms_prices,
    };
    println!("Parsed messaging prices with {} inbound and {} outbound", messaging.inbound_sms_prices.len(), messaging.outbound_sms_prices.len());

    // Fetch voice prices
    println!("Fetching voice prices for country: {}", req.country_code);
    let voice_url = format!("https://pricing.twilio.com/v1/Voice/Countries/{}", req.country_code);
    println!("Voice prices URL: {}", voice_url);

    let voice_send = client
        .get(&voice_url)
        .basic_auth(&account_sid, Some(&auth_token))
        .send()
        .await;

    let voice_send = match voice_send {
        Ok(resp) => {
            println!("Successfully sent request for voice prices");
            resp
        },
        Err(e) => {
            println!("Failed to send request for voice prices: {}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to send request for voice prices: {}", e)}))))
        },
    };

    println!("Parsing voice prices response");
    let voice: VoiceCountry = match voice_send.json().await {
        Ok(json) => {
            println!("Successfully parsed voice prices");
            json
        },
        Err(e) => {
            println!("Failed to parse voice prices: {}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to parse voice prices: {}", e)}))))
        },
    };

    let prices = TwilioPrices {
        phone_numbers,
        messaging,
        voice,
    };
    println!("Combined prices data structure created");

    let mut local = vec![];
    let mut mobile = vec![];
    if !best_type.is_empty() {
        let reg_number_type = match best_type.as_str() {
            "local" => "local",
            "mobile" => "mobile",
            _ => "",
        };
        if !reg_number_type.is_empty() {
            println!("Fetching regulations for type: {}", reg_number_type);
            let regs_resp = client
                .get("https://numbers.twilio.com/v2/RegulatoryCompliance/Regulations")
                .basic_auth(&account_sid, Some(&auth_token))
                .query(&[
                    ("IsoCountry", req.country_code.to_uppercase().as_str()),
                    ("NumberType", reg_number_type),
                    ("IncludeConstraints", "true"),
                ])
                .send()
                .await;
            let regs_resp = match regs_resp {
                Ok(resp) => resp,
                Err(e) => {
                    println!("Failed to send request for regulations: {}", e);
                    return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to fetch regulations: {}", e)}))));
                },
            };
            if !regs_resp.status().is_success() {
                let err_text = regs_resp.text().await.unwrap_or_default();
                println!("Twilio API error for regulations: {}", err_text);
            } else {
                let regs_json: RegulationsResponse = match regs_resp.json().await {
                    Ok(json) => json,
                    Err(e) => {
                        println!("Failed to parse regulations: {}", e);
                        RegulationsResponse { results: vec![], meta: Meta { page: 0, page_size: 0, first_page_url: "".to_string(), previous_page_url: None, url: "".to_string(), next_page_url: None, key: "".to_string() } }
                    },
                };
                let regs = regs_json.results;
                if best_type == "local" {
                    local = regs.clone();
                } else if best_type == "mobile" {
                    mobile = regs.clone();
                }
                println!("Retrieved {} regulations for {}", regs.len(), best_type);
            }
        }
    }
    let regulations = TwilioRegulations {
        local: local.clone(),
        mobile: mobile.clone(),
    };
    println!("Combined regulations data: {} local, {} mobile", local.len(), mobile.len());
    println!("Returning successful response");
    Ok(Json(CountryInfoResponse {
        available_numbers,
        prices,
        regulations,
    }))
}
