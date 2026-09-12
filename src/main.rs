mod alpm;
mod app;
mod aur;
mod cli;
mod config;
mod launcher;
mod term;
mod time;
mod txn;
mod ui;
mod which;
mod xdg;

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    cli::run(cli)
}
