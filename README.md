# pacpad

An omarchy-style terminal app manager for Arch Linux — browse, search, and manage
packages (repo + AUR), plus create and remove Chrome webapp and TUI-app launchers,
all from one ratatui TUI.

## Features

- **Installed / Search / Apps / Updates** tabs — installed packages listed by
  default, fuzzy search across every configured repo, and a dedicated tab for
  pacpad-managed launchers (the one thing omarchy's own scripts don't give you
  a way to browse or remove).
- **Package actions** — install, remove, reinstall, mark explicit/dependency,
  system update, list files, open upstream URL — every one shows the literal
  command in a confirm modal before running anything, with protected packages
  (`pacman`, `glibc`, the `base` group, ...) requiring a typed confirmation.
- **AUR-aware** — detects `yay`/`paru` and delegates automatically for foreign
  packages; a configured helper is respected if it's actually installed.
- **Webapp & TUI-app launchers** — `.desktop` entries pacpad creates (and only
  pacpad's own entries) can be listed and removed again from the Apps tab.
  Browser and terminal resolution both fall back sanely across whatever's
  actually installed on your system.
- **No terminal handover** — package actions run on a pseudo-terminal pacpad
  itself owns, rendered live inside pacpad's own screen. `sudo`'s password
  prompt, pacman's provider/conflict prompts, and yay's PKGBUILD review all
  still work exactly as they would in a real terminal — you just never leave
  pacpad to see them.
- Direct pacman database parsing (no `libalpm` FFI), so pacpad keeps working
  across pacman upgrades that bump its soname.

## Usage

```
pacpad                                    # launch the TUI
pacpad search <query>                     # headless search
pacpad webapp add <name> <url>            # create a webapp launcher
pacpad tui add <name> <command>           # create a TUI-app launcher
pacpad --dry-run <...>                    # print the command, run nothing
```

Inside the TUI: `1`-`4` or `Tab` to switch views, `F1` for the full keybinding
reference, `q`/`Ctrl+C` to quit.

## Building

```
cargo build --release
```

Requires a Rust toolchain (edition 2021, `rust-version = "1.82"` per `Cargo.toml`)
and a recent `pacman`.

## Status

Under active development. Packaging (AUR `PKGBUILD`, CI, release binaries) is
still to come.

## License

Apache-2.0 — see [LICENSE](LICENSE).
