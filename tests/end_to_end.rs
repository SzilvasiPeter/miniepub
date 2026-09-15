//! End-to-end tests that read real EPUB files from `tests/data/`.

use std::error::Error as StdError;

use miniepub::{Error as EpubError, open};

type VariantMatch = fn(&EpubError) -> bool;

fn assert_open_error(
    path: &str,
    prefix: &str,
    is_variant: VariantMatch,
) -> Result<(), Box<dyn StdError>> {
    let Err(error) = open(path) else {
        return Err(format!("expected error for {path}").into());
    };
    assert!(is_variant(&error), "unexpected error for {path}: {error:?}");
    assert!(
        error.to_string().starts_with(prefix),
        "unexpected message for {path}: {error:?}",
    );
    assert_eq!(
        error.source().is_some(),
        prefix != "missing rootfile",
        "unexpected source for {path}",
    );
    Ok(())
}

#[test]
fn reads_all_chapters_from_minimal_v3() -> Result<(), Box<dyn StdError>> {
    let book = open("tests/data/minimal-v3.epub")?;
    let chapters = book.chapters();
    assert_eq!(chapters.len(), 1, "spine must expose exactly one chapter");

    let chapter = &chapters[0];
    assert_eq!(chapter.title(), "Section 1");
    assert!(chapter.content().contains("This is a paragraph."));

    Ok(())
}

#[test]
fn errors_from_open_carry_variant_source_and_display() -> Result<(), Box<dyn StdError>> {
    let cases: [(&str, &str, VariantMatch); 5] = [
        ("tests/data/nonexistent.epub", "I/O error:", |error| {
            matches!(error, EpubError::Io(_))
        }),
        ("tests/data/not-a-zip.epub", "archive error:", |error| {
            matches!(error, EpubError::Zip(_))
        }),
        (
            "tests/data/malformed-container.epub",
            "XML error:",
            |error| matches!(error, EpubError::Xml(_)),
        ),
        ("tests/data/not-xml.epub", "XML error:", |error| {
            matches!(error, EpubError::Xml(_))
        }),
        ("tests/data/no-rootfile.epub", "missing rootfile", |error| {
            matches!(error, EpubError::NoRootfile)
        }),
    ];

    for (path, prefix, is_variant) in cases {
        assert_open_error(path, prefix, is_variant)?;
    }
    Ok(())
}
