//! I/O backend selection. Explicit selections never silently fall back.
#![forbid(unsafe_code)]
use compio::{
    driver::{DriverType, ProactorBuilder},
    runtime::Runtime,
};
use std::io;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Backend {
    #[default]
    Auto,
    IoUring,
    /// Linux epoll; the native poller on development platforms.
    Epoll,
}

pub fn runtime(backend: Backend, queue_depth: u32) -> io::Result<Runtime> {
    if queue_depth == 0 || !queue_depth.is_power_of_two() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "queue depth must be a nonzero power of two",
        ));
    }
    if backend == Backend::IoUring && !cfg!(target_os = "linux") {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "io_uring requires Linux",
        ));
    }
    let mut driver = ProactorBuilder::new();
    driver.capacity(queue_depth);
    match backend {
        Backend::Auto => {}
        Backend::IoUring => {
            driver.driver_type(DriverType::IoUring);
        }
        Backend::Epoll => {
            driver.driver_type(DriverType::Poll);
        }
    }
    let rt = Runtime::builder().with_proactor(driver).build()?;
    if backend == Backend::IoUring && rt.driver_type() != DriverType::IoUring {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "io_uring unavailable",
        ));
    }
    Ok(rt)
}
