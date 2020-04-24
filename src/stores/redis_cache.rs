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

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    packument: Packument,
    meta: PackageMetadata
}

#[async_trait]
impl<R: ReadableStore + Send + Sync> ReadableStore for RedisCache<R> {
    type Reader = surf::Response;

    async fn get_packument<T>(&self, package: T) -> Result<Option<(Packument, PackageMetadata)>>
    where
        T: AsRef<str> + Send + Sync,
    {
        // grab the thing

        let package_str = package.as_ref();

        let mut connection = self.redis.1.clone();
        let cached = redis::cmd("GET")
            .arg(&[package_str])
            .query_async::<MultiplexedConnection, Option<Vec<u8>>>(&mut connection).await?;

        match cached {
            Some(entry_bytes) => {
                info!("Cache hit for {}", package_str);
                let entry: CacheEntry = serde_json::from_slice(&entry_bytes[..])?;
                Ok(Some((entry.packument, entry.meta)))
            },

            None => {
                info!("Cache miss for {}", package_str);
                let inner_result = self.inner_store.get_packument(package_str).await?;
                if inner_result.is_none() {
                    return Ok(None)
                }

                let (packument, meta) = inner_result.unwrap();

                let cache_entry = CacheEntry {
                    packument,
                    meta
                };
                let entry_bytes = serde_json::to_vec(&cache_entry)?;
                let expires = self.store_for
                    .num_seconds()
                    .to_string();

                redis::cmd("SETEX")
                    .arg(&[package_str.as_bytes(), expires.as_bytes(), entry_bytes.as_slice()])
                    .query_async::<MultiplexedConnection, ()>(&mut connection).await?;

                Ok(Some((cache_entry.packument, cache_entry.meta)))
            }
        }
    }

    async fn get_tarball<T, S>(
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

    async fn get_packument_raw<T>(
        &self,
        package: T,
    ) -> Result<Option<(Self::Reader, PackageMetadata)>>
    where
        T: AsRef<str> + Send + Sync,
    {
        Ok(None)
    }
}
