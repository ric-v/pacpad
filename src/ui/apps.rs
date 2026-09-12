//! The Apps tab: pacpad-managed webapps and TUI launchers, grouped
//! into two sections, plus the shared-style detail pane and (when
//! open) the add/edit form overlay -- the tab the design canvas calls
//! out as having no omarchy equivalent.

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, TableState};
use ratatui::Frame;

use super::theme;
use crate::app::state::AppState;
use crate::launcher::entry::{Kind, ManagedEntry};

pub fn render_subheader(frame: &mut Frame, area: Rect, state: &AppState) {
    let summary = format!(
        "Managed by pacpad — {} web app{} · {} tui app{}",
        state.apps.webapps.len(),
        if state.apps.webapps.len() == 1 {
            ""
        } else {
            "s"
        },
        state.apps.tuiapps.len(),
        if state.apps.tuiapps.len() == 1 {
            ""
        } else {
            "s"
        },
    );
    let cols = Layout::horizontal([Constraint::Min(0), Constraint::Length(14)]).split(area);
    frame.render_widget(
        Paragraph::new(Line::styled(summary, Style::default().fg(theme::FG_FAINT))),
        cols[0],
    );
    frame.render_widget(
        Paragraph::new(Line::styled(
            "↵ launch",
            Style::default().fg(theme::FG_FAINT),
        ))
        .alignment(Alignment::Right),
        cols[1],
    );
}

pub fn render_body(frame: &mut Frame, area: Rect, state: &AppState) {
    if state.apps.is_empty() {
        render_empty(frame, area, state.tick);
        return;
    }

    let detail_width = super::body_detail_width(area.width);
    let cols =
        Layout::horizontal([Constraint::Min(10), Constraint::Length(detail_width)]).split(area);

    render_table(frame, cols[0], state);
    super::detail::render_managed_entry(frame, cols[1], state.apps.selected_entry());
}

/// Track width for the mascot's back-and-forth walk -- the same width
/// on every render regardless of `area`, since it's just decoration
/// centered with everything else, not meant to fill the pane.
const WALK_TRACK: u16 = 20;

fn render_empty(frame: &mut Frame, area: Rect, tick: u64) {
    let (pos, mouth_open) = walk_position(tick, WALK_TRACK);
    let glyph = if mouth_open { "ᗧ" } else { "ᗤ" };

    let mascot_line = Line::from(vec![
        Span::raw(" ".repeat(pos as usize)),
        Span::styled(glyph, Style::default().fg(theme::YELLOW)),
    ]);

    let lines = vec![
        Line::default(),
        Line::styled(
            "No apps yet",
            Style::default()
                .fg(theme::FG_DIM)
                .add_modifier(Modifier::BOLD),
        ),
        Line::default(),
        mascot_line,
        Line::default(),
        Line::styled(
            "a add a webapp · t add a TUI app",
            Style::default().fg(theme::FG_FAINT),
        ),
    ];
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), area);
}

/// A back-and-forth ("ping-pong") walk across `[0, track)`, one column
/// per tick: `tick` counts up forever, but the position it maps to
/// bounces between the two ends instead of wrapping or running off the
/// edge. Pure and tested on its own since it's easy to get the bounce
/// off-by-one wrong.
fn walk_position(tick: u64, track: u16) -> (u16, bool) {
    if track <= 1 {
        return (0, tick % 2 == 0);
    }
    let span = (track - 1) as u64;
    let period = span * 2;
    let phase = tick % period;
    let pos = if phase <= span { phase } else { period - phase };
    (pos as u16, tick % 2 == 0)
}

/// Renders each section as its own full-width label line followed by
/// its own `Table` (rather than mixing a "header" row into one shared
/// table): a `Row` with fewer cells than the table's column count
/// still gets placed starting at column 1, so a single-cell header
/// row would be clipped to that column's width (2 chars, reserved for
/// the glyph) instead of spanning the row -- confirmed live, where
/// "WEBAPPS" rendered as "WE". Two small tables sidesteps that
/// entirely and keeps each section's own row-highlight index simple:
/// `state.apps.selected` is either within the webapps table's range
/// or, offset by its length, within the tuiapps table's.
fn render_table(frame: &mut Frame, area: Rect, state: &AppState) {
    let mut constraints = Vec::new();
    if !state.apps.webapps.is_empty() {
        constraints.push(Constraint::Length(1));
        constraints.push(Constraint::Length(state.apps.webapps.len() as u16));
    }
    if !state.apps.tuiapps.is_empty() {
        constraints.push(Constraint::Length(1));
        constraints.push(Constraint::Length(state.apps.tuiapps.len() as u16));
    }
    constraints.push(Constraint::Min(0));
    let rows = Layout::vertical(constraints).split(area);

    let mut i = 0;
    if !state.apps.webapps.is_empty() {
        render_section_label(frame, rows[i], "WEBAPPS");
        i += 1;
        let highlight =
            (state.apps.selected < state.apps.webapps.len()).then_some(state.apps.selected);
        render_entries_table(
            frame,
            rows[i],
            &state.apps.webapps,
            highlight,
            theme::ACCENT,
        );
        i += 1;
    }
    if !state.apps.tuiapps.is_empty() {
        render_section_label(frame, rows[i], "TUI APPS");
        i += 1;
        let highlight = (state.apps.selected >= state.apps.webapps.len())
            .then(|| state.apps.selected - state.apps.webapps.len());
        render_entries_table(
            frame,
            rows[i],
            &state.apps.tuiapps,
            highlight,
            theme::MAGENTA,
        );
    }
}

fn render_section_label(frame: &mut Frame, area: Rect, label: &str) {
    frame.render_widget(
        Paragraph::new(Line::styled(label, Style::default().fg(theme::FG_FAINT))),
        area,
    );
}

fn render_entries_table(
    frame: &mut Frame,
    area: Rect,
    entries: &[ManagedEntry],
    highlight: Option<usize>,
    glyph_color: Color,
) {
    let rows: Vec<Row> = entries.iter().map(|e| entry_row(e, glyph_color)).collect();
    let widths = [
        Constraint::Length(2),
        Constraint::Length(20),
        Constraint::Min(10),
        Constraint::Length(14),
        Constraint::Length(11),
    ];
    let table = Table::new(rows, widths)
        .column_spacing(1)
        .row_highlight_style(
            Style::default()
                .bg(theme::ACCENT)
                .fg(theme::BG)
                .add_modifier(Modifier::BOLD),
        );

    let mut table_state = TableState::default().with_selected(highlight);
    frame.render_stateful_widget(table, area, &mut table_state);
}

fn entry_row(entry: &ManagedEntry, glyph_color: Color) -> Row<'_> {
    let target = entry
        .url
        .as_deref()
        .or(entry.command.as_deref())
        .unwrap_or("");
    let kind_label = match entry.kind {
        Kind::Webapp => "[webapp]".to_string(),
        Kind::Tui => {
            let style = if entry.window_class.as_deref() == Some("pacpad-tui-tile") {
                "tile"
            } else {
                "float"
            };
            format!("[tui · {style}]")
        }
    };
    // `created` is already a full ISO8601 string (`X-PacPad-Created`
    // is stored formatted, not as a raw timestamp) -- just the date
    // portion belongs in the narrow column here.
    let created = entry
        .created
        .as_deref()
        .and_then(|s| s.split('T').next())
        .unwrap_or("")
        .to_string();

    Row::new(vec![
        Cell::from(Span::styled("■", Style::default().fg(glyph_color))),
        Cell::from(entry.name.clone()),
        Cell::from(Span::styled(
            target.to_string(),
            Style::default().fg(theme::FG_FAINT),
        )),
        Cell::from(Span::styled(
            kind_label,
            Style::default().fg(theme::FG_FAINT),
        )),
        Cell::from(Span::styled(created, Style::default().fg(theme::FG_FAINT))),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walk_bounces_between_zero_and_the_far_end() {
        // track=5 -> valid positions 0..=4, period = 8
        let positions: Vec<u16> = (0..12).map(|t| walk_position(t, 5).0).collect();
        assert_eq!(positions, vec![0, 1, 2, 3, 4, 3, 2, 1, 0, 1, 2, 3]);
    }

    #[test]
    fn walk_never_leaves_the_track_for_any_tick() {
        for track in [2, 3, 5, WALK_TRACK] {
            for t in 0..500u64 {
                let (pos, _) = walk_position(t, track);
                assert!(
                    pos < track,
                    "pos {pos} out of bounds for track {track} at tick {t}"
                );
            }
        }
    }

    #[test]
    fn mouth_alternates_every_tick() {
        assert!(walk_position(0, WALK_TRACK).1);
        assert!(!walk_position(1, WALK_TRACK).1);
    }

    #[test]
    fn a_single_column_track_never_panics() {
        for t in 0..5 {
            let (pos, _) = walk_position(t, 1);
            assert_eq!(pos, 0);
        }
        for t in 0..5 {
            let (pos, _) = walk_position(t, 0);
            assert_eq!(pos, 0);
        }
    }
}
