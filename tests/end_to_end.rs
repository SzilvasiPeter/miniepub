//! End-to-end tests that read real EPUB files from `tests/data/`.

use std::error::Error;

use miniepub::open;

#[test]
fn reads_all_chapters_from_minimal_v3() -> Result<(), Box<dyn Error>> {
    let book = open("tests/data/minimal-v3.epub")?;
    let chapters = book.chapters();
    assert_eq!(chapters.len(), 1, "spine must expose exactly one chapter");

    let chapter = &chapters[0];
    assert_eq!(chapter.title(), "Section 1");
    assert!(chapter.content().contains("This is a paragraph."));

    Ok(())
}
