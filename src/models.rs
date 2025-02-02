use serde::{Deserialize, Serialize};
use yew::AttrValue;

pub use magnifier::MagnifierSettings;
pub use reader_state::ReaderState;

pub type VolumeId = usize;


#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct Settings {
    pub magnifier: MagnifierSettings,
}

#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct VolumeMetadata {
    /// IndexedDB specifies that all auto-incrementing keys start at 1,
    /// meaning that we can use 0u64 as our None value without complicating
    /// the model with an Option<_> type which can only be unset when a volume
    /// is uploaded. Maximum value of the key is 2^(53) + 1 which is within
    /// the bounds of usize (used it place of u64 as u64 is represented as a
    /// BigInt JsValue.
    /// ref: w3c.github.io/IndexedDB/#key-generator-construct
    #[serde(default, skip_serializing_if = "is_zero")]
    pub id: VolumeId,
    pub version: AttrValue,
    pub created_at: AttrValue,
    pub modified_at: AttrValue,
    #[serde(default)]
    pub series: AttrValue,
    pub title: AttrValue,
    pub volume: AttrValue,
    pub volume_uuid: AttrValue,
    // Pages is an array of (page_name, ocr_name) pairs.
    pub pages: Box<[(AttrValue, AttrValue)]>,

    #[serde(default)]
    cover: Option<AttrValue>,
    #[serde(default)]
    pub hide_sidebar: bool,
    #[serde(default = "default_line_height")]
    pub line_height: f64,
    #[serde(default)]
    pub magnifier: MagnifierSettings,
    #[serde(default)]
    pub reader_state: ReaderState,
}

fn is_zero(value: &VolumeId) -> bool { *value == 0 }

fn default_line_height() -> f64 { 1.0f64 }

mod magnifier {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Clone, Copy, PartialEq)]
    #[serde(default)]
    pub struct MagnifierSettings {
        pub zoom: u16,
        pub radius: u8,
        pub height: u16,
        pub width: u16,
    }

    impl Default for MagnifierSettings {
        fn default() -> Self {
            Self {
                zoom: 200,
                radius: 35,
                height: 350,
                width: 350,
            }
        }
    }
}

mod reader_state {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Clone, Copy, PartialEq)]
    #[serde(default)]
    pub struct ReaderState {
        pub single_page: bool,
        pub current_page: usize,
        pub first_page_is_cover: bool,
    }

    impl Default for ReaderState {
        fn default() -> Self {
            Self {
                single_page: false,
                current_page: 0,
                first_page_is_cover: true,
            }
        }
    }
}


impl<'a> VolumeMetadata {
    /// Convenience method for getting the name of cover art,
    /// whether `self.cover` is set or not.
    pub fn cover(&'a self) -> &'a AttrValue {
        if let Some(page) = self.cover.as_ref() {
            return page;
        }
        &self.pages[0].0
    }

    pub fn page_forward(&mut self) {
        let ReaderState {
            single_page, current_page, first_page_is_cover
        } = self.reader_state;
        let len = self.pages.len();
        let increment = match (current_page, single_page, first_page_is_cover) {
            (p, _, _) if p >= (len - 1) => 0,
            (p, _, _) if p == (len - 2) => 1,
            (p, _, true) if p % 2 == 0 => 1,
            (0.., true, _) => 1,
            (0.., false, _) => 2,
        };
        self.reader_state.current_page += increment;
    }

    pub fn page_backward(&mut self) {
        let ReaderState {
            current_page, single_page, first_page_is_cover
        } = self.reader_state;
        let decrement = match (current_page, single_page, first_page_is_cover) {
            (0, _, _) => 0,
            (1, _, _) => 1,
            (2.., true, _) => 1,
            (p, _, true) if p % 2 == 0 => 1,
            (2.., false, _) => 2,
        };
        self.reader_state.current_page -= decrement;
    }

    pub fn select_pages(&self) -> (Option<AttrValue>, Option<AttrValue>) {
        let get_page = |i: usize| -> Option<AttrValue> {
            self.pages.get(i).map(|p| p.0.clone())
        };
        let ReaderState { single_page, current_page, first_page_is_cover } = self.reader_state;
        if single_page || (current_page == 0 && first_page_is_cover) {
            return (get_page(current_page), None);
        }
        (get_page(current_page), get_page(current_page + 1))
    }
}

#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct PageOcr {
    pub img_width: u32,
    pub img_height: u32,
    pub blocks: Vec<OcrBlock>,
}

#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct OcrBlock {
    pub uuid: AttrValue,
    #[serde(rename = "box")]
    // (left, top, right, bottom)
    pub box_: (u32, u32, u32, u32),
    pub vertical: bool,
    pub font_size: u32,
    pub lines: Vec<AttrValue>,
    // lines_coords: Vec<Vec<(f32, f32)>>,
}

impl OcrBlock {
    pub fn new(
        top: f64, left: f64, bottom: f64, right: f64,
        font_size: u32, vertical: bool,
    ) -> Self {
        let uuid = {
            let ts = uuid::Timestamp::now(uuid::NoContext);
            uuid::Uuid::new_v7(ts).simple().to_string().into()
        };
        let box_ = (left as u32, top as u32, right as u32, bottom as u32);
        Self { uuid, box_, vertical, font_size, lines: Vec::default() }
    }

    pub fn validate(&self) -> bool {
        self.box_.0 <= self.box_.2 && self.box_.1 <= self.box_.3
    }

    pub fn top(&self) -> f64 { self.box_.1 as f64 }
    pub fn left(&self) -> f64 { self.box_.0 as f64 }
    pub fn height(&self) -> f64 { self.box_.3.saturating_sub(self.box_.1) as f64 }
    pub fn width(&self) -> f64 { self.box_.2.saturating_sub(self.box_.0) as f64 }
}

#[derive(Clone)]
pub struct PageImage {
    inner: gloo_file::File,
}

impl PageImage {
    pub fn new(name: &str, data: &[u8]) -> Self {
        Self { inner: gloo_file::File::new(name, data) }
    }

    pub fn size(&self) -> u64 {
        self.inner.size()
    }
}

impl AsRef<gloo_file::Blob> for PageImage {
    fn as_ref(&self) -> &gloo_file::Blob {
        &self.inner
    }
}

impl AsRef<wasm_bindgen::JsValue> for PageImage {
    fn as_ref(&self) -> &wasm_bindgen::JsValue {
        (*self.inner).as_ref()
    }
}

impl From<wasm_bindgen::JsValue> for PageImage {
    /// This is technically not a perfect "From" impl as the name is not set.
    fn from(value: wasm_bindgen::JsValue) -> Self {
        let blob: gloo_file::Blob = {
            let blob: web_sys::Blob = value.into();
            blob.into()
        };
        Self { inner: gloo_file::File::new("", blob) }
    }
}

impl From<PageImage> for gloo_file::ObjectUrl {
    fn from(page_image: PageImage) -> Self {
        page_image.inner.into()
    }
}
