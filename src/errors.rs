pub type Result<T> = std::result::Result<T, AppError>;

#[allow(dead_code)]
#[derive(Debug)]
pub enum AppError {
    InvalidMokuroFile(InvalidMokuroFileError),
    GlooFileError(gloo_file::FileReadError),
    RexieError(rexie::Error),
    SerdeJsonError(serde_json::Error),
    SerdeWasmError(serde_wasm_bindgen::Error),
    ZipError(zip::result::ZipError),
    JsValueError(wasm_bindgen::JsValue),
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum InvalidMokuroFileError {
    MissingFile(String)
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::InvalidMokuroFile(e) => write!(f, "Invalid mokuro file: {:?}", e),
            AppError::GlooFileError(e) => write!(f, "Gloo file error: {}", e),
            AppError::RexieError(e) => write!(f, "Rexie error: {}", e),
            AppError::SerdeJsonError(e) => write!(f, "Serde json error: {}", e),
            AppError::SerdeWasmError(e) => write!(f, "Serde-Wasm error: {}", e),
            AppError::ZipError(e) => write!(f, "Zip error: {}", e),
            AppError::JsValueError(e) => write!(f, "JsValue error: {:?}", e),
        }
    }
}

impl std::error::Error for AppError {}


impl From<gloo_file::FileReadError> for AppError {
    fn from(error: gloo_file::FileReadError) -> Self {
        AppError::GlooFileError(error)
    }
}

impl From<rexie::Error> for AppError {
    fn from(error: rexie::Error) -> Self {
        AppError::RexieError(error)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(error: serde_json::Error) -> Self {
        AppError::SerdeJsonError(error)
    }
}

impl From<serde_wasm_bindgen::Error> for AppError {
    fn from(error: serde_wasm_bindgen::Error) -> Self { AppError::SerdeWasmError(error) }
}

impl From<zip::result::ZipError> for AppError {
    fn from(error: zip::result::ZipError) -> Self {
        AppError::ZipError(error)
    }
}

impl From<wasm_bindgen::JsValue> for AppError {
    fn from(error: wasm_bindgen::JsValue) -> Self { AppError::JsValueError(error) }
}