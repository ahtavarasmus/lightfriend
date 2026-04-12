use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use std::sync::Arc;

use crate::AppState;

pub async fn blog_post_handler(
    Path(slug): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Response {
    match state.blog_store.get_post(&slug) {
        Some(post) => {
            let mut response = Html(post.full_page_html.clone()).into_response();
            response.headers_mut().insert(
                header::CACHE_CONTROL,
                "public, max-age=300, s-maxage=3600".parse().unwrap(),
            );
            response
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

pub async fn blog_post_md_handler(
    Path(slug): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Response {
    match state.blog_store.get_post(&slug) {
        Some(post) => {
            let md_content = format!(
                "# {}\n\n{}\n\n---\nSource: https://lightfriend.ai/blog/{}\n",
                post.frontmatter.title, post.raw_markdown, post.frontmatter.slug
            );
            let mut response = md_content.into_response();
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                "text/markdown; charset=utf-8".parse().unwrap(),
            );
            response.headers_mut().insert(
                header::CACHE_CONTROL,
                "public, max-age=300, s-maxage=3600".parse().unwrap(),
            );
            response
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

pub async fn blog_index_handler(State(state): State<Arc<AppState>>) -> Response {
    let mut response = Html(state.blog_store.blog_index_html.clone()).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        "public, max-age=300, s-maxage=3600".parse().unwrap(),
    );
    response
}

pub async fn sitemap_handler(State(state): State<Arc<AppState>>) -> Response {
    let mut response = state.blog_store.sitemap_xml.clone().into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, "application/xml".parse().unwrap());
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        "public, max-age=3600, s-maxage=86400".parse().unwrap(),
    );
    response
}
