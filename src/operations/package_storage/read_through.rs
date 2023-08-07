use std::path::{Path, PathBuf};

use crate::models::PackageIdentifier;
use crate::operations::PackageStorage;
use axum::body::Bytes;
use futures::stream::BoxStream;
use futures_util::{pin_mut, StreamExt};

#[derive(Clone, Debug)]
pub struct ReadThrough<R: PackageStorage + Clone + std::fmt::Debug + Send + Sync + 'static> {
    cache_dir: PathBuf,
    inner: R,
}

impl<R: PackageStorage + Clone + std::fmt::Debug + Send + Sync + 'static> ReadThrough<R> {
    pub fn new(cache_dir: impl AsRef<Path>, inner: R) -> Self {
        Self {
            cache_dir: PathBuf::from(cache_dir.as_ref()),
            inner,
        }
    }
}

#[async_trait::async_trait]
impl<R> PackageStorage for ReadThrough<R>
where
    R: PackageStorage + Clone + std::fmt::Debug + Send + Sync + 'static,
    <R as PackageStorage>::Error: std::error::Error + Send + Sync + 'static,
{
    type Error = std::io::Error;
    async fn stream_packument(
        &self,
        name: &PackageIdentifier,
    ) -> anyhow::Result<BoxStream<'static, Result<Bytes, Self::Error>>> {
        let key = format!("packument:{}", name);
        match cacache::Reader::open(&self.cache_dir, &key).await {
            Ok(reader) => Ok(tokio_util::io::ReaderStream::new(reader).boxed()),

            Err(cacache::Error::EntryNotFound(_, _)) => {
                use tokio::io::AsyncWriteExt;
                let stream = self.inner.stream_packument(name).await?;
                let mut writer =
                    cacache::Writer::create(self.cache_dir.as_path(), key.as_str()).await?;
                pin_mut!(stream);
                while let Some(chunk) = stream.next().await {
                    let Ok(chunk) = chunk else {
                        break;
                    };
                    writer.write_all(chunk.as_ref()).await?;
                }
                writer.commit().await?;

                return self.stream_packument(name).await;
            }
            Err(e) => return Err(e.into()),
        }
    }

    async fn stream_tarball(
        &self,
        name: &PackageIdentifier,
        version: &str,
    ) -> anyhow::Result<BoxStream<'static, Result<Bytes, Self::Error>>> {
        let key = format!("tarball:{}:{}", name, version);
        match cacache::Reader::open(&self.cache_dir, &key).await {
            Ok(reader) => Ok(tokio_util::io::ReaderStream::new(reader).boxed()),

            Err(cacache::Error::EntryNotFound(_, _)) => {
                use tokio::io::AsyncWriteExt;
                let stream = self.inner.stream_tarball(name, version).await?;
                let mut writer =
                    cacache::Writer::create(self.cache_dir.as_path(), key.as_str()).await?;
                pin_mut!(stream);
                while let Some(chunk) = stream.next().await {
                    let Ok(chunk) = chunk else {
                        break;
                    };
                    writer.write_all(chunk.as_ref()).await?;
                }
                writer.commit().await?;

                return self.stream_tarball(name, version).await;
            }
            Err(e) => return Err(e.into()),
        }
    }
}
