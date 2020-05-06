use rusoto_s3::{ GetObjectRequest, S3, S3Client };
use async_trait::async_trait;
use http_types::Result;
use chrono::Utc;

use crate::rusoto_surf::AsyncReader;
use crate::stores::{PackageMetadata, ReadableStore};

#[derive(Clone)]
pub struct S3Store<S3: rusoto_s3::S3> {
    bucket: String,
    client: S3
}

impl<S3: rusoto_s3::S3> S3Store<S3> {
    pub fn new<T: AsRef<str>>(bucket: T, client: S3) -> Self {
        S3Store {
            bucket: bucket.as_ref().to_owned(),
            client
        }
    }
}

pub struct TokioAsyncReadWrapper(Box<dyn tokio::io::AsyncRead + Send + Sync + Unpin>);

use std::pin::Pin;
use std::task::{Context, Poll};
impl futures::io::AsyncRead for TokioAsyncReadWrapper {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        mut buf: &mut [u8],
    ) -> Poll<async_std::io::Result<usize>> {

        use tokio::io::AsyncRead;
        match futures::ready!(Pin::new(&mut self.0).poll_read(cx, &mut buf)) {
            Ok(0) => Poll::Ready(Ok(0)),
            Ok(n) => Poll::Ready(Ok(n)),
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}


#[async_trait]
impl<S3: rusoto_s3::S3 + Send + Sync> ReadableStore for S3Store<S3> {
    type PackumentReader = futures::io::BufReader<TokioAsyncReadWrapper>;
    type TarballReader = futures::io::BufReader<AsyncReader>;

    async fn get_packument<T>(&self, package: T) -> Result<Option<(Self::PackumentReader, PackageMetadata)>>
    where
        T: AsRef<str> + Send + Sync,
    {

        let package_str = package.as_ref();
        let resp = self.client.get_object(GetObjectRequest {
            bucket: self.bucket.clone(),
            key: format!("packages/{}", package_str),
            ..Default::default()
        }).await?;

        if let Some(payload) = resp.body {
            let boxed = Box::new(payload.into_async_read()) as Box<dyn tokio::io::AsyncRead + Send + Unpin>;
            let wrapped = TokioAsyncReadWrapper(boxed);
            let reader = futures::io::BufReader::new(wrapped);
            return Ok(Some((reader, PackageMetadata {
                integrity: "".to_owned(),
                last_fetched_at: Utc::now()
            })));
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
        let package_str = package.as_ref();
        let version_str = version.as_ref();
        let resp = self.client.get_object(GetObjectRequest {
            bucket: self.bucket.clone(),
            key: format!("tarballs/{}/{}", package_str, version_str),
            ..Default::default()
        }).await?;

        if let Some(body) = resp.body {
            let reader = AsyncReader::new(body);

            return Ok(Some((futures::io::BufReader::new(reader), PackageMetadata {
                integrity: "".to_owned(),
                last_fetched_at: Utc::now()
            })))
        }

        Ok(None)
    }
}