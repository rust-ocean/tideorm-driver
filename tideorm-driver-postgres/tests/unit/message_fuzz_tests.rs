//! What a server, or anyone between it and the driver, can send: every backend message decoded
//! from arbitrary and corrupted bytes returns an error rather than panicking, and allocates no
//! more than those bytes justify.

use futures_util::FutureExt;

use super::*;
use crate::connection::PgStream;
use crate::error::DatabaseError;
use crate::test_support::{fuzz, mutate, Rng};
use crate::PgDatabaseError;

use BackendMessageFormat as F;

const ERROR: &[u8] = b"SERROR\0VERROR\0C42P01\0Mrelation \"t\" does not exist\0P15\0\
    Fparse_relation.c\0L1392\0Rparser_open_table\0\0";

/// A PL/pgSQL error: its position is into an internal query.
const INTERNAL_ERROR: &[u8] = b"SERROR\0VERROR\0C22012\0Mdivision by zero\0p8\0qSELECT 1/0\0\
    WPL/pgSQL function f() line 3\0\0";

fn auth(code: u32, data: &[u8]) -> Vec<u8> {
    [&code.to_be_bytes()[..], data].concat()
}

fn row_description() -> Vec<u8> {
    let mut body = vec![0, 2];
    for (name, table, column) in [(&b"id\0"[..], 16384_u32, 1_i16), (b"?column?\0", 0, 0)] {
        body.extend(name);
        body.extend(table.to_be_bytes());
        body.extend(column.to_be_bytes());
        body.extend(23_u32.to_be_bytes()); // INT4
        body.extend(4_i16.to_be_bytes());
        body.extend((-1_i32).to_be_bytes());
        body.extend(0_i16.to_be_bytes());
    }
    body
}

/// Valid bodies of each message the driver decodes.
fn samples() -> Vec<(BackendMessageFormat, Vec<Vec<u8>>)> {
    vec![
        (
            F::Authentication,
            vec![
                auth(0, b""),
                auth(3, b""),
                auth(5, b"salt"),
                auth(10, b"SCRAM-SHA-256\0SCRAM-SHA-256-PLUS\0\0"),
                auth(
                    11,
                    b"r=fyko+d2lbbFgONRv9qkxdawL3rfc,s=QSXCR+Q6sek8bf92,i=4096",
                ),
                auth(12, b"v=6rriTRBi23WpRR/wtup+mMhUZUn/dB5nLTJRsjl95G4="),
            ],
        ),
        (
            F::BackendKeyData,
            vec![vec![0, 0, 0x30, 0x39, 0xde, 0xad, 0xbe, 0xef]],
        ),
        (
            F::CommandComplete,
            vec![b"INSERT 0 5\0".to_vec(), b"CREATE TABLE\0".to_vec()],
        ),
        (F::CopyData, vec![b"1\tone\n".to_vec()]),
        (F::CopyDone, vec![Vec::new()]),
        (F::CopyInResponse, vec![vec![0, 0, 2, 0, 0, 0, 1]]),
        (F::CopyOutResponse, vec![vec![1, 0, 1, 0, 1]]),
        (
            F::DataRow,
            vec![[&[0, 3][..], &[0, 0, 0, 2], b"hi", &[0xff; 4], &[0; 4]].concat()],
        ),
        (
            F::ErrorResponse,
            vec![ERROR.to_vec(), INTERNAL_ERROR.to_vec()],
        ),
        (
            F::NoticeResponse,
            vec![b"SNOTICE\0VNOTICE\0C00000\0Mrelation exists, skipping\0\0".to_vec()],
        ),
        (
            F::NotificationResponse,
            vec![[&[0, 0, 0, 42][..], b"jobs\0", b"{\"id\":1}\0"].concat()],
        ),
        (
            F::ParameterDescription,
            vec![vec![0, 2, 0, 0, 0, 23, 0, 0, 0, 25]],
        ),
        (
            F::ParameterStatus,
            vec![b"server_version\x0016.2\0".to_vec()],
        ),
        (F::ParseComplete, vec![Vec::new()]),
        (
            F::ReadyForQuery,
            vec![b"I".to_vec(), b"T".to_vec(), b"E".to_vec()],
        ),
        (F::RowDescription, vec![row_description()]),
    ]
}

/// Decodes `body` as a `format` message and reads from it what the driver reads; whether it
/// decoded.
fn decode(format: &BackendMessageFormat, body: Bytes) -> bool {
    match format {
        F::Authentication => Authentication::decode_body(body)
            .map(|auth| {
                if let Authentication::Sasl(sasl) = auth {
                    let _ = sasl.mechanisms().count();
                }
            })
            .is_ok(),
        F::BackendKeyData => BackendKeyData::decode_body(body).is_ok(),
        F::CommandComplete => CommandComplete::decode_body(body)
            .map(|done| done.rows_affected())
            .is_ok(),
        F::CopyData => CopyData::<Bytes>::decode_body(body).is_ok(),
        F::CopyDone => CopyDone::decode_body(body).is_ok(),
        F::CopyInResponse => CopyInResponse::decode_body(body).is_ok(),
        F::CopyOutResponse => CopyOutResponse::decode_body(body).is_ok(),
        F::DataRow => DataRow::decode_body(body)
            .map(|row| (0..row.values.len()).for_each(|i| _ = row.get(i)))
            .is_ok(),
        F::ErrorResponse => PgDatabaseError::decode_body(body)
            .map(|error| {
                let _ = (error.kind(), error.position(), error.to_string());
                let _ = format!("{error:?}");
            })
            .is_ok(),
        F::NoticeResponse => Notice::decode_body(body)
            .map(|notice| {
                let _ = (notice.severity(), notice.code(), notice.message());
            })
            .is_ok(),
        F::NotificationResponse => Notification::decode_body(body).is_ok(),
        F::ParameterDescription => ParameterDescription::decode_body(body).is_ok(),
        F::ParameterStatus => ParameterStatus::decode_body(body).is_ok(),
        F::ParseComplete => ParseComplete::decode_body(body).is_ok(),
        F::ReadyForQuery => ReadyForQuery::decode_body(body).is_ok(),
        F::RowDescription => RowDescription::decode_body(body).is_ok(),
        // the driver reads only the format of these, which have no body
        F::BindComplete
        | F::CloseComplete
        | F::EmptyQueryResponse
        | F::NoData
        | F::PortalSuspended => true,
    }
}

fn frame(format: u8, body: &[u8]) -> Vec<u8> {
    let length = u32::try_from(body.len() + 4).unwrap();
    [&[format][..], &length.to_be_bytes(), body].concat()
}

/// Reads messages off `input` until it runs out or the framing fails, decoding each.
fn read_stream(input: &[u8]) {
    let mut stream = PgStream::scripted(input.to_vec());
    // a protocol error leaves the stream where it was, so it is not retried
    for _ in 0..64 {
        match stream.recv().now_or_never() {
            Some(Ok(message)) => _ = decode(&message.format, message.contents),
            Some(Err(Error::Database(_))) => {}
            Some(Err(_)) | None => break,
        }
    }
}

#[test]
fn the_samples_decode() {
    for (format, valid) in samples() {
        for body in valid {
            assert!(decode(&format, Bytes::from(body)), "{format:?}");
        }
    }
}

#[test]
fn every_message_body_decodes_or_fails_without_panicking() {
    for (seed, (format, valid)) in (1..).zip(samples()) {
        fuzz(seed, 4000, &valid, |body| {
            decode(&format, Bytes::copy_from_slice(body));
        });
    }
}

#[test]
fn a_stream_of_messages_is_framed_or_refused_without_panicking() {
    let messages: Vec<Vec<u8>> = samples()
        .into_iter()
        .flat_map(|(format, valid)| {
            let format = format as u8;
            valid.into_iter().map(move |body| frame(format, &body))
        })
        .collect();
    // a few messages in a row, so that a corrupted length misframes the ones after it
    let mut rng = Rng::new(99);
    let streams: Vec<Vec<u8>> = (0..64)
        .map(|_| (0..4).flat_map(|_| rng.pick(&messages).clone()).collect())
        .collect();
    fuzz(100, 6000, &streams, read_stream);
}

#[test]
fn a_forged_frame_length_reserves_nothing_absurd() {
    // `fuzz` bounds each input's largest allocation; these claim up to 4 GiB
    let mut rng = Rng::new(7);
    let mut inputs = Vec::new();
    for length in [u32::MAX, 0x8000_0000, 0x7fff_ffff, 1 << 30] {
        for format in *b"DEZ" {
            let mut input = frame(format, b"");
            input[1..5].copy_from_slice(&length.to_be_bytes());
            input.extend(mutate(&mut rng, b"\0\0\0\0"));
            inputs.push(input);
        }
    }
    fuzz(101, 0, &inputs, read_stream);
}
