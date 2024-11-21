pub type Result<T> = std::result::Result<T, AppError>;

#[allow(dead_code)]
#[derive(Debug)]
pub enum AppError {
    InvalidMokuroFile(InvalidMokuroFileError),
    GlooFileError(gloo_file::FileReadError),
    RexieError(rexie::Error),
    ZipError(zip::result::ZipError),
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum InvalidMokuroFileError {
    MissingFile(String)
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AppRuntimeError")
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

impl From<zip::result::ZipError> for AppError {
    fn from(error: zip::result::ZipError) -> Self {
        AppError::ZipError(error)
    }
}