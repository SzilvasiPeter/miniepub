# miniepub

[![crates.io](https://img.shields.io/crates/v/miniepub)](https://crates.io/crates/miniepub)
[![deps.rs](https://deps.rs/repo/github/SzilvasiPeter/miniepub/status.svg)](https://deps.rs/repo/github/SzilvasiPeter/miniepub)
[![coverage](https://img.shields.io/endpoint?url=https://szilvasipeter.github.io/miniepub/badge.json)](https://szilvasipeter.github.io/miniepub/html/index.html)
![forbids-unsafe](https://img.shields.io/badge/forbids-unsafe-blue)
![ci](https://github.com/SzilvasiPeter/miniepub/actions/workflows/ci.yml/badge.svg)
![cd](https://github.com/SzilvasiPeter/miniepub/actions/workflows/cd.yml/badge.svg)

A minimal EPUB reader library for Rust. Parses a ZIP of XML files into structured chapters.

## Usage

```toml
[dependencies]
miniepub = "0.1"
```

```rust
let mut book = miniepub::open("book.epub")?;

// Table of contents
for chapter in book.chapters() {
    println!("{}", chapter.title);
}

// Read the current chapter as markdown
println!("{}", book.markdown()?);

// Navigate
book.next_chapter()?;    // next chapter
book.previous_chapter()?; // previous chapter
book.navigate(2)?;       // jump to chapter 2
println!("{}", book.all()?); // whole book as markdown
```

## License

MIT
