use std::{collections::HashMap, time::Duration};

use tokio::net::TcpSocket;

/// this server is only intended for file downloads
/// e.g. downloading images, wasm modules etc.
pub struct HttpDownloadServer {
    rt: Option<tokio::runtime::Runtime>,
    join: Option<tokio::task::JoinHandle<()>>,

    pub port: u16,
}

impl HttpDownloadServer {
    pub fn new(served_files: HashMap<String, Vec<u8>>) -> anyhow::Result<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .max_blocking_threads(1)
            .build()?;
        let _g = rt.enter();

        let tcp_socket = TcpSocket::new_v4()?;
        tcp_socket.set_reuseaddr(true)?;
        tcp_socket.bind(format!("0.0.0.0:0").parse()?)?;

        let addr = tcp_socket.local_addr()?;

        let join = tokio::task::spawn(async move {
            // build our application with a single route
            let mut app = axum::Router::new();

            for (name, served_file) in served_files {
                app = app.route(
                    &format!("/{}", name),
                    axum::routing::get(|| async move { served_file }),
                );
            }

            let listener = tcp_socket.listen(1024).unwrap();
            axum::serve(listener, app).await.unwrap();
        });
        Ok(Self {
            rt: Some(rt),
            join: Some(join),

            port: addr.port(),
        })
    }
}

impl Drop for HttpDownloadServer {
    fn drop(&mut self) {
        if let Some(rt) = self.rt.take() {
            let task = self.join.take().unwrap();
            task.abort();
            // TODO: cleaner shutdown than abort?
            let _ = rt.block_on(task);
            rt.shutdown_timeout(Duration::from_secs(1));
        }
    }
}
