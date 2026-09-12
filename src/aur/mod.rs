//! Everything AUR-specific. For now that's just helper detection --
//! txn/plan.rs, app::actions, and launcher::webapp/tuiapp all treat
//! "an AUR helper" as an opaque `Option<&str>` binary name they
//! delegate to, never touching AUR concepts directly. Search across
//! the AUR itself (`aurweb`'s RPC, for surfacing not-yet-installed AUR
//! packages) is deliberately out of scope here -- pacpad only ever
//! knows about local + configured-repo packages until that's built,
//! so "AUR" today means "installed, but absent from every configured
//! repo" (`PackageEntry::is_foreign`), not a live AUR search result.

pub mod helper;
