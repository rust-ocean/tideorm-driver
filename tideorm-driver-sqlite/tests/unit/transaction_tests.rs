use super::*;
use driver_core::connection::Connection;
use driver_core::executor::Executor;
use driver_core::row::Row;

async fn connection() -> SqliteConnection {
    let mut conn = SqliteConnection::connect("sqlite::memory:").await.unwrap();
    conn.execute("CREATE TABLE items(id INTEGER PRIMARY KEY)")
        .await
        .unwrap();
    conn
}

async fn fail(conn: &mut SqliteConnection) {
    conn.execute("INSERT INTO items VALUES (1)").await.unwrap();
    assert!(conn
        .execute("INSERT OR ROLLBACK INTO items VALUES (1)")
        .await
        .is_err());
    assert_eq!(conn.transaction_state(), TransactionState::Aborted);
}

#[test]
fn automatic_rollback_refuses_statements_and_commit_until_all_depths_rollback() {
    driver_core::rt::test_block_on(async {
        let mut conn = connection().await;
        for _ in 0..3 {
            SqliteTransactionManager::begin(&mut conn, None)
                .await
                .unwrap();
        }
        assert_eq!(conn.transaction_state(), TransactionState::Active);
        fail(&mut conn).await;
        for depth in (1..=3).rev() {
            assert_eq!(
                SqliteTransactionManager::get_transaction_depth(&conn),
                depth
            );
            let err = conn
                .execute("INSERT INTO items VALUES (2)")
                .await
                .unwrap_err();
            assert!(err
                .to_string()
                .contains("ended the transaction automatically"));
            assert!(SqliteTransactionManager::begin(&mut conn, None)
                .await
                .is_err());
            assert!(SqliteTransactionManager::commit(&mut conn)
                .await
                .unwrap_err()
                .to_string()
                .contains("ended the transaction automatically"));
            SqliteTransactionManager::rollback(&mut conn).await.unwrap();
        }
        let rows: i64 = driver_core::query::query("SELECT count(*) FROM items")
            .fetch_one(&mut conn)
            .await
            .unwrap()
            .get(0);
        assert_eq!(rows, 0);
        SqliteTransactionManager::begin(&mut conn, None)
            .await
            .unwrap();
        conn.execute("INSERT INTO items VALUES (3)").await.unwrap();
        SqliteTransactionManager::commit(&mut conn).await.unwrap();
    });
}

#[test]
fn ordinary_constraint_error_preserves_active_transaction() {
    driver_core::rt::test_block_on(async {
        let mut conn = connection().await;
        let mut tx = conn.begin().await.unwrap();
        tx.execute("INSERT INTO items VALUES (1)").await.unwrap();
        assert!(tx.execute("INSERT INTO items VALUES (1)").await.is_err());
        assert_eq!(tx.transaction_state(), TransactionState::Active);
        tx.execute("INSERT INTO items VALUES (2)").await.unwrap();
        tx.commit().await.unwrap();
    });
}

#[test]
fn dropped_savepoints_rebalance_after_automatic_rollback() {
    driver_core::rt::test_block_on(async {
        let mut conn = connection().await;
        let mut tx = conn.begin().await.unwrap();
        let mut savepoint = tx.begin().await.unwrap();
        fail(&mut savepoint).await;
        drop(savepoint);
        conn_unavailable_until_outer_rollback(&mut tx).await;
        tx.rollback().await.unwrap();
        conn.flush().await.unwrap();
        assert_eq!(SqliteTransactionManager::get_transaction_depth(&conn), 0);
        conn.execute("INSERT INTO items VALUES (4)").await.unwrap();
    });
}

async fn conn_unavailable_until_outer_rollback(tx: &mut SqliteConnection) {
    tx.flush().await.unwrap();
    assert_eq!(SqliteTransactionManager::get_transaction_depth(tx), 1);
    assert!(tx.execute("INSERT INTO items VALUES (2)").await.is_err());
}

#[test]
fn pool_reuses_session_after_dropped_automatically_rolled_back_transaction() {
    driver_core::rt::test_block_on(async {
        let pool = driver_core::pool::PoolOptions::<Sqlite>::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let mut conn = pool.acquire().await.unwrap();
        conn.execute("CREATE TEMP TABLE kept(id INTEGER PRIMARY KEY)")
            .await
            .unwrap();
        let mut tx = conn.begin().await.unwrap();
        tx.execute("INSERT INTO kept VALUES (1)").await.unwrap();
        assert!(tx
            .execute("INSERT OR ROLLBACK INTO kept VALUES (1)")
            .await
            .is_err());
        drop(tx);
        conn.return_to_pool().await;
        let mut reused = pool.acquire().await.unwrap();
        assert_eq!(reused.transaction_state(), TransactionState::Idle);
        reused.execute("INSERT INTO kept VALUES (2)").await.unwrap();
        // Explicit BEGIN is also unowned and must be rolled back on release.
        reused
            .execute("BEGIN; INSERT INTO kept VALUES (3)")
            .await
            .unwrap();
        reused.return_to_pool().await;
        let count: i64 = driver_core::query::query("SELECT count(*) FROM kept")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);
        assert_eq!(count, 1);
        pool.close().await;
    });
}

#[test]
fn pool_rolls_back_transactions_started_by_release_hooks() {
    driver_core::rt::test_block_on(async {
        let pool = driver_core::pool::PoolOptions::<Sqlite>::new()
            .max_connections(1)
            .after_release(|conn, _| {
                Box::pin(async move {
                    conn.execute("BEGIN; INSERT INTO kept VALUES (9)").await?;
                    Ok(true)
                })
            })
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let mut conn = pool.acquire().await.unwrap();
        conn.execute("CREATE TEMP TABLE kept(id INTEGER)")
            .await
            .unwrap();
        conn.return_to_pool().await;
        let mut reused = pool.acquire().await.unwrap();
        assert_eq!(reused.transaction_state(), TransactionState::Idle);
        let count: i64 = driver_core::query::query("SELECT count(*) FROM kept")
            .fetch_one(&mut *reused)
            .await
            .unwrap()
            .get(0);
        assert_eq!(count, 0);
        drop(reused);
        pool.close().await;
    });
}
