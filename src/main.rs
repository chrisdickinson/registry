#![feature(async_closure)]
use std::env;
use tracing::{info, span, Level};
use rusoto_credential::EnvironmentProvider;
use rusoto_s3::S3Client;

use chrono::Duration;

use crate::middleware::{ Logging, SimpleBearerStorage, SimpleBasicStorage, Authentication, BasicAuthScheme, BearerAuthScheme };
use crate::stores::{ RemoteStore, RedisReader, S3Store, ReadThrough, CacacheStore };
use crate::rusoto_surf::SurfRequestDispatcher;
use crate::app::package_read_routes;

mod app;
mod handlers;
mod middleware;
mod packument;
mod stores;
mod rusoto_surf;

#[async_std::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = env::var("PORT").ok().unwrap_or_else(|| "8080".to_string());
    let host = env::var("HOST")
        .ok()
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let addr = format!("{}:{}", host, port);
    let remote_url = env::var("REMOTE_URL").ok().unwrap_or_else(|| "https://registry.npmjs.org".to_string());
    let redis_url = env::var("REDIS_URL").ok().unwrap_or_else(|| "redis://localhost:6379/".to_string());
    let s3_bucket = env::var("S3_BUCKET").ok().unwrap_or_else(|| "www.neversaw.us".to_string());

    let store = RemoteStore::new(
        &addr,
        &remote_url
    );

    let client = S3Client::new_with(
        SurfRequestDispatcher::new(),
        EnvironmentProvider::default(),
        rusoto_core::Region::default()
    );

    let store = ReadThrough::new(
        RedisReader::new(
            redis_url,
            Duration::minutes(5)
        ).await?,
        ReadThrough::new(
            CacacheStore::new("./.cache"),
            // S3Store::new(s3_bucket, client),
            store
        )
    );


    let auth_stores = (SimpleBearerStorage::default(), SimpleBasicStorage::default());
    let mut app = tide::with_state(auth_stores);

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
