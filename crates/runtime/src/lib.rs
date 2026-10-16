//! Local executors, explicit CPU placement, and cooperative cancellation.
#![forbid(unsafe_code)]
pub use compio::runtime::{Runtime, spawn};
pub use compio::time::{sleep, timeout};
pub use maximus_io::Backend;
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Debug)]
pub struct WorkerConfig {
    pub workers: usize,
    pub cpus: Option<Vec<usize>>,
    pub backend: Backend,
    pub queue_depth: u32,
}
impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            workers: 1,
            cpus: None,
            backend: Backend::Auto,
            queue_depth: 1024,
        }
    }
}
impl WorkerConfig {
    pub fn validate(&self) -> io::Result<()> {
        let invalid = |message| io::Error::new(io::ErrorKind::InvalidInput, message);
        if self.workers == 0 {
            return Err(invalid("worker count must be positive"));
        }
        if self.queue_depth == 0 || !self.queue_depth.is_power_of_two() {
            return Err(invalid("queue depth must be a nonzero power of two"));
        }
        if let Some(cpus) = &self.cpus {
            if cpus.len() != self.workers {
                return Err(invalid("one CPU is required per worker"));
            }
            let allowed = allowed_cpus()?;
            for (index, cpu) in cpus.iter().enumerate() {
                if !allowed.contains(cpu) || cpus[..index].contains(cpu) {
                    return Err(invalid(
                        "CPUs must be distinct and permitted by process affinity",
                    ));
                }
            }
        }
        Ok(())
    }
    /// Call on the worker thread. Allocations made afterwards use local first-touch placement.
    pub fn build(&self, worker: usize) -> io::Result<Runtime> {
        self.validate()?;
        if worker >= self.workers {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "worker index out of range",
            ));
        }
        if let Some(cpus) = &self.cpus {
            pin_cpu(cpus[worker])?;
        }
        maximus_io::runtime(self.backend, self.queue_depth)
    }
}

/// CPUs allowed by the current thread's Linux affinity mask.
#[cfg(target_os = "linux")]
pub fn allowed_cpus() -> io::Result<Vec<usize>> {
    let cpus = rustix::thread::sched_getaffinity(None)?;
    Ok((0..rustix::thread::CpuSet::MAX_CPU)
        .filter(|cpu| cpus.is_set(*cpu))
        .collect())
}
#[cfg(not(target_os = "linux"))]
pub fn allowed_cpus() -> io::Result<Vec<usize>> {
    Ok((0..std::thread::available_parallelism()?.get()).collect())
}
#[cfg(target_os = "linux")]
fn pin_cpu(cpu: usize) -> io::Result<()> {
    let mut cpus = rustix::thread::CpuSet::new();
    cpus.set(cpu);
    rustix::thread::sched_setaffinity(None, &cpus)?;
    Ok(())
}
#[cfg(not(target_os = "linux"))]
fn pin_cpu(_: usize) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "CPU affinity requires Linux",
    ))
}

/// NUMA node owning a CPU, if Linux exposes its topology in sysfs.
pub fn numa_node(cpu: usize) -> io::Result<Option<usize>> {
    let path = format!("/sys/devices/system/cpu/cpu{cpu}");
    match std::fs::read_dir(path) {
        Ok(entries) => {
            for entry in entries {
                let name = entry?.file_name();
                if let Some(node) = name
                    .to_str()
                    .and_then(|s| s.strip_prefix("node"))
                    .and_then(|s| s.parse().ok())
                {
                    return Ok(Some(node));
                }
            }
            Ok(None)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

#[derive(Default)]
struct Signal {
    cancelled: AtomicBool,
    event: event_listener::Event,
}
/// Cloneable shutdown signal. Only lifecycle notification crosses worker boundaries.
#[derive(Clone, Default)]
pub struct Shutdown(Arc<Signal>);
impl Shutdown {
    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::Release);
        self.0.event.notify(usize::MAX);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::Acquire)
    }
    pub async fn cancelled(&self) {
        loop {
            let listener = self.0.event.listen();
            if self.is_cancelled() {
                return;
            }
            listener.await;
        }
    }
}
