use anyhow::Context;
use async_trait::async_trait;
use chrono::{Duration, Utc};
use crate::packument::Packument;
use crate::stores::{PackageMetadata, ReadableStore};
use futures::prelude::*;
use http_types::Result;
use redis::AsyncCommands;
use redis::aio::MultiplexedConnection;
use serde::{Deserialize, Serialize};
use tide::http::StatusCode;
use tracing::{error, info, span, Level};

#[derive(Clone)]
pub struct RedisCache<R: ReadableStore + Send + Sync> {
    inner_store: R,
    redis: (redis::Client, MultiplexedConnection), // TKTK
    store_for: Duration,
}

impl<R: ReadableStore + Send + Sync> RedisCache<R> {
    pub async fn new<T: AsRef<str>>(redis_url: T, inner_store: R, store_for: Duration) -> anyhow::Result<Self> {
        let redis_str = redis_url.as_ref();
        let client = redis::Client::open(redis_str)?;

        // TODO: make error_msg lazy
        let error_msg = format!("failed to connect to {}", redis_str);
        let conn = client
            .get_multiplexed_async_std_connection()
            .await
            .context(error_msg)?;

        Ok(RedisCache {
            inner_store,
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
impl<R: ReadableStore + Send + Sync> ReadableStore for RedisCache<R> {
    type PackumentReader = futures::io::Cursor<Vec<u8>>;
    type TarballReader = futures::future::Either<futures::io::Cursor<Vec<u8>>, R::TarballReader>;

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

            None => {
                info!("packument cache miss for {}", package_str);
                let inner_result = self.inner_store.get_packument(package_str).await?;
                if inner_result.is_none() {
                    return Ok(None)
                }

                let (mut packument_reader, meta) = inner_result.unwrap();

                // do the dumb thing that works
                let mut packument_bytes = Vec::with_capacity(4096);
                packument_reader.read_to_end(&mut packument_bytes).await?;

                {
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

                    redis::cmd("SETEX")
                        .arg(&[cache_key.as_bytes(), expires.as_bytes(), entry_bytes.as_slice()])
                        .query_async::<MultiplexedConnection, ()>(&mut connection).await?;

                }

                Ok(Some((futures::io::Cursor::new(packument_bytes), meta)))
            }
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
                info!("packument cache hit for {} {}", package_str, version_str);
                return Ok((Some))
            },

            None => {
                info!("packument cache miss for {} {}", package_str, version_str);

            }
        }
        // read that tarball
        Ok(None)
    }
}
