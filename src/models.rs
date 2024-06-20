use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct VolumeMetadata {
    #[serde(skip_serializing)]
    pub id: Option<u32>,
    pub version: String,
    pub created_at: String,
    pub modified_at: String,
    pub title: String,
    pub volume: String,
    pub volume_uuid: String,
    // Pages is an array of (page_name, ocr_name) pairs.
    pub pages: Box<[(String, String)]>,
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


