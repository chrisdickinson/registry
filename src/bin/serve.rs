use std::net::TcpListener;

use listenfd::ListenFd;
use registry::handlers::v1;

fn setup_tracing() {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    let config = tracing_subscriber::registry().with(filter_layer);

    if atty::is(atty::Stream::Stdout) {
        config.with(fmt::layer().pretty()).init();
    } else {
        config.with(fmt::layer().json()).init();
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut listenfd = ListenFd::from_env();

    let bind = if let Some(listener) = listenfd.take_tcp_listener(0)? {
        listener
    } else {
        TcpListener::bind((
            std::env::var("HOST")
                .unwrap_or_else(|_| "127.0.0.1".to_string())
                .as_str(),
            std::env::var("POST")
                .ok()
                .and_then(|port| port.parse::<u16>().ok())
                .unwrap_or(8000),
        ))?
    };

    setup_tracing();
    let app = v1::routes();

    axum::Server::from_tcp(bind)?
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
