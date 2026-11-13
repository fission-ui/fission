//! Browser regression fixture; see tests/web/index.html and the crate README.
#[cfg(target_arch = "wasm32")]
mod browser {
    use fission_store::SqlErrorKind;
    use fission_store_sqlite::{WebSqliteStore, WebSqliteTransactionOptions};
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
}
