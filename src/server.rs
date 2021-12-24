use anyhow::Result;
use tokio::net::TcpListener;
use std::net::SocketAddr;
use std::sync::Arc;
use std::os::unix::io::AsRawFd;

use crate::router::Router;
use crate::config;
use crate::bpf::loader::load_socket_redirector;


pub struct Server {
    listeners: Vec<TcpListener>,
    _redirector_link: libbpf_rs::Link,
}

impl Server {
    pub fn new(config: config::Config) -> Result<Self> {
        let mut listeners = Vec::new();

        // Perhaps should run multiple listeners
        let ip_addr = SocketAddr::new(config.ip_addr.parse().unwrap(), 8080);

        let domain = match ip_addr {
            SocketAddr::V4(..) => socket2::Domain::IPV4,
            SocketAddr::V6(..) => socket2::Domain::IPV6,
        };

        let socket = socket2::Socket::new(
            domain,
            socket2::Type::STREAM.nonblocking().cloexec(), 
            Some(socket2::Protocol::TCP)
        )?;

        socket.set_reuse_port(true)?;
        socket.set_nodelay(true)?;
        socket.bind(&ip_addr.into())?;
        socket.listen(128)?;

        let listener = TcpListener::from_std(socket.into())?;

        let _redirector_link = load_socket_redirector(config, listener.as_raw_fd())?;

        listeners.push(listener);

        Ok(Self {
            listeners,
            _redirector_link,
        })
    }

    pub async fn run(self, router: Router) -> Result<()> {
        let mut futures = Vec::new();
        let router = Arc::new(router);

        for listener in self.listeners {
            let router = router.clone();

            let future = tokio::spawn(async move {
                loop {
                    if let Ok((stream, _)) = listener.accept().await {
                        let balancer = router.balancer();

                        tokio::spawn(async move {
                            match balancer.handle_stream(
                                stream,
                            ).await {
                                Ok(_) => (),
                                Err(e) => {
                                    dbg!(e);
                                }
                            };
                        });
                    }
                }
            });

            futures.push(future);
        }

        futures::future::join_all(futures).await;

        Ok(())
    }
}
