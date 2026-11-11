//! What a server, or anyone between it and the driver, can send: every packet decoded from
//! arbitrary and corrupted bytes returns an error rather than panicking, and allocates no more
//! than those bytes justify. Each row's values are decoded too, as every type the driver reads.

use bytes::Bytes;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

use super::connect::{AuthSwitchRequest, Handshake};
use super::response::{EofPacket, ErrPacket, OkPacket};
use super::statement::{BinaryRow, PrepareOk};
use super::text::{ColumnDefinition, ColumnFlags, ColumnType, ExtendedType, TextRow};
use super::{Capabilities, Packet, Row};
use crate::collation::Collation;
use crate::column::ColumnOrigin;
use crate::decode::Decode;
use crate::ext::ustr::UStr;
use crate::io::{MySqlBufExt, ProtocolDecode};
use crate::test_support::fuzz;
use crate::types::{Json, JsonValue, MySqlTime};
use crate::{MySql, MySqlColumn, MySqlTypeInfo, MySqlValueFormat, MySqlValueRef};

const ITERATIONS: usize = 4000;

const HANDSHAKES: [&[u8]; 2] = [
    b"\n8.0.18\x00\x19\x00\x00\x00\x114aB0c\x06g\x00\xff\xff\xff\x02\x00\xff\xc7\x15\x00\x00\x00\
      \x00\x00\x00\x00\x00\x00\x00tL\x03s\x0f[4\rl4. \x00caching_sha2_password\x00",
    b"\n5.5.5-10.4.7-MariaDB-1:10.4.7+maria~bionic\x00\x0b\x00\x00\x00t6L\\j\"dS\x00\xfe\xf7\x08\
      \x02\x00\xff\x81\x15\x00\x00\x00\x00\x00\x00\x07\x00\x00\x00U14Oph9\"<H5n\x00\
      mysql_native_password\x00",
];

const OKS: [&[u8]; 3] = [
    b"\x00\x00\x00\x02@\x00\x00",
    b"\xfe\x01\x00\x02\x00\x00\x00\x05\x09info data",
    b"\x00\xfc\x10\x27\xfd\x01\x02\x03\x02\x00\x01\x00",
];

/// The capabilities a decoder can be given: MySQL's, MariaDB's, and an old server's.
const CAPABILITIES: [Capabilities; 3] = [
    Capabilities::PROTOCOL_41.union(Capabilities::DEPRECATE_EOF),
    Capabilities::PROTOCOL_41
        .union(Capabilities::DEPRECATE_EOF)
        .union(Capabilities::MARIADB_CLIENT_EXTENDED_TYPE_INFO),
    Capabilities::empty(),
];

fn samples(packets: &[&[u8]]) -> Vec<Vec<u8>> {
    packets.iter().map(|packet| packet.to_vec()).collect()
}

/// `values` as length-encoded strings.
fn lenenc(values: &[&[u8]]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for value in values {
        bytes.push(u8::try_from(value.len()).unwrap());
        bytes.extend(*value);
    }
    bytes
}

/// A column definition, with MariaDB's extended type info when `extended` is given.
fn definition(ty: ColumnType, extended: Option<&[u8]>) -> Vec<u8> {
    let mut bytes = lenenc(&[b"def", b"db", b"t", b"t", b"c", b"c"]);
    if let Some(extended) = extended {
        bytes.extend(lenenc(&[extended]));
    }
    bytes.push(0x0c);
    bytes.extend(Collation::UTF8MB4_GENERAL_CI.0.to_le_bytes());
    bytes.extend(255_u32.to_le_bytes());
    bytes.push(ty as u8);
    bytes.extend(0_u16.to_le_bytes());
    bytes.extend([0, 0, 0]);
    bytes
}

#[test]
fn every_connection_phase_packet_decodes_or_fails_without_panicking() {
    fuzz(1, ITERATIONS, &samples(&HANDSHAKES), |packet| {
        let _ = Handshake::decode(Bytes::copy_from_slice(packet));
    });
    let switches = samples(&[
        b"\xfecaching_sha2_password\0abcdefghijabcdefghij\0",
        b"\xfemysql_native_password\0abcdefghijabcdefghij\0",
        b"\xfemysql_clear_password\0",
    ]);
    fuzz(2, ITERATIONS, &switches, |packet| {
        for cleartext in [false, true] {
            let _ = AuthSwitchRequest::decode_with(Bytes::copy_from_slice(packet), cleartext);
        }
    });
}

#[test]
fn every_reply_packet_decodes_or_fails_without_panicking() {
    let mut replies = samples(&OKS);
    replies.extend(samples(&[
        b"\xff\x84\x04Got packets out of order",
        b"\xff\x19\x04#42000Unknown database 'unknown'",
        b"\xfe\x00\x00\x02\x00",
        b"\x00\x07\x00\x00\x00\x02\x00\x01\x00\x00\x00\x00",
    ]));
    fuzz(3, ITERATIONS, &replies, |packet| {
        let packet = Bytes::copy_from_slice(packet);
        let _ = OkPacket::decode(packet.clone());
        for capabilities in CAPABILITIES {
            let _ = ErrPacket::decode_with(packet.clone(), capabilities);
            let _ = EofPacket::decode_with(packet.clone(), capabilities);
            let _ = PrepareOk::decode_with(packet.clone(), capabilities);
            let _ = Packet(packet.clone()).eof(capabilities);
        }
    });
}

#[test]
fn every_column_definition_decodes_or_fails_without_panicking() {
    let definitions = vec![
        lenenc(&[b"\x01", b"\x05"]), // column counts, as a result set's first packet
        definition(ColumnType::LongLong, None),
        definition(ColumnType::VarString, None),
        definition(ColumnType::LongBlob, Some(b"\x01\x04json")),
        definition(ColumnType::String, Some(b"\x00\x04uuid")),
    ];
    fuzz(4, ITERATIONS, &definitions, |packet| {
        let mut count = Bytes::copy_from_slice(packet);
        let _ = count.get_uint_lenenc();
        let _ = Bytes::copy_from_slice(packet).get_bytes_lenenc();
        for capabilities in CAPABILITIES {
            if let Ok(def) =
                ColumnDefinition::decode_with(Bytes::copy_from_slice(packet), capabilities)
            {
                let _ = (def.name(), def.alias(), def.table(), def.schema());
                let _ = MySqlTypeInfo::from_column(&def).to_string();
            }
        }
    });
}

const TYPES: [ColumnType; 27] = [
    ColumnType::Decimal,
    ColumnType::Tiny,
    ColumnType::Short,
    ColumnType::Long,
    ColumnType::Float,
    ColumnType::Double,
    ColumnType::Null,
    ColumnType::Timestamp,
    ColumnType::LongLong,
    ColumnType::Int24,
    ColumnType::Date,
    ColumnType::Time,
    ColumnType::Datetime,
    ColumnType::Year,
    ColumnType::VarChar,
    ColumnType::Bit,
    ColumnType::Json,
    ColumnType::NewDecimal,
    ColumnType::Enum,
    ColumnType::Set,
    ColumnType::TinyBlob,
    ColumnType::MediumBlob,
    ColumnType::LongBlob,
    ColumnType::Blob,
    ColumnType::VarString,
    ColumnType::String,
    ColumnType::Geometry,
];

/// The type of a column read from one byte: the wire type, then a variant of what the column
/// declares beyond it.
fn type_info(spec: u8) -> MySqlTypeInfo {
    let spec = usize::from(spec);
    let mut info = MySqlTypeInfo::binary(TYPES[spec % TYPES.len()]);
    match spec / TYPES.len() {
        1 => info.flags |= ColumnFlags::UNSIGNED,
        2 => info.collation = Collation::UTF8MB4_GENERAL_CI,
        3 => info.max_size = Some(1),
        4 => info.max_size = Some(16),
        5 => info.extended = ExtendedType::Json,
        6 => info.extended = ExtendedType::Uuid,
        _ => {}
    }
    info
}

/// The columns at the front of `input` (a count, then a byte for each column's type) and the
/// row after them.
fn columns_and_row(input: &[u8]) -> (Vec<MySqlColumn>, Bytes) {
    let (count, rest) = input.split_first().unwrap_or((&0, &[]));
    let (specs, row) = rest.split_at(usize::from(count % 9).min(rest.len()));
    let columns = (0..)
        .zip(specs)
        .map(|(ordinal, &spec)| MySqlColumn {
            ordinal,
            name: UStr::new("c"),
            type_info: type_info(spec),
            origin: ColumnOrigin::Unknown,
        })
        .collect();
    (columns, Bytes::copy_from_slice(row))
}

fn spec_of(ty: ColumnType) -> u8 {
    u8::try_from(TYPES.iter().position(|&t| t == ty).unwrap()).unwrap()
}

/// Decodes each value of `row` as every type the driver reads.
fn decode_values(row: &Row, columns: &[MySqlColumn], format: MySqlValueFormat) {
    for (i, column) in columns.iter().enumerate() {
        let value = MySqlValueRef {
            value: row.get(i),
            row: Some(&row.storage),
            type_info: column.type_info.clone(),
            format,
        };
        let _ = value.decode_lossless();
        decode_as::<bool>(&value);
        decode_as::<i8>(&value);
        decode_as::<i64>(&value);
        decode_as::<u64>(&value);
        decode_as::<f32>(&value);
        decode_as::<f64>(&value);
        decode_as::<Decimal>(&value);
        decode_as::<String>(&value);
        decode_as::<Vec<u8>>(&value);
        decode_as::<Uuid>(&value);
        decode_as::<Json<JsonValue>>(&value);
        decode_as::<NaiveDate>(&value);
        decode_as::<NaiveTime>(&value);
        decode_as::<NaiveDateTime>(&value);
        decode_as::<DateTime<Utc>>(&value);
        decode_as::<MySqlTime>(&value);
        decode_as::<std::time::Duration>(&value);
        decode_as::<chrono::TimeDelta>(&value);
    }
}

fn decode_as<T: for<'r> Decode<'r, MySql>>(value: &MySqlValueRef<'_>) {
    let _ = T::decode(value.clone());
}

#[test]
fn every_text_row_decodes_or_fails_without_panicking() {
    let specs = [
        spec_of(ColumnType::VarString),
        spec_of(ColumnType::LongLong) + 27, // unsigned
        spec_of(ColumnType::NewDecimal),
        spec_of(ColumnType::Datetime),
        spec_of(ColumnType::Time),
        spec_of(ColumnType::Json),
        spec_of(ColumnType::Tiny) + 27 * 3, // boolean
        spec_of(ColumnType::Double),
    ];
    let mut row = vec![8];
    row.extend(specs);
    row.extend(lenenc(&[
        b"abc",
        b"18446744073709551615",
        b"-12.3400",
        b"2024-02-29 23:59:59.999999",
        b"-838:59:59",
        b"{\"a\":[1,2]}",
        b"1",
    ]));
    row.push(0xfb); // NULL
    fuzz(5, ITERATIONS, &[row], |input| {
        let (columns, row) = columns_and_row(input);
        if let Ok(TextRow(row)) = TextRow::decode_with(row, &columns) {
            decode_values(&row, &columns, MySqlValueFormat::Text);
        }
    });
}

#[test]
fn every_binary_row_decodes_or_fails_without_panicking() {
    let specs = [
        spec_of(ColumnType::LongLong),
        spec_of(ColumnType::Datetime),
        spec_of(ColumnType::Time),
        spec_of(ColumnType::VarString),
        spec_of(ColumnType::Double),
        spec_of(ColumnType::Date),
        spec_of(ColumnType::Blob),
        spec_of(ColumnType::Short),
    ];
    let mut row = vec![8];
    row.extend(specs);
    row.extend([0, 0, 0b0000_0010]); // the header, then the NULL bitmap: the last column
    row.extend((-5_i64).to_le_bytes());
    row.extend([11, 0xe8, 0x07, 2, 29, 23, 59, 59, 0x3f, 0x42, 0x0f, 0]); // with microseconds
    row.extend([12, 1, 34, 0, 0, 0, 22, 59, 59, 0x3f, 0x42, 0x0f, 0]); // -34 days 22:59:59.999999
    row.extend(lenenc(&[b"abc"]));
    row.extend(1.5_f64.to_le_bytes());
    row.extend([4, 0xe8, 0x07, 2, 29]);
    row.extend([0xfc, 3, 0, b'x', b'y', b'z']);
    let decode_row = |input: &[u8]| {
        let (columns, row) = columns_and_row(input);
        if let Ok(BinaryRow(row)) = BinaryRow::decode_with(row, &columns) {
            decode_values(&row, &columns, MySqlValueFormat::Binary);
        }
    };
    fuzz(6, ITERATIONS, &[row], decode_row);

    // Every length a date or time can claim, which mutations seldom hit, each of a valid date and
    // time so that the bytes after them are read too.
    let moment = [0xe8, 0x07, 2, 29, 23, 59, 59, 0x3f, 0x42, 0x0f, 0];
    let mut rows = Vec::new();
    for ty in [
        ColumnType::Date,
        ColumnType::Time,
        ColumnType::Datetime,
        ColumnType::Timestamp,
    ] {
        for len in 0..=u8::MAX {
            let mut row = vec![1, spec_of(ty), 0, 0, len];
            row.extend(moment.iter().chain(std::iter::repeat(&0)).take(len.into()));
            rows.push(row);
        }
    }
    fuzz(7, 0, &rows, decode_row);
}

#[test]
fn the_samples_decode() {
    for handshake in HANDSHAKES {
        Handshake::decode(Bytes::from_static(handshake)).unwrap();
    }
    for ok in OKS {
        OkPacket::decode(Bytes::from_static(ok)).unwrap();
    }
}
