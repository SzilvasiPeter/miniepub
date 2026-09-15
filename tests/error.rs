//! Tests for the [`Error`] type's public API.

use std::error::Error as _;
use std::io::{Error as IoError, ErrorKind};

use miniepub::Error;
use roxmltree::Document;
use zip::result::ZipError;

#[test]
fn display_shows_variant_specific_messages() {
    assert_eq!(
        Error::from(IoError::from(ErrorKind::NotFound)).to_string(),
        "I/O error: entity not found",
    );
    assert_eq!(
        Error::from(ZipError::FileNotFound).to_string(),
        "archive error: specified file not found in archive",
    );
    assert_eq!(
        Error::NoRootfile.to_string(),
        "missing rootfile in container.xml",
    );
}

#[test]
fn source_exposes_underlying_errors() {
    assert!(
        Error::from(IoError::from(ErrorKind::NotFound))
            .source()
            .is_some()
    );
    assert!(Error::from(ZipError::FileNotFound).source().is_some());
    assert!(Error::NoRootfile.source().is_none());
}

#[test]
fn xml_errors_convert_with_message_and_source() {
    if let Err(xml) = Document::parse("<<<") {
        let error = Error::from(xml);
        assert!(error.to_string().starts_with("XML error:"));
        assert!(error.source().is_some());
    }
}
