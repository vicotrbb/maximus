use compio::{
    BufResult,
    buf::{IntoInner, IoBuf, IoBufMut},
    io::{AsyncRead, AsyncWrite, util::Splittable},
};
use maximus_core::{Handler, Limits, Request};
use std::{
    io,
    io::{Read, Write},
    rc::Rc,
    time::Duration,
};

/// Force the real owned-buffer boundary to complete one byte per operation.
struct OneByte<T>(T);
impl<T: AsyncRead> AsyncRead for OneByte<T> {
    async fn read<B: IoBufMut>(&mut self, buffer: B) -> BufResult<usize, B> {
        self.0
            .read(buffer.slice(..1))
            .await
            .map_buffer(IntoInner::into_inner)
    }
}
impl<T: AsyncWrite> AsyncWrite for OneByte<T> {
    async fn write<B: IoBuf>(&mut self, buffer: B) -> BufResult<usize, B> {
        let n = buffer.buf_len().min(1);
        self.0
            .write(buffer.slice(..n))
            .await
            .map_buffer(IntoInner::into_inner)
    }
    async fn flush(&mut self) -> io::Result<()> {
        self.0.flush().await
    }
    async fn shutdown(&mut self) -> io::Result<()> {
        self.0.shutdown().await
    }
}
impl<T: Splittable> Splittable for OneByte<T> {
    type ReadHalf = OneByte<T::ReadHalf>;
    type WriteHalf = OneByte<T::WriteHalf>;
    fn split(self) -> (Self::ReadHalf, Self::WriteHalf) {
        let (r, w) = self.0.split();
        (OneByte(r), OneByte(w))
    }
}
struct Echo;
impl Handler for Echo {
    async fn handle(&self, request: Request) -> maximus_core::Response {
        maximus_core::Response::new(request.into_body())
    }
}
#[test]
fn every_header_chunk_and_response_boundary_survives_one_byte_io() {
    compio::runtime::Runtime::new().unwrap().block_on(async {
        let listener=compio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address=listener.local_addr().unwrap();
        let client=std::thread::spawn(move || {
            let mut stream=std::net::TcpStream::connect(address).unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            stream.write_all(b"POST / HTTP/1.1\r\nHost: a\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n3\r\nabc\r\n0\r\nX-Check: yes\r\n\r\n").unwrap();
            let mut wire=String::new();stream.read_to_string(&mut wire).unwrap();wire
        });
        let (socket,_)=listener.accept().await.unwrap();
        maximus_http1::serve(OneByte(socket),Rc::new(Echo),Rc::new(Limits::default()),Default::default()).await.unwrap();
        let wire=client.join().unwrap();
        assert!(wire.starts_with("HTTP/1.1 200 OK"),"{wire}");
        assert!(wire.ends_with("1\r\na\r\n1\r\nb\r\n1\r\nc\r\n0\r\nx-check: yes\r\n\r\n"),"{wire}");
    });
}

#[test]
fn chunked_completion_reuses_output_without_stale_bytes() {
    compio::runtime::Runtime::new().unwrap().block_on(async {
        let listener = compio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let client = std::thread::spawn(move || {
            let mut stream = std::net::TcpStream::connect(address).unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            stream.write_all(b"POST / HTTP/1.1\r\nHost: a\r\nContent-Length: 1\r\n\r\naPOST / HTTP/1.1\r\nHost: a\r\nContent-Length: 1\r\nConnection: close\r\n\r\nb").unwrap();
            let mut wire = String::new();
            stream.read_to_string(&mut wire).unwrap();
            wire
        });
        let (socket, _) = listener.accept().await.unwrap();
        maximus_http1::serve(OneByte(socket), Rc::new(Echo), Rc::new(Limits::default()), Default::default()).await.unwrap();
        let wire = client.join().unwrap();
        assert_eq!(wire.matches("HTTP/1.1 200 OK").count(), 2, "{wire}");
        assert!(wire.contains("1\r\na\r\n0\r\n\r\nHTTP/1.1 200 OK"), "{wire}");
        assert!(wire.ends_with("1\r\nb\r\n0\r\n\r\n"), "{wire}");
    });
}
