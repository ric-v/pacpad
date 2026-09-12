//! The startup splash: a small Pac-Man chomping across a row of dots,
//! revealing the word "pacpad" as it passes -- the animation the
//! design canvas sketched out for the "make it fun" ask, actually
//! wired into the real TUI instead of only existing as a mockup.
//!
//! Pure data in, pure data out (`frame_at`) so the chomp math is
//! tested without a `ratatui::Frame` at all; `render` is the only
//! part that touches ratatui, and it does nothing but lay out
//! whatever `frame_at` already computed.

use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::theme;

const DOTS: usize = 10;
const WORD: &str = "pacpad";

/// Total columns in the strip: the dot run, the word, and one extra
/// trailing column for Pac-Man to walk onto once he's passed the last
/// letter -- without it, `pos` would clamp right on top of that
/// letter forever and it could never fully render as plain text (only
/// ever "under" the glyph), the same way a real pellet disappears the
/// instant he reaches it rather than staying half-visible beneath him.
fn total_cols() -> usize {
    DOTS + WORD.len() + 1
}

/// One rendered frame of the chomp: `before` is everything Pac-Man has
/// already passed (dots eaten -> blank, word letters -> revealed),
/// `after` is everything still ahead of him (dots still `·`, word
/// letters still hidden), and `mouth_open` alternates each tick.
pub struct SplashFrame {
    pub before: String,
    pub mouth_open: bool,
    pub after: String,
}

/// `tick` is ticks elapsed since the splash started (`state.tick`
/// directly -- a splash always starts at tick 0, per
/// `AppState::splash_until_tick`'s doc comment). Pac-Man advances one
/// column per tick; once he reaches the end of the strip, further
/// ticks just hold on the fully-revealed word rather than panicking or
/// wrapping.
pub fn frame_at(tick: u64) -> SplashFrame {
    let total = total_cols();
    let pos = (tick as usize).min(total.saturating_sub(1));

    let before: String = (0..pos).map(passed_char).collect();
    let after: String = (pos + 1..total).map(upcoming_char).collect();

    SplashFrame {
        before,
        mouth_open: tick % 2 == 0,
        after,
    }
}

/// A column Pac-Man has already crossed: an eaten dot leaves nothing
/// behind; a word letter, once passed, is revealed for good.
fn passed_char(x: usize) -> char {
    if x < DOTS {
        ' '
    } else {
        WORD.as_bytes()[x - DOTS] as char
    }
}

/// A column still ahead of Pac-Man: a dot waiting to be eaten, or a
/// word letter not yet revealed (kept blank so it doesn't spoil the
/// reveal early).
fn upcoming_char(x: usize) -> char {
    if x < DOTS {
        '·'
    } else {
        ' '
    }
}

pub fn render(frame: &mut Frame, area: Rect, tick: u64) {
    let f = frame_at(tick);
    let glyph = if f.mouth_open { "ᗧ" } else { "ᗤ" };

    let line = Line::from(vec![
        Span::styled(f.before, Style::default().fg(theme::YELLOW)),
        Span::styled(
            glyph,
            Style::default()
                .fg(theme::YELLOW)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(f.after, Style::default().fg(theme::FG_FAINT)),
    ]);

    let hint = Line::styled(
        "press any key to skip",
        Style::default().fg(theme::FG_FAINT),
    );

    let [block] = Layout::vertical([Constraint::Length(2)])
        .flex(Flex::Center)
        .areas(area);
    let rows = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(block);

    frame.render_widget(Paragraph::new(line).alignment(Alignment::Center), rows[0]);
    frame.render_widget(Paragraph::new(hint).alignment(Alignment::Center), rows[1]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_zero_shows_all_dots_and_no_letters_revealed() {
        let f = frame_at(0);
        assert_eq!(f.before, "", "nothing passed yet at tick 0");
        assert!(f.mouth_open);
        assert!(
            f.after.chars().take(DOTS - 1).all(|c| c == '·'),
            "remaining dots ahead of pacman: {:?}",
            f.after
        );
        assert!(
            !f.after.contains(|c: char| c.is_ascii_alphabetic()),
            "no letter should be visible yet: {:?}",
            f.after
        );
    }

    #[test]
    fn final_tick_reveals_the_whole_word_with_no_dots_left() {
        let total = total_cols();
        let f = frame_at((total - 1) as u64);
        let full = format!("{}{}", f.before, f.after);
        assert!(full.contains(WORD), "expected {WORD:?} within {full:?}");
        assert!(!full.contains('·'), "no dots should remain: {full:?}");
    }

    #[test]
    fn ticks_past_the_end_hold_on_the_fully_revealed_word_without_panicking() {
        let f = frame_at(9_999);
        let full = format!("{}{}", f.before, f.after);
        assert!(full.contains(WORD));
    }

    #[test]
    fn mouth_alternates_every_tick_for_the_chomp_effect() {
        assert!(frame_at(0).mouth_open);
        assert!(!frame_at(1).mouth_open);
        assert!(frame_at(2).mouth_open);
    }

    #[test]
    fn letters_reveal_left_to_right_only_as_pacman_passes_them() {
        // Pac-Man sitting exactly on the word's first letter: it must
        // not be revealed yet (he hasn't passed it, only reached it),
        // and nothing behind him should still show a dot.
        let pos = DOTS;
        let f = frame_at(pos as u64);
        assert!(!f.before.contains('·'), "before: {:?}", f.before);
        assert!(
            !f.before.contains(|c: char| c.is_ascii_alphabetic()),
            "no letter revealed until pacman fully passes it: {:?}",
            f.before
        );

        // One tick later he's passed the first letter -- exactly one
        // letter revealed (preceded by the already-eaten, now-blank
        // dot cells), nothing more.
        let f = frame_at((pos + 1) as u64);
        assert!(f.before.ends_with('p'));
        assert_eq!(f.before.trim_start(), "p");
    }
}
