# miniepub-tui

A minimal EPUB reader terminal user interface. Opens the book in a full-screen, `less`-like viewer.

## Usage

```sh
miniepub-tui <epub-file>
```

The viewer takes over the terminal (alternate screen + raw mode) and shows one chapter at a time, wrapped to the terminal width. Quitting updates the reading position, so reopening the book continues where you left off — even down to the scroll offset. On exit the original screen is restored — even after errors or a panic.

## Keys

| Key | Action |
| --- | --- |
| `↑` / `↓` (or `k` / `j`) | scroll one line |
| `PgUp` / `PgDn`, `Backspace` / `Space` | scroll one page |
| `Home` / `End` (or `g` / `G`) | top / bottom of the chapter |
| `←` / `→` | previous / next chapter |
| `Tab` | show or hide the chapter sidebar |
| `q`, `Esc`, `Ctrl-C` | quit |

The status line under the text shows the chapter title, position, and read progress.

A chapter sidebar is drawn at the left, listing the chapter titles with the current one marked `▸` in bold. Press `Tab` to hide or show it whenever you want the extra reading space; on very narrow terminals it stays hidden and the text uses the full width.

The viewer requires a terminal; running it with a piped or redirected stdout exits with an error instead.
