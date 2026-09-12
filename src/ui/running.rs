//! Renders the embedded pty pane (`txn::embed::RunningView`) as a
//! full-frame takeover -- the replacement for actually leaving
//! pacpad's screen. Pure rendering, like every other `ui::` module:
//! all the real work (spawning, reading, forwarding keystrokes)
//! happens in `cli::run_tui` and `txn::embed`; this only ever turns
//! the plain-data snapshot it's handed into styled `Line`s.

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color as RColor, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use ratatui::Frame;

use super::theme;
use crate::txn::embed::{CellColor, RunningView, TermCell};

/// The `(rows, cols)` the embedded pty should be sized to for a given
/// full-frame `area` -- shared by `render` (so what's actually drawn
/// matches) and `cli::run_tui` (so the child is spawned at the exact
/// size its output will be rendered into, before a single byte of its
/// output exists to size anything by).
///
/// Unlike the confirm modal (a small popup that deliberately leaves
/// the tab underneath visible around it), this fills the *entire*
/// frame: a real terminal session -- especially one that might be
/// showing a `sudo` password prompt -- reads as a takeover, not a
/// floating dialog, and every cell not spent on it is a column/row the
/// actual command doesn't get to use.
pub fn content_dims(area: Rect) -> (u16, u16) {
    let inner_h = area.height.saturating_sub(2); // block borders, top + bottom
    let inner_w = area.width.saturating_sub(2); // block borders, left + right
    let content_h = inner_h.saturating_sub(1); // footer row
    (content_h, inner_w)
}

pub fn render(frame: &mut Frame, area: Rect, view: &RunningView, tick: u64) {
    frame.render_widget(Clear, area);

    let (title, accent) = title_and_color(view, tick);
    let block = Block::bordered()
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(accent))
        .style(Style::default().bg(theme::BG).fg(theme::FG))
        .title(title)
        .title_alignment(Alignment::Center);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner);
    render_grid(frame, rows[0], view);
    render_footer(frame, rows[1], view);
}

fn title_and_color(view: &RunningView, tick: u64) -> (Line<'static>, RColor) {
    let (glyph, color, label) = match view.finished {
        None => {
            let mouth = if tick % 2 == 0 { "ᗧ" } else { "ᗤ" };
            (
                mouth.to_string(),
                theme::YELLOW,
                format!("Running: {}", view.title),
            )
        }
        Some(true) => (
            "✓".to_string(),
            theme::GREEN,
            format!("Done: {}", view.title),
        ),
        Some(false) => (
            "✗".to_string(),
            theme::RED,
            format!("Failed: {}", view.title),
        ),
    };
    let title = Line::from(vec![Span::styled(
        format!(" {glyph} {label} "),
        Style::default()
            .fg(theme::BG)
            .bg(color)
            .add_modifier(Modifier::BOLD),
    )]);
    (title, color)
}

fn render_footer(frame: &mut Frame, area: Rect, view: &RunningView) {
    let text = if view.finished.is_some() {
        "press any key to return to pacpad"
    } else {
        "your keystrokes go to the running command -- Ctrl+C interrupts it, not pacpad"
    };
    frame.render_widget(
        Paragraph::new(Line::styled(text, Style::default().fg(theme::FG_FAINT)))
            .alignment(Alignment::Center),
        area,
    );
}

fn render_grid(frame: &mut Frame, area: Rect, view: &RunningView) {
    let visible_rows = (area.height as usize).min(view.rows.len());
    let lines: Vec<Line> = view.rows[..visible_rows]
        .iter()
        .map(|row| render_row(row, area.width as usize))
        .collect();
    frame.render_widget(Paragraph::new(lines), area);

    if let Some((row, col)) = view.cursor {
        let (row, col) = (row as usize, col as usize);
        if row < area.height as usize && col < area.width as usize {
            let cell_rect = Rect {
                x: area.x + col as u16,
                y: area.y + row as u16,
                width: 1,
                height: 1,
            };
            let ch = view
                .rows
                .get(row)
                .and_then(|r| r.get(col))
                .map(|c| c.ch)
                .unwrap_or(' ');
            frame.render_widget(
                Paragraph::new(ch.to_string()).style(Style::default().bg(theme::FG).fg(theme::BG)),
                cell_rect,
            );
        }
    }
}

/// Coalesces runs of cells sharing the same style into a single
/// `Span` each, rather than one `Span` per character -- purely a size
/// optimization (a full row is otherwise up to ~200 tiny allocations,
/// times however many visible rows, every single frame).
fn render_row(cells: &[TermCell], max_cols: usize) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut run = String::new();
    let mut run_style: Option<Style> = None;

    for cell in cells.iter().take(max_cols) {
        let style = cell_style(cell);
        match run_style {
            Some(s) if s == style => run.push(cell.ch),
            Some(s) => {
                spans.push(Span::styled(std::mem::take(&mut run), s));
                run.push(cell.ch);
                run_style = Some(style);
            }
            None => {
                run.push(cell.ch);
                run_style = Some(style);
            }
        }
    }
    if let Some(s) = run_style {
        if !run.is_empty() {
            spans.push(Span::styled(run, s));
        }
    }
    Line::from(spans)
}

fn cell_style(cell: &TermCell) -> Style {
    let mut fg = convert(cell.fg).unwrap_or(theme::FG);
    let mut bg = convert(cell.bg).unwrap_or(theme::BG);
    if cell.inverse {
        std::mem::swap(&mut fg, &mut bg);
    }
    let mut style = Style::default().fg(fg).bg(bg);
    if cell.bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    if cell.underline {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    style
}

fn convert(c: CellColor) -> Option<RColor> {
    match c {
        CellColor::Default => None,
        CellColor::Indexed(i) => Some(RColor::Indexed(i)),
        CellColor::Rgb(r, g, b) => Some(RColor::Rgb(r, g, b)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(ch: char) -> TermCell {
        TermCell {
            ch,
            fg: CellColor::Default,
            bg: CellColor::Default,
            bold: false,
            underline: false,
            inverse: false,
        }
    }

    #[test]
    fn adjacent_cells_with_identical_style_coalesce_into_one_span() {
        let row = vec![cell('a'), cell('b'), cell('c')];
        let line = render_row(&row, 80);
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].content, "abc");
    }

    #[test]
    fn a_style_change_splits_into_a_new_span() {
        let mut bold_b = cell('b');
        bold_b.bold = true;
        let row = vec![cell('a'), bold_b, cell('c')];
        let line = render_row(&row, 80);
        assert_eq!(line.spans.len(), 3);
        assert_eq!(line.spans[0].content, "a");
        assert_eq!(line.spans[1].content, "b");
        assert_eq!(line.spans[2].content, "c");
    }

    #[test]
    fn inverse_swaps_foreground_and_background() {
        let mut inv = cell('x');
        inv.fg = CellColor::Rgb(1, 2, 3);
        inv.bg = CellColor::Rgb(4, 5, 6);
        inv.inverse = true;
        let style = cell_style(&inv);
        assert_eq!(style.fg, Some(RColor::Rgb(4, 5, 6)));
        assert_eq!(style.bg, Some(RColor::Rgb(1, 2, 3)));
    }

    #[test]
    fn row_rendering_truncates_to_the_available_width_without_panicking() {
        let row: Vec<TermCell> = "hello world".chars().map(cell).collect();
        let line = render_row(&row, 5);
        let total: usize = line.spans.iter().map(|s| s.content.len()).sum();
        assert_eq!(total, 5);
    }

    #[test]
    fn content_dims_never_panics_on_a_tiny_or_zero_sized_area() {
        for (w, h) in [(0, 0), (1, 1), (3, 3), (5, 5)] {
            let (rows, cols) = content_dims(Rect::new(0, 0, w, h));
            assert!(rows <= h && cols <= w);
        }
    }

    #[test]
    fn content_dims_accounts_for_the_block_border_and_footer_row() {
        let (rows, cols) = content_dims(Rect::new(0, 0, 120, 40));
        // full frame 120x40, inner (borders): 118x38, content: 118x37
        assert_eq!((rows, cols), (37, 118));
    }
}
