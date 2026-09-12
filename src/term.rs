//! Raw-mode / alternate-screen / mouse-capture lifecycle for the
//! interactive TUI.
//!
//! `TerminalGuard::enter()` puts the terminal into the state ratatui
//! needs; `Drop` restores it unconditionally, including on panic (via
//! a panic hook installed once at startup) -- a TUI that dies mid-draw
//! and leaves the user's shell in raw mode with no visible cursor is
//! the single worst failure mode this binary can have, worse than any
//! crash with a backtrace.
//!
//! This is deliberately the *only* place that enters this state.
//! `txn::run`'s terminal handover (leaving it temporarily to hand the
//! tty to `sudo pacman`) uses the same three crossterm primitives
//! directly rather than duplicating a second enable/disable sequence
//! here -- see its module docs for why mouse capture in particular
//! matters there.

use std::io::{self, Stdout};
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

static PANIC_HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);

/// Owns the terminal for as long as it's alive. Construct once at
/// startup, drop it (or let it fall out of scope) before printing
/// anything else to stdout or exiting.
pub struct TerminalGuard {
    pub terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalGuard {
    pub fn enter() -> io::Result<Self> {
        install_panic_hook();
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let terminal = Terminal::new(CrosstermBackend::new(stdout))?;
        Ok(TerminalGuard { terminal })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // Best-effort: a failure to restore here has nowhere useful to
        // report to (we may already be unwinding from a panic), so
        // each step is attempted independently rather than short-
        // circuiting on the first error.
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
    }
}

/// Installed once, the first time a `TerminalGuard` is created. A
/// panic inside the render/update loop would otherwise unwind past
/// the guard's `Drop`... except `Drop` *does* still run during a
/// panic's unwind in the normal case, so this hook's real job is the
/// rarer case: restoring the terminal even if the panic prints its
/// backtrace before that unwind reaches us, so the message is legible
/// instead of scrambled by leftover raw-mode/alt-screen state.
fn install_panic_hook() {
    if PANIC_HOOK_INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
        default_hook(info);
    }));
}
