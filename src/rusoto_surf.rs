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
use std::task::{Context, Poll};
use std::time::Duration;

#[derive(Default)]
pub struct SurfRequestDispatcher;

impl SurfRequestDispatcher {
    pub fn new() -> Self {
        SurfRequestDispatcher {}
    }
}

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

    if let Some(payload) = rusoto_req.payload.take() {
        pending_req = match payload {
            rusoto_core::signature::SignedRequestPayload::Buffer(xs) => pending_req.body_bytes(xs),
            rusoto_core::signature::SignedRequestPayload::Stream(xs) => {
                let async_read = AsyncReader::new(xs);
                let bufreader = async_std::io::BufReader::new(async_read);
                pending_req.body(bufreader)
            }
        };
    }

    pending_req.await
}

struct AsyncReader {
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

struct Streamer(surf::Response);

impl Stream for Streamer {
    type Item = std::io::Result<bytes::Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        use async_std::io::BufRead;

        let result: Poll<Option<Result<_, std::io::Error>>> = match futures::ready!(Pin::new(&mut self.0).poll_fill_buf(cx)) {
            Ok(bytes) => {
                if bytes.is_empty() {
                    return Poll::Ready(None)
                } else {
                    Poll::Ready(Some(Ok(bytes::Bytes::copy_from_slice(bytes))))
                }
            }
            Err(e) => return Poll::Ready(Some(Err(e))),
        };

        let response = &mut self.0;
        match result {
            Poll::Ready(Some(Ok(bytes))) => {
                Pin::new(response).consume(bytes.len());
                Poll::Ready(Some(Ok(bytes)))
            },
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending
        }
    }
}

impl DispatchSignedRequest for SurfRequestDispatcher {
    fn dispatch(
        &self,
        request: SignedRequest,
        _timeout: Option<Duration>,
    ) -> rusoto_core::request::DispatchSignedRequestFuture {
        let response_fut = surf_req(request);

        let result = response_fut
            .then(async move |result| {
                let response = result.unwrap();
                HttpResponse {
                    status: response.status().into(),
                    headers: HeaderMap::<String>::default(),
                    body: ByteStream::new(Streamer(response)),
                }
            })
            .map(Result::Ok);

        Box::pin(result)
    }
}
