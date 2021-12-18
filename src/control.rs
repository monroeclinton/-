use anyhow::Result;
use thiserror::Error;
// unix_socket_ancillary_data currently in unstable
use tokio_seqpacket::{UnixSeqpacketListener, UnixSeqpacket};
use tokio_seqpacket::ancillary::{AncillaryData, SocketAncillary};
use serde::{Serialize, Deserialize};
use std::fs;
use std::os::unix::io::RawFd;
use std::io::IoSliceMut;
use std::path::PathBuf;

// https://man7.org/linux/man-pages/man7/unix.7.html (SCM_RIGHTS)
pub const SCM_MAX_FD: usize = 253;

pub const SEND_FS: &[u8; 7] = b"SEND_FS";
pub const STREAM_INIT: &[u8; 4] = b"INIT";
pub const STREAM_SHUTDOWN: &[u8; 8] = b"SHUTDOWN";

#[derive(Serialize, Deserialize)]
enum Message {
    Retrieve,
}

#[derive(Error, Debug)]
pub enum ControlSocketError {
    #[error("The previous control server send back invalid data.")]
    InvalidData,
    #[error("The previous control server did not send back valid file descriptors.")]
    InvalidFds,
}

// UDS that passes fds to new process
pub struct ControlListener {
    listener: UnixSeqpacketListener,
}

impl ControlListener {
    pub fn bind(path: &str) -> Result<Self> {
        let path_buf = PathBuf::from(path);
        if path_buf.as_path().exists() {
            fs::remove_file(path_buf.as_path())?;
        }

        let listener = UnixSeqpacketListener::bind(path)?;

        Ok(Self {
            listener,
        })
    }

    pub async fn accept(&mut self) -> Result<UnixSeqpacket> {
        Ok(self.listener.accept().await?)
    }
}

// Used for retrieving fds from previous process
pub struct ControlStream {
    stream: UnixSeqpacket,
}

impl ControlStream {
    pub async fn connect(path: &str) -> Result<Self> {
        let stream = UnixSeqpacket::connect(path).await?;

        Ok(Self {
            stream,
        })
    }

    pub async fn retrieve_fds(&self) -> Result<Vec<RawFd>> {
        self.stream.send(STREAM_INIT).await?;

        let mut cmsg = [0; std::mem::size_of::<RawFd>() * SCM_MAX_FD];
        let mut cmsg = SocketAncillary::new(&mut cmsg);
        let mut iov_buf = [0u8; 7];

        self.stream.recv_vectored_with_ancillary(
            &mut [IoSliceMut::new(&mut iov_buf)],
            &mut cmsg
        ).await?;

        if &iov_buf != SEND_FS {
            return Err(ControlSocketError::InvalidData.into());
        }

        match cmsg.messages().next() {
            Some(Ok(AncillaryData::ScmRights(fds))) => {
                self.stream.send(STREAM_SHUTDOWN).await?;
                return Ok(fds.collect());
            }
            Some(Err(e)) => {
                println!("Invalid fds: {:?}", e);
                return Err(ControlSocketError::InvalidFds.into())
            }
            _ => {
                println!("No fds sent.");
                return Ok(Vec::new())
            }
        }
    }
}
