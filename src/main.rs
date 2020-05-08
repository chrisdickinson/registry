#![feature(async_closure)]
#![feature(type_alias_impl_trait)]
use std::env;
use tracing::{info, span, Level};
use rusoto_credential::EnvironmentProvider;
use rusoto_s3::S3Client;

use chrono::Duration;

use crate::stores::{ RemoteStore, RedisReader, S3Store, ReadThrough };
use crate::rusoto_surf::SurfRequestDispatcher;

mod handlers;
mod middleware;
mod packument;
mod stores;
mod rusoto_surf;


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
            S3Store::new(s3_bucket, client),
            store
        )
    );

    // json_logger::init("anything", log::LevelFilter::Info).unwrap();
    simple_logger::init_with_level(log::Level::Info).unwrap();
    let mut app = tide::with_state(store);
    app.middleware(middleware::Logging::new());

    let _span = span!(Level::INFO, "server started");

    app.at("/:pkg")
        .get(handlers::get_packument)
        .put(handlers::put_packument);

    app.at("/:pkg/-/*tarball").get(handlers::get_tarball);

    app.at("/:scope/:pkg/-/*tarball")
        .get(handlers::get_scoped_tarball);

    app.at("/-/v1/login").post(handlers::post_login);

    app.at("/-/v1/login/poll/:session")
        .get(handlers::get_login_poll);

    info!("server listening on address {}", &addr);
    app.listen(addr).await?;
    Ok(())
}
