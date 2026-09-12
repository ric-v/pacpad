//! `~/.config/pacpad/config.toml` -- entirely optional; every field
//! has a sensible auto-detected fallback (see `launcher::browser` and
//! `launcher::terminal`). A missing or malformed file is never fatal:
//! this is user convenience, not something pacpad depends on to run.

use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Config {
    /// Overrides browser auto-detection -- a binary name (looked up on
    /// `$PATH`) or an absolute path.
    pub browser: Option<String>,
    /// Overrides terminal auto-detection -- must be one of the known
    /// binaries in `launcher::terminal`'s table to have any effect.
    pub terminal: Option<String>,
    /// Overrides AUR helper auto-detection (see `aur::helper`) -- a
    /// binary name looked up on `$PATH`. Ignored (falls through to
    /// detection) if it doesn't resolve to something installed.
    pub aur_helper: Option<String>,
}

impl Config {
    pub fn load() -> Self {
        Self::load_from(&default_path())
    }

    pub fn load_from(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }
}

fn default_path() -> PathBuf {
    crate::xdg::config_home().join("pacpad").join("config.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_yields_defaults() {
        let cfg = Config::load_from(Path::new("/nonexistent/pacpad-config-test.toml"));
        assert!(cfg.browser.is_none());
        assert!(cfg.terminal.is_none());
    }

    #[test]
    fn malformed_toml_yields_defaults_not_a_panic() {
        let dir = std::env::temp_dir().join(format!("pacpad-cfg-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "this is not valid toml {{{").unwrap();

        let cfg = Config::load_from(&path);
        assert!(cfg.browser.is_none());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parses_both_overrides() {
        let dir = std::env::temp_dir().join(format!("pacpad-cfg-ok-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "browser = \"brave\"\nterminal = \"wezterm\"\n").unwrap();

        let cfg = Config::load_from(&path);
        assert_eq!(cfg.browser.as_deref(), Some("brave"));
        assert_eq!(cfg.terminal.as_deref(), Some("wezterm"));

        std::fs::remove_dir_all(&dir).ok();
    }
}
