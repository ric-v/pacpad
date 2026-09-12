//! Maps a crossterm key event to an `Action`, depending on which tab
//! is active. The Search tab is always "focused" for typing (like
//! fzf/telescope-style pickers) -- there is no separate mode to enter
//! before characters go into the query, which is why quit is bound to
//! `q` everywhere *except* Search (where `q` is a valid query
//! character) plus Ctrl+C everywhere (unambiguous in every tab).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::state::{Modal, Tab};
use super::Action;
use crate::app::AppState;
use crate::txn;

pub fn map_key(state: &AppState, key: KeyEvent) -> Option<Action> {
    // Global bindings that must never be shadowed by a tab's own
    // handling, regardless of what's focused.
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(Action::Quit);
    }

    // The startup splash is a treat, not a gate: any key ends it
    // immediately rather than making an impatient user wait it out or
    // learn a specific dismiss key.
    if state.splash_active() {
        return Some(Action::DismissSplash);
    }

    // The help sheet, once open, owns every key the same way a modal
    // does -- F1 (or Esc/q) closes it, nothing else reaches the tab
    // underneath while it's up.
    if state.help_open {
        return map_help_key(key);
    }

    // A modal, once open, owns every key until it's confirmed or
    // cancelled -- Tab/BackTab must not switch tabs out from under it.
    if let Some(modal) = &state.modal {
        return map_modal_key(modal, key);
    }

    // Same for an open Apps-tab form: Tab/BackTab move between its own
    // fields (see `map_form_key`), not between the app's tabs.
    if state.apps.form.is_some() {
        return map_form_key(key);
    }

    // F1 rather than the more conventional `?`: the Search tab treats
    // every unmodified character as query text (see the module docs),
    // and `?` is a perfectly reasonable thing to type there, so a
    // function key is the only choice that can never collide.
    if key.code == KeyCode::F(1) {
        return Some(Action::ToggleHelp);
    }

    match key.code {
        KeyCode::Tab => return Some(Action::NextTab),
        KeyCode::BackTab => return Some(Action::PrevTab),
        _ => {}
    }

    match state.tab {
        Tab::Search => map_search_key(key),
        Tab::Installed => map_installed_key(key),
        Tab::Apps => map_apps_key(key),
        Tab::Updates => map_placeholder_key(key),
    }
}

fn map_apps_key(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Char(d @ '1'..='4') => Tab::from_digit(d).map(Action::GoToTab),
        KeyCode::Up | KeyCode::Char('k') => Some(Action::MoveUp),
        KeyCode::Down | KeyCode::Char('j') => Some(Action::MoveDown),
        KeyCode::Char('a') => Some(Action::RequestAddWebapp),
        KeyCode::Char('t') => Some(Action::RequestAddTui),
        KeyCode::Char('e') => Some(Action::RequestEditApp),
        KeyCode::Char('d') => Some(Action::RequestRemoveApp),
        KeyCode::Enter => Some(Action::LaunchApp),
        _ => None,
    }
}

/// Every Apps-tab form (add/edit webapp or TUI app) uses the same key
/// mapping regardless of which fields it has -- `apply()` routes
/// `FormLeft`/`FormRight`/`FormInsertChar` etc. to whatever's actually
/// focused, so this doesn't need to know the form's shape at all.
fn map_form_key(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Esc => Some(Action::FormCancel),
        KeyCode::Enter => Some(Action::FormSubmit),
        KeyCode::Tab => Some(Action::FormFocusNext),
        KeyCode::BackTab => Some(Action::FormFocusPrev),
        KeyCode::Backspace => Some(Action::FormBackspace),
        KeyCode::Delete => Some(Action::FormDelete),
        KeyCode::Left => Some(Action::FormLeft),
        KeyCode::Right => Some(Action::FormRight),
        KeyCode::Home => Some(Action::FormHome),
        KeyCode::End => Some(Action::FormEnd),
        KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
            Some(Action::FormInsertChar(c))
        }
        _ => None,
    }
}

fn map_installed_key(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Char('/') => Some(Action::GoToTab(Tab::Search)),
        KeyCode::Char(d @ '1'..='4') => Tab::from_digit(d).map(Action::GoToTab),
        KeyCode::Up | KeyCode::Char('k') => Some(Action::MoveUp),
        KeyCode::Down | KeyCode::Char('j') => Some(Action::MoveDown),
        KeyCode::Left | KeyCode::Char('h') => Some(Action::CycleFilterPrev),
        KeyCode::Right | KeyCode::Char('l') => Some(Action::CycleFilterNext),
        KeyCode::Char(' ') => Some(Action::ToggleSelectCurrent),
        KeyCode::Char('s') => Some(Action::CycleSort),
        // Package actions -- see the plan's "Package actions" table.
        // Only reachable here (never in Search) because this tab has
        // no free-text field for a letter to collide with.
        KeyCode::Char('i') => Some(Action::RequestInstall),
        KeyCode::Char('d') => Some(Action::RequestRemove),
        KeyCode::Char('r') => Some(Action::RequestReinstall),
        KeyCode::Char('e') => Some(Action::RequestMarkExplicit),
        KeyCode::Char('D') => Some(Action::RequestMarkDependency),
        KeyCode::Char('u') => Some(Action::RequestUpdateSystem),
        KeyCode::Char('f') => Some(Action::RequestListFiles),
        KeyCode::Char('o') => Some(Action::RequestOpenUrl),
        _ => None,
    }
}

fn map_search_key(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Up => Some(Action::MoveUp),
        KeyCode::Down => Some(Action::MoveDown),
        KeyCode::Esc => Some(Action::SearchClear),
        KeyCode::Backspace => Some(Action::SearchBackspace),
        KeyCode::Char(' ') => Some(Action::ToggleSelectCurrent),
        // Enter, not a letter, installs the current selection -- the
        // Search tab is always typing (see the module docs), so no
        // letter can be safely reserved here: `i` alone would make it
        // impossible to type "ripgrep", "vim", or anything else
        // containing it.
        KeyCode::Enter => Some(Action::RequestInstall),
        // Every other printable character edits the query -- this is
        // deliberately last so it never shadows Up/Down/Esc/Backspace/
        // Enter above, and it excludes control-modified characters
        // (e.g. Ctrl+C already returned above; Ctrl+<other> falls
        // through to None rather than being typed into the query).
        KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
            Some(Action::SearchInsertChar(c))
        }
        _ => None,
    }
}

fn map_modal_key(modal: &Modal, key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Esc => Some(Action::ModalCancel),
        KeyCode::Enter => Some(Action::ModalConfirm),
        KeyCode::Backspace if modal.is_guarded() => Some(Action::ModalBackspace),
        // Only offered when not guarded: a guarded removal's typed
        // confirmation phrase ("REMOVE") itself contains 'r', so that
        // keystroke must go to the typed buffer, not the toggle, once
        // guarded.
        KeyCode::Char('r')
            if !modal.is_guarded() && matches!(modal.action, txn::Action::Remove { .. }) =>
        {
            Some(Action::ModalToggleRecursive)
        }
        KeyCode::Char(c)
            if modal.is_guarded()
                && (key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT) =>
        {
            Some(Action::ModalTypeChar(c))
        }
        _ => None,
    }
}

fn map_help_key(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::F(1) | KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => {
            Some(Action::ToggleHelp)
        }
        _ => None,
    }
}

fn map_placeholder_key(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Char(d @ '1'..='4') => Tab::from_digit(d).map(Action::GoToTab),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alpm::index::PackageIndex;
    use crate::alpm::local::read_local_db;
    use crate::alpm::sync::read_sync_db;
    use std::path::Path;

    fn state() -> AppState {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let locals = read_local_db(&root.join("local")).unwrap();
        let syncs = read_sync_db(&root.join("sync/extra.db"), "extra").unwrap();
        AppState::new(PackageIndex::build(locals, syncs), None)
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn key_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn ctrl_c_quits_from_every_tab() {
        for tab in super::super::state::Tab::ALL {
            let mut s = state();
            s.tab = tab;
            let action = map_key(&s, key_mod(KeyCode::Char('c'), KeyModifiers::CONTROL));
            assert_eq!(action, Some(Action::Quit), "failed for tab {tab:?}");
        }
    }

    #[test]
    fn q_quits_outside_search_but_types_inside_search() {
        let mut s = state();
        s.tab = Tab::Installed;
        assert_eq!(map_key(&s, key(KeyCode::Char('q'))), Some(Action::Quit));

        s.tab = Tab::Search;
        assert_eq!(
            map_key(&s, key(KeyCode::Char('q'))),
            Some(Action::SearchInsertChar('q'))
        );
    }

    #[test]
    fn tab_and_backtab_switch_tabs_from_anywhere_including_search() {
        let mut s = state();
        s.tab = Tab::Search;
        assert_eq!(map_key(&s, key(KeyCode::Tab)), Some(Action::NextTab));
        assert_eq!(map_key(&s, key(KeyCode::BackTab)), Some(Action::PrevTab));
    }

    #[test]
    fn digit_keys_jump_tabs_outside_search_only() {
        let mut s = state();
        s.tab = Tab::Installed;
        assert_eq!(
            map_key(&s, key(KeyCode::Char('2'))),
            Some(Action::GoToTab(Tab::Search))
        );

        s.tab = Tab::Search;
        assert_eq!(
            map_key(&s, key(KeyCode::Char('2'))),
            Some(Action::SearchInsertChar('2')),
            "digits must type into the query, not jump tabs, while searching"
        );
    }

    #[test]
    fn installed_tab_navigation_and_filter_keys() {
        let s = {
            let mut s = state();
            s.tab = Tab::Installed;
            s
        };
        assert_eq!(map_key(&s, key(KeyCode::Down)), Some(Action::MoveDown));
        assert_eq!(map_key(&s, key(KeyCode::Char('j'))), Some(Action::MoveDown));
        assert_eq!(map_key(&s, key(KeyCode::Up)), Some(Action::MoveUp));
        assert_eq!(
            map_key(&s, key(KeyCode::Left)),
            Some(Action::CycleFilterPrev)
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Right)),
            Some(Action::CycleFilterNext)
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char(' '))),
            Some(Action::ToggleSelectCurrent)
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char('s'))),
            Some(Action::CycleSort)
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char('/'))),
            Some(Action::GoToTab(Tab::Search))
        );
    }

    #[test]
    fn search_tab_esc_clears_backspace_removes_and_arrows_navigate() {
        let s = {
            let mut s = state();
            s.tab = Tab::Search;
            s
        };
        assert_eq!(map_key(&s, key(KeyCode::Esc)), Some(Action::SearchClear));
        assert_eq!(
            map_key(&s, key(KeyCode::Backspace)),
            Some(Action::SearchBackspace)
        );
        assert_eq!(map_key(&s, key(KeyCode::Down)), Some(Action::MoveDown));
        assert_eq!(map_key(&s, key(KeyCode::Up)), Some(Action::MoveUp));
        assert_eq!(
            map_key(&s, key(KeyCode::Char('R'))),
            Some(Action::SearchInsertChar('R')),
            "shift+letter should type the uppercase char, not be swallowed"
        );
    }

    #[test]
    fn placeholder_tab_only_supports_quit_and_direct_tab_jump() {
        // Updates is still a real placeholder (no content built yet).
        let s = {
            let mut s = state();
            s.tab = Tab::Updates;
            s
        };
        assert_eq!(map_key(&s, key(KeyCode::Char('q'))), Some(Action::Quit));
        assert_eq!(
            map_key(&s, key(KeyCode::Char('1'))),
            Some(Action::GoToTab(Tab::Installed))
        );
        assert_eq!(map_key(&s, key(KeyCode::Down)), None);
    }

    #[test]
    fn apps_tab_navigation_and_launcher_keys() {
        let s = {
            let mut s = state();
            s.tab = Tab::Apps;
            s
        };
        assert_eq!(map_key(&s, key(KeyCode::Char('q'))), Some(Action::Quit));
        assert_eq!(map_key(&s, key(KeyCode::Down)), Some(Action::MoveDown));
        assert_eq!(map_key(&s, key(KeyCode::Char('j'))), Some(Action::MoveDown));
        assert_eq!(map_key(&s, key(KeyCode::Up)), Some(Action::MoveUp));
        assert_eq!(
            map_key(&s, key(KeyCode::Char('a'))),
            Some(Action::RequestAddWebapp)
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char('t'))),
            Some(Action::RequestAddTui)
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char('e'))),
            Some(Action::RequestEditApp)
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char('d'))),
            Some(Action::RequestRemoveApp)
        );
        assert_eq!(map_key(&s, key(KeyCode::Enter)), Some(Action::LaunchApp));
    }

    #[test]
    fn a_form_open_on_the_apps_tab_intercepts_every_key_including_tab() {
        let mut s = state();
        s.tab = Tab::Apps;
        s.apps.form = Some(crate::app::state::AppForm::Webapp(
            crate::app::state::WebappForm::empty(),
        ));

        assert_eq!(map_key(&s, key(KeyCode::Esc)), Some(Action::FormCancel));
        assert_eq!(map_key(&s, key(KeyCode::Enter)), Some(Action::FormSubmit));
        assert_eq!(map_key(&s, key(KeyCode::Tab)), Some(Action::FormFocusNext));
        assert_eq!(
            map_key(&s, key(KeyCode::BackTab)),
            Some(Action::FormFocusPrev)
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char('a'))),
            Some(Action::FormInsertChar('a')),
            "letters must type into the field, not trigger the 'a' add-webapp shortcut"
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Backspace)),
            Some(Action::FormBackspace)
        );
        assert_eq!(map_key(&s, key(KeyCode::Left)), Some(Action::FormLeft));
        assert_eq!(map_key(&s, key(KeyCode::Right)), Some(Action::FormRight));
        assert_eq!(map_key(&s, key(KeyCode::Home)), Some(Action::FormHome));
        assert_eq!(map_key(&s, key(KeyCode::End)), Some(Action::FormEnd));
        assert_eq!(map_key(&s, key(KeyCode::Delete)), Some(Action::FormDelete));
    }

    #[test]
    fn installed_tab_package_action_keys() {
        let s = {
            let mut s = state();
            s.tab = Tab::Installed;
            s
        };
        assert_eq!(
            map_key(&s, key(KeyCode::Char('i'))),
            Some(Action::RequestInstall)
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char('d'))),
            Some(Action::RequestRemove)
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char('r'))),
            Some(Action::RequestReinstall)
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char('e'))),
            Some(Action::RequestMarkExplicit)
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char('D'))),
            Some(Action::RequestMarkDependency)
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char('u'))),
            Some(Action::RequestUpdateSystem)
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char('f'))),
            Some(Action::RequestListFiles)
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char('o'))),
            Some(Action::RequestOpenUrl)
        );
    }

    #[test]
    fn search_tab_enter_installs_but_letters_still_type() {
        let s = {
            let mut s = state();
            s.tab = Tab::Search;
            s
        };
        assert_eq!(
            map_key(&s, key(KeyCode::Enter)),
            Some(Action::RequestInstall)
        );
        // The letter that would otherwise be an Installed-tab shortcut
        // must still just type here -- this is the conflict Enter was
        // chosen specifically to avoid.
        assert_eq!(
            map_key(&s, key(KeyCode::Char('i'))),
            Some(Action::SearchInsertChar('i'))
        );
    }

    fn state_with_remove_modal(guarded: bool) -> AppState {
        let mut s = state();
        let action = crate::txn::Action::Remove {
            packages: vec!["ripgrep".to_string()],
            recursive: true,
        };
        s.modal = Some(crate::app::state::Modal {
            command: crate::txn::plan_command(&action, None),
            action,
            guard_reasons: if guarded {
                vec![("ripgrep".to_string(), "test")]
            } else {
                vec![]
            },
            typed: String::new(),
        });
        s
    }

    #[test]
    fn modal_open_intercepts_every_key_ignoring_the_active_tab() {
        let mut s = state_with_remove_modal(false);
        s.tab = Tab::Search; // even in Search, the modal must own input
        assert_eq!(map_key(&s, key(KeyCode::Esc)), Some(Action::ModalCancel));
        assert_eq!(map_key(&s, key(KeyCode::Enter)), Some(Action::ModalConfirm));
        assert_eq!(
            map_key(&s, key(KeyCode::Tab)),
            None,
            "Tab must not switch tabs while a modal is open"
        );
    }

    #[test]
    fn unguarded_remove_modal_r_toggles_recursive() {
        let s = state_with_remove_modal(false);
        assert_eq!(
            map_key(&s, key(KeyCode::Char('r'))),
            Some(Action::ModalToggleRecursive)
        );
    }

    #[test]
    fn guarded_remove_modal_r_types_into_the_confirmation_buffer_instead() {
        let s = state_with_remove_modal(true);
        assert_eq!(
            map_key(&s, key(KeyCode::Char('r'))),
            Some(Action::ModalTypeChar('r'))
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Char('E'))),
            Some(Action::ModalTypeChar('E'))
        );
        assert_eq!(
            map_key(&s, key(KeyCode::Backspace)),
            Some(Action::ModalBackspace)
        );
    }

    #[test]
    fn unguarded_modal_backspace_does_nothing() {
        let s = state_with_remove_modal(false);
        assert_eq!(map_key(&s, key(KeyCode::Backspace)), None);
    }
}
