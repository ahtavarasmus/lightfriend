use reqwest::Client;
use std::env;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use axum::{
    extract::{Json, State},
    http::StatusCode,
};
use crate::AppState;
use serde_json::{json, Value};
use diesel::prelude::*;
use crate::schema::message_status_log;
use crate::utils::email::send_sms_failure_admin_email;
use crate::utils::country::get_country_code_from_phone;
use std::time::{SystemTime, UNIX_EPOCH};

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
            println!("Successfully retrieved TWILIO_AUTH_TOKEN");
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

    println!("Parsing phone number prices response: {:#?}", phone_send);
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

    // Fetch available numbers for local
    let mut local_number: Option<AvailablePhoneNumber> = None;
    let local_url = format!(
        "https://api.twilio.com/2010-04-01/Accounts/{}/AvailablePhoneNumbers/{}/Local.json",
        account_sid, req.country_code.to_uppercase()
    );
    let local_resp = client
        .get(&local_url)
        .basic_auth(&account_sid, Some(&auth_token))
        .query(&[("pageSize", "20")])
        .send()
        .await;
    match local_resp {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<AvailablePhoneNumbersResponse>().await {
                Ok(avail_resp) => {
                    let mut candidates = avail_resp.available_phone_numbers
                        .into_iter()
                        .filter(|n| n.capabilities.sms || n.capabilities.voice)
                        .collect::<Vec<_>>();
                    if !candidates.is_empty() {
                        candidates.sort_by_key(|n| {
                            -(if n.capabilities.sms && n.capabilities.voice { 3 } else if n.capabilities.sms && n.capabilities.mms { 2 } else if n.capabilities.sms || n.capabilities.voice { 1 } else { 0 })
                        });
                        local_number = Some(candidates[0].clone());
                    }
                }
                Err(e) => println!("Failed to parse local numbers: {}", e),
            }
        }
        Ok(resp) => {
            let err_text = resp.text().await.unwrap_or_default();
            println!("Twilio API error for local numbers: {}", err_text);
        }
        Err(e) => println!("Failed to fetch local numbers: {}", e),
    }

    // Fetch available numbers for mobile
    let mut mobile_number: Option<AvailablePhoneNumber> = None;
    let mobile_url = format!(
        "https://api.twilio.com/2010-04-01/Accounts/{}/AvailablePhoneNumbers/{}/Mobile.json",
        account_sid, req.country_code.to_uppercase()
    );
    let mobile_resp = client
        .get(&mobile_url)
        .basic_auth(&account_sid, Some(&auth_token))
        .query(&[("pageSize", "20")])
        .send()
        .await;
    match mobile_resp {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<AvailablePhoneNumbersResponse>().await {
                Ok(avail_resp) => {
                    let mut candidates = avail_resp.available_phone_numbers
                        .into_iter()
                        .filter(|n| n.capabilities.sms || n.capabilities.voice)
                        .collect::<Vec<_>>();
                    if !candidates.is_empty() {
                        candidates.sort_by_key(|n| {
                            -(if n.capabilities.sms && n.capabilities.voice { 3 } else if n.capabilities.sms && n.capabilities.mms { 2 } else if n.capabilities.sms || n.capabilities.voice { 1 } else { 0 })
                        });
                        mobile_number = Some(candidates[0].clone());
                    }
                }
                Err(e) => println!("Failed to parse mobile numbers: {}", e),
            }
        }
        Ok(resp) => {
            let err_text = resp.text().await.unwrap_or_default();
            println!("Twilio API error for mobile numbers: {}", err_text);
        }
        Err(e) => println!("Failed to fetch mobile numbers: {}", e),
    }

    let available_numbers = AvailableNumbers {
        locals: local_number.clone().map(|n| vec![n]).unwrap_or_default(),
        mobiles: mobile_number.clone().map(|n| vec![n]).unwrap_or_default(),
    };
    println!("Selected numbers: locals={}, mobiles={}", available_numbers.locals.len(), available_numbers.mobiles.len());

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
        let current_price = item["prices"].as_array().and_then(|arr| arr.first()).and_then(|p| p["current_price"].as_str()).unwrap_or("0.00").to_string();
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

    // Fetch regulations for local
    let mut local_regs = vec![];
    println!("Fetching regulations for local");
    let local_regs_resp = client
        .get("https://numbers.twilio.com/v2/RegulatoryCompliance/Regulations")
        .basic_auth(&account_sid, Some(&auth_token))
        .query(&[
            ("IsoCountry", req.country_code.to_uppercase().as_str()),
            ("NumberType", "local"),
            ("IncludeConstraints", "true"),
        ])
        .send()
        .await;
    match local_regs_resp {
        Ok(resp) if resp.status().is_success() => {
            println!("resp: {:#?}", resp);
            match resp.json::<RegulationsResponse>().await {
                Ok(json) => {
                    local_regs = json.results;
                    println!("Retrieved {} local regulations", local_regs.len());
                }
                Err(e) => println!("Failed to parse local regulations: {}", e),
            }
        }
        Ok(resp) => {
            let err_text = resp.text().await.unwrap_or_default();
            println!("Twilio API error for local regulations: {}", err_text);
        }
        Err(e) => println!("Failed to fetch local regulations: {}", e),
    }

    // Fetch regulations for mobile
    let mut mobile_regs = vec![];
    println!("Fetching regulations for mobile");
    let mobile_regs_resp = client
        .get("https://numbers.twilio.com/v2/RegulatoryCompliance/Regulations")
        .basic_auth(&account_sid, Some(&auth_token))
        .query(&[
            ("IsoCountry", req.country_code.to_uppercase().as_str()),
            ("NumberType", "mobile"),
            ("IncludeConstraints", "true"),
        ])
        .send()
        .await;
    match mobile_regs_resp {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<RegulationsResponse>().await {
                Ok(json) => {
                    mobile_regs = json.results;
                    println!("Retrieved {} mobile regulations", mobile_regs.len());
                }
                Err(e) => println!("Failed to parse mobile regulations: {}", e),
            }
        }
        Ok(resp) => {
            let err_text = resp.text().await.unwrap_or_default();
            println!("Twilio API error for mobile regulations: {}", err_text);
        }
        Err(e) => println!("Failed to fetch mobile regulations: {}", e),
    }

    let regulations = TwilioRegulations {
        local: local_regs,
        mobile: mobile_regs,
    };
    println!("Combined regulations data: {} local, {} mobile", regulations.local.len(), regulations.mobile.len());
    println!("Returning successful response");
    Ok(Json(CountryInfoResponse {
        available_numbers,
        prices,
        regulations,
    }))
}

/// Twilio Status Callback payload
/// https://www.twilio.com/docs/messaging/guides/track-outbound-message-status
/// Note: Twilio sends both MessageSid/SmsSid and MessageStatus/SmsStatus with same values.
/// We only capture the Message-prefixed ones to avoid serde duplicate field errors.
#[derive(Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct TwilioStatusCallback {
    pub MessageSid: String,
    pub MessageStatus: String,
    #[serde(default)]
    pub ErrorCode: Option<String>,
    #[serde(default)]
    pub ErrorMessage: Option<String>,
    #[serde(default)]
    pub To: Option<String>,
    #[serde(default)]
    pub From: Option<String>,
    #[serde(default)]
    pub AccountSid: Option<String>,
    #[serde(default)]
    pub ApiVersion: Option<String>,
    #[serde(default)]
    pub Price: Option<String>,
    #[serde(default)]
    pub PriceUnit: Option<String>,
    // Twilio also sends these SMS-prefixed duplicates - we ignore them
    #[serde(default)]
    pub SmsSid: Option<String>,
    #[serde(default)]
    pub SmsStatus: Option<String>,
    // Additional fields Twilio may send
    #[serde(default)]
    pub RawDlrDoneDate: Option<String>,
}

/// Response from Twilio Messages API when fetching message details
#[derive(Deserialize, Debug)]
#[allow(non_snake_case)]
struct TwilioMessageResponse {
    #[serde(default)]
    pub price: Option<String>,
    #[serde(default)]
    pub price_unit: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

/// Fetch message details from Twilio API to get pricing info
async fn fetch_message_price(
    message_sid: &str,
    account_sid: &str,
    auth_token: &str,
) -> Option<(f32, String)> {
    let client = Client::new();
    let url = format!(
        "https://api.twilio.com/2010-04-01/Accounts/{}/Messages/{}.json",
        account_sid, message_sid
    );

    match client
        .get(&url)
        .basic_auth(account_sid, Some(auth_token))
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<TwilioMessageResponse>().await {
                    Ok(msg) => {
                        tracing::info!(
                            "Twilio message {} response: status={:?}, price={:?}, price_unit={:?}",
                            message_sid, msg.status, msg.price, msg.price_unit
                        );
                        if let (Some(price_str), Some(price_unit)) = (msg.price, msg.price_unit) {
                            if let Ok(price) = price_str.parse::<f32>() {
                                tracing::info!(
                                    "Fetched price for message {}: {} {}",
                                    message_sid, price, price_unit
                                );
                                return Some((price, price_unit));
                            }
                        }
                        tracing::warn!("Message {} has no price info yet (status: {:?})", message_sid, msg.status);
                        None
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse Twilio message response: {}", e);
                        None
                    }
                }
            } else if response.status() == reqwest::StatusCode::NOT_FOUND {
                tracing::warn!("Message {} not found in Twilio (already deleted?)", message_sid);
                None
            } else {
                tracing::error!(
                    "Failed to fetch message {}: status {}",
                    message_sid, response.status()
                );
                None
            }
        }
        Err(e) => {
            tracing::error!("Failed to fetch message {}: {}", message_sid, e);
            None
        }
    }
}

/// Delete a message from Twilio (called after message reaches final status)
async fn delete_message_from_twilio(
    message_sid: &str,
    account_sid: &str,
    auth_token: &str,
) -> Result<(), String> {
    let client = Client::new();
    let url = format!(
        "https://api.twilio.com/2010-04-01/Accounts/{}/Messages/{}.json",
        account_sid, message_sid
    );

    match client
        .delete(&url)
        .basic_auth(account_sid, Some(auth_token))
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                tracing::info!("Deleted message {} from Twilio", message_sid);
                Ok(())
            } else if response.status() == reqwest::StatusCode::NOT_FOUND {
                // Message already deleted - that's fine
                tracing::info!("Message {} already deleted from Twilio", message_sid);
                Ok(())
            } else {
                Err(format!(
                    "Failed to delete message {}: status {}",
                    message_sid, response.status()
                ))
            }
        }
        Err(e) => Err(format!("Failed to delete message {}: {}", message_sid, e)),
    }
}

/// Handle Twilio SMS status callback webhooks
///
/// This endpoint receives delivery status updates from Twilio for outbound messages.
/// It updates the message_status_log table and sends admin email on failures.
///
/// Status flow: queued -> sending -> sent -> delivered (success)
///              queued -> sending -> sent -> undelivered (failure)
///              queued -> failed (immediate failure)
pub async fn twilio_status_callback(
    State(state): State<Arc<AppState>>,
    body: String,
) -> StatusCode {
    tracing::info!("Twilio status callback raw body: {}", body);

    // Parse form data manually
    let payload: TwilioStatusCallback = match serde_urlencoded::from_str(&body) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("Failed to parse Twilio status callback: {}", e);
            return StatusCode::BAD_REQUEST;
        }
    };

    tracing::info!(
        "Twilio status callback parsed: sid={}, status={}, error_code={:?}",
        payload.MessageSid,
        payload.MessageStatus,
        payload.ErrorCode
    );

    // Update message status in database
    let conn = &mut match state.db_pool.get() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to get DB connection for status callback: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    // Try to update the existing record
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;
    // Parse price from string to f32 (Twilio sends as string like "-0.0075")
    let price_value: Option<f32> = payload.Price.as_ref().and_then(|p| p.parse().ok());

    let update_result = diesel::update(
        message_status_log::table.filter(message_status_log::message_sid.eq(&payload.MessageSid))
    )
    .set((
        message_status_log::status.eq(&payload.MessageStatus),
        message_status_log::error_code.eq(&payload.ErrorCode),
        message_status_log::error_message.eq(&payload.ErrorMessage),
        message_status_log::price.eq(price_value),
        message_status_log::price_unit.eq(&payload.PriceUnit),
        message_status_log::updated_at.eq(now),
    ))
    .execute(conn);

    match update_result {
        Ok(0) => {
            tracing::warn!(
                "No message_status_log record found for SID {}, status update skipped",
                payload.MessageSid
            );
        }
        Ok(_) => {
            tracing::info!(
                "Updated message_status_log for SID {} to status {}",
                payload.MessageSid,
                payload.MessageStatus
            );
        }
        Err(e) => {
            tracing::error!("Failed to update message_status_log: {}", e);
        }
    }

    // Send admin email if delivery failed
    if payload.MessageStatus == "failed" || payload.MessageStatus == "undelivered" {
        // Get user_id from the message_status_log
        let user_info: Option<(i32, String, Option<String>)> = message_status_log::table
            .filter(message_status_log::message_sid.eq(&payload.MessageSid))
            .select((
                message_status_log::user_id,
                message_status_log::to_number,
                message_status_log::from_number,
            ))
            .first(conn)
            .ok();

        if let Some((user_id, to_number, from_number)) = user_info {
            let country = get_country_code_from_phone(&to_number).unwrap_or("Unknown".to_string());
            let from = from_number.unwrap_or("Unknown".to_string());

            // Spawn email sending to not block the webhook response
            let error_code = payload.ErrorCode.clone();
            let error_message = payload.ErrorMessage.clone();
            tokio::spawn(async move {
                if let Err(e) = send_sms_failure_admin_email(
                    user_id,
                    &to_number,
                    &from,
                    error_code.as_deref(),
                    error_message.as_deref(),
                    &country,
                ).await {
                    tracing::error!("Failed to send SMS failure admin email: {}", e);
                }
            });
        }
    }

    // On final status, fetch price from Twilio API and delete the message
    let is_final_status = matches!(
        payload.MessageStatus.as_str(),
        "delivered" | "failed" | "undelivered"
    );

    if is_final_status {
        // Get Twilio credentials from env
        let account_sid = env::var("TWILIO_ACCOUNT_SID").ok();
        let auth_token = env::var("TWILIO_AUTH_TOKEN").ok();

        if let (Some(account_sid), Some(auth_token)) = (account_sid, auth_token) {
            let message_sid = payload.MessageSid.clone();
            let db_pool = state.db_pool.clone();

            // Spawn task to fetch price and delete message
            tokio::spawn(async move {
                // Wait a few seconds for Twilio to calculate pricing
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;

                // Fetch price from Twilio API
                if let Some((price, price_unit)) = fetch_message_price(
                    &message_sid,
                    &account_sid,
                    &auth_token,
                ).await {
                    // Update message_status_log with price
                    if let Ok(mut conn) = db_pool.get() {
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs() as i32;

                        if let Err(e) = diesel::update(
                            message_status_log::table
                                .filter(message_status_log::message_sid.eq(&message_sid))
                        )
                        .set((
                            message_status_log::price.eq(price),
                            message_status_log::price_unit.eq(&price_unit),
                            message_status_log::updated_at.eq(now),
                        ))
                        .execute(&mut conn) {
                            tracing::error!("Failed to update price for message {}: {}", message_sid, e);
                        } else {
                            tracing::info!("Updated price for message {}: {} {}", message_sid, price, price_unit);
                        }
                    }
                }

                // Delete message from Twilio
                if let Err(e) = delete_message_from_twilio(
                    &message_sid,
                    &account_sid,
                    &auth_token,
                ).await {
                    tracing::error!("{}", e);
                }
            });
        } else {
            tracing::warn!("Missing Twilio credentials, skipping price fetch and deletion for {}", payload.MessageSid);
        }
    }

    // Always return 200 OK to Twilio
    StatusCode::OK
}
