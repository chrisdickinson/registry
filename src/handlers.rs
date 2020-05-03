use crate::stores::ReadableStore;
use http_types::Result;
use tide::{Request, Response, http::StatusCode};
use tracing::info;

pub async fn get_packument<State: ReadableStore>(req: Request<State>) -> Result<Response> {
    let package: String = req.param("pkg").unwrap();

    info!("get packument {}", package);

    match req.state().get_packument(package).await? {
        Some((packument, _meta)) => {
            Ok(Response::new(StatusCode::Ok).body(packument))
        },
        None => {
            Ok(Response::new(StatusCode::NotFound))
        }
    }
}

pub async fn put_packument<State>(_req: Request<State>) -> Result<&'static str> {
    Ok("put packument")
}

pub async fn get_tarball<State: ReadableStore>(req: Request<State>) -> Result<Response> {
    let package: String = req.param("pkg")?;
    let tarball: String = req.param("tarball")?;

    let version_plus_tgz = &(tarball.replace(&package, "")[1..]);
    let version = &version_plus_tgz[..version_plus_tgz.len() - 4];

    info!("get tarball {} {}", package, version);

    serve_tarball(req, &package, version).await
}

pub async fn get_scoped_tarball<State: ReadableStore>(req: Request<State>) -> Result<Response> {
    let scope: String = req.param("scope")?;
    let package: String = req.param("pkg")?;
    let tarball: String = req.param("tarball")?;

    let version_plus_tgz = &(tarball.replace(&package, "")[1..]);
    let version = &version_plus_tgz[..version_plus_tgz.len() - 4];

    let full_package = format!("{}/{}", scope, package);
    info!("get scoped tarball {}", full_package);
    serve_tarball(req, &full_package, version).await
}

async fn serve_tarball<State: ReadableStore>(req: Request<State>, package: &str, version: &str) -> Result<Response> {
    match req.state().get_tarball(package, version).await? {
        Some((response, _meta)) => {
            Ok(Response::new(StatusCode::Ok).body(response))
        },
        None => {
            Ok(Response::new(StatusCode::NotFound))
        }
    }
}

pub async fn post_login<State>(_req: Request<State>) -> Result<&'static str> {
    Ok("post login")
}

pub async fn get_login_poll<State>(_req: Request<State>) -> Result<&'static str> {
    Ok("get login poll")
}
