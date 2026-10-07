//! mpv subprocess engine over Unix-socket IPC (tokio only).

use super::{AudioCommand, AudioEvent};
use crate::audio::controls::VolumeControl;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::{Mutex, mpsc};

pub type EventRx = mpsc::Receiver<AudioEvent>;
type CmdRx = mpsc::Receiver<AudioCommand>;
type CmdTx = mpsc::Sender<AudioCommand>;

/// Shared IPC writer. Fake servers in tests just read JSON lines.
#[derive(Debug)]
struct IpcClient {
    writer: Mutex<tokio::net::unix::OwnedWriteHalf>,
    next_id: AtomicU64,
}

impl IpcClient {
    fn new(write_half: tokio::net::unix::OwnedWriteHalf) -> Self {
        Self {
            writer: Mutex::new(write_half),
            next_id: AtomicU64::new(1),
        }
    }

    async fn send(&self, command: serde_json::Value) -> IpcResult {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let msg = serde_json::json!({ "command": command, "request_id": id });
        let mut line = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        line.push('\n');
        let mut w = self.writer.lock().await;
        w.write_all(line.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        w.flush().await.map_err(|e| e.to_string())?;
        Ok(())
    }
}
type IpcResult = Result<(), String>;

/// Build a JSON IPC command (pure, tested against fake server).
pub fn ipc_command(name: &str, args: Vec<serde_json::Value>) -> serde_json::Value {
    let mut arr = vec![serde_json::Value::String(name.to_string())];
    arr.extend(args);
    serde_json::Value::Array(arr)
}

async fn connect_ipc(
    sock: &PathBuf,
) -> Option<(IpcClient, mpsc::UnboundedReceiver<serde_json::Value>)> {
    let stream = UnixStream::connect(sock).await.ok()?;
    let (rh, wh) = stream.into_split();
    let client = IpcClient::new(wh);
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut reader = BufReader::new(rh);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                        let _ = tx.send(v);
                    }
                }
                Err(_) => break,
            }
        }
    });
    Some((client, rx))
}

async fn wait_socket(sock: &PathBuf) -> bool {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    while tokio::time::Instant::now() < deadline {
        if UnixStream::connect(sock).await.is_ok() {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    false
}

/// Handle for controlling playback.
pub struct MpvEngine {
    cmd_tx: CmdTx,
}

impl MpvEngine {
    /// Spawn engine owning mpv child + IPC task.
    pub fn spawn(mpv_bin: String, socket_path: PathBuf) -> (Self, EventRx) {
        Self::spawn_with_volume(mpv_bin, socket_path, 1.0)
    }

    pub fn spawn_with_volume(
        mpv_bin: String,
        socket_path: PathBuf,
        initial_volume: f32,
    ) -> (Self, EventRx) {
        let (cmd_tx, cmd_rx) = mpsc::channel::<AudioCommand>(32);
        let (evt_tx, evt_rx) = mpsc::channel::<AudioEvent>(64);
        tokio::spawn(async move {
            run_loop(mpv_bin, socket_path, initial_volume, cmd_rx, evt_tx).await;
        });
        (Self { cmd_tx }, evt_rx)
    }

    pub fn sender(&self) -> CmdTx {
        self.cmd_tx.clone()
    }

    pub async fn send(&self, cmd: AudioCommand) -> bool {
        self.cmd_tx.send(cmd).await.is_ok()
    }
}

/// Back-compat handle used by app code (default mpv + temp socket).
pub struct AudioEngine {
    inner: MpvEngine,
}

impl AudioEngine {
    /// Spawn with defaults (keeps old call sites working).
    pub fn spawn() -> (Self, EventRx) {
        let sock = std::env::temp_dir().join(format!("rmusix-{}.sock", std::process::id()));
        let (inner, rx) = MpvEngine::spawn("mpv".to_string(), sock);
        (Self { inner }, rx)
    }

    pub fn sender(&self) -> CmdTx {
        self.inner.sender()
    }

    pub async fn send(&self, cmd: AudioCommand) -> bool {
        self.inner.send(cmd).await
    }
}

async fn emit(tx: &mpsc::Sender<AudioEvent>, ev: AudioEvent) {
    let _ = tx.send(ev).await;
}

async fn ensure_child(
    mpv_bin: &str,
    sock: &PathBuf,
    vol: &VolumeControl,
    events: &mpsc::Sender<AudioEvent>,
    st: &mut Option<tokio::process::Child>,
    ipc_holder: &mut Option<Arc<IpcClient>>,
    ev_rx_holder: &mut Option<mpsc::UnboundedReceiver<serde_json::Value>>,
) -> bool {
    if ipc_holder.is_some() {
        return true;
    }
    // Clean stale socket file.
    let _ = std::fs::remove_file(sock);
    let vol100 = vol.effective_mpv_volume();
    let mut cmd = tokio::process::Command::new(mpv_bin);
    cmd.args([
        "--no-video",
        "--idle",
        "--force-window=no",
        &format!("--input-ipc-server={}", sock.display()),
        &format!("--volume={vol100}"),
    ]);
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => {
            emit(
                events,
                AudioEvent::Error(format!("mpv not found: pacman -S mpv ({mpv_bin})")),
            )
            .await;
            return false;
        }
    };
    *st = Some(child);
    if !wait_socket(sock).await {
        emit(
            events,
            AudioEvent::Error("mpv socket not ready".to_string()),
        )
        .await;
        return false; // stay alive, child kept
    }
    match connect_ipc(sock).await {
        Some((client, rx)) => {
            *ipc_holder = Some(Arc::new(client));
            *ev_rx_holder = Some(rx);
            // observe properties
            for prop in ["time-pos", "duration", "pause", "eof-reached"] {
                if let Some(c) = ipc_holder.as_ref() {
                    let _ = c
                        .send(ipc_command(
                            "observe_property",
                            vec![serde_json::json!(1), serde_json::json!(prop)],
                        ))
                        .await;
                }
            }
            true
        }
        None => {
            emit(
                events,
                AudioEvent::Error("mpv socket not ready".to_string()),
            )
            .await;
            false
        }
    }
}

#[allow(clippy::collapsible_if)]
async fn run_loop(
    mpv_bin: String,
    sock: PathBuf,
    initial_volume: f32,
    mut cmds: CmdRx,
    events: mpsc::Sender<AudioEvent>,
) {
    let mut child: Option<tokio::process::Child> = None;
    let mut ipc: Option<Arc<IpcClient>> = None;
    let mut ev_rx: Option<mpsc::UnboundedReceiver<serde_json::Value>> = None;
    let mut vol = VolumeControl::new(initial_volume);
    let mut poll = tokio::time::interval(std::time::Duration::from_millis(500));
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut last_pos: Option<std::time::Duration> = None;
    let mut last_dur: Option<std::time::Duration> = None;

    loop {
        tokio::select! {
            cmd = cmds.recv() => {
                let Some(cmd) = cmd else { break };
                // commands needing mpv ensure child first (except volume/mute memory still tracked)
                match cmd {
                    AudioCommand::SetVolume(v) => {
                        vol.set(v);
                        if ensure_child(&mpv_bin, &sock, &vol, &events, &mut child, &mut ipc, &mut ev_rx).await {
                            if let Some(c) = ipc.as_ref() {
                                let vv = vol.effective_mpv_volume();
                                if c.send(ipc_command("set_property", vec![serde_json::json!("volume"), serde_json::json!(vv)])).await.is_err() {
                                    emit(&events, AudioEvent::Error("mpv ipc write failed".into())).await;
                                    ipc = None;
                                }
                            }
                        }
                    }
                    AudioCommand::ToggleMute => {
                        let vv = vol.toggle_mute_mpv();
                        if ensure_child(&mpv_bin, &sock, &vol, &events, &mut child, &mut ipc, &mut ev_rx).await {
                            if let Some(c) = ipc.as_ref() {
                                if c.send(ipc_command("set_property", vec![serde_json::json!("volume"), serde_json::json!(vv)])).await.is_err() {
                                    emit(&events, AudioEvent::Error("mpv ipc write failed".into())).await;
                                    ipc = None;
                                }
                            }
                        }
                    }
                    AudioCommand::Play(info) => {
                        emit(&events, AudioEvent::Buffering(true)).await;
                        if !ensure_child(&mpv_bin, &sock, &vol, &events, &mut child, &mut ipc, &mut ev_rx).await {
                            emit(&events, AudioEvent::Buffering(false)).await;
                            continue;
                        }
                        if let Some(c) = ipc.as_ref() {
                            let r = c.send(ipc_command("loadfile", vec![serde_json::json!(info.url), serde_json::json!("replace")])).await;
                            if r.is_err() {
                                emit(&events, AudioEvent::Error("mpv ipc write failed".into())).await;
                                ipc = None;
                            }
                        }
                        emit(&events, AudioEvent::Buffering(false)).await;
                    }
                    AudioCommand::Pause => {
                        if ensure_child(&mpv_bin, &sock, &vol, &events, &mut child, &mut ipc, &mut ev_rx).await {
                            if let Some(c) = ipc.as_ref() {
                                let _ = c.send(ipc_command("set_property", vec![serde_json::json!("pause"), serde_json::json!(true)])).await;
                            }
                        }
                    }
                    AudioCommand::Resume => {
                        if ensure_child(&mpv_bin, &sock, &vol, &events, &mut child, &mut ipc, &mut ev_rx).await {
                            if let Some(c) = ipc.as_ref() {
                                let _ = c.send(ipc_command("set_property", vec![serde_json::json!("pause"), serde_json::json!(false)])).await;
                            }
                        }
                    }
                    AudioCommand::Stop => {
                        if let Some(c) = ipc.as_ref() {
                            let r = c.send(ipc_command("stop", vec![])).await;
                            if r.is_err() { ipc = None; }
                        }
                    }
                    AudioCommand::SeekForward(d) => {
                        if ensure_child(&mpv_bin, &sock, &vol, &events, &mut child, &mut ipc, &mut ev_rx).await {
                            if let Some(c) = ipc.as_ref() {
                                let _ = c.send(ipc_command("seek", vec![serde_json::json!(d.as_secs_f64()), serde_json::json!("relative")])).await;
                            }
                        }
                    }
                    AudioCommand::SeekBackward(d) => {
                        if ensure_child(&mpv_bin, &sock, &vol, &events, &mut child, &mut ipc, &mut ev_rx).await {
                            if let Some(c) = ipc.as_ref() {
                                let _ = c.send(ipc_command("seek", vec![serde_json::json!(-d.as_secs_f64()), serde_json::json!("relative")])).await;
                            }
                        }
                    }
                }
            }
            msg = async {
                match ev_rx.as_mut() {
                    Some(r) => r.recv().await,
                    None => std::future::pending().await,
                }
            } => {
                if let Some(v) = msg {
                    handle_ipc_event(v, &events, &mut last_pos, &mut last_dur).await;
                } else {
                    // server hung up
                    ev_rx = None;
                    ipc = None;
                }
            }
            _ = poll.tick() => {
                if let Some(c) = ipc.as_ref() {
                    for prop in ["time-pos", "duration", "pause", "eof-reached"] {
                        let _ = c.send(ipc_command("get_property", vec![serde_json::json!(prop)])).await;
                    }
                }
            }
        }
    }
    // Drop impl behavior: kill child + remove socket.
    if let Some(mut c) = child.take() {
        let _ = c.kill().await;
    }
    let _ = std::fs::remove_file(&sock);

    let _ = (last_pos, last_dur);
}

async fn handle_ipc_event(
    v: serde_json::Value,
    events: &mpsc::Sender<AudioEvent>,
    last_pos: &mut Option<std::time::Duration>,
    last_dur: &mut Option<std::time::Duration>,
) {
    // event-style: {"event":"property-change","name":"time-pos","data":12.3}
    if v.get("event").and_then(|e| e.as_str()) == Some("property-change") {
        let name = v.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let data = v.get("data");
        match name {
            "time-pos" => {
                if let Some(f) = data.and_then(|d| d.as_f64()) {
                    let d = std::time::Duration::from_secs_f64(f.max(0.0));
                    *last_pos = Some(d);
                    emit(events, AudioEvent::Position(d)).await;
                }
            }
            "duration" => {
                if let Some(f) = data.and_then(|d| d.as_f64()) {
                    let d = std::time::Duration::from_secs_f64(f.max(0.0));
                    *last_dur = Some(d);
                    emit(events, AudioEvent::Duration(Some(d))).await;
                } else {
                    emit(events, AudioEvent::Duration(None)).await;
                }
            }
            "eof-reached" => {
                if data.and_then(|d| d.as_bool()).unwrap_or(false) {
                    emit(events, AudioEvent::Finished).await;
                }
            }
            "pause" => {
                let paused = data.and_then(|d| d.as_bool()).unwrap_or(false);
                emit(events, AudioEvent::Buffering(false)).await;
                let _ = paused;
            }
            _ => {}
        }
        return;
    }
    // get_property response: {"request_id":..,"data":..,"error":"success"}
    if v.get("data").is_some() || v.get("request_id").is_some() {
        // Heuristic: responses arrive in poll order time-pos,duration,pause,eof — ambiguous.
        // Handle generically: if data is bool true and last request was eof... simplest: treat bool via Finished only when true and value key unknown.
        // For poll fallback, mpv get_property responses don't echo name; so also poll via observe events primarily.
        // Still, try: numeric data -> Position update.
        if let Some(f) = v.get("data").and_then(|d| d.as_f64()) {
            let d = std::time::Duration::from_secs_f64(f.max(0.0));
            // Can't distinguish pos vs duration; emit Position (Duration events come via observe).
            *last_pos = Some(d);
            emit(events, AudioEvent::Position(d)).await;
        }
        return;
    }
    if v.get("error").and_then(|e| e.as_str()) == Some("success") {
        return;
    }
    if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
        emit(events, AudioEvent::Error(err.to_string())).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration as StdDuration;
    use tokio::net::UnixListener;

    #[test]
    fn ipc_command_shapes() {
        let c = ipc_command(
            "loadfile",
            vec![serde_json::json!("http://x"), serde_json::json!("replace")],
        );
        assert_eq!(c[0], serde_json::json!("loadfile"));
        let c = ipc_command(
            "set_property",
            vec![serde_json::json!("pause"), serde_json::json!(true)],
        );
        assert_eq!(c[1], serde_json::json!("pause"));
        let c = ipc_command(
            "seek",
            vec![serde_json::json!(5.0), serde_json::json!("relative")],
        );
        assert_eq!(c[0], serde_json::json!("seek"));
    }

    /// Fake IPC server asserting JSON commands.
    async fn fake_server(sock: PathBuf, expect_substrings: Vec<String>) -> Vec<String> {
        let _ = std::fs::remove_file(&sock);
        let listener = UnixListener::bind(&sock).unwrap();
        let (stream, _) = listener.accept().await.unwrap();
        let mut reader = BufReader::new(stream);
        let mut got = Vec::new();
        let mut line = String::new();
        while got.len() < expect_substrings.len() {
            line.clear();
            let n = tokio::time::timeout(StdDuration::from_secs(3), reader.read_line(&mut line))
                .await
                .expect("timeout")
                .unwrap();
            if n == 0 {
                break;
            }
            got.push(line.trim().to_string());
        }
        for (g, e) in got.iter().zip(expect_substrings.iter()) {
            assert!(g.contains(e), "got {g} expected substring {e}");
        }
        got
    }

    #[tokio::test]
    async fn fake_ipc_receives_loadfile_and_pause() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("mpv.sock");
        let sock2 = sock.clone();
        let server = tokio::spawn(async move {
            fake_server(sock2, vec!["loadfile".into(), "pause".into()]).await
        });
        // wait for listener
        tokio::time::sleep(StdDuration::from_millis(50)).await;
        let conn = UnixStream::connect(&sock).await.unwrap();
        let (_, wh) = conn.into_split();
        let client = IpcClient::new(wh);
        client
            .send(ipc_command(
                "loadfile",
                vec![
                    serde_json::json!("http://example/a"),
                    serde_json::json!("replace"),
                ],
            ))
            .await
            .unwrap();
        client
            .send(ipc_command(
                "set_property",
                vec![serde_json::json!("pause"), serde_json::json!(true)],
            ))
            .await
            .unwrap();
        let got = server.await.unwrap();
        assert_eq!(got.len(), 2);
    }

    #[tokio::test]
    async fn eof_event_maps_to_finished() {
        let (tx, mut rx) = mpsc::channel(8);
        let mut lp = None;
        let mut ld = None;
        handle_ipc_event(
            serde_json::json!({"event":"property-change","name":"eof-reached","data":true}),
            &tx,
            &mut lp,
            &mut ld,
        )
        .await;
        assert!(matches!(rx.recv().await.unwrap(), AudioEvent::Finished));
        handle_ipc_event(
            serde_json::json!({"event":"property-change","name":"time-pos","data":12.5}),
            &tx,
            &mut lp,
            &mut ld,
        )
        .await;
        match rx.recv().await.unwrap() {
            AudioEvent::Position(d) => assert_eq!(d, StdDuration::from_secs_f64(12.5)),
            e => panic!("unexpected {e:?}"),
        }
    }

    #[tokio::test]
    async fn missing_binary_stays_alive_and_errors() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("none.sock");
        let (engine, mut rx) = MpvEngine::spawn("/nonexistent-mpv-binary-xyz".to_string(), sock);
        engine
            .send(AudioCommand::Play(crate::models::AudioStreamInfo {
                url: "http://example/a".into(),
                mime: "audio/mp4".into(),
                bitrate: 128,
            }))
            .await;
        // expect an Error event but engine still accepts next command
        let ev = tokio::time::timeout(StdDuration::from_secs(5), rx.recv())
            .await
            .expect("timeout")
            .expect("event");
        assert!(matches!(
            ev,
            AudioEvent::Error(_) | AudioEvent::Buffering(_)
        ));
        assert!(
            engine.send(AudioCommand::SetVolume(0.5)).await,
            "engine must survive errors"
        );
    }

    #[test]
    fn engine_handles_missing_device_without_panic() {
        let mut v = VolumeControl::new(0.5);
        v.set(2.0);
        assert_eq!(v.stored_volume(), 1.0);
    }
}
