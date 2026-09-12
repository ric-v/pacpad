//! The Search tab: a live query line (with the terminal's own cursor
//! placed at the end of it -- no hand-drawn cursor block needed, the
//! real tty cursor is more authentic than faking one), ranked results
//! across every repo plus what's installed, and the shared detail
//! pane.

use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, TableState};
use ratatui::Frame;

use super::{detail, theme};
use crate::alpm::index::PackageEntry;
use crate::app::AppState;

pub fn render_subheader(frame: &mut Frame, area: Rect, state: &AppState) {
    let count_text = format!("{} match(es)", state.search.results.len());
    let cols = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(count_text.len() as u16 + 2),
    ])
    .split(area);

    let line = Line::from(vec![
        Span::styled(" Search: ", Style::default().fg(theme::FG_FAINT)),
        Span::styled("› ", Style::default().fg(theme::ACCENT)),
        Span::raw(state.search.query.clone()),
    ]);
    frame.render_widget(Paragraph::new(line), cols[0]);
    frame.render_widget(
        Paragraph::new(Line::styled(
            count_text,
            Style::default().fg(theme::FG_FAINT),
        ))
        .alignment(ratatui::layout::Alignment::Right),
        cols[1],
    );

    // Real terminal cursor, right after the typed query -- the Search
    // tab is always "focused" for typing (see app::input's docs).
    let cursor_x = area.x + 11 + state.search.query.chars().count() as u16;
    frame.set_cursor_position(Position::new(
        cursor_x.min(area.x + area.width.saturating_sub(1)),
        area.y,
    ));
}

pub fn render_body(frame: &mut Frame, area: Rect, state: &AppState) {
    let detail_width = super::body_detail_width(area.width);
    let cols =
        Layout::horizontal([Constraint::Min(10), Constraint::Length(detail_width)]).split(area);

    render_table(frame, cols[0], state);
    detail::render(frame, cols[1], state.current_entry());
}

fn render_table(frame: &mut Frame, area: Rect, state: &AppState) {
    let header = Row::new(vec![
        Cell::from(Span::styled("NAME", Style::default().fg(theme::FG_FAINT))),
        Cell::from(Span::styled(
            "VERSION",
            Style::default().fg(theme::FG_FAINT),
        )),
        Cell::from(Span::styled("SOURCE", Style::default().fg(theme::FG_FAINT))),
        Cell::from(Span::styled("STATUS", Style::default().fg(theme::FG_FAINT))),
    ]);

    let rows: Vec<Row> = state
        .search
        .results
        .iter()
        .map(|&(i, matched_desc)| result_row(&state.entries[i], matched_desc))
        .collect();

    let widths = [
        Constraint::Min(10),
        Constraint::Length(14),
        Constraint::Length(12),
        Constraint::Length(14),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .column_spacing(1)
        .row_highlight_style(
            Style::default()
                .bg(theme::ACCENT)
                .fg(theme::BG)
                .add_modifier(Modifier::BOLD),
        );

    let selected = if state.search.results.is_empty() {
        None
    } else {
        Some(state.search.selected)
    };
    let mut table_state = TableState::default().with_selected(selected);
    frame.render_stateful_widget(table, area, &mut table_state);
}

fn result_row(entry: &PackageEntry, matched_description: bool) -> Row<'_> {
    let source = entry
        .repo()
        .map(|r| format!("[{r}]"))
        .unwrap_or_else(|| "[aur]".to_string());
    let source_color = if entry.repo().is_some() {
        theme::FG_FAINT
    } else {
        theme::AMBER
    };
    let (status, status_color) = if entry.is_installed() {
        ("✓ installed", theme::GREEN)
    } else {
        ("not installed", theme::FG_FAINT)
    };

    let name_line = if matched_description {
        Line::from(vec![
            Span::raw(entry.name.clone()),
            Span::styled("  (desc match)", Style::default().fg(theme::FG_FAINT)),
        ])
    } else {
        Line::from(entry.name.clone())
    };

    Row::new(vec![
        Cell::from(name_line),
        Cell::from(Span::styled(
            entry.version().unwrap_or("").to_string(),
            Style::default().fg(theme::FG_DIM),
        )),
        Cell::from(Span::styled(source, Style::default().fg(source_color))),
        Cell::from(Span::styled(status, Style::default().fg(status_color))),
    ])
}
