use futures::future::BoxFuture;
use http_types::Result;
use tide::{Middleware, Next, Request, Response};
use tracing::{error, info, span, Level};

#[derive(Debug)]
pub struct Logging {}

impl Logging {
    pub fn new() -> Self {
        Logging {}
    }
}

impl<Data: Send + Sync + 'static> Middleware<Data> for Logging {
    fn handle<'a>(
        &self,
        cx: Request<Data>,
        next: Next<'a, Data>,
    ) -> BoxFuture<'a, Result<Response>> {
        Box::pin(async move {
            let method = cx.method();
            let url = cx.uri();
            let _span = span!(Level::INFO, "handling request", method=?method, url=?url);
            let result = next.run(cx).await;

            match &result {
                Ok(r) => info!(status=?r.status()),
                Err(e) => error!(status=?e.status()),
            };

            result
        })
    }
}
