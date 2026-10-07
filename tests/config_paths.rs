//! Config paths + env overrides: no binaries spawned.

use rmusix::config::AppConfig;

#[test]
fn defaults_point_at_path_bins() {
    let cfg = AppConfig::default();
    assert_eq!(cfg.ytdlp_path, "yt-dlp");
    assert_eq!(cfg.mpv_path, "mpv");
}

#[test]
fn env_overrides_paths() {
    let dir = tempfile::tempdir().unwrap();
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", dir.path());
        std::env::set_var("RMUSIX_YTDLP", "/custom/yt-dlp");
        std::env::set_var("RMUSIX_MPV", "/custom/mpv");
    }
    let cfg = AppConfig::load().expect("load with env overrides");
    assert_eq!(cfg.ytdlp_path, "/custom/yt-dlp");
    assert_eq!(cfg.mpv_path, "/custom/mpv");
    unsafe {
        std::env::remove_var("RMUSIX_YTDLP");
        std::env::remove_var("RMUSIX_MPV");
        std::env::remove_var("XDG_CONFIG_HOME");
    }
    // Missing binaries resolve to Config errors with install hints.
    let cfg = AppConfig {
        ytdlp_path: "/nonexistent/yt-dlp-xyz".into(),
        mpv_path: "/nonexistent-mpv-xyz".into(),
        ..AppConfig::default()
    };
    assert!(
        cfg.resolve_ytdlp()
            .unwrap_err()
            .to_string()
            .contains("yt-dlp not found")
    );
    assert!(
        cfg.resolve_mpv()
            .unwrap_err()
            .to_string()
            .contains("mpv not found")
    );
}

#[test]
fn toml_persists_binary_paths() {
    let cfg = AppConfig {
        ytdlp_path: "/usr/bin/yt-dlp".into(),
        mpv_path: "/usr/bin/mpv".into(),
        ..AppConfig::default()
    };
    let s = toml::to_string_pretty(&cfg).unwrap();
    assert!(s.contains("ytdlp_path") && s.contains("mpv_path"));
    let back: AppConfig = toml::from_str(&s).unwrap();
    assert_eq!(back, cfg);
}
