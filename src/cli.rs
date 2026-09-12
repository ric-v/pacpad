//! Argument parsing and dispatch. With no subcommand, launches the
//! TUI (the default experience); every subcommand is a headless
//! escape hatch so pacpad stays scriptable and `.desktop` files can
//! call it directly -- the launcher subcommands in particular are
//! what let a `pacpad tui add ...` invocation install pacpad's own
//! launcher (see the README/Phase 6 smoke test).

use std::path::{Path, PathBuf};
use std::process::{Command as OsCommand, Stdio};
use std::time::Duration;

use clap::{Parser, Subcommand};
use crossterm::event::{self, Event, KeyEventKind};

use crate::alpm::{self, conf::DEFAULT_CONF_PATH, PacmanConfig};
use crate::app::state::AppsAction;
use crate::app::{self, AppState};
use crate::config::Config;
use crate::launcher::entry::ManagedEntry;
use crate::launcher::terminal::WindowStyle;
use crate::launcher::{tuiapp, webapp};
use crate::term::TerminalGuard;

#[derive(Parser)]
#[command(name = "pacpad", version, about = "A terminal app manager for Arch")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Path to pacman.conf, for testing against a fixture without root.
    #[arg(long, default_value = DEFAULT_CONF_PATH, global = true)]
    pub pacman_conf: PathBuf,

    /// Print the command any write action would run, and run nothing.
    /// This is what makes the write path (`txn`) testable without
    /// root: every install/remove/mark/update prints its argv and
    /// waits for a keypress exactly like a real run, but never
    /// spawns anything. Also gates Apps-tab launcher writes in the
    /// TUI (creating/updating/removing a `.desktop` file) -- launching
    /// an already-created app is never gated, since opening a window
    /// changes nothing pacpad would need to undo.
    #[arg(long, global = true)]
    pub dry_run: bool,
}

#[derive(Subcommand)]
pub enum Command {
    /// Search installed + repo packages by name or description.
    Search {
        query: String,
        /// Show every match instead of the top few.
        #[arg(long)]
        all: bool,
    },
    /// Manage pacpad-created webapp launchers.
    Webapp {
        #[command(subcommand)]
        action: WebappCommand,
    },
    /// Manage pacpad-created TUI-app launchers.
    Tui {
        #[command(subcommand)]
        action: TuiCommand,
    },
}

#[derive(Subcommand)]
pub enum WebappCommand {
    /// Create a Chrome app-mode launcher.
    Add {
        name: String,
        url: String,
        /// A URL or local path; omit to auto-fetch a favicon.
        #[arg(long)]
        icon: Option<String>,
    },
    Remove {
        name: String,
    },
    List,
}

#[derive(Subcommand)]
pub enum TuiCommand {
    /// Create a terminal launcher running `command`.
    Add {
        name: String,
        command: String,
        /// A local path to an icon file.
        #[arg(long)]
        icon: Option<String>,
        #[arg(long, conflicts_with = "tile")]
        float: bool,
        #[arg(long)]
        tile: bool,
    },
    Remove {
        name: String,
    },
    List,
}

pub fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Some(Command::Search { query, all }) => run_search(&cli.pacman_conf, &query, all),
        Some(Command::Webapp { action }) => run_webapp_command(action, cli.dry_run),
        Some(Command::Tui { action }) => run_tui_command(action, cli.dry_run),
        None => run_tui(&cli.pacman_conf, cli.dry_run),
    }
}

fn run_tui(pacman_conf: &Path, dry_run: bool) -> anyhow::Result<()> {
    let cfg = PacmanConfig::load(pacman_conf)?;
    let cache_path = cache_dir().join("index.bin");
    let index = alpm::load_cached(&cfg, &cache_path)?;

    // Loaded once and shared: the AUR helper decision and the
    // launcher (browser/terminal) resolution chains all read the same
    // config file, and there's no reason to re-parse it twice a frame
    // apart.
    let launcher_config = Config::load();
    let aur_helper = crate::aur::helper::detect(&launcher_config);
    let mut state = AppState::new(index, aur_helper);
    // Only the real interactive run gets the startup splash -- see the
    // doc comment on `AppState::splash_until_tick` for why this isn't
    // the constructor's default.
    state.splash_until_tick = app::state::SPLASH_TICKS;

    let apps_dir = crate::xdg::applications_dir();
    state.apps.reload(&apps_dir);

    let mut guard = TerminalGuard::enter()?;

    // The embedded pty for whatever package transaction is currently
    // running, if any -- see `txn::embed`'s module docs for why this
    // (a live process handle) is owned right here by the main loop
    // rather than inside `AppState`, which only ever holds the plain
    // `RunningView` snapshot taken from it each iteration.
    let mut pty: Option<crate::txn::PtySession> = None;

    while !state.should_quit {
        state.advance_tick();

        if let Some(session) = pty.as_mut() {
            session.poll_exit();
            state.running = Some(session.snapshot());
        }

        guard
            .terminal
            .draw(|frame| crate::ui::render(frame, &state))?;

        // 90ms rather than the read path's earlier 200ms: still cheap
        // (ratatui only repaints cells that actually changed), but
        // fast enough that the startup splash, the result-banner fade,
        // and a running command's own output all read as animation
        // rather than a slideshow.
        if event::poll(Duration::from_millis(90))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if let Some(session) = pty.as_mut() {
                        // A running (or just-finished) command owns
                        // every keystroke -- there is no `Action` for
                        // this, because the command, not pacpad, is
                        // what should interpret it. See `encode_key`'s
                        // own docs for exactly what it covers.
                        if session.poll_exit().is_some() {
                            finish_pty(&mut pty, &mut state, &cfg, &cache_path)?;
                        } else {
                            session.write_input(&crate::txn::encode_key(&key));
                        }
                    } else if let Some(action) = app::input::map_key(&state, key) {
                        app::actions::apply(&mut state, action);
                    }
                }
                Event::Resize(cols, rows) => {
                    if let Some(session) = pty.as_mut() {
                        let (content_rows, content_cols) =
                            crate::ui::running_content_dims(cols, rows);
                        session.resize(content_rows, content_cols);
                    }
                }
                _ => {}
            }
        }

        // The only I/O in this whole chain outside the pty itself:
        // `apply()` above only ever populates `state.execute`/
        // `state.apps_action` (plain data) when the user confirms a
        // modal, requests a non-destructive package action, or
        // submits/removes/launches an Apps-tab entry; actually running
        // any of it happens here.
        if let Some(command) = state.execute.take() {
            if dry_run {
                state.set_last_result(format!("[dry-run] would run: {}", command.display));
            } else {
                let (cols, rows) = crossterm::terminal::size()?;
                let (content_rows, content_cols) = crate::ui::running_content_dims(cols, rows);
                pty = Some(crate::txn::PtySession::spawn(
                    &command,
                    content_rows,
                    content_cols,
                )?);
            }
        }

        if let Some(action) = state.apps_action.take() {
            let result = run_apps_action(action, &apps_dir, &launcher_config, dry_run);
            state.set_last_result(result);
            state.apps.reload(&apps_dir);
        }
    }

    Ok(())
}

/// Tears down a just-finished pty session: records its outcome in the
/// result banner the same way a completed dry-run or Apps-tab action
/// does, and re-derives the package index (a package transaction is
/// the one thing that can actually change what's installed).
fn finish_pty(
    pty: &mut Option<crate::txn::PtySession>,
    state: &mut AppState,
    cfg: &PacmanConfig,
    cache_path: &Path,
) -> anyhow::Result<()> {
    let success = state.running.as_ref().and_then(|v| v.finished);
    state.set_last_result(match success {
        Some(true) => "done".to_string(),
        Some(false) => "command exited with a non-zero status".to_string(),
        None => "done".to_string(),
    });
    *pty = None;
    state.running = None;

    // `load_cached` fingerprints every source file's mtime, so a run
    // that changed nothing (a cancelled/failed transaction) costs
    // nothing extra -- it just reuses the warm cache.
    let index = alpm::load_cached(cfg, cache_path)?;
    state.reload(index);
    Ok(())
}

/// Executes one Apps-tab action and returns a short status line for
/// `state.last_result`. `--dry-run` short-circuits every branch that
/// would write or delete a `.desktop` file (matching the package write
/// path's own dry-run contract); launching is exempt, per this
/// module's own doc comment on [`Cli::dry_run`].
fn run_apps_action(action: AppsAction, apps_dir: &Path, config: &Config, dry_run: bool) -> String {
    match action {
        AppsAction::CreateWebapp(new) => {
            if dry_run {
                return format!("[dry-run] would create webapp {:?}", new.name);
            }
            match webapp::add(apps_dir, &new, config) {
                Ok(_) => format!("created webapp {:?}", new.name),
                Err(e) => format!("error creating webapp: {e}"),
            }
        }
        AppsAction::UpdateWebapp(path, new) => {
            if dry_run {
                return format!("[dry-run] would update webapp {:?}", new.name);
            }
            match webapp::update(&path, &new, config) {
                Ok(()) => format!("updated webapp {:?}", new.name),
                Err(e) => format!("error updating webapp: {e}"),
            }
        }
        AppsAction::CreateTui(new) => {
            if dry_run {
                return format!("[dry-run] would create tui app {:?}", new.name);
            }
            match tuiapp::add(apps_dir, &new, config) {
                Ok(_) => format!("created tui app {:?}", new.name),
                Err(e) => format!("error creating tui app: {e}"),
            }
        }
        AppsAction::UpdateTui(path, new) => {
            if dry_run {
                return format!("[dry-run] would update tui app {:?}", new.name);
            }
            match tuiapp::update(&path, &new, config) {
                Ok(()) => format!("updated tui app {:?}", new.name),
                Err(e) => format!("error updating tui app: {e}"),
            }
        }
        AppsAction::RemoveWebapp(path) => {
            if dry_run {
                return "[dry-run] would remove webapp".to_string();
            }
            match webapp::remove(&path) {
                Ok(()) => "removed webapp".to_string(),
                Err(e) => format!("error removing webapp: {e}"),
            }
        }
        AppsAction::RemoveTui(path) => {
            if dry_run {
                return "[dry-run] would remove tui app".to_string();
            }
            match tuiapp::remove(&path) {
                Ok(()) => "removed tui app".to_string(),
                Err(e) => format!("error removing tui app: {e}"),
            }
        }
        AppsAction::Launch(exec) => {
            launch_detached(&exec);
            "launched".to_string()
        }
    }
}

/// Spawns `exec` (a shell-line `Exec=` string, exactly as stored)
/// through `sh -c`, fully detached: no wait, stdio redirected to
/// `/dev/null` so the launched app's own output never interleaves
/// with pacpad's live TUI frame. A launch failure is swallowed rather
/// than surfaced as an error -- a bad launcher (moved binary, missing
/// terminal) is discoverable by trying it, not something that should
/// crash or block the TUI.
fn launch_detached(exec: &str) {
    let _ = OsCommand::new("sh")
        .arg("-c")
        .arg(exec)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

fn run_search(pacman_conf: &Path, query: &str, all: bool) -> anyhow::Result<()> {
    let cfg = PacmanConfig::load(pacman_conf)?;
    let cache_path = cache_dir().join("index.bin");
    let index = alpm::load_cached(&cfg, &cache_path)?;

    let hits = alpm::search(&index.entries, query);
    let limit = if all { hits.len() } else { 20.min(hits.len()) };

    if hits.is_empty() {
        println!("no matches for {query:?}");
        return Ok(());
    }

    let suffix = if all || hits.len() <= limit {
        String::new()
    } else {
        format!(" (showing {limit})")
    };
    println!("{} match(es){}:", hits.len(), suffix);
    for hit in &hits[..limit] {
        let e = hit.entry;
        let status = if e.is_installed() {
            if e.is_foreign() {
                "installed, foreign"
            } else {
                "installed"
            }
        } else {
            "not installed"
        };
        let repo = e.repo().unwrap_or("-");
        let version = e.version().unwrap_or("?");
        println!("  {:<24} {:<14} [{:<8}] {}", e.name, version, repo, status);
        if hit.matched_description {
            println!("      ↳ {}", e.description());
        }
    }

    let counts = index.counts();
    eprintln!(
        "\n{} installed ({} explicit, {} dependency, {} foreign, {} orphan)",
        counts.total, counts.explicit, counts.dependency, counts.foreign, counts.orphan
    );

    Ok(())
}

fn run_webapp_command(action: WebappCommand, dry_run: bool) -> anyhow::Result<()> {
    let apps_dir = crate::xdg::applications_dir();
    let config = Config::load();

    match action {
        WebappCommand::Add { name, url, icon } => {
            let new = webapp::NewWebapp {
                name: name.clone(),
                url: url.clone(),
                icon,
            };
            if dry_run {
                println!("[dry-run] would create webapp {name:?} ({url})");
                return Ok(());
            }
            let path = webapp::add(&apps_dir, &new, &config)?;
            println!("created {}", path.display());
        }
        WebappCommand::Remove { name } => {
            let entries = webapp::list(&apps_dir);
            let entry = find_by_name(&entries, &name)
                .ok_or_else(|| anyhow::anyhow!("no pacpad-managed webapp named {name:?}"))?;
            if dry_run {
                println!("[dry-run] would remove {}", entry.path.display());
                return Ok(());
            }
            webapp::remove(&entry.path)?;
            println!("removed {name:?}");
        }
        WebappCommand::List => print_entries(&webapp::list(&apps_dir)),
    }
    Ok(())
}

fn run_tui_command(action: TuiCommand, dry_run: bool) -> anyhow::Result<()> {
    let apps_dir = crate::xdg::applications_dir();
    let config = Config::load();

    match action {
        TuiCommand::Add {
            name,
            command,
            icon,
            float: _,
            tile,
        } => {
            let style = if tile {
                WindowStyle::Tile
            } else {
                WindowStyle::Float
            };
            let new = tuiapp::NewTuiApp {
                name: name.clone(),
                command: command.clone(),
                style,
                icon,
            };
            if dry_run {
                println!("[dry-run] would create tui app {name:?} ({command})");
                return Ok(());
            }
            let path = tuiapp::add(&apps_dir, &new, &config)?;
            println!("created {}", path.display());
        }
        TuiCommand::Remove { name } => {
            let entries = tuiapp::list(&apps_dir);
            let entry = find_by_name(&entries, &name)
                .ok_or_else(|| anyhow::anyhow!("no pacpad-managed tui app named {name:?}"))?;
            if dry_run {
                println!("[dry-run] would remove {}", entry.path.display());
                return Ok(());
            }
            tuiapp::remove(&entry.path)?;
            println!("removed {name:?}");
        }
        TuiCommand::List => print_entries(&tuiapp::list(&apps_dir)),
    }
    Ok(())
}

fn find_by_name<'a>(entries: &'a [ManagedEntry], name: &str) -> Option<&'a ManagedEntry> {
    entries.iter().find(|e| e.name == name)
}

fn print_entries(entries: &[ManagedEntry]) {
    if entries.is_empty() {
        println!("(none)");
        return;
    }
    for e in entries {
        let target = e.url.as_deref().or(e.command.as_deref()).unwrap_or("");
        println!("  {:<24} {}", e.name, target);
    }
}

fn cache_dir() -> PathBuf {
    crate::xdg::cache_home().join("pacpad")
}
