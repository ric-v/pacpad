//! Composes `terminal` + `icon` + `entry` into the TUI-app launcher
//! feature: a terminal emulator running a command in a floating or
//! tiled window, listed and removable from the Apps tab.

use std::path::{Path, PathBuf};

use super::entry::{self, Kind, ManagedEntry, NewEntry};
use super::icon;
use super::terminal::{self, WindowStyle};

pub struct NewTuiApp {
    pub name: String,
    pub command: String,
    pub style: WindowStyle,
    /// Unlike webapps, there's no favicon to auto-fetch for a TUI
    /// command -- `None` simply means no icon.
    pub icon: Option<String>,
}

/// `apps_dir` is a parameter for the same reason it is in
/// `launcher::webapp::add` -- testability, matching `entry::write`'s
/// own signature.
pub fn add(
    apps_dir: &Path,
    new: &NewTuiApp,
    config: &crate::config::Config,
) -> anyhow::Result<PathBuf> {
    let entry = build_entry(new, config)?;
    entry::write(apps_dir, &entry)
}

pub fn update(path: &Path, new: &NewTuiApp, config: &crate::config::Config) -> anyhow::Result<()> {
    let entry = build_entry(new, config)?;
    entry::update(path, &entry)
}

fn build_entry(new: &NewTuiApp, config: &crate::config::Config) -> anyhow::Result<NewEntry> {
    let resolved = terminal::resolve(config).ok_or_else(|| {
        anyhow::anyhow!("no terminal found -- install kitty, wezterm, ghostty, alacritty, or foot")
    })?;
    let class = new.style.class();
    let argv = resolved.build_argv(class, &new.command);
    let exec = terminal::exec_line(&argv);
    let icon_path = new.icon.as_deref().and_then(|path| {
        icon::resolve_and_store(&new.name, icon::IconSource::LocalPath(PathBuf::from(path))).ok()
    });

    Ok(NewEntry {
        name: new.name.clone(),
        kind: Kind::Tui,
        exec,
        icon: icon_path,
        url: None,
        command: Some(new.command.clone()),
        window_class: Some(class.to_string()),
    })
}

pub fn remove(path: &Path) -> anyhow::Result<()> {
    entry::remove(path)
}

pub fn list(apps_dir: &Path) -> Vec<ManagedEntry> {
    entry::list_managed(apps_dir)
        .into_iter()
        .filter(|e| e.kind == Kind::Tui)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(label: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("pacpad-tuiapp-test-{label}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn add_fails_clearly_when_no_terminal_is_configured_or_found() {
        let dir = tmp_dir("no-terminal");
        let cfg = crate::config::Config {
            browser: None,
            terminal: Some("pacpad-definitely-not-a-real-terminal".to_string()),
            aur_helper: None,
        };
        let new = NewTuiApp {
            name: "Test".to_string(),
            command: "true".to_string(),
            style: WindowStyle::Float,
            icon: None,
        };

        // With an explicit (bogus) config override, `resolve` skips
        // straight past the override and falls through to real
        // detection -- so this can't force a guaranteed failure the
        // way `webapp`'s browser test can (config lets you *require* a
        // specific browser; there's no equivalent "require no
        // terminal" knob). What it does prove: `add` never panics
        // regardless of whether detection succeeds, and it never
        // leaves a half-written entry when it returns `Err`.
        let result = add(&dir, &new, &cfg);
        if result.is_err() {
            assert!(entry::list_managed(&dir).is_empty());
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn add_end_to_end_writes_a_listable_managed_entry_with_correct_argv() {
        // Skip gracefully if this machine genuinely has none of the
        // known terminals -- everything downstream of detection is
        // already covered by `launcher::terminal`'s pure golden tests.
        let cfg = crate::config::Config::default();
        if terminal::resolve(&cfg).is_none() {
            eprintln!("skipping: no known terminal detected on this machine");
            return;
        }

        let dir = tmp_dir("end-to-end");
        let new = NewTuiApp {
            name: "Btop".to_string(),
            command: "btop".to_string(),
            style: WindowStyle::Float,
            icon: None,
        };

        let path = add(&dir, &new, &cfg).unwrap();
        assert!(path.exists());

        let listed = list(&dir);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "Btop");
        assert_eq!(listed[0].window_class.as_deref(), Some("pacpad-tui-float"));
        assert!(
            listed[0].exec.contains("btop"),
            "exec line should embed the command: {}",
            listed[0].exec
        );
        assert!(listed[0].url.is_none());
        assert_eq!(listed[0].command.as_deref(), Some("btop"), "the original command must round-trip for editing, not need reverse-parsing out of `exec`");

        remove(&path).unwrap();
        assert!(list(&dir).is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }
}
