mod bpf;
mod config;
mod listener;
mod router;
mod server;
mod signals;

use crate::config::get_config;
use crate::router::Router;
use crate::server::Server;

#[tokio::main]
async fn main() {
    let config = match get_config() {
        Ok(c) => c,
        Err(e) => panic!("Unable to read config file, check config.toml. Err: {:?}", e)
    };

    let router = Router::new(config.clone());

    match Server::new(config) {
        Ok(server) => server.run(router).await.unwrap(),
        Err(e) => {
            println!("Error starting server: {}", e);
        }
    }
}
