//! Pure state transitions. `apply` never performs I/O -- it builds a
//! [`crate::txn::PlannedCommand`] via `txn::plan`/`txn::guard` (both
//! pure) and stores it in `state.modal` or `state.execute`, but never
//! spawns anything itself. Only the main loop (`cli::run_tui`), which
//! drains `state.execute` after each `apply`, is allowed to actually
//! run a command -- that's the one place doing I/O in this whole
//! chain.

use super::state::{
    AppForm, AppState, AppsAction, Modal, SortKey, Tab, TextField, TuiForm, WebappForm,
};
use crate::launcher::entry::Kind as LauncherKind;
use crate::txn;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Quit,
    /// Ends the startup splash early -- bound to *any* key press while
    /// it's active (see `map_key`), since it's a treat, not a gate.
    DismissSplash,
    /// Opens/closes the `?`-equivalent keybinding cheat sheet (bound
    /// to F1; see `map_key` for why not `?` itself).
    ToggleHelp,
    NextTab,
    PrevTab,
    GoToTab(Tab),
    MoveUp,
    MoveDown,
    ToggleSelectCurrent,
    CycleFilterNext,
    CycleFilterPrev,
    CycleSort,
    SearchInsertChar(char),
    SearchBackspace,
    SearchClear,

    /// Opens a confirm modal for the current selection/target; never
    /// executes anything itself.
    RequestInstall,
    RequestRemove,
    RequestReinstall,
    RequestMarkExplicit,
    RequestMarkDependency,
    RequestUpdateSystem,
    /// These two skip the confirm modal (non-destructive) and go
    /// straight to `state.execute`.
    RequestListFiles,
    RequestOpenUrl,

    /// Only meaningful while `state.modal` is `Some`.
    ModalToggleRecursive,
    ModalTypeChar(char),
    ModalBackspace,
    ModalConfirm,
    ModalCancel,

    // -- Apps tab --------------------------------------------------
    RequestAddWebapp,
    RequestAddTui,
    /// Opens a form pre-filled from the currently selected entry.
    RequestEditApp,
    /// Removes the currently selected entry directly -- no confirm
    /// modal, unlike package removal: a launcher is a small, trivially
    /// recreated file with no `sudo`/system-state stakes, and the
    /// ownership-marker invariant (`launcher::entry::remove`) is
    /// already the real safety net against deleting the wrong thing.
    RequestRemoveApp,
    /// Spawns the selected entry's own `Exec=` line, detached --
    /// never gated by `--dry-run` (see `AppsAction::Launch`).
    LaunchApp,

    /// Only meaningful while `state.apps.form` is `Some`.
    FormFocusNext,
    FormFocusPrev,
    FormInsertChar(char),
    FormBackspace,
    FormDelete,
    FormLeft,
    FormRight,
    FormHome,
    FormEnd,
    FormSubmit,
    FormCancel,
}

pub fn apply(state: &mut AppState, action: Action) {
    match action {
        Action::Quit => state.should_quit = true,

        Action::DismissSplash => state.splash_until_tick = 0,
        Action::ToggleHelp => state.help_open = !state.help_open,

        Action::NextTab => state.tab = state.tab.next(),
        Action::PrevTab => state.tab = state.tab.prev(),
        Action::GoToTab(tab) => state.tab = tab,

        Action::MoveUp => move_selection(state, -1),
        Action::MoveDown => move_selection(state, 1),

        Action::ToggleSelectCurrent => toggle_select_current(state),

        Action::CycleFilterNext => {
            state.installed.filter = state.installed.filter.next();
            state.recompute_installed();
        }
        Action::CycleFilterPrev => {
            state.installed.filter = state.installed.filter.prev();
            state.recompute_installed();
        }

        Action::CycleSort => cycle_sort(state),

        Action::SearchInsertChar(c) => {
            state.search.query.push(c);
            state.recompute_search();
        }
        Action::SearchBackspace => {
            state.search.query.pop();
            state.recompute_search();
        }
        Action::SearchClear => {
            state.search.query.clear();
            state.recompute_search();
        }

        Action::RequestInstall => {
            open_modal(state, |targets| txn::Action::Install { packages: targets })
        }
        Action::RequestRemove => open_modal(state, |targets| txn::Action::Remove {
            packages: targets,
            recursive: true,
        }),
        Action::RequestReinstall => open_modal(state, |targets| txn::Action::Reinstall {
            packages: targets,
        }),
        Action::RequestMarkExplicit => open_modal(state, |targets| txn::Action::MarkExplicit {
            packages: targets,
        }),
        Action::RequestMarkDependency => open_modal(state, |targets| txn::Action::MarkDependency {
            packages: targets,
        }),
        Action::RequestUpdateSystem => open_modal(state, |_| txn::Action::UpdateSystem),

        Action::RequestListFiles => {
            if let Some(entry) = state.current_entry() {
                let action = txn::Action::ListFiles {
                    package: entry.name.clone(),
                };
                state.execute = Some(txn::plan_command(&action, None));
            }
        }
        Action::RequestOpenUrl => {
            if let Some(url) = state.current_entry().and_then(entry_url) {
                let action = txn::Action::OpenUrl { url };
                state.execute = Some(txn::plan_command(&action, None));
            }
        }

        Action::ModalToggleRecursive => toggle_recursive(state),
        Action::ModalTypeChar(c) => {
            if let Some(modal) = &mut state.modal {
                if modal.is_guarded() {
                    modal.typed.push(c);
                }
            }
        }
        Action::ModalBackspace => {
            if let Some(modal) = &mut state.modal {
                modal.typed.pop();
            }
        }
        Action::ModalConfirm => confirm_modal(state),
        Action::ModalCancel => state.modal = None,

        Action::RequestAddWebapp => state.apps.form = Some(AppForm::Webapp(WebappForm::empty())),
        Action::RequestAddTui => state.apps.form = Some(AppForm::Tui(TuiForm::empty())),
        Action::RequestEditApp => {
            if let Some(entry) = state.apps.selected_entry() {
                state.apps.form = Some(match entry.kind {
                    LauncherKind::Webapp => AppForm::Webapp(WebappForm::from_entry(entry)),
                    LauncherKind::Tui => AppForm::Tui(TuiForm::from_entry(entry)),
                });
            }
        }
        Action::RequestRemoveApp => {
            if let Some(entry) = state.apps.selected_entry() {
                state.apps_action = Some(match entry.kind {
                    LauncherKind::Webapp => AppsAction::RemoveWebapp(entry.path.clone()),
                    LauncherKind::Tui => AppsAction::RemoveTui(entry.path.clone()),
                });
            }
        }
        Action::LaunchApp => {
            if let Some(entry) = state.apps.selected_entry() {
                state.apps_action = Some(AppsAction::Launch(entry.exec.clone()));
            }
        }

        Action::FormFocusNext => form_move_focus(state, true),
        Action::FormFocusPrev => form_move_focus(state, false),
        Action::FormInsertChar(c) => {
            if let Some(field) = focused_field(state) {
                field.insert(c);
            }
        }
        Action::FormBackspace => {
            if let Some(field) = focused_field(state) {
                field.backspace();
            }
        }
        Action::FormDelete => {
            if let Some(field) = focused_field(state) {
                field.delete();
            }
        }
        Action::FormLeft => form_left_right(state, false),
        Action::FormRight => form_left_right(state, true),
        Action::FormHome => {
            if let Some(field) = focused_field(state) {
                field.home();
            }
        }
        Action::FormEnd => {
            if let Some(field) = focused_field(state) {
                field.end();
            }
        }
        Action::FormSubmit => form_submit(state),
        Action::FormCancel => state.apps.form = None,
    }
}

fn form_move_focus(state: &mut AppState, forward: bool) {
    let Some(form) = &mut state.apps.form else {
        return;
    };
    match form {
        AppForm::Webapp(f) => {
            f.focus = if forward {
                f.focus.next()
            } else {
                f.focus.prev()
            }
        }
        AppForm::Tui(f) => {
            f.focus = if forward {
                f.focus.next()
            } else {
                f.focus.prev()
            }
        }
    }
}

/// The `TextField` the form's current focus points at, or `None` when
/// focus is on a non-text control (the TUI form's window-style toggle)
/// -- every plain text-editing action routes through this one
/// accessor rather than re-matching focus in each action arm.
fn focused_field(state: &mut AppState) -> Option<&mut TextField> {
    match state.apps.form.as_mut()? {
        AppForm::Webapp(f) => Some(match f.focus {
            crate::app::state::WebappField::Name => &mut f.name,
            crate::app::state::WebappField::Url => &mut f.url,
            crate::app::state::WebappField::Icon => &mut f.icon,
        }),
        AppForm::Tui(f) => match f.focus {
            super::state::TuiField::Name => Some(&mut f.name),
            super::state::TuiField::Command => Some(&mut f.command),
            super::state::TuiField::Style => None,
            super::state::TuiField::Icon => Some(&mut f.icon),
        },
    }
}

/// Left/Right move the cursor within a focused text field, except on
/// the TUI form's window-style control, where they toggle Float/Tile
/// instead -- there's nothing else Left/Right could mean for a
/// two-state, non-textual field.
fn form_left_right(state: &mut AppState, right: bool) {
    if let Some(AppForm::Tui(f)) = &mut state.apps.form {
        if f.focus == super::state::TuiField::Style {
            f.style = match f.style {
                crate::launcher::terminal::WindowStyle::Float => {
                    crate::launcher::terminal::WindowStyle::Tile
                }
                crate::launcher::terminal::WindowStyle::Tile => {
                    crate::launcher::terminal::WindowStyle::Float
                }
            };
            return;
        }
    }
    if let Some(field) = focused_field(state) {
        if right {
            field.right();
        } else {
            field.left();
        }
    }
}

fn form_submit(state: &mut AppState) {
    let Some(form) = state.apps.form.take() else {
        return;
    };
    match form {
        AppForm::Webapp(f) => match f.to_new_webapp() {
            Ok(new) => {
                state.apps_action = Some(match &f.editing {
                    Some(path) => AppsAction::UpdateWebapp(path.clone(), new),
                    None => AppsAction::CreateWebapp(new),
                });
            }
            Err(msg) => {
                let mut f = f;
                f.error = Some(msg.to_string());
                state.apps.form = Some(AppForm::Webapp(f));
            }
        },
        AppForm::Tui(f) => match f.to_new_tuiapp() {
            Ok(new) => {
                state.apps_action = Some(match &f.editing {
                    Some(path) => AppsAction::UpdateTui(path.clone(), new),
                    None => AppsAction::CreateTui(new),
                });
            }
            Err(msg) => {
                let mut f = f;
                f.error = Some(msg.to_string());
                state.apps.form = Some(AppForm::Tui(f));
            }
        },
    }
}

fn entry_url(entry: &crate::alpm::index::PackageEntry) -> Option<String> {
    entry
        .local
        .as_ref()
        .and_then(|l| l.url.clone())
        .or_else(|| entry.sync.as_ref().and_then(|s| s.url.clone()))
}

/// Builds the target-dependent `txn::Action` (via `build`, given the
/// current selection), works out whether it needs an AUR helper and
/// whether any target is guard-protected, and opens the confirm modal
/// -- or does nothing if there's genuinely nothing to act on.
fn open_modal(state: &mut AppState, build: impl FnOnce(Vec<String>) -> txn::Action) {
    let targets = state.action_targets();
    let action = build(targets.clone());

    if action.packages().is_empty() && !matches!(action, txn::Action::UpdateSystem) {
        return;
    }

    let target_entries = state.resolve(&targets);

    let needs_aur = matches!(
        action,
        txn::Action::Install { .. } | txn::Action::Reinstall { .. }
    ) && target_entries.iter().any(|e| e.repo().is_none());
    let use_helper = match &action {
        txn::Action::UpdateSystem => state.aur_helper.as_deref(),
        _ if needs_aur => state.aur_helper.as_deref(),
        _ => None,
    };

    let command = txn::plan_command(&action, use_helper);
    let guard_reasons = if action.is_destructive_removal() {
        txn::protected_among(target_entries.iter().copied())
            .into_iter()
            .map(|(name, reason)| (name.to_string(), reason))
            .collect()
    } else {
        Vec::new()
    };

    state.modal = Some(Modal {
        action,
        command,
        guard_reasons,
        typed: String::new(),
    });
}

fn toggle_recursive(state: &mut AppState) {
    let Some(modal) = &mut state.modal else {
        return;
    };
    if let txn::Action::Remove { recursive, .. } = &mut modal.action {
        *recursive = !*recursive;
        modal.command = txn::plan_command(&modal.action, None);
    }
}

fn confirm_modal(state: &mut AppState) {
    let Some(modal) = &state.modal else { return };
    if modal.can_confirm() {
        state.execute = Some(modal.command.clone());
        state.modal = None;
    }
}

fn move_selection(state: &mut AppState, delta: i32) {
    match state.tab {
        Tab::Installed => {
            let len = state.installed.visible.len();
            state.installed.selected = clamp_move(state.installed.selected, delta, len);
        }
        Tab::Search => {
            let len = state.search.results.len();
            state.search.selected = clamp_move(state.search.selected, delta, len);
        }
        Tab::Apps => {
            let len = state.apps.len();
            state.apps.selected = clamp_move(state.apps.selected, delta, len);
        }
        Tab::Updates => {}
    }
}

fn clamp_move(current: usize, delta: i32, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let next = current as i32 + delta;
    next.clamp(0, len as i32 - 1) as usize
}

fn toggle_select_current(state: &mut AppState) {
    let Some(entry) = state.current_entry() else {
        return;
    };
    let name = entry.name.clone();
    let set = match state.tab {
        Tab::Installed => &mut state.installed.checked,
        Tab::Search => &mut state.search.checked,
        Tab::Apps | Tab::Updates => return,
    };
    if !set.remove(&name) {
        set.insert(name);
    }
}

/// Cycles through (Name asc, Name desc, Size asc, Size desc,
/// InstallDate asc, InstallDate desc) as one ordered sequence, so a
/// single key steps through every combination without needing a
/// second "toggle direction" binding.
fn cycle_sort(state: &mut AppState) {
    let (sort, desc) = match (state.installed.sort, state.installed.sort_desc) {
        (SortKey::Name, false) => (SortKey::Name, true),
        (SortKey::Name, true) => (SortKey::Size, false),
        (SortKey::Size, false) => (SortKey::Size, true),
        (SortKey::Size, true) => (SortKey::InstallDate, false),
        (SortKey::InstallDate, false) => (SortKey::InstallDate, true),
        (SortKey::InstallDate, true) => (SortKey::Name, false),
    };
    state.installed.sort = sort;
    state.installed.sort_desc = desc;
    state.recompute_installed();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alpm::index::PackageIndex;
    use crate::alpm::local::read_local_db;
    use crate::alpm::sync::read_sync_db;
    use crate::app::state::InstalledFilter;
    use std::path::Path;

    fn state() -> AppState {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let locals = read_local_db(&root.join("local")).unwrap();
        let syncs = read_sync_db(&root.join("sync/extra.db"), "extra").unwrap();
        AppState::new(PackageIndex::build(locals, syncs), None)
    }

    #[test]
    fn quit_sets_flag() {
        let mut s = state();
        apply(&mut s, Action::Quit);
        assert!(s.should_quit);
    }

    #[test]
    fn move_down_and_up_on_installed_tab() {
        let mut s = state(); // eza, htop, ripgrep
        assert_eq!(s.installed.selected, 0);
        apply(&mut s, Action::MoveDown);
        assert_eq!(s.installed.selected, 1);
        apply(&mut s, Action::MoveDown);
        apply(&mut s, Action::MoveDown); // clamp at last index (2)
        assert_eq!(s.installed.selected, 2);
        apply(&mut s, Action::MoveUp);
        assert_eq!(s.installed.selected, 1);
    }

    #[test]
    fn move_does_not_panic_on_empty_list() {
        let mut s = state();
        s.installed.filter = InstalledFilter::Foreign;
        s.recompute_installed(); // 1 item (htop)
        apply(&mut s, Action::MoveDown);
        apply(&mut s, Action::MoveDown);
        assert_eq!(s.installed.selected, 0);
    }

    #[test]
    fn toggle_select_current_adds_and_removes_by_name() {
        let mut s = state();
        let name = s.current_entry().unwrap().name.clone();
        apply(&mut s, Action::ToggleSelectCurrent);
        assert!(s.installed.checked.contains(&name));
        apply(&mut s, Action::ToggleSelectCurrent);
        assert!(!s.installed.checked.contains(&name));
    }

    #[test]
    fn filter_cycling_recomputes_visible() {
        let mut s = state();
        assert_eq!(s.installed.visible.len(), 3);
        apply(&mut s, Action::CycleFilterNext); // -> Explicit
        assert_eq!(s.installed.filter, InstalledFilter::Explicit);
        assert_eq!(s.installed.visible.len(), 2); // ripgrep, eza
    }

    #[test]
    fn sort_cycle_covers_all_six_states_and_wraps() {
        let mut s = state();
        let mut seen = vec![(s.installed.sort, s.installed.sort_desc)];
        for _ in 0..6 {
            apply(&mut s, Action::CycleSort);
            seen.push((s.installed.sort, s.installed.sort_desc));
        }
        // After 6 cycles we're back to the start.
        assert_eq!(seen[0], seen[6]);
        // All 6 intermediate states are distinct.
        let unique: std::collections::HashSet<_> = seen[..6].iter().collect();
        assert_eq!(unique.len(), 6);
    }

    #[test]
    fn search_typing_and_backspace() {
        let mut s = state();
        s.tab = Tab::Search;
        apply(&mut s, Action::SearchInsertChar('f'));
        apply(&mut s, Action::SearchInsertChar('d'));
        assert_eq!(s.search.query, "fd");
        assert_eq!(s.search.results.len(), 1);
        apply(&mut s, Action::SearchBackspace);
        assert_eq!(s.search.query, "f");
        apply(&mut s, Action::SearchClear);
        assert_eq!(s.search.query, "");
        assert_eq!(s.search.results.len(), 4); // everything, unranked
    }

    #[test]
    fn goto_tab_and_next_prev() {
        let mut s = state();
        apply(&mut s, Action::GoToTab(Tab::Updates));
        assert_eq!(s.tab, Tab::Updates);
        apply(&mut s, Action::NextTab);
        assert_eq!(s.tab, Tab::Installed);
        apply(&mut s, Action::PrevTab);
        assert_eq!(s.tab, Tab::Updates);
    }

    // -- Phase 3: request/confirm/execute --------------------------

    fn select_by_name(s: &mut AppState, name: &str) {
        let idx = s
            .installed
            .visible
            .iter()
            .position(|&i| s.entries[i].name == name)
            .unwrap();
        s.installed.selected = idx;
    }

    #[test]
    fn request_remove_opens_an_unguarded_modal_for_an_ordinary_package() {
        let mut s = state();
        s.tab = Tab::Installed;
        select_by_name(&mut s, "ripgrep");
        apply(&mut s, Action::RequestRemove);

        let modal = s.modal.as_ref().expect("modal should be open");
        assert!(!modal.is_guarded());
        assert!(modal.can_confirm());
        assert_eq!(modal.command.display, "sudo pacman -Rns ripgrep");
    }

    #[test]
    fn request_remove_on_an_unprotected_package_stays_unguarded() {
        // htop isn't one of `txn::guard`'s protected names -- see
        // `remove_on_a_protected_name_is_guarded_...` below for the
        // integration point that actually exercises a protected one,
        // via a hand-built entry rather than extending the shared
        // fixtures (which several other tests assert exact counts
        // against).
        let mut s = state();
        s.tab = Tab::Installed;
        select_by_name(&mut s, "htop");
        apply(&mut s, Action::RequestRemove);
        let modal = s.modal.as_ref().unwrap();
        assert!(!modal.is_guarded());
    }

    fn protected_entry(name: &str) -> crate::alpm::index::PackageEntry {
        use crate::alpm::local::{InstallReason, LocalPackage};
        crate::alpm::index::PackageEntry {
            name: name.to_string(),
            local: Some(LocalPackage {
                name: name.to_string(),
                version: "1.0-1".to_string(),
                base: None,
                description: String::new(),
                url: None,
                arch: None,
                license: vec![],
                packager: None,
                build_date: None,
                install_date: None,
                installed_size: 0,
                reason: InstallReason::Explicit,
                depends: vec![],
                optdepends: vec![],
                provides: vec![],
                conflicts: vec![],
                replaces: vec![],
                groups: vec![],
            }),
            sync: None,
        }
    }

    #[test]
    fn remove_on_a_protected_name_is_guarded_and_blocks_confirm_until_typed() {
        let index = PackageIndex::build(vec![], vec![]);
        let mut s = AppState::new(index, None);
        s.entries = vec![protected_entry("glibc")];
        s.recompute_installed();
        s.tab = Tab::Installed;
        s.installed.selected = 0;

        apply(&mut s, Action::RequestRemove);
        let modal = s.modal.as_ref().unwrap();
        assert!(modal.is_guarded());
        assert!(!modal.can_confirm());

        // Confirming without the right phrase must not execute.
        apply(&mut s, Action::ModalConfirm);
        assert!(
            s.modal.is_some(),
            "unconfirmed guarded modal must stay open"
        );
        assert!(s.execute.is_none());

        for c in "REMOVE".chars() {
            apply(&mut s, Action::ModalTypeChar(c));
        }
        assert!(s.modal.as_ref().unwrap().can_confirm());
        apply(&mut s, Action::ModalConfirm);
        assert!(
            s.modal.is_none(),
            "correctly-typed confirm should close the modal"
        );
        assert!(s.execute.is_some(), "and hand off a command to execute");
    }

    #[test]
    fn modal_cancel_discards_without_executing() {
        let mut s = state();
        select_by_name(&mut s, "ripgrep");
        apply(&mut s, Action::RequestRemove);
        assert!(s.modal.is_some());
        apply(&mut s, Action::ModalCancel);
        assert!(s.modal.is_none());
        assert!(s.execute.is_none());
    }

    #[test]
    fn modal_toggle_recursive_flips_the_planned_flag() {
        let mut s = state();
        select_by_name(&mut s, "ripgrep");
        apply(&mut s, Action::RequestRemove);
        assert_eq!(
            s.modal.as_ref().unwrap().command.args,
            vec!["pacman", "-Rns", "ripgrep"]
        );

        apply(&mut s, Action::ModalToggleRecursive);
        assert_eq!(
            s.modal.as_ref().unwrap().command.args,
            vec!["pacman", "-R", "ripgrep"]
        );

        apply(&mut s, Action::ModalToggleRecursive);
        assert_eq!(
            s.modal.as_ref().unwrap().command.args,
            vec!["pacman", "-Rns", "ripgrep"]
        );
    }

    #[test]
    fn multi_select_targets_the_whole_checked_set_not_just_current_row() {
        let mut s = state();
        s.installed.checked.insert("ripgrep".to_string());
        s.installed.checked.insert("eza".to_string());
        select_by_name(&mut s, "htop"); // current row deliberately NOT in the checked set

        apply(&mut s, Action::RequestMarkExplicit);
        let modal = s.modal.as_ref().unwrap();
        let mut targets = modal.action.packages().to_vec();
        targets.sort();
        assert_eq!(targets, vec!["eza".to_string(), "ripgrep".to_string()]);
    }

    #[test]
    fn install_delegates_to_aur_helper_when_target_has_no_repo() {
        // htop is foreign in the fixture set (installed, absent from
        // the extra.db sync fixture) -- `open_modal`'s AUR-need check
        // only inspects `repo().is_none()`, so this exercises that
        // branch even though pressing Install on an already-installed
        // package is not a real user flow; Phase 5's AUR search will
        // be what actually surfaces not-yet-installed AUR results.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let locals = read_local_db(&root.join("local")).unwrap();
        let syncs = read_sync_db(&root.join("sync/extra.db"), "extra").unwrap();
        let mut s = AppState::new(PackageIndex::build(locals, syncs), Some("yay".to_string()));
        select_by_name(&mut s, "htop");

        apply(&mut s, Action::RequestInstall);
        let modal = s.modal.as_ref().unwrap();
        assert_eq!(modal.command.program, "yay");
        assert_eq!(modal.command.args, vec!["-S", "htop"]);
    }

    #[test]
    fn install_of_a_repo_package_never_uses_the_helper() {
        let mut s = state();
        s.aur_helper = Some("yay".to_string());
        select_by_name(&mut s, "ripgrep"); // present in the extra.db fixture
        apply(&mut s, Action::RequestInstall);
        let modal = s.modal.as_ref().unwrap();
        assert_eq!(modal.command.program, "sudo");
    }

    #[test]
    fn update_system_prefers_the_helper_when_one_is_set() {
        let mut s = state();
        s.aur_helper = Some("paru".to_string());
        apply(&mut s, Action::RequestUpdateSystem);
        let modal = s.modal.as_ref().unwrap();
        assert_eq!(modal.command.program, "paru");
        assert_eq!(modal.command.args, vec!["-Syu"]);
    }

    #[test]
    fn request_list_files_and_open_url_skip_the_modal_entirely() {
        let mut s = state();
        select_by_name(&mut s, "ripgrep");

        apply(&mut s, Action::RequestListFiles);
        assert!(s.modal.is_none());
        let cmd = s
            .execute
            .take()
            .expect("list files should execute directly");
        assert_eq!(cmd.args, vec!["-Ql", "ripgrep"]);

        apply(&mut s, Action::RequestOpenUrl);
        assert!(s.modal.is_none());
        let cmd = s.execute.take().expect("open url should execute directly");
        assert_eq!(cmd.program, "xdg-open");
        assert_eq!(cmd.args, vec!["https://github.com/BurntSushi/ripgrep"]);
    }

    #[test]
    fn request_with_nothing_selected_and_nothing_current_does_nothing() {
        // Apps/Updates tabs have no package list at all.
        let mut s = state();
        s.tab = Tab::Apps;
        apply(&mut s, Action::RequestRemove);
        assert!(s.modal.is_none());
    }

    // -- Apps tab ----------------------------------------------------

    fn managed_entry(name: &str, kind: LauncherKind) -> crate::launcher::entry::ManagedEntry {
        crate::launcher::entry::ManagedEntry {
            path: std::path::PathBuf::from(format!("/tmp/pacpad-{name}.desktop")),
            name: name.to_string(),
            kind,
            exec: format!("echo {name}"),
            icon: None,
            url: Some("https://example.com".to_string()),
            command: Some("btop".to_string()),
            window_class: Some(
                crate::launcher::terminal::WindowStyle::Float
                    .class()
                    .to_string(),
            ),
            created: None,
        }
    }

    fn state_with_apps() -> AppState {
        let mut s = state();
        s.tab = Tab::Apps;
        s.apps.webapps = vec![managed_entry("Excalidraw", LauncherKind::Webapp)];
        s.apps.tuiapps = vec![managed_entry("Btop", LauncherKind::Tui)];
        s
    }

    #[test]
    fn request_add_webapp_and_tui_open_empty_forms() {
        let mut s = state();
        s.tab = Tab::Apps;
        apply(&mut s, Action::RequestAddWebapp);
        assert!(matches!(s.apps.form, Some(AppForm::Webapp(_))));
        apply(&mut s, Action::RequestAddTui);
        assert!(matches!(s.apps.form, Some(AppForm::Tui(_))));
    }

    #[test]
    fn request_edit_app_prefills_from_the_selected_entry_by_kind() {
        let mut s = state_with_apps();
        s.apps.selected = 0; // Excalidraw (webapp)
        apply(&mut s, Action::RequestEditApp);
        match &s.apps.form {
            Some(AppForm::Webapp(f)) => assert_eq!(f.name.value, "Excalidraw"),
            other => panic!("expected a webapp form, got {other:?}"),
        }

        s.apps.form = None;
        s.apps.selected = 1; // Btop (tui)
        apply(&mut s, Action::RequestEditApp);
        match &s.apps.form {
            Some(AppForm::Tui(f)) => assert_eq!(f.command.value, "btop"),
            other => panic!("expected a tui form, got {other:?}"),
        }
    }

    #[test]
    fn request_remove_app_dispatches_by_kind_with_no_modal() {
        let mut s = state_with_apps();
        s.apps.selected = 0;
        apply(&mut s, Action::RequestRemoveApp);
        assert!(
            s.modal.is_none(),
            "launcher removal skips the confirm modal"
        );
        assert!(matches!(s.apps_action, Some(AppsAction::RemoveWebapp(_))));

        s.apps_action = None;
        s.apps.selected = 1;
        apply(&mut s, Action::RequestRemoveApp);
        assert!(matches!(s.apps_action, Some(AppsAction::RemoveTui(_))));
    }

    #[test]
    fn launch_app_carries_the_entrys_exec_line() {
        let mut s = state_with_apps();
        s.apps.selected = 0;
        apply(&mut s, Action::LaunchApp);
        assert!(
            matches!(s.apps_action, Some(AppsAction::Launch(exec)) if exec == "echo Excalidraw")
        );
    }

    #[test]
    fn form_focus_next_and_prev_cycle_within_a_webapp_form() {
        let mut s = state();
        apply(&mut s, Action::RequestAddWebapp);
        let focus = |s: &AppState| match &s.apps.form {
            Some(AppForm::Webapp(f)) => f.focus,
            _ => panic!("expected webapp form"),
        };
        assert_eq!(focus(&s), crate::app::state::WebappField::Name);
        apply(&mut s, Action::FormFocusNext);
        assert_eq!(focus(&s), crate::app::state::WebappField::Url);
        apply(&mut s, Action::FormFocusPrev);
        assert_eq!(focus(&s), crate::app::state::WebappField::Name);
    }

    #[test]
    fn form_typing_routes_to_the_focused_field_only() {
        let mut s = state();
        apply(&mut s, Action::RequestAddWebapp);
        apply(&mut s, Action::FormInsertChar('h'));
        apply(&mut s, Action::FormInsertChar('i'));
        apply(&mut s, Action::FormFocusNext); // -> Url
        apply(&mut s, Action::FormInsertChar('x'));

        match &s.apps.form {
            Some(AppForm::Webapp(f)) => {
                assert_eq!(f.name.value, "hi");
                assert_eq!(f.url.value, "x");
            }
            _ => panic!("expected webapp form"),
        }
    }

    #[test]
    fn form_backspace_and_cursor_editing_work_through_the_action_layer() {
        let mut s = state();
        apply(&mut s, Action::RequestAddWebapp);
        for c in "helo".chars() {
            apply(&mut s, Action::FormInsertChar(c));
        }
        apply(&mut s, Action::FormLeft);
        apply(&mut s, Action::FormLeft);
        apply(&mut s, Action::FormInsertChar('l'));
        match &s.apps.form {
            Some(AppForm::Webapp(f)) => assert_eq!(f.name.value, "hello"),
            _ => panic!("expected webapp form"),
        }
        apply(&mut s, Action::FormEnd);
        apply(&mut s, Action::FormBackspace);
        match &s.apps.form {
            Some(AppForm::Webapp(f)) => assert_eq!(f.name.value, "hell"),
            _ => panic!("expected webapp form"),
        }
    }

    #[test]
    fn form_left_right_toggles_window_style_on_the_tui_forms_style_field() {
        let mut s = state();
        apply(&mut s, Action::RequestAddTui);
        // Cycle focus to the Style field: Name -> Command -> Style.
        apply(&mut s, Action::FormFocusNext);
        apply(&mut s, Action::FormFocusNext);
        let style = |s: &AppState| match &s.apps.form {
            Some(AppForm::Tui(f)) => f.style,
            _ => panic!("expected tui form"),
        };
        assert_eq!(style(&s), crate::launcher::terminal::WindowStyle::Float);
        apply(&mut s, Action::FormRight);
        assert_eq!(style(&s), crate::launcher::terminal::WindowStyle::Tile);
        apply(&mut s, Action::FormLeft);
        assert_eq!(style(&s), crate::launcher::terminal::WindowStyle::Float);
    }

    #[test]
    fn form_submit_with_missing_fields_sets_an_error_and_keeps_the_form_open() {
        let mut s = state();
        apply(&mut s, Action::RequestAddWebapp);
        apply(&mut s, Action::FormSubmit);
        match &s.apps.form {
            Some(AppForm::Webapp(f)) => assert!(f.error.is_some()),
            _ => panic!("form should stay open with an error, not close"),
        }
        assert!(s.apps_action.is_none());
    }

    #[test]
    fn form_submit_with_valid_fields_queues_a_create_action_and_closes_the_form() {
        let mut s = state();
        apply(&mut s, Action::RequestAddWebapp);
        for c in "Excalidraw".chars() {
            apply(&mut s, Action::FormInsertChar(c));
        }
        apply(&mut s, Action::FormFocusNext);
        for c in "https://excalidraw.com".chars() {
            apply(&mut s, Action::FormInsertChar(c));
        }
        apply(&mut s, Action::FormSubmit);

        assert!(s.apps.form.is_none());
        match s.apps_action {
            Some(AppsAction::CreateWebapp(new)) => {
                assert_eq!(new.name, "Excalidraw");
                assert_eq!(new.url, "https://excalidraw.com");
            }
            other => panic!(
                "expected CreateWebapp, got a different or missing action: {}",
                other.is_some()
            ),
        }
    }

    #[test]
    fn form_submit_while_editing_queues_an_update_action_with_the_original_path() {
        let mut s = state_with_apps();
        s.apps.selected = 0;
        apply(&mut s, Action::RequestEditApp);
        apply(&mut s, Action::FormSubmit); // fields already valid from prefill

        match s.apps_action {
            Some(AppsAction::UpdateWebapp(path, _)) => {
                assert_eq!(
                    path,
                    std::path::PathBuf::from("/tmp/pacpad-Excalidraw.desktop")
                );
            }
            other => panic!("expected UpdateWebapp, got: {}", other.is_some()),
        }
    }

    #[test]
    fn form_cancel_discards_the_form_without_queuing_an_action() {
        let mut s = state();
        apply(&mut s, Action::RequestAddTui);
        apply(&mut s, Action::FormInsertChar('x'));
        apply(&mut s, Action::FormCancel);
        assert!(s.apps.form.is_none());
        assert!(s.apps_action.is_none());
    }
}
