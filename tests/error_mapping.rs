use rmusix::errors::RmusixError;
use rmusix::youtube::{search, video};

#[test]
fn blank_query_and_id_are_invalid_input() {
    assert!(matches!(
        search::validate_query("   "),
        Err(RmusixError::InvalidInput(_))
    ));
    assert!(matches!(
        video::validate_video_id(""),
        Err(RmusixError::InvalidInput(_))
    ));
}

#[test]
fn duration_helper() {
    assert_eq!(search::duration_from_millis(0), None);
    assert!(search::duration_from_millis(1000).is_some());
}

#[test]
fn missing_binary_hints_mention_pacman() {
    let yt = RmusixError::Config("yt-dlp not found (pacman -S yt-dlp)".to_string());
    assert!(yt.to_string().contains("pacman -S yt-dlp"));
    let mpv = RmusixError::Config("mpv not found (pacman -S mpv)".to_string());
    assert!(mpv.to_string().contains("pacman -S mpv"));
}
