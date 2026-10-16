//! Socket creation before worker startup. Descriptors remain owned by Rust.
#![deny(unsafe_code)]
use socket2::{Domain, Protocol, Socket, Type};
use std::{
    io,
    net::{SocketAddr, TcpListener, UdpSocket},
};

#[derive(Clone, Debug)]
pub struct SocketOptions {
    #[cfg(all(target_os = "linux", feature = "ebpf"))]
    pub reuseport_program: Option<std::sync::Arc<std::os::fd::OwnedFd>>,
    pub backlog: i32,
    pub reuse_port: bool,
    pub receive_buffer: Option<usize>,
    pub send_buffer: Option<usize>,
}
impl Default for SocketOptions {
    fn default() -> Self {
        Self {
            #[cfg(all(target_os = "linux", feature = "ebpf"))]
            reuseport_program: None,
            backlog: 1024,
            reuse_port: true,
            receive_buffer: None,
            send_buffer: None,
        }
    }
}

fn socket(
    address: SocketAddr,
    kind: Type,
    protocol: Protocol,
    options: &SocketOptions,
) -> io::Result<Socket> {
    let socket = Socket::new(Domain::for_address(address), kind, Some(protocol))?;
    socket.set_reuse_address(true)?;
    socket.set_reuse_port(options.reuse_port)?;
    if address.is_ipv6() {
        socket.set_only_v6(true)?;
    }
    if let Some(size) = options.receive_buffer {
        socket.set_recv_buffer_size(size)?;
    }
    if let Some(size) = options.send_buffer {
        socket.set_send_buffer_size(size)?;
    }
    socket.bind(&address.into())?;
    Ok(socket)
}

pub fn listen(address: SocketAddr, options: &SocketOptions) -> io::Result<TcpListener> {
    if options.backlog <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "backlog must be positive",
        ));
    }
    let socket = socket(address, Type::STREAM, Protocol::TCP, options)?;
    socket.listen(options.backlog)?;
    #[cfg(all(target_os = "linux", feature = "ebpf"))]
    attach(&socket, options)?;
    Ok(socket.into())
}

pub fn bind_udp(address: SocketAddr, options: &SocketOptions) -> io::Result<UdpSocket> {
    let socket = socket(address, Type::DGRAM, Protocol::UDP, options)?;
    #[cfg(all(target_os = "linux", feature = "ebpf"))]
    attach(&socket, options)?;
    Ok(socket.into())
}

#[cfg(all(target_os = "linux", feature = "ebpf"))]
#[allow(unsafe_code)]
mod ebpf;
#[cfg(all(target_os = "linux", feature = "ebpf"))]
pub use ebpf::attach_reuseport_ebpf;
#[cfg(all(target_os = "linux", feature = "ebpf"))]
fn attach(socket: &Socket, options: &SocketOptions) -> io::Result<()> {
    use std::os::fd::AsFd;
    if let Some(program) = &options.reuseport_program {
        attach_reuseport_ebpf(socket.as_fd(), program.as_fd())?;
    }
    Ok(())
}
