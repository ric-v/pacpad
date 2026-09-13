//! The `F1` keybinding cheat sheet: a single reference for every key
//! pacpad understands, since the per-tab footer (see `ui::render_footer`)
//! only ever has room for that tab's own bindings and the global ones
//! (Tab/Shift-Tab, 1-4, Ctrl+C) never appear anywhere on screen at all.

use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use ratatui::Frame;

use super::theme;

struct Section {
    title: &'static str,
    bindings: &'static [(&'static str, &'static str)],
}

const SECTIONS: &[Section] = &[
    Section {
        title: "Global",
        bindings: &[
            ("1-4 / Tab", "switch tabs"),
            ("F1", "toggle this help"),
            ("q / Ctrl+C", "quit"),
        ],
    },
    Section {
        title: "Installed",
        bindings: &[
            ("↑↓ / jk", "move"),
            ("←→ / hl", "cycle filter"),
            ("space", "select"),
            ("s", "cycle sort"),
            ("/", "jump to search"),
            ("i / d / r", "install / remove / reinstall"),
            ("e / D", "mark explicit / as dependency"),
            ("u", "update system"),
            ("f / o", "list files / open URL"),
        ],
    },
    Section {
        title: "Search",
        bindings: &[
            ("type", "filter by name or description"),
            ("↑↓", "move"),
            ("space", "select"),
            ("enter", "install"),
            ("esc", "clear query"),
        ],
    },
    Section {
        title: "Apps",
        bindings: &[
            ("↑↓ / jk", "move"),
            ("a / t", "add webapp / add TUI app"),
            ("enter", "launch"),
            ("e / d", "edit / remove"),
        ],
    },
];

pub fn render(frame: &mut Frame, area: Rect) {
    let width = (area.width.saturating_sub(8)).clamp(40, 60);
    let height = content_height().min(area.height.saturating_sub(4));
    let popup = centered(area, width, height);

    frame.render_widget(Clear, popup);

    let title = Line::from(vec![Span::styled(
        " Keybindings ",
        Style::default()
            .fg(theme::BG)
            .bg(theme::ACCENT)
            .add_modifier(Modifier::BOLD),
    )]);
    let block = Block::bordered()
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(theme::ACCENT))
        .style(Style::default().bg(theme::BG).fg(theme::FG))
        .title(title)
        .title_alignment(Alignment::Center);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut lines: Vec<Line> = Vec::new();
    for (i, section) in SECTIONS.iter().enumerate() {
        if i > 0 {
            lines.push(Line::default());
        }
        lines.push(Line::styled(
            format!(" {}", section.title),
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ));
        for (key, label) in section.bindings {
            lines.push(Line::from(vec![
                Span::styled(format!("  {key:<12}"), Style::default().fg(theme::FG)),
                Span::styled(*label, Style::default().fg(theme::FG_DIM)),
            ]));
        }
    }
    lines.push(Line::default());
    lines.push(Line::styled(
        " F1 / esc  close",
        Style::default().fg(theme::FG_FAINT),
    ));

    frame.render_widget(Paragraph::new(lines), inner);
}

fn content_height() -> u16 {
    let bindings: usize = SECTIONS.iter().map(|s| s.bindings.len() + 1).sum();
    // borders (2) + section blank separators + trailing blank + close hint
    2 + bindings as u16 + (SECTIONS.len() as u16 - 1) + 2
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
