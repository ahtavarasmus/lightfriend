use crate::AppState;
use std::sync::Arc;
use reqwest::Client;

// Helper to check mock mode
fn is_mock_mode() -> bool {
    std::env::var("TWILIO_MOCK_MODE")
        .unwrap_or_default()
        .to_lowercase() == "true"
}

/// Revoke access to a subaccount but keep it in the pool (for US numbers)
/// Regenerates the auth token to invalidate old credentials and marks as available
pub async fn revoke_subaccount_access(
    state: &Arc<AppState>,
    subaccount_id: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get subaccount from database
    let subaccount = state.user_core.find_subaccount_by_id(subaccount_id)?
        .ok_or("Subaccount not found")?;

    tracing::info!("Revoking access to subaccount {} (will be returned to pool)", subaccount.subaccount_sid);

    // Mock mode: Just update DB
    if is_mock_mode() {
        use uuid::Uuid;
        let new_mock_token = format!("mock_token_revoked_{}", &Uuid::new_v4().to_string().replace("-", "")[..24]);

        state.user_core.update_subaccount_user_and_token(
            subaccount_id,
            "-1".to_string(),
            new_mock_token,
            "available",
        )?;

        tracing::info!("MOCK MODE: Revoked access to subaccount {} and returned to pool", subaccount.subaccount_sid);
        return Ok(());
    }

    // Real mode: Regenerate Twilio auth token
    let main_account_sid = std::env::var("TWILIO_ACCOUNT_SID")?;
    let main_auth_token = std::env::var("TWILIO_AUTH_TOKEN")?;
    let client = Client::new();

    // Regenerate auth token (this invalidates the old one)
    let update_url = format!(
        "https://api.twilio.com/2010-04-01/Accounts/{}.json",
        subaccount.subaccount_sid
    );

    let response = client
        .post(&update_url)
        .basic_auth(&main_account_sid, Some(&main_auth_token))
        .header(reqwest::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .form(&[("Status", "active")]) // Trigger token regeneration
        .send()
        .await?;

    if !response.status().is_success() {
        let error_body = response.text().await?;
        return Err(format!("Failed to regenerate subaccount token: {}", error_body).into());
    }

    let json: serde_json::Value = response.json().await?;
    let new_auth_token = json["auth_token"]
        .as_str()
        .ok_or("Missing new auth_token in response")?
        .to_string();

    // Update database: set user_id to "-1", update token, set status to "available"
    state.user_core.update_subaccount_user_and_token(
        subaccount_id,
        "-1".to_string(),
        new_auth_token,
        "available",
    )?;

    tracing::info!("Successfully revoked access to subaccount {} and returned to pool", subaccount.subaccount_sid);
    Ok(())
}

/// Fully release a non-US subaccount (delete phone number, delete subaccount, remove from DB)
pub async fn release_non_us_subaccount(
    state: &Arc<AppState>,
    subaccount_id: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get subaccount from database
    let subaccount = state.user_core.find_subaccount_by_id(subaccount_id)?
        .ok_or("Subaccount not found")?;

    tracing::info!(
        "Releasing non-US subaccount {} with number {:?} (country: {:?})",
        subaccount.subaccount_sid,
        subaccount.number,
        subaccount.country
    );

    // Mock mode: Just delete from DB
    if is_mock_mode() {
        state.user_core.delete_subaccount(subaccount_id)?;
        tracing::info!("MOCK MODE: Deleted non-US subaccount {} from database", subaccount.subaccount_sid);
        return Ok(());
    }

    // Real mode: Delete from Twilio, then DB
    let main_account_sid = std::env::var("TWILIO_ACCOUNT_SID")?;
    let main_auth_token = std::env::var("TWILIO_AUTH_TOKEN")?;
    let client = Client::new();

    // Note: Deleting the subaccount will also release any phone numbers associated with it
    let delete_url = format!(
        "https://api.twilio.com/2010-04-01/Accounts/{}.json",
        subaccount.subaccount_sid
    );

    let response = client
        .delete(&delete_url)
        .basic_auth(&main_account_sid, Some(&main_auth_token))
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 404 => {
            // Delete from database
            state.user_core.delete_subaccount(subaccount_id)?;
            tracing::info!("Successfully released non-US subaccount {}", subaccount.subaccount_sid);
            Ok(())
        }
        Ok(resp) => {
            let error_body = resp.text().await.unwrap_or_default();
            Err(format!("Failed to delete subaccount from Twilio: {}", error_body).into())
        }
        Err(e) => {
            Err(format!("Failed to connect to Twilio: {}", e).into())
        }
    }
}

/// Handle tier 3 subscription cancellation
/// - US subaccounts: Revoke access and return to pool, cleanup if pool > 10
/// - Non-US subaccounts: Fully release (delete from Twilio and DB)
pub async fn handle_tier3_cancellation(
    state: &Arc<AppState>,
    user_id: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    // Find user's active subaccount
    let subaccount = state.user_core.find_subaccount_by_user_id(user_id)?
        .ok_or("No active subaccount found for user")?;

    let country = subaccount.country.clone().unwrap_or_default();
    let subaccount_id = subaccount.id;

    tracing::info!(
        "Handling tier 3 cancellation for user {} (subaccount {}, country: {})",
        user_id,
        subaccount.subaccount_sid,
        country
    );

    if country == "US" {
        // US: Revoke access and return to pool
        revoke_subaccount_access(state, subaccount_id).await?;

        // Check if we need to cleanup excess numbers
        let free_count = state.user_core.count_free_us_subaccounts()? as usize;
        if free_count > 10 {
            tracing::info!("US pool now has {} numbers, triggering cleanup", free_count);
            crate::utils::us_number_pool::cleanup_excess_us_numbers(state).await?;
        }
    } else {
        // Non-US: Fully release
        release_non_us_subaccount(state, subaccount_id).await?;
    }

    Ok(())
}

/// Regenerate Tinfoil API key for subscription renewal
pub async fn regenerate_tinfoil_key(
    state: &Arc<AppState>,
    user_id: i32,
    new_expiry: i64,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    tracing::info!("Regenerating Tinfoil API key for user {} with new expiry {}", user_id, new_expiry);

    // Create new temporary key
    let new_key = crate::handlers::self_host_handlers::create_temp_tinfoil_api_key(
        user_id.to_string(),
        new_expiry,
    ).await.map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
        Box::new(std::io::Error::other(e.to_string()))
    })?;

    // Update subaccount's tinfoil_key in database
    let subaccount = state.user_core.find_subaccount_by_user_id(user_id)
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
            Box::new(std::io::Error::other(e.to_string()))
        })?
        .ok_or_else(|| -> Box<dyn std::error::Error + Send + Sync> {
            Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, "No active subaccount found for user"))
        })?;

    // Update the tinfoil_key field using repository method
    state.user_core.update_subaccount_tinfoil_key(subaccount.id, &new_key)
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
            Box::new(std::io::Error::other(e.to_string()))
        })?;

    tracing::info!("Successfully regenerated Tinfoil API key for user {}", user_id);
    Ok(new_key)
}
