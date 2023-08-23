use anyhow::{anyhow, Result};
use pin_project::pin_project;
use std::{
    future::Future,
    net::SocketAddr,
    os::unix::io::AsRawFd,
    pin::Pin,
    task::{ready, Context, Poll},
};
use tokio::net::{TcpListener, TcpStream};
use tower::Service;

use crate::bpf::loader::load_socket_redirector;
use crate::config::Config;

async fn handle_service<S>(mut service: S, stream: TcpStream) -> Result<()>
where
    S: Service<TcpStream>,
    S::Future: Future<Output = Result<()>>,
{
    println!(
        "Incoming request - ip: {} port: {}",
        stream.local_addr()?.ip(),
        stream.local_addr()?.port()
    );
    service.call(stream).await?;

    Ok(())
}

#[pin_project]
pub struct Server<S> {
    #[pin]
    listener: TcpListener,
    service: S,
}

impl Server<()> {
    pub fn from_config(config: Config) -> Builder {
        Builder { config }
    }
}

impl<S> Future for Server<S>
where
    S: Service<TcpStream> + Clone + Send + 'static,
    S::Future: Future<Output = Result<()>> + Send,
{
    type Output = Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        loop {
            if let Ok((stream, _)) = ready!(self.listener.poll_accept(cx)) {
                tokio::spawn(handle_service(self.service.clone(), stream));
            } else {
                return Poll::Ready(Err(anyhow!("Something went wrong")));
            };
        }
    }
}

pub struct Builder {
    config: Config,
}

impl Builder {
    pub fn serve<S>(self, service: S) -> Server<S> {
        let config = self.config;

        let addr = SocketAddr::new(config.ip_addr, config.port);

        let std_listener = std::net::TcpListener::bind(addr).unwrap();
        std_listener.set_nonblocking(true).unwrap();
        let listener = TcpListener::from_std(std_listener).unwrap();
        load_socket_redirector(config.clone(), listener.as_raw_fd()).unwrap();

        println!("Starting server");

        Server { listener, service }
    }
}
