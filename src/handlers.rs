use crate::stores::ReadableStore;
use http_types::Result;
use tide::{Next, Request, Response, http::StatusCode};
use tracing::{error, info, span, Level};

pub async fn get_packument<State: ReadableStore>(req: Request<State>) -> Result<Response> {
    let package: String = req.param("pkg").unwrap();

    info!("get packument {}", package);

    match req.state().get_packument(package).await? {
        Some((packument, meta)) => {
            Ok(Response::new(StatusCode::Ok).body_json(&packument)?)
        },
        None => {
            Ok(Response::new(StatusCode::NotFound))
        }
    }
}

pub async fn put_packument<State>(req: Request<State>) -> Result<&'static str> {
    Ok("put packument")
}

pub async fn get_tarball<State: ReadableStore>(req: Request<State>) -> Result<Response> {
    let package: String = req.param("pkg")?;
    let tarball: String = req.param("tarball")?;

    let version_plus_tgz = &(tarball.replace(&package, "")[1..]);
    let version = &version_plus_tgz[..version_plus_tgz.len() - 4];

    info!("get packument {}", package);

    match req.state().get_tarball(package, version).await? {
        Some((response, meta)) => {
            Ok(Response::new(StatusCode::Ok).body(response))
        },
        None => {
            Ok(Response::new(StatusCode::NotFound))
        }
    }
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
