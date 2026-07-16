use axum::{body::Body, http::Request};
use std::{fs, path::PathBuf};
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};

struct TempPublicDir(PathBuf);

impl TempPublicDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("lightfriend-spa-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).expect("temporary public directory should be created");
        fs::write(path.join("index.html"), "<html>Lightfriend</html>")
            .expect("temporary SPA entrypoint should be written");
        Self(path)
    }
}

impl Drop for TempPublicDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn spa_routes_return_ok_with_the_index_entrypoint() {
    let public = TempPublicDir::new();
    let service = ServeDir::new(&public.0).fallback(ServeFile::new(public.0.join("index.html")));

    let response = service
        .oneshot(
            Request::builder()
                .uri("/pricing")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("SPA fallback request should succeed");

    assert_eq!(response.status(), axum::http::StatusCode::OK);
}
