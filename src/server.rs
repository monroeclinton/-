use anyhow::Result;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio_seqpacket::ancillary::{SocketAncillary};
use std::net::{SocketAddr};
use std::sync::{Arc, Mutex};
use std::os::unix::io::{FromRawFd, RawFd};
use std::io::IoSlice;
use std::fs;

use crate::router::Router;
use crate::config;
use crate::control::{ControlListener, ControlStream, SCM_MAX_FD, SEND_FS};

pub struct Server {
    listeners: Vec<TcpListener>,
    control_socket_path: String,
}

impl Server {
    pub async fn new(config: config::Config) -> Result<Self> {

        let mut listeners = Vec::new();

        for port in config.ports {
            // Will be changed when implement bpf portion
            for app in config.apps.clone() {
                let ip_addr = SocketAddr::new(app.ip_addr.parse().unwrap(), port);

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

                listeners.push(listener);
            }
        }

        let control_socket_path = config.control_socket_path;

        Ok(Self {
            listeners,
            control_socket_path,
        })
    }

    pub async fn run(self, router: Router) -> Result<()> {
        let mut futures = Vec::new();
        let router = Arc::new(router);
        let cancelled_fds = Arc::new(Mutex::new(Vec::<RawFd>::new()));
        let cancellers = Arc::new(Mutex::new(Vec::<oneshot::Sender<()>>::new()));
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        // TODO: Version in case of failure
        let control_socket_path = self.control_socket_path;
        let mut previous_fds: Option<Vec<RawFd>> = None;

        if fs::metadata(control_socket_path.as_str()).is_ok() { 
            if let Ok(cs) = ControlStream::connect(control_socket_path.as_str()).await {
                previous_fds = match cs.retrieve_fds().await {
                    Ok(fds) => Some(fds),
                    err @ _ => {
                        println!("Error trying to overtake previous control server: {:?}", err);
                        None
                    }
                };
            }
        }

        if let Some(fds) = previous_fds {
            for fd in fds {
                let router = router.clone();
                let cfds = cancelled_fds.clone();
                let cancellers = cancellers.clone();

                let future = tokio::spawn(async move {
                    let balancer = router.balancer();

                    let (canceller, cancelled) = oneshot::channel::<()>();
                    cancellers.lock().unwrap().push(canceller);

                    // Should probably do more saftey controls...
                    let std = unsafe { std::net::TcpStream::from_raw_fd(fd) };
                    
                    if let Ok(stream) = TcpStream::from_std(std) {
                        match balancer.handle_stream(
                            stream,
                            cancelled,
                            cfds.clone()
                        ).await {
                            Ok(_) => (),
                            Err(e) => {
                                dbg!(e);
                            }
                        };
                    }

                    cancellers.lock().unwrap().retain(|x| !x.is_closed());
                   
                });
                futures.push(future);
            }
        }

        let cfds = cancelled_fds.clone();
        let controller_cancellers = cancellers.clone();
        let future = tokio::spawn(async move {
            let mut listener = ControlListener::bind(control_socket_path.as_str()).unwrap();

            loop {
                if let Ok(stream) = listener.accept().await {
                    let mut buf = [0u8; 4];
                    if let Err(_) = stream.recv(&mut buf).await {
                        println!("Proper init signal not sent.");
                    }

                    for canceller in controller_cancellers.lock().unwrap().drain(..) {
                        if let Err(_) = canceller.send(()) {
                            dbg!("Error sending oneshot stream cancel");
                        }
                    }

                    let mut cmsg = [0; 64];
                	let mut cmsg = SocketAncillary::new(&mut cmsg);

                    for (i, fd) in cfds.lock().unwrap().drain(..).enumerate() {
                        if i < SCM_MAX_FD {
                	        cmsg.add_fds(&[fd]);
                        }
                    }

                    if let Err(_) = stream.send_vectored_with_ancillary(
                        &[IoSlice::new(SEND_FS)], &mut cmsg
                    ).await {
                        println!("There was a problem sending fds.");
                    }

                    let mut buf = [0u8; 8];
                    if let Err(_) = stream.recv(&mut buf).await {
                        println!("Proper shutdown signal not sent.");
                    }

                    if let Err(_) = shutdown_tx.send(()) {
                        println!("There was a problem stopping the server.");
                    }

                    break;
                }
            }
        });
        futures.push(future);

        for listener in self.listeners {
            let router = router.clone();
            let cfds = cancelled_fds.clone();
            let cancellers = cancellers.clone();

            let future = tokio::spawn(async move {
                loop {
                    if let Ok((stream, _)) = listener.accept().await {
                        let balancer = router.balancer();
                        let cfds = cfds.clone();
                        let cancellers = cancellers.clone();

                        tokio::spawn(async move {
                            let (canceller, cancelled) = oneshot::channel::<()>();
                            cancellers.lock().unwrap().push(canceller);

                            match balancer.handle_stream(
                                stream,
                                cancelled,
                                cfds
                            ).await {
                                Ok(_) => (),
                                Err(e) => {
                                    dbg!(e);
                                }
                            };

                            cancellers.lock().unwrap().retain(|x| !x.is_closed());
                        });
                    }
                }
            });

            futures.push(future);
        }

        let join = async {
            futures::future::join_all(futures).await;
        };

        tokio::select! {
            _ = join => {
            }
            _ = async { shutdown_rx.await } => {
                println!("Stopping proxy.");
            }
        }

        Ok(())
    }
}
