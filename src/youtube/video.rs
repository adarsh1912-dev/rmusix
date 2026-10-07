//! yt-dlp video resolution (subprocess backend).

use std::time::Duration;

use crate::errors::RmusixError;
use crate::models::{AudioQuality, AudioStreamInfo};
use crate::youtube::stream::select_audio_stream;

/// Reject blank ids; return the trimmed id otherwise.
pub fn validate_video_id(video_id: &str) -> Result<String, RmusixError> {
    let trimmed = video_id.trim();
    if trimmed.is_empty() {
        return Err(RmusixError::InvalidInput(
            "video id must not be empty".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

/// Resolve a video id to a playable audio stream via yt-dlp.
///
/// Runs `yt-dlp -J --no-playlist <url>`, picks from `formats[]`, and falls
/// back to `yt-dlp -g -f bestaudio/best` when `-J` yields zero playable URLs.
pub async fn fetch_audio_info(
    ytdlp_path: &str,
    video_id: &str,
    quality: AudioQuality,
) -> Result<AudioStreamInfo, RmusixError> {
    fetch_audio_info_with_timeout(
        ytdlp_path,
        video_id,
        quality,
        crate::youtube::REQUEST_TIMEOUT,
    )
    .await
}

pub async fn fetch_audio_info_with_timeout(
    ytdlp_path: &str,
    video_id: &str,
    quality: AudioQuality,
    timeout: Duration,
) -> Result<AudioStreamInfo, RmusixError> {
    let id = validate_video_id(video_id)?;
    ensure_binary(ytdlp_path)?;
    let url = format!("https://www.youtube.com/watch?v={id}");
    let dump = run_with_timeout(ytdlp_path, &["-J", "--no-playlist", &url], timeout).await?;
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&dump) {
        if let Ok(info) = select_audio_stream(&json, quality) {
            return Ok(info);
        }
    }
    // Fallback: direct URL print.
    let printed = run_with_timeout(
        ytdlp_path,
        &["-g", "-f", "bestaudio/best", "--no-playlist", &url],
        timeout,
    )
    .await?;
    let stream_url = printed
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    if stream_url.is_empty() {
        return Err(RmusixError::StreamResolution(format!(
            "no audio formats found for {id} (total=0 audio=0)"
        )));
    }
    Ok(AudioStreamInfo {
        url: stream_url,
        mime: "webm".to_string(),
        bitrate: 0,
    })
}

fn ensure_binary(ytdlp_path: &str) -> Result<(), RmusixError> {
    if ytdlp_path.contains('/') {
        if std::path::Path::new(ytdlp_path).exists() {
            return Ok(());
        }
    } else if which::which(ytdlp_path).is_ok() {
        return Ok(());
    }
    Err(RmusixError::Config(
        "yt-dlp not found (pacman -S yt-dlp)".to_string(),
    ))
}

async fn run_with_timeout(
    bin: &str,
    args: &[&str],
    timeout: Duration,
) -> Result<String, RmusixError> {
    let mut cmd = tokio::process::Command::new(bin);
    cmd.args(args);
    let fut = cmd.output();
    let output = tokio::time::timeout(timeout, fut)
        .await
        .map_err(|_| RmusixError::Network("yt-dlp timed out".to_string()))?
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                RmusixError::Config("yt-dlp not found (pacman -S yt-dlp)".to_string())
            } else {
                RmusixError::Network(e.to_string())
            }
        })?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).to_string());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    Err(crate::youtube::search::map_search_error(
        output.status.code(),
        &stderr,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_blank_ids() {
        for id in ["", "   ", "\n "] {
            assert!(matches!(
                validate_video_id(id),
                Err(RmusixError::InvalidInput(_))
            ));
        }
        assert_eq!(
            validate_video_id("  dQw4w9WgXcQ  ").expect("valid"),
            "dQw4w9WgXcQ"
        );
    }

    const FIXTURE_J: &str = r#"{"id":"dQw4w9WgXcQ","title":"Never Gonna Give You Up","formats":[
        {"url":"http://x/low","acodec":"opus","vcodec":"none","ext":"webm","abr":48},
        {"url":"http://x/high","acodec":"opus","vcodec":"none","ext":"webm","abr":160}]}"#;

    #[test]
    fn fixture_resolves_to_url() {
        let json: serde_json::Value = serde_json::from_str(FIXTURE_J).expect("fixture");
        let info = select_audio_stream(&json, AudioQuality::High).expect("resolve");
        assert_eq!(info.url, "http://x/high");
    }

    #[tokio::test]
    async fn missing_binary_is_config_without_spawn() {
        let err = fetch_audio_info_with_timeout(
            "/nonexistent/yt-dlp",
            "dQw4w9WgXcQ",
            AudioQuality::Medium,
            Duration::from_secs(5),
        )
        .await
        .expect_err("missing bin must fail");
        assert!(
            matches!(err, RmusixError::Config(_)),
            "expected Config, got {err:?}"
        );
    }
}
