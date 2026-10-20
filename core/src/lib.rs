//! A minimal EPUB reader library: parse a ZIP of XML files into structured chapters.

#![forbid(unsafe_code)]
#![warn(clippy::print_stdout, clippy::print_stderr)]
use std::fs;
use std::path::Path;

use mdream::{html_to_markdown, types::HtmlToMarkdownOptions};
use noflate::deflate::decompress;
use rawzip::{CompressionMethod, ZipArchive, ZipSliceArchive};
use std::collections::HashSet;
use xml::attribute::OwnedAttribute;
use xml::{EventReader, reader::XmlEvent};

mod error;
pub use error::Error;

/// The media types a spine entry must carry to count as book content.
const CONTENT_MEDIA_TYPES: [&str; 2] = ["application/xhtml+xml", "text/html"];

/// An opened EPUB book with a table of contents and chapter navigation.
#[derive(Debug)]
pub struct Book {
    /// The reading units: spine documents in reading order, split into sections
    /// wherever the table of contents anchors them.
    chapters: Vec<Chapter>,
    current: usize,
    archive: ZipSliceArchive<Vec<u8>>,
}

/// A reading unit: a document from the spine, optionally scoped to a table of contents anchor.
#[derive(Debug)]
pub struct Chapter {
    /// The title from the table of contents, or the file stem for spine documents
    /// missing from it.
    pub title: String,
    /// The archive entry of the spine document.
    path: String,
    /// The anchor at which this reading unit starts; `None` covers the whole document.
    anchor: Option<String>,
    /// The anchor starting the next unit in the same document, when one exists.
    next_anchor: Option<String>,
}

/// Opens the EPUB file at `path`.
///
/// # Errors
///
/// Returns an error if the file cannot be read or the EPUB structure is invalid.
pub fn open(path: impl AsRef<Path>) -> Result<Book, Error> {
    let archive = ZipArchive::from_slice(fs::read(path)?)?;
    let container = read_entry(&archive, "META-INF/container.xml")?;
    let opf_path = attribute_values(&container, "rootfile", "full-path")
        .into_iter()
        .next()
        .ok_or(Error::MissingRootfile)?;
    let opf = read_entry(&archive, &opf_path)?;
    let package = parse_package(&opf)?;
    let docs = spine_docs(&package, &opf_path);
    let links = toc_links(&package, &opf_path, &archive)?;
    Ok(Book { chapters: reading_units(&docs, &links), current: 0, archive })
}

impl Book {
    /// Returns the table of contents as reading units.
    #[must_use]
    pub fn chapters(&self) -> &[Chapter] {
        &self.chapters
    }

    /// Moves to the next chapter.
    ///
    /// Stays on the current chapter if there is no next chapter.
    pub fn next_chapter(&mut self) {
        self.current = self
            .current
            .checked_add(1)
            .filter(|&index| index < self.chapters.len())
            .unwrap_or(self.current);
    }

    /// Moves back to the previous chapter.
    ///
    /// Stays on the current chapter if there is no previous chapter.
    pub fn previous_chapter(&mut self) {
        self.current = self.current.checked_sub(1).unwrap_or(self.current);
    }

    /// Returns the current chapter as markdown.
    ///
    /// # Errors
    ///
    /// Returns an error if the chapter cannot be read or decoded.
    pub fn current_chapter(&self) -> Result<String, Error> {
        self.chapter_at(self.current)
    }

    /// Returns the chapter at `index` as markdown, scoped to its anchor when it has one.
    ///
    /// # Errors
    ///
    /// Returns an error if `index` is out of bounds or the chapter cannot be read.
    pub fn chapter_at(&self, index: usize) -> Result<String, Error> {
        let Some(chapter) = self.chapters.get(index) else {
            return Err(Error::IndexOutOfBounds(index));
        };
        let html_bytes = read_entry(&self.archive, &chapter.path)?;
        let html = String::from_utf8(html_bytes)?;
        let section =
            section_html(&html, chapter.anchor.as_deref(), chapter.next_anchor.as_deref());
        Ok(html_to_markdown(section, HtmlToMarkdownOptions::default()))
    }

    /// Returns the whole book converted to markdown, one rendering per document.
    ///
    /// # Errors
    ///
    /// Returns an error if any chapter cannot be read or decoded.
    pub fn all(&self) -> Result<String, Error> {
        let mut seen: HashSet<&str> = HashSet::new();
        let documents = self
            .chapters
            .iter()
            .map(|chapter| chapter.path.as_str())
            .filter(|path| seen.insert(path))
            .map(|path| {
                let html_bytes = read_entry(&self.archive, path)?;
                let html = String::from_utf8(html_bytes)?;
                Ok(html_to_markdown(&html, HtmlToMarkdownOptions::default()))
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(documents.join("\n\n"))
    }
}

/// Returns the value of an attribute by local name.
#[must_use]
fn attribute<'a>(attributes: &'a [OwnedAttribute], name: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find(|attribute| attribute.name.local_name == name)
        .map(|attribute| attribute.value.as_str())
}

/// An entry of the OPF manifest.
#[derive(Debug, Default)]
struct ManifestItem {
    id: String,
    href: String,
    media_type: String,
    nav: bool,
}

/// The package document: manifest entries plus the spine in reading order.
#[derive(Debug, Default)]
struct Package {
    items: Vec<ManifestItem>,
    /// The `idref` of each spine entry, in reading order.
    spine: Vec<String>,
    /// The spine's `toc` attribute: the id of the EPUB 2 NCX entry.
    ncx_idref: Option<String>,
}

impl Package {
    fn start_element(&mut self, name: &str, attributes: &[OwnedAttribute]) {
        match name {
            "item" => self.items.push(ManifestItem {
                id: attribute(attributes, "id").unwrap_or_default().to_owned(),
                href: attribute(attributes, "href").unwrap_or_default().to_owned(),
                media_type: attribute(attributes, "media-type").unwrap_or_default().to_owned(),
                nav: attribute(attributes, "properties").is_some_and(|properties| {
                    properties.split_whitespace().any(|property| property == "nav")
                }),
            }),
            "spine" => self.ncx_idref = attribute(attributes, "toc").map(str::to_owned),
            "itemref" => {
                self.spine.push(attribute(attributes, "idref").unwrap_or_default().to_owned());
            }
            _ => {}
        }
    }
}

fn parse_package(xml: &[u8]) -> Result<Package, Error> {
    let mut package = Package::default();
    for event in EventReader::new(xml) {
        if let XmlEvent::StartElement { name, attributes, .. } = event? {
            package.start_element(name.local_name.as_str(), &attributes);
        }
    }
    Ok(package)
}

/// The paths of the book's content documents, in reading order.
fn spine_docs(package: &Package, opf_path: &str) -> Vec<String> {
    package
        .spine
        .iter()
        .filter_map(|idref| {
            let item = package.items.iter().find(|item| item.id == *idref)?;
            CONTENT_MEDIA_TYPES
                .contains(&item.media_type.as_str())
                .then(|| resolve_href(opf_path, &item.href))
        })
        .collect()
}

/// The kind of document providing the table of contents.
#[derive(Debug)]
enum TocKind {
    /// An EPUB 3 navigation document.
    Nav,
    /// An EPUB 2 NCX document.
    Ncx,
}

/// A link of the table of contents, resolved to an archive entry.
#[derive(Debug)]
struct TocLink {
    title: String,
    path: String,
    anchor: Option<String>,
}

/// Parses the table of contents document (a navigation document or an NCX),
/// with its links resolved against that document's own path.
fn toc_links(
    package: &Package,
    opf_path: &str,
    archive: &ZipSliceArchive<Vec<u8>>,
) -> Result<Vec<TocLink>, Error> {
    let toc = package
        .items
        .iter()
        .find(|item| item.nav)
        .map(|item| (item.href.as_str(), TocKind::Nav))
        .or_else(|| {
            let ncx_idref = package.ncx_idref.as_deref()?;
            package
                .items
                .iter()
                .find(|item| item.id == ncx_idref)
                .map(|item| (item.href.as_str(), TocKind::Ncx))
        })
        .ok_or(Error::MissingNav)?;
    let path = resolve_href(opf_path, toc.0);
    let bytes = read_entry(archive, &path)?;
    let links = match toc.1 {
        TocKind::Nav => parse_nav(&bytes)?,
        TocKind::Ncx => parse_ncx(&bytes)?,
    };
    Ok(links
        .into_iter()
        .map(|(title, href)| {
            let (doc_part, anchor) = split_href(&href);
            TocLink {
                title,
                path: resolve_href(&path, doc_part),
                anchor: anchor.map(str::trim).map(str::to_owned),
            }
        })
        .collect())
}

/// Builds the reading units: one per table-of-contents section of each spine
/// document, or one whole-document unit for a document without any entry.
fn reading_units(docs: &[String], links: &[TocLink]) -> Vec<Chapter> {
    let mut chapters = Vec::new();
    for doc in docs {
        let sections: Vec<&TocLink> = links.iter().filter(|link| link.path == *doc).collect();
        match sections.as_slice() {
            [] => chapters.push(Chapter {
                title: file_stem(doc),
                path: doc.clone(),
                anchor: None,
                next_anchor: None,
            }),
            sections => chapters.extend(section_units(sections, doc)),
        }
    }
    chapters
}

/// The reading units of one document, anchored on its table-of-contents entries.
fn section_units(sections: &[&TocLink], doc: &str) -> Vec<Chapter> {
    let mut chapters: Vec<Chapter> = sections
        .windows(2)
        .map(|pair| Chapter {
            title: chapter_title(pair[0], doc),
            path: doc.to_owned(),
            anchor: pair[0].anchor.clone(),
            next_anchor: pair[1].anchor.clone(),
        })
        .collect();
    let last = sections[sections.len() - 1];
    chapters.push(Chapter {
        title: chapter_title(last, doc),
        path: doc.to_owned(),
        anchor: last.anchor.clone(),
        next_anchor: None,
    });
    chapters
}

/// The chapter title, falling back to the file stem for empty titles.
fn chapter_title(link: &TocLink, doc: &str) -> String {
    if link.title.is_empty() { file_stem(doc) } else { link.title.clone() }
}

/// The file stem of an archive path.
fn file_stem(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .map_or_else(|| path.to_owned(), |stem| stem.to_string_lossy().into_owned())
}

/// Splits an href into its document part and its fragment.
fn split_href(href: &str) -> (&str, Option<&str>) {
    let href = href.trim();
    match href.split_once('#') {
        Some((path, fragment)) => (path, Some(fragment)),
        None => (href, None),
    }
}

/// Resolves the document part of an href relative to the path of the document
/// containing it, dropping fragment and percent-encoding.
fn resolve_href(base: &str, href: &str) -> String {
    let (path, _) = split_href(href);
    let dir = base.rsplit_once('/').map_or("", |(dir, _)| dir);
    let joined = format!("{dir}/{}", percent_decode(path));
    let mut segments: Vec<&str> = Vec::new();
    for segment in joined.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            segment => segments.push(segment),
        }
    }
    segments.join("/")
}

/// Percent-decodes an href; a part that is not valid UTF-8 afterwards is left as is.
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut decoded: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if let Some(byte) = escape_byte(&bytes[index..]) {
            decoded.push(byte);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).unwrap_or_else(|_| input.to_owned())
}

/// The byte encoded by the `%XX` escape at the front of `bytes`, if it is a valid one.
#[must_use]
fn escape_byte(bytes: &[u8]) -> Option<u8> {
    let (Some(&b'%'), Some([first, second])) = (bytes.first(), bytes.get(1..3)) else {
        return None;
    };
    let hex = [*first, *second];
    let hex = str::from_utf8(&hex).ok()?;
    let is_hex = hex.chars().all(|digit| digit.is_ascii_hexdigit());
    u8::from_str_radix(hex, 16).ok().filter(|_| is_hex)
}

/// Returns the byte offset where the element carrying the anchor id opens.
#[must_use]
fn element_start(html: &str, anchor: &str) -> Option<usize> {
    let quoted =
        html.find(&format!("id=\"{anchor}\"")).or_else(|| html.find(&format!("id='{anchor}'")))?;
    html[..quoted].rfind('<')
}

/// Returns the part of `html` from the element carrying the start anchor to the
/// element carrying the next section's anchor, or to the end of the document.
///
/// Falls back to the whole document when the anchors yield no slice at all.
fn section_html<'html>(html: &'html str, start: Option<&str>, next: Option<&str>) -> &'html str {
    let from = start.and_then(|anchor| element_start(html, anchor)).unwrap_or(0);
    let to = next.and_then(|anchor| element_start(html, anchor)).unwrap_or(html.len());
    if from < to { &html[from..to] } else { html }
}

/// Reads an archive entry as raw bytes, decompressing it when necessary.
fn read_entry(archive: &ZipSliceArchive<Vec<u8>>, path: &str) -> Result<Vec<u8>, Error> {
    let way_finder = archive
        .entries()
        .find_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_path().try_normalize().ok()?;
            (name.as_ref() == path).then_some(entry.wayfinder())
        })
        .ok_or_else(|| Error::EntryNotFound(path.to_owned()))?;
    let entry = archive.get_entry(way_finder)?;
    let data = match entry.local_header().compression_method() {
        CompressionMethod::DEFLATE => decompress(entry.data())?,
        CompressionMethod::STORE => entry.data().to_vec(),
        method => return Err(Error::UnsupportedCompression(format!("{method:?}"))),
    };
    Ok(data)
}

/// Returns the values of an attribute across all elements with the given name.
fn attribute_values(xml: &[u8], element: &str, attribute_name: &str) -> Vec<String> {
    EventReader::new(xml)
        .into_iter()
        .filter_map(|event| {
            let event = event.ok()?;
            let XmlEvent::StartElement { name, attributes, .. } = event else {
                return None;
            };
            (name.local_name == element)
                .then_some(attribute(&attributes, attribute_name)?.to_owned())
        })
        .collect()
}

/// A `<nav>` section of a navigation document with its `epub:type` and links.
#[derive(Debug, Default)]
struct NavSection {
    kind: Option<String>,
    entries: Vec<(String, String)>,
}

/// Scans a navigation document, grouping links by their enclosing `<nav>` section.
#[derive(Default)]
struct NavScan {
    open_nav: Option<usize>,
    navs: Vec<NavSection>,
    open_link: Option<String>,
    open_title: String,
}

impl NavScan {
    fn start_element(&mut self, name: &str, attributes: &[OwnedAttribute]) {
        match name {
            "nav" => {
                self.navs.push(NavSection {
                    kind: attribute(attributes, "epub:type").map(str::to_owned),
                    entries: Vec::new(),
                });
                self.open_nav = Some(self.navs.len() - 1);
            }
            "a" if self.open_nav.is_some() => {
                self.open_link = attribute(attributes, "href").map(str::to_owned);
                self.open_title.clear();
            }
            _ => {}
        }
    }

    fn push_text(&mut self, text: &str) {
        if self.open_link.is_some() {
            self.open_title.push_str(text);
        }
    }

    fn end_element(&mut self, name: &str) {
        match name {
            "nav" => self.open_nav = None,
            "a" => self.close_link(),
            _ => {}
        }
    }

    fn close_link(&mut self) {
        let (Some(index), Some(href)) = (self.open_nav, self.open_link.take()) else { return };
        self.navs[index].entries.push((self.open_title.trim().to_owned(), href));
    }
}

/// Returns the entries of the navigation document's `toc` section, falling back
/// to the first section with entries when none is marked as the table of contents.
fn toc_nav_entries(navs: &[NavSection]) -> Vec<(String, String)> {
    let is_toc = |section: &NavSection| {
        section
            .kind
            .as_deref()
            .is_some_and(|kind| kind.split_whitespace().any(|kind| kind == "toc"))
    };
    navs.iter()
        .find(|section| is_toc(section) && !section.entries.is_empty())
        .or_else(|| navs.iter().find(|section| !section.entries.is_empty()))
        .map_or_else(Vec::new, |section| section.entries.clone())
}

fn parse_nav(xml: &[u8]) -> Result<Vec<(String, String)>, Error> {
    let mut nav = NavScan::default();
    for event in EventReader::new(xml) {
        match event? {
            XmlEvent::EndElement { name } => nav.end_element(name.local_name.as_str()),
            XmlEvent::StartElement { name, attributes, .. } => {
                nav.start_element(name.local_name.as_str(), &attributes);
            }
            XmlEvent::Characters(text) | XmlEvent::Whitespace(text) => nav.push_text(&text),
            XmlEvent::StartDocument { .. }
            | XmlEvent::EndDocument
            | XmlEvent::ProcessingInstruction { .. }
            | XmlEvent::CData(_)
            | XmlEvent::Comment(_)
            | XmlEvent::Doctype { .. } => {}
        }
    }
    Ok(toc_nav_entries(&nav.navs))
}

/// Scans an EPUB 2 NCX table of contents.
#[derive(Default)]
struct Ncx {
    label: Option<String>,
    href: Option<String>,
    entries: Vec<(String, String)>,
}

impl Ncx {
    fn start_element(&mut self, name: &str, attributes: &[OwnedAttribute]) {
        match name {
            "navLabel" => self.label = Some(String::new()),
            "content" => self.href = attribute(attributes, "src").map(str::to_owned),
            _ => {}
        }
    }

    fn push_text(&mut self, text: &str) {
        if let Some(label) = &mut self.label {
            label.push_str(text);
        }
    }

    fn end_element(&mut self, name: &str) {
        if name == "navPoint"
            && let (Some(label), Some(href)) = (self.label.take(), self.href.take())
        {
            self.entries.push((label.trim().to_owned(), href));
        }
    }
}

fn parse_ncx(xml: &[u8]) -> Result<Vec<(String, String)>, Error> {
    let mut ncx = Ncx::default();
    for event in EventReader::new(xml) {
        match event? {
            XmlEvent::EndElement { name } => ncx.end_element(name.local_name.as_str()),
            XmlEvent::StartElement { name, attributes, .. } => {
                ncx.start_element(name.local_name.as_str(), &attributes);
            }
            XmlEvent::Characters(text) | XmlEvent::Whitespace(text) => ncx.push_text(&text),
            XmlEvent::StartDocument { .. }
            | XmlEvent::EndDocument
            | XmlEvent::ProcessingInstruction { .. }
            | XmlEvent::CData(_)
            | XmlEvent::Comment(_)
            | XmlEvent::Doctype { .. } => {}
        }
    }
    Ok(ncx.entries)
}
