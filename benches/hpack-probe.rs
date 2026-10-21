//! Standalone codec probe. Compile this file against each frozen source tree.
#[allow(dead_code)]
#[path = "../crates/http2/src/hpack.rs"]
mod hpack;
use std::{hint::black_box, time::Instant};
fn main() {
    // Obtain Appendix A fields through the decoder, without duplicating its table.
    let mut cases = Vec::new();
    for index in 1..=61 {
        let fields = hpack::Decoder::new(0, 4096, 32)
            .decode(&[0x80 | index])
            .unwrap();
        let field = fields.into_iter().next().unwrap();
        let name = field.name.into_owned();
        let value = field.value.into_owned();
        cases.push((format!("exact-{index}"), name.clone(), value));
        cases.push((format!("literal-{index}"), name, b"opaque".to_vec()));
    }
    cases.push(("unknown".into(), b"x-unknown".to_vec(), b"opaque".to_vec()));
    cases.push(("binary".into(), vec![0xfe; 1337], (0..=255).collect()));
    println!("case,operations,ns_per_operation");
    for (name, key, value) in cases {
        let mut output = Vec::with_capacity(4096);
        let fields = [(key.as_slice(), value.as_slice())];
        hpack::encode(fields, &mut output);
        let decoded = hpack::Decoder::new(0, 4096, 32).decode(&output).unwrap();
        assert_eq!(decoded[0].name.as_ref(), key);
        assert_eq!(decoded[0].value.as_ref(), value);
        const N: usize = 200_000;
        for _ in 0..1000 {
            output.clear();
            hpack::encode(black_box(fields), black_box(&mut output));
        }
        let started = Instant::now();
        for _ in 0..N {
            output.clear();
            hpack::encode(black_box(fields), black_box(&mut output));
            black_box(&output);
        }
        println!(
            "{name},{N},{:.3}",
            started.elapsed().as_nanos() as f64 / N as f64
        );
    }
}
