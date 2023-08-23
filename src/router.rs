use anyhow::{anyhow, Result};
use std::{
    collections::HashMap,
    future::Future,
    net::{IpAddr, SocketAddr},
    pin::Pin,
    task::{Context, Poll},
};
use tokio::net::TcpStream;
use tower::{balance::p2c::Balance, Service, ServiceExt};

use crate::config::{AppTarget, Config};
use crate::target::Target;

#[derive(Clone)]
pub struct Router {
    apps: HashMap<IpAddr, Vec<AppTarget>>,
}

impl Router {
    pub fn new(config: Config) -> Self {
        let mut apps = HashMap::new();

        for app_config in config.apps {
            let ip = app_config.ip_addr;
            let targets = app_config.targets;
            apps.insert(ip, targets);
        }

        Self { apps }
    }
}

impl Service<SocketAddr> for Router {
    type Response = TcpStream;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, socket_addr: SocketAddr) -> Self::Future {
        let apps = self.apps.clone();

        let fut = async move {
            let ip = socket_addr.ip();
            let port = socket_addr.port();

            let targets = match apps.get(&ip) {
                Some(a) => a,
                None => return Err(anyhow!("Unable to find app with ip: {}", ip)),
            };

            let services = targets
                .into_iter()
                .map(|target| Target {
                    ip_addr: target.ip_addr,
                    weight: target.weight,
                })
                .collect::<Vec<Target>>();

            let mut p2c = Balance::new(tower::discover::ServiceList::new(services));

            // TODO: Fail after trying all services
            loop {
                if let Ok(services) = p2c.ready().await {
                    // There should be service discovery/health checks that takes bad nodes out
                    if let Ok(outbound) = services.call(port).await {
                        return Ok(outbound);
                    };
                }
            }
        };

        Box::pin(fut)
    }
}
