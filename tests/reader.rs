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
