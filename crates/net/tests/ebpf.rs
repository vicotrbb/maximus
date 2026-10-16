#![cfg(all(target_os = "linux", feature = "ebpf"))]
use std::{
    io,
    os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd},
    sync::Arc,
};

#[repr(C)]
struct LoadProgram {
    kind: u32,
    count: u32,
    instructions: u64,
    license: u64,
    log_level: u32,
    log_size: u32,
    log_buffer: u64,
    kernel_version: u32,
    flags: u32,
}
fn program() -> io::Result<OwnedFd> {
    // BPF_MOV64_IMM r0, 0; BPF_EXIT. Reuseport socket-filter programs return a socket index.
    let instructions = [0x0000_0000_0000_00b7u64, 0x0000_0000_0000_0095u64];
    let license = b"GPL\0";
    let attr = LoadProgram {
        kind: 1,
        count: 2,
        instructions: instructions.as_ptr() as u64,
        license: license.as_ptr() as u64,
        log_level: 0,
        log_size: 0,
        log_buffer: 0,
        kernel_version: 0,
        flags: 0,
    };
    // SAFETY: All pointers in the repr(C) BPF_PROG_LOAD attributes remain valid for
    // this synchronous syscall. The kernel copies both instructions and license.
    let fd = unsafe { libc::syscall(libc::SYS_bpf, 5, &attr, std::mem::size_of::<LoadProgram>()) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: A successful BPF_PROG_LOAD returns a fresh owned file descriptor.
    Ok(unsafe { OwnedFd::from_raw_fd(fd as i32) })
}
#[test]
fn rejects_non_bpf_descriptor() {
    let listener =
        maximus_net::listen("127.0.0.1:0".parse().unwrap(), &Default::default()).unwrap();
    let file = std::fs::File::open("/dev/null").unwrap();
    assert!(maximus_net::attach_reuseport_ebpf(listener.as_fd(), file.as_fd()).is_err());
}
#[test]
#[ignore = "requires CAP_BPF or CAP_SYS_ADMIN; run scripts/test-linux-ebpf.sh"]
fn kernel_program_distributes_connections_to_selected_socket() {
    let program =
        Arc::new(program().expect("BPF_PROG_LOAD requires kernel BPF support and capability"));
    assert!(program.as_raw_fd() >= 0);
    let options = maximus_net::SocketOptions {
        reuseport_program: Some(program),
        ..Default::default()
    };
    let first = maximus_net::listen("127.0.0.1:0".parse().unwrap(), &options).unwrap();
    let address = first.local_addr().unwrap();
    let second = maximus_net::listen(address, &options).unwrap();
    first.set_nonblocking(true).unwrap();
    second.set_nonblocking(true).unwrap();
    let mut clients = Vec::new();
    for _ in 0..16 {
        clients.push(std::net::TcpStream::connect(address).unwrap());
    }
    for _ in 0..16 {
        first.accept().unwrap();
    }
    assert_eq!(
        second.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
}
