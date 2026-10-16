use maximus_io::{Backend, runtime};
#[test]
fn explicit_poll_and_local_tasks() {
    let rt = runtime(Backend::Epoll, 64).unwrap();
    assert_eq!(rt.driver_type(), compio::driver::DriverType::Poll);
    let result = rt.block_on(async {
        let value = std::rc::Rc::new(42);
        compio::runtime::spawn(async move { *value }).await.unwrap()
    });
    assert_eq!(result, 42);
}
#[test]
fn invalid_capacity_is_rejected() {
    assert!(runtime(Backend::Auto, 0).is_err());
}
#[test]
fn auto_backend_operates() {
    assert_eq!(runtime(Backend::Auto, 64).unwrap().block_on(async { 7 }), 7);
}
#[cfg(not(target_os = "linux"))]
#[test]
fn uring_never_silently_falls_back_when_explicit() {
    assert!(runtime(Backend::IoUring, 64).is_err());
}

#[cfg(target_os = "linux")]
#[test]
fn explicit_uring_is_used() {
    let rt = runtime(Backend::IoUring, 64).expect("Linux qualification requires io_uring access");
    assert_eq!(rt.driver_type(), compio::driver::DriverType::IoUring);
    assert_eq!(rt.block_on(async { 9 }), 9);
}
