//! Browser regression fixture; see tests/web/index.html and the crate README.
#[cfg(target_arch = "wasm32")]
mod browser {
    use fission_store::{SqlErrorKind, SqlQuery, SqlRows, SqlStatement, SqlStoreProvider};
    use fission_store_sqlite::{
        WebSqliteImport, WebSqliteSnapshot, WebSqliteStagedDatabase, WebSqliteStore,
        WebSqliteTransactionOptions, MAX_WEB_SQLITE_STREAM_CHUNK_BYTES,
    };
    use std::future::{poll_fn, Future};
    use std::task::Poll;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen_futures::JsFuture;

    async fn browser_turn() -> Result<(), JsValue> {
        let promise = js_sys::Promise::new(&mut |resolve, reject| {
            let scheduled = web_sys::window()
                .ok_or_else(|| JsValue::from_str("browser window is unavailable"))
                .and_then(|window| {
                    window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 10)
                });
            if let Err(error) = scheduled {
                let _ = reject.call1(&JsValue::UNDEFINED, &error);
            }
        });
        JsFuture::from(promise).await.map(|_| ())
    }

    fn failure(error: impl std::fmt::Display) -> JsValue {
        JsValue::from_str(&error.to_string())
    }

    async fn read_label(staged: &WebSqliteStagedDatabase, expected: &str) -> Result<(), JsValue> {
        let rows = staged
            .query(SqlStatement::new("SELECT label FROM snapshot_fixture WHERE id=1").into())
            .await
            .map_err(failure)?;
        let label: String = rows
            .rows
            .first()
            .ok_or_else(|| JsValue::from_str("snapshot row is missing"))?
            .get("label")
            .map_err(failure)?;
        if label != expected {
            return Err(JsValue::from_str(&format!(
                "snapshot label was {label}, expected {expected}"
            )));
        }
        Ok(())
    }

    async fn check_no_attached_alias(store: &WebSqliteStore) -> Result<(), JsValue> {
        let rows = store
            .query(SqlStatement::new("PRAGMA database_list").into())
            .await
            .map_err(failure)?;
        for row in rows.rows {
            let name: String = row.get("name").map_err(failure)?;
            if name == "incoming" {
                return Err(JsValue::from_str("staged database remained attached"));
            }
        }
        Ok(())
    }

    async fn stream_snapshot(
        snapshot: &mut WebSqliteSnapshot,
        import: &mut WebSqliteImport,
        mut copied: u64,
    ) -> Result<u64, JsValue> {
        let expected_bytes = snapshot.byte_len();
        while let Some(chunk) = snapshot
            .read_chunk(MAX_WEB_SQLITE_STREAM_CHUNK_BYTES)
            .await
            .map_err(failure)?
        {
            if chunk.is_empty() || chunk.len() > MAX_WEB_SQLITE_STREAM_CHUNK_BYTES {
                return Err(JsValue::from_str("snapshot chunk exceeded its bound"));
            }
            import.write_chunk(&chunk).await.map_err(failure)?;
            copied += chunk.len() as u64;
        }
        if copied != expected_bytes {
            return Err(JsValue::from_str(&format!(
                "snapshot transferred {copied} bytes, expected {expected_bytes}"
            )));
        }
        Ok(copied)
    }

    async fn create_stage_from_snapshot(
        source: &WebSqliteStore,
        target: &WebSqliteStore,
    ) -> Result<WebSqliteStagedDatabase, JsValue> {
        let mut snapshot = source.begin_snapshot().await.map_err(failure)?;
        let mut import = target.begin_import().await.map_err(failure)?;
        stream_snapshot(&mut snapshot, &mut import, 0).await?;
        import.finish_stage().await.map_err(failure)
    }

    #[wasm_bindgen]
    pub async fn check_cancelled_begin() -> Result<(), JsValue> {
        let store = WebSqliteStore::open_database("cancelled-begin-regression").map_err(failure)?;
        let mut warm = store
            .begin_transaction(WebSqliteTransactionOptions::default())
            .await
            .map_err(failure)?;
        warm.rollback().await.map_err(failure)?;

        for _ in 0..5 {
            let mut begin =
                Box::pin(store.begin_transaction(WebSqliteTransactionOptions::default()));
            // Start the real JS request, then drop the Rust future before it
            // receives a transaction handle. No manually fabricated rollback.
            poll_fn(|cx| match begin.as_mut().poll(cx) {
                Poll::Pending => Poll::Ready(Ok(())),
                Poll::Ready(_) => Poll::Ready(Err(JsValue::from_str(
                    "BEGIN unexpectedly completed without yielding to the browser",
                ))),
            })
            .await?;
            drop(begin);
            let deadline = js_sys::Date::now() + 10_000.0;
            loop {
                browser_turn().await?;
                match store
                    .begin_transaction(WebSqliteTransactionOptions::default())
                    .await
                {
                    Ok(mut transaction) => {
                        transaction.rollback().await.map_err(failure)?;
                        break;
                    }
                    Err(error)
                        if error.kind == SqlErrorKind::Busy && js_sys::Date::now() < deadline => {}
                    Err(error) => return Err(failure(error)),
                }
            }
        }
        store.close().map_err(failure)
    }

    #[wasm_bindgen]
    pub async fn check_portable_snapshot() -> Result<(), JsValue> {
        let source = WebSqliteStore::open_database("portable-snapshot-source-regression")
            .map_err(failure)?;
        let target = WebSqliteStore::open_database("portable-snapshot-target-regression")
            .map_err(failure)?;
        source
            .execute(SqlStatement::new(
                "CREATE TABLE IF NOT EXISTS snapshot_fixture(id INTEGER PRIMARY KEY, label TEXT NOT NULL, payload BLOB NOT NULL)",
            ))
            .await
            .map_err(failure)?;
        target
            .execute(SqlStatement::new(
                "CREATE TABLE IF NOT EXISTS snapshot_fixture(id INTEGER PRIMARY KEY, label TEXT NOT NULL, payload BLOB NOT NULL)",
            ))
            .await
            .map_err(failure)?;
        source
            .execute(SqlStatement::new("DELETE FROM snapshot_fixture"))
            .await
            .map_err(failure)?;
        target
            .execute(SqlStatement::new("DELETE FROM snapshot_fixture"))
            .await
            .map_err(failure)?;

        let abandoned = source.begin_snapshot().await.map_err(failure)?;
        drop(abandoned);
        browser_turn().await?;
        let mut cancelled = source.begin_snapshot().await.map_err(failure)?;
        cancelled.cancel().await.map_err(failure)?;

        let mut invalid_import = target.begin_import().await.map_err(failure)?;
        match invalid_import.write_chunk(&[]).await {
            Err(error) if error.kind == SqlErrorKind::InvalidParameter => {}
            Err(error) => return Err(failure(error)),
            Ok(_) => return Err(JsValue::from_str("empty import chunk was accepted")),
        }
        invalid_import
            .write_chunk(b"not a SQLite database")
            .await
            .map_err(failure)?;
        if invalid_import.finish_stage().await.is_ok() {
            return Err(JsValue::from_str(
                "invalid SQLite image passed stage validation",
            ));
        }
        invalid_import.cancel().await.map_err(failure)?;

        let payload_len = 2 * 1024 * 1024 + 37;
        source
            .execute(
                SqlStatement::new(
                    "INSERT INTO snapshot_fixture(id,label,payload) VALUES(1,'before',zeroblob(?1))",
                )
                .bind(payload_len as i64),
            )
            .await
            .map_err(failure)?;

        let mut snapshot = source.begin_snapshot().await.map_err(failure)?;
        let manifest_rows = snapshot
            .query(SqlStatement::new("SELECT label,length(payload) AS payload_bytes FROM snapshot_fixture WHERE id=1").into())
            .await
            .map_err(failure)?;
        let manifest_label: String = manifest_rows
            .rows
            .first()
            .ok_or_else(|| JsValue::from_str("snapshot manifest row is missing"))?
            .get("label")
            .map_err(failure)?;
        let stored_payload_bytes: i64 = manifest_rows.rows[0]
            .get("payload_bytes")
            .map_err(failure)?;
        if manifest_label != "before" || stored_payload_bytes != payload_len as i64 {
            return Err(JsValue::from_str(
                "snapshot manifest did not match the seeded row",
            ));
        }

        let mut import = target.begin_import().await.map_err(failure)?;
        let first_chunk = snapshot
            .read_chunk(64 * 1024)
            .await
            .map_err(failure)?
            .ok_or_else(|| JsValue::from_str("snapshot was unexpectedly empty"))?;
        import.write_chunk(&first_chunk).await.map_err(failure)?;

        // Change the live database after export has started. The subsequent
        // chunks must still describe the immutable image queried above.
        let mut live_edit = source
            .begin_transaction(WebSqliteTransactionOptions::default())
            .await
            .map_err(failure)?;
        live_edit
            .execute(SqlStatement::new(
                "UPDATE snapshot_fixture SET label='after' WHERE id=1",
            ))
            .await
            .map_err(failure)?;
        live_edit.commit().await.map_err(failure)?;
        stream_snapshot(&mut snapshot, &mut import, first_chunk.len() as u64).await?;

        let staged = import.finish_stage().await.map_err(failure)?;
        read_label(&staged, "before").await?;
        match staged
            .query(SqlStatement::new("UPDATE snapshot_fixture SET label='invalid'").into())
            .await
        {
            Err(error) if error.kind == SqlErrorKind::ReadOnly => {}
            Err(error) => return Err(failure(error)),
            Ok(_) => return Err(JsValue::from_str("staged database accepted a write query")),
        }

        let mut copy = target
            .begin_transaction(WebSqliteTransactionOptions::default())
            .await
            .map_err(failure)?;
        copy.attach_staged_database(&staged, "incoming")
            .await
            .map_err(failure)?;
        copy.execute(SqlStatement::new(
            "INSERT INTO main.snapshot_fixture SELECT id,label,payload FROM incoming.snapshot_fixture",
        ))
        .await
        .map_err(failure)?;
        // Reading the stage locks it for this transaction. Explicit release must
        // not attempt DETACH early and roll back the rows we just copied.
        copy.detach_staged_database("incoming")
            .await
            .map_err(failure)?;
        copy.detach_staged_database("incoming")
            .await
            .map_err(failure)?;
        copy.commit().await.map_err(failure)?;
        check_no_attached_alias(&target).await?;
        read_label(&staged, "before").await?;

        let mut explicit_detach = target
            .begin_transaction(WebSqliteTransactionOptions::default())
            .await
            .map_err(failure)?;
        explicit_detach
            .attach_staged_database(&staged, "incoming")
            .await
            .map_err(failure)?;
        explicit_detach
            .detach_staged_database("incoming")
            .await
            .map_err(failure)?;
        explicit_detach.rollback().await.map_err(failure)?;
        check_no_attached_alias(&target).await?;

        let mut rollback_cleanup = target
            .begin_transaction(WebSqliteTransactionOptions::default())
            .await
            .map_err(failure)?;
        rollback_cleanup
            .attach_staged_database(&staged, "incoming")
            .await
            .map_err(failure)?;
        let mut staged_for_rollback = staged;
        staged_for_rollback.cancel().await.map_err(failure)?;
        rollback_cleanup.rollback().await.map_err(failure)?;
        check_no_attached_alias(&target).await?;

        let mut second = create_stage_from_snapshot(&source, &target).await?;
        let mut commit_cleanup = target
            .begin_transaction(WebSqliteTransactionOptions::default())
            .await
            .map_err(failure)?;
        commit_cleanup
            .attach_staged_database(&second, "incoming")
            .await
            .map_err(failure)?;
        second.cancel().await.map_err(failure)?;
        commit_cleanup.commit().await.map_err(failure)?;
        check_no_attached_alias(&target).await?;

        let fresh_target =
            WebSqliteStore::open_database("portable-snapshot-empty-install-regression")
                .map_err(failure)?;
        fresh_target
            .execute(SqlStatement::new(
                "DROP TABLE IF EXISTS install_race_fixture",
            ))
            .await
            .map_err(failure)?;
        fresh_target
            .execute(SqlStatement::new("DROP TABLE IF EXISTS snapshot_fixture"))
            .await
            .map_err(failure)?;
        let mut race_snapshot = source.begin_snapshot().await.map_err(failure)?;
        let mut race_import = fresh_target.begin_import().await.map_err(failure)?;
        let first_race_chunk = race_snapshot
            .read_chunk(64 * 1024)
            .await
            .map_err(failure)?
            .ok_or_else(|| JsValue::from_str("race snapshot was unexpectedly empty"))?;
        race_import
            .write_chunk(&first_race_chunk)
            .await
            .map_err(failure)?;

        // A second tab may create this named workspace while file transfer is
        // in progress. Empty-install must check again at install time.
        fresh_target
            .execute(SqlStatement::new(
                "CREATE TABLE install_race_fixture(id INTEGER PRIMARY KEY, value TEXT NOT NULL)",
            ))
            .await
            .map_err(failure)?;
        fresh_target
            .execute(SqlStatement::new(
                "INSERT INTO install_race_fixture(id,value) VALUES(1,'created during transfer')",
            ))
            .await
            .map_err(failure)?;
        stream_snapshot(
            &mut race_snapshot,
            &mut race_import,
            first_race_chunk.len() as u64,
        )
        .await?;
        let mut race_stage = race_import.finish_stage().await.map_err(failure)?;
        match race_stage.install_into_empty_target().await {
            Err(error) if error.kind == SqlErrorKind::Constraint => {}
            Err(error) => return Err(failure(error)),
            Ok(()) => {
                return Err(JsValue::from_str(
                    "empty-target install overwrote a workspace created during transfer",
                ))
            }
        }
        let race_rows = fresh_target
            .query(SqlStatement::new("SELECT value FROM install_race_fixture WHERE id=1").into())
            .await
            .map_err(failure)?;
        let race_value: String = race_rows
            .rows
            .first()
            .ok_or_else(|| JsValue::from_str("concurrent workspace write was lost"))?
            .get("value")
            .map_err(failure)?;
        if race_value != "created during transfer" {
            return Err(JsValue::from_str("concurrent workspace row changed"));
        }
        fresh_target
            .execute(SqlStatement::new("DROP TABLE install_race_fixture"))
            .await
            .map_err(failure)?;
        race_stage
            .install_into_empty_target()
            .await
            .map_err(failure)?;
        let installed_rows = fresh_target
            .query(SqlStatement::new("SELECT label FROM snapshot_fixture WHERE id=1").into())
            .await
            .map_err(failure)?;
        let installed_label: String = installed_rows
            .rows
            .first()
            .ok_or_else(|| JsValue::from_str("empty-target install did not install the snapshot"))?
            .get("label")
            .map_err(failure)?;
        if installed_label != "after" {
            return Err(JsValue::from_str(
                "empty-target install has an unexpected snapshot",
            ));
        }
        fresh_target.close().map_err(failure)?;
        let reopened_fresh =
            WebSqliteStore::open_database("portable-snapshot-empty-install-regression")
                .map_err(failure)?;
        let reopened_rows = reopened_fresh
            .query(SqlStatement::new("SELECT label FROM snapshot_fixture WHERE id=1").into())
            .await
            .map_err(failure)?;
        let reopened_label: String = reopened_rows
            .rows
            .first()
            .ok_or_else(|| JsValue::from_str("installed snapshot was lost after reopen"))?
            .get("label")
            .map_err(failure)?;
        if reopened_label != "after" {
            return Err(JsValue::from_str("reopened snapshot row changed"));
        }
        reopened_fresh.close().map_err(failure)?;

        let cancel_target =
            WebSqliteStore::open_database("portable-snapshot-cancel-install-regression")
                .map_err(failure)?;
        cancel_target
            .execute(SqlStatement::new("DROP TABLE IF EXISTS snapshot_fixture"))
            .await
            .map_err(failure)?;
        cancel_target
            .execute(SqlStatement::new("DELETE FROM fission"))
            .await
            .map_err(failure)?;
        let cancel_stage = create_stage_from_snapshot(&source, &cancel_target).await?;
        let mut pending_install = Box::pin(cancel_stage.install_into_empty_target());
        poll_fn(|cx| match pending_install.as_mut().poll(cx) {
            Poll::Pending => Poll::Ready(Ok(())),
            Poll::Ready(_) => Poll::Ready(Err(JsValue::from_str(
                "large staged install completed before cancellation could be tested",
            ))),
        })
        .await?;

        // A normal SQL request is rejected only after install has acquired the
        // worker maintenance lock and yielded from a backup step. Cancel then
        // exercises sqlite3_backup_finish before SQLITE_DONE.
        match cancel_target
            .query(SqlStatement::new("SELECT 1").into())
            .await
        {
            Err(error) if error.kind == SqlErrorKind::Busy => {}
            Err(error) => return Err(failure(error)),
            Ok(_) => {
                return Err(JsValue::from_str(
                    "staged install did not enter maintenance before the cancellation check",
                ))
            }
        }
        cancel_stage.cancel().await.map_err(failure)?;
        match pending_install.await {
            Err(error) if error.kind == SqlErrorKind::Unavailable => {}
            Err(error) => return Err(failure(error)),
            Ok(()) => {
                return Err(JsValue::from_str(
                    "cancelled install unexpectedly succeeded",
                ))
            }
        }

        let remaining = cancel_target
            .query(
                SqlStatement::new(
                    "SELECT (SELECT count(*) FROM main.sqlite_schema WHERE substr(name,1,7)<>'sqlite_' AND NOT(type='table' AND name='fission')) + (SELECT count(*) FROM main.fission) AS remaining",
                )
                .into(),
            )
            .await
            .map_err(failure)?;
        let remaining_count: i64 = remaining
            .rows
            .first()
            .ok_or_else(|| JsValue::from_str("cancelled install did not return target state"))?
            .get("remaining")
            .map_err(failure)?;
        if remaining_count != 0 {
            return Err(JsValue::from_str(
                "cancelled install left a partial database in its target",
            ));
        }

        let retry_stage = create_stage_from_snapshot(&source, &cancel_target).await?;
        retry_stage
            .install_into_empty_target()
            .await
            .map_err(failure)?;
        let retry_live_rows = cancel_target
            .query(SqlStatement::new("SELECT label FROM snapshot_fixture WHERE id=1").into())
            .await
            .map_err(failure)?;
        let retry_live_label: String = retry_live_rows
            .rows
            .first()
            .ok_or_else(|| JsValue::from_str("retry install did not install its snapshot"))?
            .get("label")
            .map_err(failure)?;
        if retry_live_label != "after" {
            return Err(JsValue::from_str(
                "retry install has an unexpected snapshot",
            ));
        }
        cancel_target.close().map_err(failure)?;
        let reopened_cancel_target =
            WebSqliteStore::open_database("portable-snapshot-cancel-install-regression")
                .map_err(failure)?;
        let retry_rows = reopened_cancel_target
            .query(SqlStatement::new("SELECT label FROM snapshot_fixture WHERE id=1").into())
            .await
            .map_err(failure)?;
        let retry_label: String = retry_rows
            .rows
            .first()
            .ok_or_else(|| JsValue::from_str("retry install did not persist its snapshot"))?
            .get("label")
            .map_err(failure)?;
        if retry_label != "after" {
            return Err(JsValue::from_str(
                "retry install persisted an unexpected snapshot",
            ));
        }
        reopened_cancel_target.close().map_err(failure)?;

        target.close().map_err(failure)?;
        let reopened = WebSqliteStore::open_database("portable-snapshot-target-regression")
            .map_err(failure)?;
        let rows: SqlRows = reopened
            .query(SqlQuery::from(SqlStatement::new(
                "SELECT label,length(payload) AS payload_bytes FROM snapshot_fixture WHERE id=1",
            )))
            .await
            .map_err(failure)?;
        let label: String = rows
            .rows
            .first()
            .ok_or_else(|| JsValue::from_str("copied database row is missing after reopen"))?
            .get("label")
            .map_err(failure)?;
        let copied_bytes: i64 = rows.rows[0].get("payload_bytes").map_err(failure)?;
        if label != "before" || copied_bytes != payload_len as i64 {
            return Err(JsValue::from_str(
                "copied row did not survive database reopen",
            ));
        }
        reopened.close().map_err(failure)?;
        source.close().map_err(failure)
    }
}
