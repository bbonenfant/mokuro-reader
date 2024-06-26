pub use cursor::{CursorAction, use_cursor};
pub use page::{PageAction, use_page_reducer};
pub use volume::{use_volume_reducer, VolumeAction};

mod page {
    use std::rc::Rc;

    use rexie::Rexie;
    use wasm_bindgen::JsValue;
    use yew::{AttrValue, hook, Reducible, use_mut_ref, use_reducer, UseReducerHandle};
    use yew::suspense::{SuspensionResult, use_future_with};

    use crate::errors::AppError;
    use crate::models::{OcrBlock, PageImage, PageOcr};
    use crate::utils::db::{get_page_and_ocr, put_ocr};

    /// A hook which returns a reducer wrapping the state of PageOcr for
    /// the requested (volume_id, page_name). When the PageOcr is updated,
    /// the corresponding entry is updated within the IndexedDB store.
    #[hook]
    pub fn use_page_reducer(
        db: Rc<Rexie>, volume_id: u32, page_name: AttrValue,
    ) -> SuspensionResult<UseReducerHandle<PageState>> {
        let sentinel = use_mut_ref(|| false);  // whether to update DB.
        let reducer = use_reducer(PageState::default);
        let signal = reducer.dispatcher();

        // First call of hook will retrieve the page and ocr from the DB.
        // This will then trigger a rerender, and this hook will be called again.
        // This second call will register the dependency with the use_future_with
        // hook. With the registered dependency, the ocr DB entry will be updated
        // whenever the reducer's state changes.
        use_future_with((page_name.clone(), reducer.clone()), |deps| async move {
            let (name, r) = &*deps;
            let key = JsValue::from(js_sys::Array::of2(&volume_id.into(), &name.as_str().into()));
            gloo_console::log!("use_page_reducer", name.as_str());
            if r.check_key(volume_id, name) {
                if *sentinel.borrow() {
                    gloo_console::log!(" - put_ocr", name.as_str());
                    put_ocr(&db, &r.ocr, &key).await.unwrap_or_else(|error| {
                        if let AppError::RexieError(err) = error {
                            gloo_console::error!(JsValue::from(err));
                        }
                    });
                } else {
                    gloo_console::log!(" - noop", name.as_str());
                    *sentinel.borrow_mut() = true;
                }
            } else {
                gloo_console::log!(" - get_page_and_ocr", name.as_str());
                let (image, ocr) = get_page_and_ocr(&db, &key).await?;
                *sentinel.borrow_mut() = false;
                signal.dispatch(PageAction::Set((volume_id, name.clone(), image, ocr)))
            }
            Ok::<(), AppError>(())
        })?.as_ref().expect("failed to get/update volume in IndexedDB");
        Ok(reducer)
    }

    #[derive(Default)]
    pub struct PageState {
        _object: Option<gloo_file::ObjectUrl>,
        _key: (u32, AttrValue),
        pub ocr: PageOcr,
        pub url: AttrValue,
    }

    impl PageState {
        fn check_key(&self, volume_id: u32, page_name: &AttrValue) -> bool {
            self._key.0 == volume_id && self._key.1 == *page_name
        }
    }

    impl PartialEq for PageState {
        fn eq(&self, other: &Self) -> bool {
            self.ocr == other.ocr
        }
    }

    pub enum PageAction {
        Set((u32, AttrValue, PageImage, PageOcr)),
        UpdateBlock(OcrBlock),
    }

    impl Reducible for PageState {
        type Action = PageAction;

        fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
            match action {
                Self::Action::Set((id, name, image, ocr)) => {
                    let object = gloo_file::ObjectUrl::from(image);
                    let url = AttrValue::from(object.to_string());
                    Self { _object: Some(object), _key: (id, name), ocr, url }
                }
                Self::Action::UpdateBlock(block) => {
                    gloo_console::log!("updating block");
                    let mut ocr = self.ocr.clone();
                    let index =
                        ocr.blocks.iter().position(|b| b.uuid == block.uuid).unwrap();
                    ocr.blocks[index] = block;
                    Self {
                        _object: self._object.clone(),
                        _key: self._key.clone(),
                        ocr,
                        url: self.url.clone(),
                    }
                }
            }.into()
        }
    }
}

mod volume {
    use std::rc::Rc;

    use rexie::Rexie;
    use yew::{hook, Reducible, use_mut_ref, use_reducer, UseReducerHandle};
    use yew::suspense::{SuspensionResult, use_future_with};

    use crate::errors::AppError;
    use crate::models::VolumeMetadata;
    use crate::utils::db::{get_volume, put_volume};

    /// A hook which returns a reducer wrapping the state of VolumeMetadata
    /// for the requested volume_id. When the VolumeMetadata is updated,
    /// the corresponding entry is updated within the IndexedDB store.
    #[hook]
    pub fn use_volume_reducer(
        db: Rc<Rexie>, volume_id: u32,
    ) -> SuspensionResult<UseReducerHandle<VolumeState>> {
        let sentinel = use_mut_ref(|| false);  // whether to update DB.
        let reducer = use_reducer(VolumeState::default);
        let signal = reducer.dispatcher();

        // First call of hook will retrieve the volume from the DB.
        // This will then trigger a rerender, and this hook will be called again.
        // This second call will register the dependency with the use_future_with
        // hook. With the registered dependency, the DB entry will be updated
        // whenever the reducer's state changes.
        use_future_with(reducer.data.clone(), |r| async move {
            if r.id == Some(volume_id) {
                if *sentinel.borrow() {
                    put_volume(&db, &r).await?;
                } else {
                    *sentinel.borrow_mut() = true;
                }
            } else {
                let volume = get_volume(&db, volume_id).await?;
                *sentinel.borrow_mut() = false;
                signal.dispatch(VolumeAction::Set(volume.into()))
            }
            Ok::<(), AppError>(())
        })?.as_ref().expect("failed to get/update volume in IndexedDB");
        Ok(reducer)
    }

    #[derive(Default, PartialEq)]
    pub struct VolumeState {
        pub data: VolumeMetadata,
    }

    pub enum VolumeAction {
        Set(Box<VolumeMetadata>),
        NextPage,
        PrevPage,
    }

    impl Reducible for VolumeState {
        type Action = VolumeAction;

        fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
            let data = match action {
                Self::Action::Set(v) => *v,
                Self::Action::NextPage => {
                    let mut data = self.data.to_owned();
                    data.reader_state.forward();
                    data
                }
                Self::Action::PrevPage => {
                    let mut data = self.data.to_owned();
                    data.reader_state.backward();
                    data
                }
            };
            Self { data }.into()
        }
    }
}

mod cursor {
    use std::rc::Rc;

    use web_sys::MouseEvent;
    use yew::{hook, Reducible, use_reducer_eq, UseReducerDispatcher, UseReducerHandle};

    use crate::utils::timestamp;

    /// Helpful encapsulation of all the information and functionality
    /// that this app needs involving the Cursor.
    #[hook]
    pub fn use_cursor() -> (UseReducerHandle<Cursor>, UseReducerDispatcher<Cursor>) {
        let reducer = use_reducer_eq(Cursor::default);
        let dispatcher = reducer.dispatcher();
        (reducer, dispatcher)
    }

    #[derive(Default)]
    pub struct Cursor {
        pub magnify: bool,
        pub force: u64,
        pub position: (i32, i32),
    }

    impl PartialEq for Cursor {
        fn eq(&self, other: &Self) -> bool {
            self.magnify == other.magnify && self.force == other.force
        }
    }

    pub enum CursorAction {
        ForceRerender,
        Toggle,
        Update(MouseEvent),
    }

    impl Reducible for Cursor {
        type Action = CursorAction;
        fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
            match action {
                Self::Action::ForceRerender => Self {
                    magnify: self.magnify,
                    force: timestamp(),
                    position: self.position,
                },
                Self::Action::Toggle => Self {
                    magnify: !self.magnify,
                    force: self.force,
                    position: self.position,
                },
                Self::Action::Update(event) => {
                    let position = (event.page_x(), event.page_y());
                    let force = if self.magnify { timestamp() } else { self.force };
                    Self { magnify: self.magnify, force, position }
                }
            }.into()
        }
    }
}
