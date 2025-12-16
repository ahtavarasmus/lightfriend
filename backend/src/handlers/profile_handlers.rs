use std::sync::Arc;
use diesel::result::Error as DieselError;
use axum::{
    Json,
    extract::State,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct ProactiveAgentEnabledRequest {
    enabled: bool,
}

#[derive(Serialize)]
pub struct ProactiveAgentEnabledResponse {
    enabled: bool,
}


#[derive(Deserialize)]
pub struct TimezoneUpdateRequest {
    timezone: String,
}
use axum::extract::Path;
use serde_json::json;

use crate::AppState;

#[derive(Deserialize)]
pub struct UpdateProfileRequest {
    email: String,
    phone_number: String,
    nickname: String,
    info: String,
    timezone: String,
    timezone_auto: bool,
    agent_language: String,
    notification_type: Option<String>,
    save_context: Option<i32>,
    location: String,
    nearby_places: String,
    preferred_number: Option<String>,
    // Optional 2FA verification for sensitive changes
    totp_code: Option<String>,
    passkey_response: Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct SensitiveChangeRequirements {
    pub requires_2fa: bool,
    pub has_passkeys: bool,
    pub has_totp: bool,
    pub passkey_options: Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct ProfileResponse {
    id: i32,
    email: String,
    phone_number: String,
    nickname: Option<String>,
    verified: bool,
    credits: f32,
    notify: bool,
    info: Option<String>,
    preferred_number: Option<String>,
    charge_when_under: bool,
    charge_back_to: Option<f32>,
    stripe_payment_method_id: Option<String>,
    timezone: Option<String>,
    timezone_auto: Option<bool>,
    sub_tier: Option<String>,
    credits_left: f32,
    discount: bool,
    agent_language: String,
    notification_type: Option<String>,
    sub_country: Option<String>,
    save_context: Option<i32>,
    days_until_billing: Option<i32>,
    twilio_sid: Option<String>,
    twilio_token: Option<String>,
    openrouter_api_key: Option<String>,
    textbee_device_id: Option<String>,
    textbee_api_key: Option<String>,
    estimated_monitoring_cost: f32,
    location: Option<String>,
    nearby_places: Option<String>,
    phone_number_country: Option<String>,
    server_ip: Option<String>,
    plan_type: Option<String>, // "monitor" or "digest"
    phone_service_active: bool, // whether phone service is active - can be disabled for security
}
use crate::handlers::auth_middleware::AuthUser;


pub async fn get_profile(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<ProfileResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Get user profile and settings from database
    let user = state.user_core.find_by_id(auth_user.user_id).map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Database error: {}", e)}))
    ))?;
    match user {
        Some(user) => {
            // TODO can be removed in the future
            let mut phone_country = user.phone_number_country.clone();
            if phone_country.is_none() {
                match set_user_phone_country(&state, user.id, &user.phone_number).await {
                    Ok(c) => phone_country = c,
                    Err(e) => {
                        tracing::error!("Failed to set phone country: {}", e);
                    }
                }
            }
            let user_settings = state.user_core.get_user_settings(auth_user.user_id).map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))?;
            let user_info = state.user_core.get_user_info(auth_user.user_id).map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))?;
            // Get current digest settings
            let (morning_digest_time, day_digest_time, evening_digest_time) = state.user_core.get_digests(auth_user.user_id)
                .map_err(|e| {
                    tracing::error!("Failed to get digest settings: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Failed to get digest settings: {}", e)}))
                    )
                })?;
            // Count current active digests
            let current_count: i32 = [morning_digest_time.as_ref(), day_digest_time.as_ref(), evening_digest_time.as_ref()]
                .iter()
                .filter(|&&x| x.is_some())
                .count() as i32;
            let days_until_billing: Option<i32> = user.next_billing_date_timestamp.map(|date| {
                let current_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i32;
                (date - current_time) / (24 * 60 * 60)
            });
            // Fetch Twilio credentials and mask them
            let (twilio_sid, twilio_token) = match state.user_core.get_twilio_credentials(auth_user.user_id) {
                Ok((sid, token)) => {
                    let masked_sid = if sid.len() >= 4 {
                        format!("...{}", &sid[sid.len() - 4..])
                    } else {
                        "...".to_string()
                    };
                    let masked_token = if token.len() >= 4 {
                        format!("...{}", &token[token.len() - 4..])
                    } else {
                        "...".to_string()
                    };
                    (Some(masked_sid), Some(masked_token))
                },
                Err(_) => (None, None),
            };
            // Fetch Textbee credentials and mask them
            let (textbee_device_id, textbee_api_key) = match state.user_core.get_textbee_credentials(auth_user.user_id) {
                Ok((id, key)) => {
                    let masked_key= if key.len() >= 4 {
                        format!("...{}", &key[key.len() - 4..])
                    } else {
                        "...".to_string()
                    };
                    let masked_id= if id.len() >= 4 {
                        format!("...{}", &id[id.len() - 4..])
                    } else {
                        "...".to_string()
                    };
                    (Some(masked_id), Some(masked_key))
                },
                Err(_) => (None, None),
            };
            let openrouter_api_key = match state.user_core.get_openrouter_api_key(auth_user.user_id) {
                Ok(key) => {
                    let masked_key= if key.len() >= 4 {
                        format!("...{}", &key[key.len() - 4..])
                    } else {
                        "...".to_string()
                    };
                    Some(masked_key)
                },
                Err(_) => None,
            };
            // Determine country based on phone number (default to "US" if unknown)
            let country = phone_country.clone().unwrap_or_else(|| "US".to_string());
            // Get critical notification info
            let critical_info = state.user_core.get_critical_notification_info(auth_user.user_id).map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))?;
            let estimated_critical_monthly = critical_info.estimated_monthly_price;
            // Get priority notification info
            let priority_info = state.user_core.get_priority_notification_info(auth_user.user_id).map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))?;
            let estimated_priority_monthly = priority_info.estimated_monthly_price;
            // Calculate digest estimated monthly cost
            let estimated_digest_monthly = if current_count > 0 {
                let active_count_f = current_count as f32;
                let cost_per_digest = if country == "US" {
                    0.5
                } else if country == "Other" {
                    0.0
                } else {
                    0.30
                };
                active_count_f * 30.0 * cost_per_digest
            } else {
                0.0
            };
            // Calculate total estimated monitoring cost
            let estimated_monitoring_cost = estimated_critical_monthly + estimated_priority_monthly + estimated_digest_monthly;
            Ok(Json(ProfileResponse {
                id: user.id,
                email: user.email,
                phone_number: user.phone_number,
                nickname: user.nickname,
                verified: user.verified,
                credits: user.credits,
                notify: user_settings.notify,
                info: user_info.info,
                preferred_number: user.preferred_number,
                charge_when_under: user.charge_when_under,
                charge_back_to: user.charge_back_to,
                stripe_payment_method_id: user.stripe_payment_method_id,
                timezone: user_info.timezone,
                timezone_auto: user_settings.timezone_auto,
                sub_tier: user.sub_tier,
                credits_left: user.credits_left,
                discount: user.discount,
                agent_language: user_settings.agent_language,
                notification_type: user_settings.notification_type,
                sub_country: user_settings.sub_country,
                save_context: user_settings.save_context,
                days_until_billing: days_until_billing,
                twilio_sid: twilio_sid,
                twilio_token: twilio_token,
                openrouter_api_key: openrouter_api_key,
                textbee_device_id: textbee_device_id,
                textbee_api_key: textbee_api_key,
                estimated_monitoring_cost,
                location: user_info.location,
                nearby_places: user_info.nearby_places,
                phone_number_country: phone_country,
                server_ip: user_settings.server_ip,
                plan_type: user.plan_type,
                phone_service_active: user_settings.phone_service_active,
            }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"}))
        )),
    }
}


#[derive(Deserialize)]
pub struct NotifyCreditsRequest {
    notify: bool,
}

pub async fn update_notify(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(user_id): Path<i32>,
    Json(request): Json<NotifyCreditsRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {

    // Check if user is modifying their own settings or is an admin
    if auth_user.user_id != user_id && !auth_user.is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "You can only modify your own settings unless you're an admin"}))
        ));
    }

    // Update notify preference
    state.user_core.update_notify(user_id, request.notify)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
    ))?;

    Ok(Json(json!({
        "message": "Notification preference updated successfully"
    })))
}

pub async fn update_timezone(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<TimezoneUpdateRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {

    match state.user_core.update_timezone(
        auth_user.user_id,
        &request.timezone,
    ) {
        Ok(_) => Ok(Json(json!({
            "message": "Timezone updated successfully"
        }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
        )),
    }
}

#[derive(Deserialize)]
pub struct PatchFieldRequest {
    field: String,
    value: serde_json::Value,
}

/// Generic endpoint to update individual profile fields
/// Allows inline editing on the frontend without bulk updates
pub async fn patch_profile_field(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<PatchFieldRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let user_id = auth_user.user_id;

    match request.field.as_str() {
        "nickname" => {
            let value = request.value.as_str().ok_or_else(|| (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "nickname must be a string"}))
            ))?;
            if value.len() > 30 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "Nickname must be 30 characters or less"}))
                ));
            }
            state.user_core.update_nickname(user_id, value).map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))?;
        }
        "info" => {
            let value = request.value.as_str().ok_or_else(|| (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "info must be a string"}))
            ))?;
            if value.len() > 500 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "Info must be 500 characters or less"}))
                ));
            }
            state.user_core.update_info(user_id, value).map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))?;
        }
        "location" => {
            let value = request.value.as_str().ok_or_else(|| (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "location must be a string"}))
            ))?;
            state.user_core.update_location(user_id, value).map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))?;
        }
        "nearby_places" => {
            let value = request.value.as_str().ok_or_else(|| (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "nearby_places must be a string"}))
            ))?;
            state.user_core.update_nearby_places(user_id, value).map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))?;
        }
        "timezone" => {
            let value = request.value.as_str().ok_or_else(|| (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "timezone must be a string"}))
            ))?;
            // Validate timezone
            if value.parse::<chrono_tz::Tz>().is_err() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "Invalid timezone"}))
                ));
            }
            state.user_core.update_timezone(user_id, value).map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))?;
        }
        "timezone_auto" => {
            let value = request.value.as_bool().ok_or_else(|| (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "timezone_auto must be a boolean"}))
            ))?;
            state.user_core.update_timezone_auto(user_id, value).map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))?;
        }
        "agent_language" => {
            let value = request.value.as_str().ok_or_else(|| (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "agent_language must be a string"}))
            ))?;
            let allowed_languages = vec!["en", "fi", "de"];
            if !allowed_languages.contains(&value) {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "Invalid agent language. Must be 'en', 'fi', or 'de'"}))
                ));
            }
            state.user_core.update_agent_language(user_id, value).map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))?;
        }
        "notification_type" => {
            let value = request.value.as_str().ok_or_else(|| (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "notification_type must be a string"}))
            ))?;
            let allowed_types = vec!["sms", "call"];
            if !allowed_types.contains(&value) {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "Invalid notification type. Must be 'sms' or 'call'"}))
                ));
            }
            state.user_core.update_notification_type(user_id, Some(value)).map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))?;
        }
        "save_context" => {
            let value = request.value.as_i64().ok_or_else(|| (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "save_context must be an integer"}))
            ))? as i32;
            if !(0..=10).contains(&value) {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "save_context must be between 0 and 10"}))
                ));
            }
            state.user_core.update_save_context(user_id, value).map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))?;
        }
        "phone_service_active" => {
            let value = request.value.as_bool().ok_or_else(|| (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "phone_service_active must be a boolean"}))
            ))?;
            state.user_core.update_phone_service_active(user_id, value).map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))?;
        }
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("Unknown field: {}", request.field)}))
            ));
        }
    }

    Ok(Json(json!({"success": true})))
}

pub async fn set_user_phone_country(state: &Arc<AppState>, user_id: i32, phone_number: &str) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let ca_area_codes: Vec<String> = vec![
        "+1204".to_string(),
        "+1226".to_string(),
        "+1236".to_string(),
        "+1249".to_string(),
        "+1250".to_string(),
        "+1289".to_string(),
        "+1306".to_string(),
        "+1343".to_string(),
        "+1365".to_string(),
        "+1367".to_string(),
        "+1368".to_string(),
        "+1403".to_string(),
        "+1416".to_string(),
        "+1418".to_string(),
        "+1437".to_string(),
        "+1438".to_string(),
        "+1450".to_string(),
        "+1506".to_string(),
        "+1514".to_string(),
        "+1519".to_string(),
        "+1548".to_string(),
        "+1579".to_string(),
        "+1581".to_string(),
        "+1587".to_string(),
        "+1604".to_string(),
        "+1613".to_string(),
        "+1639".to_string(),
        "+1647".to_string(),
        "+1672".to_string(),
        "+1705".to_string(),
        "+1709".to_string(),
        "+1778".to_string(),
        "+1780".to_string(),
        "+1782".to_string(),
        "+1807".to_string(),
        "+1819".to_string(),
        "+1825".to_string(),
        "+1867".to_string(),
        "+1873".to_string(),
        "+1879".to_string(),
        "+1902".to_string(),
        "+1905".to_string(),
    ];
    let us_area_codes: Vec<String> = vec![
        "+1201".to_string(),
        "+1202".to_string(),
        "+1203".to_string(),
        "+1205".to_string(),
        "+1206".to_string(),
        "+1207".to_string(),
        "+1208".to_string(),
        "+1209".to_string(),
        "+1210".to_string(),
        "+1212".to_string(),
        "+1213".to_string(),
        "+1214".to_string(),
        "+1215".to_string(),
        "+1216".to_string(),
        "+1217".to_string(),
        "+1218".to_string(),
        "+1219".to_string(),
        "+1220".to_string(),
        "+1223".to_string(),
        "+1224".to_string(),
        "+1225".to_string(),
        "+1228".to_string(),
        "+1229".to_string(),
        "+1231".to_string(),
        "+1234".to_string(),
        "+1239".to_string(),
        "+1240".to_string(),
        "+1248".to_string(),
        "+1251".to_string(),
        "+1252".to_string(),
        "+1253".to_string(),
        "+1254".to_string(),
        "+1256".to_string(),
        "+1260".to_string(),
        "+1262".to_string(),
        "+1267".to_string(),
        "+1269".to_string(),
        "+1270".to_string(),
        "+1272".to_string(),
        "+1274".to_string(),
        "+1276".to_string(),
        "+1281".to_string(),
        "+1301".to_string(),
        "+1302".to_string(),
        "+1303".to_string(),
        "+1304".to_string(),
        "+1305".to_string(),
        "+1307".to_string(),
        "+1308".to_string(),
        "+1309".to_string(),
        "+1310".to_string(),
        "+1312".to_string(),
        "+1313".to_string(),
        "+1314".to_string(),
        "+1315".to_string(),
        "+1316".to_string(),
        "+1317".to_string(),
        "+1318".to_string(),
        "+1319".to_string(),
        "+1320".to_string(),
        "+1321".to_string(),
        "+1323".to_string(),
        "+1325".to_string(),
        "+1330".to_string(),
        "+1331".to_string(),
        "+1332".to_string(),
        "+1334".to_string(),
        "+1336".to_string(),
        "+1337".to_string(),
        "+1339".to_string(),
        "+1341".to_string(),
        "+1346".to_string(),
        "+1347".to_string(),
        "+1351".to_string(),
        "+1352".to_string(),
        "+1359".to_string(),
        "+1360".to_string(),
        "+1361".to_string(),
        "+1363".to_string(),
        "+1364".to_string(),
        "+1369".to_string(),
        "+1380".to_string(),
        "+1385".to_string(),
        "+1386".to_string(),
        "+1401".to_string(),
        "+1402".to_string(),
        "+1404".to_string(),
        "+1405".to_string(),
        "+1406".to_string(),
        "+1407".to_string(),
        "+1408".to_string(),
        "+1409".to_string(),
        "+1413".to_string(),
        "+1414".to_string(),
        "+1415".to_string(),
        "+1417".to_string(),
        "+1419".to_string(),
        "+1423".to_string(),
        "+1424".to_string(),
        "+1425".to_string(),
        "+1430".to_string(),
        "+1432".to_string(),
        "+1434".to_string(),
        "+1435".to_string(),
        "+1440".to_string(),
        "+1443".to_string(),
        "+1445".to_string(),
        "+1447".to_string(),
        "+1448".to_string(),
        "+1463".to_string(),
        "+1464".to_string(),
        "+1469".to_string(),
        "+1470".to_string(),
        "+1475".to_string(),
        "+1478".to_string(),
        "+1479".to_string(),
        "+1480".to_string(),
        "+1484".to_string(),
        "+1501".to_string(),
        "+1502".to_string(),
        "+1503".to_string(),
        "+1504".to_string(),
        "+1505".to_string(),
        "+1507".to_string(),
        "+1508".to_string(),
        "+1509".to_string(),
        "+1510".to_string(),
        "+1512".to_string(),
        "+1513".to_string(),
        "+1515".to_string(),
        "+1516".to_string(),
        "+1517".to_string(),
        "+1518".to_string(),
        "+1520".to_string(),
        "+1530".to_string(),
        "+1539".to_string(),
        "+1540".to_string(),
        "+1541".to_string(),
        "+1551".to_string(),
        "+1559".to_string(),
        "+1561".to_string(),
        "+1562".to_string(),
        "+1563".to_string(),
        "+1567".to_string(),
        "+1570".to_string(),
        "+1571".to_string(),
        "+1573".to_string(),
        "+1574".to_string(),
        "+1575".to_string(),
        "+1580".to_string(),
        "+1585".to_string(),
        "+1586".to_string(),
        "+1601".to_string(),
        "+1602".to_string(),
        "+1603".to_string(),
        "+1605".to_string(),
        "+1606".to_string(),
        "+1607".to_string(),
        "+1608".to_string(),
        "+1609".to_string(),
        "+1610".to_string(),
        "+1612".to_string(),
        "+1614".to_string(),
        "+1615".to_string(),
        "+1616".to_string(),
        "+1617".to_string(),
        "+1618".to_string(),
        "+1619".to_string(),
        "+1620".to_string(),
        "+1623".to_string(),
        "+1626".to_string(),
        "+1630".to_string(),
        "+1631".to_string(),
        "+1636".to_string(),
        "+1641".to_string(),
        "+1646".to_string(),
        "+1650".to_string(),
        "+1651".to_string(),
        "+1657".to_string(),
        "+1660".to_string(),
        "+1661".to_string(),
        "+1662".to_string(),
        "+1667".to_string(),
        "+1669".to_string(),
        "+1678".to_string(),
        "+1679".to_string(),
        "+1681".to_string(),
        "+1682".to_string(),
        "+1701".to_string(),
        "+1702".to_string(),
        "+1703".to_string(),
        "+1704".to_string(),
        "+1706".to_string(),
        "+1707".to_string(),
        "+1708".to_string(),
        "+1712".to_string(),
        "+1713".to_string(),
        "+1714".to_string(),
        "+1715".to_string(),
        "+1716".to_string(),
        "+1717".to_string(),
        "+1718".to_string(),
        "+1719".to_string(),
        "+1720".to_string(),
        "+1724".to_string(),
        "+1725".to_string(),
        "+1726".to_string(),
        "+1727".to_string(),
        "+1731".to_string(),
        "+1732".to_string(),
        "+1734".to_string(),
        "+1737".to_string(),
        "+1740".to_string(),
        "+1743".to_string(),
        "+1747".to_string(),
        "+1754".to_string(),
        "+1757".to_string(),
        "+1760".to_string(),
        "+1762".to_string(),
        "+1763".to_string(),
        "+1765".to_string(),
        "+1769".to_string(),
        "+1770".to_string(),
        "+1771".to_string(),
        "+1772".to_string(),
        "+1773".to_string(),
        "+1774".to_string(),
        "+1775".to_string(),
        "+1781".to_string(),
        "+1785".to_string(),
        "+1786".to_string(),
        "+1801".to_string(),
        "+1802".to_string(),
        "+1803".to_string(),
        "+1804".to_string(),
        "+1805".to_string(),
        "+1806".to_string(),
        "+1808".to_string(),
        "+1810".to_string(),
        "+1812".to_string(),
        "+1813".to_string(),
        "+1814".to_string(),
        "+1815".to_string(),
        "+1816".to_string(),
        "+1817".to_string(),
        "+1818".to_string(),
        "+1828".to_string(),
        "+1830".to_string(),
        "+1831".to_string(),
        "+1832".to_string(),
        "+1837".to_string(),
        "+1843".to_string(),
        "+1845".to_string(),
        "+1847".to_string(),
        "+1848".to_string(),
        "+1850".to_string(),
        "+1856".to_string(),
        "+1857".to_string(),
        "+1858".to_string(),
        "+1859".to_string(),
        "+1860".to_string(),
        "+1862".to_string(),
        "+1863".to_string(),
        "+1864".to_string(),
        "+1865".to_string(),
        "+1870".to_string(),
        "+1872".to_string(),
        "+1878".to_string(),
        "+1901".to_string(),
        "+1903".to_string(),
        "+1904".to_string(),
        "+1906".to_string(),
        "+1907".to_string(),
        "+1908".to_string(),
        "+1909".to_string(),
        "+1914".to_string(),
        "+1915".to_string(),
        "+1916".to_string(),
        "+1917".to_string(),
        "+1918".to_string(),
        "+1919".to_string(),
        "+1920".to_string(),
        "+1925".to_string(),
        "+1928".to_string(),
        "+1929".to_string(),
        "+1931".to_string(),
        "+1936".to_string(),
        "+1937".to_string(),
        "+1940".to_string(),
        "+1941".to_string(),
        "+1945".to_string(),
        "+1949".to_string(),
        "+1951".to_string(),
        "+1952".to_string(),
        "+1954".to_string(),
        "+1956".to_string(),
        "+1959".to_string(),
        "+1970".to_string(),
        "+1971".to_string(),
        "+1972".to_string(),
        "+1973".to_string(),
        "+1978".to_string(),
        "+1979".to_string(),
        "+1980".to_string(),
        "+1984".to_string(),
        "+1985".to_string(),
        "+1986".to_string(),
        "+1989".to_string(),
    ];
    let mut country: Option<String> = None;

    tracing::debug!("phone_number: {}, len: {}", phone_number, phone_number.len());
    if phone_number.starts_with("+1") {
        let area_code = phone_number.get(0..5).unwrap_or_default();
        tracing::debug!("Extracted area code: {}", area_code);
        if ca_area_codes.contains(&area_code.to_string()) {
            country = Some("CA".to_string());
        } else if us_area_codes.contains(&area_code.to_string()) {
            country = Some("US".to_string());
        }
    } else if phone_number.starts_with("+358") {
        country = Some("FI".to_string());
    } else if phone_number.starts_with("+31") {
        country = Some("NL".to_string());
    } else if phone_number.starts_with("+44") {
        country = Some("GB".to_string());
    } else if phone_number.starts_with("+61") {
        country = Some("AU".to_string());
    } else {
        country = Some("Other".to_string()); // Or None if preferred
    }

    tracing::debug!("country: {:#?}", country);

    if let Some(ref c) = country {
        state.user_core.update_phone_number_country(user_id, Some(c))?;
    } else {
        state.user_core.update_phone_number_country(user_id, None)?;
    }

    Ok(country)
}

/// Recalculate credits_left when user changes phone country
/// Uses proportional transfer: preserves the percentage of monthly allowance remaining
async fn recalculate_credits_for_country_change(
    state: &Arc<AppState>,
    user_id: i32,
    old_country: Option<&str>,
    new_country: Option<&str>,
    old_credits_left: f32,
    plan_type: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use crate::api::twilio_availability::get_byot_pricing;

    // Determine plan messages (40 for monitor, 120 for digest)
    let plan_messages: f32 = match plan_type {
        Some("digest") => 120.0,
        _ => 40.0, // monitor or default
    };

    // Check if country is US/CA
    let is_us_ca = |c: Option<&str>| matches!(c, Some("US") | Some("CA"));

    // Get max credits for a country
    // US/CA: always 400 messages (hosted plan)
    // Euro: credits_left is € value = plan_messages × 3 × sms_price × 1.3
    let get_max_credits = async |country: Option<&str>| -> f32 {
        if is_us_ca(country) {
            // US/CA: always 400 messages (hosted plan)
            400.0
        } else if let Some(c) = country {
            // Euro: € value based on SMS pricing
            if let Ok(pricing) = get_byot_pricing(state, c).await {
                if let Some(sms_price) = pricing.sms_price_per_segment {
                    // Max credits = plan_messages × 3 segments × sms_price × 1.3 VAT
                    return plan_messages * 3.0 * sms_price * 1.3;
                }
            }
            // Fallback: assume €0.10 per segment
            plan_messages * 3.0 * 0.10 * 1.3
        } else {
            // Unknown country fallback
            plan_messages * 3.0 * 0.10 * 1.3
        }
    };

    let old_max = get_max_credits(old_country).await;
    let new_max = get_max_credits(new_country).await;

    if old_max <= 0.0 || new_max <= 0.0 {
        tracing::warn!("Invalid max credits: old={}, new={}", old_max, new_max);
        return Ok(());
    }

    // Calculate ratio of remaining allowance (capped at 1.0)
    let ratio = (old_credits_left / old_max).min(1.0);

    // Apply ratio to new country's max
    let new_credits_left = new_max * ratio;

    tracing::info!(
        "Credit recalculation: user={}, old_country={:?}, new_country={:?}, \
         old_credits={:.2}, old_max={:.2}, ratio={:.2}, new_credits={:.2}",
        user_id, old_country, new_country, old_credits_left, old_max, ratio, new_credits_left
    );

    // Update credits_left
    state.user_repository.update_user_credits_left(user_id, new_credits_left)?;

    Ok(())
}

/// Check if 2FA is required for sensitive profile changes
/// Returns the 2FA requirements and passkey options if available
pub async fn check_sensitive_change_requirements(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<SensitiveChangeRequirements>, (StatusCode, Json<serde_json::Value>)> {
    // Check if user has TOTP enabled
    let has_totp = state.totp_repository
        .is_totp_enabled(auth_user.user_id)
        .unwrap_or(false);

    // Check if user has passkeys
    let passkey_count = state.webauthn_repository
        .get_passkey_count(auth_user.user_id)
        .unwrap_or(0);
    let has_passkeys = passkey_count > 0;

    // If user has passkeys, prepare authentication options
    let passkey_options = if has_passkeys {
        match prepare_passkey_auth_options(&state, auth_user.user_id).await {
            Ok(options) => Some(options),
            Err(e) => {
                tracing::error!("Failed to prepare passkey options: {}", e);
                None
            }
        }
    } else {
        None
    };

    Ok(Json(SensitiveChangeRequirements {
        requires_2fa: has_totp || has_passkeys,
        has_passkeys,
        has_totp,
        passkey_options,
    }))
}

/// Prepare passkey authentication options for sensitive change verification
async fn prepare_passkey_auth_options(
    state: &Arc<AppState>,
    user_id: i32,
) -> Result<serde_json::Value, String> {
    use crate::utils::webauthn_config::get_webauthn;
    use webauthn_rs::prelude::*;

    let credentials = state.webauthn_repository
        .get_credentials_by_user(user_id)
        .map_err(|e| format!("Failed to get credentials: {:?}", e))?;

    if credentials.is_empty() {
        return Err("No passkeys registered".to_string());
    }

    // Deserialize credentials back to Passkey objects
    let passkeys: Vec<Passkey> = credentials
        .iter()
        .filter_map(|c| {
            let decrypted = state.webauthn_repository.get_decrypted_public_key(c).ok()?;
            serde_json::from_str(&decrypted).ok()
        })
        .collect();

    if passkeys.is_empty() {
        return Err("Failed to load credentials".to_string());
    }

    let webauthn = get_webauthn();

    // Start authentication
    let (rcr, auth_state) = webauthn
        .start_passkey_authentication(&passkeys)
        .map_err(|e| format!("Failed to start authentication: {:?}", e))?;

    // Store authentication state with "sensitive_change" context
    let state_json = serde_json::to_string(&auth_state)
        .map_err(|e| format!("Failed to serialize auth state: {:?}", e))?;

    state.webauthn_repository
        .create_challenge(
            user_id,
            &state_json,
            "sensitive_change",
            Some("profile_update".to_string()),
            300, // 5 minute TTL
        )
        .map_err(|e| format!("Failed to store challenge: {:?}", e))?;

    // Return the options for the frontend
    Ok(serde_json::json!({ "options": rcr }))
}

/// Verify TOTP code for sensitive changes
fn verify_totp_code(state: &Arc<AppState>, user_id: i32, code: &str) -> Result<bool, String> {
    use totp_rs::{Algorithm, TOTP, Secret};

    let secret_opt = state.totp_repository
        .get_secret(user_id)
        .map_err(|e| format!("Database error: {:?}", e))?;

    let secret_base32 = secret_opt.ok_or("TOTP not configured")?;

    // Get user email
    let user = state.user_core
        .find_by_id(user_id)
        .map_err(|e| format!("Database error: {:?}", e))?
        .ok_or("User not found")?;

    let secret = Secret::Encoded(secret_base32);
    let totp = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret.to_bytes().unwrap(),
        Some("Lightfriend".to_string()),
        user.email,
    ).map_err(|e| format!("TOTP creation error: {:?}", e))?;

    Ok(totp.check_current(code).unwrap_or(false))
}

/// Verify passkey response for sensitive changes
async fn verify_passkey_response(
    state: &Arc<AppState>,
    user_id: i32,
    response: &serde_json::Value,
) -> Result<bool, String> {
    use crate::utils::webauthn_config::get_webauthn;
    use webauthn_rs::prelude::*;

    // Get the stored authentication state
    let challenge = state.webauthn_repository
        .get_valid_challenge(user_id, "sensitive_change")
        .map_err(|e| format!("Failed to get challenge: {:?}", e))?
        .ok_or("No pending authentication")?;

    // Deserialize authentication state
    let auth_state: PasskeyAuthentication = serde_json::from_str(&challenge.challenge)
        .map_err(|e| format!("Failed to deserialize auth state: {:?}", e))?;

    // Parse the response
    let pk_credential: PublicKeyCredential = serde_json::from_value(response.clone())
        .map_err(|e| format!("Failed to parse passkey response: {:?}", e))?;

    let webauthn = get_webauthn();

    // Finish authentication
    let auth_result = webauthn
        .finish_passkey_authentication(&pk_credential, &auth_state)
        .map_err(|e| format!("Authentication failed: {:?}", e))?;

    // Update the credential counter
    let credential_id = base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        auth_result.cred_id().as_ref()
    );
    let _ = state.webauthn_repository.update_counter(&credential_id, auth_result.counter() as i32);

    // Delete the challenge
    let _ = state.webauthn_repository.delete_challenges_by_type(user_id, "sensitive_change");

    Ok(true)
}

pub async fn update_profile(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(update_req): Json<UpdateProfileRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::debug!("Updating profile with notification type: {:?}", update_req.notification_type);
    use regex::Regex;
    let email_regex = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
    if !email_regex.is_match(&update_req.email) {
        tracing::debug!("Invalid email format: {}", update_req.email);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid email format"}))
        ));
    }

    let phone_regex = Regex::new(r"^\+[1-9]\d{1,14}$").unwrap();
    if !phone_regex.is_match(&update_req.phone_number) {
        tracing::debug!("Invalid phone number format: {}", update_req.phone_number);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Phone number must be in E.164 format (e.g., +1234567890)"}))
        ));
    }
    // Validate agent language
    let allowed_languages = vec!["en", "fi", "de"];
    if !allowed_languages.contains(&update_req.agent_language.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid agent language. Must be 'en', 'fi', or 'de'"}))
        ));
    }

    // Get user's current data BEFORE updating (for credit recalculation and 2FA check)
    let current_user = state.user_core.find_by_id(auth_user.user_id)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
        ))?
        .ok_or_else(|| (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"}))
        ))?;

    // Check if email or phone is changing
    let email_changing = current_user.email != update_req.email;
    let phone_changing = current_user.phone_number != update_req.phone_number;

    // If sensitive fields are changing, verify 2FA if user has it enabled
    if email_changing || phone_changing {
        let has_totp = state.totp_repository
            .is_totp_enabled(auth_user.user_id)
            .unwrap_or(false);
        let passkey_count = state.webauthn_repository
            .get_passkey_count(auth_user.user_id)
            .unwrap_or(0);
        let has_passkeys = passkey_count > 0;

        if has_totp || has_passkeys {
            // User has 2FA enabled, require verification
            let mut verified = false;

            // Try passkey verification first (if provided)
            if let Some(ref passkey_response) = update_req.passkey_response {
                match verify_passkey_response(&state, auth_user.user_id, passkey_response).await {
                    Ok(true) => {
                        verified = true;
                        tracing::info!("Passkey verification successful for sensitive change");
                    }
                    Ok(false) => {
                        return Err((
                            StatusCode::UNAUTHORIZED,
                            Json(json!({"error": "Passkey verification failed"}))
                        ));
                    }
                    Err(e) => {
                        tracing::error!("Passkey verification error: {}", e);
                        return Err((
                            StatusCode::UNAUTHORIZED,
                            Json(json!({"error": format!("Passkey verification error: {}", e)}))
                        ));
                    }
                }
            }

            // Try TOTP verification (if provided and not already verified)
            if !verified {
                if let Some(ref totp_code) = update_req.totp_code {
                    match verify_totp_code(&state, auth_user.user_id, totp_code) {
                        Ok(true) => {
                            verified = true;
                            tracing::info!("TOTP verification successful for sensitive change");
                        }
                        Ok(false) => {
                            // Also try as backup code
                            let backup_valid = state.totp_repository
                                .verify_backup_code(auth_user.user_id, totp_code)
                                .unwrap_or(false);
                            if backup_valid {
                                verified = true;
                                tracing::info!("Backup code verification successful for sensitive change");
                            } else {
                                return Err((
                                    StatusCode::UNAUTHORIZED,
                                    Json(json!({"error": "Invalid verification code"}))
                                ));
                            }
                        }
                        Err(e) => {
                            tracing::error!("TOTP verification error: {}", e);
                            return Err((
                                StatusCode::UNAUTHORIZED,
                                Json(json!({"error": format!("TOTP verification error: {}", e)}))
                            ));
                        }
                    }
                }
            }

            // If neither verification method was provided, return error with requirements
            if !verified {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(json!({
                        "error": "2FA verification required",
                        "requires_2fa": true,
                        "has_passkeys": has_passkeys,
                        "has_totp": has_totp
                    }))
                ));
            }
        }
    }

    // Re-fetch current user for credit recalculation (already fetched above)
    let current_user = state.user_core.find_by_id(auth_user.user_id)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
        ))?
        .ok_or_else(|| (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"}))
        ))?;
    let old_country = current_user.phone_number_country.clone();
    let old_credits_left = current_user.credits_left;

    match state.user_core.update_profile(
        auth_user.user_id,
        &update_req.email,
        &update_req.phone_number,
        &update_req.nickname,
        &update_req.info,
        &update_req.timezone,
        &update_req.timezone_auto,
        update_req.notification_type.as_deref(),
        update_req.save_context,
        &update_req.location,
        &update_req.nearby_places,
        update_req.preferred_number.as_deref(),
    ) {
        Ok(_) => {
            if let Err(e) = state.user_core.update_agent_language(auth_user.user_id, &update_req.agent_language) {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to update agent language: {}", e)}))
                ));
            }
            // Set phone country after update
            let new_country = match set_user_phone_country(&state, auth_user.user_id, &update_req.phone_number).await {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("Failed to set phone country after profile update: {}", e);
                    None
                }
            };

            // Recalculate credits if country changed and user has credits_left
            if old_country != new_country && old_credits_left > 0.0 && current_user.sub_tier.is_some() {
                if let Err(e) = recalculate_credits_for_country_change(
                    &state,
                    auth_user.user_id,
                    old_country.as_deref(),
                    new_country.as_deref(),
                    old_credits_left,
                    current_user.plan_type.as_deref(),
                ).await {
                    tracing::error!("Failed to recalculate credits after country change: {}", e);
                    // Continue anyway, user keeps their credits
                }
            }
        }, Err(DieselError::NotFound) => {
            return Err((
                StatusCode::CONFLICT,
                Json(json!({"error": "Email already exists"}))
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ));
        }
    }
    Ok(Json(json!({
        "message": "Profile updated successfully"
    })))
}

use axum::extract::Query;
use crate::utils::tool_exec::get_nearby_towns;

#[derive(Deserialize)]
pub struct GetNearbyPlacesQuery {
    pub location: String,
}

pub async fn get_nearby_places(
    State(_state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Query(query): Query<GetNearbyPlacesQuery>,
) -> Result<Json<Vec<String>>, (StatusCode, Json<serde_json::Value>)> {
    match get_nearby_towns(&query.location).await {
        Ok(places) => {
            Ok(Json(places))
        },
        Err(e) => Err((StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()})))),
    }
}

#[derive(Serialize)]
pub struct EmailJudgmentResponse {
    pub id: i32,
    pub email_timestamp: i32,
    pub processed_at: i32,
    pub should_notify: bool,
    pub score: i32,
    pub reason: String,
}



pub async fn get_email_judgments(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<Vec<EmailJudgmentResponse>>, (StatusCode, Json<serde_json::Value>)> {
    match state.user_repository.get_user_email_judgments(auth_user.user_id) {
        Ok(judgments) => {
            let responses: Vec<EmailJudgmentResponse> = judgments
                .into_iter()
                .map(|j| EmailJudgmentResponse {
                    id: j.id.unwrap_or(0),
                    email_timestamp: j.email_timestamp,
                    processed_at: j.processed_at,
                    should_notify: j.should_notify,
                    score: j.score,
                    reason: j.reason,
                })
                .collect();
            Ok(Json(responses))
        },
        Err(e) => {
            tracing::error!("Failed to get email judgments: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to get email judgments: {}", e)}))
            ))
        }
    }
}


#[derive(Serialize)]
pub struct DigestsResponse {
    morning_digest_time: Option<String>,
    day_digest_time: Option<String>,
    evening_digest_time: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateDigestsRequest {
    morning_digest_time: Option<String>,
    day_digest_time: Option<String>,
    evening_digest_time: Option<String>,
}


pub async fn get_digests(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<DigestsResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Get current digest settings
    let (morning_digest_time, day_digest_time, evening_digest_time) = state.user_core.get_digests(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Failed to get digest settings: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to get digest settings: {}", e)}))
            )
        })?;

    Ok(Json(DigestsResponse {
        morning_digest_time,
        day_digest_time,
        evening_digest_time,
    }))
}

pub async fn update_digests(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<UpdateDigestsRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    match state.user_core.update_digests(
        auth_user.user_id,
        request.morning_digest_time.as_deref(),
        request.day_digest_time.as_deref(),
        request.evening_digest_time.as_deref(),
    ) {
        Ok(_) => {
            let message = String::from("Digest settings updated successfully");
            let response = json!({
                "message": message,
            });
            Ok(Json(response))
        },
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to update digest settings: {}", e)}))
        )),
    }
}

#[derive(Deserialize)]
pub struct UpdateCriticalRequest {
    #[serde(default, deserialize_with = "deserialize_double_option")]
    enabled: Option<Option<String>>,
    call_notify: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    action_on_critical_message: Option<Option<String>>,
}

// Custom deserializer for Option<Option<T>> to handle {"field": null} correctly
fn deserialize_double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    Ok(Some(Option::deserialize(deserializer)?))
}

pub async fn update_critical_settings(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<UpdateCriticalRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::debug!("Received update_critical_settings request: enabled={:?}, call_notify={:?}, action={:?}",
        request.enabled, request.call_notify, request.action_on_critical_message);

    if let Some(enabled) = request.enabled {
        tracing::debug!("Updating critical_enabled to: {:?}", enabled);
        if let Err(e) = state.user_core.update_critical_enabled(auth_user.user_id, enabled) {
            tracing::error!("Failed to update critical enabled setting: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to update critical enabled setting: {}", e)}))
            ));
        }
    }
    if let Some(call_notify) = request.call_notify {
        if let Err(e) = state.user_core.update_call_notify(auth_user.user_id, call_notify) {
            tracing::error!("Failed to update call notify setting: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to update call notify setting: {}", e)}))
            ));
        }
    }
    if let Some(action) = request.action_on_critical_message {
        if let Err(e) = state.user_core.update_action_on_critical_message(auth_user.user_id, action) {
            tracing::error!("Failed to update action on critical message setting: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to update action on critical message setting: {}", e)}))
            ));
        }
    }
    Ok(Json(json!({
        "message": "Critical settings updated successfully"
    })))
}

#[derive(Serialize, Deserialize)]
pub struct CriticalNotificationInfo {
    pub enabled: Option<String>,
    pub average_critical_per_day: f32,
    pub estimated_monthly_price: f32,
    pub call_notify: bool,
    pub action_on_critical_message: Option<String>,
}

pub async fn get_critical_settings(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<CriticalNotificationInfo>, (StatusCode, Json<serde_json::Value>)> {
    match state.user_core.get_critical_notification_info(auth_user.user_id) {
        Ok(info) => Ok(Json(info)),
        Err(e) => {
            tracing::error!("Failed to get critical notification info: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to get critical notification info: {}", e)})),
            ))
        }
    }
}


pub async fn update_proactive_agent_on(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<ProactiveAgentEnabledRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {

    // Update critical enabled setting
    match state.user_core.update_proactive_agent_on(auth_user.user_id, request.enabled) {
        Ok(_) => Ok(Json(json!({
            "message": "Proactive notifications setting updated successfully"
        }))),
        Err(e) => {
            tracing::error!("Failed to update proactive notifications setting: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to update proactive notifications setting: {}", e)}))
            ))
        }
    }
}

pub async fn get_proactive_agent_on(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<ProactiveAgentEnabledResponse>, (StatusCode, Json<serde_json::Value>)> {
    match state.user_core.get_proactive_agent_on(auth_user.user_id) {
        Ok(enabled) => {
            Ok(Json(ProactiveAgentEnabledResponse{
                enabled,
            }))
        },
        Err(e) => {
            tracing::error!("Failed to get critical enabled setting: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to get critical enabled setting: {}", e)}))
            ))
        }
    }
}

pub async fn delete_user(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    axum::extract::Path(user_id): axum::extract::Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Deleting user: {}", auth_user.user_id);

    if auth_user.user_id != user_id && !auth_user.is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "You can only delete your own account unless you're an admin"}))
        ));
    }
    
    // First verify the user exists
    match state.user_core.find_by_id(user_id) {
        Ok(Some(_)) => {
            tracing::debug!("user exists");
            // User exists, proceed with deletion
            match state.user_core.delete_user(user_id) {
                Ok(_) => {
                    tracing::info!("Successfully deleted user {}", user_id);
                    Ok(Json(json!({"message": "User deleted successfully"})))
                },
                Err(e) => {
                    tracing::error!("Failed to delete user {}: {}", user_id, e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Failed to delete user: {}", e)}))
                    ))
                }
            }
        },
        Ok(None) => {
            tracing::warn!("Attempted to delete non-existent user {}", user_id);
            Err((
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"}))
            ))
        },
        Err(e) => {
            tracing::error!("Database error while checking user {}: {}", user_id, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))
        }
    }
}

// Web Chat - allows users to test the AI assistant through the dashboard
const WEB_CHAT_COST_EUR: f32 = 0.01; // €0.01 per message for Euro countries
const WEB_CHAT_COST_US: f32 = 0.5; // 0.5 messages for US/CA (uses credits_left as message count)

#[derive(Deserialize)]
pub struct WebChatRequest {
    pub message: String,
}

#[derive(Serialize)]
pub struct WebChatResponse {
    pub message: String,
    pub credits_charged: f32,
}

pub async fn web_chat(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<WebChatRequest>,
) -> Result<Json<WebChatResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Get the user
    let user = state.user_core.find_by_id(auth_user.user_id)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
        ))?
        .ok_or_else(|| (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"}))
        ))?;

    // Check subscription - only subscribed users can use web chat
    if user.sub_tier.is_none() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Please subscribe to use the web chat feature"}))
        ));
    }

    // Determine cost based on region (US/CA uses message count, others use euro value)
    let is_us_or_ca = user.phone_number.starts_with("+1");
    let (credits_left_cost, credits_cost) = if is_us_or_ca {
        (WEB_CHAT_COST_US, WEB_CHAT_COST_EUR) // US: 0.5 from credits_left (message count), or €0.01 from credits
    } else {
        (WEB_CHAT_COST_EUR, WEB_CHAT_COST_EUR) // Euro: €0.01 from either
    };

    // Check if user has sufficient credits
    let has_credits = user.credits_left >= credits_left_cost || user.credits >= credits_cost;
    if !has_credits {
        return Err((
            StatusCode::PAYMENT_REQUIRED,
            Json(json!({"error": "Insufficient credits. Please add more credits to continue."}))
        ));
    }

    // Deduct credits (prefer credits_left, then credits)
    let charged_amount = if user.credits_left >= credits_left_cost {
        let new_credits_left = user.credits_left - credits_left_cost;
        state.user_repository.update_user_credits_left(auth_user.user_id, new_credits_left)
            .map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to charge credits: {}", e)}))
            ))?;
        credits_left_cost
    } else {
        let new_credits = user.credits - credits_cost;
        state.user_repository.update_user_credits(auth_user.user_id, new_credits)
            .map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to charge credits: {}", e)}))
            ))?;
        credits_cost
    };

    // Log the usage
    let _ = state.user_repository.log_usage(
        auth_user.user_id,
        None, // sid
        "web_chat".to_string(), // activity_type
        Some(charged_amount), // credits
        None, // time_consumed
        Some(true), // success
        Some(format!("Web chat message: {}", request.message.chars().take(50).collect::<String>())), // reason
        None, // status
        None, // recharge_threshold_timestamp
        None, // zero_credits_timestamp
    );

    // Create a mock Twilio payload to reuse existing SMS processing logic
    let mock_payload = crate::api::twilio_sms::TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: user.preferred_number.unwrap_or_else(|| "+0987654321".to_string()),
        body: request.message,
        num_media: None,
        media_url0: None,
        media_content_type0: None,
        message_sid: "".to_string(),
    };

    // Process using existing SMS handler with test mode (doesn't send actual SMS)
    let (status, _, response) = crate::api::twilio_sms::process_sms(
        &state,
        mock_payload,
        true, // test mode - don't send actual SMS
    ).await;

    if status == StatusCode::OK {
        Ok(Json(WebChatResponse {
            message: response.message.clone(),
            credits_charged: charged_amount,
        }))
    } else {
        // Refund the credits if processing failed
        if user.credits_left >= credits_left_cost {
            let _ = state.user_repository.update_user_credits_left(auth_user.user_id, user.credits_left);
        } else {
            let _ = state.user_repository.update_user_credits(auth_user.user_id, user.credits);
        }

        Err((
            status,
            Json(json!({
                "error": "Failed to process message",
                "details": response.message
            }))
        ))
    }
}

// On-demand "What's new?" digest endpoint
pub async fn get_instant_digest(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<WebChatResponse>, (StatusCode, Json<serde_json::Value>)> {
    use chrono::{Duration, Utc};
    use std::collections::{HashMap, HashSet};
    use crate::proactive::utils::{DigestData, MessageInfo, CalendarEvent, generate_digest};

    // Get the user
    let user = state.user_core.find_by_id(auth_user.user_id)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
        ))?
        .ok_or_else(|| (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"}))
        ))?;

    // Check subscription
    if user.sub_tier.is_none() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Please subscribe to use the digest feature"}))
        ));
    }

    // Get user info for timezone
    let user_info = state.user_core.get_user_info(auth_user.user_id)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to get user info: {}", e)}))
        ))?;

    let timezone = user_info.timezone.clone().unwrap_or_else(|| "UTC".to_string());
    let tz: chrono_tz::Tz = timezone.parse().unwrap_or(chrono_tz::UTC);

    // Charge same as web_chat
    let is_us_or_ca = user.phone_number.starts_with("+1");
    let (credits_left_cost, credits_cost) = if is_us_or_ca {
        (WEB_CHAT_COST_US, WEB_CHAT_COST_EUR)
    } else {
        (WEB_CHAT_COST_EUR, WEB_CHAT_COST_EUR)
    };

    let has_credits = user.credits_left >= credits_left_cost || user.credits >= credits_cost;
    if !has_credits {
        return Err((
            StatusCode::PAYMENT_REQUIRED,
            Json(json!({"error": "Insufficient credits. Please add more credits to continue."}))
        ));
    }

    // Deduct credits
    let charged_amount = if user.credits_left >= credits_left_cost {
        let new_credits_left = user.credits_left - credits_left_cost;
        state.user_repository.update_user_credits_left(auth_user.user_id, new_credits_left)
            .map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to charge credits: {}", e)}))
            ))?;
        credits_left_cost
    } else {
        let new_credits = user.credits - credits_cost;
        state.user_repository.update_user_credits(auth_user.user_id, new_credits)
            .map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to charge credits: {}", e)}))
            ))?;
        credits_cost
    };

    // Log usage
    let _ = state.user_repository.log_usage(
        auth_user.user_id,
        None,
        "instant_digest".to_string(),
        Some(charged_amount),
        None,
        Some(true),
        Some("On-demand digest request".to_string()),
        None,
        None,
        None,
    );

    // Calculate cutoff time - use last instant digest time or 12 hours ago
    let now = Utc::now();
    let last_instant_time = state.user_core.get_last_instant_digest_time(auth_user.user_id)
        .unwrap_or(None);

    let cutoff_timestamp = match last_instant_time {
        Some(ts) => ts as i64,
        None => (now - Duration::hours(12)).timestamp(),
    };
    let cutoff_time = chrono::DateTime::from_timestamp(cutoff_timestamp, 0)
        .unwrap_or(now - Duration::hours(12));

    // Collect messages from all sources
    let mut messages: Vec<MessageInfo> = Vec::new();

    // Fetch emails if IMAP is configured
    if let Ok(Some(_)) = state.user_repository.get_imap_credentials(auth_user.user_id) {
        if let Ok(emails) = crate::handlers::imap_handlers::fetch_emails_imap(&state, auth_user.user_id, false, Some(50), false, true).await {
            let email_msgs: Vec<MessageInfo> = emails.into_iter()
                .filter(|email| {
                    if let Some(date) = email.date {
                        date >= cutoff_time
                    } else {
                        false
                    }
                })
                .map(|email| MessageInfo {
                    sender: email.from.unwrap_or_else(|| "Unknown".to_string()),
                    content: email.snippet.unwrap_or_else(|| "No content".to_string()),
                    timestamp_rfc: email.date_formatted.unwrap_or_else(|| "No timestamp".to_string()),
                    platform: "email".to_string(),
                })
                .collect();
            messages.extend(email_msgs);
        }
    }

    // Fetch bridge messages (WhatsApp, Telegram, Signal)
    for bridge_type in &["whatsapp", "telegram", "signal"] {
        if let Ok(Some(_)) = state.user_repository.get_bridge(auth_user.user_id, bridge_type) {
            if let Ok(bridge_msgs) = crate::utils::bridge::fetch_bridge_messages(bridge_type, &state, auth_user.user_id, cutoff_timestamp, true).await {
                let infos: Vec<MessageInfo> = bridge_msgs.into_iter()
                    .map(|msg| MessageInfo {
                        sender: msg.room_name,
                        content: msg.content,
                        timestamp_rfc: msg.formatted_timestamp,
                        platform: bridge_type.to_string(),
                    })
                    .collect();
                messages.extend(infos);
            }
        }
    }

    // Fetch calendar events for next 24 hours
    let mut calendar_events: Vec<CalendarEvent> = Vec::new();
    if let Ok(true) = state.user_repository.has_active_google_calendar(auth_user.user_id) {
        let start_time = now.to_rfc3339();
        let end_time = (now + Duration::hours(24)).to_rfc3339();
        if let Ok(axum::Json(value)) = crate::handlers::google_calendar::handle_calendar_fetching(state.as_ref(), auth_user.user_id, &start_time, &end_time).await {
            if let Some(events) = value.get("events").and_then(|e| e.as_array()) {
                for event in events {
                    if let (Some(summary), Some(start), Some(duration)) = (
                        event.get("summary").and_then(|s| s.as_str()),
                        event.get("start").and_then(|s| s.as_str()),
                        event.get("duration_minutes").and_then(|d| d.as_str()),
                    ) {
                        calendar_events.push(CalendarEvent {
                            title: summary.to_string(),
                            start_time_rfc: start.to_string(),
                            duration_minutes: duration.parse().unwrap_or(60),
                        });
                    }
                }
            }
        }
    }

    // Check if there's anything to report
    if messages.is_empty() && calendar_events.is_empty() {
        // Update last instant digest time even if empty
        let _ = state.user_core.set_last_instant_digest_time(auth_user.user_id, now.timestamp() as i32);

        return Ok(Json(WebChatResponse {
            message: "Nothing new since your last check!".to_string(),
            credits_charged: charged_amount,
        }));
    }

    // Build priority map for digest generation
    let mut priority_map: HashMap<String, HashSet<String>> = HashMap::new();
    for platform in ["email", "whatsapp", "telegram", "signal"] {
        let priors = state.user_repository.get_priority_senders(auth_user.user_id, platform).unwrap_or(Vec::new());
        let set: HashSet<String> = priors.into_iter().map(|p| p.sender).collect();
        priority_map.insert(platform.to_string(), set);
    }

    // Sort messages
    messages.sort_by(|a, b| {
        let plat_cmp = a.platform.cmp(&b.platform);
        if plat_cmp == std::cmp::Ordering::Equal {
            let a_pri = priority_map.get(&a.platform).map_or(false, |set| set.contains(&a.sender));
            let b_pri = priority_map.get(&b.platform).map_or(false, |set| set.contains(&b.sender));
            b_pri.cmp(&a_pri).then_with(|| b.timestamp_rfc.cmp(&a.timestamp_rfc))
        } else {
            plat_cmp
        }
    });

    // Get current datetime in user's timezone
    let now_local = now.with_timezone(&tz);
    let current_datetime_local = now_local.format("%Y-%m-%d %H:%M:%S").to_string();

    // Calculate hours since cutoff
    let hours_since = ((now.timestamp() - cutoff_timestamp) / 3600) as u32;

    // Prepare digest data
    let digest_data = DigestData {
        messages,
        calendar_events,
        time_period_hours: hours_since.max(1),
        current_datetime_local,
    };

    // Generate digest
    let digest_message = match generate_digest(&state, digest_data, priority_map).await {
        Ok(digest) => digest,
        Err(e) => {
            tracing::error!("Failed to generate digest: {}", e);
            "Failed to generate digest. Please try again.".to_string()
        }
    };

    // Update last instant digest time
    let _ = state.user_core.set_last_instant_digest_time(auth_user.user_id, now.timestamp() as i32);

    Ok(Json(WebChatResponse {
        message: digest_message,
        credits_charged: charged_amount,
    }))
}


