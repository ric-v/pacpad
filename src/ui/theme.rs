//! Washi and sumi-e color palette for pacpad.
//!
//! Grounded in traditional Japanese aesthetics:
//! - Deep carbon sumi ink (`sumi-iro`) background rather than cold digital black.
//! - Unbleached mulberry washi neutrals (`kinari` / `torinoko`) and ink washes for text hierarchy.
//! - Subtle charcoal ink lines (`keshi-zumi`) for borders.
//! - A deep indigo (`ai-iro`) as the primary UI accent.
//! - A single cinnabar vermillion / seal-red (`shu-iro`) accent for emphasis, errors, and stamps.
//! - Organic bamboo (`take-iro`) green and warm amber (`kohaku`) for status indicators.
//! - Warm yamabuki gold reserved for the Pac-Man mascot.

use ratatui::style::Color;

/// Deep carbon sumi ink (sumi-iro)
pub const BG: Color = Color::Rgb(22, 22, 21);

/// Unbleached washi paper cream (torinoko / kinari)
pub const FG: Color = Color::Rgb(222, 214, 200);

/// Mid-tone sumi wash (chu-sumi) for secondary text and hints
pub const FG_DIM: Color = Color::Rgb(147, 141, 130);

/// Light sumi wash (usu-sumi) for faint labels, shortcuts, and headers
pub const FG_FAINT: Color = Color::Rgb(92, 87, 78);

/// Charcoal ink line (keshi-zumi) for clean, quiet structural framing
pub const BORDER: Color = Color::Rgb(66, 61, 55);

/// Indigo dye (ai-iro) primary highlight: active tabs, focus, selection
pub const ACCENT: Color = Color::Rgb(97, 147, 190);

/// Bamboo / moss green (take-iro / moegi) for ok / installed status
pub const GREEN: Color = Color::Rgb(118, 159, 105);

/// Warm amber resin (kohaku / kuchiba) for foreign packages and warnings
pub const AMBER: Color = Color::Rgb(207, 142, 66);

/// Cinnabar vermillion / seal-red (shu-iro) for errors, destructive modals, and stamps
pub const RED: Color = Color::Rgb(208, 90, 63);

/// Formerly synthetic neon magenta; folded into vermillion seal-red (`RED`)
/// to maintain a single warm accent and eliminate competing neon hues.
#[allow(dead_code)]
pub const MAGENTA: Color = RED;

/// Yamabuki gold -- reserved for the startup splash and the Pac-Man
/// mascot animations so it stays a delight rather than competing
/// with operational UI status colors.
pub const YELLOW: Color = Color::Rgb(216, 174, 52);

