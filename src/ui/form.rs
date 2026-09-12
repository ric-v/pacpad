//! The Apps tab's add/edit form: a box-drawn popup (same visual
//! language as `ui::modal`) with labeled fields, matching the design
//! canvas's "New Web App"/"New TUI App" mockups. The focused text
//! field gets the real terminal cursor (`set_cursor_position`) rather
//! than a hand-drawn block, the same choice made for the Search tab's
//! query line -- it's more authentic, and it's what a real terminal
//! app actually does.

use ratatui::layout::{Alignment, Constraint, Flex, Layout, Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use ratatui::Frame;

use super::theme;
use crate::app::state::{AppForm, TextField, TuiField, WebappField};
use crate::launcher::terminal::WindowStyle;

pub fn render(frame: &mut Frame, area: Rect, form: &AppForm) {
    let width = (area.width.saturating_sub(8)).clamp(44, 70);
    let field_count = match form {
        AppForm::Webapp(_) => 3,
        AppForm::Tui(_) => 4,
    };
    let error_lines = match form {
        AppForm::Webapp(f) => f.error.is_some(),
        AppForm::Tui(f) => f.error.is_some(),
    };
    let height =
        (7 + field_count * 3 + if error_lines { 2 } else { 0 }).min(area.height.saturating_sub(4));
    let popup = centered(area, width, height);

    frame.render_widget(Clear, popup);

    let title = match form {
        AppForm::Webapp(f) if f.editing.is_some() => " Edit Web App ",
        AppForm::Webapp(_) => " New Web App ",
        AppForm::Tui(f) if f.editing.is_some() => " Edit TUI App ",
        AppForm::Tui(_) => " New TUI App ",
    };
    let block = Block::bordered()
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(theme::ACCENT))
        .style(Style::default().bg(theme::BG).fg(theme::FG))
        .title(Line::from(Span::styled(
            title,
            Style::default()
                .fg(theme::BG)
                .bg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        )))
        .title_alignment(Alignment::Center);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    match form {
        AppForm::Webapp(f) => render_webapp_form(frame, inner, f),
        AppForm::Tui(f) => render_tui_form(frame, inner, f),
    }
}

fn render_webapp_form(frame: &mut Frame, area: Rect, f: &crate::app::state::WebappForm) {
    let mut constraints = vec![
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
    ];
    if f.error.is_some() {
        constraints.push(Constraint::Length(2));
    }
    constraints.push(Constraint::Min(0));
    constraints.push(Constraint::Length(1));
    let rows = Layout::vertical(constraints).split(area);

    render_field(
        frame,
        rows[0],
        "NAME",
        &f.name,
        f.focus == WebappField::Name,
    );
    render_field(frame, rows[1], "URL", &f.url, f.focus == WebappField::Url);
    render_field(
        frame,
        rows[2],
        "ICON (URL, PATH, OR BLANK FOR AUTO)",
        &f.icon,
        f.focus == WebappField::Icon,
    );

    if let Some(err) = &f.error {
        render_error(frame, rows[3], err);
    }
    render_footer(frame, rows[rows.len() - 1], f.editing.is_some());
}

fn render_tui_form(frame: &mut Frame, area: Rect, f: &crate::app::state::TuiForm) {
    let mut constraints = vec![
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Length(3),
    ];
    if f.error.is_some() {
        constraints.push(Constraint::Length(2));
    }
    constraints.push(Constraint::Min(0));
    constraints.push(Constraint::Length(1));
    let rows = Layout::vertical(constraints).split(area);

    render_field(frame, rows[0], "NAME", &f.name, f.focus == TuiField::Name);
    render_field(
        frame,
        rows[1],
        "COMMAND",
        &f.command,
        f.focus == TuiField::Command,
    );
    render_style_toggle(frame, rows[2], f.style, f.focus == TuiField::Style);
    render_field(
        frame,
        rows[3],
        "ICON PATH (OPTIONAL)",
        &f.icon,
        f.focus == TuiField::Icon,
    );

    if let Some(err) = &f.error {
        render_error(frame, rows[4], err);
    }
    render_footer(frame, rows[rows.len() - 1], f.editing.is_some());
}

fn render_field(frame: &mut Frame, area: Rect, label: &str, field: &TextField, focused: bool) {
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);

    frame.render_widget(
        Paragraph::new(Line::styled(label, Style::default().fg(theme::FG_FAINT))),
        rows[0],
    );

    let value_color = if focused { theme::FG } else { theme::FG_DIM };
    frame.render_widget(
        Paragraph::new(Line::styled(
            field.value.clone(),
            Style::default().fg(value_color),
        )),
        rows[1],
    );

    let underline_color = if focused {
        theme::ACCENT
    } else {
        theme::BORDER
    };
    frame.render_widget(
        Paragraph::new(Line::styled(
            "─".repeat(area.width as usize),
            Style::default().fg(underline_color),
        )),
        rows[2],
    );

    if focused {
        let cursor_x = rows[1].x + field.cursor as u16;
        frame.set_cursor_position(Position::new(
            cursor_x.min(rows[1].x + rows[1].width.saturating_sub(1)),
            rows[1].y,
        ));
    }
}

fn render_style_toggle(frame: &mut Frame, area: Rect, style: WindowStyle, focused: bool) {
    let rows = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(area);
    let label = if focused {
        "WINDOW STYLE (←→ to change)"
    } else {
        "WINDOW STYLE"
    };
    frame.render_widget(
        Paragraph::new(Line::styled(label, Style::default().fg(theme::FG_FAINT))),
        rows[0],
    );

    let (float_style, tile_style) = match style {
        WindowStyle::Float => (
            Style::default()
                .bg(theme::ACCENT)
                .fg(theme::BG)
                .add_modifier(Modifier::BOLD),
            Style::default().fg(theme::FG_DIM),
        ),
        WindowStyle::Tile => (
            Style::default().fg(theme::FG_DIM),
            Style::default()
                .bg(theme::ACCENT)
                .fg(theme::BG)
                .add_modifier(Modifier::BOLD),
        ),
    };
    let border = if focused {
        theme::ACCENT
    } else {
        theme::BORDER
    };
    let line = Line::from(vec![
        Span::styled(" Float ", float_style),
        Span::styled("  ", Style::default()),
        Span::styled(" Tile ", tile_style),
    ]);
    frame.render_widget(
        Paragraph::new(line).style(Style::default().fg(border)),
        rows[1],
    );
}

fn render_error(frame: &mut Frame, area: Rect, message: &str) {
    let rows = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(area);
    frame.render_widget(
        Paragraph::new(Line::styled(
            format!("⚠ {message}"),
            Style::default().fg(theme::RED),
        )),
        rows[1],
    );
}

fn render_footer(frame: &mut Frame, area: Rect, editing: bool) {
    let submit_label = if editing { " save" } else { " create" };
    let spans = vec![
        Span::styled(
            " esc ",
            Style::default()
                .bg(theme::FG_FAINT)
                .fg(theme::BG)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" cancel  ", Style::default().fg(theme::FG_DIM)),
        Span::styled(
            " tab ",
            Style::default()
                .bg(theme::FG_FAINT)
                .fg(theme::BG)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" next field  ", Style::default().fg(theme::FG_DIM)),
        Span::styled(
            " enter ",
            Style::default()
                .bg(theme::GREEN)
                .fg(theme::BG)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(submit_label, Style::default().fg(theme::FG_DIM)),
    ];
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let [popup] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(area);
    let [popup] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(popup);
    popup
}
