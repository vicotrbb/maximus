//! Status-line formatting probe. The direct variant uses existing validated HTTP APIs.
use maximus::{StatusCode, http::Version};
use std::{hint::black_box, io::Write, time::Instant};
fn main() {
    println!("variant,status,operations,ns_per_operation");
    for code in [200, 204, 301, 404, 418, 500, 599, 999] {
        let status = StatusCode::from_u16(code).unwrap();
        let version = black_box(Version::HTTP_11);
        let protocol = if version == Version::HTTP_10 {
            "HTTP/1.0"
        } else {
            "HTTP/1.1"
        };
        let expected = format!(
            "{protocol} {code} {}\r\n",
            status.canonical_reason().unwrap_or("")
        );
        for mode in ["integer", "string", "append", "digits"] {
            let mut output = Vec::with_capacity(128);
            const N: usize = 2_000_000;
            let started = Instant::now();
            for _ in 0..N {
                let status = black_box(status);
                let protocol = black_box(protocol);
                output.clear();
                match mode {
                    "integer" => write!(
                        &mut output,
                        "{protocol} {} {}\r\n",
                        status.as_u16(),
                        status.canonical_reason().unwrap_or("")
                    )
                    .unwrap(),
                    "string" => write!(
                        &mut output,
                        "{protocol} {} {}\r\n",
                        status.as_str(),
                        status.canonical_reason().unwrap_or("")
                    )
                    .unwrap(),
                    "append" => {
                        output.extend_from_slice(protocol.as_bytes());
                        output.push(b' ');
                        output.extend_from_slice(status.as_str().as_bytes());
                        output.push(b' ');
                        output
                            .extend_from_slice(status.canonical_reason().unwrap_or("").as_bytes());
                        output.extend_from_slice(b"\r\n");
                    }
                    _ => {
                        let code = status.as_u16();
                        let digits = [
                            b' ',
                            b'0' + (code / 100) as u8,
                            b'0' + (code / 10 % 10) as u8,
                            b'0' + (code % 10) as u8,
                            b' ',
                        ];
                        for part in [
                            protocol.as_bytes(),
                            &digits,
                            status.canonical_reason().unwrap_or("").as_bytes(),
                            b"\r\n",
                        ] {
                            output.extend_from_slice(part);
                        }
                    }
                }
                black_box(&output);
            }
            let elapsed = started.elapsed();
            assert_eq!(output, expected.as_bytes());
            println!(
                "{mode},{code},{N},{:.3}",
                elapsed.as_nanos() as f64 / N as f64
            );
        }
    }
}
