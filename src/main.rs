use std::env;
use tide::{Next, Request, Response};
use tracing::{error, info, span, Level};
mod handlers;
mod middleware;
mod packument;
mod stores;

use chrono::Duration;

use stores::{ ReadThrough, RedisCache };

#[async_std::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let read_through = ReadThrough::new(
        "http://localhost:8080", // TODO: env var
        "https://registry.npmjs.org", // TODO: env var
        ()
    );

    let redis = RedisCache::new(
        "redis://localhost:6379/",
        read_through,
        Duration::minutes(5)
    ).await?;

    // json_logger::init("anything", log::LevelFilter::Info).unwrap();
    simple_logger::init().unwrap();
    let mut app = tide::with_state(redis);
    app.middleware(middleware::Logging::new());

    let _span = span!(Level::INFO, "server started");

    let port = env::var("PORT").ok().unwrap_or_else(|| "8080".to_string());
    let host = env::var("HOST")
        .ok()
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let addr = format!("{}:{}", host, port);

    app.at("/:pkg")
        .get(handlers::get_packument)
        .put(handlers::put_packument);

    app.at("/:pkg/-/*tarball").get(handlers::get_tarball);

    app.at("/:scope/:pkg/-/*tarball")
        .get(handlers::get_scoped_tarball);

    app.at("/-/v1/login").post(handlers::post_login);

    app.at("/-/v1/login/poll/:session")
        .get(handlers::get_login_poll);

    info!("server listening on address {}", addr);
    app.listen(addr).await?;
    Ok(())
}
