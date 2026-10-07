use crate::errors::RmusixError;
use crate::models::AudioQuality;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Application configuration, persisted at `$CONFIG_DIR/rmusix/config.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    /// Playback volume in `0.0..=1.0`.
    pub volume: f32,
    /// Max search results per query.
    pub search_limit: usize,
    /// Preferred audio quality.
    pub quality: AudioQuality,
    /// Path to the yt-dlp binary.
    #[serde(default = "default_ytdlp_path")]
    pub ytdlp_path: String,
    /// Path to the mpv binary.
    #[serde(default = "default_mpv_path")]
    pub mpv_path: String,
}

fn default_ytdlp_path() -> String {
    "yt-dlp".to_string()
}

fn default_mpv_path() -> String {
    "mpv".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            volume: 0.5,
            search_limit: 10,
            quality: AudioQuality::Medium,
            ytdlp_path: default_ytdlp_path(),
            mpv_path: default_mpv_path(),
        }
    }
}

impl AppConfig {
    fn config_path() -> Result<PathBuf, RmusixError> {
        let base = dirs::config_dir().ok_or_else(|| {
            RmusixError::Config("could not determine config directory".to_string())
        })?;
        Ok(base.join("rmusix").join("config.toml"))
    }

    /// Load config from disk, falling back to [`AppConfig::default`] when missing.
    /// `RMUSIX_YTDLP` / `RMUSIX_MPV` env vars override file values.
    pub fn load() -> Result<Self, RmusixError> {
        let path = Self::config_path()?;
        let mut cfg = if !path.exists() {
            Self::default()
        } else {
            let contents = std::fs::read_to_string(&path).map_err(|e| {
                RmusixError::Config(format!("failed to read {}: {e}", path.display()))
            })?;
            toml::from_str(&contents)?
        };
        Self::apply_env_overrides(&mut cfg);
        Ok(cfg)
    }

    fn apply_env_overrides(cfg: &mut Self) {
        if let Ok(v) = std::env::var("RMUSIX_YTDLP")
            && !v.trim().is_empty()
        {
            cfg.ytdlp_path = v;
        }
        if let Ok(v) = std::env::var("RMUSIX_MPV")
            && !v.trim().is_empty()
        {
            cfg.mpv_path = v;
        }
    }

    /// Resolve `yt-dlp` via [`which::which`], returning a config error with install hint.
    pub fn resolve_ytdlp(&self) -> Result<std::path::PathBuf, RmusixError> {
        which::which(&self.ytdlp_path)
            .map_err(|_| RmusixError::Config("yt-dlp not found (pacman -S yt-dlp)".to_string()))
    }

    /// Resolve `mpv` via [`which::which`], returning a config error with install hint.
    pub fn resolve_mpv(&self) -> Result<std::path::PathBuf, RmusixError> {
        which::which(&self.mpv_path)
            .map_err(|_| RmusixError::Config("mpv not found (pacman -S mpv)".to_string()))
    }

    /// Save config to disk, creating parent directories as needed.
    pub fn save(&self) -> Result<(), RmusixError> {
        let path = Self::config_path()?;
        let parent = path
            .parent()
            .ok_or_else(|| RmusixError::Config("invalid config path".to_string()))?;
        std::fs::create_dir_all(parent).map_err(|e| {
            RmusixError::Config(format!("failed to create {}: {e}", parent.display()))
        })?;
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(&path, contents)
            .map_err(|e| RmusixError::Config(format!("failed to write {}: {e}", path.display())))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let cfg = AppConfig::default();
        assert!((0.0..=1.0).contains(&cfg.volume));
        assert!(cfg.search_limit > 0);
        assert_eq!(cfg.quality, AudioQuality::Medium);
    }

    #[test]
    fn default_serializes_to_toml() {
        let cfg = AppConfig::default();
        let s = toml::to_string_pretty(&cfg).expect("serialize default config");
        assert!(s.contains("volume"));
        assert!(s.contains("search_limit"));
        assert!(s.contains("quality"));
        let back: AppConfig = toml::from_str(&s).expect("deserialize default config");
        assert_eq!(cfg, back);
    }

    #[test]
    fn defaults_include_binary_paths() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.ytdlp_path, "yt-dlp");
        assert_eq!(cfg.mpv_path, "mpv");
    }

    #[test]
    fn env_overrides_paths() {
        // SAFETY: test-only, single-threaded mutation of process env.
        unsafe {
            std::env::set_var("RMUSIX_YTDLP", "/custom/yt-dlp");
            std::env::set_var("RMUSIX_MPV", "/custom/mpv");
        }
        let mut cfg = AppConfig::default();
        AppConfig::apply_env_overrides(&mut cfg);
        assert_eq!(cfg.ytdlp_path, "/custom/yt-dlp");
        assert_eq!(cfg.mpv_path, "/custom/mpv");
        unsafe {
            std::env::remove_var("RMUSIX_YTDLP");
            std::env::remove_var("RMUSIX_MPV");
        }
    }
}
