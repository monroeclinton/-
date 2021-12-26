use anyhow::Result;
use tokio::net::TcpListener;
use std::net::SocketAddr;
use std::sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}};
use std::os::unix::io::{FromRawFd, AsRawFd, RawFd};

use crate::bpf::loader::load_socket_redirector;
use crate::config;
use crate::listener::create_listener_socket;
use crate::router::Router;
use crate::signals::{handle_terminate, handle_upgrades};

pub struct Server {
    connections: Arc<AtomicUsize>,
    draining: Arc<AtomicBool>,
    listener: TcpListener,
    is_child: bool,
    _redirector_link: libbpf_rs::Link,
}

impl Server {
    pub fn new(config: config::Config) -> Result<Self> {
        let listener_fd = std::env::var("LISTENER_FD");

        // Perhaps should run multiple listeners
        let socket = if let Ok(fd) = listener_fd.clone() {
            unsafe {
                socket2::Socket::from_raw_fd(fd.parse::<RawFd>().unwrap()) 
            }
        } else { 
            let ip_addr = SocketAddr::new(config.ip_addr.parse().unwrap(), 8080);

            let domain = match ip_addr {
                SocketAddr::V4(..) => socket2::Domain::IPV4,
                SocketAddr::V6(..) => socket2::Domain::IPV6,
            };

            create_listener_socket(domain, ip_addr).unwrap()
        };
            
        let listener = TcpListener::from_std(socket.into())?;

        let _redirector_link = load_socket_redirector(config, listener.as_raw_fd())?;

        Ok(Self {
            listener,
            draining: Arc::new(AtomicBool::new(false)),
            connections: Arc::new(AtomicUsize::new(0)),
            is_child: listener_fd.is_ok(),
            _redirector_link,
        })
    }

    pub async fn run(mut self, router: Router) -> Result<()> {
        self.handle_signals();

        self.handle_streams(router).await;

        Ok(())
    }

    pub async fn handle_streams(&mut self, router: Router) {
        while !self.draining.load(Ordering::Acquire) {
            if let Ok((stream, _)) = self.listener.accept().await {
                let connections = self.connections.clone();
                let balancer = router.balancer();

                tokio::spawn(async move {
                    connections.fetch_add(1, Ordering::Release);

                    match balancer.handle_stream(
                        stream,
                    ).await {
                        Ok(_) => (),
                        Err(e) => {
                            println!("{:?}", e);
                        }
                    };

                    connections.fetch_sub(1, Ordering::Release);
                });
            }
        }
    }

    pub fn handle_signals(&mut self) {
        tokio::spawn(
            handle_terminate(self.draining.clone())
        );

        tokio::spawn(
            handle_upgrades(
                self.is_child.clone(),
                self.listener.as_raw_fd()
            )
        );
    }
}
