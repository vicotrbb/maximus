use http::HeaderValue;
use std::time::SystemTime;

/// Current HTTP response date, cached per worker for one second.
#[doc(hidden)]
pub fn response_date() -> HeaderValue {
    cached_date(SystemTime::now())
}

/// HTTP dates have one-second resolution. Each worker owns its cached header;
/// equality, rather than ordering, also refreshes it when the clock moves back.
fn cached_date(now: std::time::SystemTime) -> http::HeaderValue {
    thread_local! {
        static DATE: std::cell::RefCell<Option<(u64, http::HeaderValue)>> = const { std::cell::RefCell::new(None) };
    }
    let second = now
        .duration_since(std::time::UNIX_EPOCH)
        .expect("HTTP dates require a time after the Unix epoch")
        .as_secs();
    DATE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((cached_second, value)) = &*cache
            && *cached_second == second
        {
            return value.clone();
        }
        let value: http::HeaderValue = httpdate::fmt_http_date(now)
            .parse()
            .expect("valid HTTP date");
        *cache = Some((second, value.clone()));
        value
    })
}

#[cfg(test)]
mod date_tests {
    use super::cached_date;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn dates_follow_second_day_rollover_and_backward_clock_changes() {
        // Literal civil dates are independent of the formatting/cache implementation.
        for (seconds, nanos, expected) in [
            (86_399, 999_999_999, "Thu, 01 Jan 1970 23:59:59 GMT"),
            (86_400, 0, "Fri, 02 Jan 1970 00:00:00 GMT"),
            (86_400, 500_000_000, "Fri, 02 Jan 1970 00:00:00 GMT"),
            (86_399, 0, "Thu, 01 Jan 1970 23:59:59 GMT"),
            (0, 0, "Thu, 01 Jan 1970 00:00:00 GMT"),
            (784_111_777, 0, "Sun, 06 Nov 1994 08:49:37 GMT"),
        ] {
            assert_eq!(
                cached_date(UNIX_EPOCH + Duration::new(seconds, nanos)),
                expected
            );
        }
    }
}
