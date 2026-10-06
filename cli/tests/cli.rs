//! Integration tests for CLI tool.

#![allow(clippy::unwrap_used)]
use assert_cmd::Command;
use predicates::str::contains;

const EPUB: &str = "../core/tests/data/multi-chapter.epub";
const EPUB_SINGLE: &str = "../core/tests/data/minimal-v3.epub";

#[test]
fn list_chapters_works() {
    Command::cargo_bin("miniepub")
        .unwrap()
        .arg("list-chapters")
        .arg(EPUB)
        .assert()
        .success()
        .stdout(contains("0: Chapter One"))
        .stdout(contains("1: Chapter Two"))
        .stdout(contains("2: Chapter Three"));
}

#[test]
fn read_chapter_works() {
    Command::cargo_bin("miniepub")
        .unwrap()
        .arg("read-chapter")
        .arg(EPUB)
        .arg("1")
        .assert()
        .success()
        .stdout(contains("This is the second chapter"));

    Command::cargo_bin("miniepub")
        .unwrap()
        .arg("read-chapter")
        .arg(EPUB)
        .arg("0")
        .assert()
        .success()
        .stdout(contains("This is the first chapter"));
}

#[test]
fn read_chapter_out_of_bounds_returns_error() {
    Command::cargo_bin("miniepub")
        .unwrap()
        .arg("read-chapter")
        .arg(EPUB)
        .arg("99")
        .assert()
        .failure()
        .stderr(contains("out of bounds"));
}

#[test]
fn full_works() {
    Command::cargo_bin("miniepub")
        .unwrap()
        .arg("full")
        .arg(EPUB)
        .assert()
        .success()
        .stdout(contains("This is the first chapter."));
}

#[test]
fn full_single_chapter_works() {
    Command::cargo_bin("miniepub")
        .unwrap()
        .arg("full")
        .arg(EPUB_SINGLE)
        .assert()
        .success()
        .stdout(contains("This is a paragraph."));
}

#[test]
fn unknown_command_returns_error() {
    let output = Command::cargo_bin("miniepub").unwrap().arg("unknown").arg(EPUB).output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown command"));
}

#[test]
fn missing_args_returns_usage() {
    let output = Command::cargo_bin("miniepub").unwrap().arg("list-chapters").output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Usage: miniepub"));
}
