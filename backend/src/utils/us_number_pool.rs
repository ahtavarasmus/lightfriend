use crate::AppState;
use std::sync::Arc;
use reqwest::Client;
use crate::models::user_models::NewSubaccount;

// Helper to check mock mode
fn is_mock_mode() -> bool {
    std::env::var("TWILIO_MOCK_MODE")
        .unwrap_or_default()
        .to_lowercase() == "true"
}

/// Buy a single US TollFree number and create subaccount for the pool
async fn buy_us_number_for_pool(
    state: &Arc<AppState>,
) -> Result<(), Box<dyn std::error::Error>> {
    let main_account_sid = std::env::var("TWILIO_ACCOUNT_SID")?;
    let main_auth_token = std::env::var("TWILIO_AUTH_TOKEN")?;
    let client = Client::new();

    // Mock mode: Create fake subaccount
    if is_mock_mode() {
        use uuid::Uuid;
        let mock_sid = format!("AC_pool_mock_{}", &Uuid::new_v4().to_string().replace("-", "")[..16]);
        let mock_token = format!("mock_token_{}", &Uuid::new_v4().to_string().replace("-", "")[..24]);
        let mock_phone = format!("+1555POOL{:04}", rand::random::<u16>() % 10000);

        let created_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i32,
        );

        let new_subaccount = NewSubaccount {
            user_id: "-1".to_string(),
            subaccount_sid: mock_sid.clone(),
            auth_token: mock_token,
            country: Some("US".to_string()),
            number: Some(mock_phone.clone()),
            cost_this_month: Some(0.0),
            created_at,
            status: Some("available".to_string()),
            tinfoil_key: None,
            messaging_service_sid: std::env::var("TWILIO_MESSAGING_SERVICE_SID").ok(),
            subaccount_type: "us_ca".to_string(), // Pool numbers are US
            country_code: Some("US".to_string()),
        };

        state.user_core.insert_subaccount(&new_subaccount)?;
        tracing::info!("MOCK MODE: Created pool subaccount {} with number {}", mock_sid, mock_phone);
        return Ok(());
    }

    // Real mode: Call Twilio APIs
    // 1. Create subaccount
    let subaccount_response = client
        .post("https://api.twilio.com/2010-04-01/Accounts.json")
        .basic_auth(&main_account_sid, Some(&main_auth_token))
        .header(reqwest::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .form(&[("FriendlyName", "US Pool Number")])
        .send()
        .await?;

    if !subaccount_response.status().is_success() {
        let error_body = subaccount_response.text().await?;
        return Err(format!("Failed to create pool subaccount: {}", error_body).into());
    }

    let subaccount_json: serde_json::Value = subaccount_response.json().await?;
    let subaccount_sid = subaccount_json["sid"]
        .as_str()
        .ok_or("Missing subaccount sid")?
        .to_string();
    let subaccount_auth_token = subaccount_json["auth_token"]
        .as_str()
        .ok_or("Missing subaccount auth_token")?
        .to_string();

    // 2. Buy US TollFree number
    let available_url = format!(
        "https://api.twilio.com/2010-04-01/Accounts/{}/AvailablePhoneNumbers/US/TollFree.json?Limit=1",
        main_account_sid
    );

    let available_resp = client
        .get(&available_url)
        .basic_auth(&main_account_sid, Some(&main_auth_token))
        .send()
        .await?;

    let available_json: serde_json::Value = available_resp.json().await?;
    let phone_number = available_json["available_phone_numbers"][0]["phone_number"]
        .as_str()
        .ok_or("No available US TollFree numbers")?
        .to_string();

    // 3. Buy the number
    let buy_url = format!(
        "https://api.twilio.com/2010-04-01/Accounts/{}/IncomingPhoneNumbers.json",
        main_account_sid
    );

    let buy_response = client
        .post(&buy_url)
        .basic_auth(&main_account_sid, Some(&main_auth_token))
        .header(reqwest::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .form(&[("PhoneNumber", phone_number.as_str())])
        .send()
        .await?;

    let buy_json: serde_json::Value = buy_response.json().await?;
    let pn_sid = buy_json["sid"]
        .as_str()
        .ok_or("Missing phone number sid")?
        .to_string();

    // 4. Transfer to subaccount
    let transfer_url = format!(
        "https://api.twilio.com/2010-04-01/Accounts/{}/IncomingPhoneNumbers/{}.json",
        main_account_sid, pn_sid
    );

    client
        .post(&transfer_url)
        .basic_auth(&main_account_sid, Some(&main_auth_token))
        .header(reqwest::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .form(&[("AccountSid", subaccount_sid.as_str())])
        .send()
        .await?;

    // 5. Save to database using repository
    let created_at = Some(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32,
    );

    let new_subaccount = NewSubaccount {
        user_id: "-1".to_string(),
        subaccount_sid,
        auth_token: subaccount_auth_token,
        country: Some("US".to_string()),
        number: Some(phone_number.clone()),
        cost_this_month: Some(0.0),
        created_at,
        status: Some("available".to_string()),
        tinfoil_key: None,
        messaging_service_sid: std::env::var("TWILIO_MESSAGING_SERVICE_SID").ok(),
        subaccount_type: "us_ca".to_string(), // Pool numbers are US
        country_code: Some("US".to_string()),
    };

    state.user_core.insert_subaccount(&new_subaccount)?;
    tracing::info!("Created US pool subaccount with number {}", phone_number);

    Ok(())
}

/// Maintain US buffer pool - ensure we have 3 free US numbers
pub async fn maintain_us_buffer_pool(state: &Arc<AppState>) -> Result<(), Box<dyn std::error::Error>> {
    let free_count = state.user_core.count_free_us_subaccounts()? as usize;
    tracing::info!("US pool currently has {} free numbers", free_count);

    if free_count < 3 {
        let needed = 3 - free_count;
        tracing::info!("Need to buy {} US numbers to reach buffer of 3", needed);

        for i in 0..needed {
            match buy_us_number_for_pool(state).await {
                Ok(_) => {
                    tracing::info!("Successfully bought US pool number {}/{}", i + 1, needed);
                }
                Err(e) => {
                    tracing::error!("Failed to buy US pool number {}/{}: {}", i + 1, needed, e);
                    // Continue trying to buy remaining numbers even if one fails
                }
            }
        }
    } else {
        tracing::info!("US pool buffer is sufficient ({} >= 3), no action needed", free_count);
    }

    Ok(())
}

/// Release oldest US numbers if pool exceeds 10
pub async fn cleanup_excess_us_numbers(state: &Arc<AppState>) -> Result<(), Box<dyn std::error::Error>> {
    let free_count = state.user_core.count_free_us_subaccounts()? as usize;

    if free_count > 10 {
        let to_release = free_count - 3; // Release down to 3
        tracing::info!("US pool has {} numbers, releasing {} oldest to reach buffer of 3", free_count, to_release);

        // Get oldest free US subaccounts using repository
        let oldest_subs = state.user_core.get_oldest_free_us_subaccounts(to_release as i64)?;

        let main_account_sid = std::env::var("TWILIO_ACCOUNT_SID")?;
        let main_auth_token = std::env::var("TWILIO_AUTH_TOKEN")?;
        let client = Client::new();

        for sub in oldest_subs {
            // Mock mode: Just delete from DB
            if is_mock_mode() {
                state.user_core.delete_subaccount(sub.id)?;
                tracing::info!("MOCK MODE: Deleted pool subaccount {} from database", sub.subaccount_sid);
                continue;
            }

            // Real mode: Delete from Twilio first
            if let Some(number) = &sub.number {
                tracing::info!("Releasing US number {} from pool", number);
            }

            // Delete subaccount from Twilio
            let delete_url = format!(
                "https://api.twilio.com/2010-04-01/Accounts/{}.json",
                sub.subaccount_sid
            );

            match client
                .delete(&delete_url)
                .basic_auth(&main_account_sid, Some(&main_auth_token))
                .send()
                .await
            {
                Ok(_) => {
                    // Delete from database using repository
                    state.user_core.delete_subaccount(sub.id)?;
                    tracing::info!("Successfully released pool subaccount {}", sub.subaccount_sid);
                }
                Err(e) => {
                    tracing::error!("Failed to delete pool subaccount {} from Twilio: {}", sub.subaccount_sid, e);
                }
            }
        }
    } else {
        tracing::info!("US pool size is {} (<= 10), no cleanup needed", free_count);
    }

    Ok(())
}
