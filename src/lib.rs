//! A minimal EPUB reader library.

mod error;
pub use error::Error;

use std::collections::HashMap;
use std::fs::File;
use std::io::Read as _;
use std::path::Path;

use roxmltree::{Document, Error as XmlError, ParsingOptions};
use zip::ZipArchive;

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

// EPUB content files include DOCTYPE declarations; roxmltree rejects them by default
fn parse_xml(text: &str) -> Result<Document<'_>, XmlError> {
    Document::parse_with_options(
        text,
        ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    )
}
