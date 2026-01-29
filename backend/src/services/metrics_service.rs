use stripe::{Client, ListSubscriptions, Subscription, SubscriptionId, SubscriptionStatusFilter};
use tracing::{debug, info};

/// Calculate the total smartphone-free days across all subscriptions ever created.
///
/// For each subscription:
/// - Uses `created` as the start timestamp
/// - Uses `ended_at` if the subscription is cancelled, otherwise uses `now`
/// - Calculates days = (end_time - created) / 86400
///
/// Returns the total sum of days across all subscriptions (active + cancelled).
pub async fn calculate_smartphone_free_days() -> Result<i64, String> {
    let stripe_key =
        std::env::var("STRIPE_SECRET_KEY").map_err(|_| "STRIPE_SECRET_KEY not set".to_string())?;

    let client = Client::new(stripe_key);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let mut total_days: i64 = 0;
    let mut has_more = true;
    let mut starting_after: Option<SubscriptionId> = None;
    let mut page_count = 0;

    // Fetch all subscriptions (active and cancelled) with pagination
    while has_more {
        page_count += 1;
        debug!("Fetching subscriptions page {}", page_count);

        let mut params = ListSubscriptions::new();
        params.status = Some(SubscriptionStatusFilter::All);
        params.limit = Some(100);

        if let Some(ref cursor) = starting_after {
            params.starting_after = Some(cursor.clone());
        }

        let subscriptions = Subscription::list(&client, &params)
            .await
            .map_err(|e| format!("Failed to list subscriptions: {}", e))?;

        for sub in &subscriptions.data {
            let created = sub.created;

            // Use ended_at for cancelled subscriptions, otherwise use now
            let end_time = if let Some(ended) = sub.ended_at {
                ended
            } else {
                now
            };

            // Calculate days for this subscription
            let days = (end_time - created) / 86400;
            if days > 0 {
                total_days += days;
            }
        }

        has_more = subscriptions.has_more;

        // Get the last subscription ID for pagination
        if has_more {
            if let Some(last_sub) = subscriptions.data.last() {
                starting_after = Some(last_sub.id.clone());
            } else {
                has_more = false;
            }
        }

        // Small delay to avoid rate limiting
        if has_more {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    info!(
        "Calculated smartphone-free days: {} total across {} pages of subscriptions",
        total_days, page_count
    );

    Ok(total_days)
}
