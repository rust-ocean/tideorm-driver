use super::{ColumnDefinition, ExtendedType};
use crate::collation::Collation;
use crate::io::ProtocolDecode;
use crate::protocol::Capabilities;
use crate::MySqlTypeInfo;
use bytes::Bytes;
use driver_core::type_info::TypeInfo;

/// A `LONGTEXT` column definition with `collation`, carrying MariaDB's
/// extended metadata `extended` when it is given.
fn long_text(collation: u16, extended: Option<&[u8]>) -> Bytes {
    let mut packet = Vec::new();
    for name in ["def", "app", "docs", "docs", "body", "body"] {
        packet.push(u8::try_from(name.len()).unwrap());
        packet.extend_from_slice(name.as_bytes());
    }
    if let Some(info) = extended {
        packet.push(u8::try_from(info.len()).unwrap());
        packet.extend_from_slice(info);
    }
    packet.push(0x0c);
    packet.extend_from_slice(&collation.to_le_bytes());
    packet.extend_from_slice(&u32::MAX.to_le_bytes());
    packet.push(0xfb); // LONG_BLOB
    packet.extend_from_slice(&(16_u16 | 128).to_le_bytes()); // BLOB | BINARY
    packet.push(0);
    packet.extend_from_slice(&[0, 0]);
    Bytes::from(packet)
}

fn name(packet: Bytes, capabilities: Capabilities) -> String {
    let column = ColumnDefinition::decode_with(packet, capabilities).unwrap();
    MySqlTypeInfo::from_column(&column).name().to_owned()
}

#[test]
fn binary_data_is_the_binary_collation() {
    let utf8mb4_bin = 46;
    assert_eq!(
        name(long_text(utf8mb4_bin, None), Capabilities::empty()),
        "LONGTEXT"
    );
    assert_eq!(
        name(long_text(Collation::BINARY.0, None), Capabilities::empty()),
        "LONGBLOB"
    );
}

#[test]
fn mariadb_extended_metadata_names_json_and_uuid() {
    let extended = Capabilities::MARIADB_CLIENT_EXTENDED_TYPE_INFO;
    let json = long_text(46, Some(b"\x01\x04json"));
    assert_eq!(name(json, extended), "JSON");
    let uuid = long_text(8, Some(b"\x00\x04uuid"));
    assert_eq!(name(uuid, extended), "UUID");
    // Another type, or none, leaves the wire type's name.
    let point = long_text(Collation::BINARY.0, Some(b"\x00\x05point"));
    assert_eq!(name(point, extended), "LONGBLOB");
    assert_eq!(name(long_text(46, Some(b"")), extended), "LONGTEXT");
    let column = ColumnDefinition::decode_with(long_text(46, Some(b"")), extended).unwrap();
    assert_eq!(column.extended, ExtendedType::None);
}
