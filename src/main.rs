// Foundation crate: stubs in app/ui/youtube/audio/queue are reserved for later
// agents and intentionally unused here.
#![allow(dead_code)]

mod app;
mod audio;
mod config;
mod errors;
mod models;
mod queue;
mod ui;
mod youtube;

use std::io::stdout;
use std::sync::Arc;

use clap::Parser;
use crossterm::execute;
use crossterm::terminal::disable_raw_mode;

use app::App;
use config::AppConfig;
use youtube::RustyYtdlClient;

/// Terminal YouTube music player (foundation).
#[derive(Parser, Debug)]
#[command(name = "rmusix", version, about = "terminal YouTube music player")]
struct Cli {
    /// Initial search query.
    #[arg(value_name = "QUERY")]
    query: Option<String>,
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), crossterm::terminal::LeaveAlternateScreen);
}

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_hook(info);
    }));
}

#[tokio::main]
async fn main() -> Result<(), errors::RmusixError> {
    install_panic_hook();

    let cli = Cli::parse();

    let config = match AppConfig::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("warning: using default config ({e})");
            AppConfig::default()
        }
    };

    // Spawn Ctrl-C handler that always restores the terminal first.
    tokio::spawn(async {
        if tokio::signal::ctrl_c().await.is_ok() {
            restore_terminal();
            std::process::exit(130);
        }
    });

    let state_query = cli.query.clone();

    let youtube = Arc::new(RustyYtdlClient::new());
    let app = App::new(config, youtube, state_query);
    app.run().await?;

    Ok(())
}
