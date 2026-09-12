//! The right-hand detail pane shared by the Installed and Search
//! tabs: name/version/repo, description, a meta table, and the
//! dependency list -- mirroring the design canvas's detail pane.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use super::theme;
use crate::alpm::index::PackageEntry;
use crate::alpm::InstallReason;
use crate::time::unix_to_ymd as format_date;

pub fn render(frame: &mut Frame, area: Rect, entry: Option<&PackageEntry>) {
    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(theme::BORDER));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(entry) = entry else {
        frame.render_widget(
            Paragraph::new(Line::styled(
                "  (nothing selected)",
                Style::default().fg(theme::FG_FAINT),
            )),
            inner,
        );
        return;
    };

    let mut lines: Vec<Line> = Vec::new();

    let mut header = vec![Span::styled(
        format!(" {}", entry.name),
        Style::default().fg(theme::FG).add_modifier(Modifier::BOLD),
    )];
    if let Some(v) = entry.version() {
        header.push(Span::raw(" "));
        header.push(Span::styled(
            v.to_string(),
            Style::default().fg(theme::FG_DIM),
        ));
    }
    lines.push(Line::from(header));

    let repo_tag = match entry.repo() {
        Some(r) => format!(" [{r}]"),
        None if entry.is_foreign() => " [aur]".to_string(),
        None => String::new(),
    };
    if !repo_tag.is_empty() {
        let color = if entry.is_foreign() {
            theme::AMBER
        } else {
            theme::FG_FAINT
        };
        lines.push(Line::styled(repo_tag, Style::default().fg(color)));
    }

    lines.push(Line::default());
    let desc = entry.description();
    if !desc.is_empty() {
        lines.push(Line::styled(
            format!(" {desc}"),
            Style::default().fg(theme::FG_DIM),
        ));
        lines.push(Line::default());
    }

    meta_row(
        &mut lines,
        "SIZE",
        &format_size(installed_or_download_size(entry)),
    );
    if let Some(local) = &entry.local {
        if !local.license.is_empty() {
            meta_row(&mut lines, "LICENSE", &local.license.join(", "));
        }
        if let Some(date) = local.install_date {
            meta_row(&mut lines, "INSTALLED", &format_date(date));
        }
        let (reason_text, reason_color) = match local.reason {
            InstallReason::Explicit => ("Explicit", theme::GREEN),
            InstallReason::Dependency => ("Dependency", theme::FG_DIM),
        };
        meta_row_colored(&mut lines, "REASON", reason_text, reason_color);
    }
    if let Some(url) = entry
        .local
        .as_ref()
        .and_then(|l| l.url.as_deref())
        .or_else(|| entry.sync.as_ref().and_then(|s| s.url.as_deref()))
    {
        meta_row_colored(&mut lines, "UPSTREAM", url, theme::ACCENT);
    }

    let depends: &[String] = entry
        .local
        .as_ref()
        .map(|l| l.depends.as_slice())
        .or_else(|| entry.sync.as_ref().map(|s| s.depends.as_slice()))
        .unwrap_or(&[]);
    if !depends.is_empty() {
        lines.push(Line::default());
        lines.push(Line::styled(
            " DEPENDENCIES",
            Style::default().fg(theme::FG_FAINT),
        ));
        lines.push(Line::styled(
            format!(" {}", depends.join(" · ")),
            Style::default().fg(theme::FG_DIM),
        ));
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

/// The Apps tab's detail pane: same visual language as [`render`]
/// (name/badge header, a meta table, a bordered command block) but for
/// a pacpad-managed launcher rather than a package.
pub fn render_managed_entry(
    frame: &mut Frame,
    area: Rect,
    entry: Option<&crate::launcher::entry::ManagedEntry>,
) {
    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(theme::BORDER));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(entry) = entry else {
        frame.render_widget(
            Paragraph::new(Line::styled(
                "  (nothing selected)",
                Style::default().fg(theme::FG_FAINT),
            )),
            inner,
        );
        return;
    };

    use crate::launcher::entry::Kind;

    let mut lines: Vec<Line> = Vec::new();

    let kind_badge = match entry.kind {
        Kind::Webapp => "[webapp]",
        Kind::Tui => "[tui]",
    };
    lines.push(Line::from(vec![
        Span::styled(
            format!(" {}", entry.name),
            Style::default().fg(theme::FG).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(kind_badge, Style::default().fg(theme::FG_FAINT)),
    ]));

    if let Some(url) = &entry.url {
        lines.push(Line::styled(
            format!(" {url}"),
            Style::default().fg(theme::FG_FAINT),
        ));
    } else if let Some(command) = &entry.command {
        lines.push(Line::styled(
            format!(" {command}"),
            Style::default().fg(theme::FG_FAINT),
        ));
    }

    lines.push(Line::default());
    meta_row(
        &mut lines,
        "WINDOW CLASS",
        entry.window_class.as_deref().unwrap_or("—"),
    );
    if let Some(created) = entry.created.as_deref().and_then(|s| s.split('T').next()) {
        meta_row(&mut lines, "CREATED", created);
    }

    lines.push(Line::default());
    lines.push(Line::styled(
        " LAUNCH COMMAND",
        Style::default().fg(theme::FG_FAINT),
    ));
    lines.push(Line::styled(
        format!(" {}", entry.exec),
        Style::default().fg(theme::FG_DIM),
    ));

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn meta_row(lines: &mut Vec<Line<'static>>, label: &str, value: &str) {
    meta_row_colored(lines, label, value, theme::FG);
}

fn meta_row_colored(
    lines: &mut Vec<Line<'static>>,
    label: &str,
    value: &str,
    color: ratatui::style::Color,
) {
    // Width 14 covers the longest label used anywhere this renders
    // (`WINDOW CLASS`, 12 chars) with room for a visible gap -- this
    // was caught live: `{label:<10}` on a 12-char label doesn't
    // truncate, it just skips padding entirely, so the value ran
    // straight into the label with no space (`WINDOW CLASSpacpad-...`).
    lines.push(Line::from(vec![
        Span::styled(
            format!(" {label:<14}"),
            Style::default().fg(theme::FG_FAINT),
        ),
        Span::styled(value.to_string(), Style::default().fg(color)),
    ]));
}

fn installed_or_download_size(entry: &PackageEntry) -> u64 {
    entry
        .local
        .as_ref()
        .map(|l| l.installed_size)
        .or_else(|| entry.sync.as_ref().map(|s| s.installed_size))
        .unwrap_or(0)
}

pub(super) fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[0])
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_size_scales_units() {
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(4_900_000), "4.7 MB");
        assert_eq!(format_size(142_000_000), "135.4 MB");
    }
}
