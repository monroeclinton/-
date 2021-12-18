use anyhow::{anyhow, Result};
use tokio::net::TcpStream;
use tokio::io::{self, AsyncWriteExt};
use tokio::sync::oneshot;
use thiserror::Error;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::os::unix::io::{AsRawFd, RawFd};

#[derive(Error, Debug)]
pub enum RouterError {
    #[error("Unable to connect to target server.")]
    Connection,
}

pub struct Router {
    apps: HashMap<IpAddr, App>,
}

impl Router {
    pub fn new() -> Self {
        Self {
            apps: HashMap::new(),
        }
    }

    pub fn add_app(&mut self, app: App) {
        self.apps.insert(app.ip_addr, app);
    }

    pub fn add_target(&mut self, ip_addr: IpAddr, target: AppTarget) {
        if let Some(app) = self.apps.get_mut(&ip_addr) {
            app.targets.push(target);
        }
    }

    pub fn set_weight(&mut self, app_ip_addr: IpAddr, target_ip_addr: IpAddr, weight: u8) {
        if let Some(app) = self.apps.get_mut(&app_ip_addr) {
            if let Some(target) = app.targets.iter_mut().find(|x| x.ip_addr == target_ip_addr) {
                target.weight = weight;
            }
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
        cancelled: oneshot::Receiver<()>,
        fds: Arc<Mutex<Vec<RawFd>>>
    ) -> Result<()> {
        let ip_addr = inbound.local_addr()?.ip();

        let app = match self.apps.get(&ip_addr) {
            Some(a) => a,
            None => return Err(anyhow!("Unable to find app with ip: {}", ip_addr)),
        };

        if app.targets.len() == 0 {
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
            let target_ip_addr = target.ip_addr.clone();

            let socket_addr = SocketAddr::new(target_ip_addr, local_port);

            match TcpStream::connect(socket_addr).await {
                Ok(outbound) => {
                    match proxy_stream(inbound, outbound, cancelled, fds).await {
                        Ok(_) => (), 
                        Err(e) => {
                            println!("Error proxying stream: {:?}", e);
                        }
                    }

                    return Ok(());
                },
                _ => (),
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
    pub fn new(ip_addr: IpAddr) -> Self {
        Self {
            ip_addr,
            weight: 0,
        }
    }
}

async fn proxy_stream(
    mut inbound: TcpStream,
    mut outbound: TcpStream,
    cancelled: oneshot::Receiver<()>,
    fds: Arc<Mutex<Vec<RawFd>>>
) -> Result<()> {
    let fd = inbound.as_raw_fd();
    fds.lock().unwrap().push(fd);

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


    let join = async {
        match tokio::try_join!(client_to_server, server_to_client) {
            Ok(_) => {
                return Ok(());
            }
            Err(e) => {
                return Err(anyhow!("Something went wrong proxying stream. Error: {:?}", e));
            }
        }
    };

    tokio::select! {
        _ = join => {
            fds.lock().unwrap().retain(|x| x != &fd);
            println!("Finished copying stream.");
        }
        _ = async { cancelled.await } => {
        }
    };
    
    Ok(())
}
