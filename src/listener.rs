use anyhow::Result;
use socket2::Socket;
use std::net::SocketAddr;

pub fn create_listener_socket(
    domain: socket2::Domain,
    ip_addr: SocketAddr,
) -> Result<Socket> {
    let socket2 = Socket::new(
        domain,
        socket2::Type::STREAM.nonblocking(),
        Some(socket2::Protocol::TCP)
    )?;

    socket2.set_reuse_port(true)?;
    socket2.set_nodelay(true)?;
    socket2.set_cloexec(false)?;
    socket2.bind(&ip_addr.into())?;
    socket2.listen(128)?;

    Ok(socket2)
}
