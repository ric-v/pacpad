//! Merges the local and sync databases into one queryable index: for
//! every unique package name, what's installed (if anything) and what
//! each configured repo currently offers (if anything).
//!
//! Parsing every sync repo from scratch is not something you want to
//! pay for on every launch -- `extra.db` alone is several megabytes of
//! gzipped tar. The merged result is cached to disk, keyed on a
//! fingerprint of every source `.db` file's path/mtime/size, so a
//! second launch with nothing changed on disk skips parsing entirely.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use super::conf::PacmanConfig;
use super::local::{read_local_db, InstallReason, LocalPackage};
use super::sync::{read_all_sync_dbs, SyncPackage};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageEntry {
    pub name: String,
    /// `Some` if the package is installed on this system.
    pub local: Option<LocalPackage>,
    /// `Some` if a configured repo currently offers this package. A
    /// package can be `local`-only (installed but no longer in any
    /// repo -- often the AUR, sometimes a repo that dropped it) or
    /// `sync`-only (available but not installed); both are valid and
    /// meaningful states, not error cases.
    pub sync: Option<SyncPackage>,
}

impl PackageEntry {
    pub fn is_installed(&self) -> bool {
        self.local.is_some()
    }

    /// Installed, but absent from every configured repo -- the AUR
    /// case, though it can also mean a repo package that was dropped
    /// upstream.
    pub fn is_foreign(&self) -> bool {
        self.local.is_some() && self.sync.is_none()
    }

    pub fn repo(&self) -> Option<&str> {
        self.sync.as_ref().map(|s| s.repo.as_str())
    }

    /// Installed as a dependency and required by nothing currently
    /// installed -- matches `pacman -Qdt` semantics. `required` is
    /// [`PackageIndex::required_names`], passed in rather than
    /// recomputed per-entry since it's the same set for every check.
    pub fn is_orphan(&self, required: &std::collections::HashSet<String>) -> bool {
        let Some(local) = &self.local else {
            return false;
        };
        if local.reason != InstallReason::Dependency {
            return false;
        }
        if required.contains(&self.name) {
            return false;
        }
        !local
            .provides
            .iter()
            .any(|p| required.contains(&strip_version_constraint(p)))
    }

    pub fn description(&self) -> &str {
        self.local
            .as_ref()
            .map(|l| l.description.as_str())
            .or_else(|| self.sync.as_ref().map(|s| s.description.as_str()))
            .unwrap_or_default()
    }

    pub fn reason(&self) -> Option<InstallReason> {
        self.local.as_ref().map(|l| l.reason)
    }

    /// The version installed, if any -- otherwise the version a repo
    /// currently offers.
    pub fn version(&self) -> Option<&str> {
        self.local
            .as_ref()
            .map(|l| l.version.as_str())
            .or_else(|| self.sync.as_ref().map(|s| s.version.as_str()))
    }

    /// A newer version is available in a repo than what's installed.
    ///
    /// NOTE: this compares version strings for plain inequality, not
    /// full pacman `vercmp` (epoch:pkgver-pkgrel ordering, alpha vs.
    /// numeric segment rules). That's a known simplification -- see
    /// the plan's Updates tab, which needs real vercmp before this can
    /// tell "different" apart from "older". Fine for detecting *that*
    /// an update exists; not for ordering by how far behind.
    // Consumed by the Updates tab (Phase 2); not yet called from Phase 1's
    // headless `search` subcommand.
    #[allow(dead_code)]
    pub fn has_update(&self) -> bool {
        match (&self.local, &self.sync) {
            (Some(l), Some(s)) => l.version != s.version,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PackageIndex {
    pub entries: Vec<PackageEntry>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Counts {
    pub total: usize,
    pub explicit: usize,
    pub dependency: usize,
    pub foreign: usize,
    pub orphan: usize,
}

impl PackageIndex {
    pub fn build(locals: Vec<LocalPackage>, syncs: Vec<SyncPackage>) -> Self {
        let mut by_name: HashMap<String, PackageEntry> =
            HashMap::with_capacity(locals.len().max(syncs.len()));

        for l in locals {
            let name = l.name.clone();
            by_name
                .entry(name.clone())
                .or_insert_with(|| PackageEntry {
                    name,
                    local: None,
                    sync: None,
                })
                .local = Some(l);
        }
        for s in syncs {
            let name = s.name.clone();
            let entry = by_name.entry(name.clone()).or_insert_with(|| PackageEntry {
                name,
                local: None,
                sync: None,
            });
            // First repo wins on a name collision across repos (rare in
            // practice); later repos in config order are shadowed,
            // matching pacman's own resolution order.
            if entry.sync.is_none() {
                entry.sync = Some(s);
            }
        }

        let mut entries: Vec<PackageEntry> = by_name.into_values().collect();
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        PackageIndex { entries }
    }

    pub fn counts(&self) -> Counts {
        let mut c = Counts::default();
        for e in &self.entries {
            if let Some(reason) = e.reason() {
                c.total += 1;
                match reason {
                    InstallReason::Explicit => c.explicit += 1,
                    InstallReason::Dependency => c.dependency += 1,
                }
                if e.is_foreign() {
                    c.foreign += 1;
                }
            }
        }
        // Computed once outside the loop below -- `required_names`
        // rebuilds a full set by scanning every package's depends
        // list, so calling it per-entry inside a filter turned this
        // into O(entries^2). Caught by running the real TUI against
        // this machine's ~16k merged entries, where it looked like a
        // hang rather than a crash.
        let required = self.required_names();
        c.orphan = self
            .entries
            .iter()
            .filter(|e| e.is_orphan(&required))
            .count();
        c
    }

    /// Every name that some installed package's `%DEPENDS%` list
    /// requires, with version comparators stripped (`glibc>=2.31` ->
    /// `glibc`). A dependency-reason package is an orphan exactly when
    /// neither its own name nor anything it `%PROVIDES%` appears here
    /// -- the same rule `pacman -Qdt` uses, computed from local data
    /// alone (no reverse-dependency graph needs to be stored; this is
    /// cheap enough to recompute on demand for a few thousand
    /// packages).
    pub fn required_names(&self) -> std::collections::HashSet<String> {
        let mut set = std::collections::HashSet::new();
        for e in &self.entries {
            if let Some(local) = &e.local {
                for dep in &local.depends {
                    set.insert(strip_version_constraint(dep));
                }
            }
        }
        set
    }
}

fn strip_version_constraint(dep: &str) -> String {
    let end = dep.find(['<', '>', '=']).unwrap_or(dep.len());
    dep[..end].to_string()
}

/// Reads local + every configured sync repo from disk and merges them.
/// No caching -- callers that want the cache should use [`load_cached`].
pub fn load(cfg: &PacmanConfig) -> anyhow::Result<PackageIndex> {
    let locals = read_local_db(&cfg.local_db_dir())?;
    let syncs = read_all_sync_dbs(&cfg.sync_db_dir(), &cfg.repos);
    Ok(PackageIndex::build(locals, syncs))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SourceStamp {
    path: PathBuf,
    modified_secs: u64,
    len: u64,
}

#[derive(Serialize, Deserialize)]
struct CacheFile {
    fingerprint: Vec<SourceStamp>,
    entries: Vec<PackageEntry>,
}

/// One stamp per source `.db`/`desc`-tree we depend on: the local db
/// directory itself (its mtime changes whenever a package is
/// installed/removed) plus every configured repo's sync `.db` file.
fn fingerprint(cfg: &PacmanConfig) -> Vec<SourceStamp> {
    let mut stamps = Vec::new();
    if let Some(s) = stamp(&cfg.local_db_dir()) {
        stamps.push(s);
    }
    for repo in &cfg.repos {
        let db_file = cfg.sync_db_dir().join(format!("{repo}.db"));
        if let Some(s) = stamp(&db_file) {
            stamps.push(s);
        }
    }
    stamps
}

fn stamp(path: &Path) -> Option<SourceStamp> {
    let meta = fs::metadata(path).ok()?;
    let modified_secs = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(SourceStamp {
        path: path.to_path_buf(),
        modified_secs,
        len: meta.len(),
    })
}

/// Loads the merged index, using a bincode cache at `cache_path` when
/// its fingerprint still matches every source file on disk. Any cache
/// problem -- missing, corrupt, stale, unwritable -- falls back to a
/// full reparse rather than failing; the cache is purely an
/// optimization, never a dependency for correctness.
pub fn load_cached(cfg: &PacmanConfig, cache_path: &Path) -> anyhow::Result<PackageIndex> {
    let current = fingerprint(cfg);

    if let Ok(bytes) = fs::read(cache_path) {
        if let Ok((cached, _)) =
            bincode::serde::decode_from_slice::<CacheFile, _>(&bytes, bincode::config::standard())
        {
            if cached.fingerprint == current {
                return Ok(PackageIndex {
                    entries: cached.entries,
                });
            }
        }
    }

    let index = load(cfg)?;

    let cache = CacheFile {
        fingerprint: current,
        entries: index.entries.clone(),
    };
    if let Ok(bytes) = bincode::serde::encode_to_vec(&cache, bincode::config::standard()) {
        if let Some(parent) = cache_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(cache_path, bytes);
    }

    Ok(index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn cfg() -> PacmanConfig {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        PacmanConfig {
            db_path: root.clone(),
            root_dir: PathBuf::from("/"),
            repos: vec!["extra".to_string()],
        }
    }

    #[test]
    fn merges_local_and_sync_by_name() {
        let idx = load(&cfg()).unwrap();
        assert_eq!(idx.entries.len(), 4); // ripgrep, eza, htop (local) + fd (sync-only)

        let ripgrep = idx.entries.iter().find(|e| e.name == "ripgrep").unwrap();
        assert!(ripgrep.is_installed());
        assert!(!ripgrep.is_foreign());
        assert_eq!(ripgrep.repo(), Some("extra"));

        let fd = idx.entries.iter().find(|e| e.name == "fd").unwrap();
        assert!(!fd.is_installed());
        assert_eq!(fd.repo(), Some("extra"));

        let htop = idx.entries.iter().find(|e| e.name == "htop").unwrap();
        assert!(htop.is_installed());
        assert!(htop.is_foreign()); // not in the extra.db fixture
    }

    #[test]
    fn counts_match_reason_semantics() {
        let idx = load(&cfg()).unwrap();
        let c = idx.counts();
        assert_eq!(c.total, 3); // ripgrep, eza, htop are installed; fd is not
        assert_eq!(c.explicit, 2); // ripgrep, eza
        assert_eq!(c.dependency, 1); // htop
        assert_eq!(c.foreign, 1); // htop only
                                  // htop is dependency-reason and nothing in the fixture set
                                  // depends on it (or on anything it provides) -- a real orphan.
        assert_eq!(c.orphan, 1);
    }

    #[test]
    fn htop_is_the_orphan_ripgrep_and_eza_are_not() {
        let idx = load(&cfg()).unwrap();
        let required = idx.required_names();
        assert_eq!(
            required,
            std::collections::HashSet::from([
                "glibc".to_string(),
                "libgcc".to_string(),
                "pcre2".to_string(),
                "ncurses".to_string()
            ])
        );

        let htop = idx.entries.iter().find(|e| e.name == "htop").unwrap();
        let ripgrep = idx.entries.iter().find(|e| e.name == "ripgrep").unwrap();
        let eza = idx.entries.iter().find(|e| e.name == "eza").unwrap();

        assert!(htop.is_orphan(&required));
        assert!(!ripgrep.is_orphan(&required)); // explicit, not a dependency at all
        assert!(!eza.is_orphan(&required)); // explicit
    }

    #[test]
    fn entries_sorted_by_name() {
        let idx = load(&cfg()).unwrap();
        let names: Vec<&str> = idx.entries.iter().map(|e| e.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);
    }

    #[test]
    fn cache_round_trips_and_is_reused_when_fresh() {
        let dir =
            std::env::temp_dir().join(format!("pacpad-index-cache-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cache_path = dir.join("index.bin");

        let first = load_cached(&cfg(), &cache_path).unwrap();
        assert!(cache_path.exists());
        assert_eq!(first.entries.len(), 4);

        // Second call must return the same data via the cache path
        // (fingerprint unchanged -- fixtures didn't move).
        let second = load_cached(&cfg(), &cache_path).unwrap();
        assert_eq!(second.entries.len(), first.entries.len());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn corrupt_cache_falls_back_to_full_parse() {
        let dir =
            std::env::temp_dir().join(format!("pacpad-index-cache-corrupt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cache_path = dir.join("index.bin");
        std::fs::write(&cache_path, b"not a valid bincode cache file at all").unwrap();

        let idx = load_cached(&cfg(), &cache_path).unwrap();
        assert_eq!(idx.entries.len(), 4);

        std::fs::remove_dir_all(&dir).ok();
    }
}
