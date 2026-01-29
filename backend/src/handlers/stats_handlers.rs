use crate::AppState;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;
use std::sync::Arc;

/// GET /api/stats/smartphone-free-days
///
/// Returns the total number of smartphone-free days powered by Lightfriend.
/// This is a public endpoint that reads from the cached site_metrics table.
pub async fn get_smartphone_free_days(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    match state.metrics_repository.get_metric("smartphone_free_days") {
        Ok(Some(metric)) => {
            let days: i64 = metric.metric_value.parse().unwrap_or(0);
            Ok(Json(json!({ "days": days })))
        }
        Ok(None) => {
            // No metric yet, return 0
            Ok(Json(json!({ "days": 0 })))
        }
        Err(e) => {
            tracing::error!("Failed to get smartphone_free_days metric: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
