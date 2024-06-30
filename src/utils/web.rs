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

pub fn get_screen_size() -> (f64, f64) {
    let window = web_sys::window().unwrap();
    let width = window.inner_width().unwrap().as_f64().unwrap();
    let height = window.inner_height().unwrap().as_f64().unwrap();
    (width, height)
}

/// Convenience function to avoid repeating expect logic.
#[inline(always)]
pub fn window() -> web_sys::Window {
    web_sys::window().expect_throw("Can't find the global Window")
}

/// Try to get selected text within the html document.
pub fn get_selection() -> Option<web_sys::Selection> {
    window().get_selection().ok().flatten()
}

#[inline(always)]
pub fn focus(node: &yew::NodeRef) -> bool {
    node.cast::<web_sys::HtmlElement>()
        .expect_throw("Could not resolve node reference")
        .focus().is_ok()
}

#[allow(dead_code)]
pub fn is_focused(node: &yew::NodeRef) -> bool {
    let document = window().document()
        .expect_throw("Can't find the global Document");
    let element = node.cast::<web_sys::Element>()
        .expect_throw("Could not resolve node reference");
    document.active_element().is_some_and(|elm| elm == element)
}
