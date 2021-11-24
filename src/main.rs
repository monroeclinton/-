mod router;
mod config;
mod control;
mod server;

use crate::router::{Router, App, AppTarget};
use crate::config::{get_config};
use crate::server::Server;

#[tokio::main]
async fn main() {
    let config = match get_config() {
        Ok(c) => c,
        Err(e) => panic!("Unable to read config file, check config.toml. Err: {:?}", e)
    };

    let mut router = Router::new();

    for app_config in config.apps.clone() {
        let app_ip_addr = match app_config.ip_addr.parse() {
            Ok(ip) => ip,
            Err(e) => {
                println!("Unable to parse app IP. Err: {}", e);
                continue;
            }
        };

        router.add_app(
            App::new(app_ip_addr)
        );

        for target_config in app_config.targets {
            let target_ip_addr = match target_config.ip_addr.parse() {
                Ok(ip) => ip,
                Err(e) => {
                    println!("Unable to parse target IP. Err: {}", e);
                    continue;
                }
            };

            router.add_target(
                app_ip_addr,
                AppTarget::new(target_ip_addr)
            );

            router.set_weight(
                app_ip_addr,
                target_ip_addr,
                target_config.weight
            );
        }

    }

    match Server::new(config).await {
        Ok(server) => server.run(router).await.unwrap(),
        Err(e) => {
            println!("Error starting server: {}", e);
        }
    }
}
