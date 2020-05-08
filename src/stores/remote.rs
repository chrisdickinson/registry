use crate::packument::Packument;
use crate::stores::{PackageMetadata, ReadableStore};
use async_trait::async_trait;
use chrono::Utc;
use futures::prelude::*;
use http_types::Result;
use tracing::info;
use tide::http::StatusCode;

#[derive(Clone)]
pub struct RemoteStore {
    public_hostname: String,
    upstream_url: String
}

impl RemoteStore {
    pub fn new<T: AsRef<str>>(public_hostname: T, upstream_url: T) -> Self {
        RemoteStore {
            public_hostname: public_hostname.as_ref().to_string(),
            upstream_url: upstream_url.as_ref().to_string()
        }
    }
}

impl RemoteStore {
    async fn get_packument_raw<T>(
        &self,
        package: T,
    ) -> Result<Option<(surf::Response, PackageMetadata)>>
    where
        T: AsRef<str> + Send + Sync,
    {
        let path = package.as_ref().to_string().replace("/", "%2F");
        let url = format!("{}/{}", &self.upstream_url, &path);
        info!("ReadThrough packument request to {}", url);

        let response = surf::get(url).await?;

        match response.status() {
            StatusCode::Ok => {
                Ok(Some((
                    response,
                    PackageMetadata {
                        integrity: String::from(""),
                        last_fetched_at: Utc::now(),
                    },
                )))
            },
            StatusCode::NotFound => Ok(None),
            _ => {
                // TODO: return a http_types::Error result here
                Ok(None)
            }
        }
    }
}

#[async_trait]
impl ReadableStore for RemoteStore {
    type PackumentReader = futures::io::Cursor<Vec<u8>>;
    type TarballReader = surf::Response;

    async fn get_packument<T>(&self, package: T) -> Result<Option<(Self::PackumentReader, PackageMetadata)>>
    where
        T: AsRef<str> + Send + Sync,
    {
        if let Some((mut reader, meta)) = self.get_packument_raw(package).await? {
            let mut bytes = Vec::with_capacity(4096);
            reader.read_to_end(&mut bytes).await?;

            let mut packument: Packument = serde_json::from_slice(&bytes[..])?;
            for mut version_data in &mut packument.versions.values_mut() {
                version_data.dist.tarball = version_data.dist.tarball.replace(&self.upstream_url, &self.public_hostname);
            }

            let entry_bytes = serde_json::to_vec(&packument)?;
            return Ok(Some((futures::io::Cursor::new(entry_bytes), meta)));
        }

        Ok(None)
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
        // @foo/bar @ version -> /@foo/bar/-/bar-version.tgz
        // bar @ version -> /bar/-/bar-version.tgz

        let package_str = package.as_ref();
        let version_str = version.as_ref();
        if package_str.is_empty() || version_str.is_empty() {
            // bail
            return Ok(None)
        }

        let url = if &package_str[0..1] == "@" {
            let bits: Vec<_> = package_str.split('/').take(2).collect();
            let (scope, name) = (&bits[0], &bits[1]);
            format!("{}/{}/{}/-/{}-{}.tgz", &self.upstream_url, scope, name, name, version_str)
        } else {
            format!("{}/{}/-/{}-{}.tgz", &self.upstream_url, package_str, package_str, version_str)
        };

        info!("ReadThrough tarball request to {}", url);

        let response = surf::get(url).await?;

        match response.status() {
            StatusCode::Ok => {
                Ok(Some((
                    response,
                    PackageMetadata {
                        integrity: String::from(""),
                        last_fetched_at: Utc::now(),
                    },
                )))
            },
            StatusCode::NotFound => Ok(None),
            _ => {
                Ok(None)
            }
        }
    }
}
