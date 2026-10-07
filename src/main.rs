// Glue (Agent 4): wire config + yt-dlp/mpv backends into the TUI.
// Owns startup only: config load, binary checks, client + socket construction.
// Run loop / playback live in `app`, backends in `youtube` / `audio`.
use rmusix::Cli;

use clap::Parser;
use crossterm::execute;
use crossterm::terminal::disable_raw_mode;

use rmusix::app::{App, mpv_socket_path};
use rmusix::config::AppConfig;
use rmusix::youtube::YtDlpClient;

use std::io::stdout;
use std::path::PathBuf;
use std::sync::Arc;

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), crossterm::terminal::LeaveAlternateScreen);
}

fn remove_socket(sock: &PathBuf) {
    let _ = std::fs::remove_file(sock);
}

/// Best-effort kill of the mpv child tied to `sock`.
///
/// The child is spawned as `mpv --input-ipc-server=<sock> ...`, so
/// `pkill -f <sock>` targets exactly our instance. The graceful `q` /
/// Ctrl-C-key path in `App::run` already sends `Stop`, drops the engine
/// (`kill_on_drop(true)`) and removes the socket; this is only a fallback
/// for the async-signal path where destructors are skipped via `exit()`.
fn kill_mpv_for_socket(sock: &PathBuf) {
    let needle = sock.display().to_string();
    // `pkill` may be missing; ignore all failures (graceful path covers it).
    let _ = std::process::Command::new("pkill")
        .args(["-f", &needle])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

fn install_panic_hook(sock: PathBuf) {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        remove_socket(&sock);
        restore_terminal();
        default_hook(info);
    }));
}

/// Missing-binary error is printed after restoring the terminal so it is
/// visible (not trapped in the alternate screen); exit code 2 per spec.
fn exit_missing_binary(err: impl std::fmt::Display) -> ! {
    restore_terminal();
    eprintln!("error: {err}");
    eprintln!("install missing binaries and retry (pacman -S yt-dlp mpv)");
    std::process::exit(2);
}

#[tokio::main]
async fn main() -> Result<(), rmusix::errors::RmusixError> {
    // Socket path first: panic hook + Ctrl-C fallback need it even if
    // config load fails. `$XDG_RUNTIME_DIR` else `/tmp/rmusix-$UID`.
    let sock = mpv_socket_path();
    install_panic_hook(sock.clone());

    let cli = Cli::parse();

    // `AppConfig::load` already applies `RMUSIX_YTDLP` / `RMUSIX_MPV` env
    // overrides over file values (config < env).
    let config = match AppConfig::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("warning: using default config ({e})");
            AppConfig::default()
        }
    };

    // Startup `which::which` checks (via `resolve_*`); TUI-friendly error +
    // terminal restore + exit 2 when a binary is missing.
    if let Err(e) = config.resolve_ytdlp() {
        exit_missing_binary(e);
    }
    if let Err(e) = config.resolve_mpv() {
        exit_missing_binary(e);
    }

    // Fallback Ctrl-C (SIGINT-as-signal) handler. Keyboard Ctrl-C in raw
    // mode arrives as a key event handled gracefully in `App::run`; this
    // covers external `kill -INT` and pre/post-TUI signals.
    {
        let sock = sock.clone();
        tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                remove_socket(&sock);
                restore_terminal();
                kill_mpv_for_socket(&sock);
                std::process::exit(130);
            }
        });
    }

    let state_query = cli.query.clone();

    // Production YouTube backend: yt-dlp subprocess client. Binary path
    // comes from config (already env-overridden) and was validated above.
    let youtube = Arc::new(YtDlpClient::with_bin(config.ytdlp_path.clone()));
    // `MpvEngine` itself is spawned inside `App::run` from
    // `config.mpv_path` + `mpv_socket_path()` (same `sock` as above), so
    // `Play(yt-dlp URL, HLS-capable)` flows to `mpv loadfile replace`.
    let app = App::new(config, youtube, state_query);
    let res = app.run().await;

    // Foreground guarantee: no stale socket even if the background engine
    // task was aborted on shutdown (it also removes on drop).
    remove_socket(&sock);
    res
}
