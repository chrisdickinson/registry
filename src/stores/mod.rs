use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::io::AsyncWrite;
use futures::prelude::*;
use http_types::Result;
use serde::{Deserialize, Serialize};

mod readthrough;
mod redis_cache;
mod chained;
mod guard;
mod s3;

pub use guard::GuardStore;

#[derive(Serialize, Deserialize)]
pub struct PackageMetadata {
    integrity: String,
    last_fetched_at: DateTime<Utc>,
}

pub use readthrough::ReadThrough;
pub use redis_cache::RedisReader;
pub use s3::S3Store;

#[async_trait]
pub trait ReadableStore : Sync {
    type PackumentReader: AsyncBufRead + Send + Sync + std::marker::Unpin + 'static;
    type TarballReader: AsyncBufRead + Send + Sync + std::marker::Unpin + 'static;

    async fn get_packument<T>(&self, _package: T) -> Result<Option<(Self::PackumentReader, PackageMetadata)>>
    where
        T: AsRef<str> + Send + Sync,
    {
        Ok(None)
    }

    async fn get_tarball<T, S>(
        &self,
        _package: T,
        _version: S,
    ) -> Result<Option<(Self::TarballReader, PackageMetadata)>>
    where
        T: AsRef<str> + Send + Sync,
        S: AsRef<str> + Send + Sync,
    {
        Ok(None)
    }
}

#[async_trait]
pub trait WritableStore {
    async fn write_packument<T, W>(&self, _package: T, _data: W, _meta: PackageMetadata) -> Result<Option<bool>>
        where T: AsRef<str> + Send + Sync,
              W: AsyncWrite + Send + Sync {
        Ok(None)
    }

    async fn write_tarball<T, S, W>(
        &self,
        _package: T,
        _version: S,
        _data: W,
        _meta: PackageMetadata
    ) -> Result<Option<bool>>
    where
        T: AsRef<str> + Send + Sync,
        S: AsRef<str> + Send + Sync,
        W: AsyncWrite + Send + Sync
    {
        Ok(None)
    }
}

/**
  The empty read store. Implemented on unit as a way to define a store that
  always 404s.
*/
impl ReadableStore for () {
    type PackumentReader = Box<dyn AsyncBufRead + Send + Sync + std::marker::Unpin>;
    type TarballReader = Box<dyn AsyncBufRead + Send + Sync + std::marker::Unpin>;
}
