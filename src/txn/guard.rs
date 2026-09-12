//! Hard-blocks casual removal of packages that would break the
//! system: `pacman`, `sudo`, anything named `linux*` (the kernel and
//! its variants/headers/firmware), `systemd`, `glibc`, and anything in
//! the `base` group. These don't get a plain confirm keypress -- the
//! modal requires the user to type a confirmation phrase instead,
//! per the plan.
//!
//! Scoped to removal only, matching the plan: reinstalling or
//! updating `glibc` is normal maintenance; removing it is how you
//! brick the system.

use crate::alpm::index::PackageEntry;

const PROTECTED_NAMES: &[&str] = &["pacman", "sudo", "systemd", "glibc"];
const PROTECTED_GROUP: &str = "base";

/// `Some(reason)` if removing `entry` should require typed
/// confirmation rather than a keypress; `None` if it's an ordinary
/// removal.
pub fn protection_reason(entry: &PackageEntry) -> Option<&'static str> {
    // Nothing to guard if it isn't even installed -- there's no
    // removal to protect against. Checked first so a search result
    // for e.g. "linux-zen" that isn't on this system doesn't get
    // flagged just because of its name.
    let local = entry.local.as_ref()?;

    if PROTECTED_NAMES.contains(&entry.name.as_str()) {
        return Some("a critical system package");
    }
    if entry.name.starts_with("linux") {
        return Some("a kernel package");
    }
    if local.groups.iter().any(|g| g == PROTECTED_GROUP) {
        return Some("a member of the base group");
    }
    None
}

/// Every protected package among `entries`, with its reason -- empty
/// if none are protected, in which case the removal needs only the
/// ordinary confirm keypress.
pub fn protected_among<'a>(
    entries: impl IntoIterator<Item = &'a PackageEntry>,
) -> Vec<(&'a str, &'static str)> {
    entries
        .into_iter()
        .filter_map(|e| protection_reason(e).map(|r| (e.name.as_str(), r)))
        .collect()
}

/// The phrase a guarded removal's confirm modal requires the user to
/// type, verbatim, before it will proceed.
pub const CONFIRMATION_PHRASE: &str = "REMOVE";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alpm::index::PackageEntry;
    use crate::alpm::local::{InstallReason, LocalPackage};

    fn local_pkg(name: &str, groups: &[&str]) -> PackageEntry {
        PackageEntry {
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
                groups: groups.iter().map(|s| s.to_string()).collect(),
            }),
            sync: None,
        }
    }

    #[test]
    fn named_critical_packages_are_protected() {
        for name in ["pacman", "sudo", "systemd", "glibc"] {
            let pkg = local_pkg(name, &[]);
            assert!(
                protection_reason(&pkg).is_some(),
                "{name} should be protected"
            );
        }
    }

    #[test]
    fn linux_prefix_glob_catches_kernel_variants() {
        for name in [
            "linux",
            "linux-lts",
            "linux-zen",
            "linux-firmware",
            "linux-headers",
        ] {
            let pkg = local_pkg(name, &[]);
            assert!(
                protection_reason(&pkg).is_some(),
                "{name} should be protected"
            );
        }
    }

    #[test]
    fn linux_substring_but_not_prefix_is_not_protected() {
        // A package that merely contains "linux" (cross toolchain
        // naming) must not be caught by the glob -- only a name that
        // literally starts with "linux" should be.
        let pkg = local_pkg("aarch64-linux-gnu-gcc", &[]);
        assert!(protection_reason(&pkg).is_none());
    }

    #[test]
    fn base_group_membership_is_protected() {
        let pkg = local_pkg("filesystem", &["base"]);
        assert!(protection_reason(&pkg).is_some());
    }

    #[test]
    fn ordinary_package_is_not_protected() {
        let pkg = local_pkg("ripgrep", &[]);
        assert!(protection_reason(&pkg).is_none());
    }

    #[test]
    fn foreign_package_with_no_local_data_is_not_protected() {
        // sync-only entries (not installed) have no `local` at all --
        // guard only ever matters for something that's actually
        // installed and being removed.
        let pkg = PackageEntry {
            name: "linux-zen".to_string(),
            local: None,
            sync: None,
        };
        assert!(protection_reason(&pkg).is_none());
    }

    #[test]
    fn protected_among_collects_only_the_protected_ones() {
        let entries = [
            local_pkg("ripgrep", &[]),
            local_pkg("glibc", &[]),
            local_pkg("htop", &[]),
        ];
        let protected = protected_among(entries.iter());
        assert_eq!(protected.len(), 1);
        assert_eq!(protected[0].0, "glibc");
    }
}
