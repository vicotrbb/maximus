#![no_main]
use libfuzzer_sys::fuzz_target;
// Compile the owned codec directly so fuzzing does not expand the public API.
#[path = "../../crates/http2/src/hpack.rs"]
mod hpack;

fuzz_target!(|input: &[u8]| {
    let mut decoder = hpack::Decoder::new(4096, 16384, 64);
    // Length-prefixed blocks preserve dynamic state across successful decodes.
    let mut rest = input;
    while rest.len() >= 2 {
        let size = usize::from(u16::from_be_bytes([rest[0], rest[1]])).min(rest.len() - 2);
        let block = &rest[2..2 + size];
        rest = &rest[2 + size..];
        let Ok(headers) = decoder.decode(block) else {
            break;
        };
        assert!(headers.len() <= 64);
        assert!(
            headers
                .iter()
                .map(|(n, v)| n.len() + v.len() + 32)
                .sum::<usize>()
                <= 16384
        );
        let fields: Vec<_> = headers
            .iter()
            .map(|(n, v)| (n.as_slice(), v.as_slice()))
            .collect();
        let mut encoded = Vec::new();
        hpack::encode(&fields, &mut encoded);
        assert_eq!(
            hpack::Decoder::new(0, 16384, 64).decode(&encoded).unwrap(),
            headers
        );
    }
});
