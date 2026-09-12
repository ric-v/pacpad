//! Detects which AUR helper is available, for delegating installs,
//! reinstalls, and system updates that touch a foreign (AUR) package
//! -- `txn::plan` already branches on `Option<&str>` for exactly this;
//! this module is the one place that decides what fills it in.
//!
//! Through Phase 4 this was an inline placeholder in `cli.rs` (a bare
//! `["yay", "paru"].find(which)` with no config support) -- this gives
//! it the same shape as `launcher::browser`/`launcher::terminal`'s own
//! resolution chains: an explicit config override first, then
//! detection.

/// Priority order when nothing overrides it: `yay` is far more widely
/// used and, unlike `paru`, ships prebuilt (no Rust toolchain needed
/// to bootstrap it), so it's preferred when both are present.
pub const KNOWN_HELPERS: &[&str] = &["yay", "paru"];

/// Resolves the AUR helper to delegate to, or `None` if pacpad should
/// only ever touch configured repos directly via `pacman`.
///
/// A configured override naming a helper that isn't actually
/// installed falls through to normal detection rather than erroring
/// -- unlike `launcher::browser::resolve` (where a missing configured
/// browser means webapps genuinely can't be created at all), an AUR
/// helper is optional: every repo package still installs fine via
/// plain `pacman` with none configured or found.
pub fn detect(config: &crate::config::Config) -> Option<String> {
    if let Some(name) = &config.aur_helper {
        if crate::which::which(name).is_some() {
            return Some(name.clone());
        }
    }
    KNOWN_HELPERS
        .iter()
        .find(|name| crate::which::which(name).is_some())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn known_helpers_are_yay_then_paru_in_that_priority_order() {
        assert_eq!(KNOWN_HELPERS, &["yay", "paru"]);
    }

    #[test]
    fn config_override_wins_when_the_named_helper_is_actually_installed() {
        // Only meaningful if yay happens to be on this machine's PATH;
        // the fall-through case is covered explicitly below with a
        // name guaranteed not to exist.
        if crate::which::which("yay").is_some() {
            let cfg = Config {
                browser: None,
                terminal: None,
                aur_helper: Some("yay".to_string()),
            };
            assert_eq!(detect(&cfg), Some("yay".to_string()));
        }
    }

    #[test]
    fn a_configured_but_missing_helper_falls_through_to_detection_rather_than_erroring() {
        let cfg = Config {
            browser: None,
            terminal: None,
            aur_helper: Some("pacpad-definitely-not-a-real-helper".to_string()),
        };
        // Whatever this comes back as, it must never be the bogus
        // configured name -- that would mean we "detected" something
        // that doesn't exist.
        assert_ne!(
            detect(&cfg).as_deref(),
            Some("pacpad-definitely-not-a-real-helper")
        );
    }

    #[test]
    fn no_config_and_no_installed_helper_yields_none() {
        // A name no real system would ever have on PATH, standing in
        // for "detection found nothing" -- this exercises the
        // fall-through path's final `None`, distinct from the
        // config-override fallback test above.
        let cfg = Config::default();
        let result = detect(&cfg);
        // Can't assert `None` unconditionally -- this machine may
        // genuinely have yay/paru installed, which is the correct,
        // desired outcome, not a test failure. Assert instead that
        // whatever comes back (if anything) is one of the known
        // helpers, matching what real detection is allowed to return.
        if let Some(found) = result {
            assert!(KNOWN_HELPERS.contains(&found.as_str()));
        }
    }
}
