use maximus_runtime::{Shutdown, WorkerConfig};
#[test]
fn worker_config_rejects_invalid_limits() {
    assert!(
        WorkerConfig {
            workers: 0,
            ..Default::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        WorkerConfig {
            workers: 2,
            cpus: Some(vec![0]),
            ..Default::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        WorkerConfig {
            workers: 2,
            cpus: Some(vec![0, 0]),
            ..Default::default()
        }
        .validate()
        .is_err()
    );
}
#[test]
fn shutdown_wakes_all_waiters_and_late_subscribers() {
    let rt = WorkerConfig::default().build(0).unwrap();
    rt.block_on(async {
        let shutdown = Shutdown::default();
        let first = shutdown.clone();
        let a = compio::runtime::spawn(async move {
            first.cancelled().await;
            1
        });
        let second = shutdown.clone();
        let b = compio::runtime::spawn(async move {
            second.cancelled().await;
            2
        });
        shutdown.cancel();
        shutdown.cancel();
        assert_eq!(a.await.unwrap() + b.await.unwrap(), 3);
        shutdown.cancelled().await;
    });
}
#[test]
fn worker_stays_on_its_thread() {
    let thread = std::thread::current().id();
    let rt = WorkerConfig::default().build(0).unwrap();
    rt.block_on(async {
        let rc = std::rc::Rc::new(1);
        assert_eq!(
            compio::runtime::spawn(async move { (*rc, std::thread::current().id()) })
                .await
                .unwrap(),
            (1, thread)
        );
    });
}

#[cfg(target_os = "linux")]
#[test]
fn affinity_pins_to_an_allowed_cpu() {
    let cpu = maximus_runtime::allowed_cpus().unwrap()[0];
    std::thread::spawn(move || {
        let config = WorkerConfig {
            cpus: Some(vec![cpu]),
            ..Default::default()
        };
        let _rt = config.build(0).unwrap();
        assert_eq!(maximus_runtime::allowed_cpus().unwrap(), vec![cpu]);
    })
    .join()
    .unwrap();
}
