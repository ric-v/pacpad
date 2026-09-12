//! Everything to do with pacpad-managed launchers (webapps and TUI
//! app shortcuts): reading/writing `.desktop` files under the
//! ownership-marker invariant (`entry`), resolving a browser
//! (`browser`) or terminal (`terminal`) to actually launch with, and
//! fetching/storing an icon (`icon`). `webapp`/`tuiapp` compose these
//! into the two concrete launcher kinds the Apps tab manages.

pub mod browser;
pub mod entry;
pub mod icon;
pub mod terminal;
pub mod tuiapp;
pub mod webapp;
