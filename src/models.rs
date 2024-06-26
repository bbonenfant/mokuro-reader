use serde::{Deserialize, Serialize};
use yew::AttrValue;

pub use magnifier::MagnifierSettings;
pub use reader_state::ReaderState;

#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct VolumeMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    pub version: AttrValue,
    pub created_at: AttrValue,
    pub modified_at: AttrValue,
    pub title: AttrValue,
    pub volume: AttrValue,
    pub volume_uuid: AttrValue,
    // Pages is an array of (page_name, ocr_name) pairs.
    pub pages: Box<[(AttrValue, AttrValue)]>,

    cover: Option<AttrValue>,
    #[serde(default)]
    pub magnifier: MagnifierSettings,
    #[serde(default)]
    pub reader_state: ReaderState,
}

mod magnifier {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Clone, Copy, PartialEq)]
    pub struct MagnifierSettings {
        #[serde(default = "default_zoom")]
        pub zoom: u16,
        #[serde(default = "default_radius")]
        pub radius: u8,
        #[serde(default = "default_size")]
        pub height: u16,
        #[serde(default = "default_size")]
        pub width: u16,
    }

    fn default_zoom() -> u16 { 200 }

    fn default_radius() -> u8 { 35 }

    fn default_size() -> u16 { 350 }

    impl Default for MagnifierSettings {
        fn default() -> Self {
            Self {
                zoom: default_zoom(),
                radius: default_radius(),
                height: default_size(),
                width: default_size(),
            }
        }
    }
}

mod reader_state {
    use serde::{Deserialize, Serialize};
    use yew::AttrValue;

    #[derive(Serialize, Deserialize, Clone, Copy, Default, PartialEq)]
    pub struct ReaderState {
        #[serde(default)]
        pub single_page: bool,
        #[serde(default)]
        pub current_page: usize,
        #[serde(default)]
        pub first_page_is_cover: bool,
    }

    impl ReaderState {
        pub fn select_pages(&self, pages: &[(AttrValue, AttrValue)]) -> (Option<AttrValue>, Option<AttrValue>) {
            let get_page = |i: usize| -> Option<AttrValue> {
                pages.get(i).map(|p| p.0.clone())
            };
            if self.single_page || (self.current_page == 0 && !self.first_page_is_cover) {
                return (get_page(self.current_page), None);
            }
            (get_page(self.current_page), get_page(self.current_page + 1))
        }

        pub fn forward(&mut self) {
            if self.single_page || (self.current_page == 0 && !self.first_page_is_cover) {
                self.current_page += 1;
            } else {
                self.current_page += 2;
            }
        }

        pub fn backward(&mut self) {
            if self.single_page || (self.current_page == 0 && !self.first_page_is_cover) {
                if self.current_page > 0 {
                    self.current_page -= 1;
                }
            } else if self.current_page == 1 {
                self.current_page -= 1;
            } else if self.current_page > 1 {
                self.current_page -= 2;
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
    pub box_: (u32, u32, u32, u32),
    pub vertical: bool,
    pub font_size: u32,
    pub lines: Vec<AttrValue>,
    // lines_coords: Vec<Vec<(f32, f32)>>,
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
