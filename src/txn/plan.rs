//! Turns an intended action into the exact command line that will run
//! -- pure computation, no I/O, no process spawning. This is what
//! makes the whole write path testable without root: every branch
//! here (recursive vs. plain remove, pacman vs. an AUR helper) is a
//! golden-test-able function from inputs to an argv.
//!
//! Every command that touches the package database runs through
//! `sudo` unless it's delegated to an AUR helper (`yay`/`paru`), which
//! elevates internally for its own pacman calls -- wrapping a helper
//! invocation in `sudo` too would ask for a password to run a program
//! that immediately re-prompts for one itself.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedCommand {
    pub program: String,
    pub args: Vec<String>,
    /// Human-readable form for the confirm modal -- what actually runs
    /// via `program`/`args` is the source of truth; this is `program`
    /// plus `args` space-joined (with defensive quoting for any
    /// argument that happens to contain whitespace, though package
    /// names never do in practice).
    pub display: String,
}

impl PlannedCommand {
    fn new(program: &str, args: Vec<String>) -> Self {
        let mut display = String::from(program);
        for a in &args {
            display.push(' ');
            if a.contains(' ') {
                display.push('"');
                display.push_str(a);
                display.push('"');
            } else {
                display.push_str(a);
            }
        }
        PlannedCommand {
            program: program.to_string(),
            args,
            display,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Install {
        packages: Vec<String>,
    },
    /// `recursive` selects `-Rns` (also remove now-unneeded
    /// dependencies and their configs) vs. plain `-R` -- the confirm
    /// modal offers this as a toggle, per the plan.
    Remove {
        packages: Vec<String>,
        recursive: bool,
    },
    Reinstall {
        packages: Vec<String>,
    },
    MarkExplicit {
        packages: Vec<String>,
    },
    MarkDependency {
        packages: Vec<String>,
    },
    UpdateSystem,
    ListFiles {
        package: String,
    },
    OpenUrl {
        url: String,
    },
}

impl Action {
    /// The packages this action targets, for display in the confirm
    /// modal and for the guard check. Empty for actions with no
    /// package list (`UpdateSystem`, `OpenUrl`).
    pub fn packages(&self) -> &[String] {
        match self {
            Action::Install { packages }
            | Action::Remove { packages, .. }
            | Action::Reinstall { packages }
            | Action::MarkExplicit { packages }
            | Action::MarkDependency { packages } => packages,
            Action::UpdateSystem | Action::ListFiles { .. } | Action::OpenUrl { .. } => &[],
        }
    }

    pub fn is_destructive_removal(&self) -> bool {
        matches!(self, Action::Remove { .. })
    }
}

/// `aur_helper`: the detected helper binary (`yay`, `paru`, ...), if
/// any -- used for `Install`/`Reinstall`/`UpdateSystem` when at least
/// one target package isn't in any configured repo. Passed in rather
/// than detected here: detection is Phase 5's job (`aur::helper`);
/// this function only ever branches on the `Option` it's given.
pub fn plan(action: &Action, aur_helper: Option<&str>) -> PlannedCommand {
    match action {
        Action::Install { packages } => match aur_helper {
            Some(helper) => PlannedCommand::new(helper, args_with("-S", packages)),
            None => PlannedCommand::new("sudo", prefixed_args("pacman", "-S", packages)),
        },
        Action::Remove {
            packages,
            recursive,
        } => {
            let flag = if *recursive { "-Rns" } else { "-R" };
            PlannedCommand::new("sudo", prefixed_args("pacman", flag, packages))
        }
        Action::Reinstall { packages } => match aur_helper {
            Some(helper) => PlannedCommand::new(helper, args_with("-S", packages)),
            None => PlannedCommand::new("sudo", prefixed_args("pacman", "-S", packages)),
        },
        Action::MarkExplicit { packages } => PlannedCommand::new(
            "sudo",
            prefixed_args_2("pacman", "-D", "--asexplicit", packages),
        ),
        Action::MarkDependency { packages } => PlannedCommand::new(
            "sudo",
            prefixed_args_2("pacman", "-D", "--asdeps", packages),
        ),
        Action::UpdateSystem => match aur_helper {
            Some(helper) => PlannedCommand::new(helper, vec!["-Syu".to_string()]),
            None => PlannedCommand::new("sudo", vec!["pacman".to_string(), "-Syu".to_string()]),
        },
        Action::ListFiles { package } => {
            PlannedCommand::new("pacman", vec!["-Ql".to_string(), package.clone()])
        }
        Action::OpenUrl { url } => PlannedCommand::new("xdg-open", vec![url.clone()]),
    }
}

fn args_with(flag: &str, packages: &[String]) -> Vec<String> {
    let mut args = vec![flag.to_string()];
    args.extend(packages.iter().cloned());
    args
}

fn prefixed_args(program: &str, flag: &str, packages: &[String]) -> Vec<String> {
    let mut args = vec![program.to_string(), flag.to_string()];
    args.extend(packages.iter().cloned());
    args
}

fn prefixed_args_2(program: &str, flag1: &str, flag2: &str, packages: &[String]) -> Vec<String> {
    let mut args = vec![program.to_string(), flag1.to_string(), flag2.to_string()];
    args.extend(packages.iter().cloned());
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkgs(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn install_via_pacman_without_aur_helper() {
        let action = Action::Install {
            packages: pkgs(&["ripgrep", "fd"]),
        };
        let cmd = plan(&action, None);
        assert_eq!(cmd.program, "sudo");
        assert_eq!(cmd.args, vec!["pacman", "-S", "ripgrep", "fd"]);
        assert_eq!(cmd.display, "sudo pacman -S ripgrep fd");
    }

    #[test]
    fn install_delegates_to_aur_helper_without_outer_sudo() {
        let action = Action::Install {
            packages: pkgs(&["ripgrep-all"]),
        };
        let cmd = plan(&action, Some("yay"));
        assert_eq!(cmd.program, "yay");
        assert_eq!(cmd.args, vec!["-S", "ripgrep-all"]);
        assert_eq!(cmd.display, "yay -S ripgrep-all");
    }

    #[test]
    fn remove_recursive_uses_rns() {
        let action = Action::Remove {
            packages: pkgs(&["cowsay"]),
            recursive: true,
        };
        let cmd = plan(&action, None);
        assert_eq!(cmd.args, vec!["pacman", "-Rns", "cowsay"]);
    }

    #[test]
    fn remove_plain_uses_r() {
        let action = Action::Remove {
            packages: pkgs(&["cowsay"]),
            recursive: false,
        };
        let cmd = plan(&action, None);
        assert_eq!(cmd.args, vec!["pacman", "-R", "cowsay"]);
    }

    #[test]
    fn reinstall_is_plain_install_with_same_aur_rule() {
        let action = Action::Reinstall {
            packages: pkgs(&["ripgrep"]),
        };
        assert_eq!(plan(&action, None).args, vec!["pacman", "-S", "ripgrep"]);
        assert_eq!(plan(&action, Some("paru")).args, vec!["-S", "ripgrep"]);
    }

    #[test]
    fn mark_explicit_and_dependency() {
        let mark_explicit = Action::MarkExplicit {
            packages: pkgs(&["htop"]),
        };
        assert_eq!(
            plan(&mark_explicit, None).args,
            vec!["pacman", "-D", "--asexplicit", "htop"]
        );

        let mark_dep = Action::MarkDependency {
            packages: pkgs(&["htop"]),
        };
        assert_eq!(
            plan(&mark_dep, None).args,
            vec!["pacman", "-D", "--asdeps", "htop"]
        );
    }

    #[test]
    fn update_system_prefers_helper_when_available() {
        assert_eq!(
            plan(&Action::UpdateSystem, None).args,
            vec!["pacman", "-Syu"]
        );
        let with_helper = plan(&Action::UpdateSystem, Some("yay"));
        assert_eq!(with_helper.program, "yay");
        assert_eq!(with_helper.args, vec!["-Syu"]);
    }

    #[test]
    fn list_files_and_open_url_need_no_privilege() {
        let files = plan(
            &Action::ListFiles {
                package: "ripgrep".to_string(),
            },
            None,
        );
        assert_eq!(files.program, "pacman");
        assert_eq!(files.args, vec!["-Ql", "ripgrep"]);

        let open = plan(
            &Action::OpenUrl {
                url: "https://example.com".to_string(),
            },
            None,
        );
        assert_eq!(open.program, "xdg-open");
        assert_eq!(open.args, vec!["https://example.com"]);
    }

    #[test]
    fn packages_accessor_is_empty_for_whole_system_actions() {
        assert!(Action::UpdateSystem.packages().is_empty());
        assert!(Action::OpenUrl {
            url: "x".to_string()
        }
        .packages()
        .is_empty());
        assert_eq!(
            Action::Remove {
                packages: pkgs(&["a", "b"]),
                recursive: true
            }
            .packages(),
            &["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn only_remove_is_flagged_destructive() {
        assert!(Action::Remove {
            packages: pkgs(&["x"]),
            recursive: true
        }
        .is_destructive_removal());
        assert!(!Action::Install {
            packages: pkgs(&["x"])
        }
        .is_destructive_removal());
        assert!(!Action::UpdateSystem.is_destructive_removal());
    }
}
