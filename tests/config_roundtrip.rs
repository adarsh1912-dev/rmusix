use rmusix::config::AppConfig;
use rmusix::models::AudioQuality;

#[test]
fn toml_roundtrip() {
    let cfg = AppConfig {
        volume: 0.7,
        search_limit: 25,
        quality: AudioQuality::High,
    };
    let s = toml::to_string_pretty(&cfg).expect("serialize");
    assert!(s.contains("volume"));
    assert!(s.contains("search_limit"));
    assert!(s.contains("quality"));
    let back: AppConfig = toml::from_str(&s).expect("deserialize");
    assert_eq!(cfg, back);
}

#[test]
fn defaults_roundtrip() {
    let cfg = AppConfig::default();
    let s = toml::to_string_pretty(&cfg).expect("serialize default");
    let back: AppConfig = toml::from_str(&s).expect("deserialize default");
    assert_eq!(cfg, back);
    assert!((0.0..=1.0).contains(&back.volume));
}

#[test]
fn quality_variants_roundtrip() {
    for q in [AudioQuality::Low, AudioQuality::Medium, AudioQuality::High] {
        let cfg = AppConfig {
            quality: q,
            ..AppConfig::default()
        };
        let s = toml::to_string_pretty(&cfg).unwrap();
        let back: AppConfig = toml::from_str(&s).unwrap();
        assert_eq!(back.quality, q);
    }
}
