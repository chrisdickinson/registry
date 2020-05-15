#![feature(async_closure)]
use std::env;
use tracing::{info, span, Level};
use rusoto_credential::EnvironmentProvider;
use rusoto_s3::S3Client;

use chrono::Duration;

use crate::middleware::{ Logging, Authentication };
use crate::auth::{ BasicAuthScheme, BearerAuthScheme };
use crate::stores::{ RemoteStore, RedisReader, S3Store, ReadThrough, CacacheStore };
use crate::rusoto_surf::SurfRequestDispatcher;
use crate::app::package_read_routes;

mod app;
mod auth;
mod handlers;
mod middleware;
mod packument;
mod stores;
mod rusoto_surf;

pub struct User {
    pub(crate) username: String,
    pub(crate) email: String
}

struct RegistryState {}

#[async_trait::async_trait]
impl auth::AuthnStorage<User, auth::BasicAuthRequest> for RegistryState {
    async fn get_user(&self, request: auth::BasicAuthRequest) -> http_types::Result<Option<User>> {
        // Lest you worry, this is a fake password.
        if request.username == "chris" && request.password == "applecat1" { 
            Ok(Some(User { username: "chris".to_owned(), email: "chris@neversaw.us".to_owned() }))
        } else {
            Ok(None)
        }
    }
}

#[async_trait::async_trait]
impl auth::AuthnStorage<User, auth::BearerAuthRequest> for RegistryState {
    async fn get_user(&self, request: auth::BearerAuthRequest) -> http_types::Result<Option<User>> {
        if request.token == "r_9e768f7a-8ab3-4c15-81ea-34a37e29b215" {
            Ok(Some(User { username: "chris".to_owned(), email: "chris@neversaw.us".to_owned() }))
        } else {
            Ok(None)
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    smol::run(async {
        handle().await
    })
}

async fn handle() -> Result<(), Box<dyn std::error::Error>> {
    let port = env::var("PORT").ok().unwrap_or_else(|| "8080".to_string());
    let host = env::var("HOST")
        .ok()
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let addr = format!("{}:{}", host, port);
    let remote_url = env::var("REMOTE_URL").ok().unwrap_or_else(|| "https://registry.npmjs.org".to_string());
    let redis_url = env::var("REDIS_URL").ok().unwrap_or_else(|| "redis://localhost:6379/".to_string());
    let s3_bucket = env::var("S3_BUCKET").ok().unwrap_or_else(|| "www.neversaw.us".to_string());
    let aws_region = env::var("AWS_DEFAULT_REGION").ok().or_else(|| Some("us-west-2".to_string())).map(|xs| {
        xs.parse().ok()
    }).unwrap().expect("Expected a valid AWS region");

    let store = RemoteStore::new(
        &addr,
        &remote_url
    );

    let client = S3Client::new(aws_region);

    let store = ReadThrough::new(
        RedisReader::new(
            redis_url,
            Duration::minutes(5)
        ).await?,
        ReadThrough::new(
            CacacheStore::new("./.cache"),
            S3Store::new(s3_bucket, client),
            // store
        )
    );


    let mut app = tide::with_state(RegistryState {});

    // json_logger::init("anything", log::LevelFilter::Info).unwrap();
    simple_logger::init_with_level(log::Level::Info).unwrap();
    app.middleware(Logging::new());

    app.middleware(Authentication::new(BasicAuthScheme::default()));
    app.middleware(Authentication::new(BearerAuthScheme::default()));

    let _span = span!(Level::INFO, "server started");

    let app = package_read_routes(app, store);
    // app.at("/-/v1/login").post(handlers::post_login);

    // app.at("/-/v1/login/poll/:session")
    //    .get(handlers::get_login_poll);

    info!("server listening on address {}", &addr);
    app.listen(addr).await?;
    Ok(())
}
