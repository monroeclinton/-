use anyhow::Result;
use std::{
    future::Future,
    net::IpAddr,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::net::TcpStream;
use tower::{discover::Change, load::Load, Service};

use crate::config::AppTarget;

pub struct TargetDiscover {
    pub targets: Vec<AppTarget>,
}

impl futures::stream::Stream for TargetDiscover {
    type Item = Result<Change<IpAddr, Target>>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(target) = self.targets.pop() {
            Poll::Ready(Some(Ok(Change::Insert(
                target.ip_addr,
                Target {
                    ip_addr: target.ip_addr,
                    weight: target.weight,
                },
            ))))
        } else {
            Poll::Ready(None)
        }
    }
}

pub struct Target {
    pub ip_addr: IpAddr,
    pub weight: u8,
}

impl Service<u16> for Target {
    type Response = TcpStream;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, port: u16) -> Self::Future {
        let ip = self.ip_addr.clone();

        let fut = async move {
            let stream = TcpStream::connect((ip, port)).await?;
            Ok(stream)
        };

        Box::pin(fut)
    }
}

impl Load for Target {
    type Metric = u8;

    // TODO: Calculate load
    fn load(&self) -> Self::Metric {
        self.weight
    }
}
