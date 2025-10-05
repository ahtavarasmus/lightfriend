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
}

#[derive(Serialize)]
pub struct SubscriptionInfo {
    id: String,
    status: String,
    next_bill_date: i32,
    stage: String,
    is_scheduled_to_cancel: Option<bool>,
}

#[derive(Serialize)]
pub struct ProfileResponse {
    id: i32,
    phone_number: String,
    nickname: Option<String>,
    credits: f32,
    info: Option<String>,
    preferred_number: Option<String>,
    timezone: Option<String>,
    timezone_auto: Option<bool>,
    credits_left: f32,
    agent_language: String,
    notification_type: Option<String>,
    save_context: Option<i32>,
    twilio_sid: Option<String>,
    twilio_token: Option<String>,
    openrouter_api_key: Option<String>,
    textbee_device_id: Option<String>,
    textbee_api_key: Option<String>,
    estimated_monitoring_cost: f32,
    location: Option<String>,
    nearby_places: Option<String>,
    phone_number_country: Option<String>,
}
use crate::handlers::auth_middleware::AuthUser;


pub async fn get_profile(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<ProfileResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Get user profile and settings from database
    let user = state.user_core.get_user().map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Database error: {}", e)}))
    ))?;
    match user {
        Some(user) => {
            let mut phone_country = user.phone_number_country.clone();
            if phone_country.is_none() {
                match set_user_phone_country(&state, &user.phone_number).await {
                    Ok(c) => phone_country = c,
                    Err(e) => {
                        tracing::error!("Failed to set phone country: {}", e);
                    }
                }
            }
            let user_settings = state.user_core.get_user_settings().map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))?;
            let user_info = state.user_core.get_user_info().map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))?;
            let current_time = std::time::SystemTime::now()
                                                    .duration_since(std::time::UNIX_EPOCH)
                                                    .unwrap()
                                                    .as_secs() as i32;
            // Get current digest settings
            let (morning_digest_time, day_digest_time, evening_digest_time) = state.user_core.get_digests()
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
            // Fetch Twilio credentials and mask them
            let (twilio_sid, twilio_token, _) = match state.user_core.get_twilio_credentials() {
                Ok((sid, token, _)) => {
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
                    (Some(masked_sid), Some(masked_token), Some(""))
                },
                Err(_) => (None, None, None),
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
            // Get critical notification info
            let critical_info = state.user_core.get_critical_notification_info(auth_user.user_id).map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))?;
            let estimated_critical_monthly = critical_info.estimated_monthly_price;
            // Get priority notification info
            let priority_info = state.user_core.get_priority_notification_info().map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))?;
            let estimated_priority_monthly = priority_info.estimated_monthly_price;
            // Calculate digest estimated monthly cost
            let estimated_digest_monthly = if current_count > 0 {
                let active_count_f = current_count as f32;
                let cost_per_digest = if phone_country == Some("US".to_string()) {
                    0.5
                } else if phone_country == Some("Other".to_string()) {
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
                phone_number: user.phone_number,
                nickname: user.nickname,
                credits: user.credits,
                info: user_info.info,
                preferred_number: user.preferred_number,
                timezone: user_info.timezone,
                timezone_auto: user_settings.timezone_auto,
                credits_left: user.credits_left,
                agent_language: user_settings.agent_language,
                notification_type: user_settings.notification_type,
                save_context: user_settings.save_context,
                twilio_sid: twilio_sid,
                twilio_token: twilio_token,
                openrouter_api_key: openrouter_api_key,
                textbee_device_id: textbee_device_id,
                textbee_api_key: textbee_api_key,
                estimated_monitoring_cost,
                location: user_info.location,
                nearby_places: user_info.nearby_places,
                phone_number_country: phone_country,
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

#[derive(Deserialize)]
pub struct PreferredNumberRequest {
    preferred_number: String,
}

pub async fn update_preferred_number(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<PreferredNumberRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Get user and settings to check their subscription status
    let user = state.user_core.get_user()
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
        ))?
        .ok_or_else(|| (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"}))
        ))?;

    // TODO this should only be used when user uses their own twilio number. 

    // Update preferred number
    state.user_core.update_preferred_number(&request.preferred_number)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
    ))?;

    println!("Updated preferred number to: {}", request.preferred_number);
    Ok(Json(json!({
        "message": "Preferred number updated successfully"
    })))
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

use reqwest::Client;
use serde_json::Value;
use std::env;


pub async fn set_user_phone_country(state: &Arc<AppState>, phone_number: &str) -> Result<Option<String>, Box<dyn std::error::Error>> {
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

    println!("phone_number: {}, len: {}", phone_number, phone_number.len());
    if phone_number.starts_with("+1") {
        let area_code = phone_number.get(0..5).unwrap_or_default();
        println!("Extracted area code: {}", area_code);
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

    println!("country: {:#?}", country);

    if let Some(ref c) = country {
        state.user_core.update_phone_number_country(Some(c))?;
    } else {
        state.user_core.update_phone_number_country(None)?;
    }

    Ok(country)
}


pub async fn update_profile(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(update_req): Json<UpdateProfileRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Updating profile with notification type: {:?}", update_req.notification_type);
    use regex::Regex;
    let email_regex = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
    if !email_regex.is_match(&update_req.email) {
        println!("Invalid email format: {}", update_req.email);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid email format"}))
        ));
    }
   
    let phone_regex = Regex::new(r"^\+[1-9]\d{1,14}$").unwrap();
    if !phone_regex.is_match(&update_req.phone_number) {
        println!("Invalid phone number format: {}", update_req.phone_number);
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
    ) {
        Ok(_) => {
            if let Err(e) = state.user_core.update_agent_language(auth_user.user_id, &update_req.agent_language) {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to update agent language: {}", e)}))
                ));
            }
            // Set phone country after update
            if let Err(e) = set_user_phone_country(&state, &update_req.phone_number).await {
                tracing::error!("Failed to set phone country after profile update: {}", e);
                // Continue anyway, as it's non-critical
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
    let (morning_digest_time, day_digest_time, evening_digest_time) = state.user_core.get_digests()
        .map_err(|e| {
            tracing::error!("Failed to get digest settings: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to get digest settings: {}", e)}))
            )
        })?;

    // Count current active digests
    let current_count = [morning_digest_time.as_ref(), day_digest_time.as_ref(), evening_digest_time.as_ref()]
        .iter()
        .filter(|&&x| x.is_some())
        .count();


    // Get user for credit check
    let user = state.user_core.get_user()
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to get user: {}", e)}))
        ))?
        .ok_or_else(|| (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"}))
        ))?;

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
    // Get current digest settings
    let (current_morning, current_day, current_evening) = state.user_core.get_digests()
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to get current digest settings: {}", e)}))
        ))?;
    // Count current active digests
    let current_count = [current_morning.as_ref(), current_day.as_ref(), current_evening.as_ref()]
        .iter()
        .filter(|&&x| x.is_some())
        .count();
    // Count new active digests
    let new_count = [request.morning_digest_time.as_ref(), request.day_digest_time.as_ref(), request.evening_digest_time.as_ref()]
        .iter()
        .filter(|&&x| x.is_some())
        .count();
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
                "digests_changed": new_count != current_count,
                "previous_digest_count": current_count,
                "new_digest_count": new_count,
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
    enabled: Option<Option<String>>,
    call_notify: Option<bool>,
    action_on_critical_message: Option<Option<String>>,
}

pub async fn update_critical_settings(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<UpdateCriticalRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if let Some(enabled) = request.enabled {
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
    match state.user_core.get_proactive_agent_on() {
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

