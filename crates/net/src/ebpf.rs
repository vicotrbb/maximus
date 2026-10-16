//! The only owned unsafe production boundary: Linux's socket option ABI.
use std::{
    io,
    os::fd::{AsRawFd, BorrowedFd},
};

/// Attach an already loaded reuseport eBPF program. The caller retains its descriptor.
/// The kernel validates the program type and takes its own reference on success.
pub fn attach_reuseport_ebpf(socket: BorrowedFd<'_>, program: BorrowedFd<'_>) -> io::Result<()> {
    let fd: libc::c_int = program.as_raw_fd();
    // SAFETY: Both borrowed descriptors remain open for the duration of this synchronous
    // call. optval points to an initialized c_int, and optlen exactly matches that type.
    // setsockopt copies the integer and neither retains nor writes to this Rust pointer.
    let result = unsafe {
        libc::setsockopt(
            socket.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_ATTACH_REUSEPORT_EBPF,
            (&fd as *const libc::c_int).cast(),
            std::mem::size_of_val(&fd) as libc::socklen_t,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
