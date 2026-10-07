// Glue (Agent 5, testing & integration): expose modules as a library so that
// `tests/` integration tests can import them. No src/ logic changed.
pub mod app;
pub mod audio;
pub mod config;
pub mod errors;
pub mod models;
pub mod queue;
pub mod ui;
pub mod youtube;

use clap::Parser;

/// Terminal YouTube music player CLI (shared with `main.rs` for testability).
#[derive(Parser, Debug)]
#[command(name = "rmusix", version, about = "terminal YouTube music player")]
pub struct Cli {
    /// Initial search query.
    #[arg(value_name = "QUERY")]
    pub query: Option<String>,
}
