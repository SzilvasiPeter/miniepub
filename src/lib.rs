//! A minimal EPUB reader library: parse a ZIP of XML files into structured chapters.

#![forbid(unsafe_code)]
use std::error::Error;
use std::fs;
use std::path::Path;

use mdream::{html_to_markdown, types::HTMLToMarkdownOptions};
use noflate::deflate::decompress;
use rawzip::{CompressionMethod, ZipArchive, ZipSliceArchive};
use xml::{EventReader, reader::XmlEvent};

/// An opened EPUB book with a table of contents and chapter navigation.
#[derive(Debug)]
pub struct Book {
    chapters: Vec<Chapter>,
    current: usize,
    archive: ZipSliceArchive<Vec<u8>>,
}

/// A table of contents entry.
#[derive(Debug)]
pub struct Chapter {
    /// The chapter title from the navigation document.
    pub title: String,
    path: String,
}

/// Opens the EPUB file at `path`.
///
/// # Errors
///
/// Returns an error if the file cannot be read or the EPUB structure is invalid.
pub fn open(path: impl AsRef<Path>) -> Result<Book, Box<dyn Error>> {
    let archive = ZipArchive::from_slice(fs::read(path)?)?;
    let container = read_entry(&archive, "META-INF/container.xml")?;
    let opf_path = attribute_values(&container, "rootfile", "full-path")
        .into_iter()
        .next()
        .ok_or_else(|| "missing rootfile full-path in container.xml".to_owned())?;
    let opf = read_entry(&archive, &opf_path)?;
    let nav_path = nav_href(&opf)
        .map(|href| resolve_href(&opf_path, &href))
        .ok_or_else(|| "missing nav document in manifest".to_owned())?;
    let nav = read_entry(&archive, &nav_path)?;
    let chapters = parse_nav(&nav)?
        .into_iter()
        .map(|(title, href)| Chapter { title, path: resolve_href(&opf_path, &href) })
        .collect();
    Ok(Book { chapters, current: 0, archive })
}

impl Book {
    /// Returns the table of contents.
    #[must_use]
    pub fn chapters(&self) -> &[Chapter] {
        &self.chapters
    }

    /// Moves to the next chapter and returns it as markdown.
    ///
    /// Stays on the current chapter if there is no next chapter.
    ///
    /// # Errors
    ///
    /// Returns an error if the chapter cannot be read or decoded.
    pub fn next_chapter(&mut self) -> Result<String, Box<dyn Error>> {
        let index = self
            .current
            .checked_add(1)
            .filter(|&index| index < self.chapters.len())
            .unwrap_or(self.current);
        self.current = index;
        self.markdown_at(index)
    }

    /// Moves back to the previous chapter and returns it as markdown.
    ///
    /// Stays on the current chapter if there is no previous chapter.
    ///
    /// # Errors
    ///
    /// Returns an error if the chapter cannot be read or decoded.
    pub fn previous_chapter(&mut self) -> Result<String, Box<dyn Error>> {
        let index = self.current.checked_sub(1).unwrap_or(self.current);
        self.current = index;
        self.markdown_at(index)
    }

    /// Returns the whole book converted to markdown.
    ///
    /// # Errors
    ///
    /// Returns an error if any chapter cannot be read or decoded.
    pub fn all(&self) -> Result<String, Box<dyn Error>> {
        let chapters: Result<Vec<String>, Box<dyn Error>> =
            (0..self.chapters.len()).map(|index| self.markdown_at(index)).collect();
        Ok(chapters?.join("\n\n"))
    }

    fn markdown_at(&self, index: usize) -> Result<String, Box<dyn Error>> {
        let Some(chapter) = self.chapters.get(index) else {
            return Ok(String::new());
        };
        let xhtml = read_entry(&self.archive, &chapter.path)?;
        let html = String::from_utf8(xhtml)?;
        Ok(html_to_markdown(&html, HTMLToMarkdownOptions::default()))
    }
}

fn read_entry(archive: &ZipSliceArchive<Vec<u8>>, path: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let way_finder = archive
        .entries()
        .find_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_path().try_normalize().ok()?;
            (name.as_ref() == path).then_some(entry.wayfinder())
        })
        .ok_or_else(|| format!("entry not found: {path}"))?;
    let entry = archive.get_entry(way_finder)?;
    let data = match entry.local_header().compression_method() {
        CompressionMethod::DEFLATE => decompress(entry.data())?,
        CompressionMethod::STORE => entry.data().to_vec(),
        method => return Err(format!("unsupported compression method {method:?}").into()),
    };
    Ok(data)
}

fn attribute_values(xml: &[u8], element: &str, attribute: &str) -> Vec<String> {
    EventReader::new(xml)
        .into_iter()
        .filter_map(|event| {
            let event = event.ok()?;
            let XmlEvent::StartElement { name, attributes, .. } = event else {
                return None;
            };
            (name.local_name == element).then_some(
                attributes.into_iter().find(|attr| attr.name.local_name == attribute)?.value,
            )
        })
        .collect()
}

fn nav_href(xml: &[u8]) -> Option<String> {
    EventReader::new(xml)
        .into_iter()
        .filter_map(|event| {
            let XmlEvent::StartElement { name, attributes, .. } = event.ok()? else {
                return None;
            };
            (name.local_name == "item").then_some(attributes)
        })
        .find(|attributes| {
            attributes.iter().any(|attr| {
                attr.name.local_name == "properties"
                    && attr.value.split_whitespace().any(|value| value == "nav")
            })
        })
        .and_then(|attributes| {
            attributes
                .iter()
                .find(|attr| attr.name.local_name == "href")
                .map(|attr| attr.value.clone())
        })
}

/// Accumulates table of contents entries while parsing a navigation document.
#[derive(Default)]
struct Toc {
    chapters: Vec<(String, String)>,
    in_nav: bool,
    href: Option<String>,
    title: String,
}

impl Toc {
    fn start_element(&mut self, name: &str, href: Option<String>) {
        match name {
            "nav" => self.in_nav = true,
            "a" if self.in_nav => {
                self.href = href;
                self.title.clear();
            }
            _ => {}
        }
    }

    fn end_element(&mut self, name: &str) {
        match name {
            "nav" => self.in_nav = false,
            "a" => self.chapters.extend(self.href.take().map(|href| (self.title.clone(), href))),
            _ => {}
        }
    }

    fn push_text(&mut self, text: &str) {
        self.title.push_str(text);
    }
}

fn parse_nav(xml: &[u8]) -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let mut toc = Toc::default();
    for event in EventReader::new(xml) {
        let event = event?;
        if let XmlEvent::StartElement { name, attributes, .. } = event {
            let href = attributes
                .into_iter()
                .find(|attr| attr.name.local_name == "href")
                .map(|attr| attr.value);
            toc.start_element(name.local_name.as_str(), href);
        } else if let XmlEvent::EndElement { name } = event {
            toc.end_element(name.local_name.as_str());
        } else if let XmlEvent::Characters(text) | XmlEvent::Whitespace(text) = event {
            toc.push_text(&text);
        }
    }
    Ok(toc.chapters)
}

fn resolve_href(base: &str, href: &str) -> String {
    base.rsplit_once('/').map_or_else(|| href.to_owned(), |(dir, _)| format!("{dir}/{href}"))
}
