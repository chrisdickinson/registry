use axum::body::{Body, HttpBody, StreamBody};
use axum::extract::{Path, State};
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{any, get, post, put};
use axum::{Json, Router};
use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;
use tower_http::sensitive_headers::SetSensitiveRequestHeadersLayer;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tower_http::LatencyUnit;

use serde_json::json;
use tracing::{instrument, Level};

use crate::extractors::Authenticated;
use crate::models::{PackageIdentifier, PackageModification, Packument};
use crate::operations::{Authenticator, Configurator, PackageStorage, TokenAuthorizer};

#[instrument(level = "info", fields(pkg))]
async fn get_packument<Storage>(
    State(state): State<Storage>,
    Path(pkg): Path<String>,
) -> Result<impl IntoResponse, StatusCode>
where
    Storage: PackageStorage + std::fmt::Debug,
{
    let Ok(pkg) = pkg.parse() else {
        return Err(StatusCode::BAD_REQUEST)
    };

    let stream = state
        .stream_packument(&pkg)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StreamBody::new(stream))
}

#[instrument(level = "info", fields(pkg))]
async fn put_packument<Storage>(
    State(state): State<Storage>,
    Path(pkg): Path<String>,
    Json(payload): Json<Packument>,
) -> Result<impl IntoResponse, StatusCode>
where
    Storage: PackageStorage + std::fmt::Debug,
{
    if payload.id.as_deref() != Some(pkg.as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let Ok(pkg) = pkg.parse() else {
        return Err(StatusCode::BAD_REQUEST)
    };

    let old_packument = state
        .fetch_packument(&pkg)
        .await
        .ok()
        .unwrap_or(Default::default());

    let Ok(_modification) = PackageModification::from_diff(old_packument, payload) else {
        return Err(StatusCode::BAD_REQUEST)
    };

    Ok(StatusCode::NOT_FOUND)
}

#[instrument(level = "info", fields(pkg))]
async fn put_packument_at_rev<Storage>(
    state: State<Storage>,
    Path((pkg, rev)): Path<(String, String)>,
    payload: Json<Packument>,
) -> Result<impl IntoResponse, StatusCode>
where
    Storage: PackageStorage + std::fmt::Debug,
{
    put_packument(state, Path(pkg), payload).await
}

#[instrument(level = "info", fields(pkg))]
async fn put_scoped_packument<Storage>(
    state: State<Storage>,
    Path((scope, pkg)): Path<(String, String)>,
    payload: Json<Packument>,
) -> Result<impl IntoResponse, StatusCode>
where
    Storage: PackageStorage + std::fmt::Debug,
{
    let pkg = format!("@{}/{}", scope, pkg);
    put_packument(state, Path(pkg), payload).await
}

async fn get_scoped_packument<Storage>(
    State(state): State<Storage>,
    Path((scope, pkg)): Path<(String, String)>,
) -> Result<impl IntoResponse, StatusCode>
where
    Storage: PackageStorage + std::fmt::Debug,
{
    let pkg = format!("@{}/{}", scope, pkg);
    get_packument(State(state), Path(pkg)).await
}

#[instrument(level = "info", fields(pkg, tarball))]
async fn get_tarball<Storage>(
    State(state): State<Storage>,
    Path((pkg, tarball)): Path<(String, String)>,
) -> Result<impl IntoResponse, StatusCode>
where
    Storage: PackageStorage + std::fmt::Debug,
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
        .stream_tarball(&pkg, version)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StreamBody::new(stream))
}

async fn get_scoped_tarball<Storage>(
    State(state): State<Storage>,
    Path((scope, pkg, tarball)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, StatusCode>
where
    Storage: PackageStorage + std::fmt::Debug,
{
    let pkg = format!("@{}/{}", scope, pkg);
    get_tarball(State(state), Path((pkg, tarball))).await
}

#[instrument]
async fn get_login_poll<Auth>(
    State(state): State<Auth>,
    Path(session): Path<String>,
) -> impl IntoResponse
where
    Auth: Authenticator + TokenAuthorizer + std::fmt::Debug,
{
    let Ok(session) = session.parse::<<Auth as Authenticator>::SessionId>() else {
        todo!();
    };

    let Ok(user) = state.poll_login_session(session).await else {
        todo!();
    };

    if let Some(user) = user {
        // TODO: this is the point at which we add them to UserStorage -- which is where
        // we may wish to apply WASM-based filtering of incoming users.
        let Ok(token) = state.start_session(user.into()).await else {
            todo!();
        };

        (
            StatusCode::OK,
            [("x-ok", "x-ok")],
            Json(serde_json::json!({
                "message": "ok",
                "token": token.to_string()
            })),
        )
    } else {
        (
            StatusCode::ACCEPTED,
            [("retry-after", "5")],
            Json(serde_json::json!({
                "message": "ok"
            })),
        )
    }
}

#[instrument]
async fn post_login<Auth, B>(
    State(state): State<Auth>,
    req: Request<B>,
) -> Result<impl IntoResponse, impl IntoResponse>
where
    Auth: Authenticator + Configurator + std::fmt::Debug,
    B: std::fmt::Debug + Into<axum::body::Body>,
{
    let fqdn = state.fqdn();

    let (parts, body) = req.into_parts();
    let req = Request::from_parts(parts, body.into());
    let Ok(id) = state.start_login_session(req).await else {
        return Err(StatusCode::BAD_REQUEST)
    };

    Ok(Json(json!({
        "doneUrl": format!("{}/-/v1/login/poll/{}", fqdn, id),
        "loginUrl": format!("{}/-/v1/login/www/{}", fqdn, id)
    })))
}

#[instrument]
async fn www_login<Auth, B>(
    State(state): State<Auth>,
    session: Option<Path<String>>,
    req: Request<B>,
) -> impl IntoResponse
where
    Auth: Authenticator + Configurator + std::fmt::Debug,
    B: std::fmt::Debug + Into<axum::body::Body>,
{
    let (parts, body) = req.into_parts();
    let req = Request::from_parts(parts, body.into());

    let session = if let Some(Path(session)) = session {
        let Ok(session) = session.parse::<<Auth as Authenticator>::SessionId>() else {
            todo!("invalid session id, bailing...");
        };
        Some(session)
    } else {
        None
    };

    let Ok(result) = state.complete_login_session(&state, req, session).await.map_err(|e| dbg!(e)) else {
        todo!("could not complete login session...");
    };

    result
}

#[instrument]
async fn get_user<Auth>(
    State(state): State<Auth>,
    Path(user): Path<String>,
) -> Result<impl IntoResponse, StatusCode>
where
    Auth: Authenticator + std::fmt::Debug,
{
    let Some(username) = user.strip_prefix(':') else {
        return Err(StatusCode::NOT_FOUND);
    };

    let Ok(Some(user)) = state.get_user(username).await else {
        return Err(StatusCode::NOT_FOUND);
    };

    // TODO: "fetch user" capability
    Ok(Json(json!({
        "_id": format!("org.couchdb.user:{}", user.name),
        "name": user.name,
        "email": ""
    })))
}

#[instrument]
async fn whoami(Authenticated(user): Authenticated) -> impl IntoResponse {
    Json(json!({
        "username": user.name
    }))
}

pub fn routes<S, B>(state: S) -> Router<(), B>
where
    S: PackageStorage
        + Clone
        + Sync
        + Send
        + 'static
        + std::fmt::Debug
        + Authenticator
        + TokenAuthorizer
        + Configurator,
    B: Sync + Send + HttpBody + std::fmt::Debug + Into<Body> + 'static,
    <B as HttpBody>::Data: 'static + Send + Sync,
    <B as HttpBody>::Error: std::error::Error + 'static + Send + Sync,
{
    Router::new()
        .route("/@:scope/:pkg/-/*tarball", get(get_scoped_tarball::<S>))
        .route(
            "/@:scope/:pkg",
            get(get_scoped_packument::<S>)
                .layer(ServiceBuilder::new().layer(CompressionLayer::new()))
                .put(put_scoped_packument::<S>),
        )
        .route(
            "/:pkg",
            get(get_packument::<S>)
                .layer(ServiceBuilder::new().layer(CompressionLayer::new()))
                .put(put_packument::<S>),
        )
        .route("/:pkg/-rev/:rev", put(put_packument_at_rev::<S>))
        .route("/:pkg/-/*tarball", get(get_tarball::<S>))
        .route("/-/v1/login", post(post_login::<S, B>))
        .route("/-/v1/login/poll/:session", get(get_login_poll::<S>))
        .route("/-/v1/login/www/:session", any(www_login::<S, B>))
        .route("/-/v1/login/www/", any(www_login::<S, B>))
        // .route("/-/v1/npm/tokens", get(get_tokens::<S>))
        .route("/-/user/org.couchdb.user:user", get(get_user::<S>))
        .route("/-/whoami", get(whoami))
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                .layer(SetSensitiveRequestHeadersLayer::new(std::iter::once(
                    axum::http::header::AUTHORIZATION,
                )))
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(DefaultMakeSpan::new().include_headers(true))
                        .on_response(
                            DefaultOnResponse::new()
                                .level(Level::INFO)
                                .include_headers(true)
                                .latency_unit(LatencyUnit::Micros),
                        ),
                ),
        )
}
