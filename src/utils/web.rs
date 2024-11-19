use wasm_bindgen::UnwrapThrowExt;

/// Convenience functions to avoid repeating expect logic.
#[inline(always)]
pub fn window() -> web_sys::Window {
    web_sys::window().expect_throw("Can't find the global Window")
}

#[inline(always)]
pub fn document() -> web_sys::Document {
    window().document().expect_throw("Can't find the global Document")
}


pub async fn is_web_storage_persisted() -> Result<bool, wasm_bindgen::JsValue> {
    let promise = window().navigator().storage().persisted()?;
    let result = wasm_bindgen_futures::JsFuture::from(promise).await?;
    Ok(result.as_bool().unwrap_throw())
}

/// This method only functions as expected for HTTPS sites.
/// ref: developer.mozilla.org/docs/Web/API/StorageManager/persist
pub async fn ask_to_persist_storage() -> Result<bool, wasm_bindgen::JsValue> {
    let promise = window().navigator().storage().persist()?;
    let result = wasm_bindgen_futures::JsFuture::from(promise).await?;
    Ok(result.as_bool().unwrap_throw())
}

pub fn get_screen_size() -> (f64, f64) {
    let window = window();
    let width = window.inner_width().unwrap().as_f64().unwrap();
    let height = window.inner_height().unwrap().as_f64().unwrap();
    (width, height)
}

pub fn get_bounding_rect(node: &yew::NodeRef) -> web_sys::DomRect {
    let element = node.cast::<web_sys::Element>()
        .expect_throw("could not resolve node reference");
    element.get_bounding_client_rect()
}


/// Try to get selected text within the html document.
pub fn get_selection() -> Option<web_sys::Selection> {
    window().get_selection().ok().flatten()
}

/// Attempts to set the caret (text cursor) at the start of the
/// contenteditable element.
pub fn set_caret(node: &yew::NodeRef) {
    let element = node.cast::<web_sys::HtmlElement>()
        .expect_throw("Could not resolve node reference");

    let range = document().create_range().unwrap_throw();
    element.child_nodes().get(0).map(|child| {
        range.set_start(&child, 0).unwrap_throw();
        range.collapse_with_to_start(true);

        window().get_selection().unwrap_throw().map(|selection| {
            selection.remove_all_ranges().unwrap_throw();
            selection.add_range(&range).unwrap_throw();
        })
    });
}

#[inline(always)]
pub fn focus(node: &yew::NodeRef) -> bool {
    node.cast::<web_sys::HtmlElement>()
        .expect_throw("Could not resolve node reference")
        .focus().is_ok()
}

pub fn focused_element() -> Option<web_sys::Element> {
    document().active_element()
}

#[allow(dead_code)]
pub fn is_focused(node: &yew::NodeRef) -> bool {
    let element = node.cast::<web_sys::Element>()
        .expect_throw("Could not resolve node reference");
    focused_element().is_some_and(|elm| elm == element)
}

pub fn get_input_bool(node: &yew::NodeRef) -> Option<bool> {
    node.cast::<web_sys::HtmlInputElement>()
        .and_then(|elm| Some(elm.checked()))
}

pub fn get_input_f64(node: &yew::NodeRef) -> Option<f64> {
    node.cast::<web_sys::HtmlInputElement>()
        .and_then(|elm| elm.check_validity().then_some(elm.value_as_number()))
}

pub fn get_input_u16(node: &yew::NodeRef) -> Option<u16> {
    node.cast::<web_sys::HtmlInputElement>()
        .and_then(|elm| elm.check_validity().then_some(elm.value_as_number() as u16))
}

pub fn get_input_u8(node: &yew::NodeRef) -> Option<u8> {
    node.cast::<web_sys::HtmlInputElement>()
        .and_then(|elm| elm.check_validity().then_some(elm.value_as_number() as u8))
}
