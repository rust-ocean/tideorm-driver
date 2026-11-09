use super::*;

/// `payload` framed after `prefix`, one packet per 0xFF_FF_FF bytes, the last
/// shorter (empty when the payload fills the others exactly).
fn framed(prefix: &[u8], payload: &[u8], mut sequence_id: u8) -> Vec<u8> {
    let mut parts: Vec<&[u8]> = payload.chunks(0xFF_FF_FF).collect();
    if payload.len().is_multiple_of(0xFF_FF_FF) {
        parts.push(&[]);
    }
    let mut bytes = prefix.to_vec();
    for part in parts {
        bytes.extend(&u32::try_from(part.len()).unwrap().to_le_bytes()[..3]);
        bytes.push(sequence_id);
        sequence_id = sequence_id.wrapping_add(1);
        bytes.extend(part);
    }
    bytes
}

#[test]
fn a_payload_is_split_into_packets_of_at_most_0xffffff_bytes() {
    for len in [0, 5, 0xFF_FF_FE, 0xFF_FF_FF, 2 * 0xFF_FF_FF + 5] {
        let payload: Vec<u8> = (0..=u8::MAX).cycle().take(len).collect();
        let mut buf = vec![0xAA];
        let mut sequence_id = 7;
        Packet(&payload[..])
            .encode_with(&mut buf, (Capabilities::empty(), &mut sequence_id))
            .unwrap();
        // not `assert_eq!`, which would print every byte
        assert!(buf == framed(&[0xAA], &payload, 7), "{len}");
        let packets = u8::try_from(len / 0xFF_FF_FF + 1).unwrap();
        assert_eq!(sequence_id, 7 + packets, "{len}");
    }
}
