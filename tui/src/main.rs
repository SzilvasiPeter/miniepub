//! A minimal EPUB reader TUI.

#![forbid(unsafe_code)]
// TODO: crossterm 0.29 (pulled by termimad) pins derive-more v2, which uses syn v2, while the rest of the termimad stack uses syn v3.
// remove allow lint after the https://github.com/JelteF/derive_more/pull/567 PR is solved.
#![allow(clippy::multiple_crate_versions)]
use std::env;
use std::process;

fn main() {
    let Some(path) = env::args().nth(1) else {
        eprintln!("Usage: miniepub-tui <epub-file>");
        process::exit(1);
    };
    if let Err(err) = render(&path) {
        eprintln!("Error: {err}");
        process::exit(1);
    }
}

fn render(path: &str) -> Result<(), miniepub::Error> {
    let book = miniepub::open(path)?;
    termimad::print_text(&book.all()?);
    Ok(())
}
