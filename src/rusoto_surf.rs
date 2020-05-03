use rusoto_core::DispatchSignedRequest;
use rusoto_core::request::HttpResponse;
use rusoto_core::signature::SignedRequest;
use std::time::Duration;
use http::HeaderMap;
use futures::FutureExt;
use std::convert::TryInto;
use std::marker::Unpin;
use std::pin::Pin;

#[derive(Default)]
pub struct SurfRequestDispatcher;

impl SurfRequestDispatcher {
    pub fn new () -> Self {
        SurfRequestDispatcher { }
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
            rusoto_core::signature::SignedRequestPayload::Buffer(xs) => {
                pending_req.body_bytes(xs)
            },
            rusoto_core::signature::SignedRequestPayload::Stream(_xs) => {
                // TODO: how do we pipe a Stream into a surf request?
                pending_req
            }
        };
    }

    pending_req.await
}

// TODO: This is a failed attempt at turning our AsyncRead Response object into a Stream suitable
// for passing to the rusoto Response payload ByteStream.
//
// It very nearly worked: the bytestream would attempt to read via poll_next, and we'd forward the
// read request in to the surf Response object. HOWEVER. Surf's Response object had already
// consumed the response data, so we'd always get "0" bytes back.
// 
// Since the data is already consumed, I moved to the "body_bytes()" solution you see below, at
// the next TODO.
struct AsyncReadWrapper<R: futures::io::AsyncRead + Unpin> {
    inner: R
}

impl <R: futures::io::AsyncRead + Unpin + std::fmt::Debug> futures::stream::Stream for AsyncReadWrapper<R> {
    type Item = Result<bytes::Bytes, std::io::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut futures::task::Context) -> futures::task::Poll<Option<Self::Item>> {

        let myself = self.get_mut();
        dbg!(&myself.inner);
        let value = &mut myself.inner;
        let pinned = Pin::new(value);
        let mut buf = Vec::new();
        match futures::io::AsyncRead::poll_read(pinned, cx, &mut buf) {
            futures::task::Poll::Ready(result) => {
                match result {
                    Ok(bytes_read) => {
                        let bytes_read = dbg!(bytes_read);
                        if bytes_read == 0 {
                            return futures::task::Poll::Ready(None);
                        }
                        futures::task::Poll::Ready(Some(Ok(bytes::Bytes::copy_from_slice(&buf[0..bytes_read]))))
                    },
                    Err(_e) => {
                        futures::task::Poll::Ready(None)
                    }
                }
            },
            futures::task::Poll::Pending => futures::task::Poll::Pending,
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

        let result = response_fut.then(async move |result| {
            let mut response = result.unwrap();

            // TODO: Make this stream, vs. buffer the entire response in memory.
            let body_bytes = response.body_bytes().await.unwrap();

            HttpResponse {
                status: response.status().into(),
                headers: HeaderMap::<String>::default(),
                body: body_bytes.into()
            }
        }).map(Result::Ok);

        Box::pin(result)
    }
}

