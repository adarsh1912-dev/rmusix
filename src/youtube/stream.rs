//! Audio-only format picking for resolved streams.
//!
//! Pure selection over `rusty_ytdl` formats; no files, no subprocesses.

use crate::errors::RmusixError;
use crate::models::{AudioQuality, AudioStreamInfo};

/// Effective bitrate for ranking (prefers explicit audio bitrate).
fn effective_bitrate(format: &rusty_ytdl::VideoFormat) -> u64 {
    format.audio_bitrate.unwrap_or(format.bitrate)
}

/// True for direct audio-only candidates with a usable URL.
fn is_direct_audio_only(format: &rusty_ytdl::VideoFormat) -> bool {
    format.has_audio
        && !format.has_video
        && !format.url.trim().is_empty()
        && !format.is_hls
        && !format.is_dash_mpd
        && !format.is_live
}

/// True for any audio-only candidate with a usable URL (fallback pool).
fn is_any_audio_only(format: &rusty_ytdl::VideoFormat) -> bool {
    format.has_audio && !format.has_video && !format.url.trim().is_empty()
}

/// Pick an audio-only format for `quality`; `None` when no candidate exists.
///
/// - `Low` → lowest effective bitrate
/// - `High` → highest effective bitrate
/// - `Medium` → median effective bitrate (upper-middle on even counts)
///
/// Prefers direct (non-HLS/DASH/live) audio; falls back to any audio-only
/// URL when no direct candidate exists. Never panics.
pub fn pick_audio_format(
    formats: &[rusty_ytdl::VideoFormat],
    quality: AudioQuality,
) -> Option<rusty_ytdl::VideoFormat> {
    let mut direct: Vec<&rusty_ytdl::VideoFormat> =
        formats.iter().filter(|f| is_direct_audio_only(f)).collect();
    if direct.is_empty() {
        direct = formats.iter().filter(|f| is_any_audio_only(f)).collect();
    }
    if direct.is_empty() {
        return None;
    }
    direct.sort_by_key(|f| effective_bitrate(f));
    let index = match quality {
        AudioQuality::Low => 0,
        AudioQuality::High => direct.len().saturating_sub(1),
        AudioQuality::Medium => direct.len() / 2,
    };
    direct.get(index).map(|f| (*f).clone())
}

/// Build [`AudioStreamInfo`] from a chosen format; errors on blank URLs.
pub fn audio_info_from_format(
    format: &rusty_ytdl::VideoFormat,
) -> Result<AudioStreamInfo, RmusixError> {
    if format.url.trim().is_empty() {
        return Err(RmusixError::StreamResolution(
            "chosen audio format has no URL".to_string(),
        ));
    }
    let mime = mime_string(format);
    let bitrate = u32::try_from(effective_bitrate(format)).unwrap_or(u32::MAX);
    Ok(AudioStreamInfo {
        url: format.url.clone(),
        mime,
        bitrate,
    })
}

/// Pick + convert in one step; errors when no audio-only format exists.
pub fn select_audio_stream(
    formats: &[rusty_ytdl::VideoFormat],
    quality: AudioQuality,
) -> Result<AudioStreamInfo, RmusixError> {
    let chosen = pick_audio_format(formats, quality).ok_or_else(|| {
        RmusixError::StreamResolution("no audio-only formats available".to_string())
    })?;
    audio_info_from_format(&chosen)
}

fn mime_string(format: &rusty_ytdl::VideoFormat) -> String {
    let rendered = format.mime_type.mime.to_string();
    if rendered.trim().is_empty() {
        "audio/mp4".to_string()
    } else {
        rendered
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_format(bitrate: u64, url: &str) -> rusty_ytdl::VideoFormat {
        let value = json!({
            "itag": bitrate,
            "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"",
            "bitrate": bitrate,
            "url": url,
            "hasVideo": false,
            "hasAudio": true,
            "isLive": false,
            "isHLS": false,
            "isDashMPD": false
        });
        serde_json::from_value(value).expect("fixture must deserialize")
    }

    fn video_only_format() -> rusty_ytdl::VideoFormat {
        let value = json!({
            "itag": 137,
            "mimeType": "video/mp4; codecs=\"avc1.640028\"",
            "bitrate": 5_000_000_u64,
            "url": "https://example.com/video",
            "hasVideo": true,
            "hasAudio": false,
            "isLive": false,
            "isHLS": false,
            "isDashMPD": false
        });
        serde_json::from_value(value).expect("fixture must deserialize")
    }

    #[test]
    fn picker_low_medium_high() {
        let formats = vec![
            test_format(48_000, "https://example.com/low"),
            test_format(128_000, "https://example.com/med"),
            test_format(256_000, "https://example.com/high"),
        ];
        let low = pick_audio_format(&formats, AudioQuality::Low).expect("low");
        let med = pick_audio_format(&formats, AudioQuality::Medium).expect("med");
        let high = pick_audio_format(&formats, AudioQuality::High).expect("high");
        assert_eq!(low.url, "https://example.com/low");
        assert_eq!(med.url, "https://example.com/med");
        assert_eq!(high.url, "https://example.com/high");
    }

    #[test]
    fn picker_ignores_video_only_and_blank_urls() {
        let formats = vec![
            video_only_format(),
            test_format(64_000, "   "),
            test_format(96_000, "https://example.com/only"),
        ];
        let picked = pick_audio_format(&formats, AudioQuality::High).expect("one valid");
        assert_eq!(picked.url, "https://example.com/only");
    }

    #[test]
    fn picker_empty_is_none() {
        assert!(pick_audio_format(&[], AudioQuality::High).is_none());
        assert!(pick_audio_format(&[video_only_format()], AudioQuality::Low).is_none());
        assert!(select_audio_stream(&[], AudioQuality::Medium).is_err());
    }

    #[test]
    fn audio_info_carries_url_mime_bitrate() {
        let format = test_format(128_000, "https://example.com/audio");
        let info = audio_info_from_format(&format).expect("info");
        assert_eq!(info.url, "https://example.com/audio");
        assert!(info.mime.contains("audio/"), "mime was {}", info.mime);
        assert_eq!(info.bitrate, 128_000);
    }

    #[test]
    fn medium_picks_upper_middle_on_even_counts() {
        let formats = vec![
            test_format(10_000, "https://example.com/a"),
            test_format(20_000, "https://example.com/b"),
            test_format(30_000, "https://example.com/c"),
            test_format(40_000, "https://example.com/d"),
        ];
        // Sorted ascending; index len/2 = 2 → third element.
        let med = pick_audio_format(&formats, AudioQuality::Medium).expect("med");
        assert_eq!(med.url, "https://example.com/c");
    }
}
