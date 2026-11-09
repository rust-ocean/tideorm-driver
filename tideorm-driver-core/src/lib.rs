//! The database-independent core of TideORM's drivers: the `Database`,
//! `Connection` and `Executor` traits, the connection pool, transactions,
//! statements, the encoding and decoding of values, and the async runtime and
//! network layers every backend crate builds on.
//!
//! Its API serves the backend crates (`tideorm-driver-postgres`, `-mysql`,
//! `-sqlite`) and the front crate `tideorm-driver`; applications use the front
//! crate.
#![recursion_limit = "512"]
#![warn(future_incompatible, rust_2018_idioms)]
#![allow(clippy::needless_doctest_main, clippy::type_complexity)]
// The only unsafe code in the drivers is the SQLite backend's FFI, in its own crate.
#![forbid(unsafe_code)]

#[macro_use]
pub mod ext;

#[macro_use]
pub mod error;

#[macro_use]
pub mod arguments;

#[macro_use]
pub mod pool;

pub mod connection;

#[macro_use]
pub mod transaction;

#[macro_use]
pub mod encode;

#[macro_use]
pub mod decode;

#[macro_use]
pub mod types;

#[macro_use]
pub mod query;

#[macro_use]
pub mod acquire;

#[macro_use]
pub mod column;

#[macro_use]
pub mod statement;

pub mod common;
pub mod database;
pub mod executor;
pub mod fs;
pub mod io;
pub mod logger;
pub mod net;
pub mod sql_str;

pub mod row;
pub mod rt;
pub mod sync;
pub mod type_info;
pub mod value;

pub use error::{Error, Result};

pub use either::Either;
pub use hashbrown::HashMap;
pub use indexmap::IndexMap;
pub use percent_encoding;
pub use smallvec::SmallVec;
pub use url::{self, Url};

pub use bytes;

/// The modules and types each backend crate glob-imports at its root, so its code names
/// them as `crate::<module>`. Nothing here clashes with a module a backend defines itself.
pub mod driver_prelude {
    pub use crate::{
        acquire, common, decode, encode, executor, ext, fs, io, logger, net, pool, query, rt, sync,
    };

    pub use crate::error::{Error, Result};
    pub use crate::HashMap;
    pub use either::Either;
}
