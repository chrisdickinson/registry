use std::env;
use tracing::{info, span, Level};
use rusoto_credential::EnvironmentProvider;
use rusoto_s3::S3Client;

use chrono::Duration;

use crate::stores::{ ReadableStore, RemoteStore, RedisReader, S3Store, ReadThrough, CacacheStore };
use crate::rusoto_surf::SurfRequestDispatcher;
use crate::handlers;
use tide::Server;

pub(crate) fn package_read_routes<Outer, Inner>(
        mut app: Server<Outer>,
        store: Inner
    ) -> Server<Outer> 
    where Outer: Send + Sync + 'static,
          Inner: ReadableStore + Send + Sync + 'static {
    let mut inner = tide::with_state(store);
    inner.at("/:pkg")
        .get(handlers::get_packument);

    inner.at("/:pkg/-/*tarball")
        .get(handlers::get_tarball);

    inner.at("/:scope/:pkg/-/*tarball")
        .get(handlers::get_scoped_tarball);

    app.at("")
        .nest(inner);

    app
}
