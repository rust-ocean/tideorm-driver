//! Helpers the unit tests share.

/// `payload` framed as a packet with sequence id 0.
pub(crate) fn packet(payload: &[u8]) -> Vec<u8> {
    let mut bytes = u32::try_from(payload.len()).unwrap().to_le_bytes().to_vec();
    bytes.extend(payload);
    bytes
}
