pub async fn is_web_storage_persisted() -> Result<bool, wasm_bindgen::JsValue> {
    let promise = web_sys::window().unwrap().navigator().storage().persisted()?;
    let result = wasm_bindgen_futures::JsFuture::from(promise).await?;
    Ok(result.as_bool().unwrap())
}

pub async fn ask_to_persist_storage() -> Result<bool, wasm_bindgen::JsValue> {
    let promise = web_sys::window().unwrap().navigator().storage().persist()?;
    let result = wasm_bindgen_futures::JsFuture::from(promise).await?;
    Ok(result.as_bool().unwrap())
}