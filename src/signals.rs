use tokio::signal::unix::{signal, SignalKind};
use std::os::unix::io::RawFd;
use std::iter::once;
use std::process::Command;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

pub async fn handle_terminate(draining: Arc<AtomicBool>) {
    signal(SignalKind::terminate()).unwrap().recv().await;

    draining.store(true, Ordering::Release);
}

pub async fn handle_upgrades(is_child: bool, fd: RawFd) {
    let ppid = unsafe {
        libc::getppid()
    };

    if ppid > 1 && is_child {
        unsafe { libc::kill(ppid, libc::SIGTERM); };
    }

    signal(SignalKind::user_defined1()).unwrap().recv().await;

    let args = std::env::args()
        .collect::<Vec<String>>();

    let vars = std::env::vars()
        .filter(|(k, _)| k != "LISTENER_FD")
        .chain(once((String::from("LISTENER_FD"), fd.to_string())))
        .collect::<Vec<(String, String)>>();

    Command::new(args[0].clone())
        .args(args)
        .envs(vars)
        .spawn()
        .unwrap();
}
