//! Pure application state: no I/O, no ratatui, no crossterm. Every
//! field here is data a test can construct and assert against without
//! a terminal -- matching herdr's own rule that `AppState` is testable
//! without a real runtime underneath it.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use crate::alpm::index::{Counts, PackageEntry, PackageIndex};
use crate::alpm::search;
use crate::launcher::entry::ManagedEntry;
use crate::launcher::terminal::WindowStyle;
use crate::launcher::{tuiapp::NewTuiApp, webapp::NewWebapp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Installed,
    Search,
    Apps,
    Updates,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Installed, Tab::Search, Tab::Apps, Tab::Updates];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Installed => "1 Installed",
            Tab::Search => "2 Search",
            Tab::Apps => "3 Apps",
            Tab::Updates => "4 Updates",
        }
    }

    pub fn next(self) -> Tab {
        let i = Self::ALL.iter().position(|t| *t == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Tab {
        let i = Self::ALL.iter().position(|t| *t == self).unwrap_or(0);
        Self::ALL[(i + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    pub fn from_digit(d: char) -> Option<Tab> {
        match d {
            '1' => Some(Tab::Installed),
            '2' => Some(Tab::Search),
            '3' => Some(Tab::Apps),
            '4' => Some(Tab::Updates),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstalledFilter {
    All,
    Explicit,
    Dependency,
    Orphan,
    Foreign,
}

impl InstalledFilter {
    const ALL: [InstalledFilter; 5] = [
        InstalledFilter::All,
        InstalledFilter::Explicit,
        InstalledFilter::Dependency,
        InstalledFilter::Orphan,
        InstalledFilter::Foreign,
    ];

    pub fn label(self) -> &'static str {
        match self {
            InstalledFilter::All => "All",
            InstalledFilter::Explicit => "Explicit",
            InstalledFilter::Dependency => "Dependencies",
            InstalledFilter::Orphan => "Orphans",
            InstalledFilter::Foreign => "Foreign (AUR)",
        }
    }

    pub fn next(self) -> InstalledFilter {
        let i = Self::ALL.iter().position(|f| *f == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> InstalledFilter {
        let i = Self::ALL.iter().position(|f| *f == self).unwrap_or(0);
        Self::ALL[(i + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    fn matches(self, entry: &PackageEntry, required: &HashSet<String>) -> bool {
        match self {
            InstalledFilter::All => entry.is_installed(),
            InstalledFilter::Explicit => {
                matches!(entry.reason(), Some(crate::alpm::InstallReason::Explicit))
            }
            InstalledFilter::Dependency => {
                matches!(entry.reason(), Some(crate::alpm::InstallReason::Dependency))
            }
            InstalledFilter::Orphan => entry.is_orphan(required),
            InstalledFilter::Foreign => entry.is_foreign(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortKey {
    Name,
    Size,
    InstallDate,
}

impl SortKey {
    pub fn label(self) -> &'static str {
        match self {
            SortKey::Name => "Name",
            SortKey::Size => "Size",
            SortKey::InstallDate => "Install date",
        }
    }
}

pub struct InstalledState {
    pub filter: InstalledFilter,
    pub sort: SortKey,
    pub sort_desc: bool,
    pub selected: usize,
    /// Indices into `AppState::entries` matching the current filter,
    /// in the current sort order. Recomputed whenever the filter,
    /// sort, or underlying entries change -- never mutated in place.
    pub visible: Vec<usize>,
    pub checked: HashSet<String>,
}

pub struct SearchState {
    pub query: String,
    /// (entry index, matched-on-description) pairs, in ranked order.
    pub results: Vec<(usize, bool)>,
    pub selected: usize,
    pub checked: HashSet<String>,
}

/// A confirm modal awaiting the user's Enter/Esc -- built by
/// `app::actions` (which calls into `txn::plan`/`txn::guard`, both
/// pure) whenever an action key is pressed. Nothing here does I/O;
/// the actual command only runs once the main loop drains
/// `AppState::execute` after a confirm.
pub struct Modal {
    pub action: crate::txn::Action,
    pub command: crate::txn::PlannedCommand,
    /// (package name, reason) for every target that `txn::guard`
    /// flags -- empty means an ordinary keypress confirms; non-empty
    /// means `typed` must equal [`crate::txn::CONFIRMATION_PHRASE`]
    /// before Enter does anything.
    pub guard_reasons: Vec<(String, &'static str)>,
    pub typed: String,
}

impl Modal {
    pub fn is_guarded(&self) -> bool {
        !self.guard_reasons.is_empty()
    }

    pub fn can_confirm(&self) -> bool {
        !self.is_guarded() || self.typed == crate::txn::CONFIRMATION_PHRASE
    }
}

pub struct AppState {
    pub entries: Vec<PackageEntry>,
    pub required_names: HashSet<String>,
    pub host_label: String,
    /// Detected once at startup (a plain filesystem PATH scan done by
    /// the caller before `AppState::new` -- kept out of this
    /// constructor so state-building itself stays I/O-free). `None`
    /// means AUR-only packages can be searched but not installed.
    pub aur_helper: Option<String>,
    pub tab: Tab,
    pub installed: InstalledState,
    pub search: SearchState,
    pub modal: Option<Modal>,
    pub apps: AppsState,
    /// Set by `Action::ModalConfirm`; drained by the main loop, which
    /// is the only place allowed to actually spawn a process.
    pub execute: Option<crate::txn::PlannedCommand>,
    /// Set by an Apps-tab create/update/remove/launch action; drained
    /// by the main loop the same way `execute` is, since writing a
    /// `.desktop` file, fetching an icon, or spawning a launched app
    /// are all I/O this module never performs itself.
    pub apps_action: Option<AppsAction>,
    /// The result of the most recently completed handover, shown
    /// briefly in the footer area -- purely informational.
    pub last_result: Option<String>,
    /// `tick` at the moment `last_result` was last set -- lets the UI
    /// fade the banner out over a few seconds instead of leaving a
    /// stale "done" message on screen forever.
    pub last_result_tick: Option<u64>,
    pub should_quit: bool,

    /// Advanced by one every main-loop iteration (see
    /// `advance_tick`) -- the single clock every animation in `ui::`
    /// reads from. Not itself a user action, which is why it's
    /// mutated directly by the main loop rather than through
    /// `Action`/`apply`, the same way `apps.reload` already is.
    pub tick: u64,
    /// While `tick < splash_until_tick`, `ui::render` shows the
    /// startup splash instead of the normal frame. Any keypress ends
    /// it immediately via `Action::DismissSplash` -- it's a treat, not
    /// a gate, so it never costs a impatient user real time.
    ///
    /// Defaults to `0` (no splash) here deliberately: `AppState::new`
    /// is also what every test in this module (and `input.rs`,
    /// `actions.rs`) builds its fixture state from, and none of them
    /// should have their first simulated keypress silently swallowed
    /// as a splash-dismiss. The real interactive run loop opts in
    /// explicitly, right after construction, in `cli::run_tui`.
    pub splash_until_tick: u64,
    /// Toggled by `Action::ToggleHelp` (bound to F1, chosen precisely
    /// because it can never collide with a Search-tab query
    /// character). Owns input the same way an open modal does.
    pub help_open: bool,

    /// A live snapshot of the embedded pty pane (see
    /// `txn::embed::PtySession`), refreshed by the main loop every
    /// iteration while a package transaction is running -- `None`
    /// otherwise. Deliberately plain data with no ratatui or `vt100`
    /// types leaking in (`ui::running` does that conversion), matching
    /// how every other piece of this struct stays UI-agnostic. While
    /// this is `Some`, the main loop forwards raw keystrokes into the
    /// pty instead of calling `app::input::map_key` at all -- there is
    /// no `Action` for "a key happened while a command is running",
    /// because the command, not pacpad, is what should interpret it.
    pub running: Option<crate::txn::RunningView>,
}

/// How long the startup splash runs before yielding to the real UI on
/// its own, in ticks -- see `ui::splash`.
pub const SPLASH_TICKS: u64 = 20;

impl AppState {
    pub fn new(index: PackageIndex, aur_helper: Option<String>) -> Self {
        let counts = index.counts();
        let required_names = index.required_names();
        let host_label = build_host_label(&counts);
        let entries = index.entries;

        let mut state = AppState {
            entries,
            required_names,
            host_label,
            aur_helper,
            tab: Tab::Installed,
            installed: InstalledState {
                filter: InstalledFilter::All,
                sort: SortKey::Name,
                sort_desc: false,
                selected: 0,
                visible: Vec::new(),
                checked: HashSet::new(),
            },
            search: SearchState {
                query: String::new(),
                results: Vec::new(),
                selected: 0,
                checked: HashSet::new(),
            },
            modal: None,
            apps: AppsState::default(),
            execute: None,
            apps_action: None,
            last_result: None,
            last_result_tick: None,
            should_quit: false,
            tick: 0,
            splash_until_tick: 0,
            help_open: false,
            running: None,
        };
        state.recompute_installed();
        state.recompute_search();
        state
    }

    /// Called once per main-loop iteration, before input is read --
    /// the one piece of state mutation the main loop does directly
    /// rather than through `Action`/`apply`, matching how it already
    /// calls `apps.reload` directly.
    pub fn advance_tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }

    /// Whether the startup splash should still be shown instead of the
    /// normal frame.
    pub fn splash_active(&self) -> bool {
        self.tick < self.splash_until_tick
    }

    /// Records a just-completed handover's result, timestamped so
    /// `ui::` can fade it out later instead of leaving it on screen
    /// forever.
    pub fn set_last_result(&mut self, message: String) {
        self.last_result = Some(message);
        self.last_result_tick = Some(self.tick);
    }

    /// Replaces the package data in place after a completed
    /// transaction, re-deriving everything computed from it, while
    /// preserving the rest of the UI's state (tab, filter, sort,
    /// search query, ...) -- called by the main loop right after a
    /// handover returns, so the Installed/Search views reflect what
    /// actually changed without restarting the app.
    pub fn reload(&mut self, index: PackageIndex) {
        let counts = index.counts();
        self.required_names = index.required_names();
        self.host_label = build_host_label(&counts);
        self.entries = index.entries;
        self.installed.checked.clear();
        self.search.checked.clear();
        self.recompute_installed();
        self.recompute_search();
    }

    /// The package names an action should target: the multi-selected
    /// set for the active tab if non-empty, otherwise just whatever
    /// row is currently under the cursor.
    pub fn action_targets(&self) -> Vec<String> {
        let (checked, current) = match self.tab {
            Tab::Installed => (&self.installed.checked, self.current_entry()),
            Tab::Search => (&self.search.checked, self.current_entry()),
            Tab::Apps | Tab::Updates => return Vec::new(),
        };
        if !checked.is_empty() {
            checked.iter().cloned().collect()
        } else {
            current.map(|e| vec![e.name.clone()]).unwrap_or_default()
        }
    }

    /// Resolves target names back to their full entries -- needed by
    /// the guard check (package groups) and the AUR-helper-needed
    /// check (repo presence), neither of which a bare name string
    /// carries.
    pub fn resolve<'a>(&'a self, names: &[String]) -> Vec<&'a PackageEntry> {
        names
            .iter()
            .filter_map(|n| self.entries.iter().find(|e| &e.name == n))
            .collect()
    }

    pub fn recompute_installed(&mut self) {
        let filter = self.installed.filter;
        let required = &self.required_names;
        let mut visible: Vec<usize> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| filter.matches(e, required))
            .map(|(i, _)| i)
            .collect();

        let entries = &self.entries;
        visible.sort_by(|&a, &b| {
            let (ea, eb) = (&entries[a], &entries[b]);
            let ord = match self.installed.sort {
                SortKey::Name => ea.name.cmp(&eb.name),
                SortKey::Size => installed_size(ea).cmp(&installed_size(eb)),
                SortKey::InstallDate => install_date(ea).cmp(&install_date(eb)),
            };
            if self.installed.sort_desc {
                ord.reverse()
            } else {
                ord
            }
        });

        self.installed.visible = visible;
        if self.installed.selected >= self.installed.visible.len() {
            self.installed.selected = self.installed.visible.len().saturating_sub(1);
        }
    }

    pub fn recompute_search(&mut self) {
        let hits = search::search(&self.entries, &self.search.query);
        self.search.results = hits
            .iter()
            .map(|h| (h.index, h.matched_description))
            .collect();
        if self.search.selected >= self.search.results.len() {
            self.search.selected = self.search.results.len().saturating_sub(1);
        }
    }

    /// The entry currently under the cursor in the active tab's list,
    /// if any.
    pub fn current_entry(&self) -> Option<&PackageEntry> {
        match self.tab {
            Tab::Installed => self
                .installed
                .visible
                .get(self.installed.selected)
                .map(|&i| &self.entries[i]),
            Tab::Search => self
                .search
                .results
                .get(self.search.selected)
                .map(|&(i, _)| &self.entries[i]),
            _ => None,
        }
    }
}

fn installed_size(e: &PackageEntry) -> u64 {
    e.local.as_ref().map(|l| l.installed_size).unwrap_or(0)
}

fn install_date(e: &PackageEntry) -> i64 {
    e.local.as_ref().and_then(|l| l.install_date).unwrap_or(0)
}

fn build_host_label(counts: &Counts) -> String {
    format!(
        "{} · {} pkgs · {} explicit · {} foreign",
        distro_id(),
        counts.total,
        counts.explicit,
        counts.foreign
    )
}

/// Reads `ID=` out of `/etc/os-release` for the little "cachyos ·"
/// touch in the top bar. Falls back to a generic label rather than
/// failing -- this is decorative, never load-bearing.
fn distro_id() -> String {
    fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|text| {
            text.lines().find_map(|line| {
                line.strip_prefix("ID=")
                    .map(|v| v.trim_matches('"').to_string())
            })
        })
        .unwrap_or_else(|| "linux".to_string())
}

// ---------------------------------------------------------------- Apps tab

/// A single-line, cursor-aware text buffer for form fields. Cursor
/// position is a *character* index, not a byte index -- editing at an
/// arbitrary point in a UTF-8 string (an app name with an accented
/// letter, say) must never split a multi-byte character.
#[derive(Debug, Clone, Default)]
pub struct TextField {
    pub value: String,
    pub cursor: usize,
}

impl TextField {
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        let cursor = value.chars().count();
        TextField { value, cursor }
    }

    fn byte_index(&self) -> usize {
        self.value
            .char_indices()
            .nth(self.cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.value.len())
    }

    pub fn insert(&mut self, c: char) {
        let at = self.byte_index();
        self.value.insert(at, c);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let end = self.byte_index();
        let start = self.value[..end]
            .char_indices()
            .last()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.value.replace_range(start..end, "");
        self.cursor -= 1;
    }

    pub fn delete(&mut self) {
        let start = self.byte_index();
        if start >= self.value.len() {
            return;
        }
        let end = self.value[start..]
            .char_indices()
            .nth(1)
            .map(|(i, _)| start + i)
            .unwrap_or(self.value.len());
        self.value.replace_range(start..end, "");
    }

    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.value.chars().count());
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.value.chars().count();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebappField {
    Name,
    Url,
    Icon,
}

impl WebappField {
    pub fn next(self) -> Self {
        match self {
            WebappField::Name => WebappField::Url,
            WebappField::Url => WebappField::Icon,
            WebappField::Icon => WebappField::Name,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            WebappField::Name => WebappField::Icon,
            WebappField::Url => WebappField::Name,
            WebappField::Icon => WebappField::Url,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WebappForm {
    pub name: TextField,
    pub url: TextField,
    pub icon: TextField,
    pub focus: WebappField,
    /// `Some(path)` when this form is editing an existing entry
    /// in-place rather than creating a new one.
    pub editing: Option<PathBuf>,
    pub error: Option<String>,
}

impl WebappForm {
    pub fn empty() -> Self {
        WebappForm {
            name: TextField::default(),
            url: TextField::default(),
            icon: TextField::default(),
            focus: WebappField::Name,
            editing: None,
            error: None,
        }
    }

    pub fn from_entry(entry: &ManagedEntry) -> Self {
        WebappForm {
            name: TextField::new(entry.name.clone()),
            url: TextField::new(entry.url.clone().unwrap_or_default()),
            icon: TextField::new(
                entry
                    .icon
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
            ),
            focus: WebappField::Name,
            editing: Some(entry.path.clone()),
            error: None,
        }
    }

    pub fn to_new_webapp(&self) -> Result<NewWebapp, &'static str> {
        let name = self.name.value.trim();
        let url = self.url.value.trim();
        if name.is_empty() || url.is_empty() {
            return Err("Name and URL are required");
        }
        let icon = self.icon.value.trim();
        Ok(NewWebapp {
            name: name.to_string(),
            url: url.to_string(),
            icon: if icon.is_empty() {
                None
            } else {
                Some(icon.to_string())
            },
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiField {
    Name,
    Command,
    Style,
    Icon,
}

impl TuiField {
    pub fn next(self) -> Self {
        match self {
            TuiField::Name => TuiField::Command,
            TuiField::Command => TuiField::Style,
            TuiField::Style => TuiField::Icon,
            TuiField::Icon => TuiField::Name,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            TuiField::Name => TuiField::Icon,
            TuiField::Command => TuiField::Name,
            TuiField::Style => TuiField::Command,
            TuiField::Icon => TuiField::Style,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TuiForm {
    pub name: TextField,
    pub command: TextField,
    pub style: WindowStyle,
    pub icon: TextField,
    pub focus: TuiField,
    pub editing: Option<PathBuf>,
    pub error: Option<String>,
}

impl TuiForm {
    pub fn empty() -> Self {
        TuiForm {
            name: TextField::default(),
            command: TextField::default(),
            style: WindowStyle::Float,
            icon: TextField::default(),
            focus: TuiField::Name,
            editing: None,
            error: None,
        }
    }

    pub fn from_entry(entry: &ManagedEntry) -> Self {
        let style = if entry.window_class.as_deref() == Some(WindowStyle::Tile.class()) {
            WindowStyle::Tile
        } else {
            WindowStyle::Float
        };
        TuiForm {
            name: TextField::new(entry.name.clone()),
            command: TextField::new(entry.command.clone().unwrap_or_default()),
            style,
            icon: TextField::new(
                entry
                    .icon
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
            ),
            focus: TuiField::Name,
            editing: Some(entry.path.clone()),
            error: None,
        }
    }

    pub fn to_new_tuiapp(&self) -> Result<NewTuiApp, &'static str> {
        let name = self.name.value.trim();
        let command = self.command.value.trim();
        if name.is_empty() || command.is_empty() {
            return Err("Name and Command are required");
        }
        let icon = self.icon.value.trim();
        Ok(NewTuiApp {
            name: name.to_string(),
            command: command.to_string(),
            style: self.style,
            icon: if icon.is_empty() {
                None
            } else {
                Some(icon.to_string())
            },
        })
    }
}

#[derive(Debug, Clone)]
pub enum AppForm {
    Webapp(WebappForm),
    Tui(TuiForm),
}

/// What an Apps-tab action resolved to; drained and executed by the
/// main loop (`cli::run_tui`), the only place allowed to write a
/// `.desktop` file, fetch an icon, or spawn a launched app.
pub enum AppsAction {
    CreateWebapp(NewWebapp),
    UpdateWebapp(PathBuf, NewWebapp),
    CreateTui(NewTuiApp),
    UpdateTui(PathBuf, NewTuiApp),
    RemoveWebapp(PathBuf),
    RemoveTui(PathBuf),
    /// Launching is never gated by `--dry-run` -- opening a window
    /// changes nothing pacpad is responsible for undoing.
    Launch(String),
}

#[derive(Debug, Clone, Default)]
pub struct AppsState {
    pub webapps: Vec<ManagedEntry>,
    pub tuiapps: Vec<ManagedEntry>,
    pub selected: usize,
    pub form: Option<AppForm>,
}

impl AppsState {
    pub fn len(&self) -> usize {
        self.webapps.len() + self.tuiapps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn selected_entry(&self) -> Option<&ManagedEntry> {
        if self.selected < self.webapps.len() {
            self.webapps.get(self.selected)
        } else {
            self.tuiapps.get(self.selected - self.webapps.len())
        }
    }

    /// Re-reads both launcher lists from disk and clamps `selected`
    /// to still be in range -- called by the main loop after startup
    /// and after every completed Apps-tab action.
    pub fn reload(&mut self, apps_dir: &std::path::Path) {
        self.webapps = crate::launcher::webapp::list(apps_dir);
        self.tuiapps = crate::launcher::tuiapp::list(apps_dir);
        let len = self.len();
        if self.selected >= len {
            self.selected = len.saturating_sub(1);
        }
    }
}

#[cfg(test)]
mod apps_state_tests {
    use super::*;
    use crate::launcher::entry::Kind as LauncherKind;

    fn entry(name: &str, kind: LauncherKind) -> ManagedEntry {
        ManagedEntry {
            path: PathBuf::from(format!("/tmp/pacpad-{name}.desktop")),
            name: name.to_string(),
            kind,
            exec: "true".to_string(),
            icon: None,
            url: None,
            command: None,
            window_class: None,
            created: None,
        }
    }

    #[test]
    fn text_field_insert_backspace_delete_move_cursor_correctly() {
        let mut f = TextField::default();
        f.insert('a');
        f.insert('b');
        f.insert('c');
        assert_eq!(f.value, "abc");
        assert_eq!(f.cursor, 3);

        f.left();
        f.left();
        assert_eq!(f.cursor, 1);
        f.insert('X');
        assert_eq!(f.value, "aXbc");
        assert_eq!(f.cursor, 2);

        f.backspace();
        assert_eq!(f.value, "abc");
        assert_eq!(f.cursor, 1);

        f.delete();
        assert_eq!(f.value, "ac");
        assert_eq!(f.cursor, 1);

        f.home();
        assert_eq!(f.cursor, 0);
        f.backspace(); // no-op at start
        assert_eq!(f.value, "ac");

        f.end();
        assert_eq!(f.cursor, 2);
        f.right(); // no-op at end
        assert_eq!(f.cursor, 2);
    }

    #[test]
    fn text_field_editing_is_utf8_safe_at_a_multibyte_boundary() {
        let mut f = TextField::new("café");
        assert_eq!(f.cursor, 4); // 4 chars, not 5 bytes
        f.backspace();
        assert_eq!(f.value, "caf");
        f.insert('é');
        assert_eq!(f.value, "café");
    }

    #[test]
    fn webapp_field_cycles() {
        assert_eq!(WebappField::Name.next(), WebappField::Url);
        assert_eq!(WebappField::Icon.next(), WebappField::Name);
        assert_eq!(WebappField::Name.prev(), WebappField::Icon);
    }

    #[test]
    fn tui_field_cycles() {
        assert_eq!(TuiField::Name.next(), TuiField::Command);
        assert_eq!(TuiField::Icon.next(), TuiField::Name);
        assert_eq!(TuiField::Name.prev(), TuiField::Icon);
    }

    #[test]
    fn webapp_form_validates_required_fields() {
        let mut form = WebappForm::empty();
        assert!(form.to_new_webapp().is_err());
        form.name = TextField::new("Excalidraw");
        assert!(form.to_new_webapp().is_err(), "url still missing");
        form.url = TextField::new("https://excalidraw.com");
        let new = form.to_new_webapp().unwrap();
        assert_eq!(new.name, "Excalidraw");
        assert_eq!(new.url, "https://excalidraw.com");
        assert!(new.icon.is_none());
    }

    #[test]
    fn webapp_form_from_entry_prefills_for_editing() {
        let mut e = entry("Excalidraw", LauncherKind::Webapp);
        e.url = Some("https://excalidraw.com".to_string());
        let form = WebappForm::from_entry(&e);
        assert_eq!(form.name.value, "Excalidraw");
        assert_eq!(form.url.value, "https://excalidraw.com");
        assert_eq!(form.editing, Some(e.path));
    }

    #[test]
    fn tui_form_validates_required_fields_and_preserves_style() {
        let mut form = TuiForm::empty();
        form.style = WindowStyle::Tile;
        assert!(form.to_new_tuiapp().is_err());
        form.name = TextField::new("Btop");
        form.command = TextField::new("btop");
        let new = form.to_new_tuiapp().unwrap();
        assert_eq!(new.name, "Btop");
        assert_eq!(new.command, "btop");
        assert_eq!(new.style, WindowStyle::Tile);
    }

    #[test]
    fn tui_form_from_entry_recovers_style_from_window_class() {
        let mut e = entry("Btop", LauncherKind::Tui);
        e.command = Some("btop".to_string());
        e.window_class = Some(WindowStyle::Tile.class().to_string());
        let form = TuiForm::from_entry(&e);
        assert_eq!(form.command.value, "btop");
        assert_eq!(form.style, WindowStyle::Tile);
    }

    #[test]
    fn apps_state_selected_entry_spans_webapps_then_tuiapps() {
        let mut apps = AppsState {
            webapps: vec![
                entry("A", LauncherKind::Webapp),
                entry("B", LauncherKind::Webapp),
            ],
            tuiapps: vec![entry("C", LauncherKind::Tui)],
            selected: 0,
            form: None,
        };
        assert_eq!(apps.selected_entry().unwrap().name, "A");
        apps.selected = 2;
        assert_eq!(apps.selected_entry().unwrap().name, "C");
        apps.selected = 99;
        assert!(apps.selected_entry().is_none());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alpm::{local::read_local_db, sync::read_sync_db};
    use std::path::Path;

    fn index() -> PackageIndex {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let locals = read_local_db(&root.join("local")).unwrap();
        let syncs = read_sync_db(&root.join("sync/extra.db"), "extra").unwrap();
        PackageIndex::build(locals, syncs)
    }

    #[test]
    fn default_filter_shows_all_installed() {
        let state = AppState::new(index(), None);
        // ripgrep, eza, htop are installed; fd is sync-only.
        assert_eq!(state.installed.visible.len(), 3);
    }

    #[test]
    fn filter_to_foreign_shows_only_htop() {
        let mut state = AppState::new(index(), None);
        state.installed.filter = InstalledFilter::Foreign;
        state.recompute_installed();
        let names: Vec<&str> = state
            .installed
            .visible
            .iter()
            .map(|&i| state.entries[i].name.as_str())
            .collect();
        assert_eq!(names, vec!["htop"]);
    }

    #[test]
    fn filter_to_orphan_shows_only_htop() {
        let mut state = AppState::new(index(), None);
        state.installed.filter = InstalledFilter::Orphan;
        state.recompute_installed();
        let names: Vec<&str> = state
            .installed
            .visible
            .iter()
            .map(|&i| state.entries[i].name.as_str())
            .collect();
        assert_eq!(names, vec!["htop"]);
    }

    #[test]
    fn sort_by_name_ascending_by_default() {
        let state = AppState::new(index(), None);
        let names: Vec<&str> = state
            .installed
            .visible
            .iter()
            .map(|&i| state.entries[i].name.as_str())
            .collect();
        assert_eq!(names, vec!["eza", "htop", "ripgrep"]);
    }

    #[test]
    fn sort_by_size_descending() {
        let mut state = AppState::new(index(), None);
        state.installed.sort = SortKey::Size;
        state.installed.sort_desc = true;
        state.recompute_installed();
        let names: Vec<&str> = state
            .installed
            .visible
            .iter()
            .map(|&i| state.entries[i].name.as_str())
            .collect();
        // ripgrep 3742597 > eza 1918514 > htop 1258291
        assert_eq!(names, vec!["ripgrep", "eza", "htop"]);
    }

    #[test]
    fn selection_clamped_when_filter_shrinks_visible_list() {
        let mut state = AppState::new(index(), None);
        state.installed.selected = 2; // last of 3
        state.installed.filter = InstalledFilter::Foreign; // shrinks to 1
        state.recompute_installed();
        assert_eq!(state.installed.selected, 0);
        assert_eq!(state.installed.visible.len(), 1);
    }

    #[test]
    fn search_results_resolve_to_correct_entries() {
        let mut state = AppState::new(index(), None);
        state.search.query = "ripgrep".to_string();
        state.recompute_search();
        assert_eq!(state.search.results.len(), 1);
        let (idx, _) = state.search.results[0];
        assert_eq!(state.entries[idx].name, "ripgrep");
    }

    #[test]
    fn current_entry_follows_active_tab() {
        let mut state = AppState::new(index(), None);
        state.tab = Tab::Installed;
        let installed_pick = state.current_entry().unwrap().name.clone();
        assert_eq!(installed_pick, "eza"); // first alphabetically

        state.tab = Tab::Search;
        state.search.query = "fd".to_string();
        state.recompute_search();
        assert_eq!(state.current_entry().unwrap().name, "fd");
    }

    #[test]
    fn tab_cycling_wraps() {
        assert_eq!(Tab::Updates.next(), Tab::Installed);
        assert_eq!(Tab::Installed.prev(), Tab::Updates);
    }

    #[test]
    fn filter_cycling_wraps() {
        assert_eq!(InstalledFilter::Foreign.next(), InstalledFilter::All);
        assert_eq!(InstalledFilter::All.prev(), InstalledFilter::Foreign);
    }
}
