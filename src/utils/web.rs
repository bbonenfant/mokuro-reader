use wasm_bindgen::UnwrapThrowExt;

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

pub fn get_screen_size() -> (u32, u32) {
    let window = web_sys::window().unwrap();
    let width = window.inner_width().unwrap().as_f64().unwrap();
    let height = window.inner_height().unwrap().as_f64().unwrap();
    (width as u32, height as u32)
}

/// Convenience function to avoid repeating expect logic.
#[inline(always)]
pub fn window() -> web_sys::Window {
    web_sys::window().expect_throw("Can't find the global Window")
}

/// Try to get selected text within the html document.
pub fn get_selected_text() -> Option<String> {
    window().get_selection().ok().flatten()
        .and_then(|s| s.to_string().as_string())
}
