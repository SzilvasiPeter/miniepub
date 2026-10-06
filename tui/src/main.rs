//! A minimal EPUB reader TUI.

#![forbid(unsafe_code)]
use std::env;
use std::process;

fn main() {
    let Some(path) = env::args().nth(1) else {
        eprintln!("Usage: miniepub-tui <epub-file>");
        process::exit(1);
    };
    let book = match miniepub::open(&path) {
        Ok(book) => book,
        Err(err) => {
            eprintln!("Error: {err}");
            process::exit(1);
        }
    };
    println!(
        "{} chapters loaded from {path}; the interactive TUI is not implemented yet.",
        book.chapters().len()
    );
}
