use futures_util::{
    FutureExt, StreamExt,
    future::{Either, FusedFuture, pending},
    stream::FuturesUnordered,
};
use maximus_core::{Handler, Limits};
use maximus_memory::Slab;
use maximus_net::SocketOptions;
use maximus_runtime::{Shutdown, WorkerConfig};
use std::{io, net::SocketAddr, rc::Rc, sync::mpsc, thread};

pub struct Server {
    address: SocketAddr,
    workers: WorkerConfig,
    limits: Limits,
    socket: SocketOptions,
    transport: crate::transport::Transport,
    #[cfg(feature = "http3")]
    http3: Option<maximus_tls::TlsConfig>,
}
impl Server {
    pub fn new(address: SocketAddr) -> Self {
        Self {
            address,
            workers: WorkerConfig::default(),
            limits: Limits::default(),
            socket: SocketOptions::default(),
            transport: crate::transport::Transport::default(),
            #[cfg(feature = "http3")]
            http3: None,
        }
    }
    #[cfg(feature = "tls")]
    pub fn tls(mut self, config: maximus_tls::TlsConfig) -> Self {
        self.transport.tls = Some(config);
        self
    }
    #[cfg(feature = "http2")]
    pub fn http2_prior_knowledge(mut self, enabled: bool) -> Self {
        self.transport.http2_prior_knowledge = enabled;
        self
    }
    #[cfg(feature = "http3")]
    pub fn http3(mut self, config: maximus_tls::TlsConfig) -> Self {
        self.http3 = Some(config);
        self
    }
    pub fn workers(mut self, workers: WorkerConfig) -> Self {
        self.workers = workers;
        self
    }
    pub fn limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }
    pub fn socket_options(mut self, socket: SocketOptions) -> Self {
        self.socket = socket;
        self
    }
    /// Bind all sockets and verify each worker starts before returning.
    /// The factory runs once per worker; its handlers and state need not be Send.
    pub fn start<F, H>(self, factory: F) -> io::Result<RunningServer>
    where
        F: Fn() -> H + Clone + Send + 'static,
        H: Handler,
    {
        #[cfg(feature = "http3")]
        let mut self_ = self;
        #[cfg(not(feature = "http3"))]
        let self_ = self;
        self_.workers.validate()?;
        // TCP and UDP have independent admission queues but share the worker budget.
        #[cfg(feature = "http3")]
        if self_.http3.is_some() {
            self_.limits.memory_bytes /= 2;
            if self_.limits.protocol_capacity(true, true) == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "memory budget cannot admit one HTTP/3 connection",
                ));
            }
        }
        self_.limits.validate()?;
        if self_.transport.capacity(&self_.limits) == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "memory budget cannot admit one TCP connection",
            ));
        }
        self_.transport.validate()?;
        let mut listeners = Vec::with_capacity(self_.workers.workers);
        let first = maximus_net::listen(self_.address, &self_.socket)?;
        let address = first.local_addr()?;
        listeners.push(first);
        for _ in 1..self_.workers.workers {
            listeners.push(maximus_net::listen(address, &self_.socket)?);
        }
        #[cfg(feature = "http3")]
        let mut udp_sockets = if self_.http3.is_some() {
            (0..self_.workers.workers)
                .map(|_| maximus_net::bind_udp(address, &self_.socket))
                .collect::<io::Result<Vec<_>>>()?
        } else {
            Vec::new()
        };
        let shutdown = Shutdown::default();
        let start = Shutdown::default();
        let (ready_tx, ready_rx) = mpsc::channel();
        let mut running = RunningServer {
            address,
            shutdown: shutdown.clone(),
            threads: Vec::new(),
        };
        for (worker, listener) in listeners.into_iter().enumerate() {
            #[cfg(feature = "http3")]
            let quic = udp_sockets.pop().zip(self_.http3.clone());
            let transport = self_.transport.clone();
            let config = self_.workers.clone();
            let limits = self_.limits.clone();
            let factory = factory.clone();
            let shutdown = shutdown.clone();
            let start = start.clone();
            let ready = ready_tx.clone();
            let result = thread::Builder::new()
                .name(format!("maximus-{worker}"))
                .spawn(move || {
                    let _exit = CancelOnExit(shutdown.clone());
                    let runtime = match config.build(worker) {
                        Ok(runtime) => runtime,
                        Err(error) => {
                            let _ = ready.send(Err(error.to_string()));
                            return Err(error);
                        }
                    };
                    runtime.block_on(async move {
                        let listener = match compio::net::TcpListener::from_std(listener) {
                            Ok(listener) => listener,
                            Err(error) => {
                                let _ = ready.send(Err(error.to_string()));
                                return Err(error);
                            }
                        };
                        #[cfg(feature = "http3")]
                        let endpoint = if let Some((socket, config)) = quic {
                            match maximus_http3::endpoint(socket, &config, &limits) {
                                Ok(endpoint) => Some(endpoint),
                                Err(error) => {
                                    let _ = ready.send(Err(error.to_string()));
                                    return Err(error);
                                }
                            }
                        } else {
                            None
                        };
                        let handler = Rc::new(factory());
                        let _ = ready.send(Ok(()));
                        drop(ready);
                        let launch = start.cancelled().fuse();
                        let stop = shutdown.cancelled().fuse();
                        futures_util::pin_mut!(launch, stop);
                        futures_util::select_biased! { _ = stop => return Ok(()), _ = launch => {} }
                        let limits = Rc::new(limits);
                        #[cfg(feature = "http3")]
                        if let Some(endpoint) = endpoint {
                            futures_util::try_join!(
                                worker_loop(
                                    listener,
                                    handler.clone(),
                                    limits.clone(),
                                    shutdown.clone(),
                                    transport
                                ),
                                maximus_http3::serve(endpoint, handler, limits, shutdown.clone())
                            )?;
                            return Ok(());
                        }
                        worker_loop(listener, handler, limits, shutdown.clone(), transport).await
                    })
                });
            match result {
                Ok(thread) => running.threads.push(thread),
                Err(error) => return Err(error),
            }
        }
        drop(ready_tx);
        for _ in 0..self_.workers.workers {
            match ready_rx.recv() {
                Ok(Ok(())) => {}
                Ok(Err(message)) => return Err(io::Error::other(message)),
                Err(_) => return Err(io::Error::other("worker panicked during startup")),
            }
        }
        start.cancel();
        Ok(running)
    }
}

async fn worker_loop<H: Handler>(
    listener: compio::net::TcpListener,
    handler: Rc<H>,
    limits: Rc<Limits>,
    shutdown: Shutdown,
    transport: crate::transport::Transport,
) -> io::Result<()> {
    let capacity = transport.capacity(&limits);
    let mut connections = Slab::new(capacity);
    let mut tasks = FuturesUnordered::new();
    // Keep submitted accepts alive when another connection completes. Cancelling an
    // io_uring accept can otherwise discard a socket the kernel already accepted.
    let mut accept = futures_util::future::Fuse::terminated();
    loop {
        if connections.len() < capacity && accept.is_terminated() {
            accept = listener.accept().boxed_local().fuse();
        }
        let completed = if tasks.is_empty() {
            Either::Left(pending())
        } else {
            Either::Right(tasks.next())
        }
        .fuse();
        let stop = shutdown.cancelled().fuse();
        futures_util::pin_mut!(completed, stop);
        futures_util::select_biased! {
            _ = stop => break,
            completed = completed => { if let Some(key) = completed { connections.remove(key); } },
            accepted = accept => {
                let (stream, _) = match accepted {
                    Ok(value) => value,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted || error.kind() == io::ErrorKind::ConnectionAborted => continue,
                    Err(error) => return Err(error),
                };
                stream.set_nodelay(true)?;
                let key = connections.insert(()).expect("admitted below connection capacity");
                let handler = handler.clone();
                let limits = limits.clone();
                let shutdown = shutdown.clone();
                let transport = transport.clone();
                tasks.push(async move {
                    // Protocol errors terminate only this connection. Malformed peers cannot stop a worker.
                    transport.serve(stream, handler, limits, shutdown).await;
                    key
                });
            },
        }
    }
    drop(accept);
    listener.close().await?;
    let _ = compio::time::timeout(limits.drain_timeout, async {
        while tasks.next().await.is_some() {}
    })
    .await;
    Ok(())
}

/// Owns worker lifetimes. Dropping cancels, drains to the configured deadline, and joins.
pub struct RunningServer {
    address: SocketAddr,
    shutdown: Shutdown,
    threads: Vec<thread::JoinHandle<io::Result<()>>>,
}
impl RunningServer {
    pub fn local_addr(&self) -> SocketAddr {
        self.address
    }
    pub fn shutdown_handle(&self) -> Shutdown {
        self.shutdown.clone()
    }
    pub fn shutdown(mut self) -> io::Result<()> {
        self.shutdown.cancel();
        self.join()
    }
    pub fn wait(mut self) -> io::Result<()> {
        self.join()
    }
    fn join(&mut self) -> io::Result<()> {
        let mut result = Ok(());
        for thread in self.threads.drain(..) {
            match thread.join() {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    self.shutdown.cancel();
                    result = Err(error);
                }
                Err(_) => {
                    self.shutdown.cancel();
                    result = Err(io::Error::other("worker panicked"));
                }
            }
        }
        result
    }
}
impl Drop for RunningServer {
    fn drop(&mut self) {
        self.shutdown.cancel();
        let _ = self.join();
    }
}

struct CancelOnExit(Shutdown);
impl Drop for CancelOnExit {
    fn drop(&mut self) {
        self.0.cancel();
    }
}
