//! Reading and writing `.desktop` files, and the one invariant that
//! makes uninstalling a launcher safe:
//!
//! **pacpad never deletes or modifies a `.desktop` file that lacks
//! `X-PacPad-Managed=true`.**
//!
//! `~/.local/share/applications/` on a real machine already holds
//! entries pacpad did not create -- terminal emulators, other apps'
//! PWAs, editors. Deleting the wrong one is the worst bug this project
//! can have, so the check happens by re-reading the file from disk
//! immediately before every delete, never by trusting an
//! already-parsed struct that might be stale.
//!
//! Every entry pacpad writes also gets a `pacpad-` filename prefix, as
//! a second, independent line of defense: even if the marker check
//! were ever bypassed, pacpad's own files can never collide with an
//! existing name in the first place.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const MANAGED_KEY: &str = "X-PacPad-Managed";
const KIND_KEY: &str = "X-PacPad-Kind";
const URL_KEY: &str = "X-PacPad-Url";
const COMMAND_KEY: &str = "X-PacPad-Command";
const ICON_KEY: &str = "X-PacPad-Icon";
const CREATED_KEY: &str = "X-PacPad-Created";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Webapp,
    Tui,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Webapp => "webapp",
            Kind::Tui => "tui",
        }
    }

    fn parse(s: &str) -> Option<Kind> {
        match s {
            "webapp" => Some(Kind::Webapp),
            "tui" => Some(Kind::Tui),
            _ => None,
        }
    }
}

/// What pacpad needs to create a launcher. `window_class` is used both
/// for `StartupWMClass` and embedded in `exec` by the caller
/// (`launcher::webapp`/`launcher::tuiapp`) before this struct is built
/// -- `entry.rs` itself doesn't know how a browser or terminal
/// constructs its argv, only how to serialize the result.
pub struct NewEntry {
    pub name: String,
    pub kind: Kind,
    pub exec: String,
    pub icon: Option<PathBuf>,
    pub url: Option<String>,
    /// The TUI app's original, unresolved command as the user typed it
    /// (e.g. `btop`, or `bash -c 'dust; read -n 1 -s'`) -- stored
    /// separately from `exec` (which embeds it inside the terminal's
    /// own argv) so editing a TUI launcher can recover exactly what
    /// was typed, the same way `url` lets editing a webapp recover its
    /// URL without reverse-parsing `exec`. `None` for webapps.
    pub command: Option<String>,
    pub window_class: Option<String>,
}

/// A `.desktop` file pacpad created and recognizes as its own --
/// parsed fresh from disk, never cached longer than one render/action
/// cycle (the Apps tab rebuilds this list from disk every time it's
/// entered, exactly like a filesystem listing would).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedEntry {
    pub path: PathBuf,
    pub name: String,
    pub kind: Kind,
    pub exec: String,
    pub icon: Option<PathBuf>,
    pub url: Option<String>,
    pub command: Option<String>,
    pub window_class: Option<String>,
    pub created: Option<String>,
}

/// A slug safe to use in a filename and as a window class: lowercase,
/// alphanumerics kept, everything else collapsed to a single `-`,
/// trimmed of leading/trailing `-`.
pub fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut last_was_dash = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash && !out.is_empty() {
            out.push('-');
            last_was_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("app");
    }
    out
}

/// Parses the `[Desktop Entry]` section of a `.desktop` file into a
/// flat key -> value map. Other sections (`[Desktop Action ...]`) are
/// skipped entirely -- pacpad never writes or reads those. Comments
/// (`#`) and malformed lines are silently ignored, matching the
/// tolerance of every other format reader in this codebase: a
/// corrupt or hand-edited file degrades to "missing field", not a
/// crash.
fn parse_desktop_entry(text: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    let mut in_target_section = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(section) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            in_target_section = section == "Desktop Entry";
            continue;
        }
        if !in_target_section {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            fields.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    fields
}

fn parse_managed_entry(path: &Path, text: &str) -> Option<ManagedEntry> {
    let fields = parse_desktop_entry(text);
    if fields.get(MANAGED_KEY).map(|v| v.as_str()) != Some("true") {
        return None;
    }
    let kind = Kind::parse(fields.get(KIND_KEY)?)?;
    Some(ManagedEntry {
        path: path.to_path_buf(),
        name: fields.get("Name")?.clone(),
        kind,
        exec: fields.get("Exec")?.clone(),
        icon: fields.get(ICON_KEY).map(PathBuf::from),
        url: fields.get(URL_KEY).cloned(),
        command: fields.get(COMMAND_KEY).cloned(),
        window_class: fields.get("StartupWMClass").cloned(),
        created: fields.get(CREATED_KEY).cloned(),
    })
}

/// Every pacpad-managed entry under `apps_dir`, in filename order.
/// Unmanaged `.desktop` files (the common case -- terminal emulators,
/// other apps' launchers) are invisible here by construction, not by
/// a filter the caller has to remember to apply.
pub fn list_managed(apps_dir: &Path) -> Vec<ManagedEntry> {
    let Ok(read_dir) = fs::read_dir(apps_dir) else {
        return Vec::new();
    };
    let mut entries: Vec<ManagedEntry> = read_dir
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("desktop"))
        .filter_map(|path| {
            let text = fs::read_to_string(&path).ok()?;
            parse_managed_entry(&path, &text)
        })
        .collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

/// Writes a new managed `.desktop` file into `apps_dir`, named
/// `pacpad-<slug>.desktop`. Refuses to overwrite an existing file at
/// that path -- `launcher::webapp`/`launcher::tuiapp` are responsible
/// for picking a name that doesn't collide (or explicitly updating an
/// already-managed entry via [`update`]).
pub fn write(apps_dir: &Path, entry: &NewEntry) -> anyhow::Result<PathBuf> {
    fs::create_dir_all(apps_dir)?;
    let path = apps_dir.join(format!("pacpad-{}.desktop", slugify(&entry.name)));
    if path.exists() {
        anyhow::bail!("{} already exists", path.display());
    }
    fs::write(&path, render(entry, &crate::time::now_iso8601()))?;
    make_executable(&path);
    refresh_desktop_database(apps_dir);
    Ok(path)
}

/// Overwrites an already-managed entry in place (same path, same
/// `X-PacPad-Created`) -- used by the Apps tab's edit flow. Refuses if
/// `path` isn't currently managed, for the same reason [`remove`]
/// does: never touch a `.desktop` file this tool didn't create.
pub fn update(path: &Path, entry: &NewEntry) -> anyhow::Result<()> {
    let existing =
        fs::read_to_string(path).map_err(|e| anyhow::anyhow!("reading {}: {e}", path.display()))?;
    let managed = parse_managed_entry(path, &existing).ok_or_else(|| {
        anyhow::anyhow!(
            "refusing to modify {}: not a pacpad-managed entry",
            path.display()
        )
    })?;
    let created = managed.created.unwrap_or_else(crate::time::now_iso8601);
    fs::write(path, render(entry, &created))?;
    if let Some(dir) = path.parent() {
        refresh_desktop_database(dir);
    }
    Ok(())
}

/// Deletes a managed `.desktop` file and, if its icon lives under
/// pacpad's own icon directory, the icon file too.
///
/// **The invariant**: re-reads `path` from disk right here, right
/// before deleting, and bails out if it no longer carries
/// `X-PacPad-Managed=true` -- covers a file that was never pacpad's
/// (the caller passed a bad path) as well as one a user hand-edited
/// to strip the marker between listing and this call.
pub fn remove(path: &Path) -> anyhow::Result<()> {
    let text =
        fs::read_to_string(path).map_err(|e| anyhow::anyhow!("reading {}: {e}", path.display()))?;
    let managed = parse_managed_entry(path, &text).ok_or_else(|| {
        anyhow::anyhow!(
            "refusing to remove {}: not a pacpad-managed entry",
            path.display()
        )
    })?;

    fs::remove_file(path).map_err(|e| anyhow::anyhow!("removing {}: {e}", path.display()))?;

    if let Some(icon) = &managed.icon {
        if icon.starts_with(crate::xdg::icons_dir()) {
            let _ = fs::remove_file(icon);
        }
    }

    if let Some(dir) = path.parent() {
        refresh_desktop_database(dir);
    }
    Ok(())
}

fn render(entry: &NewEntry, created: &str) -> String {
    let mut out = String::new();
    out.push_str("[Desktop Entry]\n");
    out.push_str("Version=1.0\n");
    out.push_str("Type=Application\n");
    out.push_str(&format!("Name={}\n", entry.name));
    out.push_str(&format!("Comment={}\n", entry.name));
    out.push_str(&format!("Exec={}\n", entry.exec));
    if let Some(icon) = &entry.icon {
        out.push_str(&format!("Icon={}\n", icon.display()));
    }
    out.push_str("Terminal=false\n");
    out.push_str("StartupNotify=true\n");
    if let Some(class) = &entry.window_class {
        out.push_str(&format!("StartupWMClass={class}\n"));
    }
    out.push_str(&format!("{MANAGED_KEY}=true\n"));
    out.push_str(&format!("{KIND_KEY}={}\n", entry.kind.as_str()));
    if let Some(url) = &entry.url {
        out.push_str(&format!("{URL_KEY}={url}\n"));
    }
    if let Some(command) = &entry.command {
        out.push_str(&format!("{COMMAND_KEY}={command}\n"));
    }
    if let Some(icon) = &entry.icon {
        out.push_str(&format!("{ICON_KEY}={}\n", icon.display()));
    }
    out.push_str(&format!("{CREATED_KEY}={created}\n"));
    out
}

fn make_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(path) {
            let mut perms = meta.permissions();
            perms.set_mode(perms.mode() | 0o111);
            let _ = fs::set_permissions(path, perms);
        }
    }
}

fn refresh_desktop_database(apps_dir: &Path) {
    if crate::which::which("update-desktop-database").is_some() {
        let _ = std::process::Command::new("update-desktop-database")
            .arg(apps_dir)
            .status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(label: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("pacpad-entry-test-{label}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample(name: &str) -> NewEntry {
        NewEntry {
            name: name.to_string(),
            kind: Kind::Webapp,
            exec: "google-chrome-stable --app=https://example.com".to_string(),
            icon: None,
            url: Some("https://example.com".to_string()),
            command: None,
            window_class: Some(format!("pacpad-{}", slugify(name))),
        }
    }

    #[test]
    fn slugify_handles_spaces_case_and_punctuation() {
        assert_eq!(slugify("Excalidraw"), "excalidraw");
        assert_eq!(slugify("My Cool App!"), "my-cool-app");
        assert_eq!(slugify("  --leading--  "), "leading");
        assert_eq!(slugify(""), "app");
        assert_eq!(slugify("!!!"), "app");
    }

    #[test]
    fn write_then_list_round_trips() {
        let dir = tmp_dir("roundtrip");
        let path = write(&dir, &sample("Excalidraw")).unwrap();
        assert!(path.ends_with("pacpad-excalidraw.desktop"));

        let managed = list_managed(&dir);
        assert_eq!(managed.len(), 1);
        assert_eq!(managed[0].name, "Excalidraw");
        assert_eq!(managed[0].kind, Kind::Webapp);
        assert_eq!(managed[0].url.as_deref(), Some("https://example.com"));
        assert!(managed[0].created.is_some());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_refuses_to_clobber_an_existing_file() {
        let dir = tmp_dir("clobber");
        write(&dir, &sample("Excalidraw")).unwrap();
        let result = write(&dir, &sample("Excalidraw"));
        assert!(result.is_err());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unmanaged_desktop_file_is_invisible_to_list_managed() {
        let dir = tmp_dir("invisible");
        fs::write(
            dir.join("firefox.desktop"),
            "[Desktop Entry]\nType=Application\nName=Firefox\nExec=firefox\n",
        )
        .unwrap();
        write(&dir, &sample("Excalidraw")).unwrap();

        let managed = list_managed(&dir);
        assert_eq!(managed.len(), 1);
        assert_eq!(managed[0].name, "Excalidraw");

        fs::remove_dir_all(&dir).ok();
    }

    /// The core safety invariant: an unmanaged `.desktop` file must
    /// survive every removal path pacpad has, unconditionally.
    #[test]
    fn remove_refuses_an_unmanaged_desktop_file() {
        let dir = tmp_dir("guard-unmanaged");
        let path = dir.join("Alacritty.desktop");
        fs::write(
            &path,
            "[Desktop Entry]\nType=Application\nName=Alacritty\nExec=alacritty\n",
        )
        .unwrap();

        let result = remove(&path);
        assert!(result.is_err());
        assert!(path.exists(), "unmanaged file must not be deleted");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn remove_refuses_a_desktop_file_with_marker_stripped_after_the_fact() {
        // Simulates a user hand-editing a once-managed file to remove
        // the marker between when the Apps tab listed it and when
        // they pressed 'd' -- the invariant re-reads from disk, so it
        // must still refuse.
        let dir = tmp_dir("guard-stripped");
        let path = write(&dir, &sample("Excalidraw")).unwrap();
        let stripped = fs::read_to_string(&path)
            .unwrap()
            .replace("X-PacPad-Managed=true\n", "");
        fs::write(&path, stripped).unwrap();

        let result = remove(&path);
        assert!(result.is_err());
        assert!(path.exists());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn remove_refuses_a_nonexistent_path_rather_than_treating_it_as_success() {
        let dir = tmp_dir("guard-missing");
        let path = dir.join("pacpad-ghost.desktop");
        assert!(!path.exists());
        assert!(remove(&path).is_err());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn remove_deletes_a_genuinely_managed_entry_and_its_icon_under_pacpad_dir() {
        let dir = tmp_dir("remove-ok");
        let icon_dir = dir.join("icons");
        fs::create_dir_all(&icon_dir).unwrap();
        let icon_path = icon_dir.join("excalidraw.png");
        fs::write(&icon_path, b"fake png bytes").unwrap();

        let mut entry = sample("Excalidraw");
        entry.icon = Some(icon_path.clone());
        let path = write(&dir, &entry).unwrap();

        // `remove` only deletes an icon that lives under the REAL
        // `xdg::icons_dir()`, not an arbitrary temp path -- so this
        // test only proves the .desktop file itself is removed for a
        // genuinely-managed entry; the icon-scoping rule is covered by
        // `remove_never_deletes_an_icon_outside_pacpads_own_dir`.
        assert!(remove(&path).is_ok());
        assert!(!path.exists());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn remove_never_deletes_an_icon_outside_pacpads_own_dir() {
        // A temp-dir icon path is guaranteed not to fall under the
        // real `xdg::icons_dir()` -- proving `remove` only ever
        // deletes an icon it's certain it owns.
        let dir = tmp_dir("icon-scope");
        let icon_path = dir.join("not-pacpads.png");
        fs::write(&icon_path, b"someone else's icon").unwrap();

        let mut entry = sample("Excalidraw");
        entry.icon = Some(icon_path.clone());
        let path = write(&dir, &entry).unwrap();

        assert!(remove(&path).is_ok());
        assert!(
            icon_path.exists(),
            "icon outside pacpad's own dir must survive"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn update_refuses_an_unmanaged_file_and_preserves_it() {
        let dir = tmp_dir("update-guard");
        let path = dir.join("kitty.desktop");
        fs::write(
            &path,
            "[Desktop Entry]\nType=Application\nName=Kitty\nExec=kitty\n",
        )
        .unwrap();

        let result = update(&path, &sample("Kitty"));
        assert!(result.is_err());
        let untouched = fs::read_to_string(&path).unwrap();
        assert!(
            untouched.contains("Exec=kitty"),
            "must not have been overwritten"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn update_preserves_the_original_created_timestamp() {
        let dir = tmp_dir("update-preserve-created");
        let path = write(&dir, &sample("Excalidraw")).unwrap();
        let original_created = list_managed(&dir)[0].created.clone();

        std::thread::sleep(std::time::Duration::from_millis(1100)); // ensure a different second
        let mut edited = sample("Excalidraw");
        edited.url = Some("https://excalidraw.com/changed".to_string());
        update(&path, &edited).unwrap();

        let after = list_managed(&dir);
        assert_eq!(after.len(), 1);
        assert_eq!(
            after[0].url.as_deref(),
            Some("https://excalidraw.com/changed")
        );
        assert_eq!(
            after[0].created, original_created,
            "Created timestamp must not change on edit"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parse_ignores_other_desktop_action_sections() {
        let text = "[Desktop Entry]\nType=Application\nName=Foo\nExec=foo\nX-PacPad-Managed=true\nX-PacPad-Kind=tui\n\n[Desktop Action new-window]\nExec=foo --new-window\nName=New Window\n";
        let fields = parse_desktop_entry(text);
        // The action section's own Exec/Name must not clobber the
        // main section's -- BTreeMap insertion order means a later
        // section WOULD win if section-scoping weren't enforced.
        assert_eq!(fields.get("Exec").map(String::as_str), Some("foo"));
        assert_eq!(fields.get("Name").map(String::as_str), Some("Foo"));
    }
}
