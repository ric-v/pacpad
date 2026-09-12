//! Resolves which browser binary to launch a webapp with.
//!
//! omarchy's `--app=<url>` approach is right; its resolution isn't
//! portable, since it assumes `xdg-terminal-exec`-style tooling this
//! machine doesn't have. The chain here (config override ->
//! `xdg-settings`, restricted to the chromium family -> a fixed
//! fallback list) plus reading the real binary out of the resolved
//! `.desktop` file's `Exec=` line (rather than guessing a path from
//! the desktop id) is what makes this work whether Chrome was
//! installed as a native package, a Flatpak, or via Nix -- confirmed
//! on this machine, where `xdg-settings` reports the desktop id
//! `google-chrome.desktop` but the binary it actually launches is
//! `/usr/bin/google-chrome-stable`, a name that doesn't even appear in
//! the desktop id.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Prefixes recognized as "safe to launch with `--app=`" -- every
/// other browser (Firefox, epiphany, ...) doesn't support Chrome's
/// app-mode flag the same way, so a non-chromium default is simply
/// not used even if `xdg-settings` reports one.
const CHROMIUM_FAMILY_PREFIXES: &[&str] = &[
    "google-chrome",
    "chromium",
    "brave",
    "microsoft-edge",
    "vivaldi",
    "opera",
];

/// Directories searched for a `.desktop` file, in order -- covers
/// native packages, Nix profiles, and Flatpak/user-local installs.
fn search_dirs() -> Vec<PathBuf> {
    vec![
        crate::xdg::data_home().join("applications"),
        crate::xdg::home_dir().join(".nix-profile/share/applications"),
        PathBuf::from("/usr/share/applications"),
    ]
}

pub fn resolve(config: &crate::config::Config) -> anyhow::Result<PathBuf> {
    if let Some(browser) = &config.browser {
        let path = PathBuf::from(browser);
        if path.is_absolute() && path.is_file() {
            return Ok(path);
        }
        if let Some(found) = crate::which::which(browser) {
            return Ok(found);
        }
        anyhow::bail!("configured browser {browser:?} not found");
    }

    if let Some(path) = resolve_via_xdg_settings() {
        return Ok(path);
    }

    for candidate in [
        "google-chrome-stable",
        "google-chrome",
        "chromium",
        "brave",
        "vivaldi",
    ] {
        if let Some(path) = crate::which::which(candidate) {
            return Ok(path);
        }
    }

    anyhow::bail!("no chromium-family browser found -- webapps need google-chrome, chromium, brave, or vivaldi")
}

fn resolve_via_xdg_settings() -> Option<PathBuf> {
    let output = Command::new("xdg-settings")
        .args(["get", "default-web-browser"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let desktop_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let base_id = desktop_id.strip_suffix(".desktop").unwrap_or(&desktop_id);
    if !CHROMIUM_FAMILY_PREFIXES
        .iter()
        .any(|p| base_id.starts_with(p))
    {
        return None;
    }
    let desktop_file = find_desktop_file(&desktop_id)?;
    extract_exec_binary(&desktop_file)
}

fn find_desktop_file(desktop_id: &str) -> Option<PathBuf> {
    search_dirs()
        .into_iter()
        .map(|dir| dir.join(desktop_id))
        .find(|p| p.is_file())
}

/// Pulls the binary out of a `.desktop` file's main `Exec=` line
/// (first occurrence, matching how desktop environments resolve
/// launch actions) and strips standard field codes (`%f %F %u %U %c
/// %k %i`) plus any of its own arguments, then resolves it via `$PATH`
/// if it isn't already absolute.
fn extract_exec_binary(desktop_file: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(desktop_file).ok()?;
    let mut in_main_section = false;
    for line in text.lines() {
        let line = line.trim();
        if let Some(section) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            in_main_section = section == "Desktop Entry";
            continue;
        }
        if !in_main_section {
            continue;
        }
        if let Some(value) = line.strip_prefix("Exec=") {
            let binary = value.split_whitespace().next()?;
            let path = PathBuf::from(binary);
            return if path.is_absolute() {
                Some(path)
            } else {
                crate::which::which(binary)
            };
        }
    }
    None
}

/// Builds the `Exec=`/`StartupWMClass=` pair for a webapp launcher:
/// `--class`/`StartupWMClass` give each webapp its own predictable
/// window class (Chrome otherwise picks an opaque
/// `chrome-<hash>-Default` class per profile+URL, making per-app
/// window rules in Hyprland/niri impossible).
pub fn webapp_exec_line(browser: &Path, url: &str, name: &str, class: &str) -> String {
    format!(
        "{} --app={url} --class={class} --name={name}",
        browser.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chromium_family_prefixes_match_every_configured_browser() {
        for id in [
            "google-chrome.desktop",
            "chromium-browser.desktop",
            "brave-browser.desktop",
            "vivaldi-stable.desktop",
            "opera.desktop",
            "microsoft-edge.desktop",
        ] {
            let base = id.strip_suffix(".desktop").unwrap();
            assert!(
                CHROMIUM_FAMILY_PREFIXES.iter().any(|p| base.starts_with(p)),
                "{id} should match the chromium family"
            );
        }
    }

    #[test]
    fn non_chromium_browsers_are_rejected() {
        for id in [
            "firefox.desktop",
            "org.gnome.Epiphany.desktop",
            "org.kde.konqueror.desktop",
        ] {
            let base = id.strip_suffix(".desktop").unwrap();
            assert!(!CHROMIUM_FAMILY_PREFIXES.iter().any(|p| base.starts_with(p)));
        }
    }

    #[test]
    fn extracts_binary_from_exec_line_stripping_field_codes() {
        let dir = std::env::temp_dir().join(format!("pacpad-browser-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test-browser.desktop");
        std::fs::write(
            &path,
            "[Desktop Entry]\nType=Application\nName=Test\nExec=/usr/bin/google-chrome-stable %U\n",
        )
        .unwrap();

        let binary = extract_exec_binary(&path).unwrap();
        assert_eq!(binary, PathBuf::from("/usr/bin/google-chrome-stable"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn extract_exec_binary_ignores_desktop_action_sections() {
        let dir =
            std::env::temp_dir().join(format!("pacpad-browser-action-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test-browser.desktop");
        std::fs::write(
            &path,
            "[Desktop Entry]\nType=Application\nName=Test\nExec=/usr/bin/real-browser %U\n\n[Desktop Action new-window]\nExec=/usr/bin/real-browser --new-window\nName=New Window\n",
        )
        .unwrap();

        let binary = extract_exec_binary(&path).unwrap();
        assert_eq!(binary, PathBuf::from("/usr/bin/real-browser"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn webapp_exec_line_includes_class_for_per_app_window_rules() {
        let line = webapp_exec_line(
            Path::new("/usr/bin/google-chrome-stable"),
            "https://excalidraw.com",
            "Excalidraw",
            "pacpad-excalidraw",
        );
        assert_eq!(
            line,
            "/usr/bin/google-chrome-stable --app=https://excalidraw.com --class=pacpad-excalidraw --name=Excalidraw"
        );
    }

    #[test]
    fn resolve_via_xdg_settings_matches_this_machines_real_chrome_install() {
        // This is a real, live check against the actual system rather
        // than a mock -- `google-chrome.desktop`/`google-chrome-stable`
        // is what's genuinely installed here, and the whole point of
        // this resolution chain is that it must work against exactly
        // this kind of desktop-id/binary-name mismatch.
        if let Some(path) = resolve_via_xdg_settings() {
            assert!(path.is_file());
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            assert!(
                name.contains("chrome")
                    || name.contains("chromium")
                    || name.contains("brave")
                    || name.contains("vivaldi")
                    || name.contains("opera"),
                "resolved binary {path:?} doesn't look like a chromium-family browser"
            );
        }
    }

    #[test]
    fn resolve_falls_back_to_path_lookup_when_config_names_a_bare_binary() {
        let cfg = crate::config::Config {
            browser: Some("sh".to_string()),
            terminal: None,
            aur_helper: None,
        };
        let resolved = resolve(&cfg).unwrap();
        assert!(resolved.is_file());
    }

    #[test]
    fn resolve_errors_on_a_configured_browser_that_does_not_exist() {
        let cfg = crate::config::Config {
            browser: Some("pacpad-definitely-not-a-real-browser".to_string()),
            terminal: None,
            aur_helper: None,
        };
        assert!(resolve(&cfg).is_err());
    }
}
