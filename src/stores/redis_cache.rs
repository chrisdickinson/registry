use anyhow::Context;
use async_trait::async_trait;
use chrono::Duration;
use crate::stores::{PackageMetadata, ReadableStore, WritableStore};
use futures::prelude::*;
use http_types::Result;
use redis::aio::MultiplexedConnection;
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Clone)]
pub struct RedisReader {
    redis: (redis::Client, MultiplexedConnection), // TKTK
    store_for: Duration,
}

impl RedisReader {
    pub async fn new<T: AsRef<str>>(redis_url: T, store_for: Duration) -> anyhow::Result<Self> {
        let redis_str = redis_url.as_ref();
        let client = redis::Client::open(redis_str)?;

        // TODO: make error_msg lazy
        let error_msg = format!("failed to connect to {}", redis_str);
        let conn = client
            .get_multiplexed_async_std_connection()
            .await
            .context(error_msg)?;

        Ok(RedisReader {
            store_for,
            redis: (client, conn)
        })
    }
}

#[derive(Serialize)]
struct PackumentCacheEntry<'a> {
    packument: &'a str,
    meta: &'a PackageMetadata
}

#[derive(Deserialize)]
struct ReadPackumentCacheEntry {
    packument: String,
    meta: PackageMetadata
}

#[async_trait]
impl WritableStore for RedisReader {
    async fn write_packument<T, W>(&self, package: T, mut packument_reader: W, meta: PackageMetadata) -> Result<Option<bool>>
        where T: AsRef<str> + Send + Sync,
              W: AsyncBufRead + Send + Sync + Unpin {
        let mut packument_bytes = Vec::with_capacity(4096);
        packument_reader.read_to_end(&mut packument_bytes).await?;

        let packument_string = unsafe {
            std::str::from_utf8_unchecked(&packument_bytes)
        };

        let cache_entry = PackumentCacheEntry {
            packument: packument_string,
            meta: &meta
        };

        let entry_bytes = serde_json::to_vec(&cache_entry)?;
        let expires = self.store_for
            .num_seconds()
            .to_string();

        let cache_key = format!("p {}", package.as_ref());
        let mut connection = self.redis.1.clone();
        redis::cmd("SETEX")
            .arg(&[cache_key.as_bytes(), expires.as_bytes(), entry_bytes.as_slice()])
            .query_async::<MultiplexedConnection, ()>(&mut connection).await?;

        Ok(Some(true))
    }

    async fn write_tarball<T, S, W>(
        &self,
        package: T,
        version: S,
        tarball_reader: W,
        meta: PackageMetadata
    ) -> Result<Option<bool>>
    where
        T: AsRef<str> + Send + Sync,
        S: AsRef<str> + Send + Sync,
        W: AsyncBufRead + Send + Sync
    {
        let package_str = package.as_ref();
        let version_str = version.as_ref();
        let cache_key = format!("t {} {}", package_str, version_str);
        let mut connection = self.redis.1.clone();

        let meta_bytes = serde_json::to_vec(&meta)?;
        let len = (meta_bytes.len() as u32).to_be_bytes();

        let mut cache_entry = Vec::with_capacity(4096);
        std::io::Write::write_all(&mut cache_entry, &len)?;
        std::io::Write::write_all(&mut cache_entry, &meta_bytes)?;
        futures::io::copy(tarball_reader, &mut cache_entry).await?;

        let expires = self.store_for
            .num_seconds()
            .to_string();

        redis::cmd("SETEX")
            .arg(&[cache_key.as_bytes(), expires.as_bytes(), &cache_entry])
            .query_async::<MultiplexedConnection, ()>(&mut connection).await?;

        let mut tarball = futures::io::Cursor::new(cache_entry);
        tarball.seek(futures::io::SeekFrom::Start(4 + meta_bytes.len() as u64)).await?;
        Ok(Some(true))
    }
}

#[async_trait]
impl ReadableStore for RedisReader {
    type PackumentReader = futures::io::Cursor<Vec<u8>>;
    type TarballReader = futures::io::Cursor<Vec<u8>>;

    async fn get_packument<T>(&self, package: T) -> Result<Option<(Self::PackumentReader, PackageMetadata)>>
    where
        T: AsRef<str> + Send + Sync,
    {
        let package_str = package.as_ref();
        let cache_key = format!("p {}", package_str);
        let mut connection = self.redis.1.clone();
        let cached = redis::cmd("GET")
            .arg(&[&cache_key])
            .query_async::<MultiplexedConnection, Option<Vec<u8>>>(&mut connection).await?;

        match cached {
            Some(entry_bytes) => {
                info!("packument cache hit for {}", package_str);
                let entry: ReadPackumentCacheEntry = serde_json::from_slice(&entry_bytes[..])?;
                Ok(Some((futures::io::Cursor::new(entry.packument.into_bytes()), entry.meta)))
            },
            None => Ok(None)
        }
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
        let package_str = package.as_ref();
        let version_str = version.as_ref();
        let cache_key = format!("t {} {}", package_str, version_str);

        let mut connection = self.redis.1.clone();
        let cached = redis::cmd("GET")
            .arg(&[&cache_key])
            .query_async::<MultiplexedConnection, Option<Vec<u8>>>(&mut connection).await?;

        match cached {
            Some(tarball_bytes) => {
                info!("tarball cache hit for {} {}", package_str, version_str);

                // 4 byte metadata size, followed by metadata, followed by tarball
                let mut meta_size_bytes = [0u8; 4];
                meta_size_bytes.copy_from_slice(&tarball_bytes[0..4]);
                let meta_size = u32::from_be_bytes(meta_size_bytes) as usize;
                let meta: PackageMetadata = serde_json::from_slice(&tarball_bytes[4..(4 + meta_size)])?;
                let mut tarball = futures::io::Cursor::new(tarball_bytes);
                tarball.seek(futures::io::SeekFrom::Start(4 + meta_size as u64)).await?;

                Ok(Some((tarball, meta)))
            },

            None => Ok(None)
        }
    }
}
