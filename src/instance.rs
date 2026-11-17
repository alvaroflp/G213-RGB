//! One hardware controller per Linux user. A socket in the abstract namespace
//! is released by the kernel when the process ends, including after a crash.

use std::io::{self, Read, Write};
use std::os::{
    fd::AsRawFd,
    linux::net::SocketAddrExt,
    unix::net::{SocketAddr, UnixListener, UnixStream},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub struct Instance {
    listener: UnixListener,
    address: SocketAddr,
}

fn address() -> io::Result<SocketAddr> {
    // SAFETY: geteuid takes no pointers or other arguments.
    let uid = unsafe { libc::geteuid() };
    SocketAddr::from_abstract_name(format!("g213-rgb-{uid}-controller"))
}

impl Instance {
    pub fn claim() -> io::Result<Option<Self>> {
        Self::bind(address()?)
    }

    fn bind(address: SocketAddr) -> io::Result<Option<Self>> {
        match UnixListener::bind_addr(&address) {
            Ok(listener) => Ok(Some(Self { listener, address })),
            Err(error) if error.kind() == io::ErrorKind::AddrInUse => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub fn reopen() -> io::Result<()> {
        Self::reopen_at(&address()?)
    }

    fn reopen_at(address: &SocketAddr) -> io::Result<()> {
        let mut stream = UnixStream::connect_addr(address)?;
        if !same_user(&stream) {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        let mut response = [0];
        stream.read_exact(&mut response)?;
        if response == [1] {
            Ok(())
        } else {
            Err(io::ErrorKind::InvalidData.into())
        }
    }

    pub fn listen(self, ctx: egui::Context) -> io::Result<Activation> {
        let running = Arc::new(AtomicBool::new(true));
        let requested = Arc::new(AtomicBool::new(false));
        let alive = running.clone();
        let request = requested.clone();
        let thread = thread::Builder::new()
            .name("g213-activation".into())
            .spawn(move || {
                for stream in self.listener.incoming() {
                    if !alive.load(Ordering::Acquire) {
                        break;
                    }
                    match stream {
                        Ok(mut stream) if same_user(&stream) => {
                            request.store(true, Ordering::Release);
                            ctx.request_repaint();
                            let _ = stream.write_all(&[1]);
                        }
                        Err(error) if error.kind() != io::ErrorKind::Interrupted => break,
                        _ => {}
                    }
                }
            })?;
        Ok(Activation {
            address: self.address,
            running,
            requested,
            thread: Some(thread),
        })
    }
}

fn same_user(stream: &UnixStream) -> bool {
    let mut credentials = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut len = std::mem::size_of_val(&credentials) as libc::socklen_t;
    // SAFETY: credentials is writable for len bytes and stream owns the descriptor.
    unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut credentials as *mut libc::ucred).cast(),
            &mut len,
        ) == 0
            && len as usize == std::mem::size_of_val(&credentials)
            && credentials.uid == libc::geteuid()
    }
}

pub struct Activation {
    address: SocketAddr,
    running: Arc<AtomicBool>,
    requested: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Activation {
    pub fn take_request(&self) -> bool {
        self.requested.swap(false, Ordering::AcqRel)
    }
}

impl Drop for Activation {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
        // Wake accept so the socket is released before the application exits.
        let _ = UnixStream::connect_addr(&self.address);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_instance_receives_activation_and_releases_socket_on_exit() {
        let address = || {
            SocketAddr::from_abstract_name(format!("g213-activation-test-{}", std::process::id()))
                .unwrap()
        };
        let primary = Instance::bind(address()).unwrap().unwrap();
        assert!(Instance::bind(address()).unwrap().is_none());
        let activation = primary.listen(egui::Context::default()).unwrap();
        Instance::reopen_at(&address()).unwrap();
        assert!(activation.take_request());
        assert!(!activation.take_request());
        drop(activation);
        assert!(Instance::bind(address()).unwrap().is_some());
    }
}
