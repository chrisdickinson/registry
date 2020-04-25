use crate::packument::{Human, Packument};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::future::BoxFuture;
use futures::prelude::*;
use http_types::Result;
use serde::{Deserialize, Serialize};
use tracing::{error, info, span, Level};
mod readthrough;
mod redis_cache;

#[derive(Serialize, Deserialize)]
pub struct PackageMetadata {
    integrity: String,
    last_fetched_at: DateTime<Utc>,
}

pub use readthrough::ReadThrough;
pub use redis_cache::RedisCache;

#[async_trait]
pub trait ReadableStore : Sync {
    type PackumentReader: AsyncBufRead + Send + Sync + std::marker::Unpin + 'static;
    type TarballReader: AsyncBufRead + Send + Sync + std::marker::Unpin + 'static;

    async fn get_packument<T>(&self, package: T) -> Result<Option<(Self::PackumentReader, PackageMetadata)>>
    where
        T: AsRef<str> + Send + Sync,
    {
        Ok(None)
    }

    async fn get_tarball<T, S>(
        &self,
        package: T,
        version: S,
    ) -> Result<Option<(Self::TarballReader, PackageMetadata)>>
    where
        T: AsRef<str> + Send + Sync,
        S: AsRef<str> + Send + Sync,
    {
        Ok(None)
    }
}

pub trait WritableStore {
    fn upsert_packument<T: AsRef<str>, B: std::io::Read>(
        &self,
        package: T,
        body: B,
    ) -> Result<PackageMetadata>;

    fn update_metadata<T: AsRef<str>>(
        &self,
        package: T,
        metadata: PackageMetadata,
    ) -> Result<PackageMetadata>;
}

/**
  The empty read store. Implemented on unit as a way to define a store that
  always 404s.
*/
impl ReadableStore for () {
    type PackumentReader = Box<dyn AsyncBufRead + Send + Sync + std::marker::Unpin>;
    type TarballReader = Box<dyn AsyncBufRead + Send + Sync + std::marker::Unpin>;
}
