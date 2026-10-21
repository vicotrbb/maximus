//! Private gzip decision probe: compile the actual module, without exporting a benchmark API.
extern crate maximus as maximus_core;
#[allow(dead_code)]
mod compression {
    pub fn accepts(headers: &maximus::HeaderMap) -> bool {
        accepts_gzip(headers)
    }
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/framework/src/compression.rs"
    ));
}
use std::{hint::black_box, time::Instant};
fn main() {
    println!("engine,case,operations,ns_per_operation");
    for (name, value, expected) in [
        ("absent", "", false),
        ("gzip", "gzip", true),
        ("weighted", "br;q=1, gzip;q=0.8, identity;q=0.5", true),
        ("veto", "*;q=1, gzip;q=0", false),
        ("unknown", "br;q=1, zstd;q=0.9, deflate;q=0.7", false),
        ("duplicates", "gzip;q=1, gzip;q=0.1, *;q=0", true),
        ("smallest", "gzip;q=0.001", true),
    ] {
        let mut headers = maximus::HeaderMap::new();
        if !value.is_empty() {
            headers.insert("accept-encoding", value.parse().unwrap());
        }
        const N: usize = 2_000_000;
        let start = Instant::now();
        let mut actual = false;
        for _ in 0..N {
            actual = black_box(compression::accepts(black_box(&headers)));
        }
        assert_eq!(actual, expected);
        println!(
            "maximus,{name},{N},{:.3}",
            start.elapsed().as_nanos() as f64 / N as f64
        );
    }
}
