//! Audio format picking with progressive fallback for resolved streams.
//!
//! Pure selection over `rusty_ytdl` formats; no files, no subprocesses.
//!
//! Tier order (first non-empty tier wins): audio-only direct, audio-only any,
//! muxed progressive direct (video+audio in one URL — rodio/symphonia demuxes
//! the audio fine), then anything carrying audio (incl. HLS/DASH/live).

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

/// True for direct muxed (progressive) candidates: audio+video in one URL,
/// excluding HLS/DASH/live manifests. Decodes audio fine via rodio.
fn is_muxed_direct(format: &rusty_ytdl::VideoFormat) -> bool {
    format.has_audio
        && !format.url.trim().is_empty()
        && !format.is_hls
        && !format.is_dash_mpd
        && !format.is_live
}

/// Last resort: anything carrying audio with a usable URL (incl. HLS/DASH/live).
fn is_any_with_audio(format: &rusty_ytdl::VideoFormat) -> bool {
    format.has_audio && !format.url.trim().is_empty()
}

/// Pick a playable format for `quality`; `None` only when nothing carries audio.
///
/// Tiers (first non-empty wins), then Low/Med/High by effective bitrate:
/// T1 audio-only direct, T2 audio-only any, T3 muxed progressive direct,
/// T4 any-with-audio. Never panics; blank URLs excluded everywhere.
pub fn pick_audio_format(
    formats: &[rusty_ytdl::VideoFormat],
    quality: AudioQuality,
) -> Option<rusty_ytdl::VideoFormat> {
    let mut tier: Vec<&rusty_ytdl::VideoFormat> =
        formats.iter().filter(|f| is_direct_audio_only(f)).collect();
    if tier.is_empty() {
        tier = formats.iter().filter(|f| is_any_audio_only(f)).collect();
    }
    if tier.is_empty() {
        tier = formats.iter().filter(|f| is_muxed_direct(f)).collect();
    }
    if tier.is_empty() {
        tier = formats.iter().filter(|f| is_any_with_audio(f)).collect();
    }
    if tier.is_empty() {
        return None;
    }
    tier.sort_by_key(|f| effective_bitrate(f));
    let index = match quality {
        AudioQuality::Low => 0,
        AudioQuality::High => tier.len().saturating_sub(1),
        AudioQuality::Medium => tier.len() / 2,
    };
    tier.get(index).map(|f| (*f).clone())
}

/// Build [`AudioStreamInfo`] from a chosen format; errors on blank URLs.
///
/// Any mime is accepted: progressive (muxed) formats carry a `video/*` mime
/// but demux/decode to audio fine via rodio/symphonia.
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

/// Pick + convert in one step; errors with tier counts when nothing playable.
pub fn select_audio_stream(
    formats: &[rusty_ytdl::VideoFormat],
    quality: AudioQuality,
) -> Result<AudioStreamInfo, RmusixError> {
    if let Some(chosen) = pick_audio_format(formats, quality) {
        return audio_info_from_format(&chosen);
    }
    let total = formats.len();
    let audio_only = formats
        .iter()
        .filter(|f| f.has_audio && !f.has_video && !f.url.trim().is_empty())
        .count();
    let muxed = formats
        .iter()
        .filter(|f| f.has_audio && f.has_video && !f.url.trim().is_empty())
        .count();
    Err(RmusixError::StreamResolution(format!(
        "no playable audio formats: {total} formats checked \
         (audio_only={audio_only}, muxed={muxed}) for quality={quality:?}"
    )))
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

    fn muxed_format(bitrate: u64, url: &str) -> rusty_ytdl::VideoFormat {
        let value = json!({
            "itag": 18,
            "mimeType": "video/mp4; codecs=\"avc1.42001E, mp4a.40.2\"",
            "bitrate": bitrate,
            "url": url,
            "hasVideo": true,
            "hasAudio": true,
            "isLive": false,
            "isHLS": false,
            "isDashMPD": false
        });
        serde_json::from_value(value).expect("fixture must deserialize")
    }

    fn muxed_hls_format(bitrate: u64, url: &str) -> rusty_ytdl::VideoFormat {
        let value = json!({
            "itag": 96,
            "mimeType": "video/mp4; codecs=\"avc1.42001E, mp4a.40.2\"",
            "bitrate": bitrate,
            "url": url,
            "hasVideo": true,
            "hasAudio": true,
            "isLive": true,
            "isHLS": true,
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
        // None only when zero formats carry audio at all.
        assert!(pick_audio_format(&[], AudioQuality::High).is_none());
        assert!(pick_audio_format(&[video_only_format()], AudioQuality::Low).is_none());
        assert!(select_audio_stream(&[], AudioQuality::Medium).is_err());
        assert!(
            pick_audio_format(
                &[muxed_format(500_000, "https://example.com/muxed")],
                AudioQuality::Low
            )
            .is_some()
        );
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

    #[test]
    fn muxed_progressive_picked_when_no_audio_only() {
        let formats = vec![
            video_only_format(),
            muxed_format(500_000, "https://example.com/muxed"),
        ];
        let picked = pick_audio_format(&formats, AudioQuality::High).expect("muxed fallback");
        assert_eq!(picked.url, "https://example.com/muxed");
        let info = select_audio_stream(&formats, AudioQuality::Medium).expect("resolves");
        assert_eq!(info.url, "https://example.com/muxed");
    }

    #[test]
    fn direct_audio_preferred_over_muxed() {
        let formats = vec![
            muxed_format(5_000_000, "https://example.com/muxed-huge"),
            test_format(96_000, "https://example.com/audio"),
        ];
        // T1 tier wins despite the muxed URL having the higher bitrate.
        for quality in [AudioQuality::Low, AudioQuality::Medium, AudioQuality::High] {
            let picked = pick_audio_format(&formats, quality).expect("audio");
            assert_eq!(picked.url, "https://example.com/audio");
        }
    }

    #[test]
    fn live_hls_with_audio_picked_as_last_resort() {
        let formats = vec![muxed_hls_format(300_000, "https://example.com/live-muxed")];
        let picked = pick_audio_format(&formats, AudioQuality::Low).expect("last resort");
        assert_eq!(picked.url, "https://example.com/live-muxed");
    }

    #[test]
    fn empty_error_carries_counts() {
        let err = select_audio_stream(&[], AudioQuality::Medium).expect_err("empty must fail");
        let msg = err.to_string();
        assert!(msg.contains("no playable audio formats"), "got {msg}");
        assert!(msg.contains("0 formats checked"), "got {msg}");
        let err = select_audio_stream(&[video_only_format()], AudioQuality::High)
            .expect_err("video-only must fail");
        let msg = err.to_string();
        assert!(msg.contains("1 formats checked"), "got {msg}");
        assert!(msg.contains("audio_only=0"), "got {msg}");
    }

    #[test]
    fn video_mp4_mime_accepted() {
        let format = muxed_format(500_000, "https://example.com/muxed");
        let info = audio_info_from_format(&format).expect("progressive mime accepted");
        assert_eq!(info.url, "https://example.com/muxed");
        assert!(info.mime.contains("video/"), "mime was {}", info.mime);
    }
}
