use super::*;

#[test]
fn test_decode_ready_for_query() -> Result<(), Error> {
    const DATA: &[u8] = b"E";

    let m = ReadyForQuery::decode_body(Bytes::from_static(DATA))?;

    assert!(matches!(m.transaction_status, TransactionStatus::Error));

    Ok(())
}

#[test]
fn an_empty_ready_for_query_is_refused() {
    assert!(ReadyForQuery::decode_body(Bytes::new()).is_err());
}
