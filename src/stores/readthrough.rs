use crate::packument::Packument;
use crate::stores::{PackageMetadata, ReadableStore, WritableStore};
use async_trait::async_trait;
use chrono::Utc;
use futures::prelude::*;
use http_types::Result;
use std::collections::HashSet;
use std::marker::PhantomData;
use surf;
use tracing::{error, info, span, Level};
use tide::http::StatusCode;

#[derive(Clone)]
pub struct ReadThrough<R: ReadableStore + Send + Sync> {
    inner_store: R,
    public_hostname: String,
    upstream_url: String
}

impl<R: ReadableStore + Send + Sync> ReadThrough<R> {
    pub fn new<T: AsRef<str>>(public_hostname: T, upstream_url: T, inner_store: R) -> Self {
        ReadThrough {
            public_hostname: public_hostname.as_ref().to_string(),
            upstream_url: upstream_url.as_ref().to_string(),
            inner_store,
        }
    }
}

#[async_trait]
impl<R: ReadableStore + Send + Sync> ReadableStore for ReadThrough<R> {
    type Reader = surf::Response;

    async fn get_packument<T>(&self, package: T) -> Result<Option<(Packument, PackageMetadata)>>
    where
        T: AsRef<str> + Send + Sync,
    {
        if let Some((mut reader, meta)) = self.get_packument_raw(package).await? {
            let mut bytes = Vec::with_capacity(4096);
            reader.read_to_end(&mut bytes).await?;

            let mut packument: Packument = serde_json::from_slice(&bytes[..])?;
            for (_version_id, mut version_data) in &mut packument.versions {
                version_data.dist.tarball = version_data.dist.tarball.replace(&self.upstream_url, &self.public_hostname);
            }

            return Ok(Some((packument, meta)));
        }

        Ok(None)
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
        // @foo/bar @ version -> /@foo/bar/-/bar-version.tgz
        // bar @ version -> /bar/-/bar-version.tgz

        let package_str = package.as_ref();
        let version_str = version.as_ref();
        if package_str.len() == 0 || version_str.len() == 0 {
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
            StatusCode::NotFound => Ok(None), // defer to inner
            _ => {
                // TODO: return a http_types::Error result here, or maybe defer to inner
                Ok(None)
            }
        }
    }

    async fn get_packument_raw<T>(
        &self,
        package: T,
    ) -> Result<Option<(Self::Reader, PackageMetadata)>>
    where
        T: AsRef<str> + Send + Sync,
    {
        //        if let Some((reader, metadata)) = self.inner_store.get_packument_raw(package).await? {
        //            let now = Utc::now();
        //            let dur = now.signed_duration_since(metadata.last_fetched_at);
        //
        //            if dur >= self.fetch_after {
        //                // refetch and update. if it's a 304, update
        //                // the metadata and store that, otherwise walk
        //                // and grab each tarball and store those.
        //
        //            }
        //
        //            return Ok(Some((Box::new(reader), metadata)))
        //        }

        // Ok, go fetch it and store it in the inner store.

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
