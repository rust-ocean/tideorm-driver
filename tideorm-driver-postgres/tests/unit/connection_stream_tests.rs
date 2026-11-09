use super::*;
use driver_core::transaction::TransactionState;

#[test]
fn test_parse_server_version_num() {
    // old style
    assert_eq!(parse_server_version("9.6.1"), Some(90601));
    // new style
    assert_eq!(parse_server_version("10.1"), Some(100001));
    // old style without minor version
    assert_eq!(parse_server_version("9.6devel"), Some(90600));
    // new style without minor version, e.g.  */
    assert_eq!(parse_server_version("10devel"), Some(100000));
    assert_eq!(parse_server_version("13devel87"), Some(130000));
    // unknown
    assert_eq!(parse_server_version("unknown"), None);
}

#[test]
fn every_ready_for_query_updates_server_state() {
    driver_core::rt::test_block_on(async {
        let input = b"ITEI"
            .iter()
            .flat_map(|&status| [b'Z', 0, 0, 0, 5, status])
            .collect();
        let mut stream = PgStream::scripted(input);
        for expected in [
            TransactionState::Idle,
            TransactionState::Active,
            TransactionState::Failed,
            TransactionState::Idle,
        ] {
            stream.recv().await.unwrap();
            assert_eq!(stream.transaction_state, expected);
        }
    });
}

#[test]
fn a_length_below_its_own_size_is_refused() {
    driver_core::rt::test_block_on(async {
        for len in 0..4 {
            let mut stream = PgStream::scripted(vec![b'Z', 0, 0, 0, len, b'I', b'I', b'I']);
            assert!(matches!(
                stream.recv_unchecked().await,
                Err(Error::Protocol(_))
            ));
        }
    });
}
