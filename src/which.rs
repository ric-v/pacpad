//! A plain `$PATH` scan for an executable file -- no subprocess, no
//! `libc::access` call, just a directory-entry check. Shared by AUR
//! helper detection (`cli::detect_aur_helper`) and the browser/
//! terminal resolution chains in `launcher::{browser,terminal}`.

use std::path::PathBuf;

/// Resolves `name` to a full path if it exists as a regular file in
/// any `$PATH` entry, in `$PATH` order. Doesn't check the executable
/// bit -- a non-executable match is rare enough, and the eventual
/// `Command::spawn` will surface a clear "Permission denied" if it
/// ever happens, rather than this silently skipping a real match.
pub fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_binary_known_to_exist_on_any_posix_system() {
        // /bin/sh is as close to universal as it gets; still resolve
        // it through PATH rather than hardcoding /bin/sh, since this
        // is testing PATH resolution itself.
        let found = which("sh");
        assert!(found.is_some(), "expected to find `sh` on $PATH");
        assert!(found.unwrap().is_file());
    }

    #[test]
    fn returns_none_for_a_name_that_does_not_exist() {
        assert_eq!(which("pacpad-definitely-does-not-exist-anywhere"), None);
    }
}
