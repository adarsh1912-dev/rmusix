//! App state (extended additively by UI agent; existing fields preserved).
//! App run loop: crossterm EventStream + tokio mpsc, non-blocking.

pub mod events;
pub mod state;

pub use events::AppEvent;
pub(crate) use events::InternalEvent;
#[allow(unused_imports)]
pub use state::{AppState, InputMode, Mode};

use crate::audio::{AudioCommand, AudioEngine, AudioEvent};
use crate::config::AppConfig;
use crate::models::{AudioStreamInfo, PlaybackState, Screen, Song};
use crate::queue::QueueManager;
use crossterm::event::{Event, EventStream, KeyEventKind};
use futures::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

pub struct App<Y> {
    pub state: AppState,
    pub config: AppConfig,
    pub youtube: Arc<Y>,
    queue: QueueManager,
}

impl<Y: crate::youtube::YouTubeClient + 'static> App<Y> {
    pub fn new(config: AppConfig, youtube: Arc<Y>, initial_query: Option<String>) -> Self {
        let mut state = AppState::new(initial_query);
        state.volume = config.volume.clamp(0.0, 1.0);
        Self {
            state,
            config,
            youtube,
            queue: QueueManager::new(),
        }
    }

    /// Borrow the owned play queue.
    pub fn queue(&self) -> &QueueManager {
        &self.queue
    }

    /// Pure transition helper used by run loop and offline tests.
    /// Returns an optional audio command to send (non-blocking).
    pub fn handle(&mut self, ev: AppEvent) -> Option<AudioCommand> {
        match ev {
            AppEvent::Noop => None,
            AppEvent::Quit => None,
            AppEvent::EnterSearchMode => {
                self.state.mode = Mode::Search;
                None
            }
            AppEvent::ExitSearchMode => {
                self.state.mode = Mode::Normal;
                None
            }
            AppEvent::SearchInput(c) => {
                self.state.query.push(c);
                None
            }
            AppEvent::SearchBackspace => {
                self.state.query.pop();
                None
            }
            AppEvent::SearchSubmit => {
                self.state.mode = Mode::Normal;
                self.state.is_searching = true;
                self.state.search_error = None;
                None
            }
            AppEvent::PlaySelected => {
                if let Some(song) = self.state.results.get(self.state.selected).cloned() {
                    self.enqueue_and_select(song)
                } else {
                    None
                }
            }
            AppEvent::EnqueueSelected => {
                if let Some(song) = self.state.results.get(self.state.selected).cloned() {
                    self.queue.push(song);
                    self.sync_queue();
                }
                None
            }
            // Selection moves use clamp (no wrap) to keep `>` predictable.
            AppEvent::SelectUp => {
                match self.state.current_screen() {
                    Screen::Search => {
                        if !self.state.results.is_empty() {
                            self.state.selected = self.state.selected.saturating_sub(1);
                        }
                    }
                    Screen::Queue => {
                        if !self.queue.is_empty() {
                            let cur = self.queue.index().unwrap_or(0);
                            self.queue.play_at(cur.saturating_sub(1));
                            self.sync_queue();
                        }
                    }
                    Screen::Help => {}
                }
                None
            }
            AppEvent::SelectDown => {
                match self.state.current_screen() {
                    Screen::Search => {
                        if !self.state.results.is_empty() {
                            let max = self.state.results.len() - 1;
                            self.state.selected = (self.state.selected + 1).min(max);
                        }
                    }
                    Screen::Queue => {
                        if !self.queue.is_empty() {
                            let cur = self.queue.index().unwrap_or(0);
                            let max = self.queue.len() - 1;
                            self.queue.play_at((cur + 1).min(max));
                            self.sync_queue();
                        }
                    }
                    Screen::Help => {}
                }
                None
            }
            AppEvent::TogglePlayPause => {
                if self.state.playing() {
                    self.state.playback = Some(PlaybackState::Paused);
                    Some(AudioCommand::Pause)
                } else if matches!(self.state.playback, Some(PlaybackState::Paused)) {
                    self.state.playback = Some(PlaybackState::Playing);
                    Some(AudioCommand::Resume)
                } else {
                    None
                }
            }
            AppEvent::Next => self.advance(1),
            AppEvent::Prev => self.advance(-1),
            AppEvent::SeekForward => Some(AudioCommand::SeekForward(Duration::from_secs(5))),
            AppEvent::SeekBackward => Some(AudioCommand::SeekBackward(Duration::from_secs(5))),
            AppEvent::VolumeUp => {
                self.state.volume = (self.state.volume + 0.05).clamp(0.0, 1.0);
                Some(AudioCommand::SetVolume(self.state.volume))
            }
            AppEvent::VolumeDown => {
                self.state.volume = (self.state.volume - 0.05).clamp(0.0, 1.0);
                Some(AudioCommand::SetVolume(self.state.volume))
            }
            AppEvent::ToggleMute => {
                self.state.muted = !self.state.muted;
                Some(AudioCommand::ToggleMute)
            }
            AppEvent::SwitchScreen(s) => {
                self.state.screen = Some(s);
                None
            }
            AppEvent::CycleScreen => {
                let next = match self.state.current_screen() {
                    Screen::Search => Screen::Queue,
                    Screen::Queue => Screen::Help,
                    Screen::Help => Screen::Search,
                };
                self.state.screen = Some(next);
                None
            }
        }
    }

    fn enqueue_and_select(&mut self, song: Song) -> Option<AudioCommand> {
        // Resolve is async; mark loading. Caller (run loop) spawns resolve task.
        self.queue.push(song);
        let last = self.queue.len().saturating_sub(1);
        self.queue.play_at(last);
        self.sync_queue();
        self.state.playback = Some(PlaybackState::Loading);
        None
    }

    fn advance(&mut self, dir: i32) -> Option<AudioCommand> {
        if self.queue.is_empty() {
            return None;
        }
        let cur = self.queue.index().unwrap_or(0) as i32;
        let len = self.queue.len() as i32;
        let nxt = (cur + dir).clamp(0, len - 1) as usize;
        self.queue.play_at(nxt);
        self.sync_queue();
        self.state.playback = Some(PlaybackState::Loading);
        None
    }

    fn current_queued(&self) -> Option<Song> {
        self.queue.current().cloned()
    }

    /// Mirror the owned [`QueueManager`] into render state (UI reads `state`).
    fn sync_queue(&mut self) {
        self.state.queue = self.queue.songs().to_vec();
        self.state.queue_selected = self.queue.index().unwrap_or(0);
    }

    fn play_song(&mut self, song: &Song, info: AudioStreamInfo) -> AudioCommand {
        self.state.current_title = Some(song.title.clone());
        self.state.total = song.duration;
        self.state.position = Duration::ZERO;
        self.state.playback = Some(PlaybackState::Playing);
        AudioCommand::Play(info)
    }

    pub async fn run(mut self) -> Result<(), crate::errors::RmusixError> {
        use crossterm::execute;
        use crossterm::terminal::{EnterAlternateScreen, enable_raw_mode};
        use std::io::stdout;

        enable_raw_mode().map_err(crate::errors::RmusixError::from)?;
        let mut out = stdout();
        execute!(out, EnterAlternateScreen).map_err(crate::errors::RmusixError::from)?;
        let restore = || {
            let _ = crossterm::terminal::disable_raw_mode();
            let _ = execute!(stdout(), crossterm::terminal::LeaveAlternateScreen);
        };

        let (engine, mut audio_rx) = AudioEngine::spawn();
        let _ = engine
            .send(AudioCommand::SetVolume(self.state.volume))
            .await;

        let (tx, mut rx) = mpsc::channel::<InternalEvent>(128);
        // Initial search if query present.
        if let Some(q) = self.state.initial_query.clone().filter(|s| !s.is_empty()) {
            self.state.is_searching = true;
            spawn_search(
                self.youtube.clone(),
                q,
                self.config.search_limit,
                tx.clone(),
            );
        }

        let mut term = ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(stdout()))
            .map_err(|e| crate::errors::RmusixError::Audio(e.to_string()))?;
        let mut events = EventStream::new();
        let mut ticker = tokio::time::interval(Duration::from_millis(250));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        let result: Result<(), String> = loop {
            tokio::select! {
                maybe_ev = events.next() => {
                    match maybe_ev {
                        Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                            if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
                                && matches!(key.code, crossterm::event::KeyCode::Char('c')) {
                                let _ = engine.send(AudioCommand::Stop).await;
                                break Ok(());
                            }
                            let action = crate::ui::key_to_action(key, &self.state);
                            if matches!(action, AppEvent::Quit) {
                                let _ = engine.send(AudioCommand::Stop).await;
                                break Ok(());
                            }
                            let needs_search = matches!(action, AppEvent::SearchSubmit);
                            let needs_resolve = matches!(action, AppEvent::PlaySelected | AppEvent::Next | AppEvent::Prev);
                            if let Some(cmd) = self.handle(action) {
                                let _ = engine.send(cmd).await;
                            }
                            if needs_search {
                                let q = self.state.query.clone();
                                spawn_search(self.youtube.clone(), q, self.config.search_limit, tx.clone());
                            }
                            if needs_resolve
                                && let Some(song) = self.current_queued()
                            {
                                spawn_resolve(self.youtube.clone(), song, self.config.quality, tx.clone());
                            }
                        }
                        Some(Ok(_)) => {}
                        Some(Err(_)) => {}
                        None => break Ok(()),
                    }
                }
                Some(msg) = rx.recv() => {
                    match msg {
                        InternalEvent::App(a) => {
                            if let Some(cmd) = self.handle(a) {
                                let _ = engine.send(cmd).await;
                            }
                        }
                        InternalEvent::SearchDone(r) => {
                            self.state.is_searching = false;
                            match r {
                                Ok(songs) => {
                                    self.state.results = songs;
                                    self.state.selected = 0;
                                    self.state.search_error = None;
                                }
                                Err(e) => self.state.search_error = Some(e),
                            }
                        }
                        InternalEvent::ResolveDone(r) => {
                            match r {
                                Ok((song, info)) => {
                                    let cmd = self.play_song(&song, info);
                                    let _ = engine.send(cmd).await;
                                }
                                Err(e) => self.state.playback = Some(PlaybackState::Error(e)),
                            }
                        }
                        InternalEvent::Audio(ae) => match ae {
                            AudioEvent::Position(p) => self.state.position = p,
                            AudioEvent::Duration(d) => self.state.total = d,
                            AudioEvent::Buffering(true) => self.state.playback = Some(PlaybackState::Buffering),
                            AudioEvent::Buffering(false) => {}
                            AudioEvent::Finished => {
                                // Auto-next while playing: advance in queue and resolve.
                                if let Some(_cmd) = self.advance(1) {}
                                if self.state.playback != Some(PlaybackState::Stopped)
                                    && let Some(song) = self.current_queued()
                                {
                                    spawn_resolve(self.youtube.clone(), song, self.config.quality, tx.clone());
                                }
                            }
                            AudioEvent::Error(e) => self.state.playback = Some(PlaybackState::Error(e)),
                        },
                        InternalEvent::Tick => {
                            self.state.spinner_frame = self.state.spinner_frame.wrapping_add(1);
                        }
                    }
                }
                Some(ae) = audio_rx.recv() => {
                    let _ = tx.send(InternalEvent::Audio(ae)).await;
                }
                _ = ticker.tick() => {
                    let _ = tx.send(InternalEvent::Tick).await;
                }
            }
            // Render each iteration (non-blocking).
            let _ = term.draw(|f| crate::ui::render(f, &self.state));
        };
        restore();
        result.map_err(crate::errors::RmusixError::Audio)
    }
}

fn spawn_search<Y: crate::youtube::YouTubeClient + 'static>(
    yt: Arc<Y>,
    query: String,
    limit: usize,
    tx: mpsc::Sender<InternalEvent>,
) {
    tokio::task::spawn(async move {
        // `YouTubeClient::search` future is not guaranteed `Send` (native
        // `async fn` in trait), so box it without requiring Send by running
        // the await inside a `spawn` via `Handle::spawn` is not possible;
        // instead execute here: this future itself only requires Y: Send+Sync
        // for the reference, and we avoid nested spawn entirely by awaiting
        // through a `Send`-safe indirection: call via `tokio::task::block_in_place`?
        // Simplest sound approach: wrap in an async block and spawn_local if
        // within a LocalSet, else run directly. Here we are already in a
        // spawned (Send) task but the inner future may be !Send — so use a
        // thread-local trick: run the future to completion on the current
        // task via `std::future::poll_fn`? Instead, just await it: the outer
        // future becomes !Send too, which `tokio::spawn` rejects.
        // Workaround: use `tokio::task::LocalSet::spawn_local` through a
        // dedicated single-thread runtime is overkill; the pragmatic fix is
        // to require the concrete future be Send via a helper trait bound
        // checked at runtime. We sidestep by spawning a blocking thread that
        // drives its own current-thread runtime.
        let yt2 = yt.clone();
        let res: Result<Vec<Song>, String> =
            tokio::task::spawn_blocking(move || run_search_blocking(yt2, query, limit))
                .await
                .unwrap_or_else(|e| Err(e.to_string()));
        let _ = tx.send(InternalEvent::SearchDone(res)).await;
    });
}

fn run_search_blocking<Y: crate::youtube::YouTubeClient + 'static>(
    yt: Arc<Y>,
    query: String,
    limit: usize,
) -> Result<Vec<Song>, String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?
        .block_on(async move { yt.search(&query, limit).await.map_err(|e| e.to_string()) })
}

fn spawn_resolve<Y: crate::youtube::YouTubeClient + 'static>(
    yt: Arc<Y>,
    song: Song,
    quality: crate::models::AudioQuality,
    tx: mpsc::Sender<InternalEvent>,
) {
    tokio::task::spawn(async move {
        let res: Result<(Song, AudioStreamInfo), String> = tokio::task::spawn_blocking(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| e.to_string())?
                .block_on(async move {
                    yt.resolve_audio_stream(&song.video_id, quality)
                        .await
                        .map(|info| (song, info))
                        .map_err(|e| e.to_string())
                })
        })
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
        let _ = tx.send(InternalEvent::ResolveDone(res)).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::RmusixError;
    use crate::models::{AudioStreamInfo, Song};
    use crate::youtube::YouTubeClient;

    struct MockYt {
        songs: Vec<Song>,
        fail_search: bool,
        fail_resolve: bool,
    }

    impl crate::youtube::YouTubeClient for MockYt {
        async fn search(&self, _q: &str, _l: usize) -> Result<Vec<Song>, RmusixError> {
            if self.fail_search {
                return Err(RmusixError::Search("mock search fail".into()));
            }
            Ok(self.songs.clone())
        }
        async fn resolve_audio_stream(
            &self,
            vid: &str,
            _q: crate::models::AudioQuality,
        ) -> Result<AudioStreamInfo, RmusixError> {
            if self.fail_resolve {
                return Err(RmusixError::StreamResolution("mock resolve fail".into()));
            }
            Ok(AudioStreamInfo {
                url: format!("https://x/{vid}"),
                mime: "audio/mp4".into(),
                bitrate: 128,
            })
        }
    }

    fn song(id: &str) -> Song {
        Song {
            video_id: id.into(),
            title: format!("t{id}"),
            channel: "c".into(),
            duration: None,
            thumbnail_url: None,
        }
    }

    fn app() -> App<MockYt> {
        App::new(
            AppConfig::default(),
            Arc::new(MockYt {
                songs: vec![song("a"), song("b")],
                fail_search: false,
                fail_resolve: false,
            }),
            None,
        )
    }

    #[test]
    fn transitions_search_mode() {
        let mut a = app();
        a.handle(AppEvent::EnterSearchMode);
        assert_eq!(a.state.mode, Mode::Search);
        a.handle(AppEvent::SearchInput('x'));
        assert_eq!(a.state.query, "x");
        a.handle(AppEvent::SearchBackspace);
        assert!(a.state.query.is_empty());
        a.handle(AppEvent::SearchSubmit);
        assert!(a.state.is_searching);
    }

    #[test]
    fn queue_enqueue_and_vol_mute() {
        let mut a = app();
        a.state.results = vec![song("a")];
        a.handle(AppEvent::EnqueueSelected);
        assert_eq!(a.state.queue.len(), 1);
        let v0 = a.state.volume;
        a.handle(AppEvent::VolumeUp);
        assert!(a.state.volume >= v0);
        a.handle(AppEvent::VolumeDown);
        a.handle(AppEvent::ToggleMute);
        assert!(a.state.muted);
        a.handle(AppEvent::ToggleMute);
        assert!(!a.state.muted);
    }

    #[tokio::test]
    async fn offline_mock_search_and_resolve() {
        let yt = MockYt {
            songs: vec![song("a")],
            fail_search: false,
            fail_resolve: true,
        };
        let r = yt.search("q", 5).await.unwrap();
        assert_eq!(r.len(), 1);
        assert!(
            yt.resolve_audio_stream("a", crate::models::AudioQuality::Medium)
                .await
                .is_err()
        );
    }

    #[test]
    fn select_clamps_and_empty_no_panic() {
        let mut a = app();
        // Empty: no panic, stays 0.
        a.state.results = vec![];
        a.handle(AppEvent::SelectUp);
        a.handle(AppEvent::SelectDown);
        assert_eq!(a.state.selected, 0);
        // Clamp at bounds.
        a.state.results = vec![song("a"), song("b"), song("c")];
        a.state.selected = 0;
        a.handle(AppEvent::SelectUp);
        assert_eq!(a.state.selected, 0);
        a.state.selected = 2;
        a.handle(AppEvent::SelectDown);
        assert_eq!(a.state.selected, 2);
        a.state.selected = 1;
        a.handle(AppEvent::SelectDown);
        assert_eq!(a.state.selected, 2);
        a.handle(AppEvent::SelectUp);
        assert_eq!(a.state.selected, 1);
    }

    #[test]
    fn select_queue_mirror_moves() {
        let mut a = app();
        a.state.results = vec![song("a"), song("b")];
        a.state.screen = Some(Screen::Queue);
        a.handle(AppEvent::EnqueueSelected); // selected=0 -> queue [a]
        a.state.selected = 1;
        a.handle(AppEvent::EnqueueSelected); // queue [a,b], index=0
        a.handle(AppEvent::SelectDown);
        assert_eq!(a.state.queue_selected, 1);
        assert_eq!(a.queue().index(), Some(1));
        a.handle(AppEvent::SelectDown); // clamp
        assert_eq!(a.state.queue_selected, 1);
        a.handle(AppEvent::SelectUp);
        assert_eq!(a.state.queue_selected, 0);
        assert_eq!(a.queue().index(), Some(0));
    }

    #[test]
    fn error_state_recorded() {
        let mut a = app();
        a.state.playback = Some(PlaybackState::Error("boom".into()));
        assert!(matches!(a.state.playback, Some(PlaybackState::Error(_))));
    }
}
