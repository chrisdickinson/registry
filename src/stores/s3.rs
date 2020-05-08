use rusoto_core::RusotoError;
use rusoto_s3::{
    GetObjectOutput,
    GetObjectRequest,
    GetObjectError,
    HeadObjectOutput,
    HeadObjectRequest,
    HeadObjectError,
    PutObjectOutput,
    PutObjectRequest,
    PutObjectError,
    S3
};
use futures::io::AsyncBufRead;
use async_trait::async_trait;
use http_types::Result;
use chrono::Utc;
use rusoto_core::ByteStream;

use tracing::info;
use crate::rusoto_surf::AsyncReader;
use crate::stores::{PackageMetadata, ReadableStore, WritableStore};
use crate::rusoto_surf::Streamer;

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

pub struct TokioAsyncReadWrapper(Box<dyn tokio::io::AsyncRead + Send + Unpin>);

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

unsafe impl Sync for TokioAsyncReadWrapper {}

#[async_trait]
impl<S3: rusoto_s3::S3 + Send + Sync> WritableStore for S3Store<S3> {
    async fn write_packument<T, W>(&self, package: T, packument_reader: W, meta: PackageMetadata) -> Result<Option<bool>>
        where T: AsRef<str> + Send + Sync,
              W: AsyncBufRead + Send + Sync + Unpin + 'static {

        // HEAD object request
        let package_str = package.as_ref();
        let key = format!("packages/{}/packument.json", package_str);
        let result = self.client.head_object(HeadObjectRequest {
            bucket: self.bucket.clone(),
            key: key.clone(),
            ..Default::default()
        }).await;

        match result {
            Ok(_xs) => {
                // no need to put the object back
                // TODO: compare xs.last_modified to incoming meta
                info!("skipping write, already present");
                return Ok(Some(false))
            },
            Err(RusotoError::Service(HeadObjectError::NoSuchKey(_))) => {},
            Err(RusotoError::Unknown(rusoto_core::request::BufferedHttpResponse { status: http::StatusCode::NOT_FOUND, body: _, headers: _ })) => {},
            Err(e) => return Err(e.into())
        };

        // then PUT object request if DNE
        self.client.put_object(PutObjectRequest {
            bucket: self.bucket.clone(),
            key: key.clone(),
            body: Some(ByteStream::new(Streamer::new(packument_reader))),
            metadata: Some(meta.into()),
            ..Default::default()
        }).await?;
        Ok(Some(true))
    }

    async fn write_tarball<T, S, W>(
        &self,
        package: T,
        version: S,
        tarball_reader: W,
        meta: PackageMetadata
    ) -> Result<Option<bool>>
    where
        T: AsRef<str> + Send + Sync,
        S: AsRef<str> + Send + Sync,
        W: AsyncBufRead + Send + Sync + Unpin + 'static
    {

        // HEAD object request
        let package_str = package.as_ref();
        let version_str = version.as_ref();
        let key = format!("packages/{}/{}.tgz", package_str, version_str);
        let result = self.client.head_object(HeadObjectRequest {
            bucket: self.bucket.clone(),
            key: key.clone(),
            ..Default::default()
        }).await;

        match result {
            Ok(_xs) => {
                // no need to put the object back
                // TODO: compare xs.last_modified to incoming meta
                return Ok(Some(false))
            },
            Err(e) => {
                if let RusotoError::Service(HeadObjectError::NoSuchKey(_)) = e {
                } else {
                    return Err(e.into())
                }
            }
        };

        // then PUT object request if DNE
        self.client.put_object(PutObjectRequest {
            bucket: self.bucket.clone(),
            key: key.clone(),
            body: Some(ByteStream::new(Streamer::new(tarball_reader))),
            metadata: Some(meta.into()),
            ..Default::default()
        }).await?;
        Ok(Some(true))
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
        let result = self.client.get_object(GetObjectRequest {
            bucket: self.bucket.clone(),
            key: format!("packages/{}/packument.json", package_str),
            ..Default::default()
        }).await;

        let resp = match result {
            Ok(xs) => xs,
            Err(e) => {
                if let RusotoError::Service(GetObjectError::NoSuchKey(_)) = e {
                    return Ok(None)
                }
                return Err(dbg!(e).into())
            }
        };

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
            key: format!("packages/{}/{}.tgz", package_str, version_str),
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
