use serde::{Deserialize, Serialize};
use yew::AttrValue;

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct VolumeMetadata {
    #[serde(skip_serializing)]
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

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct PageOcr {
    pub img_width: u32,
    pub img_height: u32,
    pub blocks: Vec<OcrBlock>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct OcrBlock {
    #[serde(rename = "box")]
    pub box_: (u32, u32, u32, u32),
    pub vertical: bool,
    pub font_size: u32,
    pub lines: Vec<String>,
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
