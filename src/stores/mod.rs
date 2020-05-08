use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::prelude::*;
use http_types::Result;
use serde::{Deserialize, Serialize};

mod readthrough;
mod redis_cache;
mod cacache;
mod chained;
mod remote;
mod guard;
mod s3;

pub use crate::stores::cacache::CacacheStore;
pub use readthrough::ReadThrough;
pub use redis_cache::RedisReader;
pub use remote::RemoteStore;
pub use guard::GuardStore;
pub use s3::S3Store;

use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
pub struct PackageMetadata {
    pub(crate) integrity: String,
    pub(crate) last_fetched_at: DateTime<Utc>,
}

impl Into<HashMap<String, String>> for PackageMetadata {
    fn into(self) -> HashMap<String, String> {
        let mut hm = HashMap::new();
        hm.insert(
            "integrity".to_string(),
            self.integrity
        );
        hm.insert(
            "last-fetched-at".to_string(),
            self.last_fetched_at.to_rfc3339()
        );
        hm
    }
}

impl From<HashMap<String, String>> for PackageMetadata {
    fn from(mut hm: HashMap<String, String>) -> Self {
        PackageMetadata {
            integrity: hm.remove("integrity")
                .unwrap_or_else(|| "".to_string()),
            last_fetched_at: hm.remove("last_fetched_at")
                .unwrap_or_else(|| "1996-01-01T00:00:00".to_string())
                .parse()
                .unwrap()
        }
    }
}

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
              W: AsyncBufRead + Send + Sync + std::marker::Unpin + 'static {
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
        W: AsyncBufRead + Send + Sync + std::marker::Unpin + 'static
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
