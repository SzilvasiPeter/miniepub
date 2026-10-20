# miniepub-tui

A minimal EPUB reader terminal user interface. Opens the book in a full-screen, `less`-like viewer.

## Usage

```sh
miniepub-tui <epub-file>
```

The viewer takes over the terminal (alternate screen + raw mode) and shows one chapter at a time, wrapped to the terminal width. On exit the original screen is restored — even after errors or a panic.

## Keys

| Key | Action |
| --- | --- |
| `↑` / `↓` (or `k` / `j`) | scroll one line |
| `PgUp` / `PgDn`, `Backspace` / `Space` | scroll one page |
| `Home` / `End` (or `g` / `G`) | top / bottom of the chapter |
| `←` / `→` | previous / next chapter |
| `q`, `Esc`, `Ctrl-C` | quit |

The status line at the bottom shows the chapter title, position, and read progress.

The viewer requires a terminal; running it with a piped or redirected stdout exits with an error instead.
