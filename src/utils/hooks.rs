use std::rc::Rc;

use rexie::Rexie;
use yew::{AttrValue, hook, use_mut_ref};
use yew::suspense::{SuspensionResult, use_future_with};

use crate::models::{PageImage, PageOcr};
use crate::utils::db::get_page_and_ocr;

/// Convenience hook for fetching the pair of PageImage and PageOcr from the
///   IndexedDB instance, and converts/stores the ObjectUrl for the PageImage.
/// The ObjectUrl needs to stored (prevented from dropping) to prevent the
///   URL from being revoked.
#[hook]
pub fn use_reader_page(db: &Rc<Rexie>, volume_id: u32, page_name: &AttrValue) -> SuspensionResult<(AttrValue, PageOcr)> {
    let state = use_mut_ref(UrlState::default);
    let key = js_sys::Array::of2(&volume_id.into(), &page_name.as_str().into());
    let future = use_future_with(
        (volume_id, page_name.clone()),
        |_| get_page_and_ocr(db.clone(), key.into()),
    )?;
    let (image, ocr) = future.as_ref().unwrap().clone();

    if state.borrow().is_empty() {
        *state.borrow_mut() = image.into();
    }
    let url = state.borrow().url.clone();
    Ok((url, ocr))
}


#[derive(Default)]
struct UrlState {
    object: Option<gloo_file::ObjectUrl>,
    url: AttrValue,
}

impl UrlState {
    fn is_empty(&self) -> bool {
        self.object.is_none()
    }
}

impl From<PageImage> for UrlState {
    fn from(image: PageImage) -> UrlState {
        let object = gloo_file::ObjectUrl::from(image);
        let url = AttrValue::from(object.to_string());
        UrlState { object: Some(object), url }
    }
}
