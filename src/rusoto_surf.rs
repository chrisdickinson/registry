use async_std::stream::StreamExt;
use bytes::BufMut;
use futures::FutureExt;
use futures::Stream;
use http::HeaderMap;
use rusoto_core::request::HttpResponse;
use rusoto_core::signature::SignedRequest;
use rusoto_core::ByteStream;
use rusoto_core::DispatchSignedRequest;
use std::convert::TryInto;
use std::pin::Pin;
use std::marker::Unpin;
use std::task::{Context, Poll};
use std::time::Duration;

#[derive(Default)]
pub struct SurfRequestDispatcher;

impl SurfRequestDispatcher {
    pub fn new() -> Self {
        SurfRequestDispatcher {}
    }
}

use std::str::FromStr;
async fn surf_req(mut rusoto_req: SignedRequest) -> surf::Result<surf::Response> {
    let mut final_uri = format!(
        "{}://{}{}",
        rusoto_req.scheme(),
        rusoto_req.hostname(),
        rusoto_req.canonical_path()
    );
    if !rusoto_req.canonical_query_string().is_empty() {
        final_uri = final_uri + &format!("?{}", rusoto_req.canonical_query_string());
    }

    let url: url::Url = final_uri.parse().unwrap();
    let mut pending_req = surf::Request::new(rusoto_req.method().try_into().unwrap(), url);

    for (header, value) in rusoto_req.headers {
        let header = http_types::headers::HeaderName::from_str(&header[..]).unwrap();
        let value = unsafe { std::str::from_utf8_unchecked(&value[0][..]) };
        pending_req = pending_req.set_header(header, value);
    }

    if let Some(payload) = rusoto_req.payload.take() {
        pending_req = match payload {
            rusoto_core::signature::SignedRequestPayload::Buffer(xs) => pending_req.body_bytes(xs),
            rusoto_core::signature::SignedRequestPayload::Stream(xs) => {
                let mut async_read = AsyncReader::new(xs);
                let mut bytes = Vec::with_capacity(4096);

                use futures::io::AsyncReadExt;
                async_read.read_to_end(&mut bytes).await?;
                pending_req.body_bytes(bytes)
            }
        };
    }

    pending_req.await
}

pub struct AsyncReader {
    buffer: bytes::BytesMut,
    stream: async_std::stream::Fuse<ByteStream>,
}

impl AsyncReader {
    pub fn new(stream: ByteStream) -> Self {
        AsyncReader {
            buffer: bytes::BytesMut::new(),
            stream: stream.fuse(),
        }
    }
}

impl async_std::io::Read for AsyncReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<async_std::io::Result<usize>> {
        if self.buffer.is_empty() {
            match futures::ready!(Pin::new(&mut self.stream).poll_next(cx)) {
                None => return Poll::Ready(Ok(0)),
                Some(Err(e)) => return Poll::Ready(Err(e)),
                Some(Ok(bytes)) => {
                    self.buffer.put(bytes);
                }
            }
        }
        let available = std::cmp::min(buf.len(), self.buffer.len());
        let bytes = self.buffer.split_to(available);
        let (left, _) = buf.split_at_mut(available);
        left.copy_from_slice(&bytes[..available]);
        Poll::Ready(Ok(available))
    }
}

use futures::io::AsyncRead;
pub(crate) struct Streamer<R: AsyncRead + Unpin>(R);

impl<R: AsyncRead + Unpin> Streamer<R> {
    pub(crate) fn new(xs: R) -> Self {
        Streamer(xs)
    }
}


impl<R: AsyncRead + Unpin> Stream for Streamer<R> {
    type Item = std::io::Result<bytes::Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut buf = vec![0; 1024];
        match futures::ready!(Pin::new(&mut self.0).poll_read(cx, &mut buf)) {
            Ok(0) => Poll::Ready(None),
            Ok(n) => Poll::Ready(Some(Ok(bytes::Bytes::copy_from_slice(&buf[0..n])))),
            Err(e) => Poll::Ready(Some(Err(e))),
        }
    }
}

use rusoto_core::request::HttpDispatchError;
impl DispatchSignedRequest for SurfRequestDispatcher {
    fn dispatch(
        &self,
        request: SignedRequest,
        _timeout: Option<Duration>,
    ) -> rusoto_core::request::DispatchSignedRequestFuture {
        let response_fut = surf_req(request);

        let result = response_fut
            .then(|result| async move {
                let response = match result {
                    Ok(x) => x,
                    Err(e) => {
                        return Err(HttpDispatchError::new(e.status().canonical_reason().to_owned()))
                    }
                };

                // TODO: make this better.
                use http_types::headers::HeaderName;
                let mut headers = HeaderMap::<String>::default();
                let header_names = response.header_names().map(|xs| { xs.as_str().to_owned() });
                for name in header_names {
                    let header = HeaderName::from_str(&name[..]).unwrap();
                    let value = response.header(&header).unwrap()[0].as_str().to_owned();
                    headers.insert(http::header::HeaderName::from_bytes(name.as_bytes()).unwrap(), value);
                }

                let status = http::status::StatusCode::from_u16(response.status().into()).unwrap();
                Ok(HttpResponse {
                    status,
                    headers,
                    body: ByteStream::new(Streamer::new(response)),
                })
            });

        Box::pin(result)
    }
}
