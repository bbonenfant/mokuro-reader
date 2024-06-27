pub mod ocr {
    use std::fmt::{Display, Formatter};
    use std::rc::Rc;

    use yew::{NodeRef, Reducible, UseReducerHandle};
    use yew::functional::{hook, use_node_ref, use_reducer_eq};

    use crate::utils::web::{focus, get_selection};

    #[hook]
    pub fn use_ocr_reducer(editable: bool) -> UseReducerHandle<OcrState> {
        let ref_ = use_node_ref();
        let reducer = use_reducer_eq(move || {
            OcrState { ref_, state: TextBlockState::from_editable(editable) }
        });
        // TODO: this causes double render.
        reducer.dispatch(OcrAction::Editable(editable));
        reducer
    }

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub enum TextBlockState {
        #[default]
        Default,
        Editable,
        EditableFocused,
        EditableFocusedContent,
    }

    impl TextBlockState {
        fn from_editable(editable: bool) -> Self {
            if editable { Self::Editable } else { Self::Default }
        }

        fn editable(&self) -> bool {
            matches!(self, Self::Editable | Self::EditableFocused | Self::EditableFocusedContent)
        }

        fn focus(self) -> Self {
            match self {
                Self::Default => Self::Default,
                Self::Editable | Self::EditableFocused => Self::EditableFocused,
                Self::EditableFocusedContent => Self::EditableFocusedContent,
            }
        }

        fn unfocus(self) -> Self {
            if self.editable() { Self::Editable } else { Self::Default }
        }

        fn to_content_editable(self) -> Self {
            if self == Self::EditableFocused { return Self::EditableFocusedContent; }
            self
        }
    }

    impl Display for TextBlockState {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "{self:?}")
        }
    }

    pub struct OcrState {
        pub ref_: NodeRef,
        pub state: TextBlockState,
    }


    type TBS = TextBlockState;

    impl OcrState {
        pub fn editable(&self) -> bool {
            self.state.editable()
        }

        pub fn contenteditable(&self) -> Option<&'static str> {
            if self.state == TBS::EditableFocusedContent {
                return Some("true");
            }
            None
        }

        pub fn cursor(&self) -> &str {
            match self.state {
                TBS::Default | TBS::EditableFocusedContent => "",
                TBS::Editable => "cursor: pointer;",
                TBS::EditableFocused => "cursor: move;"
            }
        }

        pub fn outline(&self) -> &str {
            match self.state {
                TextBlockState::Editable => "outline: 1.5px solid red;",
                TextBlockState::EditableFocused | TextBlockState::EditableFocusedContent
                => "outline: 1.5px solid #dd3300; box-shadow: 0 0 0 3px #aa6600;",
                TextBlockState::Default => "",
            }
        }

        pub fn user_select(&self) -> &str {
            match self.state {
                TBS::Editable | TBS::EditableFocused => "user-select: none;",
                TBS::Default | TBS::EditableFocusedContent => "",
            }
        }
    }

    impl PartialEq for OcrState {
        fn eq(&self, other: &Self) -> bool {
            self.state == other.state
        }
    }

    pub enum OcrAction {
        Focus,
        Unfocus,
        Editable(bool),
        EditContent,
    }

    impl Reducible for OcrState {
        type Action = OcrAction;
        fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
            match action {
                OcrAction::Focus => {
                    let state = self.state.focus();
                    if state == TBS::EditableFocused { focus(&self.ref_); }
                    Self { ref_: self.ref_.clone(), state }
                }
                OcrAction::Unfocus => {
                    get_selection().and_then(|s| s.empty().ok());
                    let state = self.state.unfocus();
                    Self { ref_: self.ref_.clone(), state }
                }
                OcrAction::Editable(editable) => {
                    let state = if self.state.editable() != editable {
                        TextBlockState::from_editable(editable)
                    } else { self.state };
                    Self { ref_: self.ref_.clone(), state }
                }
                OcrAction::EditContent => {
                    let state = self.state.to_content_editable();
                    if state == TextBlockState::EditableFocusedContent {
                        get_selection().and_then(|s| s.empty().ok());
                        focus(&self.ref_);
                    }
                    Self { ref_: self.ref_.clone(), state }
                }
            }.into()
        }
    }
}

pub mod page {
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
        use_future_with((page_name, reducer.clone()), |deps| async move {
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
        DeleteBlock(AttrValue),
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
                Self::Action::DeleteBlock(uuid) => {
                    gloo_console::log!("deleting block");
                    let mut ocr = self.ocr.clone();
                    let index =
                        ocr.blocks.iter().position(|b| b.uuid == uuid).unwrap();
                    ocr.blocks.remove(index);
                    Self {
                        _object: self._object.clone(),
                        _key: self._key.clone(),
                        ocr,
                        url: self.url.clone(),
                    }
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

pub mod volume {
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

pub mod cursor {
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
