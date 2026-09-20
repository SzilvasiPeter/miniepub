//! Print the XHTML content files of an EPUB with `rawzip`, `noflate`, and `xml`.

#![forbid(unsafe_code)]
use std::error::Error;
use std::fs;

use noflate::deflate::decompress;
use rawzip::{CompressionMethod, ZipArchive, ZipSliceArchive};
use xml::{EventReader, reader::XmlEvent};

fn main() -> Result<(), Box<dyn Error>> {
    let archive = ZipArchive::from_slice(fs::read("tests/data/minimal-v3.epub")?)?;
    // TODO: verify container.xml and the OPF against the EPUB schemas before relying on their contents.
    let container = read_entry(&archive, "META-INF/container.xml")?;
    let opf_path = attribute_values(&container, "rootfile", "full-path")
        .into_iter()
        .next()
        .ok_or_else(|| "missing rootfile full-path in container.xml".to_owned())?;
    let opf = read_entry(&archive, &opf_path)?;
    for item in attribute_values(&opf, "item", "href") {
        let path = resolve_href(&opf_path, &item);
        let xhtml = read_entry(&archive, &path)?;
        println!("== {path} ==\n{}", String::from_utf8(xhtml)?);
    }
    Ok(())
}

fn read_entry(archive: &ZipSliceArchive<Vec<u8>>, path: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let wayfinder = archive
        .entries()
        .find_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_path().try_normalize().ok()?;
            (name.as_ref() == path).then_some(entry.wayfinder())
        })
        .ok_or_else(|| format!("entry not found: {path}"))?;
    let entry = archive.get_entry(wayfinder)?;
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

fn resolve_href(base: &str, href: &str) -> String {
    base.rsplit_once('/').map_or_else(|| href.to_owned(), |(dir, _)| format!("{dir}/{href}"))
}
