//! Composes `browser` + `icon` + `entry` into the webapp launcher
//! feature: a Chrome (or other chromium-family) app-mode window with
//! its own predictable class, listed and removable from the Apps tab.

use std::path::{Path, PathBuf};

use super::entry::{self, Kind, ManagedEntry, NewEntry};
use super::{browser, icon};
use crate::config::Config;

pub struct NewWebapp {
    pub name: String,
    pub url: String,
    /// `None` triggers the same Google-favicon auto-fetch omarchy's
    /// own installer uses; `Some` is either a URL or a local path,
    /// disambiguated by whether it parses as one.
    pub icon: Option<String>,
}

/// `apps_dir` is a parameter (rather than always `xdg::applications_dir()`
/// internally) purely for testability -- callers pass
/// `xdg::applications_dir()` in real use, exactly like `entry::write`
/// itself already requires.
pub fn add(apps_dir: &Path, new: &NewWebapp, config: &Config) -> anyhow::Result<PathBuf> {
    let entry = build_entry(new, config)?;
    entry::write(apps_dir, &entry)
}

pub fn update(path: &Path, new: &NewWebapp, config: &Config) -> anyhow::Result<()> {
    let entry = build_entry(new, config)?;
    entry::update(path, &entry)
}

fn build_entry(new: &NewWebapp, config: &Config) -> anyhow::Result<NewEntry> {
    let browser_bin = browser::resolve(config)?;
    let class = format!("pacpad-{}", entry::slugify(&new.name));
    let icon_path = resolve_icon(&new.name, new.icon.as_deref(), &new.url);
    let exec = browser::webapp_exec_line(&browser_bin, &new.url, &new.name, &class);
    Ok(NewEntry {
        name: new.name.clone(),
        kind: Kind::Webapp,
        exec,
        icon: icon_path,
        url: Some(new.url.clone()),
        command: None,
        window_class: Some(class),
    })
}

/// Best-effort: a failed icon fetch/copy never blocks creating the
/// webapp itself -- a launcher with no icon is still useful, so the
/// error is swallowed here rather than propagated.
fn resolve_icon(name: &str, requested: Option<&str>, app_url: &str) -> Option<PathBuf> {
    icon::resolve_and_store(name, icon_source(requested, app_url)).ok()
}

/// What the user typed into the "Icon" field decides which
/// `IconSource` to use: nothing (or blank) means auto-fetch a
/// favicon, an `http(s)://` string is a URL to fetch directly,
/// anything else is treated as a local file path to copy in.
fn icon_source(requested: Option<&str>, app_url: &str) -> icon::IconSource {
    match requested {
        None | Some("") => icon::IconSource::AutoFavicon {
            app_url: app_url.to_string(),
        },
        Some(value) if value.starts_with("http://") || value.starts_with("https://") => {
            icon::IconSource::Url(value.to_string())
        }
        Some(value) => icon::IconSource::LocalPath(PathBuf::from(value)),
    }
}

pub fn remove(path: &Path) -> anyhow::Result<()> {
    entry::remove(path)
}

pub fn list(apps_dir: &Path) -> Vec<ManagedEntry> {
    entry::list_managed(apps_dir)
        .into_iter()
        .filter(|e| e.kind == Kind::Webapp)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(label: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("pacpad-webapp-test-{label}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn icon_source_routes_by_what_the_user_typed() {
        assert!(
            matches!(icon_source(Some("https://example.com/icon.png"), "https://app.example.com"), icon::IconSource::Url(u) if u == "https://example.com/icon.png")
        );
        assert!(matches!(
            icon_source(
                Some("http://example.com/icon.png"),
                "https://app.example.com"
            ),
            icon::IconSource::Url(_)
        ));
        assert!(
            matches!(icon_source(Some("/home/me/icon.png"), "https://app.example.com"), icon::IconSource::LocalPath(p) if p.as_path() == Path::new("/home/me/icon.png"))
        );
        assert!(
            matches!(icon_source(None, "https://app.example.com"), icon::IconSource::AutoFavicon { app_url } if app_url == "https://app.example.com")
        );
        assert!(matches!(
            icon_source(Some(""), "https://app.example.com"),
            icon::IconSource::AutoFavicon { .. }
        ));
    }

    #[test]
    fn add_fails_clearly_when_no_chromium_browser_is_configured_or_found() {
        // A config override naming a nonexistent browser makes
        // `browser::resolve` fail deterministically regardless of what
        // browsers happen to be installed on the machine running the
        // test, so `add` propagating that error (rather than writing a
        // broken launcher) is what's actually under test here.
        let dir = tmp_dir("no-browser");
        let cfg = Config {
            browser: Some("pacpad-definitely-not-a-real-browser".to_string()),
            terminal: None,
            aur_helper: None,
        };
        let new = NewWebapp {
            name: "Test".to_string(),
            url: "https://example.com".to_string(),
            icon: None,
        };

        let result = add(&dir, &new, &cfg);
        assert!(result.is_err());
        assert!(
            entry::list_managed(&dir).is_empty(),
            "no half-written entry should be left behind"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn add_end_to_end_with_a_local_icon_writes_a_listable_managed_entry() {
        let dir = tmp_dir("end-to-end");
        let icon_src_dir = tmp_dir("end-to-end-icon-src");
        let icon_path = icon_src_dir.join("mine.png");
        std::fs::write(&icon_path, b"\x89PNG\r\n\x1a\nfake-but-magic-correct").unwrap();

        // A bare binary name that's virtually guaranteed to exist,
        // standing in for a real chromium-family browser so this test
        // doesn't depend on Chrome actually being installed.
        let cfg = Config {
            browser: Some("sh".to_string()),
            terminal: None,
            aur_helper: None,
        };
        let new = NewWebapp {
            name: "Excalidraw".to_string(),
            url: "https://excalidraw.com".to_string(),
            icon: Some(icon_path.display().to_string()),
        };

        let path = add(&dir, &new, &cfg).unwrap();
        assert!(path.exists());

        let listed = list(&dir);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "Excalidraw");
        assert_eq!(listed[0].url.as_deref(), Some("https://excalidraw.com"));
        assert_eq!(listed[0].window_class.as_deref(), Some("pacpad-excalidraw"));
        assert!(listed[0].exec.contains("--app=https://excalidraw.com"));
        assert!(listed[0].exec.contains("--class=pacpad-excalidraw"));

        remove(&path).unwrap();
        assert!(list(&dir).is_empty());

        std::fs::remove_dir_all(&dir).ok();
        std::fs::remove_dir_all(&icon_src_dir).ok();
    }
}
