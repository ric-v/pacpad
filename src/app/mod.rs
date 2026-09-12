//! Pure application state and its transitions -- no I/O, no ratatui.
//! `state.rs` is data; `actions.rs` mutates it; `input.rs` maps
//! keyboard events to actions. `ui/` (a sibling module) renders
//! `&AppState` and never mutates it.

pub mod actions;
pub mod input;
pub mod state;

pub use actions::Action;
pub use state::AppState;
