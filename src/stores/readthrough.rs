use tracing::{error, info, span, Level};
use crate::stores::{ReadableStore, WritableStore, PackageMetadata};
use chrono::{Duration, Utc};
use async_trait::async_trait;
use futures::prelude::*;
use std::collections::HashSet;
use std::marker::PhantomData;
use http_types::Result;
use surf;

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
            fetch_after
        }
    }
}

#[async_trait]
impl<R: ReadableStore + Send + Sync> ReadableStore for ReadThrough<R> {
    type Reader = surf::Response;

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
        info!("ReadThrough request to {}", url);
        let response = surf::get(url).await?;
        Ok(Some((response, PackageMetadata {
            integrity: String::from(""),
            last_fetched_at: Utc::now()
        })))
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
