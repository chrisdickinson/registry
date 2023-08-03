use std::path::{Path, PathBuf};

use crate::models::{PackageIdentifier, Packument};
use crate::operations::{FetchPackument, StreamPackument, StreamTarball};
use axum::body::Bytes;
use futures::stream::BoxStream;
use futures_util::{StreamExt, TryStreamExt};

#[derive(Clone, Debug)]
pub struct RemoteRegistry {
    registry: String,
}

impl Default for RemoteRegistry {
    fn default() -> Self {
        Self {
            registry: "https://registry.npmjs.org".to_string(),
        }
    }
}

#[async_trait::async_trait]
impl FetchPackument for RemoteRegistry {
    async fn fetch(&self, name: &PackageIdentifier) -> anyhow::Result<Packument> {
        let pkg: Result<Packument, _> = reqwest::get(format!("{}/{}", self.registry, name))
            .await?
            .json()
            .await;

        if let Err(ref e) = pkg {
            tracing::error!(
                "Failed to fetch packument; name={name}, error={error}",
                name = name,
                error = e
            );
        }

        pkg.map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl StreamPackument for RemoteRegistry {
    type Error = reqwest::Error;
    async fn stream(
        &self,
        name: &PackageIdentifier,
    ) -> anyhow::Result<BoxStream<'static, Result<Bytes, Self::Error>>> {
        Ok(reqwest::get(format!("{}/{}", self.registry, name))
            .await?
            .bytes_stream()
            .boxed())
    }
}

#[async_trait::async_trait]
impl StreamTarball for RemoteRegistry {
    type Error = reqwest::Error;
    async fn stream(
        &self,
        pkg: &PackageIdentifier,
        version: &str,
    ) -> anyhow::Result<BoxStream<'static, Result<Bytes, Self::Error>>> {
        let url = if let Some(ref scope) = pkg.scope {
            format!(
                "{}/@{}/{}/-/{}-{}.tgz",
                self.registry, scope, pkg.name, pkg.name, version
            )
        } else {
            format!(
                "{}/{}/-/{}-{}.tgz",
                self.registry, pkg.name, pkg.name, version
            )
        };

        Ok(reqwest::get(url).await?.bytes_stream().boxed())
    }
}

#[derive(Clone, Debug)]
struct ReadThrough<
    R: StreamTarball
        + StreamPackument
        + FetchPackument
        + Clone
        + std::fmt::Debug
        + Send
        + Sync
        + 'static,
> {
    cache_dir: PathBuf,
    inner: R,
}

impl<
        R: StreamTarball
            + StreamPackument
            + FetchPackument
            + Clone
            + std::fmt::Debug
            + Send
            + Sync
            + 'static,
    > ReadThrough<R>
{
    fn new(cache_dir: impl AsRef<Path>, inner: R) -> Self {
        Self {
            cache_dir: PathBuf::from(cache_dir.as_ref()),
            inner,
        }
    }
}

#[async_trait::async_trait]
impl<R> StreamPackument for ReadThrough<R>
where
    R: StreamTarball
        + StreamPackument
        + FetchPackument
        + Clone
        + std::fmt::Debug
        + Send
        + Sync
        + 'static,
{
    type Error = Box<dyn std::error::Error + Send + Sync + 'static>;
    async fn stream(
        &self,
        name: &PackageIdentifier,
    ) -> anyhow::Result<BoxStream<'static, Result<Bytes, Self::Error>>> {
        let key = format!("packument:{}", name);
        match cacache::Reader::open(&self.cache_dir, &key).await {
            Ok(_reader) => {}

            Err(cacache::Error::EntryNotFound(_, _)) => {
                let _stream = StreamPackument::stream(&self.inner, name).await?;
            }
            Err(e) => return Err(e.into()),
        }

        todo!();
    }
}
