use crate::UserCoreOps;
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::repositories::user_repository::UsageDataPoint;
use crate::{handlers::auth_middleware::AuthUser, AppState};

#[derive(Deserialize)]
pub struct AutoTopupSettings {
    pub active: bool,
    pub amount: Option<f32>,
}

#[derive(Serialize)]
pub struct OverageStatus {
    pub billing_system: &'static str,
    pub provisioned: bool,
    pub overage_enabled: bool,
    pub payment_ready: bool,
    pub usage_entitled: bool,
    pub charge_threshold_usd: i32,
    pub invoice_cadence: &'static str,
    pub consent_version: &'static str,
    pub available_usage_usd: Option<f64>,
    pub resets_at: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateOverageRequest {
    pub enabled: bool,
    pub consent_version: Option<String>,
}

pub async fn get_overage_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<OverageStatus>, (StatusCode, Json<serde_json::Value>)> {
    let user = state
        .user_core
        .find_by_id(auth_user.user_id)
        .map_err(internal_billing_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"})),
            )
        })?;
    let repository = crate::BillingRepository::new(state.pg_pool.clone());
    let mut account = repository
        .ensure_account(user.id)
        .map_err(internal_billing_error)?;
    if crate::services::metronome_billing::metronome_enabled()
        && user.sub_tier.as_deref() == Some("tier 2")
        && (account.provisioning_status != "provisioned" || !account.payment_ready)
    {
        match crate::services::metronome_billing::provision_user(&state, &user).await {
            Ok(provisioned) => account = provisioned,
            Err(error) => {
                tracing::error!("Failed to provision billing account {}: {}", user.id, error);
                account = repository
                    .get_account(user.id)
                    .map_err(internal_billing_error)?
                    .unwrap_or(account);
            }
        }
    }
    let balance = if crate::services::metronome_billing::metronome_enabled()
        && account.provisioning_status == "provisioned"
    {
        match crate::services::metronome_billing::MetronomeClient::from_env() {
            Ok(client) => match client.customer_usage_balance(&account).await {
                Ok(balance) => Some(balance),
                Err(error) => {
                    tracing::warn!(user_id = user.id, "Failed to load usage balance: {error}");
                    None
                }
            },
            Err(error) => {
                tracing::warn!(
                    user_id = user.id,
                    "Failed to load Metronome client: {error}"
                );
                None
            }
        }
    } else {
        None
    };
    Ok(Json(overage_status(&account, balance.as_ref())))
}

pub async fn update_overage(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<UpdateOverageRequest>,
) -> Result<Json<OverageStatus>, (StatusCode, Json<serde_json::Value>)> {
    if !crate::services::metronome_billing::metronome_enabled() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "Usage billing has not been activated yet"})),
        ));
    }
    if request.enabled
        && request.consent_version.as_deref()
            != Some(crate::services::metronome_billing::OVERAGE_CONSENT_VERSION)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Please accept the current overage billing terms"})),
        ));
    }
    let user = state
        .user_core
        .find_by_id(auth_user.user_id)
        .map_err(internal_billing_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"})),
            )
        })?;
    if user.sub_tier.as_deref() != Some("tier 2") || state.user_core.is_byot_user(user.id) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Overage billing requires an active hosted plan"})),
        ));
    }

    let account = crate::services::metronome_billing::provision_user(&state, &user)
        .await
        .map_err(|error| {
            tracing::error!("Failed to provision billing account {}: {}", user.id, error);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "Billing is temporarily unavailable"})),
            )
        })?;
    if request.enabled && !account.payment_ready {
        return Err((
            StatusCode::PAYMENT_REQUIRED,
            Json(json!({
                "error": "Add or update your default payment method before enabling overage",
                "payment_method_required": true
            })),
        ));
    }

    let client =
        crate::services::metronome_billing::MetronomeClient::from_env().map_err(|error| {
            tracing::error!("Metronome configuration error: {}", error);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "Billing is temporarily unavailable"})),
            )
        })?;
    client
        .set_overage(&account, request.enabled)
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to update Metronome overage for {}: {}",
                user.id,
                error
            );
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "Could not update overage billing"})),
            )
        })?;

    let repository = crate::BillingRepository::new(state.pg_pool.clone());
    repository
        .set_overage(
            user.id,
            request.enabled,
            request
                .enabled
                .then_some(crate::services::metronome_billing::OVERAGE_CONSENT_VERSION),
        )
        .map_err(internal_billing_error)?;
    if request.enabled {
        repository
            .set_usage_entitled(user.id, true)
            .map_err(internal_billing_error)?;
    }
    let account = repository
        .get_account(user.id)
        .map_err(internal_billing_error)?
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Billing account not found"})),
            )
        })?;
    let balance = client.customer_usage_balance(&account).await.ok();
    Ok(Json(overage_status(&account, balance.as_ref())))
}

fn overage_status(
    account: &crate::pg_models::BillingAccount,
    balance: Option<&crate::services::metronome_billing::CustomerUsageBalance>,
) -> OverageStatus {
    OverageStatus {
        billing_system: if crate::services::metronome_billing::metronome_enabled() {
            "metronome"
        } else {
            "legacy"
        },
        provisioned: account.provisioning_status == "provisioned",
        overage_enabled: account.overage_enabled,
        payment_ready: account.payment_ready,
        usage_entitled: account.usage_entitled,
        charge_threshold_usd: 10,
        invoice_cadence: "weekly",
        consent_version: crate::services::metronome_billing::OVERAGE_CONSENT_VERSION,
        available_usage_usd: balance.map(|value| value.available_usage_usd),
        resets_at: balance.and_then(|value| value.resets_at.clone()),
    }
}

fn internal_billing_error(error: impl std::fmt::Display) -> (StatusCode, Json<serde_json::Value>) {
    tracing::error!("Billing database error: {}", error);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": "Billing database error"})),
    )
}

pub async fn metronome_webhook(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    let client = crate::services::metronome_billing::MetronomeClient::from_env().map_err(|_| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "Billing webhook is not configured"})),
        )
    })?;
    if client.webhook_secret().is_empty() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "Billing webhook secret is not configured"})),
        ));
    }
    let date = headers
        .get("date")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Missing Date header"})),
            )
        })?;
    let signature = headers
        .get("metronome-webhook-signature")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Missing webhook signature"})),
            )
        })?;
    crate::services::metronome_billing::verify_webhook_signature(
        client.webhook_secret(),
        date,
        &body,
        signature,
        chrono::Utc::now().timestamp(),
    )
    .map_err(|error| {
        tracing::warn!("Rejected Metronome webhook: {}", error);
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid or stale webhook signature"})),
        )
    })?;

    let payload: serde_json::Value = serde_json::from_slice(&body).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid JSON"})),
        )
    })?;
    let event_id = payload["id"].as_str().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Missing event ID"})),
        )
    })?;
    let event_type = payload["type"].as_str().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Missing event type"})),
        )
    })?;
    let repository = crate::BillingRepository::new(state.pg_pool.clone());
    if repository
        .webhook_seen(event_id)
        .map_err(internal_billing_error)?
    {
        return Ok(StatusCode::OK);
    }
    let customer_id = payload["properties"]["customer_id"]
        .as_str()
        .or_else(|| payload["customer_id"].as_str());
    let Some(customer_id) = customer_id else {
        repository
            .record_webhook_once(event_id, event_type)
            .map_err(internal_billing_error)?;
        return Ok(StatusCode::OK);
    };
    let Some(account) = repository
        .find_by_metronome_customer_id(customer_id)
        .map_err(internal_billing_error)?
    else {
        tracing::warn!("Metronome webhook for unknown customer {}", customer_id);
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "Billing customer is still provisioning"})),
        ));
    };

    match event_type {
        "payment_gate.payment_status" => {
            let paid = payload["properties"]["payment_status"].as_str() == Some("paid");
            repository
                .set_payment_ready(account.user_id, paid)
                .map_err(internal_billing_error)?;
            repository
                .set_usage_entitled(account.user_id, paid && account.overage_enabled)
                .map_err(internal_billing_error)?;
            if !paid {
                repository
                    .set_overage(account.user_id, false, None)
                    .map_err(internal_billing_error)?;
            }
        }
        "payment_gate.payment_pending_action_required" | "invoice.billing_provider_error" => {
            repository
                .set_payment_ready(account.user_id, false)
                .map_err(internal_billing_error)?;
            repository
                .set_usage_entitled(account.user_id, false)
                .map_err(internal_billing_error)?;
            repository
                .set_overage(account.user_id, false, None)
                .map_err(internal_billing_error)?;
        }
        kind if kind.starts_with("alerts.low_remaining_") && kind.ends_with("_resolved") => {
            repository
                .set_usage_entitled(account.user_id, true)
                .map_err(internal_billing_error)?;
            let _ = state
                .user_core
                .clear_last_credits_notification(account.user_id);
        }
        kind if kind.starts_with("alerts.low_remaining_") => {
            let remaining_cents = payload["properties"]["remaining_balance"]
                .as_f64()
                .or_else(|| {
                    payload["properties"]["remaining_balance"]
                        .as_i64()
                        .map(|value| value as f64)
                })
                .unwrap_or(0.0);
            let exhausted = remaining_cents <= 0.0;
            let entitled = !exhausted || (account.overage_enabled && account.payment_ready);
            if exhausted {
                repository
                    .set_usage_entitled(account.user_id, entitled)
                    .map_err(internal_billing_error)?;
            }
            if let Ok(Some(user)) = state.user_core.find_by_id(account.user_id) {
                let reset_date = crate::services::metronome_billing::customer_reset_date_label(
                    &state,
                    account.user_id,
                )
                .await
                .unwrap_or_else(|| "your billing date".to_string());
                let message = if !exhausted {
                    format!(
                        "${:.2} left. Resets {}.",
                        remaining_cents / 100.0,
                        reset_date
                    )
                } else if entitled {
                    format!("$25 used. Overage on. Resets {}.", reset_date)
                } else {
                    format!("$25 used. Resets {}. Enable overage?", reset_date)
                };
                let _ = state.user_core.update_last_credits_notification(
                    account.user_id,
                    chrono::Utc::now().timestamp() as i32,
                );
                let state = state.clone();
                tokio::spawn(async move {
                    if let Err(error) = state
                        .channel_router
                        .send_to_user(&user, &message, None)
                        .await
                    {
                        tracing::warn!(
                            user_id = user.id,
                            "Failed to send monthly usage notice: {error}"
                        );
                    }
                });
            }
        }
        _ => {}
    }
    repository
        .record_webhook_once(event_id, event_type)
        .map_err(internal_billing_error)?;
    Ok(StatusCode::OK)
}

#[derive(Deserialize)]
pub struct UsageDataRequest {
    pub from: i32,
}

pub async fn get_usage_data(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<UsageDataRequest>,
) -> Result<Json<Vec<UsageDataPoint>>, (StatusCode, Json<serde_json::Value>)> {
    println!("in get_usage_data route");

    // Get usage data using the provided 'from' timestamp
    let usage_data = state
        .user_repository
        .get_usage_data(auth_user.user_id, request.from)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?;

    Ok(Json(usage_data))
}

pub async fn reset_credits(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    axum::extract::Path(user_id): axum::extract::Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Check if user is an admin
    if !auth_user.is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Only admins can reset credits"})),
        ));
    }

    // Reset user's credits to zero in database
    state
        .user_repository
        .update_user_credits(user_id, 0.00)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?;

    Ok(Json(json!({
        "message": "credits reset successfully"
    })))
}

pub async fn increase_credits(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    axum::extract::Path(user_id): axum::extract::Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if crate::services::metronome_billing::metronome_enabled() {
        return Err((
            StatusCode::GONE,
            Json(
                json!({"error": "Credit purchases have been replaced by optional overage billing"}),
            ),
        ));
    }
    // Check if user is modifying their own credits or is an admin
    if auth_user.user_id != user_id && !auth_user.is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "You can only modify your own credits unless you're an admin"})),
        ));
    }

    // Update user's credits in database
    state
        .user_repository
        .increase_credits(user_id, 1.00)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?;

    Ok(Json(json!({
        "message": "credits increased successfully"
    })))
}

#[derive(Serialize)]
pub struct UsageLogEntry {
    pub activity_type: String,
    pub credits: Option<f32>,
    pub created_at: i32,
    pub call_duration: Option<i32>,
}

pub async fn get_recent_usage(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<Vec<UsageLogEntry>>, (StatusCode, Json<serde_json::Value>)> {
    // Get the current included-usage window for usage feed filtering.
    let user = state
        .user_core
        .find_by_id(auth_user.user_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"})),
        ))?;
    let user = crate::utils::usage::ensure_current_included_usage_window(&state, &user)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;
    let usage_window_start = user
        .included_usage_window_start_timestamp
        .unwrap_or(now - crate::utils::usage::INCLUDED_USAGE_WINDOW_SECONDS);

    let logs = state
        .user_repository
        .get_recent_usage_logs(auth_user.user_id, usage_window_start, 50)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?;

    let entries: Vec<UsageLogEntry> = logs
        .into_iter()
        .map(|log| UsageLogEntry {
            activity_type: log.activity_type,
            credits: log.credits,
            created_at: log.created_at,
            call_duration: log.call_duration,
        })
        .collect();

    Ok(Json(entries))
}

pub async fn update_topup(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(settings): Json<AutoTopupSettings>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if crate::services::metronome_billing::metronome_enabled() {
        return Err((
            StatusCode::GONE,
            Json(json!({"error": "Auto top-up has been replaced by overage billing"})),
        ));
    }
    // Update the user's auto-topup settings with fixed threshold of 3.00
    match state
        .user_core
        .update_auto_topup(auth_user.user_id, settings.active, settings.amount)
    {
        Ok(_) => Ok(Json(json!({
            "success": true,
            "message": "Auto top-up settings updated successfully"
        }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(
                json!({"success": false, "message": format!("Failed to update auto top-up settings: {}", e)}),
            ),
        )),
    }
}
