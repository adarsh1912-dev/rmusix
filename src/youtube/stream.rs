//! yt-dlp format picking (subprocess backend).

use crate::errors::RmusixError;
use crate::models::{AudioQuality, AudioStreamInfo};

/// Pick the best audio-only format from yt-dlp `-J` JSON.
///
/// `formats_json` is either the full `-J` object (with `formats` array) or
/// the `formats` array itself. Prefers audio-only (`vcodec == "none"`,
/// `acodec != "none"` with a `url`), ranks by `abr` (falling back to `tbr`),
/// and selects min (Low) / median (Medium) / max (High).
pub fn select_audio_stream(
    formats_json: &serde_json::Value,
    quality: AudioQuality,
) -> Result<AudioStreamInfo, RmusixError> {
    let formats: &[serde_json::Value] = if let Some(arr) = formats_json.as_array() {
        arr
    } else if let Some(arr) = formats_json.get("formats").and_then(|f| f.as_array()) {
        arr
    } else {
        return Err(RmusixError::StreamResolution(
            "no audio formats found (total=0 audio=0)".to_string(),
        ));
    };
    select_from_formats(formats, quality)
}

fn bitrate_of(f: &serde_json::Value) -> f64 {
    for key in ["abr", "tbr"] {
        if let Some(n) = f.get(key).and_then(serde_json::Value::as_f64)
            && n > 0.0
        {
            return n;
        }
        // yt-dlp sometimes emits integers; as_f64 already covers those.
    }
    0.0
}

fn select_from_formats(
    formats: &[serde_json::Value],
    quality: AudioQuality,
) -> Result<AudioStreamInfo, RmusixError> {
    let total = formats.len();
    let mut audio: Vec<&serde_json::Value> = formats
        .iter()
        .filter(|f| {
            f.get("url")
                .and_then(|u| u.as_str())
                .is_some_and(|u| !u.is_empty())
        })
        .filter(|f| {
            !matches!(
                f.get("acodec").and_then(|v| v.as_str()),
                None | Some("none") | Some("")
            )
        })
        .collect();
    let audio_count_all_codec = audio.len();
    // Prefer audio-only streams; fall back to any audio-bearing format.
    let audio_only: Vec<&serde_json::Value> = audio
        .iter()
        .filter(|f| matches!(f.get("vcodec").and_then(|v| v.as_str()), Some("none")))
        .copied()
        .collect();
    if !audio_only.is_empty() {
        audio = audio_only;
    }
    if audio.is_empty() {
        return Err(RmusixError::StreamResolution(format!(
            "no audio formats found (total={total} audio={audio_count_all_codec})"
        )));
    }
    audio.sort_by(|a, b| {
        bitrate_of(a)
            .partial_cmp(&bitrate_of(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let picked = match quality {
        AudioQuality::Low => audio[0],
        AudioQuality::High => audio[audio.len() - 1],
        AudioQuality::Medium => audio[audio.len() / 2],
    };
    let url = picked
        .get("url")
        .and_then(|u| u.as_str())
        .unwrap_or_default()
        .to_string();
    let ext = picked
        .get("ext")
        .and_then(|e| e.as_str())
        .unwrap_or("webm")
        .to_string();
    let bitrate = bitrate_of(picked).round() as u32;
    Ok(AudioStreamInfo {
        url,
        mime: ext,
        bitrate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> serde_json::Value {
        serde_json::json!({
            "formats": [
                {"url": "http://x/low", "acodec": "opus", "vcodec": "none", "ext": "webm", "abr": 48.0},
                {"url": "http://x/med", "acodec": "opus", "vcodec": "none", "ext": "webm", "abr": 128.0},
                {"url": "http://x/high", "acodec": "opus", "vcodec": "none", "ext": "webm", "abr": 256.0},
                {"url": "http://x/av", "acodec": "aac", "vcodec": "avc1", "ext": "mp4", "tbr": 512.0}
            ]
        })
    }

    #[test]
    fn quality_ranking_min_mid_max() {
        assert_eq!(
            select_audio_stream(&fixture(), AudioQuality::Low)
                .expect("low")
                .url,
            "http://x/low"
        );
        assert_eq!(
            select_audio_stream(&fixture(), AudioQuality::Medium)
                .expect("med")
                .url,
            "http://x/med"
        );
        assert_eq!(
            select_audio_stream(&fixture(), AudioQuality::High)
                .expect("high")
                .url,
            "http://x/high"
        );
    }

    #[test]
    fn prefers_audio_only_over_muxed() {
        // High must be the best audio-only, not the muxed 512 tbr entry.
        let info = select_audio_stream(&fixture(), AudioQuality::High).expect("high");
        assert_eq!(info.mime, "webm");
    }

    #[test]
    fn accepts_bare_formats_array() {
        let arr = fixture()["formats"].clone();
        let info = select_audio_stream(&arr, AudioQuality::Low).expect("low");
        assert_eq!(info.bitrate, 48);
    }

    #[test]
    fn no_urls_reports_counts() {
        let v = serde_json::json!({"formats": [
            {"acodec": "none", "vcodec": "avc1", "ext": "mp4"},
            {"acodec": "opus", "vcodec": "none", "ext": "webm"}
        ]});
        let err = select_audio_stream(&v, AudioQuality::Medium).expect_err("must fail");
        match err {
            RmusixError::StreamResolution(msg) => {
                assert!(msg.contains("total=2"), "got {msg}");
                assert!(msg.contains("audio="), "got {msg}");
            }
            other => panic!("wrong variant: {other:?}"),
        }
    }
}
