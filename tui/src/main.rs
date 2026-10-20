//! A minimal EPUB reader TUI.

#![forbid(unsafe_code)]
// TODO: crossterm 0.29 (pulled by termimad) pins derive-more v2, which uses syn v2, while the rest of the termimad stack uses syn v3.
// remove allow lint after the https://github.com/JelteF/derive_more/pull/567 PR is solved.
#![allow(clippy::multiple_crate_versions)]
use std::env;
use std::io;
use std::io::IsTerminal;
use std::process;

pub mod viewer;

fn main() {
    let Some(path) = env::args().nth(1) else {
        eprintln!("Usage: miniepub-tui <epub-file>");
        process::exit(1);
    };
    if !io::stdout().is_terminal() {
        eprintln!("miniepub-tui requires a terminal");
        process::exit(1);
    }
    if let Err(err) = run(&path) {
        eprintln!("Error: {err}");
        process::exit(1);
    }
}

fn run(path: &str) -> Result<(), miniepub::Error> {
    use std::path::Path;

    let book = miniepub::open(path)?;
    viewer::run(Path::new(path), &book, viewer::state_base().as_deref())
}
