use axum::body::Bytes;
use futures::stream::BoxStream;

use crate::models::{PackageIdentifier, Packument};

pub(crate) mod read_through;
pub(crate) mod remote;

#[async_trait::async_trait]
pub trait PackageStorage: Send + Sync {
    type Error: Into<axum::BoxError> + Send + Sync + 'static;
    async fn fetch_packument(&self, name: &PackageIdentifier) -> anyhow::Result<Packument> {
        let stream = self.stream_packument(name).await?;
        use futures::TryStreamExt;

        let data: Vec<Bytes> = stream.try_collect().await.map_err(|e| {
            let box_error: axum::BoxError = e.into();
            anyhow::anyhow!(box_error)
        })?;
        let data = data.as_slice().concat();

        Ok(serde_json::from_slice(data.as_slice())?)
    }

    async fn stream_packument(
        &self,
        name: &PackageIdentifier,
    ) -> anyhow::Result<BoxStream<'static, Result<Bytes, Self::Error>>>;

    async fn stream_tarball(
        &self,
        name: &PackageIdentifier,
        version: &str,
    ) -> anyhow::Result<BoxStream<'static, Result<Bytes, Self::Error>>>;
}
