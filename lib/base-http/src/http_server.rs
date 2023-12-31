/// this server is only intended for file downloads
/// e.g. downloading images, wasm modules etc.
pub struct HttpDownloadServer {
    rt: tokio::runtime::Runtime,
    join: Option<tokio::task::JoinHandle<()>>,
}

impl HttpDownloadServer {
    pub fn new() -> anyhow::Result<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .max_blocking_threads(1)
            .build()?;
        let _g = rt.enter();
        let join = tokio::task::spawn(async {
            // build our application with a single route
            let app =
                axum::Router::new().route("/", axum::routing::get(|| async { "Hello, World!" }));

            // run it with hyper on localhost:3000
            axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
                .serve(app.into_make_service())
                .await
                .unwrap();
        });
        Ok(Self {
            rt,
            join: Some(join),
        })
    }
}

impl Drop for HttpDownloadServer {
    fn drop(&mut self) {
        self.rt.block_on(self.join.take().unwrap()).unwrap();
    }
}
