//! Measured kernel primitives, separate from HTTP. Every byte is checked.
#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("kernel comparison requires Linux");
}
#[cfg(target_os = "linux")]
fn main() -> std::io::Result<()> {
    use compio::{
        BufResult,
        io::{AsyncRead, AsyncReadManaged, AsyncReadMulti, AsyncWriteExt, AsyncWriteZerocopy},
    };
    use futures_util::{StreamExt, pin_mut};
    use std::{
        io::{Read, Write},
        time::Instant,
    };
    println!("backend,mode,bytes,repeat,roundtrips,ns_per_roundtrip");
    for repeat in 0..3 {
        for backend in [maximus_io::Backend::IoUring, maximus_io::Backend::Epoll] {
            for mode in ["copy", "managed", "multishot", "zerocopy"] {
                if backend == maximus_io::Backend::Epoll && mode == "zerocopy" {
                    continue;
                }
                for size in [32usize, 8192, 65536] {
                    let iterations = 2000;
                    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
                    let address = listener.local_addr()?;
                    let echo = std::thread::spawn(move || -> std::io::Result<()> {
                        let (mut socket, _) = listener.accept()?;
                        socket.set_nodelay(true)?;
                        let mut buffer = vec![0; size];
                        for _ in 0..iterations {
                            socket.read_exact(&mut buffer)?;
                            assert!(buffer.iter().all(|b| *b == 7));
                            socket.write_all(&buffer)?;
                        }
                        Ok(())
                    });
                    let elapsed = maximus_io::runtime(backend, 256)?.block_on(async {
                        let mut socket = compio::net::TcpStream::connect(address).await?;
                        socket.set_nodelay(true)?;
                        let mut reader = socket.clone();
                        let multi = reader.read_multi(8192);
                        pin_mut!(multi);
                        let mut buffer = Vec::with_capacity(size);
                        let mut output = vec![7; size];
                        let started = Instant::now();
                        for _ in 0..iterations {
                            if mode == "zerocopy" {
                                // Preserve ownership until the kernel releases the pages. Handle partial writes.
                                let mut offset = 0;
                                while offset < output.len() {
                                    use compio::buf::{IntoInner, IoBuf};
                                    let BufResult(result, ready) =
                                        socket.write_zerocopy(output.slice(offset..)).await;
                                    offset += result?;
                                    output = ready.await.into_inner();
                                }
                            } else {
                                let BufResult(result, returned) = socket.write_all(output).await;
                                result?;
                                output = returned;
                            }
                            let mut received = 0;
                            while received < size {
                                if mode == "managed" || mode == "multishot" {
                                    let chunk = if mode == "managed" {
                                        socket.read_managed(8192).await?.expect("peer alive")
                                    } else {
                                        multi.next().await.expect("stream alive")?
                                    };
                                    assert!(!chunk.is_empty());
                                    assert!(chunk.iter().all(|b| *b == 7));
                                    received += chunk.len();
                                } else {
                                    buffer.clear();
                                    let BufResult(result, returned) = socket.read(buffer).await;
                                    buffer = returned;
                                    let count = result?;
                                    assert!(count > 0);
                                    assert!(buffer[..count].iter().all(|b| *b == 7));
                                    received += count;
                                }
                            }
                            assert_eq!(received, size);
                        }
                        Ok::<_, std::io::Error>(started.elapsed())
                    })?;
                    echo.join().unwrap()?;
                    println!(
                        "{backend:?},{mode},{size},{repeat},{iterations},{}",
                        elapsed.as_nanos() / iterations
                    );
                }
            }
        }
    }
    Ok(())
}
