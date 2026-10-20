//! A full-screen, `less`-like reader on the alternate screen.

use std::io::{self, Write};

use miniepub::{Book, Error};
use termimad::crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute, queue,
    style::Print,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use termimad::{Area, MadSkin, TextView};

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
        KeyCode::Esc | KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Char('c') if key.modifiers == KeyModifiers::CONTROL => Some(Action::Quit),
        KeyCode::Enter
        | KeyCode::Tab
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
    /// The screen area the chapter text may occupy.
    area: Area,
}

impl<'b> Viewer<'b> {
    /// Opens a viewer on the book's first chapter.
    ///
    /// # Errors
    ///
    /// Returns an error when the first chapter cannot be read.
    fn new(book: &'b Book, area: Area) -> Result<Self, Error> {
        let mut viewer = Self {
            book,
            index: 0,
            markdown: String::new(),
            scroll: 0,
            skin: MadSkin::default(),
            area,
        };
        viewer.load(0)?;
        Ok(viewer)
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
                self.scroll_lines(i32::from(self.area.height) * *pages);
            }
            Action::Top => self.scroll = 0,
            Action::Bottom => self.scroll = self.boundary(),
            Action::NextChapter => self.load(self.index.saturating_add(1))?,
            Action::PreviousChapter => self.load(self.index.saturating_sub(1))?,
            Action::Quit => {}
        }
        Ok(())
    }

    /// Loads the chapter `direction` chapters away when it exists.
    ///
    /// # Errors
    ///
    /// Returns an error when that chapter cannot be read.
    /// Draws the chapter text and the status line into `out`.
    ///
    /// # Errors
    ///
    /// Returns an error when the terminal cannot be written.
    fn render<W: Write>(&self, out: &mut W) -> io::Result<()> {
        queue!(out, Clear(ClearType::All))?;
        let text = self.skin.area_text(&self.markdown, &self.area);
        let mut view = TextView::from(&self.area, &text);
        view.set_scroll(self.scroll);
        view.write_on(out).map_err(io::Error::other)?;
        let boundary = text.lines.len().saturating_sub(self.area.height as usize);
        let percent = view.scroll * 100 / boundary.max(1);
        queue!(out, MoveTo(0, self.area.top + self.area.height), Print(self.status(percent)))?;
        out.flush()
    }

    /// Re-fits the area to the current screen after a resize.
    fn resize(&mut self) {
        self.area = screen_area();
    }

    /// The last scroll offset that puts chapter text in every row of the area.
    fn boundary(&self) -> usize {
        let content = self.skin.area_text(&self.markdown, &self.area).lines.len();
        content.saturating_sub(self.area.height as usize)
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
            " {} · {} / {} · {percent}% · ←/→ chapter · q quit",
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
/// # Errors
///
/// Returns an error when a chapter cannot be read or the terminal misbehaves.
pub fn run(book: &Book) -> Result<(), Error> {
    let mut viewer = Viewer::new(book, screen_area())?;
    let _guard = TerminalGuard;
    let mut out = io::stdout();
    queue!(out, EnterAlternateScreen)?;
    terminal::enable_raw_mode()?;
    queue!(out, Hide)?;
    loop {
        viewer.render(&mut out)?;
        match event::read()? {
            Event::Key(key) => match action_for(key) {
                Some(Action::Quit) => break,
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

    use super::{Action, Area, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, Viewer, action_for};

    const EPUB: &str = "../core/tests/data/multi-chapter.epub";
    /// A viewport taller than the chapter text, so it never scrolls.
    const PAGE: Area = Area { left: 0, top: 0, width: 80, height: 24 };
    /// A viewport shorter than the chapter text, so it scrolls.
    const SLIVER: Area = Area { left: 0, top: 0, width: 80, height: 2 };

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
    fn maps_quit_keys() {
        assert_eq!(action_for(plain(Char('q'))), Some(Action::Quit));
        assert_eq!(action_for(plain(Esc)), Some(Action::Quit));
        assert_eq!(action_for(pressed(Char('c'), KeyModifiers::CONTROL)), Some(Action::Quit));
        assert_eq!(action_for(plain(Char('c'))), None);
    }

    #[test]
    fn ignores_unused_keys() {
        let unused = [
            plain(Enter),
            plain(Tab),
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
    fn keeps_chapter_on_resize() {
        let book = miniepub::open(EPUB).unwrap();
        let mut viewer = Viewer::new(&book, SLIVER).unwrap();

        viewer.resize();
        assert!(viewer.area.width > 0);
        let rendered = render_chapter(&viewer);
        assert!(rendered.contains("This is the first chapter."));
        assert!(rendered.contains("1 / 3"));
    }
}
