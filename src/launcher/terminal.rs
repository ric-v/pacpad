//! Terminal detection and per-terminal argv construction for TUI-app
//! launchers.
//!
//! `xdg-terminal-exec` doesn't exist on this machine (or plenty of
//! others), which is exactly why omarchy's own `Exec=xdg-terminal-exec
//! --app-id=... -e $CMD` approach can't be copied verbatim -- the
//! flags a real terminal wants for "set a window class" and "run this
//! command" genuinely differ per terminal, so that's a data table, not
//! a single format string.
//!
//! The user's command is always wrapped in `sh -c "<command>"` rather
//! than naively split on whitespace: the omarchy convention this
//! mirrors explicitly expects compound commands (`bash -c 'dust; read
//! -n 1 -s'`), and only a shell can parse those correctly.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClassFlagStyle {
    /// `--class=<c>`
    Equals,
    /// `--class <c>` (two argv elements)
    Spaced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExecStyle {
    /// `<term> [class] -e sh -c <command>`
    DashE,
    /// `<term> start [class] -- sh -c <command>` (wezterm)
    StartDashDash,
    /// `<term> [class] sh -c <command>` (foot: no `-e` at all)
    Bare,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSpec {
    pub binary: &'static str,
    class_flag: &'static str,
    class_style: ClassFlagStyle,
    exec_style: ExecStyle,
}

/// The plan's own table, verbatim. Order matters: it's also the
/// detection fallback order when nothing more specific (config,
/// `xdg-terminal-exec`, `$TERMINAL`) resolves to something installed.
pub const TERMINALS: &[TerminalSpec] = &[
    TerminalSpec {
        binary: "ghostty",
        class_flag: "--class",
        class_style: ClassFlagStyle::Equals,
        exec_style: ExecStyle::DashE,
    },
    TerminalSpec {
        binary: "kitty",
        class_flag: "--class",
        class_style: ClassFlagStyle::Spaced,
        exec_style: ExecStyle::DashE,
    },
    TerminalSpec {
        binary: "wezterm",
        class_flag: "--class",
        class_style: ClassFlagStyle::Spaced,
        exec_style: ExecStyle::StartDashDash,
    },
    TerminalSpec {
        binary: "alacritty",
        class_flag: "--class",
        class_style: ClassFlagStyle::Spaced,
        exec_style: ExecStyle::DashE,
    },
    TerminalSpec {
        binary: "foot",
        class_flag: "--app-id",
        class_style: ClassFlagStyle::Equals,
        exec_style: ExecStyle::Bare,
    },
];

/// `pacpad-tui-float` or `pacpad-tui-tile` -- omarchy's own float/tile
/// convention, kept so existing window rules built against it keep
/// working even though pacpad is a different tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowStyle {
    Float,
    Tile,
}

impl WindowStyle {
    pub fn class(self) -> &'static str {
        match self {
            WindowStyle::Float => "pacpad-tui-float",
            WindowStyle::Tile => "pacpad-tui-tile",
        }
    }
}

fn find_spec(binary: &str) -> Option<&'static TerminalSpec> {
    TERMINALS.iter().find(|t| t.binary == binary)
}

/// Builds the full argv for running `command` inside `spec`'s
/// terminal, with `class` set as its window class. Pure data
/// transformation -- no process spawning, no terminal required to run
/// or test this.
pub fn build_argv(spec: &TerminalSpec, class: &str, command: &str) -> Vec<String> {
    let mut argv = vec![spec.binary.to_string()];

    let push_class = |argv: &mut Vec<String>| match spec.class_style {
        ClassFlagStyle::Equals => argv.push(format!("{}={}", spec.class_flag, class)),
        ClassFlagStyle::Spaced => {
            argv.push(spec.class_flag.to_string());
            argv.push(class.to_string());
        }
    };
    let push_shell_command = |argv: &mut Vec<String>| {
        argv.push("sh".to_string());
        argv.push("-c".to_string());
        argv.push(command.to_string());
    };

    match spec.exec_style {
        ExecStyle::DashE => {
            push_class(&mut argv);
            argv.push("-e".to_string());
            push_shell_command(&mut argv);
        }
        ExecStyle::StartDashDash => {
            argv.push("start".to_string());
            push_class(&mut argv);
            argv.push("--".to_string());
            push_shell_command(&mut argv);
        }
        ExecStyle::Bare => {
            push_class(&mut argv);
            push_shell_command(&mut argv);
        }
    }
    argv
}

/// Renders an argv as a single `Exec=` line for a `.desktop` file --
/// space-joined, with any argument containing whitespace
/// double-quoted (only the user's own command is ever likely to need
/// this; binary names and class strings never contain spaces).
pub fn exec_line(argv: &[String]) -> String {
    argv.iter()
        .map(|a| {
            if a.contains(' ') {
                format!("\"{a}\"")
            } else {
                a.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Resolves which terminal to use, in the plan's stated order: an
/// explicit config override, `xdg-terminal-exec` if it's ever present
/// (using omarchy's own invocation form directly, since it already
/// knows how to set a class and exec a command), `$TERMINAL`, then the
/// first terminal from the table found on `$PATH`.
pub fn resolve(config: &crate::config::Config) -> Option<ResolvedTerminal> {
    if let Some(name) = &config.terminal {
        if let Some(spec) = find_spec(name) {
            if crate::which::which(name).is_some() {
                return Some(ResolvedTerminal::Known(*spec));
            }
        }
    }
    if crate::which::which("xdg-terminal-exec").is_some() {
        return Some(ResolvedTerminal::XdgTerminalExec);
    }
    if let Ok(term_env) = std::env::var("TERMINAL") {
        if let Some(spec) = find_spec(&term_env) {
            if crate::which::which(&term_env).is_some() {
                return Some(ResolvedTerminal::Known(*spec));
            }
        }
    }
    TERMINALS
        .iter()
        .find(|t| crate::which::which(t.binary).is_some())
        .map(|s| ResolvedTerminal::Known(*s))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedTerminal {
    Known(TerminalSpec),
    XdgTerminalExec,
}

impl ResolvedTerminal {
    pub fn build_argv(&self, class: &str, command: &str) -> Vec<String> {
        match self {
            ResolvedTerminal::Known(spec) => build_argv(spec, class, command),
            ResolvedTerminal::XdgTerminalExec => {
                vec![
                    "xdg-terminal-exec".to_string(),
                    format!("--app-id={class}"),
                    "-e".to_string(),
                    "sh".to_string(),
                    "-c".to_string(),
                    command.to_string(),
                ]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec_for(binary: &str) -> TerminalSpec {
        *find_spec(binary).unwrap_or_else(|| panic!("{binary} missing from TERMINALS table"))
    }

    #[test]
    fn ghostty_uses_equals_class_and_dash_e() {
        let argv = build_argv(&spec_for("ghostty"), "pacpad-tui-float", "btop");
        assert_eq!(
            argv,
            vec![
                "ghostty",
                "--class=pacpad-tui-float",
                "-e",
                "sh",
                "-c",
                "btop"
            ]
        );
    }

    #[test]
    fn kitty_uses_spaced_class_and_dash_e() {
        let argv = build_argv(&spec_for("kitty"), "pacpad-tui-float", "btop");
        assert_eq!(
            argv,
            vec![
                "kitty",
                "--class",
                "pacpad-tui-float",
                "-e",
                "sh",
                "-c",
                "btop"
            ]
        );
    }

    #[test]
    fn wezterm_uses_start_class_dash_dash() {
        let argv = build_argv(&spec_for("wezterm"), "pacpad-tui-tile", "lazygit");
        assert_eq!(
            argv,
            vec![
                "wezterm",
                "start",
                "--class",
                "pacpad-tui-tile",
                "--",
                "sh",
                "-c",
                "lazygit"
            ]
        );
    }

    #[test]
    fn alacritty_uses_spaced_class_and_dash_e() {
        let argv = build_argv(&spec_for("alacritty"), "pacpad-tui-float", "htop");
        assert_eq!(
            argv,
            vec![
                "alacritty",
                "--class",
                "pacpad-tui-float",
                "-e",
                "sh",
                "-c",
                "htop"
            ]
        );
    }

    #[test]
    fn foot_uses_equals_app_id_with_no_dash_e() {
        let argv = build_argv(&spec_for("foot"), "pacpad-tui-float", "btop");
        assert_eq!(
            argv,
            vec!["foot", "--app-id=pacpad-tui-float", "sh", "-c", "btop"]
        );
    }

    #[test]
    fn compound_shell_commands_stay_a_single_argv_element() {
        // Exactly the omarchy placeholder example: semicolons and
        // quoting inside the user's command must survive as ONE
        // argument to `sh -c`, not get split on whitespace.
        let argv = build_argv(&spec_for("kitty"), "pacpad-tui-float", "dust; read -n 1 -s");
        assert_eq!(argv.last().unwrap(), "dust; read -n 1 -s");
        assert_eq!(argv[argv.len() - 2], "-c");
    }

    #[test]
    fn window_style_classes_match_omarchys_convention() {
        assert_eq!(WindowStyle::Float.class(), "pacpad-tui-float");
        assert_eq!(WindowStyle::Tile.class(), "pacpad-tui-tile");
    }

    #[test]
    fn exec_line_quotes_only_arguments_with_whitespace() {
        let argv = vec![
            "kitty".to_string(),
            "--class".to_string(),
            "pacpad-tui-float".to_string(),
            "-e".to_string(),
            "sh".to_string(),
            "-c".to_string(),
            "dust; read -n 1 -s".to_string(),
        ];
        assert_eq!(
            exec_line(&argv),
            r#"kitty --class pacpad-tui-float -e sh -c "dust; read -n 1 -s""#
        );
    }

    #[test]
    fn resolve_prefers_config_override_when_the_binary_actually_exists() {
        let cfg = crate::config::Config {
            browser: None,
            terminal: Some("kitty".to_string()),
            aur_helper: None,
        };
        // Only meaningful if kitty happens to be on this machine's
        // PATH; if not, resolve() correctly falls through instead,
        // which the next test covers explicitly with a fake name.
        if crate::which::which("kitty").is_some() {
            let resolved = resolve(&cfg).unwrap();
            assert_eq!(resolved.build_argv("pacpad-tui-float", "btop")[0], "kitty");
        }
    }

    #[test]
    fn resolve_ignores_a_config_override_for_a_binary_that_is_not_installed() {
        let cfg = crate::config::Config {
            browser: None,
            terminal: Some("pacpad-definitely-not-a-real-terminal".to_string()),
            aur_helper: None,
        };
        // Must fall through to detection rather than returning a
        // terminal that doesn't actually exist on this machine.
        let resolved = resolve(&cfg);
        if let Some(r) = resolved {
            assert_ne!(
                r.build_argv("pacpad-tui-float", "btop")[0],
                "pacpad-definitely-not-a-real-terminal"
            );
        }
    }
}
