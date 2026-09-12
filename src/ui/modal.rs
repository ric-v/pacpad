//! The confirm modal: a small box-drawn popup centered over whatever
//! tab triggered it, showing the literal command that will run --
//! matching the design canvas's "Confirm Removal" mockup, generalized
//! to every action rather than just removal.
//!
//! Deliberately no scrim/dimming behind it: the tab underneath keeps
//! rendering exactly as it would otherwise, matching how a ratatui
//! `Clear` + `Block` popup actually paints (it replaces the cells
//! under it; it doesn't touch anything else on screen).

use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Wrap};
use ratatui::Frame;

use super::theme;
use crate::app::state::Modal;
use crate::txn::CONFIRMATION_PHRASE;

pub fn render(frame: &mut Frame, area: Rect, modal: &Modal) {
    let width = (area.width.saturating_sub(8)).clamp(40, 70);
    let height = content_height(modal).min(area.height.saturating_sub(4));
    let popup = centered(area, width, height);

    frame.render_widget(Clear, popup);

    let accent_color = if modal.is_guarded() {
        theme::RED
    } else {
        theme::ACCENT
    };
    let title = Line::from(vec![Span::styled(
        format!(" {} ", title_for(modal)),
        Style::default()
            .fg(theme::BG)
            .bg(accent_color)
            .add_modifier(Modifier::BOLD),
    )]);

    let block = Block::bordered()
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(accent_color))
        .style(Style::default().bg(theme::BG).fg(theme::FG))
        .title(title)
        .title_alignment(Alignment::Center);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut lines: Vec<Line> = Vec::new();

    let count = modal.action.packages().len();
    if count > 0 {
        let noun = if count == 1 { "package" } else { "packages" };
        lines.push(Line::styled(
            format!(" {count} {noun}: {}", modal.action.packages().join(", ")),
            Style::default().fg(theme::FG_DIM),
        ));
        lines.push(Line::default());
    }

    lines.push(Line::styled(" $", Style::default().fg(theme::FG_FAINT)));
    lines.push(Line::styled(
        format!(" {}", modal.command.display),
        Style::default().fg(theme::FG).add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::default());

    if let crate::txn::Action::Remove { recursive, .. } = &modal.action {
        let hint = if *recursive {
            "-Rns: also removes now-unneeded dependencies and configs"
        } else {
            "-R: removes only the listed package(s)"
        };
        lines.push(Line::styled(
            format!(" {hint}"),
            Style::default().fg(theme::FG_FAINT),
        ));
        lines.push(Line::styled(
            " r  toggle -Rns / -R",
            Style::default().fg(theme::FG_FAINT),
        ));
        lines.push(Line::default());
    }

    if modal.is_guarded() {
        lines.push(Line::styled(
            " ⚠ this touches a protected package:",
            Style::default().fg(theme::RED).add_modifier(Modifier::BOLD),
        ));
        for (name, reason) in &modal.guard_reasons {
            lines.push(Line::styled(
                format!("   {name} — {reason}"),
                Style::default().fg(theme::RED),
            ));
        }
        lines.push(Line::default());
        lines.push(Line::from(vec![
            Span::styled(
                format!(" type {CONFIRMATION_PHRASE} to confirm: "),
                Style::default().fg(theme::FG_DIM),
            ),
            Span::styled(
                modal.typed.clone(),
                Style::default().fg(theme::RED).add_modifier(Modifier::BOLD),
            ),
            Span::styled("█", Style::default().fg(theme::RED)),
        ]));
        lines.push(Line::default());
    }

    let footer_area_height = 1;
    let body_area = Rect {
        height: inner.height.saturating_sub(footer_area_height),
        ..inner
    };
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), body_area);

    let footer_area = Rect {
        y: inner.y + inner.height.saturating_sub(footer_area_height),
        height: footer_area_height,
        ..inner
    };
    render_footer(frame, footer_area, modal);
}

fn render_footer(frame: &mut Frame, area: Rect, modal: &Modal) {
    let confirm_color = if modal.can_confirm() {
        theme::GREEN
    } else {
        theme::FG_FAINT
    };
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
            " enter ",
            Style::default()
                .bg(confirm_color)
                .fg(theme::BG)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" confirm", Style::default().fg(theme::FG_DIM)),
    ];
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn title_for(modal: &Modal) -> &'static str {
    use crate::txn::Action;
    match &modal.action {
        Action::Install { .. } => "Install",
        Action::Remove { .. } => "Confirm Removal",
        Action::Reinstall { .. } => "Reinstall",
        Action::MarkExplicit { .. } => "Mark Explicit",
        Action::MarkDependency { .. } => "Mark as Dependency",
        Action::UpdateSystem => "Update System",
        Action::ListFiles { .. } => "List Files",
        Action::OpenUrl { .. } => "Open URL",
    }
}

fn content_height(modal: &Modal) -> u16 {
    let mut h = 6; // border/title + command lines + footer + padding
    if !modal.action.packages().is_empty() {
        h += 2;
    }
    if matches!(modal.action, crate::txn::Action::Remove { .. }) {
        h += 3;
    }
    if modal.is_guarded() {
        h += 3 + modal.guard_reasons.len() as u16;
    }
    h
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
