#![no_main]
use http_body_util::BodyExt;
use libfuzzer_sys::fuzz_target;
use maximus_core::{IntoResponse, Limits, Request};
use std::{rc::Rc, time::Duration};

mod wire;
use wire::Wire;
thread_local! { static RUNTIME: compio::runtime::Runtime = compio::runtime::Runtime::new().unwrap(); }
fuzz_target!(|input: &[u8]| {
    if input.len() > 65536 {
        return;
    }
    RUNTIME.with(|runtime| {
        runtime.block_on(async {
            let limits = Rc::new(Limits {
                body_bytes: 16384,
                requests_per_connection: 64,
                header_timeout: Duration::from_millis(20),
                request_timeout: Duration::from_millis(20),
                ..Limits::default()
            });
            let handler = Rc::new(|request: Request| async {
                match request.into_body().collect().await {
                    Ok(body) => body.to_bytes().len().to_string().into_response(),
                    Err(error) => error.into_response(),
                }
            });
            let _ = maximus_http1::serve_futures(
                Wire {
                    input: input.to_vec(),
                    position: 0,
                    output: 0,
                },
                handler,
                limits,
                maximus_runtime::Shutdown::default(),
            )
            .await;
        })
    });
});
