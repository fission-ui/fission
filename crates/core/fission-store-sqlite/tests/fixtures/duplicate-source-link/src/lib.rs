use wasm_bindgen::prelude::*;

/// Keeps each source identity's bridge call in the linked Wasm module.
#[wasm_bindgen]
pub fn close_both_sqlite_providers() -> Result<(), JsValue> {
    fission_store_sqlite_current::WebSqliteStore::new()
        .close()
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    fission_store_sqlite_previous::WebSqliteStore::new()
        .close()
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    Ok(())
}
