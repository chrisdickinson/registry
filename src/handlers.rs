use tide::{Next, Request, Response};
use crate::stores::ReadableStore;
use http_types::Result;
use tracing::{error, info, span, Level};

pub async fn get_packument<State>(req: Request<State>) -> Result<Response> {
    let package: String = req.param("pkg").unwrap();

    info!("get packument {}", package);
    let response = surf::get(format!("https://registry.npmjs.org/{}", package)).await?;
    Ok(Response::new(response.status()).body(response))
}

pub async fn put_packument<State>(req: Request<State>) -> Result<&'static str> {
    Ok("put packument")
}

pub async fn get_tarball<State>(req: Request<State>) -> Result<&'static str> {
    Ok("get tarball")
}

pub async fn get_scoped_tarball<State>(req: Request<State>) -> Result<&'static str> {
    Ok("get scoped tarball")
}

pub async fn post_login<State>(req: Request<State>) -> Result<&'static str> {
    Ok("post login")
}

pub async fn get_login_poll<State>(req: Request<State>) -> Result<&'static str> {
    Ok("get login poll")
}
