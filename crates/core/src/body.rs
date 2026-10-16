use crate::{Error, StatusCode};
use bytes::Bytes;
use futures_util::Stream;
use http_body::{Body as HttpBody, Frame, SizeHint};
use std::{
    pin::Pin,
    task::{Context, Poll, ready},
};

type Frames = Pin<Box<dyn Stream<Item = Result<Frame<Bytes>, Error>>>>;
enum Kind {
    Empty,
    Full(Bytes),
    Incoming(hyper::body::Incoming),
    Stream(Frames),
}

/// Pull-based body. Fixed payloads do not box or allocate; streams are boxed once.
pub struct Body {
    kind: Kind,
    remaining: usize,
}
impl Body {
    pub fn is_buffered(&self) -> bool {
        matches!(self.kind, Kind::Empty | Kind::Full(_))
    }
    /// Take an already-buffered payload without collecting a stream or bypassing a byte limit.
    pub fn take_bytes(&mut self) -> Option<Bytes> {
        match &self.kind {
            Kind::Empty => Some(Bytes::new()),
            Kind::Full(bytes) if bytes.len() <= self.remaining => {
                match std::mem::replace(&mut self.kind, Kind::Empty) {
                    Kind::Full(bytes) => Some(bytes),
                    _ => unreachable!(),
                }
            }
            _ => None,
        }
    }
    pub fn empty() -> Self {
        Self {
            kind: Kind::Empty,
            remaining: usize::MAX,
        }
    }
    pub fn incoming(body: hyper::body::Incoming, limit: usize) -> Self {
        Self {
            kind: Kind::Incoming(body),
            remaining: limit,
        }
    }
    pub fn stream(stream: impl Stream<Item = Result<Bytes, Error>> + 'static) -> Self {
        use futures_util::StreamExt;
        Self::frames(stream.map(|chunk| chunk.map(Frame::data)))
    }
    pub fn frames(stream: impl Stream<Item = Result<Frame<Bytes>, Error>> + 'static) -> Self {
        Self {
            kind: Kind::Stream(Box::pin(stream)),
            remaining: usize::MAX,
        }
    }
    pub fn limited(mut self, limit: usize) -> Self {
        self.remaining = self.remaining.min(limit);
        self
    }
}
impl Default for Body {
    fn default() -> Self {
        Self::empty()
    }
}
impl From<Bytes> for Body {
    fn from(bytes: Bytes) -> Self {
        Self {
            kind: Kind::Full(bytes),
            remaining: usize::MAX,
        }
    }
}
impl From<&'static str> for Body {
    fn from(value: &'static str) -> Self {
        Bytes::from_static(value.as_bytes()).into()
    }
}
impl From<String> for Body {
    fn from(value: String) -> Self {
        Bytes::from(value).into()
    }
}
impl From<Vec<u8>> for Body {
    fn from(value: Vec<u8>) -> Self {
        Bytes::from(value).into()
    }
}
impl HttpBody for Body {
    type Data = Bytes;
    type Error = Error;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Error>>> {
        let frame = match &mut self.kind {
            Kind::Empty => return Poll::Ready(None),
            Kind::Full(bytes) => {
                let bytes = std::mem::take(bytes);
                self.kind = Kind::Empty;
                if bytes.is_empty() {
                    return Poll::Ready(None);
                }
                Ok(Frame::data(bytes))
            }
            Kind::Incoming(body) => match ready!(Pin::new(body).poll_frame(cx)) {
                Some(frame) => frame.map_err(|_| Error::bad_request("invalid request body")),
                None => {
                    self.kind = Kind::Empty;
                    return Poll::Ready(None);
                }
            },
            Kind::Stream(stream) => match ready!(stream.as_mut().poll_next(cx)) {
                Some(frame) => frame,
                None => {
                    self.kind = Kind::Empty;
                    return Poll::Ready(None);
                }
            },
        };
        if let Ok(frame) = &frame {
            let size = frame.data_ref().map_or(0, Bytes::len);
            if size > self.remaining {
                self.kind = Kind::Empty;
                return Poll::Ready(Some(Err(Error::new(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "body limit exceeded",
                ))));
            }
            self.remaining -= size;
        } else {
            self.kind = Kind::Empty;
        }
        Poll::Ready(Some(frame))
    }
    fn is_end_stream(&self) -> bool {
        match &self.kind {
            Kind::Empty => true,
            Kind::Full(b) => b.is_empty(),
            Kind::Incoming(b) => b.is_end_stream(),
            Kind::Stream(_) => false,
        }
    }
    fn size_hint(&self) -> SizeHint {
        match &self.kind {
            Kind::Empty => SizeHint::with_exact(0),
            Kind::Full(b) => SizeHint::with_exact(b.len() as u64),
            Kind::Incoming(b) => b.size_hint(),
            Kind::Stream(_) => SizeHint::default(),
        }
    }
}
