//! Integration tests for the TUI markdown rendering.

#![allow(clippy::unwrap_used, clippy::multiple_crate_versions)]
use assert_cmd::Command;
use predicates::str::contains;

const EPUB: &str = "../core/tests/data/multi-chapter.epub";

#[test]
fn opens_book_and_renders_content() {
    Command::cargo_bin("miniepub-tui")
        .unwrap()
        .arg(EPUB)
        .assert()
        .success()
        .stdout(contains("Chapter One"))
        .stdout(contains("This is the first chapter."));
}

#[test]
fn missing_args_returns_usage() {
    let output = Command::cargo_bin("miniepub-tui").unwrap().output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Usage: miniepub-tui"));
}
