use crate::stores::{PackageMetadata, ReadableStore, WritableStore};
use async_trait::async_trait;
use chrono::{Duration, Utc};
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
    upstream_url: String,
    fetch_after: Duration,
}

impl<R: ReadableStore + Send + Sync> ReadThrough<R> {
    pub fn new<T: AsRef<str>>(upstream_url: T, inner_store: R, fetch_after: Duration) -> Self {
        ReadThrough {
            inner_store,
            upstream_url: upstream_url.as_ref().to_string(),
            fetch_after,
        }
    }
}

#[async_trait]
impl<R: ReadableStore + Send + Sync> ReadableStore for ReadThrough<R> {
    type Reader = surf::Response;

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
            StatusCode::NotFound => Ok(None),
            _ => {
                // TODO: return a http_types::Error result here
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

/*
pub struct ReadThrough<R: AsyncRead + AsyncReadExt + std::marker::Unpin, T: ReadableStore<R> + WritableStore<R>> {
    inner_store: T,
    upstream_url: String,
    allow: Option<HashSet<String>>,
    block: Option<HashSet<String>>,
    fetch_after: Duration,
    _pd: PhantomData<R>
}

impl<Store: ReadableStore + WritableStore> ReadableStore for ReadThrough<Store> {
    fn get_packument_raw<T: AsRef<str>>(&self, package: T) -> Option<(Box<dyn Read>, [u8; 32])> {

        if let Some(ref block) = self.block {
            if block.has(package.as_ref().to_string()) {
                return None
            }
        }

        if let Some(ref allow) = self.allow {
            if !allow.has(package.as_ref().to_string()) {
                return None
            }
        }

        if let Some((reader, metadata)) = self.inner_store.get_packument_raw(package) {
            let now = Utc::now();
            let dur = now.signed_duration_since(metadata.last_fetched_at);

            if dur >= self.fetch_after {
                // refetch and update. if it's a 304, update
                // the metadata and store that, otherwise walk
                // and grab each tarball and store those.
            }

            Some((reader, metadata))
        }

        // Ok, go fetch it and store it in the inner store.

        None
    }

}
*/
