use anyhow::Result;

mod bpf;
mod config;
mod proxy;
mod router;
mod server;
mod target;

use tower::ServiceBuilder;

use crate::config::get_config;
use crate::proxy::ProxyLayer;
use crate::router::Router;
use crate::server::Server;

#[tokio::main]
async fn main() -> Result<()> {
    let config = get_config()?;

    let svc = ServiceBuilder::new()
        .layer(ProxyLayer)
        .service(Router::new(config.clone()));

    Server::from_config(config).serve(svc).await
}
