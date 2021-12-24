use anyhow::{anyhow, Result};
use tokio::net::TcpStream;
use tokio::io::{self, AsyncWriteExt};
use thiserror::Error;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};

use crate::config::Config;

#[derive(Error, Debug)]
pub enum RouterError {
    #[error("Unable to connect to target server.")]
    Connection,
}

pub struct Router {
    apps: HashMap<IpAddr, App>,
}

impl Router {
    pub fn new(config: Config) -> Self {
        let mut apps = HashMap::new();

        for app_config in config.apps {
            let app_ip_addr = match app_config.ip_addr.parse() {
                Ok(ip) => ip,
                Err(e) => {
                    panic!("Unable to parse app IP. Err: {}", e);
                }
            };

            let mut app = App::new(app_ip_addr);

            for target_config in app_config.targets {
                let target_ip_addr = match target_config.ip_addr.parse() {
                    Ok(ip) => ip,
                    Err(e) => {
                        panic!("Unable to parse target IP. Err: {}", e);
                    }
                };
                
                app.targets.push(
                    AppTarget::new(target_ip_addr, target_config.weight)
                );
            }

            apps.insert(app_ip_addr, App::new(app_ip_addr));
        }

        Self {
            apps,
        }
    }

    pub fn balancer(&self) -> Balancer {
        Balancer::new(self.apps.clone())
    }
}

pub struct Balancer {
    apps: HashMap<IpAddr, App>,
}

impl Balancer {
    pub fn new(apps: HashMap<IpAddr, App>) -> Self {
        Self {
            apps
        }
    }

    pub async fn handle_stream(
        &self,
        inbound: TcpStream,
    ) -> Result<()> {
        let ip_addr = inbound.local_addr()?.ip();

        let app = match self.apps.get(&ip_addr) {
            Some(a) => a,
            None => return Err(anyhow!("Unable to find app with ip: {}", ip_addr)),
        };

        if app.targets.is_empty() {
            return Err(anyhow!("No targets for app with ip: {}", app.ip_addr));
        }

        let local_port = match inbound.local_addr() {
            Ok(addr) => addr.port(),
            Err(_) => return Err(
                anyhow!("Unable to find port of incoming connection.")
            ),
        };

        let mut targets = app.targets.clone();
        targets.sort_by(|x, y| y.weight.cmp(&x.weight));
        for target in targets {
            let target_ip_addr = target.ip_addr;

            let socket_addr = SocketAddr::new(target_ip_addr, local_port);

            if let Ok(outbound) = TcpStream::connect(socket_addr).await {
                match proxy_stream(inbound, outbound).await {
                    Ok(_) => (), 
                    Err(e) => {
                        println!("Error proxying stream: {:?}", e);
                    }
                }

                return Ok(());
            };
        }

        Err(RouterError::Connection.into())
    }
}

#[derive(Clone)]
pub struct App {
    ip_addr: IpAddr,
    targets: Vec<AppTarget>,
}

impl App {
    pub fn new(ip_addr: IpAddr) -> Self {
        Self {
            ip_addr,
            targets: Vec::new(),
        }
    }
}

#[derive(Clone)]
pub struct AppTarget {
    ip_addr: IpAddr,
    weight: u8,
}

impl AppTarget {
    pub fn new(ip_addr: IpAddr, weight: u8) -> Self {
        Self {
            ip_addr,
            weight,
        }
    }
}

async fn proxy_stream(
    mut inbound: TcpStream,
    mut outbound: TcpStream,
) -> Result<()> {
    let (mut ro, mut wo) = outbound.split();
    let (mut ri, mut wi) = inbound.split();

    let client_to_server = async {
        match io::copy(&mut ri, &mut wo).await {
            Ok(_) => wo.shutdown().await,
            Err(e) => Err(e),
        }
    };

    let server_to_client = async {
        match io::copy(&mut ro, &mut wi).await {
            Ok(_) => wi.shutdown().await,
            Err(e) => Err(e),
        }
    };


    match tokio::try_join!(client_to_server, server_to_client) {
        Ok(_) => {
            Ok(())
        }
        Err(e) => {
            Err(anyhow!("Something went wrong proxying stream. Error: {:?}", e))
        }
    }
}
