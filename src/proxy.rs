use anyhow::Result;
use std::{
    future::Future,
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::net::TcpStream;
use tower::{Layer, Service};

pub struct ProxyLayer;

impl<S> Layer<S> for ProxyLayer {
    type Service = Proxy<S>;

    fn layer(&self, service: S) -> Self::Service {
        Proxy { service }
    }
}

#[derive(Clone)]
pub struct Proxy<S> {
    service: S,
}

impl<S> Service<TcpStream> for Proxy<S>
where
    S: Service<SocketAddr, Response = TcpStream, Error = anyhow::Error> + Clone + Send + 'static,
    S::Future: Send,
{
    type Response = ();
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, mut inbound: TcpStream) -> Self::Future {
        let mut service = self.service.clone();

        let fut = async move {
            let socket_addr = inbound.local_addr()?;
            let mut outbound = service.call(socket_addr).await?;
            tokio::io::copy_bidirectional(&mut inbound, &mut outbound).await?;

            Ok(())
        };

        Box::pin(fut)
    }
}
