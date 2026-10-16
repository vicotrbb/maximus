use maximus_tls::TlsConfig;
#[test]
fn validates_pem_and_sets_protocols() {
    assert!(TlsConfig::from_pem(b"not a certificate", b"not a key", false).is_err());
    let rcgen::CertifiedKey { cert, signing_key } =
        rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let config = TlsConfig::from_pem(
        cert.pem().as_bytes(),
        signing_key.serialize_pem().as_bytes(),
        true,
    )
    .unwrap();
    assert_eq!(
        config.rustls().alpn_protocols,
        [b"h2".to_vec(), b"http/1.1".to_vec()]
    );
    assert_eq!(config.rustls().max_early_data_size, 0);
}
