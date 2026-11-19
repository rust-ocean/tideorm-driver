# Changelog

All notable changes to the TideORM database drivers are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and the crates
adhere to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Each crate
(`tideorm-driver`, `tideorm-driver-core`, `-postgres`, `-mysql` and `-sqlite`) carries its own
version, so a heading names the crates that release published.

## [0.2.1] - 2026-11-19

tideorm-driver-core, tideorm-driver-postgres and tideorm-driver-sqlite.

### Changed

- tideorm-driver-core: `hashbrown` 0.17 and `hashlink` 0.12. The re-exported `HashMap` is
  `hashbrown` 0.17's.
- tideorm-driver-postgres: `atoi` 3 and `base64` 0.23.
- tideorm-driver-sqlite: accepts `libsqlite3-sys` up to 0.40, so a fresh build bundles
  SQLite 3.54.1 (0.37 bundled 3.51.3).

## [0.2.0] - 2026-11-18

All five crates.

### Breaking

- A column read by a name two result columns share is an error ("more than one result column is
  named `id`; give each an alias or read them by position") instead of the last of them. Reads by
  position are unchanged (core: `column_by_name`, `insert_column_name`).
- TLS trusts only a configured CA (`sslrootcert`, `ssl_ca`), not one more beside the public
  roots. `verify-ca` without one refuses to connect, as libpq does, and PostgreSQL's `require`
  with `sslrootcert` checks the chain. A refused handshake is `Error::Tls`, not an I/O error.
- Decimal text and PostgreSQL `NUMERIC` values a `Decimal` cannot hold exactly are errors, not
  rounded (core: `types::decimal_from_text`). An `f32` read past its range is an error, not
  infinity. A date past 9999 sent to MySQL or MariaDB is an error (MariaDB stored the zero date).
- SQLite connections turn off double-quoted string literals.
- PostgreSQL `08P01` is no longer `ConnectionClosed`; `57P05`, `25P03`, `25P04` and MySQL `4031`
  are.

### Fixed

- SCRAM leaves the user name to the startup message, so a role whose name has a `,` logs in, and
  it sends a password SASLprep refuses unprepared, as libpq does.
- MySQL and MariaDB accounts with an empty password log in.
- A PostgreSQL statement whose cached plan went stale (`0A000`) is dropped from the cache, so the
  next run prepares it again.
- An error in the reply of a dropped statement no longer fails the next statement (PostgreSQL,
  MySQL).
- Malformed fixed-width PostgreSQL values and short `DataRow`s are decode errors, not panics.
- PostgreSQL text timestamps with BC or five-digit years or with offsets in seconds, `TIMETZ`
  offsets in seconds, and SQLite Julian-day reals decode.
- SQLite `:memory:` and `file::memory:` with a shared cache are one database for the whole pool.
- Reading a MySQL packet allocates nothing, and `rust_decimal`'s `maths` feature is no longer
  enabled.

## 2026-11-16: core 0.1.4, postgres 0.1.5, mysql 0.1.7, sqlite 0.1.3

### Changed

- A connection with nothing pending and no transaction open goes back to the pool as it drops, so
  a checkout right after finds it idle. The backends implement core's `Connection::is_flushed`
  and require core 0.1.4.
- Reads land in the buffer's spare capacity: only the unread part of a message moves, once.
- MySQL reuses a prepared statement's column definitions while a result's match them, and no
  longer reserves memory for a column count before the definitions arrive.
- `futures-intrusive` is needed only with async-std.

### Fixed

- TiDB's write conflict (9007) is a serialization failure, so it is retryable.
- PostgreSQL drops a notification no listener takes instead of handing it to a statement.
- The SQLite worker numbers each command, so whether a connection is idle stays exact when a send
  is cancelled.

## 2026-11-15: tideorm-driver 0.1.1, postgres 0.1.4, mysql 0.1.5 and 0.1.6

### Added

- `PgConnection::server()` and `MySqlConnection::server()` (`PgServer`, `MySqlServer`) say which
  server answered, read as the connection opens: PostgreSQL, CockroachDB or YugabyteDB; MySQL,
  MariaDB, TiDB or Vitess.
- CockroachDB connections set `serial_normalization = sql_sequence`,
  `autocommit_before_ddl = off` and `create_table_with_schema_locked = off`.

### Fixed

- mysql 0.1.5: a `NaiveDateTime` reads a `TIMESTAMP` column, as `DateTime<Utc>` does.
- Vitess: unsigned integers past their signed range are bound as signed values (Vitess read a
  `u8` 255 as -1), its syntax errors are classified as such, and sessions leave out
  `PIPES_AS_CONCAT` (a Vitess reporting a plain MySQL version is recognized when it refuses it).
- tideorm-driver 0.1.1 requires these backend versions, so a lock cannot keep older ones.

## 2026-11-13: postgres 0.1.3, mysql 0.1.4

### Fixed

- MySQL: every connection's session setup raises `group_concat_max_len` to its maximum, so
  MariaDB's `JSON_ARRAYAGG` no longer fails or truncates past 1 MiB.
- PostgreSQL: one SQL text run with an `INT4` and then an `INT8` argument reused the statement
  parsed for the first type and failed with `22P03`. Such a run now parses an unnamed statement of
  its own and leaves the cache as it is.

## 2026-11-12: core 0.1.3, postgres 0.1.2, mysql 0.1.3, sqlite 0.1.2

### Changed

- PostgreSQL runs a statement's first execution in one round trip (parse, describe, bind and
  execute go out together), closes a statement evicted from the full cache in that same flight,
  and closes a cancelled prepare's statement with the next one.
- SQLite sends a large result's rows to the caller in batches of up to 64, and returns a finished
  connection to the pool without a round trip to its worker.
- The pool hands out the connection released last, keeps idle times across maintenance, and
  drops the `crossbeam-queue` dependency.
- MySQL and MariaDB take `NO_BACKSLASH_ESCAPES` out of the session's SQL mode, and refuse a
  character set, collation or time zone that is not a plain name.

### Fixed

- A dropped or refused PostgreSQL `COPY` no longer leaves the connection a reply behind.
- Empty binary `jsonb`, `ltree` and `lquery` values and odd `lquery` levels fail to decode instead
  of panicking.
- Options no longer show a password or client key in `Debug`.
- SQLite reads the new columns of a cached statement whose column count changed instead of
  crashing the worker.
- MySQL and MariaDB read the rest of a one-row reply before returning, so a later error fails
  that query.

## 2026-11-11: core 0.1.2, postgres 0.1.1, mysql 0.1.2

### Fixed

- An empty MySQL OK or auth-switch packet, or a binary `TIME` of the wrong length, is a protocol
  or decode error instead of a panic.
- A message length the server sends no longer reserves more than 1 MiB ahead of the bytes that
  arrived.
- PostgreSQL's server version is parsed from any reported form.

## 2026-11-10: core 0.1.1, mysql 0.1.1, sqlite 0.1.1

### Removed

- The unused `futures-io` (core takes `AsyncRead` from `futures_util::io`), `digest` (mysql takes
  `Digest` from `sha2`) and `form_urlencoded` (sqlite takes it from `url`) dependencies.

## [0.1.0] - 2026-11-10

First release of all five crates.
