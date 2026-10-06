use std::error::Error as StdError;
use std::fmt;
use std::io;
use std::string::FromUtf8Error;

use xml::reader;

/// Errors that can occur while reading an EPUB book.
#[derive(Debug)]
pub enum Error {
    /// An I/O error occurred.
    Io(io::Error),
    /// A UTF-8 decoding error occurred.
    Utf8(FromUtf8Error),
    /// An XML parsing error occurred.
    Xml(reader::Error),
    /// A ZIP archive error occurred.
    Zip(rawzip::Error),
    /// A decompression error occurred.
    Decompress(noflate::Error),
    /// An entry was not found in the archive.
    EntryNotFound(String),
    /// An unsupported compression method was encountered.
    UnsupportedCompression(String),
    /// The rootfile full-path is missing from container.xml.
    MissingRootfile,
    /// The nav document is missing from the manifest.
    MissingNav,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "IO error: {err}"),
            Self::Utf8(err) => write!(f, "UTF-8 error: {err}"),
            Self::Xml(err) => write!(f, "XML error: {err}"),
            Self::Zip(err) => write!(f, "ZIP error: {err}"),
            Self::Decompress(err) => write!(f, "decompression error: {err}"),
            Self::EntryNotFound(path) => write!(f, "entry not found: {path}"),
            Self::UnsupportedCompression(method) => write!(f, "unsupported compression: {method}"),
            Self::MissingRootfile => write!(f, "missing rootfile full-path in container.xml"),
            Self::MissingNav => write!(f, "missing nav document in manifest"),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Utf8(err) => Some(err),
            Self::Xml(err) => Some(err),
            Self::Zip(err) => Some(err),
            Self::Decompress(_)
            | Self::EntryNotFound(_)
            | Self::UnsupportedCompression(_)
            | Self::MissingRootfile
            | Self::MissingNav => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<FromUtf8Error> for Error {
    fn from(err: FromUtf8Error) -> Self {
        Self::Utf8(err)
    }
}

impl From<reader::Error> for Error {
    fn from(err: reader::Error) -> Self {
        Self::Xml(err)
    }
}

impl From<rawzip::Error> for Error {
    fn from(err: rawzip::Error) -> Self {
        Self::Zip(err)
    }
}

impl From<noflate::Error> for Error {
    fn from(err: noflate::Error) -> Self {
        Self::Decompress(err)
    }
}
