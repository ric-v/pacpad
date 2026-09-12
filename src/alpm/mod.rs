//! Everything to do with reading pacman's own on-disk state: parsing
//! `/etc/pacman.conf`, the local package database, and sync repo
//! databases, then merging them into one searchable index.
//!
//! Deliberately read-only. Every write (install/remove/mark/update)
//! shells out to `pacman`/`yay` elsewhere in the tree (`txn/`) -- this
//! module never links against `libalpm` and never invokes pacman
//! itself, so a pacman upgrade that bumps its soname or output format
//! can't break the read path.

pub mod conf;
pub mod desc;
pub mod index;
pub mod local;
pub mod search;
pub mod sync;

// Re-exported public surface for the phases that consume this module
// next (the TUI's app/state and the launcher code). Not everything
// here is used yet within Phase 1 itself.
#[allow(unused_imports)]
pub use conf::PacmanConfig;
#[allow(unused_imports)]
pub use index::{load, load_cached, Counts, PackageEntry, PackageIndex};
#[allow(unused_imports)]
pub use local::{InstallReason, LocalPackage};
#[allow(unused_imports)]
pub use search::{search, SearchHit};
#[allow(unused_imports)]
pub use sync::SyncPackage;
