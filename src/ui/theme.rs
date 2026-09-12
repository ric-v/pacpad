//! Colors, converted from the design canvas's oklch palette to sRGB
//! (via the standard OKLab formulas) rather than hand-picked -- so the
//! TUI actually matches the mockups instead of approximating them.

use ratatui::style::Color;

pub const BG: Color = Color::Rgb(4, 6, 8);
pub const FG: Color = Color::Rgb(214, 215, 218);
pub const FG_DIM: Color = Color::Rgb(120, 122, 126);
pub const FG_FAINT: Color = Color::Rgb(70, 72, 75);
pub const BORDER: Color = Color::Rgb(85, 88, 93);
pub const ACCENT: Color = Color::Rgb(0, 210, 211);
pub const GREEN: Color = Color::Rgb(83, 190, 112);
pub const AMBER: Color = Color::Rgb(230, 172, 61);
pub const RED: Color = Color::Rgb(236, 91, 87);
pub const MAGENTA: Color = Color::Rgb(209, 121, 202);
/// Pac-Man yellow -- used only by the startup splash and the Apps-tab
/// empty-state mascot, so it stays a treat rather than a color that
/// competes with the real status colors above.
pub const YELLOW: Color = Color::Rgb(241, 196, 15);
