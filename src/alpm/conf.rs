//! Parser for `/etc/pacman.conf`.
//!
//! We only need three things out of it: `DBPath` (where the local and
//! sync databases live), `RootDir`, and the list of configured repos
//! (every `[section]` other than `[options]`, in file order). Mirror
//! server lists, signature levels, and every other `[options]`/`Server`
//! key are irrelevant to a tool that only ever reads local `.db` files
//! and shells out to pacman/yay for writes -- so they're parsed far
//! enough to skip over, never stored.
//!
//! `Include = <path>` is expanded recursively and inline, exactly as
//! pacman's own parser treats it: on a real Arch/CachyOS install it
//! almost always points at a mirrorlist (`Server = ...` lines) sitting
//! inside a repo section, but the directive itself is generic -- an
//! included file's own `[section]` headers register as repos too. A
//! missing or unreadable include is skipped silently rather than
//! failing the whole parse: mirrorlists are written by `pacman -Sy`
//! and may simply not exist yet.

use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_CONF_PATH: &str = "/etc/pacman.conf";
const DEFAULT_DB_PATH: &str = "/var/lib/pacman/";
const DEFAULT_ROOT_DIR: &str = "/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacmanConfig {
    pub db_path: PathBuf,
    pub root_dir: PathBuf,
    /// Repo names in the order they appear in the config (Include
    /// expansions included), deduplicated by first occurrence.
    pub repos: Vec<String>,
}

impl Default for PacmanConfig {
    fn default() -> Self {
        PacmanConfig {
            db_path: PathBuf::from(DEFAULT_DB_PATH),
            root_dir: PathBuf::from(DEFAULT_ROOT_DIR),
            repos: Vec::new(),
        }
    }
}

impl PacmanConfig {
    pub fn local_db_dir(&self) -> PathBuf {
        self.db_path.join("local")
    }

    pub fn sync_db_dir(&self) -> PathBuf {
        self.db_path.join("sync")
    }

    /// Reads and parses the config at `path`, following `Include`
    /// directives. Missing includes are skipped; a missing top-level
    /// file is an error (there is no reasonable default to fall back
    /// to if `/etc/pacman.conf` itself is gone).
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("reading {}: {e}", path.display()))?;
        Ok(Self::parse(&text))
    }

    pub fn parse(text: &str) -> Self {
        let mut db_path = None;
        let mut root_dir = None;
        let mut repos: Vec<String> = Vec::new();
        let mut in_options = true;

        parse_into(
            text,
            &mut in_options,
            &mut db_path,
            &mut root_dir,
            &mut repos,
            0,
        );

        PacmanConfig {
            db_path: db_path.unwrap_or_else(|| PathBuf::from(DEFAULT_DB_PATH)),
            root_dir: root_dir.unwrap_or_else(|| PathBuf::from(DEFAULT_ROOT_DIR)),
            repos,
        }
    }
}

/// Recursion guard: a config that includes itself, directly or through
/// a chain, must not hang the parser.
const MAX_INCLUDE_DEPTH: u8 = 8;

fn parse_into(
    text: &str,
    in_options: &mut bool,
    db_path: &mut Option<PathBuf>,
    root_dir: &mut Option<PathBuf>,
    repos: &mut Vec<String>,
    depth: u8,
) {
    for raw_line in text.lines() {
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }

        if let Some(section) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            *in_options = section == "options";
            if !*in_options && !repos.iter().any(|r| r == section) {
                repos.push(section.to_string());
            }
            continue;
        }

        let Some((key, value)) = split_key_value(line) else {
            continue; // bare flag (Color, ILoveCandy, ...) -- nothing we track
        };

        match key {
            "DBPath" if *in_options => *db_path = Some(PathBuf::from(value)),
            "RootDir" if *in_options => *root_dir = Some(PathBuf::from(value)),
            "Include" => {
                if depth >= MAX_INCLUDE_DEPTH {
                    continue;
                }
                if let Ok(included) = fs::read_to_string(value) {
                    parse_into(&included, in_options, db_path, root_dir, repos, depth + 1);
                }
                // Unreadable include (mirrorlist not yet fetched, path
                // doesn't exist): silently skipped, matching pacman's
                // own tolerance for a missing mirrorlist at parse time.
            }
            _ => {}
        }
    }
}

fn strip_comment(line: &str) -> &str {
    match line.find('#') {
        Some(i) => &line[..i],
        None => line,
    }
}

fn split_key_value(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once('=')?;
    Some((key.trim(), value.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    fn fixture(name: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/conf")
            .join(name);
        fs::read_to_string(path).expect("fixture should exist")
    }

    #[test]
    fn vanilla_arch_repos_and_defaults() {
        let cfg = PacmanConfig::parse(&fixture("vanilla.conf"));
        assert_eq!(cfg.repos, vec!["core", "extra", "multilib"]);
        assert_eq!(cfg.db_path, PathBuf::from(DEFAULT_DB_PATH));
        assert_eq!(cfg.root_dir, PathBuf::from(DEFAULT_ROOT_DIR));
    }

    #[test]
    fn cachyos_multi_repo_order_preserved() {
        let cfg = PacmanConfig::parse(&fixture("cachyos.conf"));
        assert_eq!(
            cfg.repos,
            vec![
                "cachyos-v3",
                "cachyos-extra-v3",
                "cachyos-core-v3",
                "cachyos",
                "core",
                "extra",
                "multilib",
            ]
        );
    }

    #[test]
    fn custom_dbpath_rootdir_and_missing_include_is_harmless() {
        let cfg = PacmanConfig::parse(&fixture("with-glob-include.conf"));
        assert_eq!(cfg.db_path, PathBuf::from("/custom/lib/pacman/"));
        assert_eq!(cfg.root_dir, PathBuf::from("/custom/root/"));
        // The dangling `Include = /etc/pacman.d/local-repos.conf` must not
        // panic and must not fabricate a repo.
        assert_eq!(cfg.repos, vec!["core", "extra"]);
    }

    #[test]
    fn include_directive_recursively_registers_repos() {
        // Real pacman treats Include as literal inlining, so a file
        // included at top level can itself carry [section] headers.
        // Build two real temp files to exercise the actual filesystem
        // read path (not just string concatenation).
        let dir = env::temp_dir().join(format!("pacpad-conf-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let sub_path = dir.join("chaotic.conf");
        fs::write(
            &sub_path,
            "[chaotic-aur]\nInclude = /etc/pacman.d/chaotic-mirrorlist\n",
        )
        .unwrap();

        let main_text = format!(
            "[options]\nArchitecture = auto\n\n[core]\nInclude = /etc/pacman.d/mirrorlist\n\nInclude = {}\n",
            sub_path.display()
        );

        let cfg = PacmanConfig::parse(&main_text);
        assert_eq!(cfg.repos, vec!["core", "chaotic-aur"]);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn deeply_nested_self_include_does_not_hang() {
        let dir = env::temp_dir().join(format!("pacpad-conf-cycle-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cycle.conf");
        fs::write(&path, format!("[options]\nInclude = {}\n", path.display())).unwrap();

        let text = fs::read_to_string(&path).unwrap();
        // Must terminate (the assertion is just "we got here").
        let cfg = PacmanConfig::parse(&text);
        assert!(cfg.repos.is_empty());

        fs::remove_dir_all(&dir).ok();
    }
}
