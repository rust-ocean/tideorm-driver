//! Conversions between Rust and **SQLite** types.
//!
//! # Types
//!
//! | Rust type                             | SQLite type(s)                                       |
//! |---------------------------------------|------------------------------------------------------|
//! | `bool`                                | BOOLEAN                                              |
//! | `i8`                                  | INTEGER                                              |
//! | `i16`                                 | INTEGER                                              |
//! | `i32`                                 | INTEGER, INT4                                        |
//! | `i64`                                 | BIGINT, INT8                                         |
//! | `u8`                                  | INTEGER                                              |
//! | `u16`                                 | INTEGER                                              |
//! | `u32`                                 | INTEGER                                              |
//! | `u64`                                 | INTEGER (up to `i64::MAX`; see note)                 |
//! | `f32`                                 | REAL                                                 |
//! | `f64`                                 | REAL                                                 |
//! | `&str`, [`String`]                    | TEXT                                                 |
//! | `&[u8]`, `Vec<u8>`                    | BLOB                                                 |
//!
//! #### Note: Unsigned Integers
//! Decoding of unsigned integer types simply performs a checked conversion
//! to ensure that overflow does not occur.
//!
//! Encoding of the unsigned integer types `u8`, `u16` and `u32` is implemented by zero-extending to
//! the next-larger signed type. So `u8` becomes `i16`, `u16` becomes `i32`, and `u32` becomes `i64`
//! while still retaining their semantic values.
//!
//! SQLite stores integers in a variable-width encoding and always handles them in memory as 64-bit
//! signed values, so no space is wasted by this implicit widening.
//!
//! There is no larger type for `u64` in SQLite (it would need a 16-byte integer), so a `u64` is
//! encoded as an `i64` and one past `i64::MAX` is an encoding error. Bit-casting it to `i64`, or
//! storing it as `REAL`, `BLOB` or `TEXT`, would change what the value means in SQL.
//!
//! ### [`chrono`](https://crates.io/crates/chrono)
//!
//! | Rust type                             | Sqlite type(s)                                       |
//! |---------------------------------------|------------------------------------------------------|
//! | `chrono::NaiveDateTime`               | DATETIME (TEXT, INTEGER, REAL)                       |
//! | `chrono::DateTime<Utc>`               | DATETIME (TEXT, INTEGER, REAL)                       |
//! | `chrono::DateTime<Local>`             | DATETIME (TEXT, INTEGER, REAL)                       |
//! | `chrono::DateTime<FixedOffset>`       | DATETIME (TEXT, INTEGER, REAL)                       |
//! | `chrono::NaiveDate`                   | DATE (TEXT only)                                     |
//! | `chrono::NaiveTime`                   | TIME (TEXT only)                                     |
//!
//! ##### NOTE: `DATETIME` conversions
//! SQLite may represent `DATETIME` values as one of three types: `TEXT`, `REAL`, or `INTEGER`.
//! Which one is used is entirely up to you and how you store timestamps in your database.
//!
//! The deserialization for `NaiveDateTime`, `DateTime<Utc>` and `DateTime<Local>` infer the date
//! format from the type of the value they're being decoded from:
//!
//! * If `TEXT`, the format is assumed to be an ISO-8601 compatible datetime string.
//!   A number of possible formats are tried; see
//!   `tideorm-driver-sqlite/src/types/chrono.rs` for the current set of formats.
//! * If `INTEGER`, it is expected to be the number of seconds since January 1, 1970 00:00 UTC,
//!   as if returned from the `unixepoch()` function (without the `subsec` modifier).
//! * If `REAL`, it is expected to be the (possibly fractional) number of days since the Julian epoch,
//!   November 24, 4714 BCE 12:00 UTC, as if returned from the `julianday()` function.
//!
//! These types will always encode to a datetime string, either
//! with a timezone offset (`DateTime<Tz>` for any `Tz: TimeZone`) or without (`NaiveDateTime`).
//!
//! ##### NOTE: `CURRENT_TIMESTAMP` and comparison/interoperability of `DATETIME` values
//! As stated previously, `DateTime<Tz>` always encodes to a date-time string
//! _with_ a timezone offset,
//! in [RFC 3339 format][::chrono::DateTime::to_rfc3339_opts] (with `use_z: false`).
//!
//! However, most of SQLite's datetime functions
//! (including `datetime()` and `DEFAULT CURRENT_TIMESTAMP`)
//! do not use this format. They instead use `YYYY-MM-DD HH:MM:SS.SSSS` without a timezone offset.
//!
//! This may cause problems with interoperability with other applications, and especially
//! when comparing datetime values, which compares the actual string values lexicographically.
//!
//! Date-time strings in the SQLite format will generally _not_ compare consistently
//! with date-time strings in the RFC 3339 format.
//!
//! We recommend that you decide up-front whether `DATETIME` values should be stored
//! with explicit time zones or not, and use the corresponding type
//! (and its corresponding offset, if applicable) _consistently_ throughout your
//! application:
//!
//! * RFC 3339 format: `DateTime<Tz>` (e.g. `DateTime<Utc>`, `DateTime<Local>`, `DateTime<FixedOffset>`)
//!   * Changing or mixing and matching offsets may break comparisons with existing timestamps.
//!   * `DateTime<Local>` is **not recommended** for portable applications.
//!   * `DateTime<FixedOffset>` is only recommended if the offset is **constant**.
//! * SQLite format: `NaiveDateTime`
//!
//! Note that non-constant offsets may still cause issues when comparing timestamps,
//! as the comparison operators are not timezone-aware.
//!
//! ### [`uuid`](https://crates.io/crates/uuid)
//!
//! | Rust type                             | Sqlite type(s)                                       |
//! |---------------------------------------|------------------------------------------------------|
//! | `uuid::Uuid`                          | BLOB, TEXT                                           |
//! | `uuid::fmt::Hyphenated`               | TEXT                                                 |
//! | `uuid::fmt::Simple`                   | TEXT                                                 |
//!
//! ### [`json`](https://crates.io/crates/serde_json)
//!
//! | Rust type                             | Sqlite type(s)                                       |
//! |---------------------------------------|------------------------------------------------------|
//! | [`Json<T>`]                           | TEXT                                                 |
//! | `serde_json::JsonValue`               | TEXT                                                 |
//! | `&serde_json::value::RawValue`        | TEXT                                                 |
//!
//! # Nullable
//!
//! In addition, `Option<T>` is supported where `T` implements `Type`. An `Option<T>` represents
//! a potentially `NULL` value from SQLite.
//!

pub(crate) use driver_core::types::*;

mod chrono;
mod decoded;
mod int;
mod scalar;
mod uint;
mod uuid;

pub use decoded::SqliteDecodedValue;
