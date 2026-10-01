use axum::{
    body::Body,
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::sync::Arc;
use tokio::io::AsyncReadExt;

use crate::{
    handlers::auth_middleware::AuthUser,
    repositories::analytics_export_repository::AnalyticsExportRepository, AppState,
};

/// GET /api/admin/exports/analytics — all retained history, no date/row limit.
pub async fn export_analytics(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    // Defense in depth, in addition to the admin router's middleware.
    if !auth.is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Admin access required"})),
        ));
    }
    let pool = state.pg_pool.clone();
    let result =
        tokio::task::spawn_blocking(move || AnalyticsExportRepository::new(pool).create_archive())
            .await;
    let file = match result {
        Ok(Ok(file)) => file,
        _ => {
            // Do not log DB errors: they can include row values/PII.
            tracing::error!(
                admin_user_id = auth.user_id,
                "Analytics archive generation failed"
            );
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Analytics export failed. No partial archive was returned; retry before shutdown."
                })),
            ));
        }
    };
    let length = file
        .metadata()
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Could not read analytics archive"})),
            )
        })?
        .len();
    let filename = format!(
        "attachment; filename=\"lightfriend-analytics-{}.json\"",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
    );
    let mut file = tokio::fs::File::from_std(file);
    let stream = async_stream::try_stream! {
        loop {
            let mut chunk = vec![0u8; 64 * 1024];
            let read = file.read(&mut chunk).await?;
            if read == 0 { break; }
            chunk.truncate(read);
            yield axum::body::Bytes::from(chunk);
        }
    };
    let stream: std::pin::Pin<
        Box<dyn futures::Stream<Item = Result<axum::body::Bytes, std::io::Error>> + Send>,
    > = Box::pin(stream);
    tracing::info!(
        admin_user_id = auth.user_id,
        bytes = length,
        "Analytics archive generated"
    );
    Ok((
        [
            (
                header::CONTENT_TYPE,
                "application/json; charset=utf-8".to_owned(),
            ),
            (header::CONTENT_DISPOSITION, filename),
            (header::CONTENT_LENGTH, length.to_string()),
            (header::CACHE_CONTROL, "no-store, private".to_owned()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_owned()),
        ],
        Body::from_stream(stream),
    )
        .into_response())
}
