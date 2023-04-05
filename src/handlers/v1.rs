use axum::body::HttpBody;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use tracing::{info, instrument};

#[instrument(ret)]
async fn get_packument() -> impl IntoResponse {
    info!("just trying this out");
    "hello world"
}

#[instrument]
async fn put_packument() -> impl IntoResponse {}

#[instrument]
async fn put_scoped_packument() -> impl IntoResponse {}

#[instrument]
async fn get_scoped_packument() -> impl IntoResponse {}

#[instrument]
async fn get_tarball() -> impl IntoResponse {}

#[instrument]
async fn get_scoped_tarball() -> impl IntoResponse {}

#[instrument]
async fn get_login_poll() -> impl IntoResponse {}

#[instrument]
async fn post_login() -> impl IntoResponse {}

pub fn routes<S, B>() -> Router<S, B>
where
    S: Clone + Sync + Send + 'static,
    B: Sync + Send + HttpBody + 'static,
{
    Router::new()
        .route("/@:scope/:pkg/-/*tarball", get(get_scoped_tarball))
        .route(
            "/@:scope/:pkg",
            get(get_scoped_packument).put(put_scoped_packument),
        )
        .route("/:pkg", get(get_packument).put(put_packument))
        .route("/:pkg/-/*tarball", get(get_tarball))
        .route("/-/v1/login", post(post_login))
        .route("/-/v1/login/poll/:session", get(get_login_poll))
}
