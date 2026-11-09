# tideorm-driver

The database drivers TideORM's engine runs on, re-exported as
`tideorm::orm::driver`. A workspace of the front crate `tideorm-driver`
(lib `driver`, a feature per backend and runtime) and the crates behind it:

| Crate | Lib | What it holds |
| --- | --- | --- |
| `tideorm-driver-core` | `driver_core` | Pool, executor and the `Database`/`Row`/`Encode`/`Decode` traits |
| `tideorm-driver-postgres` | `driver_postgres` | PostgreSQL |
| `tideorm-driver-mysql` | `driver_mysql` | MySQL and MariaDB |
| `tideorm-driver-sqlite` | `driver_sqlite` | SQLite, bundled |

## Tests

Unit and doc tests need no server:

```bash
FEATURES=tideorm-driver/postgres,tideorm-driver/mysql,tideorm-driver/sqlite,tideorm-driver/tls-rustls
cargo +1.99.0 test --workspace --lib --features "$FEATURES,tideorm-driver/runtime-tokio"
cargo +1.99.0 test --workspace --lib --features "$FEATURES,tideorm-driver/runtime-async-std"
cargo +1.99.0 test --workspace --doc --features "$FEATURES,tideorm-driver/runtime-tokio"
```

TideORM's live suites exercise the drivers against real servers.
