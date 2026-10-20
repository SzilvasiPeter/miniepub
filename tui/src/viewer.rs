//! A full-screen, `less`-like reader on the alternate screen.

use std::collections::hash_map::DefaultHasher;
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use miniepub::{Book, Error};
use termimad::crossterm::style::{Attribute, Print, SetAttribute};
use termimad::crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute, queue,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use termimad::{Area, MadSkin, TextView};

/// The width of the chapter sidebar.
const SIDEBAR_WIDTH: u16 = 20;
/// The narrowest text pane that still makes the sidebar worth the room.
const MIN_TEXT_WIDTH: u16 = 50;

/// A sidebar row: the mark, then the title truncated to the pane width with an ellipsis.
fn sidebar_line(mark: &str, title: &str) -> String {
    truncate_to(&format!("{mark}{title}"), SIDEBAR_WIDTH as usize)
}

/// A line clamped to `width` display characters, ending with an ellipsis when cut.
fn truncate_to(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let cut: String = text.chars().take(width - 1).collect();
    format!("{cut}…")
}

/// Where a book was left off: the chapter index and the scroll offset inside it.
#[derive(Debug, Default, PartialEq, Eq)]
struct Progress {
    index: usize,
    scroll: usize,
}

impl Progress {
    /// Reads where the book was left off, defaulting to its beginning.
    fn load(base: Option<&Path>, book: &Path) -> Self {
        base.and_then(|base| fs::read_to_string(Self::state_file(base, book)).ok())
            .and_then(|state| parse_state(&state).map(|(index, scroll)| Self { index, scroll }))
            .unwrap_or_default()
    }

    /// Best-effort write of where the book was left off; no state directory drops it.
    fn save(self, base: Option<&Path>, book: &Path) -> io::Result<()> {
        let Some(base) = base else {
            return Ok(());
        };
        fs::create_dir_all(base.join("progress"))?;
        fs::write(Self::state_file(base, book), format!("{} {}", self.index, self.scroll))
    }

    /// The file holding this book's progress: the book path, hashed, inside `base`.
    fn state_file(base: &Path, book: &Path) -> PathBuf {
        let mut hasher = DefaultHasher::new();
        book.canonicalize().unwrap_or_else(|_| book.to_path_buf()).hash(&mut hasher);
        base.join("progress").join(format!("{:016x}", hasher.finish()))
    }
}

/// Parses the saved `index scroll` pair.
fn parse_state(state: &str) -> Option<(usize, usize)> {
    let mut words = state.split_whitespace().filter_map(|word| word.parse::<usize>().ok());
    Some((words.next()?, words.next()?))
}

/// The directory holding all reading positions, or `None` when no state home is known.
pub fn state_base() -> Option<PathBuf> {
    let xdg = env::var_os("XDG_STATE_HOME").filter(|dir| !dir.as_os_str().is_empty());
    let home_state = env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"));
    xdg.map(PathBuf::from).or(home_state).map(|dir| dir.join("miniepub"))
}

/// What a key press asks the viewer to do.
#[derive(Debug, PartialEq, Eq)]
enum Action {
    /// Scroll the chapter text by the given number of lines.
    ScrollLines(i32),
    /// Scroll the chapter text by the given number of pages.
    ScrollPages(i32),
    /// Jump to the top of the chapter.
    Top,
    /// Jump to the bottom of the chapter.
    Bottom,
    /// Open the next chapter.
    NextChapter,
    /// Open the previous chapter.
    PreviousChapter,
    /// Show or hide the chapter sidebar.
    ToggleSidebar,
    /// Close the viewer.
    Quit,
}

/// Maps a key press to the action it requests, or `None` when the key is not used.
fn action_for(key: KeyEvent) -> Option<Action> {
    if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
        return None;
    }
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => Some(Action::ScrollLines(-1)),
        KeyCode::Down | KeyCode::Char('j') => Some(Action::ScrollLines(1)),
        KeyCode::PageUp | KeyCode::Backspace => Some(Action::ScrollPages(-1)),
        KeyCode::PageDown | KeyCode::Char(' ') => Some(Action::ScrollPages(1)),
        KeyCode::Home | KeyCode::Char('g') => Some(Action::Top),
        KeyCode::End | KeyCode::Char('G') => Some(Action::Bottom),
        KeyCode::Left => Some(Action::PreviousChapter),
        KeyCode::Right => Some(Action::NextChapter),
        KeyCode::Tab => Some(Action::ToggleSidebar),
        KeyCode::Esc | KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Char('c') if key.modifiers == KeyModifiers::CONTROL => Some(Action::Quit),
        KeyCode::Enter
        | KeyCode::BackTab
        | KeyCode::Delete
        | KeyCode::Insert
        | KeyCode::Null
        | KeyCode::CapsLock
        | KeyCode::ScrollLock
        | KeyCode::NumLock
        | KeyCode::PrintScreen
        | KeyCode::Pause
        | KeyCode::Menu
        | KeyCode::KeypadBegin
        | KeyCode::F(_)
        | KeyCode::Char(_)
        | KeyCode::Media(_)
        | KeyCode::Modifier(_) => None,
    }
}

/// The visible state of the reader: which chapter, its text, and how far it is scrolled.
struct Viewer<'b> {
    book: &'b Book,
    /// The index of the displayed chapter into [`Book::chapters`].
    index: usize,
    /// The markdown of the displayed chapter.
    markdown: String,
    /// The scroll offset in rendered lines.
    scroll: usize,
    /// The styles used to turn markdown into terminal text.
    skin: MadSkin,
    /// The whole screen, minus the row reserved for the status line.
    screen: Area,
    /// Whether the chapter sidebar is wanted; it may still not fit the screen.
    sidebar_enabled: bool,
}

impl<'b> Viewer<'b> {
    /// Opens a viewer on the book's first chapter.
    ///
    /// # Errors
    ///
    /// Returns an error when the first chapter cannot be read.
    fn new(book: &'b Book, screen: Area) -> Result<Self, Error> {
        let mut viewer = Self {
            book,
            index: 0,
            markdown: String::new(),
            scroll: 0,
            skin: MadSkin::default(),
            screen,
            sidebar_enabled: true,
        };
        viewer.load(0)?;
        Ok(viewer)
    }

    /// Opens the book at where `progress` left off.
    ///
    /// # Errors
    ///
    /// Returns an error when the saved chapter cannot be read.
    fn resume(book: &'b Book, screen: Area, progress: &Progress) -> Result<Self, Error> {
        let mut viewer = Self::new(book, screen)?;
        viewer.index = progress.index.min(viewer.book.chapters().len().saturating_sub(1));
        viewer.load(viewer.index)?;
        viewer.scroll = progress.scroll.min(viewer.boundary());
        Ok(viewer)
    }

    /// The area the chapter list occupies, or `None` when hidden or too narrow.
    fn sidebar_area(&self) -> Option<Area> {
        let too_narrow = self.screen.width < SIDEBAR_WIDTH + 2 + MIN_TEXT_WIDTH;
        (self.sidebar_enabled && !too_narrow).then_some(Area {
            left: self.screen.left,
            top: self.screen.top,
            width: SIDEBAR_WIDTH,
            height: self.screen.height,
        })
    }

    /// The area the chapter text occupies: everything but the sidebar, with a margin column.
    fn text_area(&self) -> Area {
        let sidebar_width = match self.sidebar_area() {
            Some(_) => SIDEBAR_WIDTH + 2,
            None => 0,
        };
        let mut area = Area {
            left: self.screen.left + sidebar_width,
            top: self.screen.top,
            width: self.screen.width - sidebar_width,
            height: self.screen.height,
        };
        area.pad_for_max_width(100);
        area
    }

    /// Loads the markdown of the chapter at `index`; out of bounds leaves the viewer as is.
    ///
    /// # Errors
    ///
    /// Returns an error when the chapter cannot be read.
    fn load(&mut self, index: usize) -> Result<(), Error> {
        if self.book.chapters().get(index).is_none() {
            return Ok(());
        }
        let markdown = self.book.chapter_at(index)?;
        self.index = index;
        self.markdown = markdown;
        self.scroll = 0;
        Ok(())
    }

    /// Applies an action to the viewer state; navigation past the ends stays put.
    ///
    /// # Errors
    ///
    /// Returns an error when a chapter change cannot be read.
    fn apply(&mut self, action: &Action) -> Result<(), Error> {
        match action {
            Action::ScrollLines(lines) => self.scroll_lines(*lines),
            Action::ScrollPages(pages) => {
                self.scroll_lines(i32::from(self.text_area().height) * *pages);
            }
            Action::Top => self.scroll = 0,
            Action::Bottom => self.scroll = self.boundary(),
            Action::NextChapter => self.load(self.index.saturating_add(1))?,
            Action::PreviousChapter => self.load(self.index.saturating_sub(1))?,
            Action::ToggleSidebar => self.sidebar_enabled = !self.sidebar_enabled,
            Action::Quit => {}
        }
        Ok(())
    }

    /// Draws the chapter text and the status line into `out`.
    ///
    /// # Errors
    ///
    /// Returns an error when the terminal cannot be written.
    fn render<W: Write>(&self, out: &mut W) -> io::Result<()> {
        queue!(out, Clear(ClearType::All))?;
        self.draw_sidebar(out)?;
        let area = self.text_area();
        let text = self.skin.area_text(&self.markdown, &area);
        let mut view = TextView::from(&area, &text);
        view.set_scroll(self.scroll);
        view.write_on(out).map_err(io::Error::other)?;
        let boundary = text.lines.len().saturating_sub(area.height as usize);
        let percent = view.scroll * 100 / boundary.max(1);
        queue!(
            out,
            MoveTo(area.left, area.top + area.height),
            Print(truncate_to(&self.status(percent), area.width as usize))
        )?;
        out.flush()
    }

    /// Draws the chapter list and its separator column at the left of the screen.
    ///
    /// # Errors
    ///
    /// Returns an error when the terminal cannot be written.
    fn draw_sidebar<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let Some(sidebar) = self.sidebar_area() else {
            return Ok(());
        };
        let chapters = self.visible_chapters();
        for (row, chapter) in (0..sidebar.height).zip(chapters) {
            self.draw_sidebar_row(out, &sidebar, row, chapter)?;
        }
        for row in 0..sidebar.height {
            queue!(out, MoveTo(sidebar.left + sidebar.width, sidebar.top + row), Print("│"))?;
        }
        Ok(())
    }

    /// Draws one sidebar row, bolded when it shows the current chapter.
    fn draw_sidebar_row<W: Write>(
        &self,
        out: &mut W,
        sidebar: &Area,
        row: u16,
        chapter: usize,
    ) -> io::Result<()> {
        let marked = chapter == self.index;
        let mark = if marked { "▸ " } else { "· " };
        let attribute = if marked { Attribute::Bold } else { Attribute::Reset };
        let title = &self.book.chapters()[chapter].title;
        queue!(
            out,
            SetAttribute(attribute),
            MoveTo(sidebar.left, sidebar.top + row),
            Print(sidebar_line(mark, title))
        )
    }

    /// The chapters shown in the sidebar, half before and half after the current one.
    fn visible_chapters(&self) -> impl Iterator<Item = usize> + '_ {
        let count = self.book.chapters().len();
        let height = self.sidebar_area().map_or(1, |sidebar| sidebar.height as usize);
        let first = if count <= height {
            0
        } else {
            self.index.saturating_sub(height / 2).min(count - height)
        };
        (first..(first + height).min(count)).take(height)
    }

    /// Re-fits the area to the current screen after a resize.
    fn resize(&mut self) {
        self.screen = screen_area();
    }

    /// The last scroll offset that puts chapter text in every row of the area.
    fn boundary(&self) -> usize {
        let area = self.text_area();
        let content = self.skin.area_text(&self.markdown, &area).lines.len();
        content.saturating_sub(area.height as usize)
    }

    /// Moves the scroll by `lines`, clamped to the chapter text.
    fn scroll_lines(&mut self, lines: i32) {
        let boundary = self.boundary();
        let shift = lines.unsigned_abs().try_into().unwrap_or(usize::MAX);
        self.scroll = if lines < 0 {
            self.scroll.saturating_sub(shift)
        } else {
            self.scroll.saturating_add(shift).min(boundary)
        };
    }

    /// The bottom line of the viewer: chapter title, position, read progress, and hints.
    fn status(&self, percent: usize) -> String {
        let chapter = &self.book.chapters()[self.index];
        format!(
            " {} · {} / {} · {percent}% · Tab chapters · q quit",
            chapter.title,
            self.index + 1,
            self.book.chapters().len()
        )
    }
}

/// The available screen area, minus the row reserved for the status line.
fn screen_area() -> Area {
    let mut area = Area::full_screen();
    area.pad_for_max_width(100);
    // degenerate terminal sizes would overflow upstream in termimad
    area.width = area.width.max(1);
    area.height = area.height.max(2).saturating_sub(1);
    area
}

/// Restores the terminal when dropped, even on panic.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut out = io::stdout();
        drop(terminal::disable_raw_mode());
        drop(execute!(out, Show, LeaveAlternateScreen));
    }
}

/// Takes over the screen and runs the key loop until the viewer quits.
///
/// The reading position is restored from `state` when present, and written back on quit.
///
/// # Errors
///
/// Returns an error when a chapter cannot be read or the terminal misbehaves.
pub fn run(book_path: &Path, book: &Book, state: Option<&Path>) -> Result<(), Error> {
    let mut viewer = Viewer::resume(book, screen_area(), &Progress::load(state, book_path))?;
    let _guard = TerminalGuard;
    let mut out = io::stdout();
    queue!(out, EnterAlternateScreen)?;
    terminal::enable_raw_mode()?;
    queue!(out, Hide)?;
    loop {
        viewer.render(&mut out)?;
        match event::read()? {
            Event::Key(key) => match action_for(key) {
                Some(Action::Quit) => {
                    Progress { index: viewer.index, scroll: viewer.scroll }
                        .save(state, book_path)
                        .ok();
                    break;
                }
                Some(action) => viewer.apply(&action)?,
                None => {}
            },
            Event::Resize(..) => viewer.resize(),
            Event::FocusGained | Event::FocusLost | Event::Mouse(_) | Event::Paste(_) => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use termimad::crossterm::event::{KeyCode::*, KeyEventState, MediaKeyCode, ModifierKeyCode};

    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process;

    use super::{
        Action, Area, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, Progress, Viewer, action_for,
        sidebar_line, truncate_to,
    };

    const EPUB: &str = "../core/tests/data/multi-chapter.epub";
    /// A viewport taller than the chapter text, so it never scrolls.
    const PAGE: Area = Area { left: 0, top: 0, width: 80, height: 24 };
    /// A viewport shorter than the chapter text, so it scrolls.
    const SLIVER: Area = Area { left: 0, top: 0, width: 80, height: 2 };
    /// A screen wide enough to show the sidebar.
    const WIDE: Area = Area { left: 0, top: 0, width: 120, height: 20 };
    /// A screen too narrow to show the sidebar.
    const NARROW: Area = Area { left: 0, top: 0, width: 40, height: 10 };

    fn pressed(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    fn plain(code: KeyCode) -> KeyEvent {
        pressed(code, KeyModifiers::NONE)
    }

    fn released(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Release,
            state: KeyEventState::NONE,
        }
    }

    fn render_chapter(viewer: &Viewer<'_>) -> String {
        let mut buffer = Vec::new();
        viewer.render(&mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    }

    #[test]
    fn maps_scroll_keys() {
        assert_eq!(action_for(plain(Up)), Some(Action::ScrollLines(-1)));
        assert_eq!(action_for(plain(Down)), Some(Action::ScrollLines(1)));
        assert_eq!(action_for(plain(PageUp)), Some(Action::ScrollPages(-1)));
        assert_eq!(action_for(plain(PageDown)), Some(Action::ScrollPages(1)));
        assert_eq!(action_for(plain(Char(' '))), Some(Action::ScrollPages(1)));
        assert_eq!(action_for(plain(Backspace)), Some(Action::ScrollPages(-1)));
    }

    #[test]
    fn maps_jumps() {
        assert_eq!(action_for(plain(Home)), Some(Action::Top));
        assert_eq!(action_for(plain(Char('g'))), Some(Action::Top));
        assert_eq!(action_for(plain(End)), Some(Action::Bottom));
        assert_eq!(action_for(plain(Char('G'))), Some(Action::Bottom));
    }

    #[test]
    fn maps_chapter_keys() {
        assert_eq!(action_for(plain(Left)), Some(Action::PreviousChapter));
        assert_eq!(action_for(plain(Right)), Some(Action::NextChapter));
    }

    #[test]
    fn maps_toggle_key() {
        assert_eq!(action_for(plain(Tab)), Some(Action::ToggleSidebar));
    }

    #[test]
    fn maps_quit_keys() {
        assert_eq!(action_for(plain(Char('q'))), Some(Action::Quit));
        assert_eq!(action_for(plain(Esc)), Some(Action::Quit));
        assert_eq!(action_for(pressed(Char('c'), KeyModifiers::CONTROL)), Some(Action::Quit));
        assert_eq!(action_for(plain(Char('c'))), None);
    }

    #[test]
    fn hints_the_sidebar_toggle() {
        let book = miniepub::open(EPUB).unwrap();
        let mut viewer = Viewer::new(&book, PAGE).unwrap();

        assert!(render_chapter(&viewer).contains("Tab chapters"));
        viewer.apply(&Action::ToggleSidebar).unwrap();
        assert!(render_chapter(&viewer).contains("Tab chapters"));
    }

    #[test]
    fn truncates_long_status() {
        assert_eq!(truncate_to("short", 10), "short");
        assert_eq!(truncate_to("an overly long status line", 10).chars().count(), 10);
        assert!(truncate_to("an overly long status line", 10).ends_with('…'));
        assert_eq!(truncate_to("any", 0), "");
    }

    /// A throwaway state directory unique to this test and process.
    fn scratch(tag: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("miniepub-test-{tag}-{}", process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn progress_round_trips() {
        let base = scratch("roundtrip");
        let book = Path::new("books/island.epub");

        Progress { index: 2, scroll: 7 }.save(Some(&base), book).unwrap();
        assert_eq!(Progress::load(Some(&base), book), Progress { index: 2, scroll: 7 });
    }

    #[test]
    fn progress_defaults_without_a_saved_file() {
        let base = scratch("unsaved");

        assert_eq!(
            Progress::load(Some(&base), Path::new("any.epub")),
            Progress { index: 0, scroll: 0 }
        );
    }

    #[test]
    fn keeps_progress_per_book() {
        let base = scratch("perbook");
        let island = Path::new("books/island.epub");
        let river = Path::new("books/river.epub");

        Progress { index: 1, scroll: 2 }.save(Some(&base), island).unwrap();
        Progress { index: 3, scroll: 4 }.save(Some(&base), river).unwrap();
        assert_eq!(Progress::load(Some(&base), island), Progress { index: 1, scroll: 2 });
        assert_eq!(Progress::load(Some(&base), river), Progress { index: 3, scroll: 4 });
    }

    #[test]
    fn discards_an_unreadable_state_file() {
        let base = scratch("garbage");
        let book = Path::new("books/island.epub");
        let file = Progress::state_file(&base, book);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "not numbers").unwrap();

        assert_eq!(Progress::load(Some(&base), book), Progress { index: 0, scroll: 0 });
    }

    #[test]
    fn ignores_a_missing_state_directory() {
        let book = Path::new("x.epub");

        Progress { index: 5, scroll: 1 }.save(None, book).unwrap();
        assert_eq!(Progress::load(None, book), Progress { index: 0, scroll: 0 });
    }

    #[test]
    fn reopens_at_the_saved_position() {
        let book = miniepub::open(EPUB).unwrap();

        let viewer = Viewer::resume(&book, SLIVER, &Progress { index: 1, scroll: 1 }).unwrap();
        assert_eq!(viewer.index, 1);
        assert_eq!(viewer.scroll, 1);
    }

    #[test]
    fn clamps_a_stale_saved_position() {
        let book = miniepub::open(EPUB).unwrap();

        let viewer = Viewer::resume(&book, SLIVER, &Progress { index: 9, scroll: 9_999 }).unwrap();
        assert_eq!(viewer.index, 2);
        assert_eq!(viewer.scroll, viewer.boundary());
    }

    #[test]
    fn ignores_unused_keys() {
        let unused = [
            plain(Enter),
            plain(BackTab),
            plain(Delete),
            plain(Insert),
            plain(Null),
            plain(CapsLock),
            plain(ScrollLock),
            plain(NumLock),
            plain(PrintScreen),
            plain(Pause),
            plain(Menu),
            plain(KeypadBegin),
            plain(F(1)),
            plain(Media(MediaKeyCode::Play)),
            plain(Modifier(ModifierKeyCode::LeftShift)),
        ];
        for key in unused {
            assert_eq!(action_for(key), None);
        }
    }

    #[test]
    fn ignores_released_keys() {
        assert_eq!(action_for(released(Char('q'))), None);
        assert_eq!(action_for(released(Down)), None);
    }

    #[test]
    fn renders_first_chapter() {
        let book = miniepub::open(EPUB).unwrap();
        let viewer = Viewer::new(&book, PAGE).unwrap();

        let rendered = render_chapter(&viewer);
        assert!(rendered.contains("Chapter One"));
        assert!(rendered.contains("This is the first chapter."));
        assert!(rendered.contains("1 / 3"));
    }

    #[test]
    fn scrolls_within_the_chapter() {
        let book = miniepub::open(EPUB).unwrap();
        let mut viewer = Viewer::new(&book, SLIVER).unwrap();
        let top = render_chapter(&viewer);

        viewer.apply(&Action::ScrollLines(1)).unwrap();
        assert_ne!(render_chapter(&viewer), top);

        viewer.apply(&Action::ScrollLines(-1)).unwrap();
        assert_eq!(render_chapter(&viewer), top);
    }

    #[test]
    fn clamps_scrolling_to_the_chapter() {
        let book = miniepub::open(EPUB).unwrap();
        let mut viewer = Viewer::new(&book, SLIVER).unwrap();

        viewer.apply(&Action::ScrollPages(50)).unwrap();
        let clamped = viewer.scroll;
        viewer.apply(&Action::Bottom).unwrap();
        assert_eq!(viewer.scroll, clamped);
        assert!(viewer.scroll > 0);

        viewer.apply(&Action::ScrollLines(-1)).unwrap();
        assert!(viewer.scroll < clamped);

        viewer.apply(&Action::Top).unwrap();
        assert_eq!(viewer.scroll, 0);
    }

    #[test]
    fn resets_scroll_on_chapter_change() {
        let book = miniepub::open(EPUB).unwrap();
        let mut viewer = Viewer::new(&book, SLIVER).unwrap();
        viewer.apply(&Action::Bottom).unwrap();

        viewer.apply(&Action::NextChapter).unwrap();
        assert_eq!(viewer.scroll, 0);
    }

    #[test]
    fn advances_to_next_chapter() {
        let book = miniepub::open(EPUB).unwrap();
        let mut viewer = Viewer::new(&book, PAGE).unwrap();

        viewer.apply(&Action::NextChapter).unwrap();
        let rendered = render_chapter(&viewer);
        assert!(rendered.contains("This is the second chapter with"));
        assert!(rendered.contains("2 / 3"));
        assert!(!rendered.contains("This is the first chapter."));
    }

    #[test]
    fn stays_on_last_chapter_at_the_end() {
        let book = miniepub::open(EPUB).unwrap();
        let mut viewer = Viewer::new(&book, PAGE).unwrap();
        viewer.apply(&Action::NextChapter).unwrap();
        viewer.apply(&Action::NextChapter).unwrap();

        viewer.apply(&Action::NextChapter).unwrap();
        let rendered = render_chapter(&viewer);
        assert!(rendered.contains("3 / 3"));
        assert!(rendered.contains("This is the third chapter."));
    }

    #[test]
    fn steps_back_to_previous_chapter() {
        let book = miniepub::open(EPUB).unwrap();
        let mut viewer = Viewer::new(&book, PAGE).unwrap();
        viewer.apply(&Action::NextChapter).unwrap();

        viewer.apply(&Action::PreviousChapter).unwrap();
        let rendered = render_chapter(&viewer);
        assert!(rendered.contains("This is the first chapter."));
        assert!(rendered.contains("1 / 3"));
    }

    #[test]
    fn stays_on_first_chapter_at_the_start() {
        let book = miniepub::open(EPUB).unwrap();
        let mut viewer = Viewer::new(&book, PAGE).unwrap();

        viewer.apply(&Action::PreviousChapter).unwrap();
        let rendered = render_chapter(&viewer);
        assert!(rendered.contains("1 / 3"));
        assert!(rendered.contains("This is the first chapter."));
    }

    #[test]
    fn truncates_long_titles() {
        let overflowing = "A chapter title much longer than the pane";
        let line = sidebar_line("· ", overflowing);
        assert_eq!(line.chars().count(), 20);
        assert!(line.ends_with('…'));
        assert_eq!(sidebar_line("· ", "Chapter Two"), "· Chapter Two");
    }

    #[test]
    fn keeps_chapter_on_resize() {
        let book = miniepub::open(EPUB).unwrap();
        let mut viewer = Viewer::new(&book, SLIVER).unwrap();

        viewer.resize();
        assert!(viewer.screen.width > 0);
        let rendered = render_chapter(&viewer);
        assert!(rendered.contains("This is the first chapter."));
        assert!(rendered.contains("1 / 3"));
    }

    #[test]
    fn lists_chapters_in_the_sidebar() {
        let book = miniepub::open(EPUB).unwrap();
        let viewer = Viewer::new(&book, WIDE).unwrap();

        let rendered = render_chapter(&viewer);
        assert!(rendered.contains("▸ Chapter One"));
        assert!(rendered.contains("· Chapter Two"));
        assert!(rendered.contains("· Chapter Three"));
        assert!(rendered.contains("│"));
    }

    #[test]
    fn marks_the_current_chapter() {
        let book = miniepub::open(EPUB).unwrap();
        let mut viewer = Viewer::new(&book, WIDE).unwrap();

        assert!(render_chapter(&viewer).contains("▸ Chapter One"));
        viewer.apply(&Action::NextChapter).unwrap();
        let rendered = render_chapter(&viewer);
        assert!(rendered.contains("▸ Chapter Two"));
        assert!(!rendered.contains("▸ Chapter One"));
    }

    #[test]
    fn hides_the_sidebar_on_a_narrow_screen() {
        let book = miniepub::open(EPUB).unwrap();
        let viewer = Viewer::new(&book, NARROW).unwrap();

        assert_eq!(viewer.text_area().width, 40);
        let rendered = render_chapter(&viewer);
        assert!(!rendered.contains("· Chapter"));
        assert!(!rendered.contains("│"));
    }

    #[test]
    fn toggles_the_sidebar() {
        let book = miniepub::open(EPUB).unwrap();
        let mut viewer = Viewer::new(&book, WIDE).unwrap();

        viewer.apply(&Action::ToggleSidebar).unwrap();
        let rendered = render_chapter(&viewer);
        assert!(!rendered.contains("· Chapter"));
        assert!(!rendered.contains("│"));

        viewer.apply(&Action::ToggleSidebar).unwrap();
        let rendered = render_chapter(&viewer);
        assert!(rendered.contains("▸ Chapter One"));
        assert!(rendered.contains("· Chapter Two"));
    }

    #[test]
    fn scrolls_the_sidebar_window() {
        let book = miniepub::open(EPUB).unwrap();
        // two sidebar rows for three chapters: the window must follow the current one
        let slim = Area { left: 0, top: 0, width: 120, height: 2 };
        let mut viewer = Viewer::new(&book, slim).unwrap();

        let rendered = render_chapter(&viewer);
        assert!(rendered.contains("▸ Chapter One"));
        assert!(rendered.contains("· Chapter Two"));
        assert!(!rendered.contains("Chapter Three"));

        viewer.apply(&Action::NextChapter).unwrap();
        let rendered = render_chapter(&viewer);
        assert!(rendered.contains("▸ Chapter Two"));
        assert!(!rendered.contains("▸ Chapter One"));

        viewer.apply(&Action::NextChapter).unwrap();
        let rendered = render_chapter(&viewer);
        assert!(rendered.contains("▸ Chapter Three"));
        assert!(!rendered.contains("Chapter One"));
    }
}
