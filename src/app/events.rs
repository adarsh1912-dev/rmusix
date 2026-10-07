//! App events (mpsc messages + key-mapped actions).

use crate::models::{AudioStreamInfo, Screen, Song};

#[derive(Debug, Clone, PartialEq, Default)]
pub enum AppEvent {
    #[default]
    Noop,
    Quit,
    EnterSearchMode,
    ExitSearchMode,
    SearchInput(char),
    SearchBackspace,
    SearchSubmit,
    PlaySelected,
    TogglePlayPause,
    EnqueueSelected,
    Next,
    Prev,
    SeekForward,
    SeekBackward,
    VolumeUp,
    VolumeDown,
    ToggleMute,
    SwitchScreen(Screen),
    CycleScreen,
}

/// Internal async completions delivered over the mpsc channel.
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) enum InternalEvent {
    App(AppEvent),
    SearchDone(Result<Vec<Song>, String>),
    ResolveDone(Result<(Song, AudioStreamInfo), String>),
    Audio(crate::audio::AudioEvent),
    Tick,
}
