//! Integration tests for the TUI viewer.

#![allow(clippy::unwrap_used, clippy::multiple_crate_versions)]
use assert_cmd::Command;

const EPUB: &str = "../core/tests/data/multi-chapter.epub";

#[test]
fn requires_a_terminal() {
    let output = Command::cargo_bin("miniepub-tui").unwrap().arg(EPUB).output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("requires a terminal"));
}

#[test]
fn missing_args_returns_usage() {
    let output = Command::cargo_bin("miniepub-tui").unwrap().output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Usage: miniepub-tui"));
}
