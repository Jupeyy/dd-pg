use std::collections::HashMap;

/// this server is only intended for file downloads
/// e.g. downloading images, wasm modules etc.
pub struct HttpDownloadServer {
    rt: tokio::runtime::Runtime,
    join: Option<tokio::task::JoinHandle<()>>,

    pub port: u16,
}

impl HttpDownloadServer {
    pub fn new(port: u16, served_files: HashMap<String, Vec<u8>>) -> anyhow::Result<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .max_blocking_threads(1)
            .build()?;
        let _g = rt.enter();
        let join = tokio::task::spawn(async move {
            // build our application with a single route
            let mut app = axum::Router::new();

            for (name, served_file) in served_files {
                app = app.route(
                    &format!("/{}", name),
                    axum::routing::get(|| async move { served_file }),
                );
            }

            // run it with hyper on localhost:3000
            let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
                .await
                .unwrap();
            axum::serve(listener, app).await.unwrap();
        });
        Ok(Self {
            rt,
            join: Some(join),

            port,
        })
    }
}

impl Drop for HttpDownloadServer {
    fn drop(&mut self) {
        let task = self.join.take().unwrap();
        task.abort();
        // TODO: cleaner shutdown than abort?
        let _ = self.rt.block_on(task);
    }
}
