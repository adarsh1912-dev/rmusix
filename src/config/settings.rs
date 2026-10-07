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
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            volume: 0.5,
            search_limit: 10,
            quality: AudioQuality::Medium,
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
    pub fn load() -> Result<Self, RmusixError> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(&path)
            .map_err(|e| RmusixError::Config(format!("failed to read {}: {e}", path.display())))?;
        let cfg: Self = toml::from_str(&contents)?;
        Ok(cfg)
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
}
