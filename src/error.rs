//! Errors that can occur while opening an EPUB.

use std::error::Error as StdError;
use std::fmt::{self, Formatter};
use std::io::Error as IoError;

use roxmltree::Error as XmlError;
use zip::result::ZipError;

/// An error that can occur while opening an EPUB.
#[derive(Debug)]
pub enum Error {
    /// An I/O error.
    Io(IoError),
    /// An archive (ZIP) error.
    Zip(ZipError),
    /// An XML parsing error.
    Xml(XmlError),
    /// The container manifest has no root element.
    NoRootfile,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "I/O error: {error}"),
            Self::Zip(error) => write!(formatter, "archive error: {error}"),
            Self::Xml(error) => write!(formatter, "XML error: {error}"),
            Self::NoRootfile => formatter.write_str("missing rootfile in container.xml"),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Zip(error) => Some(error),
            Self::Xml(error) => Some(error),
            Self::NoRootfile => None,
        }
    }
}

impl From<IoError> for Error {
    fn from(error: IoError) -> Self {
        Self::Io(error)
    }
}

impl From<ZipError> for Error {
    fn from(error: ZipError) -> Self {
        Self::Zip(error)
    }
}

impl From<XmlError> for Error {
    fn from(error: XmlError) -> Self {
        Self::Xml(error)
    }
}
