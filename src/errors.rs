use thiserror::Error;

/// Foundation error type. No `unwrap` / `expect` in `src/` — propagate via `Result<T, RmusixError>`.
/// Binary hints: "yt-dlp not found (pacman -S yt-dlp)", "mpv not found (pacman -S mpv)".
#[derive(Error, Debug)]
pub enum RmusixError {
    #[error("network error: {0}")]
    Network(String),

    #[error("search error: {0}")]
    Search(String),

    #[error("stream resolution error: {0}")]
    StreamResolution(String),

    #[error("audio error: {0}")]
    Audio(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("config error: {0}")]
    Config(String),
}

impl From<reqwest::Error> for RmusixError {
    fn from(e: reqwest::Error) -> Self {
        Self::Network(e.to_string())
    }
}

impl From<std::io::Error> for RmusixError {
    fn from(e: std::io::Error) -> Self {
        Self::Config(e.to_string())
    }
}

impl From<toml::de::Error> for RmusixError {
    fn from(e: toml::de::Error) -> Self {
        Self::Config(e.to_string())
    }
}

impl From<toml::ser::Error> for RmusixError {
    fn from(e: toml::ser::Error) -> Self {
        Self::Config(e.to_string())
    }
}

impl From<serde_json::Error> for RmusixError {
    fn from(e: serde_json::Error) -> Self {
        Self::Search(e.to_string())
    }
}
