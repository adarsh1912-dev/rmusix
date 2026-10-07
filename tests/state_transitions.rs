use rmusix::app::{App, AppEvent, Mode};
use rmusix::config::AppConfig;
use rmusix::errors::RmusixError;
use rmusix::models::{AudioQuality, AudioStreamInfo, PlaybackState, Screen, Song};
use rmusix::youtube::YouTubeClient;
use std::sync::Arc;

struct MockYt {
    songs: Vec<Song>,
}

impl MockYt {
    fn song(id: &str) -> Song {
        Song {
            video_id: id.into(),
            title: format!("t{id}"),
            channel: "c".into(),
            duration: None,
            thumbnail_url: None,
        }
    }
}

impl YouTubeClient for MockYt {
    async fn search(&self, _q: &str, _l: usize) -> Result<Vec<Song>, RmusixError> {
        Ok(self.songs.clone())
    }
    async fn resolve_audio_stream(
        &self,
        vid: &str,
        _q: AudioQuality,
    ) -> Result<AudioStreamInfo, RmusixError> {
        Ok(AudioStreamInfo {
            url: format!("https://x/{vid}"),
            mime: "audio/mp4".into(),
            bitrate: 128,
        })
    }
}

fn app() -> App<MockYt> {
    App::new(
        AppConfig::default(),
        Arc::new(MockYt {
            songs: vec![MockYt::song("a"), MockYt::song("b")],
        }),
        None,
    )
}

#[test]
fn search_mode_cycle() {
    let mut a = app();
    a.handle(AppEvent::EnterSearchMode);
    assert_eq!(a.state.mode, Mode::Search);
    a.handle(AppEvent::SearchInput('x'));
    assert_eq!(a.state.query, "x");
    a.handle(AppEvent::SearchBackspace);
    assert!(a.state.query.is_empty());
    a.handle(AppEvent::SearchSubmit);
    assert!(a.state.is_searching);
    assert_eq!(a.state.mode, Mode::Normal);
}

#[test]
fn play_pause_vol_mute_and_screens() {
    let mut a = app();
    a.state.results = vec![MockYt::song("a")];
    a.handle(AppEvent::EnqueueSelected);
    assert_eq!(a.state.queue.len(), 1);

    // PlaySelected marks loading (async resolve happens in run loop).
    a.handle(AppEvent::PlaySelected);
    assert_eq!(a.state.playback, Some(PlaybackState::Loading));

    // Simulate resolved playback, then toggle pause/resume.
    a.state.playback = Some(PlaybackState::Playing);
    a.handle(AppEvent::TogglePlayPause);
    assert_eq!(a.state.playback, Some(PlaybackState::Paused));
    a.handle(AppEvent::TogglePlayPause);
    assert_eq!(a.state.playback, Some(PlaybackState::Playing));

    // Seek-while-playing issues commands without touching playback state.
    assert!(a.handle(AppEvent::SeekForward).is_some());
    assert!(a.handle(AppEvent::SeekBackward).is_some());

    // Volume + mute.
    a.handle(AppEvent::VolumeUp);
    assert!(a.state.volume > 0.0);
    a.handle(AppEvent::ToggleMute);
    assert!(a.state.muted);

    // Screen switching incl. search-while-playing screen preservation.
    a.handle(AppEvent::SwitchScreen(Screen::Queue));
    assert_eq!(a.state.current_screen(), Screen::Queue);
    a.handle(AppEvent::CycleScreen);
    assert_eq!(a.state.current_screen(), Screen::Help);

    // Queue skip via Next/Prev.
    a.state.results = vec![MockYt::song("b")];
    a.handle(AppEvent::EnqueueSelected);
    a.handle(AppEvent::Next);
    assert_eq!(a.queue().current().unwrap().video_id, "b");
    a.handle(AppEvent::Prev);
    assert_eq!(a.queue().current().unwrap().video_id, "a");
}

#[tokio::test]
async fn mocked_search_never_touches_network() {
    let yt = MockYt {
        songs: vec![MockYt::song("a")],
    };
    let out = yt.search("anything", 5).await.unwrap();
    assert_eq!(out.len(), 1);
    let info = yt
        .resolve_audio_stream("a", AudioQuality::Medium)
        .await
        .unwrap();
    assert!(info.url.contains('a'));
}
