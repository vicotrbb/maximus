//! Owned RFC 7541 HPACK codec. Header names and values remain opaque bytes.

use std::collections::VecDeque;
type Fields = Vec<(Vec<u8>, Vec<u8>)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    InvalidIndex,
    Truncated,
    IntegerOverflow,
    InvalidHuffman,
    InvalidTableSize,
    HeaderListTooLarge,
    TooManyHeaders,
}

/// One directional HPACK context. Discard the connection after any decode error.
pub struct Decoder {
    dynamic: VecDeque<(Vec<u8>, Vec<u8>)>,
    table_bytes: usize,
    table_limit: usize,
    max_table_size: usize,
    max_header_list_size: usize,
    max_headers: usize,
}

impl Decoder {
    pub fn new(max_table_size: usize, max_header_list_size: usize, max_headers: usize) -> Self {
        Self {
            dynamic: VecDeque::new(),
            table_bytes: 0,
            table_limit: max_table_size,
            max_table_size,
            max_header_list_size,
            max_headers,
        }
    }

    /// Decodes a complete header block, bounding the list by name + value + 32
    /// bytes per field. Field syntax is validated by the HTTP/2 layer.
    pub fn decode(&mut self, mut block: &[u8]) -> Result<Fields, Error> {
        let mut headers = Vec::new();
        let mut remaining = self.max_header_list_size;
        while let Some(&first) = block.first() {
            if first & 0xe0 == 0x20 {
                if !headers.is_empty() {
                    return Err(Error::InvalidTableSize);
                }
                let size = integer(&mut block, 5)?;
                if size > self.max_table_size {
                    return Err(Error::InvalidTableSize);
                }
                self.table_limit = size;
                self.evict_to(size);
                continue;
            }
            if headers.len() == self.max_headers {
                return Err(Error::TooManyHeaders);
            }
            let budget = remaining.checked_sub(32).ok_or(Error::HeaderListTooLarge)?;
            let indexed = first & 0x80 != 0;
            let incremental = first & 0xc0 == 0x40;
            let prefix = if indexed {
                7
            } else if incremental {
                6
            } else {
                4
            };
            let index = integer(&mut block, prefix)?;
            let (name, value) = if indexed {
                let (name, value) = self.lookup(index)?;
                if name.len() > budget || value.len() > budget - name.len() {
                    return Err(Error::HeaderListTooLarge);
                }
                (name.to_vec(), value.to_vec())
            } else {
                let name = if index == 0 {
                    string(&mut block, budget)?
                } else {
                    let (name, _) = self.lookup(index)?;
                    if name.len() > budget {
                        return Err(Error::HeaderListTooLarge);
                    }
                    name.to_vec()
                };
                let value = string(&mut block, budget - name.len())?;
                (name, value)
            };
            let size = name.len() + value.len() + 32;
            remaining -= size;
            if incremental {
                if size > self.table_limit {
                    self.evict_to(0);
                } else {
                    self.evict_to(self.table_limit - size);
                    self.table_bytes += size;
                    self.dynamic.push_front((name.clone(), value.clone()));
                }
            }
            headers.push((name, value));
        }
        Ok(headers)
    }

    fn lookup(&self, index: usize) -> Result<(&[u8], &[u8]), Error> {
        let zero = index.checked_sub(1).ok_or(Error::InvalidIndex)?;
        if let Some(&(name, value)) = STATIC.get(zero) {
            return Ok((name, value));
        }
        self.dynamic
            .get(zero - STATIC.len())
            .map(|(name, value)| (name.as_slice(), value.as_slice()))
            .ok_or(Error::InvalidIndex)
    }

    fn evict_to(&mut self, size: usize) {
        while self.table_bytes > size {
            if let Some((name, value)) = self.dynamic.pop_back() {
                self.table_bytes -= name.len() + value.len() + 32;
            }
        }
    }
}

fn integer(block: &mut &[u8], prefix: u32) -> Result<usize, Error> {
    let (&first, rest) = block.split_first().ok_or(Error::Truncated)?;
    *block = rest;
    let mask = (1usize << prefix) - 1;
    let mut value = usize::from(first) & mask;
    if value < mask {
        return Ok(value);
    }
    let mut shift = 0;
    loop {
        let (&byte, rest) = block.split_first().ok_or(Error::Truncated)?;
        *block = rest;
        let part = usize::from(byte & 0x7f);
        if shift >= usize::BITS || part > (usize::MAX >> shift) {
            return Err(Error::IntegerOverflow);
        }
        value = value
            .checked_add(part << shift)
            .ok_or(Error::IntegerOverflow)?;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
        shift += 7;
    }
}

fn string(block: &mut &[u8], limit: usize) -> Result<Vec<u8>, Error> {
    let huffman = block.first().ok_or(Error::Truncated)? & 0x80 != 0;
    let len = integer(block, 7)?;
    let bytes = block.get(..len).ok_or(Error::Truncated)?;
    *block = &block[len..];
    if huffman {
        return decode_huffman(bytes, limit);
    }
    if len > limit {
        return Err(Error::HeaderListTooLarge);
    }
    Ok(bytes.to_vec())
}

/// Appends a deterministic block using static indices and never-indexed literals.
/// No dynamic encoder state is needed, regardless of the peer's table limit.
pub fn encode<'a>(headers: impl IntoIterator<Item = (&'a [u8], &'a [u8])>, output: &mut Vec<u8>) {
    // Explicitly disable the dynamic table, including after a peer lowers its
    // SETTINGS_HEADER_TABLE_SIZE. A size update may precede every block.
    output.push(0x20);
    for (name, value) in headers {
        if let Some(index) = STATIC.iter().position(|&(n, v)| n == name && v == value) {
            write_integer(index + 1, 7, 0x80, output);
            continue;
        }
        let index = STATIC
            .iter()
            .position(|&(n, _)| n == name)
            .map_or(0, |i| i + 1);
        write_integer(index, 4, 0x10, output);
        if index == 0 {
            write_integer(name.len(), 7, 0, output);
            output.extend_from_slice(name);
        }
        write_integer(value.len(), 7, 0, output);
        output.extend_from_slice(value);
    }
}

fn write_integer(mut value: usize, prefix: u32, flags: u8, output: &mut Vec<u8>) {
    let mask = (1usize << prefix) - 1;
    if value < mask {
        output.push(flags | value as u8);
        return;
    }
    output.push(flags | mask as u8);
    value -= mask;
    while value >= 128 {
        output.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    output.push(value as u8);
}

#[derive(Default)]
struct Node {
    children: [usize; 2],
    symbol: Option<u16>,
}

fn decode_huffman(bytes: &[u8], limit: usize) -> Result<Vec<u8>, Error> {
    // The immutable trie is shared across all connections and built only once.
    static TREE: std::sync::OnceLock<Vec<Node>> = std::sync::OnceLock::new();
    let tree = TREE.get_or_init(|| {
        let mut tree = vec![Node::default()];
        for (symbol, &(code, width)) in HUFFMAN.iter().enumerate() {
            let mut node = 0;
            for shift in (0..width).rev() {
                let bit = ((code >> shift) & 1) as usize;
                if tree[node].children[bit] == 0 {
                    tree[node].children[bit] = tree.len();
                    tree.push(Node::default());
                }
                node = tree[node].children[bit];
            }
            tree[node].symbol = Some(symbol as u16);
        }
        tree
    });
    let mut output = Vec::new();
    let mut node = 0;
    let mut tail_len = 0;
    let mut tail_ones = true;
    for &byte in bytes {
        for shift in (0..8).rev() {
            let bit = ((byte >> shift) & 1) as usize;
            node = tree[node].children[bit];
            if node == 0 {
                return Err(Error::InvalidHuffman);
            }
            tail_len += 1;
            tail_ones &= bit == 1;
            if let Some(symbol) = tree[node].symbol {
                if symbol == 256 {
                    return Err(Error::InvalidHuffman);
                }
                if output.len() == limit {
                    return Err(Error::HeaderListTooLarge);
                }
                output.push(symbol as u8);
                node = 0;
                tail_len = 0;
                tail_ones = true;
            }
        }
    }
    // Padding must be at most seven bits and a prefix of the all-ones EOS.
    if tail_len > 7 || !tail_ones {
        return Err(Error::InvalidHuffman);
    }
    Ok(output)
}

// Tables below are transcribed from RFC 7541 Appendices A and B.
// Copyright (c) 2015 IETF Trust and the persons identified as the document
// authors (R. Peon and H. Ruellan). All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
// 1. Redistributions of source code must retain the above copyright notice,
//    this list of conditions and the following disclaimer.
// 2. Redistributions in binary form must reproduce the above copyright notice,
//    this list of conditions and the following disclaimer in the documentation
//    and/or other materials provided with the distribution.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR CONTRIBUTORS BE
// LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
// CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
// SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
// CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
// POSSIBILITY OF SUCH DAMAGE.

// RFC 7541 Appendix A: https://www.rfc-editor.org/rfc/rfc7541#appendix-A
const STATIC: &[(&[u8], &[u8])] = &[
    (b":authority", b""),                   // 1
    (b":method", b"GET"),                   // 2
    (b":method", b"POST"),                  // 3
    (b":path", b"/"),                       // 4
    (b":path", b"/index.html"),             // 5
    (b":scheme", b"http"),                  // 6
    (b":scheme", b"https"),                 // 7
    (b":status", b"200"),                   // 8
    (b":status", b"204"),                   // 9
    (b":status", b"206"),                   // 10
    (b":status", b"304"),                   // 11
    (b":status", b"400"),                   // 12
    (b":status", b"404"),                   // 13
    (b":status", b"500"),                   // 14
    (b"accept-charset", b""),               // 15
    (b"accept-encoding", b"gzip, deflate"), // 16
    (b"accept-language", b""),              // 17
    (b"accept-ranges", b""),                // 18
    (b"accept", b""),                       // 19
    (b"access-control-allow-origin", b""),  // 20
    (b"age", b""),                          // 21
    (b"allow", b""),                        // 22
    (b"authorization", b""),                // 23
    (b"cache-control", b""),                // 24
    (b"content-disposition", b""),          // 25
    (b"content-encoding", b""),             // 26
    (b"content-language", b""),             // 27
    (b"content-length", b""),               // 28
    (b"content-location", b""),             // 29
    (b"content-range", b""),                // 30
    (b"content-type", b""),                 // 31
    (b"cookie", b""),                       // 32
    (b"date", b""),                         // 33
    (b"etag", b""),                         // 34
    (b"expect", b""),                       // 35
    (b"expires", b""),                      // 36
    (b"from", b""),                         // 37
    (b"host", b""),                         // 38
    (b"if-match", b""),                     // 39
    (b"if-modified-since", b""),            // 40
    (b"if-none-match", b""),                // 41
    (b"if-range", b""),                     // 42
    (b"if-unmodified-since", b""),          // 43
    (b"last-modified", b""),                // 44
    (b"link", b""),                         // 45
    (b"location", b""),                     // 46
    (b"max-forwards", b""),                 // 47
    (b"proxy-authenticate", b""),           // 48
    (b"proxy-authorization", b""),          // 49
    (b"range", b""),                        // 50
    (b"referer", b""),                      // 51
    (b"refresh", b""),                      // 52
    (b"retry-after", b""),                  // 53
    (b"server", b""),                       // 54
    (b"set-cookie", b""),                   // 55
    (b"strict-transport-security", b""),    // 56
    (b"transfer-encoding", b""),            // 57
    (b"user-agent", b""),                   // 58
    (b"vary", b""),                         // 59
    (b"via", b""),                          // 60
    (b"www-authenticate", b""),             // 61
];

// https://www.rfc-editor.org/rfc/rfc7541#appendix-B
const HUFFMAN: &[(u32, u8)] = &[
    (0x1ff8, 13),     // 0
    (0x7fffd8, 23),   // 1
    (0xfffffe2, 28),  // 2
    (0xfffffe3, 28),  // 3
    (0xfffffe4, 28),  // 4
    (0xfffffe5, 28),  // 5
    (0xfffffe6, 28),  // 6
    (0xfffffe7, 28),  // 7
    (0xfffffe8, 28),  // 8
    (0xffffea, 24),   // 9
    (0x3ffffffc, 30), // 10
    (0xfffffe9, 28),  // 11
    (0xfffffea, 28),  // 12
    (0x3ffffffd, 30), // 13
    (0xfffffeb, 28),  // 14
    (0xfffffec, 28),  // 15
    (0xfffffed, 28),  // 16
    (0xfffffee, 28),  // 17
    (0xfffffef, 28),  // 18
    (0xffffff0, 28),  // 19
    (0xffffff1, 28),  // 20
    (0xffffff2, 28),  // 21
    (0x3ffffffe, 30), // 22
    (0xffffff3, 28),  // 23
    (0xffffff4, 28),  // 24
    (0xffffff5, 28),  // 25
    (0xffffff6, 28),  // 26
    (0xffffff7, 28),  // 27
    (0xffffff8, 28),  // 28
    (0xffffff9, 28),  // 29
    (0xffffffa, 28),  // 30
    (0xffffffb, 28),  // 31
    (0x14, 6),        // 32
    (0x3f8, 10),      // 33
    (0x3f9, 10),      // 34
    (0xffa, 12),      // 35
    (0x1ff9, 13),     // 36
    (0x15, 6),        // 37
    (0xf8, 8),        // 38
    (0x7fa, 11),      // 39
    (0x3fa, 10),      // 40
    (0x3fb, 10),      // 41
    (0xf9, 8),        // 42
    (0x7fb, 11),      // 43
    (0xfa, 8),        // 44
    (0x16, 6),        // 45
    (0x17, 6),        // 46
    (0x18, 6),        // 47
    (0x0, 5),         // 48
    (0x1, 5),         // 49
    (0x2, 5),         // 50
    (0x19, 6),        // 51
    (0x1a, 6),        // 52
    (0x1b, 6),        // 53
    (0x1c, 6),        // 54
    (0x1d, 6),        // 55
    (0x1e, 6),        // 56
    (0x1f, 6),        // 57
    (0x5c, 7),        // 58
    (0xfb, 8),        // 59
    (0x7ffc, 15),     // 60
    (0x20, 6),        // 61
    (0xffb, 12),      // 62
    (0x3fc, 10),      // 63
    (0x1ffa, 13),     // 64
    (0x21, 6),        // 65
    (0x5d, 7),        // 66
    (0x5e, 7),        // 67
    (0x5f, 7),        // 68
    (0x60, 7),        // 69
    (0x61, 7),        // 70
    (0x62, 7),        // 71
    (0x63, 7),        // 72
    (0x64, 7),        // 73
    (0x65, 7),        // 74
    (0x66, 7),        // 75
    (0x67, 7),        // 76
    (0x68, 7),        // 77
    (0x69, 7),        // 78
    (0x6a, 7),        // 79
    (0x6b, 7),        // 80
    (0x6c, 7),        // 81
    (0x6d, 7),        // 82
    (0x6e, 7),        // 83
    (0x6f, 7),        // 84
    (0x70, 7),        // 85
    (0x71, 7),        // 86
    (0x72, 7),        // 87
    (0xfc, 8),        // 88
    (0x73, 7),        // 89
    (0xfd, 8),        // 90
    (0x1ffb, 13),     // 91
    (0x7fff0, 19),    // 92
    (0x1ffc, 13),     // 93
    (0x3ffc, 14),     // 94
    (0x22, 6),        // 95
    (0x7ffd, 15),     // 96
    (0x3, 5),         // 97
    (0x23, 6),        // 98
    (0x4, 5),         // 99
    (0x24, 6),        // 100
    (0x5, 5),         // 101
    (0x25, 6),        // 102
    (0x26, 6),        // 103
    (0x27, 6),        // 104
    (0x6, 5),         // 105
    (0x74, 7),        // 106
    (0x75, 7),        // 107
    (0x28, 6),        // 108
    (0x29, 6),        // 109
    (0x2a, 6),        // 110
    (0x7, 5),         // 111
    (0x2b, 6),        // 112
    (0x76, 7),        // 113
    (0x2c, 6),        // 114
    (0x8, 5),         // 115
    (0x9, 5),         // 116
    (0x2d, 6),        // 117
    (0x77, 7),        // 118
    (0x78, 7),        // 119
    (0x79, 7),        // 120
    (0x7a, 7),        // 121
    (0x7b, 7),        // 122
    (0x7ffe, 15),     // 123
    (0x7fc, 11),      // 124
    (0x3ffd, 14),     // 125
    (0x1ffd, 13),     // 126
    (0xffffffc, 28),  // 127
    (0xfffe6, 20),    // 128
    (0x3fffd2, 22),   // 129
    (0xfffe7, 20),    // 130
    (0xfffe8, 20),    // 131
    (0x3fffd3, 22),   // 132
    (0x3fffd4, 22),   // 133
    (0x3fffd5, 22),   // 134
    (0x7fffd9, 23),   // 135
    (0x3fffd6, 22),   // 136
    (0x7fffda, 23),   // 137
    (0x7fffdb, 23),   // 138
    (0x7fffdc, 23),   // 139
    (0x7fffdd, 23),   // 140
    (0x7fffde, 23),   // 141
    (0xffffeb, 24),   // 142
    (0x7fffdf, 23),   // 143
    (0xffffec, 24),   // 144
    (0xffffed, 24),   // 145
    (0x3fffd7, 22),   // 146
    (0x7fffe0, 23),   // 147
    (0xffffee, 24),   // 148
    (0x7fffe1, 23),   // 149
    (0x7fffe2, 23),   // 150
    (0x7fffe3, 23),   // 151
    (0x7fffe4, 23),   // 152
    (0x1fffdc, 21),   // 153
    (0x3fffd8, 22),   // 154
    (0x7fffe5, 23),   // 155
    (0x3fffd9, 22),   // 156
    (0x7fffe6, 23),   // 157
    (0x7fffe7, 23),   // 158
    (0xffffef, 24),   // 159
    (0x3fffda, 22),   // 160
    (0x1fffdd, 21),   // 161
    (0xfffe9, 20),    // 162
    (0x3fffdb, 22),   // 163
    (0x3fffdc, 22),   // 164
    (0x7fffe8, 23),   // 165
    (0x7fffe9, 23),   // 166
    (0x1fffde, 21),   // 167
    (0x7fffea, 23),   // 168
    (0x3fffdd, 22),   // 169
    (0x3fffde, 22),   // 170
    (0xfffff0, 24),   // 171
    (0x1fffdf, 21),   // 172
    (0x3fffdf, 22),   // 173
    (0x7fffeb, 23),   // 174
    (0x7fffec, 23),   // 175
    (0x1fffe0, 21),   // 176
    (0x1fffe1, 21),   // 177
    (0x3fffe0, 22),   // 178
    (0x1fffe2, 21),   // 179
    (0x7fffed, 23),   // 180
    (0x3fffe1, 22),   // 181
    (0x7fffee, 23),   // 182
    (0x7fffef, 23),   // 183
    (0xfffea, 20),    // 184
    (0x3fffe2, 22),   // 185
    (0x3fffe3, 22),   // 186
    (0x3fffe4, 22),   // 187
    (0x7ffff0, 23),   // 188
    (0x3fffe5, 22),   // 189
    (0x3fffe6, 22),   // 190
    (0x7ffff1, 23),   // 191
    (0x3ffffe0, 26),  // 192
    (0x3ffffe1, 26),  // 193
    (0xfffeb, 20),    // 194
    (0x7fff1, 19),    // 195
    (0x3fffe7, 22),   // 196
    (0x7ffff2, 23),   // 197
    (0x3fffe8, 22),   // 198
    (0x1ffffec, 25),  // 199
    (0x3ffffe2, 26),  // 200
    (0x3ffffe3, 26),  // 201
    (0x3ffffe4, 26),  // 202
    (0x7ffffde, 27),  // 203
    (0x7ffffdf, 27),  // 204
    (0x3ffffe5, 26),  // 205
    (0xfffff1, 24),   // 206
    (0x1ffffed, 25),  // 207
    (0x7fff2, 19),    // 208
    (0x1fffe3, 21),   // 209
    (0x3ffffe6, 26),  // 210
    (0x7ffffe0, 27),  // 211
    (0x7ffffe1, 27),  // 212
    (0x3ffffe7, 26),  // 213
    (0x7ffffe2, 27),  // 214
    (0xfffff2, 24),   // 215
    (0x1fffe4, 21),   // 216
    (0x1fffe5, 21),   // 217
    (0x3ffffe8, 26),  // 218
    (0x3ffffe9, 26),  // 219
    (0xffffffd, 28),  // 220
    (0x7ffffe3, 27),  // 221
    (0x7ffffe4, 27),  // 222
    (0x7ffffe5, 27),  // 223
    (0xfffec, 20),    // 224
    (0xfffff3, 24),   // 225
    (0xfffed, 20),    // 226
    (0x1fffe6, 21),   // 227
    (0x3fffe9, 22),   // 228
    (0x1fffe7, 21),   // 229
    (0x1fffe8, 21),   // 230
    (0x7ffff3, 23),   // 231
    (0x3fffea, 22),   // 232
    (0x3fffeb, 22),   // 233
    (0x1ffffee, 25),  // 234
    (0x1ffffef, 25),  // 235
    (0xfffff4, 24),   // 236
    (0xfffff5, 24),   // 237
    (0x3ffffea, 26),  // 238
    (0x7ffff4, 23),   // 239
    (0x3ffffeb, 26),  // 240
    (0x7ffffe6, 27),  // 241
    (0x3ffffec, 26),  // 242
    (0x3ffffed, 26),  // 243
    (0x7ffffe7, 27),  // 244
    (0x7ffffe8, 27),  // 245
    (0x7ffffe9, 27),  // 246
    (0x7ffffea, 27),  // 247
    (0x7ffffeb, 27),  // 248
    (0xffffffe, 28),  // 249
    (0x7ffffec, 27),  // 250
    (0x7ffffed, 27),  // 251
    (0x7ffffee, 27),  // 252
    (0x7ffffef, 27),  // 253
    (0x7fffff0, 27),  // 254
    (0x3ffffee, 26),  // 255
    (0x3fffffff, 30), // 256
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoder_appends_static_and_never_indexed_fields() {
        let mut output = vec![0xfe];
        encode(
            [
                (b":method".as_slice(), b"GET".as_slice()),
                (b"custom-key", b"custom-value"),
                (b"authorization", b"secret"),
            ],
            &mut output,
        );
        assert_eq!(
            output,
            hex("fe2082100a637573746f6d2d6b65790c637573746f6d2d76616c75651f0806736563726574")
        );
        let mut decoder = Decoder::new(0, 1024, 10);
        assert_eq!(
            decoder.decode(&output[1..]).unwrap(),
            headers(&[
                (":method", "GET"),
                ("custom-key", "custom-value"),
                ("authorization", "secret")
            ])
        );
        assert_eq!(decoder.decode(&[0xbe]), Err(Error::InvalidIndex));
    }

    #[test]
    fn encoder_accepts_borrowed_fields_with_duplicates_and_opaque_values() {
        let mut headers = maximus_core::HeaderMap::new();
        headers.append("x-repeat", maximus_core::HeaderValue::from_static("one"));
        headers.append("x-repeat", maximus_core::HeaderValue::from_static("two"));
        headers.insert(
            "x-bin",
            maximus_core::HeaderValue::from_bytes(&[0x80, 0xff]).unwrap(),
        );
        let mut output = Vec::new();
        encode(
            headers
                .iter()
                .map(|(n, v)| (n.as_str().as_bytes(), v.as_bytes())),
            &mut output,
        );
        let decoded = Decoder::new(0, 1024, 3).decode(&output).unwrap();
        assert_eq!(
            decoded
                .iter()
                .filter(|(n, _)| n == b"x-repeat")
                .map(|(_, v)| v.as_slice())
                .collect::<Vec<_>>(),
            [b"one".as_slice(), b"two".as_slice()],
        );
        assert!(decoded.contains(&(b"x-bin".to_vec(), vec![0x80, 0xff])));
    }

    #[test]
    fn rfc_responses_evict_entries_with_and_without_huffman() {
        let variants = [
            [
                "4803333032580770726976617465611d4d6f6e2c203231204f637420323031332032303a31333a323120474d546e1768747470733a2f2f7777772e6578616d706c652e636f6d",
                "4803333037c1c0bf",
                "88c1611d4d6f6e2c203231204f637420323031332032303a31333a323220474d54c05a04677a69707738666f6f3d4153444a4b48514b425a584f5157454f50495541585157454f49553b206d61782d6167653d333630303b2076657273696f6e3d31",
            ],
            [
                "488264025885aec3771a4b6196d07abe941054d444a8200595040b8166e082a62d1bff6e919d29ad171863c78f0b97c8e9ae82ae43d3",
                "4883640effc1c0bf",
                "88c16196d07abe941054d444a8200595040b8166e084a62d1bffc05a839bd9ab77ad94e7821dd7f2e6c7b335dfdfcd5b3960d5af27087f3672c1ab270fb5291f9587316065c003ed4ee5b1063d5007",
            ],
        ];
        for blocks in variants {
            let mut decoder = Decoder::new(256, 1024, 10);
            let mut expected = headers(&[
                (":status", "302"),
                ("cache-control", "private"),
                ("date", "Mon, 21 Oct 2013 20:13:21 GMT"),
                ("location", "https://www.example.com"),
            ]);
            assert_eq!(decoder.decode(&hex(blocks[0])).unwrap(), expected);
            expected[0].1 = b"307".to_vec();
            assert_eq!(decoder.decode(&hex(blocks[1])).unwrap(), expected);
            expected[0].1 = b"200".to_vec();
            expected[2].1 = b"Mon, 21 Oct 2013 20:13:22 GMT".to_vec();
            expected.extend(headers(&[
                ("content-encoding", "gzip"),
                (
                    "set-cookie",
                    "foo=ASDJKHQKBZXOQWEOPIUAXQWEOIU; max-age=3600; version=1",
                ),
            ]));
            assert_eq!(decoder.decode(&hex(blocks[2])).unwrap(), expected);
            assert_eq!(
                decoder.decode(&[0xbe, 0xbf, 0xc0]).unwrap(),
                vec![
                    expected[5].clone(),
                    expected[4].clone(),
                    expected[2].clone()
                ]
            );
            assert_eq!(decoder.decode(&[0xc1]), Err(Error::InvalidIndex));
        }
    }

    #[test]
    fn huffman_rejects_eos_long_padding_and_incomplete_codes() {
        for malformed in [
            "0001618118",
            "00016181ff",
            "00016184ffffffff",
            "00016182fffe",
            "0001618100",
        ] {
            assert_eq!(
                Decoder::new(4096, 1024, 10).decode(&hex(malformed)),
                Err(Error::InvalidHuffman),
                "{malformed}"
            );
        }
        assert_eq!(
            Decoder::new(4096, 1024, 10)
                .decode(&hex("000161811f"))
                .unwrap(),
            headers(&[("a", "a")])
        );
        assert_eq!(
            Decoder::new(4096, 1024, 10).decode(&hex("008080")).unwrap(),
            headers(&[("", "")])
        );
    }

    #[test]
    fn truncated_representations_and_invalid_indices_are_rejected() {
        for malformed in ["ff", "40", "000161", "000261", "00016182ff", "3f"] {
            assert_eq!(
                Decoder::new(4096, 1024, 10).decode(&hex(malformed)),
                Err(Error::Truncated),
                "{malformed}"
            );
        }
        for malformed in ["80", "be", "0f7f00"] {
            assert_eq!(
                Decoder::new(4096, 1024, 10).decode(&hex(malformed)),
                Err(Error::InvalidIndex),
                "{malformed}"
            );
        }
        let mut overflow = vec![0xff; 24];
        overflow.push(0);
        assert_eq!(
            Decoder::new(4096, 1024, 10).decode(&overflow),
            Err(Error::IntegerOverflow)
        );
        let mut overlong = vec![0xff];
        overlong.extend_from_slice(&[0x80; 24]);
        overlong.push(0);
        assert_eq!(
            Decoder::new(4096, 1024, 10).decode(&overlong),
            Err(Error::IntegerOverflow)
        );
    }

    #[test]
    fn bounds_include_field_overhead_and_huffman_expansion() {
        assert_eq!(
            Decoder::new(0, 42, 1).decode(&[0x82]).unwrap(),
            headers(&[(":method", "GET")])
        );
        assert_eq!(
            Decoder::new(0, 41, 1).decode(&[0x82]),
            Err(Error::HeaderListTooLarge)
        );
        assert_eq!(
            Decoder::new(0, 83, 2).decode(&[0x82, 0x82]),
            Err(Error::HeaderListTooLarge)
        );
        assert_eq!(
            Decoder::new(0, 84, 1).decode(&[0x82, 0x82]),
            Err(Error::TooManyHeaders)
        );
        assert_eq!(Decoder::new(0, 0, 0).decode(&[]).unwrap(), vec![]);
        let compressed = hex("0001618318c63f");
        assert_eq!(
            Decoder::new(0, 37, 1).decode(&compressed).unwrap(),
            headers(&[("a", "aaaa")])
        );
        assert_eq!(
            Decoder::new(0, 36, 1).decode(&compressed),
            Err(Error::HeaderListTooLarge)
        );
        assert_eq!(
            Decoder::new(0, 33, 1).decode(&hex("0001610162")),
            Err(Error::HeaderListTooLarge)
        );
        assert_eq!(
            Decoder::new(0, 32, 1).decode(&hex("00016100")),
            Err(Error::HeaderListTooLarge)
        );
    }

    #[test]
    fn table_size_updates_evict_and_only_precede_fields() {
        let mut decoder = Decoder::new(34, 1024, 10);
        assert_eq!(
            decoder.decode(&hex("4001610162be")).unwrap(),
            headers(&[("a", "b"), ("a", "b")])
        );
        // A dynamic name is copied before inserting its new value evicts it.
        assert_eq!(
            decoder.decode(&hex("7e0163be")).unwrap(),
            headers(&[("a", "c"), ("a", "c")])
        );
        assert_eq!(
            decoder.decode(&hex("203f034001640165be")).unwrap(),
            headers(&[("d", "e"), ("d", "e")])
        );
        assert_eq!(decoder.decode(&hex("20be")), Err(Error::InvalidIndex));
        assert_eq!(
            Decoder::new(34, 1024, 10).decode(&hex("3f04")),
            Err(Error::InvalidTableSize)
        );
        assert_eq!(
            Decoder::new(34, 1024, 10).decode(&hex("8220")),
            Err(Error::InvalidTableSize)
        );
    }

    #[test]
    fn oversized_table_entry_empties_the_table_and_still_emits_field() {
        let mut decoder = Decoder::new(34, 1024, 10);
        decoder.decode(&hex("4001610162")).unwrap();
        assert_eq!(
            decoder.decode(&hex("400161026263")).unwrap(),
            headers(&[("a", "bc")])
        );
        assert_eq!(decoder.decode(&[0xbe]), Err(Error::InvalidIndex));
    }

    #[test]
    fn literal_representations_preserve_opaque_bytes_and_duplicates() {
        let block = hex("0001ff01001001ff01004001ff0100bebe");
        assert_eq!(
            Decoder::new(4096, 1024, 10).decode(&block).unwrap(),
            vec![(vec![255], vec![0]); 5]
        );
    }

    #[test]
    fn encoder_roundtrips_long_binary_literals_and_empty_values() {
        let name = vec![0xfe; 1337];
        let value: Vec<u8> = (0..=255).collect();
        let input = [
            (name.as_slice(), value.as_slice()),
            (b"x".as_slice(), b"".as_slice()),
        ];
        let mut output = Vec::new();
        encode(input, &mut output);
        assert_eq!(
            Decoder::new(0, 2000, 2).decode(&output).unwrap(),
            vec![(name, value), (b"x".to_vec(), vec![])]
        );
    }

    #[test]
    fn independently_encoded_huffman_covers_every_octet() {
        // Generated once with python-hyper/hpack 4.1.0, without sharing our table.
        // Expected bytes are the complete byte alphabet in ascending order.
        let wire = hex(concat!(
            "4081f3ffc803ffc7fffd8fffffe2fffffe3fffffe4fffffe5fffffe6fffffe7fffffe8ffffeafffffff3fffffa7fffffabff",
            "ffffdfffffebfffffecfffffedfffffeefffffefffffff0ffffff1ffffff2fffffffbfffffcffffffd3fffffd7fffffdbfff",
            "ffdffffffe3fffffe7fffffebfffffed4fe3f9ffaffcabf1febfafefe7fdfd2cbb00089969b71d79fb9f7fff20ffbff3ff50",
            "ddbd7f061c58f265cd9f469d5af66dddbf871e5f9cff7ff7fffc3ff9ffe45fff4719242cb34e6e9d68a6a3d7dac426defe3c",
            "faf7fffbfe7ffbffdffffffcfffe6ffff4bfff9ffffa3fffd3ffff53fffd5ffffb3fffeb7fffdaffffb7ffff73fffeeffffd",
            "effffebffffbfffffd9ffffdbfffebffffe0ffffeeffffc3ffff8bffff1ffffe4fffee7fffb1ffff97fffd9ffffcdffff9ff",
            "fffbffffdafffeeffff4ffffb7fffee7fffe8ffffd3fffdeffffd5fffeeffffbdffffe1fffdfffff7fffff5ffffecffff07f",
            "ff87fffe0ffff17fffedffff87ffff77fffeffffeaffff8bfffe3ffff93ffff87fffcbffff37ffff1fffff83ffffe1fffebf",
            "ffe3ffff3fffff2ffffa3ffffd9fffff17ffffc7fffff27ffffdefffffbffffff2fffff8fffffb7fff97fff8fffffe6fffff",
            "c1fffff87ffffe7fffffc5ffffe5fffe4ffff2fffffd1fffff4ffffffefffffe3fffffc9fffff97fffb3ffffcffffb7fffcd",
            "ffff4ffff9ffffd1ffffcffffeaffffafffffddffffeffffff4fffff5fffffabffffa7ffffd7fffff9bffffecfffffb7ffff",
            "f3fffffe8fffffd3fffffabfffff5fffffff7ffffecfffffdbfffffbbfffff7ffffff0fffffbbf",
        ));
        assert_eq!(
            Decoder::new(4096, 289, 1).decode(&wire).unwrap(),
            vec![(b"x".to_vec(), (0..=255).collect())]
        );
        assert_eq!(
            Decoder::new(4096, 288, 1).decode(&wire),
            Err(Error::HeaderListTooLarge)
        );
    }

    fn hex(s: &str) -> Vec<u8> {
        let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        compact
            .as_bytes()
            .chunks(2)
            .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
            .collect()
    }

    fn headers(pairs: &[(&str, &str)]) -> Vec<(Vec<u8>, Vec<u8>)> {
        pairs
            .iter()
            .map(|(n, v)| (n.as_bytes().to_vec(), v.as_bytes().to_vec()))
            .collect()
    }

    #[test]
    fn rfc_requests_share_dynamic_table_with_huffman() {
        let mut decoder = Decoder::new(4096, 1024, 10);
        let mut expected = headers(&[
            (":method", "GET"),
            (":scheme", "http"),
            (":path", "/"),
            (":authority", "www.example.com"),
        ]);
        assert_eq!(
            decoder
                .decode(&hex("828684418cf1e3c2e5f23a6ba0ab90f4ff"))
                .unwrap(),
            expected
        );
        expected.push((b"cache-control".to_vec(), b"no-cache".to_vec()));
        assert_eq!(
            decoder.decode(&hex("828684be5886a8eb10649cbf")).unwrap(),
            expected
        );
        assert_eq!(
            decoder
                .decode(&hex("828785bf408825a849e95ba97d7f8925a849e95bb8e8b4bf"))
                .unwrap(),
            headers(&[
                (":method", "GET"),
                (":scheme", "https"),
                (":path", "/index.html"),
                (":authority", "www.example.com"),
                ("custom-key", "custom-value")
            ])
        );
    }

    #[test]
    fn rfc_requests_share_dynamic_table_without_huffman() {
        let mut decoder = Decoder::new(4096, 1024, 10);
        let mut expected = headers(&[
            (":method", "GET"),
            (":scheme", "http"),
            (":path", "/"),
            (":authority", "www.example.com"),
        ]);
        assert_eq!(
            decoder
                .decode(&hex("828684410f7777772e6578616d706c652e636f6d"))
                .unwrap(),
            expected
        );
        expected.push((b"cache-control".to_vec(), b"no-cache".to_vec()));
        assert_eq!(
            decoder
                .decode(&hex("828684be58086e6f2d6361636865"))
                .unwrap(),
            expected
        );
        assert_eq!(
            decoder
                .decode(&hex(
                    "828785bf400a637573746f6d2d6b65790c637573746f6d2d76616c7565"
                ))
                .unwrap(),
            headers(&[
                (":method", "GET"),
                (":scheme", "https"),
                (":path", "/index.html"),
                (":authority", "www.example.com"),
                ("custom-key", "custom-value")
            ])
        );
    }

    #[test]
    fn decodes_static_indexed_header() {
        let mut decoder = Decoder::new(4096, 1024, 10);
        assert_eq!(
            decoder.decode(&[0x82]).unwrap(),
            vec![(b":method".to_vec(), b"GET".to_vec())]
        );
    }
}
