use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

#[derive(Debug)]
pub enum ScanError {
    NotDirectory(PathBuf),
    Io { path: PathBuf, message: String },
    Json { path: PathBuf, message: String },
}

impl Display for ScanError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotDirectory(path) => write!(formatter, "not a directory: {}", path.display()),
            Self::Io { path, message } => write!(formatter, "{}: {message}", path.display()),
            Self::Json { path, message } => write!(formatter, "{}: {message}", path.display()),
        }
    }
}

impl Error for ScanError {}
