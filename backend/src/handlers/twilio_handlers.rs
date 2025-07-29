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

#[derive(Deserialize, Debug, Serialize)]
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
    pub latitude: Option<f64>,
    #[serde(default)]
    pub longitude: Option<f64>,
    #[serde(default)]
    pub postal_code: Option<String>,
    #[serde(default)]
    pub rate_center: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct Capabilities {
    #[serde(default)]
    pub voice: bool,
    #[serde(default)]
    pub sms: bool,
    #[serde(default)]
    pub mms: bool,
    #[serde(default)]
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

#[derive(Deserialize, Serialize)]
pub struct MessagingCountry {
    pub country: String,
    pub iso_country: String,
    pub url: String,
    pub mcc: Option<String>,
    pub mnc: Option<String>,
    pub inbound_sms_prices: Vec<InboundSmsPrice>,
    pub outbound_sms_prices: Vec<OutboundSmsPrice>,
}

#[derive(Deserialize, Serialize)]
pub struct InboundSmsPrice {
    pub carrier: Option<String>,
    pub mcc: Option<String>,
    pub mnc: Option<String>,
    pub number_type: String,
    pub prices: Vec<Price>,
}

#[derive(Deserialize, Serialize)]
pub struct OutboundSmsPrice {
    pub carrier: String,
    pub mcc: String,
    pub mnc: String,
    pub prices: Vec<OutboundPrice>,
}

#[derive(Deserialize, Serialize)]
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

#[derive(Deserialize, Serialize, Debug)]
pub struct Regulation {
    pub sid: String,
    pub friendly_name: String,
    pub iso_country: String,
    pub number_type: String,
    pub end_user_type: String,
    pub requirements: Requirements,
    pub url: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Requirements {
    pub end_user: Vec<EndUserRequirement>,
    pub supporting_document: Vec<Vec<SupportingDocumentRequirement>>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct EndUserRequirement {
    pub name: String,
    #[serde(rename = "type")]
    pub req_type: String,
    pub requirement_name: String,
    pub url: String,
    pub fields: Vec<String>,
    pub detailed_fields: Vec<FieldDetail>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct SupportingDocumentRequirement {
    pub name: String,
    #[serde(rename = "type")]
    pub req_type: String,
    pub requirement_name: String,
    pub description: String,
    pub accepted_documents: Vec<AcceptedDocument>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct AcceptedDocument {
    pub name: String,
    #[serde(rename = "type")]
    pub doc_type: String,
    pub url: String,
    pub fields: Vec<String>,
    pub detailed_fields: Vec<FieldDetail>,
}

#[derive(Deserialize, Serialize, Debug)]
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

    // Fetch local numbers
    println!("Fetching local numbers for country: {}", req.country_code);
    let local_url = format!(
        "https://api.twilio.com/2010-04-01/Accounts/{}/AvailablePhoneNumbers/{}/Local.json",
        account_sid, req.country_code
    );
    println!("Local numbers URL: {}", local_url);
    
    let local_resp = client
        .get(&local_url)
        .basic_auth(&account_sid, Some(&auth_token))
        .query(&[
            ("smsEnabled", "true"),
            ("voiceEnabled", "true"),
            ("pageSize", "5"),
        ])
        .send()
        .await;

    let local_resp = match local_resp {
        Ok(resp) => {
            println!("Successfully sent request for local numbers");
            resp
        },
        Err(e) => {
            println!("Failed to send request for local numbers: {}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to send request for local numbers: {}", e)}))))
        },
    };

    let locals = if !local_resp.status().is_success() {
        let err_text = match local_resp.text().await {
            Ok(text) => text,
            Err(e) => format!("Failed to read error body: {}", e),
        };
        println!("Twilio API error for local numbers: {}", err_text);
        Vec::new()
    } else {
        println!("Parsing local numbers response: {:#?}", local_resp);
        let local_json: AvailablePhoneNumbersResponse = match local_resp.json().await {
            Ok(json) => {
                println!("Successfully parsed local numbers response");
                json
            },
            Err(e) => {
                println!("Failed to parse local numbers: {}", e);
                return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to parse local numbers: {}", e)}))))
            },
        };
        local_json.available_phone_numbers
    };
    println!("Retrieved {} local numbers", locals.len());

    // Fetch mobile numbers
    println!("Fetching mobile numbers for country: {}", req.country_code);
    let mobile_url = format!(
        "https://api.twilio.com/2010-04-01/Accounts/{}/AvailablePhoneNumbers/{}/Mobile.json",
        account_sid, req.country_code
    );
    println!("Mobile numbers URL: {}", mobile_url);

    let mobile_resp = client
        .get(&mobile_url)
        .basic_auth(&account_sid, Some(&auth_token))
        .query(&[
            ("smsEnabled", "true"),
            ("voiceEnabled", "true"),
            ("pageSize", "5"),
        ])
        .send()
        .await;

    let mobile_resp = match mobile_resp {
        Ok(resp) => {
            println!("Successfully sent request for mobile numbers");
            resp
        },
        Err(e) => {
            println!("Failed to send request for mobile numbers: {}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to send request for mobile numbers: {}", e)}))))
        },
    };

    let mobiles = if !mobile_resp.status().is_success() {
        let err_text = match mobile_resp.text().await {
            Ok(text) => text,
            Err(e) => format!("Failed to read error body: {}", e),
        };
        println!("Twilio API error for mobile numbers: {}", err_text);
        Vec::new()
    } else {
        println!("Parsing mobile numbers response: {:#?}", mobile_resp);
        let mobile_json: AvailablePhoneNumbersResponse = match mobile_resp.json().await {
            Ok(json) => {
                println!("Successfully parsed mobile numbers response");
                json
            },
            Err(e) => {
                println!("Failed to parse mobile numbers: {}", e);
                return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to parse mobile numbers: {}", e)}))))
            },
        };
        mobile_json.available_phone_numbers
    };
    println!("Retrieved {} mobile numbers", mobiles.len());

    let available_numbers = AvailableNumbers { locals, mobiles };
    println!("Combined available numbers: {} local, {} mobile", available_numbers.locals.len(), available_numbers.mobiles.len());

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
    let messaging: MessagingCountry = match messaging_send.json().await {
        Ok(json) => {
            println!("Successfully parsed messaging prices");
            json
        },
        Err(e) => {
            println!("Failed to parse messaging prices: {}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to parse messaging prices: {}", e)}))))
        },
    };

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

    let mut local_regs = Vec::new();
    if !available_numbers.locals.is_empty() {
        println!("Fetching local regulations for country: {}", req.country_code);
        let local_regs_send = client
            .get("https://numbers.twilio.com/v2/RegulatoryCompliance/Regulations")
            .basic_auth(&account_sid, Some(&auth_token))
            .query(&[
                ("IsoCountry", req.country_code.as_str()),
                ("NumberType", "local"),
                ("IncludeConstraints", "true"),
            ])
            .send()
            .await;

        let local_regs_send = match local_regs_send {
            Ok(resp) => {
                println!("Successfully sent request for local regulations");
                resp
            },
            Err(e) => {
                println!("Failed to send request for local regulations: {}", e);
                return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to send request for local regulations: {}", e)}))))
            },
        };

        println!("Parsing local regulations response");
        let local_regs_json: RegulationsResponse = match local_regs_send.json().await {
            Ok(json) => {
                println!("Successfully parsed local regulations");
                json
            },
            Err(e) => {
                println!("Failed to parse local regulations: {}", e);
                return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to parse local regulations: {}", e)}))))
            },
        };

        local_regs = local_regs_json.results;
        println!("Retrieved {} local regulations", local_regs.len());
    } else {
        println!("No local numbers available, skipping local regulations");
    }

    let mut mobile_regs = Vec::new();
    if !available_numbers.mobiles.is_empty() {
        println!("Fetching mobile regulations for country: {}", req.country_code);
        let mobile_regs_send = client
            .get("https://numbers.twilio.com/v2/RegulatoryCompliance/Regulations")
            .basic_auth(&account_sid, Some(&auth_token))
            .query(&[
                ("IsoCountry", req.country_code.as_str()),
                ("NumberType", "mobile"),
                ("IncludeConstraints", "true"),
            ])
            .send()
            .await;

        let mobile_regs_send = match mobile_regs_send {
            Ok(resp) => {
                println!("Successfully sent request for mobile regulations");
                resp
            },
            Err(e) => {
                println!("Failed to send request for mobile regulations: {}", e);
                return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to send request for mobile regulations: {}", e)}))))
            },
        };

        println!("Parsing mobile regulations response");
        let mobile_regs_json: RegulationsResponse = match mobile_regs_send.json().await {
            Ok(json) => {
                println!("Successfully parsed mobile regulations");
                json
            },
            Err(e) => {
                println!("Failed to parse mobile regulations: {}", e);
                return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to parse mobile regulations: {}", e)}))))
            },
        };

        mobile_regs = mobile_regs_json.results;
        println!("Retrieved {} mobile regulations", mobile_regs.len());
    } else {
        println!("No mobile numbers available, skipping mobile regulations");
    }

    let regulations = TwilioRegulations {
        local: local_regs,
        mobile: mobile_regs,
    };
    println!("Combined regulations data structure created");

    println!("Returning successful response");
    Ok(Json(CountryInfoResponse {
        available_numbers,
        prices,
        regulations,
    }))
}
