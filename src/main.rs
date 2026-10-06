//! A minimal EPUB reader CLI.

#![forbid(unsafe_code)]
use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        print_usage();
        process::exit(1);
    }

    let command = args[1].as_str();
    let path = &args[2];

    let result = match command {
        "list-chapters" => list_chapters(path),
        "read-chapter" => {
            if args.len() < 4 {
                eprintln!("Error: read-chapter requires a chapter index");
                print_usage();
                process::exit(1);
            }
            let index: usize = args[3].parse().unwrap_or_else(|_| {
                eprintln!("Error: chapter index must be a number");
                process::exit(1);
            });
            read_chapter(path, index)
        }
        "full" => full(path),
        _ => {
            eprintln!("Error: unknown command '{command}'");
            print_usage();
            process::exit(1);
        }
    };

    if let Err(err) = result {
        eprintln!("Error: {err}");
        process::exit(1);
    }
}

fn print_usage() {
    eprintln!("Usage: miniepub <command> <epub-file> [args]");
    eprintln!("Commands:");
    eprintln!("  list-chapters <file>       List all chapter titles");
    eprintln!("  read-chapter <file> <n>    Print chapter n content");
    eprintln!("  full <file>                Print all content");
}

fn list_chapters(path: &str) -> Result<(), miniepub::Error> {
    let book = miniepub::open(path)?;
    for (index, chapter) in book.chapters().iter().enumerate() {
        println!("{index}: {}", chapter.title);
    }
    Ok(())
}

fn read_chapter(path: &str, index: usize) -> Result<(), miniepub::Error> {
    let book = miniepub::open(path)?;
    println!("{}", book.chapter_at(index)?);
    Ok(())
}

fn full(path: &str) -> Result<(), miniepub::Error> {
    let book = miniepub::open(path)?;
    println!("{}", book.all()?);
    Ok(())
}
