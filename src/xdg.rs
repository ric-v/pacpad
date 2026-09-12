//! XDG Base Directory helpers. `$XDG_*_HOME` are unset on plenty of
//! real systems (confirmed unset on this one) -- the spec's fallback
//! defaults are not optional extras, they're the common case.

use std::path::PathBuf;

pub fn data_home() -> PathBuf {
    from_env_or_home("XDG_DATA_HOME", ".local/share")
}

pub fn config_home() -> PathBuf {
    from_env_or_home("XDG_CONFIG_HOME", ".config")
}

pub fn cache_home() -> PathBuf {
    from_env_or_home("XDG_CACHE_HOME", ".cache")
}

fn from_env_or_home(var: &str, fallback_under_home: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(fallback_under_home))
}

/// `$HOME`, or `.` if it's somehow unset -- used directly (not just as
/// a fallback base) by anything that needs to search a fixed
/// `~/something` path outside the XDG spec, like
/// `~/.nix-profile/share/applications`.
pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// `~/.local/share/applications` -- where every launcher pacpad
/// creates (and only those) lives.
pub fn applications_dir() -> PathBuf {
    data_home().join("applications")
}

/// `~/.local/share/pacpad/icons` -- the *only* directory an icon may
/// be removed from; this is what makes the ownership invariant's icon
/// cleanup safe (see `launcher::entry`).
pub fn icons_dir() -> PathBuf {
    data_home().join("pacpad").join("icons")
}

#[cfg(test)]
mod tests {
    use super::*;

    // `env::set_var`/`remove_var` are process-wide, but `cargo test`
    // runs test functions concurrently by default -- without this
    // lock, these two tests (or any other test that happens to touch
    // HOME/XDG_*) could interleave and read each other's half-applied
    // state. Every test in this module that mutates env vars holds
    // this for its whole body.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn falls_back_under_home_when_env_unset() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let saved_home = std::env::var_os("HOME");
        let saved_data = std::env::var_os("XDG_DATA_HOME");

        // SAFETY: serialized by `ENV_LOCK` above.
        unsafe {
            std::env::remove_var("XDG_DATA_HOME");
            std::env::set_var("HOME", "/home/testuser");
        }
        assert_eq!(data_home(), PathBuf::from("/home/testuser/.local/share"));
        assert_eq!(
            applications_dir(),
            PathBuf::from("/home/testuser/.local/share/applications")
        );
        assert_eq!(
            icons_dir(),
            PathBuf::from("/home/testuser/.local/share/pacpad/icons")
        );

        unsafe {
            restore(saved_home, "HOME");
            restore(saved_data, "XDG_DATA_HOME");
        }
    }

    #[test]
    fn honors_env_override_when_set() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let saved = std::env::var_os("XDG_CONFIG_HOME");

        // SAFETY: serialized by `ENV_LOCK` above.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", "/custom/config");
        }
        assert_eq!(config_home(), PathBuf::from("/custom/config"));

        unsafe {
            restore(saved, "XDG_CONFIG_HOME");
        }
    }

    /// SAFETY: caller must already hold `ENV_LOCK`.
    unsafe fn restore(value: Option<std::ffi::OsString>, var: &str) {
        match value {
            Some(v) => unsafe { std::env::set_var(var, v) },
            None => unsafe { std::env::remove_var(var) },
        }
    }
}
