//! The embedded terminal: runs a package-transaction command on a
//! pseudo-terminal pacpad itself owns, instead of leaving pacpad's own
//! screen the way an earlier version of this project did.
//!
//! This is what makes `sudo`'s password prompt, pacman's provider/
//! conflict/`.pacnew` prompts, and yay's PKGBUILD-review prompt all
//! keep working correctly: the child gets a *real* pty (not a pipe),
//! so it never knows it isn't talking to a real terminal -- pacpad
//! just relays raw keystrokes into it (see `encode_key`) and parses
//! its output (via the `vt100` crate) into a plain grid snapshot
//! (`RunningView`) for `ui::running` to render inside pacpad's own
//! frame.
//!
//! Reading happens on a dedicated background thread (a pty read blocks
//! until output arrives, which the main render loop can't afford to
//! do) that feeds bytes straight into a shared, mutex-guarded
//! `vt100::Parser`; the main loop only ever takes a cheap read lock to
//! copy out a snapshot once per frame. Writing (forwarding the user's
//! keystrokes) happens directly on the main thread, since a pty write
//! is never expected to block for meaningfully long.

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

use super::plan::PlannedCommand;

/// A plain-data snapshot of the embedded terminal's visible grid --
/// deliberately as ratatui-agnostic as every other piece of
/// `AppState` (see `app/state.rs`'s own module docs): `ui::running` is
/// the only place that turns this into styled `Line`s.
pub struct RunningView {
    pub title: String,
    pub rows: Vec<Vec<TermCell>>,
    /// `(row, col)` within the grid, or `None` while the child has
    /// hidden the cursor (as `less` does, for instance).
    pub cursor: Option<(u16, u16)>,
    /// `None` while still running; `Some(true)` / `Some(false)` once
    /// the child has exited, for success/failure.
    pub finished: Option<bool>,
}

#[derive(Clone)]
pub struct TermCell {
    pub ch: char,
    pub fg: CellColor,
    pub bg: CellColor,
    pub bold: bool,
    pub underline: bool,
    pub inverse: bool,
}

impl TermCell {
    fn blank() -> Self {
        TermCell {
            ch: ' ',
            fg: CellColor::Default,
            bg: CellColor::Default,
            bold: false,
            underline: false,
            inverse: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CellColor {
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

fn convert_color(c: vt100::Color) -> CellColor {
    match c {
        vt100::Color::Default => CellColor::Default,
        vt100::Color::Idx(i) => CellColor::Indexed(i),
        vt100::Color::Rgb(r, g, b) => CellColor::Rgb(r, g, b),
    }
}

pub struct PtySession {
    child: Box<dyn Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    parser: Arc<Mutex<vt100::Parser>>,
    title: String,
    /// Cached once observed: some backends only report a `Child`'s
    /// exit status once, so a later `poll_exit` must keep returning
    /// the same answer rather than asking again.
    finished: Option<bool>,
    // Never read again after spawn -- kept only so the thread isn't
    // detached (and thus doesn't outlive `PtySession` in spirit, even
    // though it exits on its own once the child closes its end of the
    // pty).
    _reader_thread: std::thread::JoinHandle<()>,
}

impl PtySession {
    /// Spawns `cmd` on a fresh pty sized `rows` x `cols` -- callers
    /// size this to match the pane they're about to render it into,
    /// so the child never wraps output at the wrong width.
    pub fn spawn(cmd: &PlannedCommand, rows: u16, cols: u16) -> anyhow::Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut builder = CommandBuilder::new(&cmd.program);
        builder.args(&cmd.args);

        let child = pair.slave.spawn_command(builder)?;
        // The child now holds its own copy of the slave fd; dropping
        // ours is what lets the master's reader see EOF when the
        // child exits, instead of the pty staying open forever
        // because pacpad itself still has the slave open too.
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        // No scrollback kept (`0`): the visible grid is all `ui::running`
        // ever renders, and pacman/yay/less output doesn't need pacpad
        // to also be a terminal-history browser.
        let parser = Arc::new(Mutex::new(vt100::Parser::new(rows, cols, 0)));

        let reader_parser = Arc::clone(&parser);
        let reader_thread = std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if let Ok(mut parser) = reader_parser.lock() {
                            parser.process(&buf[..n]);
                        } else {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(PtySession {
            child,
            writer,
            master: pair.master,
            parser,
            title: cmd.display.clone(),
            finished: None,
            _reader_thread: reader_thread,
        })
    }

    /// Non-blocking. Returns the child's outcome once it has actually
    /// exited; `None` while still running.
    pub fn poll_exit(&mut self) -> Option<bool> {
        if self.finished.is_none() {
            if let Ok(Some(status)) = self.child.try_wait() {
                self.finished = Some(status.success());
            }
        }
        self.finished
    }

    /// Forwards raw bytes to the child's stdin -- see `encode_key` for
    /// how a keypress becomes these bytes. Best-effort: a write
    /// failing (e.g. the child already closed its end) isn't a reason
    /// to error the whole session out from under the render loop.
    pub fn write_input(&mut self, bytes: &[u8]) {
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
    }

    /// Propagates a real terminal resize to both the pty (so the
    /// child's own `SIGWINCH` handling sees it) and the parser (so
    /// snapshots immediately reflect the new dimensions rather than
    /// waiting for the child to redraw).
    pub fn resize(&mut self, rows: u16, cols: u16) {
        let _ = self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
        if let Ok(mut parser) = self.parser.lock() {
            parser.screen_mut().set_size(rows, cols);
        }
    }

    /// A cheap, self-contained copy of the current grid -- see the
    /// module docs for why this (and not the `vt100::Parser` itself)
    /// is what `AppState` holds.
    pub fn snapshot(&self) -> RunningView {
        let guard = match self.parser.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        let screen = guard.screen();
        let (rows, cols) = screen.size();

        let mut grid = Vec::with_capacity(rows as usize);
        for r in 0..rows {
            let mut row = Vec::with_capacity(cols as usize);
            for c in 0..cols {
                row.push(match screen.cell(r, c) {
                    Some(cell) => TermCell {
                        ch: cell.contents().chars().next().unwrap_or(' '),
                        fg: convert_color(cell.fgcolor()),
                        bg: convert_color(cell.bgcolor()),
                        bold: cell.bold(),
                        underline: cell.underline(),
                        inverse: cell.inverse(),
                    },
                    None => TermCell::blank(),
                });
            }
            grid.push(row);
        }

        let cursor = (!screen.hide_cursor()).then(|| screen.cursor_position());

        RunningView {
            title: self.title.clone(),
            rows: grid,
            cursor,
            finished: self.finished,
        }
    }
}

/// Encodes a key event back into the raw bytes a real terminal would
/// have sent -- the inverse of what crossterm's own key-event decoding
/// does, needed because the embedded child (sudo, pacman, yay, less)
/// reads raw bytes, not `KeyEvent`s. Covers the subset those programs
/// actually rely on: printable text, line editing (Enter/Backspace/
/// Tab/Esc), arrow/navigation keys (for `less` and shell line editing),
/// and Ctrl+letter control codes (Ctrl+C to interrupt the child,
/// Ctrl+D for EOF, ...). Anything outside that returns an empty vec
/// rather than guessing.
pub fn encode_key(key: &KeyEvent) -> Vec<u8> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        if let KeyCode::Char(c) = key.code {
            let lower = c.to_ascii_lowercase();
            if lower.is_ascii_lowercase() {
                return vec![(lower as u8) - b'a' + 1];
            }
        }
    }

    match key.code {
        KeyCode::Char(c) => c.to_string().into_bytes(),
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn key_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn printable_chars_encode_as_their_own_utf8_bytes() {
        assert_eq!(encode_key(&key(KeyCode::Char('a'))), b"a");
        assert_eq!(encode_key(&key(KeyCode::Char('Z'))), b"Z");
        assert_eq!(encode_key(&key(KeyCode::Char('é'))), "é".as_bytes());
    }

    #[test]
    fn control_letters_encode_as_the_matching_control_code() {
        assert_eq!(
            encode_key(&key_mod(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            vec![0x03],
            "Ctrl+C must reach the child so it can interrupt itself"
        );
        assert_eq!(
            encode_key(&key_mod(KeyCode::Char('d'), KeyModifiers::CONTROL)),
            vec![0x04]
        );
        assert_eq!(
            encode_key(&key_mod(KeyCode::Char('a'), KeyModifiers::CONTROL)),
            vec![0x01]
        );
    }

    #[test]
    fn editing_and_navigation_keys_use_their_standard_escape_sequences() {
        assert_eq!(encode_key(&key(KeyCode::Enter)), vec![b'\r']);
        assert_eq!(encode_key(&key(KeyCode::Backspace)), vec![0x7f]);
        assert_eq!(encode_key(&key(KeyCode::Tab)), vec![b'\t']);
        assert_eq!(encode_key(&key(KeyCode::Esc)), vec![0x1b]);
        assert_eq!(encode_key(&key(KeyCode::Up)), b"\x1b[A");
        assert_eq!(encode_key(&key(KeyCode::Down)), b"\x1b[B");
        assert_eq!(encode_key(&key(KeyCode::Left)), b"\x1b[D");
        assert_eq!(encode_key(&key(KeyCode::Right)), b"\x1b[C");
    }

    #[test]
    fn unmapped_keys_produce_nothing_rather_than_a_guess() {
        assert!(encode_key(&key(KeyCode::F(5))).is_empty());
    }

    fn trivial(program: &str, args: &[&str]) -> PlannedCommand {
        PlannedCommand {
            program: program.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            display: format!("{program} {}", args.join(" ")),
        }
    }

    /// Polls `f` until it returns `Some`, or panics after a generous
    /// timeout -- the reader thread updates the shared parser
    /// asynchronously, so a snapshot immediately after spawn can
    /// legitimately still be empty.
    fn wait_for<T>(mut f: impl FnMut() -> Option<T>) -> T {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(v) = f() {
                return v;
            }
            assert!(Instant::now() < deadline, "timed out waiting for condition");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn a_real_child_s_output_shows_up_in_the_snapshot() {
        let mut session =
            PtySession::spawn(&trivial("/bin/sh", &["-c", "printf hello"]), 24, 80).expect("spawn");

        wait_for(|| {
            let view = session.snapshot();
            let text: String = view.rows[0].iter().map(|c| c.ch).collect();
            text.contains("hello").then_some(())
        });

        wait_for(|| session.poll_exit());
        assert_eq!(session.snapshot().finished, Some(true));
    }

    #[test]
    fn a_nonzero_exit_is_reported_as_a_failure() {
        let mut session =
            PtySession::spawn(&trivial("/bin/sh", &["-c", "exit 7"]), 24, 80).expect("spawn");
        let outcome = wait_for(|| session.poll_exit());
        assert!(!outcome);
    }

    #[test]
    fn keystrokes_written_in_reach_the_child_s_stdin() {
        // `cat` echoes stdin back to stdout; feeding it a line proves
        // `write_input` actually reaches the child rather than being
        // silently dropped.
        let mut session = PtySession::spawn(&trivial("/bin/cat", &[]), 24, 80).expect("spawn");
        session.write_input(b"marco\r");

        wait_for(|| {
            let view = session.snapshot();
            let text: String = view
                .rows
                .iter()
                .flat_map(|r| r.iter().map(|c| c.ch))
                .collect();
            text.contains("marco").then_some(())
        });

        // `cat` only exits on EOF (Ctrl+D), not after one line.
        session.write_input(&[0x04]);
        wait_for(|| session.poll_exit());
    }
}
