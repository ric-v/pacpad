//! Everything to do with actually changing package state: turning an
//! `Action` into an argv (`plan`), catching casual removal of
//! anything that would break the system (`guard`), and actually
//! running the resulting command on a pty pacpad itself owns, embedded
//! in pacpad's own screen instead of handing the real terminal over to
//! it (`embed`).
//!
//! `plan` and `guard` are pure -- no I/O, fully unit-testable. `embed`
//! is the one place that spawns a process.

pub mod embed;
pub mod guard;
pub mod plan;

pub use embed::{encode_key, PtySession, RunningView};
pub use guard::{protected_among, CONFIRMATION_PHRASE};
pub use plan::{plan as plan_command, Action, PlannedCommand};
