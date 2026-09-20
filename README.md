# miniepub

A minimal EPUB reader library for Rust. Parses a ZIP of XML files into structured chapters.

## Usage

```toml
[dependencies]
miniepub = "0.1"
```

```rust
let book = miniepub::open("book.epub")?;
for chapter in book.chapters() {
    println!("{}", chapter.title());
    println!("{}", chapter.body());
}
```

## License

MIT OR Apache-2.0

test ci cancel
