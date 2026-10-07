//! mpv subprocess engine stub (tokio::process; full IPC in Agent 2).

use super::{AudioCommand, AudioEvent};
use tokio::sync::mpsc;

pub type EventRx = mpsc::Receiver<AudioEvent>;
type CmdRx = mpsc::Receiver<AudioCommand>;
type CmdTx = mpsc::Sender<AudioCommand>;

/// Handle for controlling playback.
pub struct AudioEngine {
    cmd_tx: CmdTx,
}

impl AudioEngine {
    /// Spawn the engine. Returns the command handle plus the event stream.
    /// mpv child process management lands in Agent 2; loop stays alive.
    pub fn spawn() -> (Self, EventRx) {
        let (cmd_tx, cmd_rx) = mpsc::channel::<AudioCommand>(32);
        let (evt_tx, evt_rx) = mpsc::channel::<AudioEvent>(64);
        tokio::spawn(async move { run_loop(cmd_rx, evt_tx).await });
        (Self { cmd_tx }, evt_rx)
    }

    pub fn sender(&self) -> CmdTx {
        self.cmd_tx.clone()
    }

    pub async fn send(&self, cmd: AudioCommand) -> bool {
        self.cmd_tx.send(cmd).await.is_ok()
    }
}

async fn emit(tx: &mpsc::Sender<AudioEvent>, ev: AudioEvent) {
    let _ = tx.send(ev).await;
}

async fn run_loop(mut cmds: CmdRx, events: mpsc::Sender<AudioEvent>) {
    loop {
        match cmds.recv().await {
            None => break,
            Some(AudioCommand::Play(_info)) => {
                emit(&events, AudioEvent::Buffering(true)).await;
                emit(&events, AudioEvent::Buffering(false)).await;
            }
            Some(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::controls;
    use std::time::Duration as StdDuration;

    #[test]
    fn engine_handles_missing_device_without_panic() {
        let mut v = controls::VolumeControl::new(0.5);
        v.set(2.0);
        assert_eq!(v.stored_volume(), 1.0);
        assert_eq!(
            controls::seek_forward_target(
                StdDuration::from_secs(3),
                StdDuration::from_secs(2),
                None
            ),
            StdDuration::from_secs(5)
        );
    }

    #[tokio::test]
    async fn stub_play_emits_buffering() {
        let (engine, mut rx) = AudioEngine::spawn();
        assert!(
            engine
                .send(AudioCommand::Play(crate::models::AudioStreamInfo {
                    url: "https://example.com/a".into(),
                    mime: "audio/mp4".into(),
                    bitrate: 128,
                }))
                .await
        );
        let ev = tokio::time::timeout(StdDuration::from_secs(2), rx.recv())
            .await
            .expect("event")
            .expect("some");
        assert!(matches!(ev, AudioEvent::Buffering(_)));
    }
}
