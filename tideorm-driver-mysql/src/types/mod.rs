//! Conversions between Rust and **MySQL/MariaDB** types.
//!
//! # Types
//!
//! | Rust type                             | MySQL/MariaDB type(s)                                |
//! |---------------------------------------|------------------------------------------------------|
//! | `bool`                                | TINYINT(1), BOOLEAN, BOOL (see below)                |
//! | `i8`                                  | TINYINT                                              |
//! | `i16`                                 | SMALLINT                                             |
//! | `i32`                                 | INT                                                  |
//! | `i64`                                 | BIGINT                                               |
//! | `u8`                                  | TINYINT UNSIGNED                                     |
//! | `u16`                                 | SMALLINT UNSIGNED                                    |
//! | `u32`                                 | INT UNSIGNED                                         |
//! | `u64`                                 | BIGINT UNSIGNED                                      |
//!
//! Every integer type is read from any integer column whose value fits it,
//! signed or not.
//!
//! | `f32`                                 | FLOAT                                                |
//! | `f64`                                 | DOUBLE                                               |
//! | `&str`, [`String`]                    | VARCHAR, CHAR, TEXT                                  |
//! | `&[u8]`, `Vec<u8>`                    | VARBINARY, BINARY, BLOB                              |
//! | `IpAddr`                              | VARCHAR, TEXT                                        |
//! | `Ipv4Addr`                            | INET4 (MariaDB-only), VARCHAR, TEXT                  |
//! | `Ipv6Addr`                            | INET6 (MariaDB-only), VARCHAR, TEXT                  |
//! | [`MySqlTime`]                         | TIME (encode and decode full range)                  |
//! | [`Duration`][std::time::Duration]     | TIME (for decoding positive values only)             |
//!
//! ##### Note: `BOOLEAN`/`BOOL` Type
//! MySQL and MariaDB treat `BOOLEAN` as an alias of the `TINYINT` type:
//!
//! * [Using Data Types from Other Database Engines (MySQL)](https://dev.mysql.com/doc/refman/8.0/en/other-vendor-data-types.html)
//! * [BOOLEAN (MariaDB)](https://mariadb.com/kb/en/boolean/)
//!
//! You can use the Rust type `bool` when encoding or decoding a value with the dynamic
//! query interface. Because the MySQL wire protocol does not distinguish between
//! `TINYINT` and `BOOLEAN`, use `i8` instead when a `TINYINT` column is numeric.
//!
//! ### NOTE: MySQL's `TIME` type is signed
//! MySQL's `TIME` type can be used as either a time-of-day value, or a signed interval.
//! Thus, it may take on negative values.
//!
//! Decoding a [`std::time::Duration`] returns an error if the `TIME` value is negative.
//!
//! ### [`chrono`](https://crates.io/crates/chrono)
//!
//! | Rust type                             | MySQL/MariaDB type(s)                                |
//! |---------------------------------------|------------------------------------------------------|
//! | `chrono::DateTime<Utc>`               | DATETIME (as UTC; reads TIMESTAMP too)               |
//! | `chrono::DateTime<Local>`             | DATETIME (as UTC; reads TIMESTAMP too)               |
//! | `chrono::DateTime<FixedOffset>`       | DATETIME (as UTC; reads TIMESTAMP too)               |
//! | `chrono::NaiveDateTime`               | DATETIME                                             |
//! | `chrono::NaiveDate`                   | DATE                                                 |
//! | `chrono::NaiveTime`                   | TIME (time-of-day only)                              |
//! | `chrono::TimeDelta`                   | TIME (decodes full range; see note for encoding)     |
//!
//! ### NOTE: MySQL's `TIME` type is dual-purpose
//! MySQL's `TIME` type can be used as either a time-of-day value, or an interval.
//! However, `chrono::NaiveTime` is designed only to represent a time-of-day.
//!
//! Decoding a `TIME` value as `chrono::NaiveTime` will return an error if the value is out of range.
//!
//! The [`MySqlTime`] type supports the full range and it also implements `TryInto<chrono::NaiveTime>`.
//!
//! Decoding a `chrono::TimeDelta` also supports the full range.
//!
//! To encode a `chrono::TimeDelta`, convert it to [`MySqlTime`] first using `TryFrom`/`TryInto`.
//!
//! ### [`decimal`](https://crates.io/crates/rust_decimal)
//! | Rust type                             | MySQL/MariaDB type(s)                                |
//! |---------------------------------------|------------------------------------------------------|
//! | `rust_decimal::Decimal`               | DECIMAL                                              |
//!
//! ### [`uuid`](https://crates.io/crates/uuid)
//!
//! | Rust type                             | MySQL/MariaDB type(s)                                |
//! |---------------------------------------|------------------------------------------------------|
//! | `uuid::Uuid`                          | BINARY(16) (see note)                                |
//! | `uuid::fmt::Hyphenated`               | CHAR(36), VARCHAR, TEXT, UUID (MariaDB-only)         |
//! | `uuid::fmt::Simple`                   | CHAR(32), VARCHAR, TEXT                              |
//!
//! #### Note: `Uuid` uses binary format
//!
//! MySQL does not have a native datatype for UUIDs.
//! The `UUID()` function returns a 36-character `TEXT` value,
//! which encourages storing UUIDs as text.
//!
//! MariaDB's `UUID` type stores and retrieves as text, though it has a better representation
//! for index sorting (see [MariaDB manual: UUID data-type][mariadb-uuid] for details).
//!
//! This driver maps `uuid::Uuid` to and from binary format by default (16 bytes, the raw
//! value of a UUID; SQL type `BINARY(16)`). This saves 20 bytes over the text format.
//!
//! The `impl Decode<MySql> for Uuid` does not support the text format, and will return an error.
//!
//! If you want to use the text format compatible with the `UUID()` function,
//! use [`uuid::fmt::Hyphenated`][::uuid::fmt::Hyphenated] in the place of `Uuid`.
//!
//! The MySQL official blog has an article showing how to support both binary and text format UUIDs
//! by storing the binary and adding a generated column for the text format, though this is rather
//! verbose and fiddly: <https://dev.mysql.com/blog-archive/storing-uuid-values-in-mysql-tables/>
//!
//! [mariadb-uuid]: https://mariadb.com/kb/en/uuid-data-type/
//!
//! ### [`json`](https://crates.io/crates/serde_json)
//!
//! | Rust type                             | MySQL/MariaDB type(s)                                |
//! |---------------------------------------|------------------------------------------------------|
//! | [`Json<T>`]                           | JSON                                                 |
//! | `serde_json::JsonValue`               | JSON                                                 |
//! | `&serde_json::value::RawValue`        | JSON                                                 |
//!
//! # Nullable
//!
//! In addition, `Option<T>` is supported where `T` implements `Type`. An `Option<T>` represents
//! a potentially `NULL` value from MySQL/MariaDB.

pub(crate) use driver_core::types::*;

pub use mysql_time::{MySqlTime, MySqlTimeError, MySqlTimeSign};

mod bool;
mod bytes;
mod float;
mod int;
mod mysql_time;
mod str;
mod text;
mod uint;

mod json;

mod chrono;

mod uuid;

mod decoded;
pub use decoded::MySqlDecodedValue;
