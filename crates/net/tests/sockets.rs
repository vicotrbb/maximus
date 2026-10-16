use maximus_net::{SocketOptions, bind_udp, listen};
#[test]
fn reuseport_tcp_and_udp_bind() {
    let opts = SocketOptions::default();
    let a = listen("127.0.0.1:0".parse().unwrap(), &opts).unwrap();
    let address = a.local_addr().unwrap();
    let b = listen(address, &opts).unwrap();
    assert_eq!(b.local_addr().unwrap(), address);
    let udp = bind_udp(address, &opts).unwrap();
    assert_eq!(udp.local_addr().unwrap(), address);
}
#[test]
fn invalid_backlog_fails() {
    let options = SocketOptions {
        backlog: 0,
        ..SocketOptions::default()
    };
    assert!(listen("127.0.0.1:0".parse().unwrap(), &options).is_err());
}
