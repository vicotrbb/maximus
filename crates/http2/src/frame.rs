//! HTTP/2 frame envelope validation. Stream state is checked by the connection.
pub(crate) const PROTOCOL: u32 = 1;
pub(crate) const FRAME_SIZE: u32 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Header {
    pub len: usize,
    pub kind: u8,
    pub flags: u8,
    pub stream: u32,
}
impl Header {
    pub fn decode(bytes: [u8; 9], maximum: usize) -> Result<Self, u32> {
        let h = Self {
            len: u32::from_be_bytes([0, bytes[0], bytes[1], bytes[2]]) as usize,
            kind: bytes[3],
            flags: bytes[4],
            stream: u32::from_be_bytes(bytes[5..].try_into().unwrap()) & 0x7fff_ffff,
        };
        if h.len > maximum {
            return Err(FRAME_SIZE);
        }
        let invalid_length = match h.kind {
            0 => h.flags & 8 != 0 && h.len == 0,
            1 => h.len < usize::from(h.flags & 8 != 0) + if h.flags & 0x20 != 0 { 5 } else { 0 },
            2 => h.len != 5,
            3 | 8 => h.len != 4,
            4 => !h.len.is_multiple_of(6) || h.flags & 1 != 0 && h.len != 0,
            6 => h.len != 8,
            7 => h.len < 8,
            _ => false,
        };
        if invalid_length {
            return Err(FRAME_SIZE);
        }
        if matches!(h.kind, 0..=3 | 9) && h.stream == 0
            || matches!(h.kind, 4 | 6 | 7) && h.stream != 0
            || h.kind == 5
        {
            return Err(PROTOCOL);
        }
        Ok(h)
    }
    pub fn encode(self) -> [u8; 9] {
        let length = (self.len as u32).to_be_bytes();
        let stream = self.stream.to_be_bytes();
        [
            length[1],
            length[2],
            length[3],
            self.kind,
            self.flags,
            stream[0] & 0x7f,
            stream[1],
            stream[2],
            stream[3],
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn decode(kind: u8, flags: u8, stream: u32, len: usize) -> Result<Header, u32> {
        Header::decode(
            Header {
                len,
                kind,
                flags,
                stream,
            }
            .encode(),
            16384,
        )
    }
    #[test]
    fn valid_envelopes_and_unknown_extensions() {
        for (kind, flags, stream, len) in [
            (0, 1, 1, 0),
            (1, 4, 1, 3),
            (2, 0, 1, 5),
            (3, 0, 1, 4),
            (4, 0, 0, 6),
            (4, 1, 0, 0),
            (6, 1, 0, 8),
            (7, 0, 0, 8),
            (8, 0, 0, 4),
            (8, 0, 1, 4),
            (9, 4, 1, 2),
            (255, 255, 0, 16384),
        ] {
            assert_eq!(
                decode(kind, flags, stream, len),
                Ok(Header {
                    len,
                    kind,
                    flags,
                    stream
                })
            );
        }
    }
    #[test]
    fn rejects_invalid_frame_lengths() {
        for (kind, flags, stream, len) in [
            (2, 0, 1, 4),
            (3, 0, 1, 5),
            (4, 0, 0, 5),
            (4, 1, 0, 6),
            (6, 0, 0, 7),
            (7, 0, 0, 7),
            (8, 0, 0, 3),
            (0, 0, 1, 16385),
            (1, 0x20, 1, 4),
            (1, 0x28, 1, 5),
            (0, 8, 1, 0),
            (1, 8, 1, 0),
        ] {
            assert_eq!(decode(kind, flags, stream, len), Err(FRAME_SIZE));
        }
    }
    #[test]
    fn rejects_wrong_stream_scope_and_client_push() {
        for (kind, stream, len) in [
            (0, 0, 0),
            (1, 0, 1),
            (2, 0, 5),
            (3, 0, 4),
            (4, 1, 0),
            (6, 1, 8),
            (7, 1, 8),
            (9, 0, 0),
            (5, 1, 4),
        ] {
            assert_eq!(decode(kind, 0, stream, len), Err(PROTOCOL));
        }
    }
    #[test]
    fn ignores_reserved_stream_bit() {
        let mut bytes = Header {
            len: 0,
            kind: 0,
            flags: 1,
            stream: 7,
        }
        .encode();
        bytes[5] |= 128;
        assert_eq!(Header::decode(bytes, 16384).unwrap().stream, 7);
    }
}
