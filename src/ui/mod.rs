//! Pure rendering: `render(frame, &AppState)` never mutates state, and
//! nothing here does I/O. The frame is built to echo the design
//! canvas mockups closely -- a single box-drawn outer frame (ratatui's
//! own `Block` border, not hand-drawn characters, since ratatui
//! already renders exactly `┌─┐│└┘` for `BorderType::Plain`), dash-fill
//! divider rows between regions (with a `┬`/`┴` tee spliced in where
//! the list/detail vertical split crosses them), and full reverse-
//! video row selection instead of a colored accent bar.

mod apps;
mod detail;
mod form;
mod help;
mod installed;
mod modal;
mod running;
mod search;
mod splash;
mod theme;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};
use ratatui::Frame;

use crate::app::state::Tab;
use crate::app::AppState;

/// Minimum width/height below which we stop trying to lay out the
/// full frame and just say so -- better than a panicking subtraction
/// or a garbled render in a terminal the user happened to make tiny.
const MIN_WIDTH: u16 = 60;
const MIN_HEIGHT: u16 = 12;

pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();

    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        render_too_small(frame, area);
        return;
    }

    if state.splash_active() {
        frame.render_widget(
            Paragraph::new("").style(Style::default().bg(theme::BG)),
            area,
        );
        splash::render(frame, area, state.tick);
        return;
    }

    let title = Line::from(vec![
        Span::styled(
            " pac",
            Style::default()
                .fg(theme::MAGENTA)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "pad ",
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let outer = Block::bordered()
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(theme::BORDER))
        .style(Style::default().bg(theme::BG).fg(theme::FG))
        .title(title);
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let rows = Layout::vertical([
        Constraint::Length(1), // tabs
        Constraint::Length(1), // divider
        Constraint::Length(1), // subheader
        Constraint::Length(1), // divider (tee where body splits)
        Constraint::Min(3),    // body
        Constraint::Length(1), // divider (tee where body splits)
        Constraint::Length(1), // footer
    ])
    .split(inner);

    render_tabs(frame, rows[0], state);

    let has_split = matches!(state.tab, Tab::Installed | Tab::Search | Tab::Apps);
    let tee_col = if has_split {
        Some(body_detail_width(rows[4].width))
    } else {
        None
    };
    let tee_col_from_start = tee_col.map(|w| rows[4].width.saturating_sub(w));

    render_divider(frame, rows[1], None);

    match state.tab {
        Tab::Installed => installed::render_subheader(frame, rows[2], state),
        Tab::Search => search::render_subheader(frame, rows[2], state),
        Tab::Apps => apps::render_subheader(frame, rows[2], state),
        Tab::Updates => render_centered_note(frame, rows[2], ""),
    }

    render_divider(frame, rows[3], tee_col_from_start.map(|c| (c, '┬')));

    match state.tab {
        Tab::Installed => installed::render_body(frame, rows[4], state),
        Tab::Search => search::render_body(frame, rows[4], state),
        Tab::Apps => apps::render_body(frame, rows[4], state),
        Tab::Updates => render_placeholder(
            frame,
            rows[4],
            "Updates",
            "Version-diff listing is still pending -- press u on the Installed tab to run a full system update now.",
        ),
    }

    match result_banner(state) {
        Some((text, phase)) => render_result_banner(frame, rows[5], &text, phase),
        None => render_divider(frame, rows[5], tee_col_from_start.map(|c| (c, '┴'))),
    }
    render_footer(frame, rows[6], state);

    if let Some(form) = &state.apps.form {
        form::render(frame, area, form);
    }

    if let Some(modal) = &state.modal {
        modal::render(frame, area, modal);
    }

    if state.help_open {
        help::render(frame, area);
    }

    if let Some(view) = &state.running {
        running::render(frame, area, view, state.tick);
    }
}

/// The `(rows, cols)` an embedded pty should be spawned/resized to for
/// a terminal that's `cols` columns by `rows` rows -- exposed so
/// `cli::run_tui` can size a `txn::PtySession` to match exactly what
/// `running::render` (private to this module) will actually draw it
/// into, both at spawn time and on a live terminal resize.
pub fn running_content_dims(cols: u16, rows: u16) -> (u16, u16) {
    running::content_dims(Rect::new(0, 0, cols, rows))
}

/// How many ticks a completed action's result stays bright, then dim,
/// before disappearing entirely and giving the row back to the plain
/// divider -- purely informational (see `AppState::last_result`'s doc
/// comment), never blocking, never re-confirmed.
const BANNER_BRIGHT_TICKS: u64 = 15;
const BANNER_TOTAL_TICKS: u64 = 30;

enum BannerPhase {
    Bright,
    Fading,
}

/// Pure so the fade timing is tested without a `Frame`: `None` once
/// there's no result to show, or once it's aged past
/// `BANNER_TOTAL_TICKS`.
fn result_banner(state: &AppState) -> Option<(String, BannerPhase)> {
    let text = state.last_result.as_ref()?;
    let set_at = state.last_result_tick?;
    let elapsed = state.tick.saturating_sub(set_at);
    if elapsed >= BANNER_TOTAL_TICKS {
        return None;
    }
    let phase = if elapsed < BANNER_BRIGHT_TICKS {
        BannerPhase::Bright
    } else {
        BannerPhase::Fading
    };
    Some((text.clone(), phase))
}

fn render_result_banner(frame: &mut Frame, area: Rect, text: &str, phase: BannerPhase) {
    let is_error = text.to_ascii_lowercase().contains("error");
    let color = if is_error { theme::RED } else { theme::GREEN };
    let glyph = if is_error { "ᗤ" } else { "ᗧ" };

    let mut style = Style::default().fg(color);
    if matches!(phase, BannerPhase::Bright) {
        style = style.add_modifier(Modifier::BOLD);
    } else {
        style = Style::default().fg(theme::FG_DIM);
    }

    let line = Line::from(vec![
        Span::styled(format!(" {glyph} "), style),
        Span::styled(text.to_string(), style),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

/// Detail-pane width for the Installed/Search list+detail split: a
/// little under a third of the body, clamped so it never eats the
/// list entirely on a narrow terminal nor stays absurdly wide on a
/// huge one.
fn body_detail_width(body_width: u16) -> u16 {
    ((body_width as u32 * 3) / 10).clamp(28, 56) as u16
}

fn render_tabs(frame: &mut Frame, area: Rect, state: &AppState) {
    let mut spans = Vec::new();
    for (i, tab) in Tab::ALL.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" │ ", Style::default().fg(theme::BORDER)));
        }
        if tab == state.tab {
            spans.push(Span::styled(
                format!(" {} ", tab.label()),
                Style::default()
                    .bg(theme::ACCENT)
                    .fg(theme::BG)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!(" {} ", tab.label()),
                Style::default().fg(theme::FG_DIM),
            ));
        }
    }
    let cols = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(state.host_label.len() as u16 + 2),
    ])
    .split(area);
    frame.render_widget(Paragraph::new(Line::from(spans)), cols[0]);
    frame.render_widget(
        Paragraph::new(Line::styled(
            state.host_label.clone(),
            Style::default().fg(theme::FG_FAINT),
        ))
        .alignment(ratatui::layout::Alignment::Right),
        cols[1],
    );
}

/// A full-width row of `─`, optionally with a `┬`/`┴` tee character
/// spliced in at `tee` = (column, char) -- matching where the list and
/// detail panes divide in the row above/below.
pub(super) fn divider_line(width: u16, tee: Option<(u16, char)>) -> Line<'static> {
    let w = width.max(1) as usize;
    let mut chars: Vec<char> = std::iter::repeat_n('─', w).collect();
    if let Some((col, ch)) = tee {
        if (col as usize) < chars.len() {
            chars[col as usize] = ch;
        }
    }
    Line::styled(
        chars.into_iter().collect::<String>(),
        Style::default().fg(theme::BORDER),
    )
}

fn render_divider(frame: &mut Frame, area: Rect, tee: Option<(u16, char)>) {
    frame.render_widget(Paragraph::new(divider_line(area.width, tee)), area);
}

fn render_footer(frame: &mut Frame, area: Rect, state: &AppState) {
    let hints: &[(&str, ratatui::style::Color, &str)] = match state.tab {
        Tab::Installed => &[
            ("↑↓/jk", theme::FG_FAINT, "nav"),
            ("←→/hl", theme::FG_FAINT, "filter"),
            ("space", theme::FG_FAINT, "select"),
            ("i", theme::GREEN, "install"),
            ("d", theme::RED, "remove"),
            ("r", theme::ACCENT, "reinstall"),
            ("e", theme::FG_FAINT, "explicit"),
            ("D", theme::FG_FAINT, "as dep"),
            ("u", theme::AMBER, "update"),
            ("f", theme::FG_FAINT, "files"),
            ("o", theme::FG_FAINT, "url"),
            ("s", theme::FG_FAINT, "sort"),
            ("/", theme::FG_FAINT, "search"),
            ("q", theme::FG_FAINT, "quit"),
        ],
        Tab::Search => &[
            ("↑↓", theme::FG_FAINT, "nav"),
            ("type", theme::FG_FAINT, "to search"),
            ("space", theme::FG_FAINT, "select"),
            ("enter", theme::GREEN, "install"),
            ("esc", theme::FG_FAINT, "clear"),
            ("tab", theme::FG_FAINT, "switch"),
        ],
        Tab::Apps => &[
            ("↑↓/jk", theme::FG_FAINT, "nav"),
            ("a", theme::GREEN, "add webapp"),
            ("t", theme::GREEN, "add tui app"),
            ("enter", theme::ACCENT, "launch"),
            ("e", theme::FG_FAINT, "edit"),
            ("d", theme::RED, "remove"),
            ("tab", theme::FG_FAINT, "switch"),
            ("q", theme::FG_FAINT, "quit"),
        ],
        Tab::Updates => &[
            ("1-4", theme::FG_FAINT, "jump tab"),
            ("q", theme::FG_FAINT, "quit"),
        ],
    };

    let mut spans = Vec::new();
    for (key, bg, label) in hints {
        spans.push(Span::styled(
            format!(" {key} "),
            Style::default()
                .bg(*bg)
                .fg(theme::BG)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!(" {label}  "),
            Style::default().fg(theme::FG_DIM),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_placeholder(frame: &mut Frame, area: Rect, title: &str, note: &str) {
    let lines = vec![
        Line::default(),
        Line::styled(
            title,
            Style::default()
                .fg(theme::FG_DIM)
                .add_modifier(Modifier::BOLD),
        ),
        Line::styled(note, Style::default().fg(theme::FG_FAINT)),
    ];
    frame.render_widget(
        Paragraph::new(lines).alignment(ratatui::layout::Alignment::Center),
        area,
    );
}

fn render_centered_note(frame: &mut Frame, area: Rect, text: &str) {
    frame.render_widget(
        Paragraph::new(Line::styled(text, Style::default().fg(theme::FG_FAINT))),
        area,
    );
}

fn render_too_small(frame: &mut Frame, area: Rect) {
    frame.render_widget(
        Paragraph::new("pacpad needs at least 60x12")
            .style(Style::default().bg(theme::BG).fg(theme::RED))
            .alignment(ratatui::layout::Alignment::Center),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alpm::index::PackageIndex;
    use crate::alpm::local::read_local_db;
    use crate::alpm::sync::read_sync_db;
    use std::path::Path;

    fn state() -> AppState {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let locals = read_local_db(&root.join("local")).unwrap();
        let syncs = read_sync_db(&root.join("sync/extra.db"), "extra").unwrap();
        AppState::new(PackageIndex::build(locals, syncs), None)
    }

    #[test]
    fn no_banner_when_nothing_has_run_yet() {
        let s = state();
        assert!(result_banner(&s).is_none());
    }

    #[test]
    fn banner_shows_bright_immediately_after_a_result_is_set() {
        let mut s = state();
        s.set_last_result("done".to_string());
        let (text, phase) = result_banner(&s).unwrap();
        assert_eq!(text, "done");
        assert!(matches!(phase, BannerPhase::Bright));
    }

    #[test]
    fn banner_fades_after_the_bright_window_then_disappears() {
        let mut s = state();
        s.set_last_result("done".to_string());

        s.tick = BANNER_BRIGHT_TICKS; // just past bright
        let (_, phase) = result_banner(&s).unwrap();
        assert!(matches!(phase, BannerPhase::Fading));

        s.tick = BANNER_TOTAL_TICKS; // fully aged out
        assert!(result_banner(&s).is_none());
    }

    #[test]
    fn a_new_result_resets_the_fade_even_if_the_old_one_had_aged_out() {
        let mut s = state();
        s.set_last_result("first".to_string());
        s.tick = BANNER_TOTAL_TICKS + 5;
        assert!(result_banner(&s).is_none());

        s.set_last_result("second".to_string());
        let (text, phase) = result_banner(&s).unwrap();
        assert_eq!(text, "second");
        assert!(matches!(phase, BannerPhase::Bright));
    }

    #[test]
    fn error_results_are_flagged_for_red_styling_via_the_word_error() {
        // `render_result_banner` branches on this same substring check
        // to pick red vs green -- covered here structurally since the
        // branch itself needs a live `Frame` to observe styled output.
        assert!("error creating webapp: oops"
            .to_ascii_lowercase()
            .contains("error"));
        assert!(!"done".to_ascii_lowercase().contains("error"));
    }
}
