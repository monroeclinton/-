use anyhow::{Result, Context};
use std::fs::OpenOptions;
use std::path::Path;
use std::net::Ipv4Addr;
use std::os::unix::io::{RawFd, IntoRawFd};

use crate::config::Config;

#[path = ".output/socket_redirector.skel.rs"]
mod socket_redirector;
use socket_redirector::*;

fn print_to_log(level: libbpf_rs::PrintLevel, msg: String) {
    match level {
        libbpf_rs::PrintLevel::Debug => println!("{}", msg),
        libbpf_rs::PrintLevel::Info => println!("{}", msg),
        libbpf_rs::PrintLevel::Warn => println!("{}", msg),
    }
}

pub fn load_socket_redirector(config: Config, server_socket: RawFd) -> Result<libbpf_rs::Link> {
    let map_path = Path::new("/sys/fs/bpf/");
    let mut skel_builder = SocketRedirectorSkelBuilder::default();

    if config.debug {
        libbpf_rs::set_print(Some((libbpf_rs::PrintLevel::Debug, print_to_log)));
        skel_builder.obj_builder.debug(true);
    }

    let mut open_skel = skel_builder.open()?;

    open_skel.maps_mut().ips().set_max_entries(config.apps.len() as u32)?;

    open_skel.progs_mut().redirector().set_prog_type(libbpf_rs::ProgramType::SkLookup);
    open_skel.progs_mut().redirector().set_attach_type(libbpf_rs::ProgramAttachType::SkLookup);

    let mut skel = open_skel.load()?;

    // TODO: Will maps hit RLIMIT_MEMLOCK?

    // Configure socket map
    let mut path_map_socket = map_path.join("socket_map");

    if path_map_socket.as_path().exists() {
        skel.maps_mut().sockets().unpin(&mut path_map_socket)?;
    }

    skel.maps_mut().sockets().pin(&mut path_map_socket)?;

    skel.maps_mut().sockets().update(
        &[0, 0, 0, 0], &(server_socket as u64).to_ne_bytes(), libbpf_rs::MapFlags::empty()
    )?;

    // Configure ips map
    let mut path_map_ips = map_path.join("ips_map");

    if path_map_ips.as_path().exists() {
        skel.maps_mut().ips().unpin(&mut path_map_ips)?;
    }

    skel.maps_mut().ips().pin(&mut path_map_ips)?;

    for app in config.apps {
        let ip_bytes = u32::from(app.ip_addr.parse::<Ipv4Addr>().unwrap()).to_ne_bytes();

        skel.maps_mut().ips().update(
            &ip_bytes, &[0], libbpf_rs::MapFlags::empty()
        )?;
    }

    // Configure redirector program
    let mut path_progs_redirect = map_path.join("redirector_prog");

    if path_progs_redirect.as_path().exists() {
        skel.progs_mut().redirector().unpin(&mut path_progs_redirect)?;
    }

    skel.progs_mut().redirector().pin(&mut path_progs_redirect)?;

    let netns = OpenOptions::new()
        .read(true)
        .open("/proc/self/ns/net")
        .context("Unable to open network namespace.")?;

    let link = skel.progs_mut().redirector().attach_netns(netns.into_raw_fd())?;

    Ok(link)
}
