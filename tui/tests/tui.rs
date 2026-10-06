//! Integration tests for the TUI placeholder.

#![allow(clippy::unwrap_used)]
use assert_cmd::Command;
use predicates::str::contains;

const EPUB: &str = "../core/tests/data/multi-chapter.epub";

#[test]
fn opens_book_and_reports_placeholder() {
    Command::cargo_bin("miniepub-tui")
        .unwrap()
        .arg(EPUB)
        .assert()
        .success()
        .stdout(contains("3 chapters"))
        .stdout(contains("not implemented"));
}

#[test]
fn missing_args_returns_usage() {
    let output = Command::cargo_bin("miniepub-tui").unwrap().output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Usage: miniepub-tui"));
}
