//! A minimal EPUB reader library.

use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt::{self, Formatter};
use std::fs::File;
use std::io::{Error as IoError, Read as _};
use std::path::Path;

use roxmltree::{Document, Error as XmlError, ParsingOptions};
use zip::{ZipArchive, result::ZipError};

/// An opened EPUB book.
#[derive(Debug)]
pub struct Book {
    chapters: Vec<Chapter>,
}

/// A single chapter extracted from the book.
#[derive(Debug)]
pub struct Chapter {
    title: String,
    content: String,
}

impl Chapter {
    /// Returns the chapter title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns the chapter's text content.
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }
}

impl Book {
    /// Returns the chapters in spine order.
    #[must_use]
    pub fn chapters(&self) -> &[Chapter] {
        &self.chapters
    }
}

/// An error that can occur while opening a book.
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
            Self::Xml(_) | Self::NoRootfile => None,
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

/// Opens an EPUB file and returns the parsed [`Book`].
///
/// # Errors
///
/// Returns an [`Error`] if the file cannot be read, the ZIP archive is
/// invalid, an XML document cannot be parsed, or the container manifest
/// has no root element.
pub fn open(path: impl AsRef<Path>) -> Result<Book, Error> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;

    let container = read_entry(&mut archive, "META-INF/container.xml")?;
    let container_doc = parse_xml(&container)?;
    let opf_path = container_doc
        .descendants()
        .find(|node| node.has_tag_name("rootfile"))
        .and_then(|node| node.attribute("full-path"))
        .ok_or(Error::NoRootfile)?;

    let opf_xml = read_entry(&mut archive, opf_path)?;
    let opf_doc = parse_xml(&opf_xml)?;

    let manifest: HashMap<&str, &str> = opf_doc
        .descendants()
        .filter(|node| node.has_tag_name("item"))
        .filter_map(|node| Some((node.attribute("id")?, node.attribute("href")?)))
        .collect();

    let opf_dir = opf_path.rsplit_once('/').map_or("", |(dir, _)| dir);

    let chapters = opf_doc
        .descendants()
        .filter(|node| node.has_tag_name("itemref"))
        .filter_map(|node| node.attribute("idref"))
        .filter_map(|idref| {
            let href = *manifest.get(idref)?;
            let full_path = if opf_dir.is_empty() {
                href.to_string()
            } else {
                format!("{opf_dir}/{href}")
            };
            let xhtml = read_entry(&mut archive, &full_path).ok()?;
            let doc = parse_xml(&xhtml).ok()?;
            let title = doc
                .descendants()
                .find(|node| node.has_tag_name("title"))
                .and_then(|node| node.text())
                .unwrap_or("")
                .to_owned();
            let content = doc
                .descendants()
                .find(|node| node.has_tag_name("body"))?
                .descendants()
                .filter_map(|node| node.text())
                .collect::<Vec<_>>()
                .join("");
            Some(Chapter { title, content })
        })
        .collect();

    Ok(Book { chapters })
}

fn read_entry(archive: &mut ZipArchive<File>, entry_name: &str) -> Result<String, Error> {
    let mut entry = archive.by_name(entry_name)?;
    let mut buffer = String::new();
    entry.read_to_string(&mut buffer)?;
    Ok(buffer)
}

fn parse_xml(text: &str) -> Result<Document<'_>, XmlError> {
    Document::parse_with_options(
        text,
        ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    )
}
