//

pub fn routes() {
    let r = Router::new();

    r.route("/:pkg", get(get_packument).put(put_packument))
        .route("/:pkg/-/*tarball", get(get_tarball))
        .route("/:scope/:pkg/-/*tarball", get(get_scoped_tarball))
        .route("/-/v1/login", post(post_login))
        .route("/-/v1/login/poll/:session", get(get_login_poll));
}
