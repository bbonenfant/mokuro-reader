use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

use rexie::Rexie;
use yew::{AttrValue, hook, Reducible, use_mut_ref, use_reducer, UseReducerHandle};
use yew::suspense::{SuspensionResult, use_future_with};

use crate::errors::AppError;
use crate::models::{PageImage, PageOcr, VolumeMetadata};
use crate::utils::db::{get_page_and_ocr, get_volume, put_volume};

/// Convenience hook for fetching the pair of PageImage and PageOcr from the
///   IndexedDB instance, and converts/stores the ObjectUrl for the PageImage.
/// The ObjectUrl needs to stored (prevented from dropping) to prevent the
///   URL from being revoked.
#[hook]
pub fn use_reader_page(db: &Rc<Rexie>, volume_id: u32, page_name: &AttrValue) -> SuspensionResult<(AttrValue, PageOcr)> {
    let state = use_mut_ref(UrlState::default);
    let key = js_sys::Array::of2(&volume_id.into(), &page_name.as_str().into());
    let future = {
        let db = db.clone();
        let state = state.clone();
        use_future_with((volume_id, page_name.clone()), |_| async move {
            let (image, ocr) = get_page_and_ocr(db, key.into()).await?;
            *state.borrow_mut() = image.into();
            Ok::<_, AppError>(ocr)
        })?
    };
    let ocr = future.as_ref().unwrap().clone();
    let url = state.borrow().url.clone();
    Ok((url, ocr))
}


#[derive(Default)]
struct UrlState {
    _object: Option<gloo_file::ObjectUrl>,
    url: AttrValue,
}

impl From<PageImage> for UrlState {
    fn from(image: PageImage) -> UrlState {
        let object = gloo_file::ObjectUrl::from(image);
        let url = AttrValue::from(object.to_string());
        UrlState { _object: Some(object), url }
    }
}


#[hook]
pub fn use_volume_reducer(
    db: Rc<Rexie>, volume_id: u32,
) -> SuspensionResult<UseReducerHandle<VolumeState>> {
    let reducer = use_reducer(VolumeState::default);
    use_future_with(reducer.clone(), |r| async move {
        if r.borrow().id == Some(volume_id) {
            put_volume(&db, &r.borrow()).await?;
        } else {
            let volume = get_volume(db, volume_id).await?;
            r.dispatch(VolumeAction::Set(volume.into()))
        }
        Ok::<(), AppError>(())
    })?.as_ref().expect("failed to get/update volume in IndexedDB");
    Ok(reducer)
}

pub enum VolumeAction {
    Set(Box<VolumeMetadata>),
    NextPage,
    PrevPage,

}

#[derive(Default, PartialEq)]
pub struct VolumeState {
    pub data: RefCell<VolumeMetadata>,
}

impl VolumeState {
    pub fn borrow(&self) -> Ref<VolumeMetadata> {
        self.data.borrow()
    }

    pub fn borrow_mut(&self) -> RefMut<VolumeMetadata> {
        self.data.borrow_mut()
    }
}

impl Reducible for VolumeState {
    type Action = VolumeAction;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        match action {
            Self::Action::Set(data) => {
                *self.data.borrow_mut() = *data;
            }
            Self::Action::NextPage => self.data.borrow_mut().reader_state.forward(),
            Self::Action::PrevPage => self.data.borrow_mut().reader_state.backward()
        }
        self
    }
}
