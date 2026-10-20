//! Tests for the miniepub reader API.

#![allow(clippy::unwrap_used)]

#[test]
fn open_reads_table_of_contents() {
    let book = miniepub::open("tests/data/multi-chapter.epub").unwrap();
    let titles: Vec<&str> = book.chapters().iter().map(|chapter| chapter.title.as_str()).collect();
    assert_eq!(titles, ["Chapter One", "Chapter Two", "Chapter Three"]);
}

#[test]
fn open_missing_file_fails() {
    assert!(miniepub::open("tests/data/does-not-exist.epub").is_err());
}

#[test]
fn next_moves_through_chapters() {
    let mut book = miniepub::open("tests/data/multi-chapter.epub").unwrap();
    book.next_chapter();
    assert!(
        book.current_chapter()
            .unwrap()
            .contains("This is the second chapter with *emphasis* and **strong** text.")
    );
    book.next_chapter();
    assert!(book.current_chapter().unwrap().contains("This is the third chapter."));
    book.next_chapter();
    assert!(book.current_chapter().unwrap().contains("This is the third chapter."));
}

#[test]
fn back_moves_to_previous_chapter() {
    let mut book = miniepub::open("tests/data/multi-chapter.epub").unwrap();
    book.previous_chapter();
    assert!(book.current_chapter().unwrap().contains("This is the first chapter."));
    book.next_chapter();
    book.next_chapter();
    book.previous_chapter();
    assert!(
        book.current_chapter()
            .unwrap()
            .contains("This is the second chapter with *emphasis* and **strong** text.")
    );
    book.previous_chapter();
    assert!(book.current_chapter().unwrap().contains("This is the first chapter."));
}

#[test]
fn all_returns_whole_content() {
    let book = miniepub::open("tests/data/multi-chapter.epub").unwrap();
    let all = book.all().unwrap();
    assert!(all.contains("This is the first chapter."));
    assert!(all.contains("This is the second chapter with *emphasis* and **strong** text."));
    assert!(all.contains("This is the third chapter."));
}

#[test]
fn single_chapter_book_stays_in_place() {
    let mut book = miniepub::open("tests/data/minimal-v3.epub").unwrap();
    assert_eq!(book.chapters().len(), 1);
    book.next_chapter();
    assert!(book.current_chapter().unwrap().contains("This is a paragraph."));
    book.previous_chapter();
    assert!(book.current_chapter().unwrap().contains("This is a paragraph."));
}

#[test]
fn chapter_at_out_of_bounds_returns_error() {
    let book = miniepub::open("tests/data/multi-chapter.epub").unwrap();
    assert!(book.chapter_at(99).is_err());
}

#[test]
fn open_reads_epub2_ncx_table_of_contents() {
    let book = miniepub::open("tests/data/epub2-ncx.epub").unwrap();
    let titles: Vec<&str> = book.chapters().iter().map(|chapter| chapter.title.as_str()).collect();
    assert_eq!(titles, ["First", "Second"]);
}

#[test]
fn ncx_anchor_scopes_section_content() {
    let book = miniepub::open("tests/data/epub2-ncx.epub").unwrap();
    let first = book.chapter_at(0).unwrap();
    let second = book.chapter_at(1).unwrap();
    assert!(first.contains("First section paragraph."));
    assert!(!first.contains("Second section paragraph."));
    assert!(second.contains("Second section paragraph."));
}

#[test]
fn open_reads_nav_fragments_and_ignores_landmarks() {
    let book = miniepub::open("tests/data/nav-fragments.epub").unwrap();
    let titles: Vec<&str> = book.chapters().iter().map(|chapter| chapter.title.as_str()).collect();
    assert_eq!(titles, ["Cover", "Introduction", "Method", "Appendix"]);
}

#[test]
fn fragment_links_scope_section_content() {
    let book = miniepub::open("tests/data/nav-fragments.epub").unwrap();
    let introduction = book.chapter_at(1).unwrap();
    assert!(introduction.contains("Introduction paragraph."));
    assert!(!introduction.contains("Method paragraph."));
}

#[test]
fn spine_document_without_toc_entry_renders_whole() {
    let book = miniepub::open("tests/data/nav-fragments.epub").unwrap();
    let cover = book.chapter_at(0).unwrap();
    assert!(cover.contains("Cover page."));
}

#[test]
fn nav_hrefs_resolve_relative_to_nav_document() {
    let book = miniepub::open("tests/data/nav-subdir.epub").unwrap();
    let titles: Vec<&str> = book.chapters().iter().map(|chapter| chapter.title.as_str()).collect();
    assert_eq!(titles, ["Cover", "Story"]);
    assert!(book.chapter_at(1).unwrap().contains("Subdir story page."));
}

#[test]
fn all_renders_each_document_once() {
    let book = miniepub::open("tests/data/nav-fragments.epub").unwrap();
    let all = book.all().unwrap();
    assert_eq!(all.matches("Introduction paragraph.").count(), 1);
    assert!(all.contains("Method paragraph."));
    assert!(all.contains("Appendix paragraph."));
}
