#![feature(async_closure)]
use std::env;
use tracing::{info, span, Level};
mod handlers;
mod middleware;
mod packument;
mod stores;
mod rusoto_surf;

use rusoto_credential::EnvironmentProvider;
use rusoto_s3::{ GetObjectRequest, S3, S3Client };

use chrono::Duration;

use stores::{ ReadThrough, RedisReader };

/*
struct AWSCredentials {
    env: EnvironmentProvider,
    instance_profile: AutoRefreshingProvider<InstanceMetadataProvider>
}

#[async_trait]
impl ProvideAwsCredentials for AWSCredentials {
    async fn credentials(&self) -> Result<rusoto_credential::AwsCredentials, rusoto_credential::CredentialsError> {
        if let Ok(creds) = self.env.credentials().await {
            Ok(creds)
        } else {
            Err(rusoto_credential::CredentialsError::s
            // self.instance_profile.credentials().await
        }
    }
}
    let credentials = AWSCredentials {
        env: ,
        instance_profile: AutoRefreshingProvider::new(InstanceMetadataProvider::default())?
    };
*/

use crate::rusoto_surf::SurfRequestDispatcher;
use futures::prelude::*;

#[async_std::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let read_through = ReadThrough::new(
        "http://localhost:8080", // TODO: env var
        "https://registry.npmjs.org", // TODO: env var
    );

    let redis = RedisReader::new(
        "redis://localhost:6379/",
        read_through,
        Duration::minutes(5)
    ).await?;

    let client = S3Client::new_with(SurfRequestDispatcher::new(), EnvironmentProvider::default(), rusoto_core::Region::default());

    let resp = client.get_object(GetObjectRequest {
        bucket: "www.neversaw.us".to_owned(),
        key: "scratch/old-terrain/media/js/game.js".to_owned(),
        ..Default::default()
    }).await?;

    if let Some(body) = resp.body {
        let result: Vec<_> = body
            .collect().await;

        dbg!(result);
    }

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
