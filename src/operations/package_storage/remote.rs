use crate::models::PackageIdentifier;
use crate::operations::PackageStorage;
use axum::body::Bytes;
use futures::stream::BoxStream;
use futures_util::StreamExt;

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
impl PackageStorage for RemoteRegistry {
    type Error = reqwest::Error;
    async fn stream_packument(
        &self,
        name: &PackageIdentifier,
    ) -> anyhow::Result<BoxStream<'static, Result<Bytes, Self::Error>>> {
        Ok(reqwest::get(format!("{}/{}", self.registry, name))
            .await?
            .bytes_stream()
            .boxed())
    }

    async fn stream_tarball(
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
