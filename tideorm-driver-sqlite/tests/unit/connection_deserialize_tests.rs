use super::*;
use crate::Sqlite;
use driver_core::connection::Connection;
use driver_core::executor::Executor;
use driver_core::query::query;
use driver_core::row::Row;

async fn memory() -> SqliteConnection {
    SqliteConnection::connect("sqlite::memory:").await.unwrap()
}

fn copy(bytes: &[u8]) -> SqliteOwnedBuf {
    SqliteOwnedBuf::try_from(bytes).unwrap()
}

#[test]
fn a_database_survives_serialize_and_deserialize() {
    driver_core::rt::test_block_on(async {
        let mut source = memory().await;
        source
            .execute("CREATE TABLE t (v TEXT); INSERT INTO t VALUES ('kept')")
            .await
            .unwrap();
        let bytes = source.serialize(None).await.unwrap();
        assert!(bytes.starts_with(b"SQLite format 3\0"));
        assert_eq!(&copy(&bytes)[..], &bytes[..]);

        // The buffer SQLite made and a padded copy of it load alike; only a read-only one
        // refuses writes.
        let images = [(copy(&bytes), false), (copy(&bytes), true), (bytes, false)];
        for (data, read_only) in images {
            let mut target = memory().await;
            target.deserialize(None, data, read_only).await.unwrap();

            let row = query::<Sqlite>("SELECT v FROM t")
                .fetch_one(&mut target)
                .await
                .unwrap();
            assert_eq!(row.get::<String, _>(0), "kept");

            let write = target.execute("INSERT INTO t VALUES ('more')").await;
            assert_eq!(write.is_err(), read_only, "{write:?}");
        }
    });
}

#[test]
fn a_copy_is_followed_by_zeroed_padding() {
    let (ptr, size, capacity) = copy(b"image").into_raw();
    assert_eq!((size, capacity), (5, 5 + PADDING));

    // SAFETY: the copy allocated `capacity` initialized bytes, which `whole` now owns and
    // frees.
    let whole = unsafe { SqliteOwnedBuf::from_raw(ptr, capacity) }.unwrap();
    assert_eq!(&whole[..5], b"image");
    assert!(whole[5..].iter().all(|&byte| byte == 0));
}

/// SQLite could not open a WAL image once deserialized, so it is refused and the schema
/// keeps its database.
#[test]
fn a_wal_image_is_refused() {
    driver_core::rt::test_block_on(async {
        let mut conn = memory().await;
        conn.execute("CREATE TABLE t (v)").await.unwrap();

        for version_byte in [18, 19] {
            let mut header = [0_u8; 100];
            header[version_byte] = 2;
            match conn.deserialize(None, copy(&header), false).await {
                Err(Error::InvalidArgument(message)) => {
                    assert!(message.contains("WAL mode"), "{message}")
                }
                other => panic!("byte {version_byte}: {other:?}"),
            }
        }

        conn.execute("INSERT INTO t VALUES (1)").await.unwrap();
    });
}

#[test]
fn schemas_are_named_and_checked() {
    driver_core::rt::test_block_on(async {
        let mut conn = memory().await;
        conn.execute("ATTACH ':memory:' AS aux; CREATE TABLE aux.t (v)")
            .await
            .unwrap();
        let aux = conn.serialize(Some("aux")).await.unwrap();
        assert!(aux.starts_with(b"SQLite format 3\0"));

        let missing = conn.serialize(Some("missing")).await.unwrap_err();
        assert!(
            missing
                .to_string()
                .contains("database missing does not exist"),
            "{missing}"
        );

        for error in [
            conn.serialize(Some("a\0b")).await.unwrap_err(),
            conn.deserialize(Some("a\0b"), aux, false)
                .await
                .unwrap_err(),
        ] {
            match error {
                Error::InvalidArgument(message) => assert_eq!(
                    message,
                    r#"schema name "a\0b" contains a zero byte at index 1"#
                ),
                other => panic!("{other:?}"),
            }
        }
    });
}

#[test]
fn an_owned_buffer_cannot_be_empty() {
    match SqliteOwnedBuf::try_from(&[][..]) {
        Err(Error::InvalidArgument(message)) => {
            assert_eq!(message, "SQLite owned buffer cannot be empty")
        }
        other => panic!("{other:?}"),
    }
}
