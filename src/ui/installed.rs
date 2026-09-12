//! The Installed tab: filter chips, a sortable table of installed
//! packages, and the shared detail pane on the right.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, TableState};
use ratatui::Frame;

use super::{detail, theme};
use crate::alpm::index::PackageEntry;
use crate::alpm::InstallReason;
use crate::app::state::InstalledFilter;
use crate::app::AppState;

pub fn render_subheader(frame: &mut Frame, area: Rect, state: &AppState) {
    let mut spans = Vec::new();
    for (i, filter) in [
        InstalledFilter::All,
        InstalledFilter::Explicit,
        InstalledFilter::Dependency,
        InstalledFilter::Orphan,
        InstalledFilter::Foreign,
    ]
    .into_iter()
    .enumerate()
    {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        if filter == state.installed.filter {
            spans.push(Span::styled(
                format!(" {} ", filter.label()),
                Style::default()
                    .bg(theme::ACCENT)
                    .fg(theme::BG)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                filter.label(),
                Style::default().fg(theme::FG_DIM),
            ));
        }
    }

    let sort_text = format!(
        "Sort: {} {}",
        state.installed.sort.label(),
        if state.installed.sort_desc {
            "▼"
        } else {
            "▲"
        }
    );
    let cols = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(sort_text.len() as u16 + 2),
    ])
    .split(area);
    frame.render_widget(Paragraph::new(Line::from(spans)), cols[0]);
    frame.render_widget(
        Paragraph::new(Line::styled(
            sort_text,
            Style::default().fg(theme::FG_FAINT),
        ))
        .alignment(ratatui::layout::Alignment::Right),
        cols[1],
    );
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
        Cell::from(""),
        Cell::from(""),
        Cell::from(Span::styled("NAME", Style::default().fg(theme::FG_FAINT))),
        Cell::from(Span::styled(
            "VERSION",
            Style::default().fg(theme::FG_FAINT),
        )),
        Cell::from(Span::styled("REPO", Style::default().fg(theme::FG_FAINT))),
        Cell::from(Span::styled("SIZE", Style::default().fg(theme::FG_FAINT))),
    ]);

    let rows: Vec<Row> = state
        .installed
        .visible
        .iter()
        .map(|&i| {
            package_row(
                &state.entries[i],
                state.installed.checked.contains(&state.entries[i].name),
            )
        })
        .collect();

    let widths = [
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Min(10),
        Constraint::Length(14),
        Constraint::Length(12),
        Constraint::Length(9),
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

    let selected = if state.installed.visible.is_empty() {
        None
    } else {
        Some(state.installed.selected)
    };
    let mut table_state = TableState::default().with_selected(selected);
    frame.render_stateful_widget(table, area, &mut table_state);
}

fn package_row<'a>(entry: &'a PackageEntry, checked: bool) -> Row<'a> {
    let checkbox = if checked { "[x]" } else { "[ ]" };
    let (dot, dot_color) = match entry.reason() {
        Some(InstallReason::Explicit) => ("●", theme::GREEN),
        Some(InstallReason::Dependency) => ("○", theme::FG_FAINT),
        None => (" ", theme::FG_FAINT),
    };
    let repo_label = entry
        .repo()
        .map(|r| format!("[{r}]"))
        .unwrap_or_else(|| "[aur]".to_string());
    let repo_color = if entry.repo().is_some() {
        theme::FG_FAINT
    } else {
        theme::AMBER
    };
    let name_color = if matches!(entry.reason(), Some(InstallReason::Dependency)) {
        theme::FG_DIM
    } else {
        theme::FG
    };

    Row::new(vec![
        Cell::from(checkbox),
        Cell::from(Span::styled(dot, Style::default().fg(dot_color))),
        Cell::from(Span::styled(
            entry.name.clone(),
            Style::default().fg(name_color),
        )),
        Cell::from(Span::styled(
            entry.version().unwrap_or("").to_string(),
            Style::default().fg(theme::FG_DIM),
        )),
        Cell::from(Span::styled(repo_label, Style::default().fg(repo_color))),
        Cell::from(Span::styled(
            detail::format_size(installed_size(entry)),
            Style::default().fg(theme::FG_DIM),
        )),
    ])
}

fn installed_size(entry: &PackageEntry) -> u64 {
    entry.local.as_ref().map(|l| l.installed_size).unwrap_or(0)
}
