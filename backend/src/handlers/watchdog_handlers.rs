use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json};
use serde_json::json;
use std::sync::Arc;

use crate::AppState;

/// Validates the X-Watchdog-Key header against the WATCHDOG_API_KEY env var.
fn check_watchdog_key(headers: &HeaderMap) -> bool {
    let expected = match std::env::var("WATCHDOG_API_KEY") {
        Ok(s) if !s.is_empty() => s,
        _ => return false,
    };
    headers
        .get("X-Watchdog-Key")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == expected)
}

/// GET /api/watchdog/bridge/health
///
/// Comprehensive health check for all users' WhatsApp bridges.
/// Returns aggregate status + per-user check details.
pub async fn bridge_health_check(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !check_watchdog_key(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response();
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    // Get all users with active bridges
    let users_with_bridges = match state.user_repository.get_users_with_active_bridges() {
        Ok(m) => m,
        Err(e) => {
            tracing::error!("Watchdog: failed to get active bridges: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "overall_status": "unhealthy",
                    "timestamp": now,
                    "error": format!("Failed to query bridges: {}", e)
                })),
            )
                .into_response();
        }
    };

    let mut checks_detail = Vec::new();
    let mut healthy_count = 0;
    let mut total_count = 0;

    for (user_id, bridges) in &users_with_bridges {
        for bridge in bridges {
            if bridge.bridge_type != "whatsapp" {
                continue;
            }
            total_count += 1;

            let user_checks = run_bridge_checks(&state, *user_id, bridge, now).await;
            let bridge_status = derive_status(&user_checks);

            if bridge_status == "healthy" {
                healthy_count += 1;
            }

            checks_detail.push(json!({
                "user_id": user_id,
                "bridge_type": "whatsapp",
                "status": bridge_status,
                "checks": user_checks,
            }));
        }
    }

    let overall_status = if total_count == 0 {
        "no_bridges"
    } else if healthy_count == total_count {
        "healthy"
    } else if healthy_count > 0 {
        "degraded"
    } else {
        "unhealthy"
    };

    Json(json!({
        "overall_status": overall_status,
        "timestamp": now,
        "total_bridges": total_count,
        "healthy": healthy_count,
        "unhealthy": total_count - healthy_count,
        "checks_detail": checks_detail,
    }))
    .into_response()
}

/// Run all health checks for a single bridge.
async fn run_bridge_checks(
    state: &Arc<AppState>,
    user_id: i32,
    bridge: &crate::pg_models::PgBridge,
    now: i64,
) -> serde_json::Value {
    // Check 1: Bridge record
    let bridge_record_check = json!({
        "status": "pass",
        "detail": format!("Bridge exists, status={}", bridge.status),
    });

    // Check 2: Matrix client
    let (matrix_check, client) =
        match crate::utils::matrix_auth::get_cached_client(user_id, state).await {
            Ok(c) => (
                json!({
                    "status": "pass",
                    "detail": "Matrix client available",
                }),
                Some(c),
            ),
            Err(e) => (
                json!({
                    "status": "fail",
                    "detail": format!("Failed to get Matrix client: {}", e),
                }),
                None,
            ),
        };

    // Check 3: Room access (only if we have a client)
    let room_access_check = if let Some(ref client) = client {
        match crate::utils::bridge::get_service_rooms(client, "whatsapp").await {
            Ok(rooms) => {
                let count = rooms.len();
                if count > 0 {
                    json!({
                        "status": "pass",
                        "room_count": count,
                        "detail": format!("{} WhatsApp rooms accessible", count),
                    })
                } else {
                    json!({
                        "status": "warn",
                        "room_count": 0,
                        "detail": "No WhatsApp rooms found (bridge may be connected but empty)",
                    })
                }
            }
            Err(e) => json!({
                "status": "fail",
                "room_count": 0,
                "detail": format!("Failed to fetch rooms: {}", e),
            }),
        }
    } else {
        json!({
            "status": "fail",
            "room_count": 0,
            "detail": "Skipped - no Matrix client",
        })
    };

    // Check 4: Management room messages
    let management_room_check =
        if let (Some(ref client), Some(ref room_id)) = (&client, &bridge.room_id) {
            match crate::utils::bridge::read_management_room_messages(client, room_id, 20).await {
                Ok(messages) => {
                    let bot_messages: Vec<_> = messages.iter().filter(|m| m.is_from_bot).collect();

                    // Use is_disconnection_message on the fly to detect problems
                    let one_hour_ago = now - 3600;
                    let recent_disconnections = bot_messages
                        .iter()
                        .filter(|m| {
                            m.timestamp >= one_hour_ago
                                && crate::utils::bridge::is_disconnection_message(&m.body)
                        })
                        .count();

                    let last_bot_msg = bot_messages.first();
                    let last_bot_message_age = last_bot_msg.map(|m| now - m.timestamp);

                    let recent_texts: Vec<&str> = bot_messages
                        .iter()
                        .take(5)
                        .map(|m| m.body.as_str())
                        .collect();

                    let status = if recent_disconnections > 0 {
                        "fail"
                    } else {
                        "pass"
                    };

                    json!({
                        "status": status,
                        "last_bot_message_age_secs": last_bot_message_age,
                        "recent_disconnections": recent_disconnections,
                        "recent_bot_messages": recent_texts,
                        "detail": if recent_disconnections > 0 {
                            format!("{} disconnection messages in last hour", recent_disconnections)
                        } else {
                            "No disconnection messages in last hour".to_string()
                        },
                    })
                }
                Err(e) => json!({
                    "status": "warn",
                    "detail": format!("Failed to read management room: {}", e),
                }),
            }
        } else {
            json!({
                "status": "warn",
                "detail": if client.is_none() {
                    "Skipped - no Matrix client"
                } else {
                    "No management room ID stored"
                },
            })
        };

    // Check 5: Data freshness
    let data_freshness_check = {
        let last_seen_age = bridge
            .last_seen_online
            .map(|ts| now - ts as i64)
            .unwrap_or(i64::MAX);

        let status = if last_seen_age < 7200 {
            "pass"
        } else if last_seen_age < 86400 {
            "warn"
        } else {
            "fail"
        };

        json!({
            "status": status,
            "last_seen_online_age_secs": if last_seen_age == i64::MAX { None } else { Some(last_seen_age) },
            "detail": if last_seen_age == i64::MAX {
                "No last_seen_online timestamp".to_string()
            } else {
                format!("Last seen {} seconds ago", last_seen_age)
            },
        })
    };

    json!({
        "bridge_record": bridge_record_check,
        "matrix_client": matrix_check,
        "room_access": room_access_check,
        "management_room": management_room_check,
        "data_freshness": data_freshness_check,
    })
}

/// Derive overall status from individual check results.
fn derive_status(checks: &serde_json::Value) -> &'static str {
    let check_names = [
        "bridge_record",
        "matrix_client",
        "room_access",
        "management_room",
        "data_freshness",
    ];

    let mut has_fail = false;
    let mut has_warn = false;

    for name in &check_names {
        if let Some(status) = checks
            .get(name)
            .and_then(|c| c.get("status"))
            .and_then(|s| s.as_str())
        {
            match status {
                "fail" => has_fail = true,
                "warn" => has_warn = true,
                _ => {}
            }
        }
    }

    if has_fail {
        "unhealthy"
    } else if has_warn {
        "degraded"
    } else {
        "healthy"
    }
}

/// POST /api/watchdog/bridge/test-send
///
/// Send a test message to the admin's Note to Self room via WhatsApp bridge.
/// Rate-limited to once every 2 hours. User 1 only.
pub async fn bridge_send_test(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !check_watchdog_key(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response();
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let user_id = 1;
    let bridge_type = "whatsapp";

    // Rate limit: check if last test was within 2 hours
    let two_hours_ago = now - 7200;
    if let Ok(recent_logs) =
        state
            .user_repository
            .get_watchdog_logs_since(user_id, bridge_type, two_hours_ago)
    {
        let recent_test = recent_logs
            .iter()
            .any(|l| l.event_type == "test_send_ok" || l.event_type == "test_send_fail");
        if recent_test {
            return Json(json!({
                "status": "skipped",
                "skipped": true,
                "skip_reason": "Last test was within 2 hours",
            }))
            .into_response();
        }
    }

    // Get the phone number for Note to Self
    let phone_number = match std::env::var("WATCHDOG_TEST_PHONE") {
        Ok(p) if !p.is_empty() => p,
        _ => {
            return Json(json!({
                "status": "skipped",
                "skipped": true,
                "skip_reason": "WATCHDOG_TEST_PHONE env var not set",
            }))
            .into_response();
        }
    };

    // Send test message
    let test_message = format!("Watchdog ping {}", now);
    let result = crate::utils::bridge::send_bridge_message(
        bridge_type,
        &state,
        user_id,
        &phone_number,
        &test_message,
        None,
        None,
    )
    .await;

    // Compute bridge age for metadata
    let bridge_age = state
        .user_repository
        .get_bridge(user_id, bridge_type)
        .ok()
        .flatten()
        .and_then(|b| b.created_at.map(|c| now - c));

    let metadata = json!({
        "bridge_age_secs": bridge_age,
        "phone": phone_number,
    })
    .to_string();

    match result {
        Ok(_) => {
            let _ = state.user_repository.insert_watchdog_log(
                crate::pg_models::NewPgBridgeWatchdogLog {
                    user_id,
                    bridge_type: bridge_type.to_string(),
                    event_type: "test_send_ok".to_string(),
                    message: test_message,
                    metadata: Some(metadata),
                    created_at: now,
                },
            );

            Json(json!({
                "status": "pass",
                "skipped": false,
                "detail": "Test message sent successfully",
            }))
            .into_response()
        }
        Err(e) => {
            let error_msg = format!("Test send failed: {}", e);
            let _ = state.user_repository.insert_watchdog_log(
                crate::pg_models::NewPgBridgeWatchdogLog {
                    user_id,
                    bridge_type: bridge_type.to_string(),
                    event_type: "test_send_fail".to_string(),
                    message: error_msg.clone(),
                    metadata: Some(metadata),
                    created_at: now,
                },
            );

            Json(json!({
                "status": "fail",
                "skipped": false,
                "detail": error_msg,
            }))
            .into_response()
        }
    }
}
