//! Reads the local package database: `<DBPath>/local/<name>-<version>/desc`.
//!
//! This is the source of truth for "what's installed" -- the Installed
//! tab, and the install-reason (`%REASON%`) that drives the
//! Explicit/Dependency/Orphan filters.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::desc::DescBlock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstallReason {
    /// `%REASON%` absent from the desc file -- the common case.
    Explicit,
    /// `%REASON%` present (pacman only ever writes value `1`).
    Dependency,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalPackage {
    pub name: String,
    pub version: String,
    pub base: Option<String>,
    pub description: String,
    pub url: Option<String>,
    pub arch: Option<String>,
    pub license: Vec<String>,
    pub packager: Option<String>,
    pub build_date: Option<i64>,
    pub install_date: Option<i64>,
    /// `%SIZE%` -- bytes on disk once installed.
    pub installed_size: u64,
    pub reason: InstallReason,
    pub depends: Vec<String>,
    pub optdepends: Vec<String>,
    pub provides: Vec<String>,
    pub conflicts: Vec<String>,
    pub replaces: Vec<String>,
    pub groups: Vec<String>,
}

impl LocalPackage {
    /// `None` only when the desc block is missing the two fields pacman
    /// never omits (`NAME`, `VERSION`) -- a corrupt or foreign entry we
    /// have no business guessing at.
    fn from_desc(d: &DescBlock) -> Option<Self> {
        Some(LocalPackage {
            name: d.first_owned("NAME")?,
            version: d.first_owned("VERSION")?,
            base: d.first_owned("BASE"),
            description: d.first_owned("DESC").unwrap_or_default(),
            url: d.first_owned("URL"),
            arch: d.first_owned("ARCH"),
            license: d.all_owned("LICENSE"),
            packager: d.first_owned("PACKAGER"),
            build_date: d.first_i64("BUILDDATE"),
            install_date: d.first_i64("INSTALLDATE"),
            installed_size: d.first_u64("SIZE").unwrap_or(0),
            reason: if d.has("REASON") {
                InstallReason::Dependency
            } else {
                InstallReason::Explicit
            },
            depends: d.all_owned("DEPENDS"),
            optdepends: d.all_owned("OPTDEPENDS"),
            provides: d.all_owned("PROVIDES"),
            conflicts: d.all_owned("CONFLICTS"),
            replaces: d.all_owned("REPLACES"),
            groups: d.all_owned("GROUPS"),
        })
    }
}

/// Scans `<DBPath>/local/*/desc` and parses every entry.
///
/// A single unreadable or malformed entry is skipped rather than
/// failing the whole scan -- one damaged directory (e.g. a package
/// mid-install) shouldn't take down the entire Installed tab.
pub fn read_local_db(local_dir: &Path) -> anyhow::Result<Vec<LocalPackage>> {
    let mut out = Vec::new();
    let entries = fs::read_dir(local_dir)
        .map_err(|e| anyhow::anyhow!("reading local db dir {}: {e}", local_dir.display()))?;

    for entry in entries {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let desc_path = path.join("desc");
        let Ok(text) = fs::read_to_string(&desc_path) else {
            continue;
        };
        if let Some(pkg) = LocalPackage::from_desc(&DescBlock::parse(&text)) {
            out.push(pkg);
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/local")
    }

    #[test]
    fn reads_all_three_fixture_packages() {
        let pkgs = read_local_db(&fixtures_dir()).unwrap();
        assert_eq!(pkgs.len(), 3);
        let names: Vec<&str> = {
            let mut n: Vec<&str> = pkgs.iter().map(|p| p.name.as_str()).collect();
            n.sort_unstable();
            n
        };
        assert_eq!(names, vec!["eza", "htop", "ripgrep"]);
    }

    #[test]
    fn explicit_vs_dependency_reason() {
        let pkgs = read_local_db(&fixtures_dir()).unwrap();
        let ripgrep = pkgs.iter().find(|p| p.name == "ripgrep").unwrap();
        let htop = pkgs.iter().find(|p| p.name == "htop").unwrap();
        assert_eq!(ripgrep.reason, InstallReason::Explicit);
        assert_eq!(htop.reason, InstallReason::Dependency);
    }

    #[test]
    fn multi_value_fields_parsed_in_order() {
        let pkgs = read_local_db(&fixtures_dir()).unwrap();
        let eza = pkgs.iter().find(|p| p.name == "eza").unwrap();
        assert_eq!(eza.replaces, vec!["exa"]);
        assert_eq!(eza.conflicts, vec!["exa"]);
        assert_eq!(eza.provides, vec!["exa"]);
        assert_eq!(eza.depends, vec!["glibc", "libgcc"]);

        let htop = pkgs.iter().find(|p| p.name == "htop").unwrap();
        assert_eq!(htop.optdepends, vec!["lm_sensors: sensors support"]);
    }

    #[test]
    fn numeric_and_optional_fields() {
        let pkgs = read_local_db(&fixtures_dir()).unwrap();
        let ripgrep = pkgs.iter().find(|p| p.name == "ripgrep").unwrap();
        assert_eq!(ripgrep.version, "15.2.0-1");
        assert_eq!(ripgrep.installed_size, 3742597);
        assert_eq!(ripgrep.install_date, Some(1788972672));
        assert_eq!(ripgrep.license, vec!["MIT OR Unlicense"]);
        assert_eq!(
            ripgrep.url.as_deref(),
            Some("https://github.com/BurntSushi/ripgrep")
        );
    }

    #[test]
    fn nonexistent_dir_is_an_error_not_a_panic() {
        let missing = fixtures_dir().join("does-not-exist");
        assert!(read_local_db(&missing).is_err());
    }
}
