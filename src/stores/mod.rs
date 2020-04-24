use crate::packument::{Human, Packument};
use http_types::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::future::BoxFuture;
use futures::prelude::*;
use std::io::Read;

mod readthrough;

pub struct PackageMetadata {
    integrity: String,
    last_fetched_at: DateTime<Utc>,
}

pub use readthrough::ReadThrough;

#[async_trait]
pub trait ReadableStore {
    type Reader: AsyncRead + Send + std::marker::Unpin;

    async fn get_packument<T>(&self, package: T) -> Result<Option<Packument>>
    where
        T: AsRef<str> + Send + Sync,
    {
        if let Some((mut reader, hash)) = self.get_packument_raw(package).await? {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;

            let packument = serde_json::from_slice(&bytes[..])?;
            return Ok(Some(packument));
        }

        Ok(None)
    }

    async fn get_packument_raw<T>(
        &self,
        package: T,
    ) -> Result<Option<(Self::Reader, PackageMetadata)>>
    where
        T: AsRef<str> + Send + Sync,
    {
        Ok(None)
    }

    fn get_packument_readme<T>(&self, package: T) -> Result<Option<Self::Reader>>
    where
        T: AsRef<str> + Send + Sync,
    {
        Ok(None)
    }

    fn get_tarball<T, S>(
        &self,
        package: T,
        version: S,
    ) -> Result<Option<(Self::Reader, PackageMetadata)>>
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

/*
#[async_trait]
pub trait AuthorityStore {
    async fn check_password<T, S>(&self, username: T, password: S) -> Result<bool>
        where T: AsRef<str> + Send + Sync,
              S: AsRef<str> + Send + Sync;

    async fn signup<T, S, V>(&self, username: T, password: S, email: V) -> Result<Human>
        where T: AsRef<str> + Send + Sync,
              S: AsRef<str> + Send + Sync,
              V: AsRef<str> + Send + Sync;
}
*/

/**
  The empty read store. Implemented on unit as a way to define a store that
  always 404s.
*/
impl ReadableStore for () {
    type Reader = Box<dyn AsyncRead + Send + std::marker::Unpin>;
}
