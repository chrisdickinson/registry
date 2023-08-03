use axum::body::{HttpBody, StreamBody};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};

use serde_json::json;
use tracing::instrument;

use crate::models::{PackageIdentifier, Packument};
use crate::operations::{FetchPackument, PackageModification, StreamPackument, StreamTarball};

#[instrument(level = "info", fields(pkg))]
async fn get_packument<Fetch>(
    State(state): State<Fetch>,
    Path(pkg): Path<String>,
) -> Result<impl IntoResponse, StatusCode>
where
    Fetch: StreamPackument + std::fmt::Debug,
{
    let Ok(pkg) = pkg.parse() else {
        return Err(StatusCode::BAD_REQUEST)
    };

    let stream = state
        .stream(&pkg)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StreamBody::new(stream))
}

#[instrument(level = "info", fields(pkg))]
async fn put_packument<Fetch>(
    State(state): State<Fetch>,
    Path(pkg): Path<String>,
    Json(payload): Json<Packument>,
) -> Result<impl IntoResponse, StatusCode>
where
    Fetch: FetchPackument + std::fmt::Debug,
{
    if payload.id.as_deref() != Some(pkg.as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let Ok(pkg) = pkg.parse() else {
        return Err(StatusCode::BAD_REQUEST)
    };

    let old_packument = state.fetch(&pkg).await.ok().unwrap_or(Default::default());

    let Ok(modification) = PackageModification::from_diff(old_packument, payload) else {
        return Err(StatusCode::BAD_REQUEST)
    };

    eprintln!("{:?}", modification);

    Ok(StatusCode::NOT_FOUND)
}

#[instrument]
async fn put_scoped_packument() -> impl IntoResponse {
    eprintln!("oh no!");
    StatusCode::NOT_FOUND
}

#[instrument(level = "info", fields(scope, pkg))]
async fn get_scoped_packument<Fetch>(
    State(state): State<Fetch>,
    Path((scope, pkg)): Path<(String, String)>,
) -> Result<impl IntoResponse, StatusCode>
where
    Fetch: StreamPackument + std::fmt::Debug,
{
    let pkg = format!("@{}/{}", scope, pkg);
    let pkg = pkg.parse().unwrap();
    let stream = state
        .stream(&pkg)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StreamBody::new(stream))
}

#[instrument(level = "info", fields(pkg, tarball))]
async fn get_tarball<Fetch>(
    State(state): State<Fetch>,
    Path((pkg, tarball)): Path<(String, String)>,
) -> Result<impl IntoResponse, StatusCode>
where
    Fetch: StreamTarball + std::fmt::Debug,
{
    let pkg: PackageIdentifier = pkg.parse().unwrap();
    if !tarball.starts_with(pkg.name.as_str())
        || tarball.get(pkg.name.len()..pkg.name.len() + 1) != Some("-")
        || !tarball.ends_with(".tgz")
    {
        return Err(StatusCode::BAD_REQUEST);
    }

    let version = tarball.get(pkg.name.len() + 1..tarball.len() - 4).unwrap();

    let stream = state
        .stream(&pkg, version)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StreamBody::new(stream))
}

#[instrument]
async fn get_scoped_tarball<Fetch>(
    State(state): State<Fetch>,
    Path((scope, pkg, tarball)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, StatusCode>
where
    Fetch: StreamTarball + std::fmt::Debug,
{
    let pkg = format!("@{}/{}", scope, pkg);
    let pkg: PackageIdentifier = pkg.parse().unwrap();
    if !tarball.starts_with(pkg.name.as_str())
        || tarball.get(pkg.name.len()..pkg.name.len() + 1) != Some("-")
        || !tarball.ends_with(".tgz")
    {
        return Err(StatusCode::BAD_REQUEST);
    }

    let version = tarball.get(pkg.name.len() + 1..tarball.len() - 4).unwrap();

    let stream = state
        .stream(&pkg, version)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StreamBody::new(stream))
}

#[instrument]
async fn get_login_poll() -> impl IntoResponse {}

#[instrument]
async fn post_login() -> impl IntoResponse {}

#[instrument]
async fn whoami() -> impl IntoResponse {
    Json(json!({
        "username": "chris"
    }))
}

pub fn routes<S, B>(state: S) -> Router<(), B>
where
    S: StreamTarball
        + StreamPackument
        + FetchPackument
        + Clone
        + Sync
        + Send
        + 'static
        + std::fmt::Debug,
    B: Sync + Send + HttpBody + 'static,
    <B as HttpBody>::Data: 'static + Send + Sync,
    <B as HttpBody>::Error: std::error::Error + 'static + Send + Sync,
{
    Router::new()
        .route("/@:scope/:pkg/-/*tarball", get(get_scoped_tarball::<S>))
        .route(
            "/@:scope/:pkg",
            get(get_scoped_packument::<S>).put(put_scoped_packument),
        )
        .route("/:pkg", get(get_packument::<S>).put(put_packument::<S>))
        .route("/:pkg/-/*tarball", get(get_tarball::<S>))
        .route("/-/v1/login", post(post_login))
        .route("/-/v1/login/poll/:session", get(get_login_poll))
        .route("/-/whoami", get(whoami))
        .with_state(state)
}
