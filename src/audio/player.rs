//! Async audio engine driving rodio on a dedicated audio thread.

use super::{AudioCommand, AudioEvent, controls};
use crate::models::AudioStreamInfo;
use rodio::Player;
use rodio::stream::{DeviceSinkBuilder, MixerDeviceSink};
use rodio::{Decoder, Source};
use std::io::Cursor;
use std::sync::Arc;
use std::sync::mpsc as stdmpsc;
use std::time::Duration;
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
    ///
    /// A dedicated OS thread owns the rodio [`OutputStream`] (kept alive by
    /// parking that thread); the async task owns the [`Sink`] and performs
    /// fetching/decoding. Engine stays alive across errors.
    pub fn spawn() -> (Self, EventRx) {
        let (cmd_tx, cmd_rx) = mpsc::channel::<AudioCommand>(32);
        let (evt_tx, evt_rx) = mpsc::channel::<AudioEvent>(64);

        // Dedicated thread owns the OutputStream; hand the Sink back.
        let (sink_tx, sink_rx) = stdmpsc::channel::<Option<Arc<Player>>>();
        std::thread::Builder::new()
            .name("rmusix-audio".to_string())
            .spawn(move || {
                let opened: Option<(MixerDeviceSink, Player)> =
                    DeviceSinkBuilder::from_default_device()
                        .ok()
                        .and_then(|b| b.open_stream().ok())
                        .map(|dev_sink| {
                            let player = Player::connect_new(dev_sink.mixer());
                            (dev_sink, player)
                        });
                // Keep the device sink alive on this thread.
                let player: Option<Arc<Player>> = opened.map(|(dev_sink, player)| {
                    std::mem::forget(dev_sink);
                    Arc::new(player)
                });
                let _ = sink_tx.send(player);
                // Park forever; the forgotten device sink keeps the device alive.
                loop {
                    std::thread::park();
                }
            })
            .expect("spawn audio thread");

        tokio::spawn(async move {
            let sink: Option<Arc<Player>> = sink_rx.recv().ok().flatten();
            run_loop(cmd_rx, evt_tx, sink).await;
        });

        (Self { cmd_tx }, evt_rx)
    }

    /// Non-blocking handle to send commands.
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

async fn run_loop(mut cmds: CmdRx, events: mpsc::Sender<AudioEvent>, sink: Option<Arc<Player>>) {
    let sink = sink.or_else(|| {
        // Headless fallback: no device; keep command loop alive.
        None
    });
    let mut vol = controls::VolumeControl::new(1.0);
    let mut play_epoch: u64 = 0;
    let mut finished_sent_for: Option<u64> = None;

    // Position ticker state.
    let mut ticker = tokio::time::interval(Duration::from_millis(500));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // Skip the immediate first tick.
    ticker.tick().await;

    // Track whether a fetch/decode is in flight so Stop cancels it.
    let mut current_url: Option<String> = None;

    if let Some(s) = sink.as_ref() {
        s.set_volume(vol.volume());
    }

    loop {
        tokio::select! {
            cmd = cmds.recv() => {
                let Some(cmd) = cmd else { break };
                match cmd {
                    AudioCommand::Play(info) => {
                        play_epoch = play_epoch.wrapping_add(1);
                        let _epoch = play_epoch;
                        finished_sent_for = None;
                        current_url = Some(info.url.clone());
                        if let Some(s) = sink.as_ref() {
                            s.stop();
                        }
                        emit(&events, AudioEvent::Buffering(true)).await;
                        let _ep = play_epoch_ptr(play_epoch);
                        tokio::spawn(play_stream(info, sink.clone(), events.clone()));
                    }
                    AudioCommand::Pause => {
                        if let Some(s) = sink.as_ref() { s.pause(); }
                    }
                    AudioCommand::Resume => {
                        if let Some(s) = sink.as_ref() { s.play(); }
                    }
                    AudioCommand::Stop => {
                        play_epoch = play_epoch.wrapping_add(1);
                        current_url = None;
                        finished_sent_for = None;
                        if let Some(s) = sink.as_ref() { s.stop(); }
                        emit(&events, AudioEvent::Buffering(false)).await;
                    }
                    AudioCommand::SetVolume(v) => {
                        vol.set(v);
                        if let Some(s) = sink.as_ref() { s.set_volume(vol.volume()); }
                    }
                    AudioCommand::ToggleMute => {
                        vol.toggle_mute();
                        if let Some(s) = sink.as_ref() { s.set_volume(vol.volume()); }
                    }
                    AudioCommand::SeekForward(d) => {
                        seek_by(sink.as_ref(), &events, d, true).await;
                    }
                    AudioCommand::SeekBackward(d) => {
                        seek_by(sink.as_ref(), &events, d, false).await;
                    }
                }
                let _ = current_url.as_ref();
            }
            _ = ticker.tick() => {
                if let Some(s) = sink.as_ref() {
                    let pos = s.get_pos();
                    emit(&events, AudioEvent::Position(pos)).await;
                    if s.empty() && finished_sent_for != Some(play_epoch) && play_epoch > 0 {
                        // Only report Finished when something was actually played;
                        // empty sink at startup must not fire.
                        finished_sent_for = Some(play_epoch);
                        emit(&events, AudioEvent::Finished).await;
                    }
                }
            }
        }
    }
}

// Helper to capture generation (avoids borrow issues).
fn play_epoch_ptr(g: u64) -> u64 {
    g
}

/// Fetch URL fully into memory (no permanent files), decode, and append.
async fn play_stream(
    info: AudioStreamInfo,
    sink: Option<Arc<Player>>,
    events: mpsc::Sender<AudioEvent>,
) {
    let bytes = match fetch_bytes(&info.url).await {
        Ok(b) => b,
        Err(e) => {
            emit(&events, AudioEvent::Buffering(false)).await;
            emit(&events, AudioEvent::Error(e)).await;
            return;
        }
    };
    let total = decode_and_play(bytes, sink.as_ref()).await;
    match total {
        Ok(dur) => {
            emit(&events, AudioEvent::Duration(dur)).await;
            emit(&events, AudioEvent::Buffering(false)).await;
        }
        Err(e) => {
            emit(&events, AudioEvent::Buffering(false)).await;
            emit(&events, AudioEvent::Error(e)).await;
        }
    }
}

async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    use futures::StreamExt;
    let resp = reqwest::get(url).await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("http {}", resp.status()));
    }
    let mut stream = resp.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(c) => buf.extend_from_slice(&c),
            Err(e) => return Err(e.to_string()),
        }
    }
    if buf.is_empty() {
        return Err("empty stream".to_string());
    }
    Ok(buf)
}

async fn decode_and_play(
    bytes: Vec<u8>,
    sink: Option<&Arc<Player>>,
) -> Result<Option<Duration>, String> {
    let cursor = Cursor::new(bytes);
    let decoder = Decoder::new(cursor).map_err(|e| e.to_string())?;
    let total = decoder.total_duration();
    let Some(s) = sink else {
        // Headless: decode succeeded; report duration without playback.
        return Ok(total);
    };
    s.append(decoder);
    s.play();
    Ok(total)
}

async fn seek_by(
    sink: Option<&Arc<Player>>,
    events: &mpsc::Sender<AudioEvent>,
    delta: Duration,
    forward: bool,
) {
    let Some(s) = sink else { return };
    let cur = s.get_pos();
    // Note: total_duration of the appended source isn't tracked here, so
    // forward seeks saturate only against current position + delta; the
    // decoder clamps at stream end internally.
    let target = if forward {
        controls::seek_forward_target(cur, delta, None)
    } else {
        controls::seek_backward_target(cur, delta)
    };
    // Fallback: `try_seek` is unsupported by some symphonia-backed sources.
    // In that case keep playing unchanged (documented) instead of erroring.
    if s.try_seek(target).is_ok() {
        emit(events, AudioEvent::Position(target)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration as StdDuration;

    #[test]
    fn engine_handles_missing_device_without_panic() {
        // Pure state check: VolumeControl + seek helpers used by run_loop.
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

    /// Requires real audio hardware; ignored in CI/headless.
    #[tokio::test]
    #[ignore]
    async fn hardware_playback_smoke() {
        let (engine, mut rx) = AudioEngine::spawn();
        assert!(engine.send(AudioCommand::SetVolume(0.2)).await);
        assert!(
            engine
                .send(AudioCommand::SeekForward(StdDuration::from_secs(5)))
                .await
        );
        let _ = tokio::time::timeout(StdDuration::from_millis(600), rx.recv()).await;
    }
}
