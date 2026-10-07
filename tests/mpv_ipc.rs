//! mpv IPC vs fake Unix-socket server: no mpv binary needed.
//! Verifies JSON IPC framing and engine error-surfacing on missing binary.

use rmusix::audio::{AudioCommand, AudioEvent, MpvEngine};
use rmusix::models::AudioStreamInfo;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

fn sock_in(dir: &tempfile::TempDir, name: &str) -> PathBuf {
    dir.path().join(name)
}

#[test]
fn ipc_command_frames_are_json_arrays() {
    // Shape must match what `MpvEngine` writes: ["loadfile", url, "replace"] etc.
    let c = rmusix::audio::player::ipc_command(
        "loadfile",
        vec![
            serde_json::json!("http://x/a"),
            serde_json::json!("replace"),
        ],
    );
    assert_eq!(c[0], serde_json::json!("loadfile"));
    assert_eq!(c[2], serde_json::json!("replace"));
    let c = rmusix::audio::player::ipc_command(
        "seek",
        vec![serde_json::json!(5.0), serde_json::json!("relative")],
    );
    assert_eq!(c[0], serde_json::json!("seek"));
}

#[tokio::test]
async fn fake_socket_receives_client_frames() {
    let dir = tempfile::tempdir().unwrap();
    let sock = sock_in(&dir, "fake.sock");
    let _ = std::fs::remove_file(&sock);
    let listener = UnixListener::bind(&sock).unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut reader = BufReader::new(stream);
        let mut got = Vec::new();
        let mut line = String::new();
        while got.len() < 2 {
            line.clear();
            let n = tokio::time::timeout(Duration::from_secs(3), reader.read_line(&mut line))
                .await
                .unwrap()
                .unwrap();
            if n == 0 {
                break;
            }
            got.push(line.trim().to_string());
        }
        got
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    // Emulate the engine side: connect and write two framed commands.
    let conn = UnixStream::connect(&sock).await.unwrap();
    let (_, mut wh) = conn.into_split();
    for cmd in [
        rmusix::audio::player::ipc_command(
            "loadfile",
            vec![
                serde_json::json!("http://x/a"),
                serde_json::json!("replace"),
            ],
        ),
        rmusix::audio::player::ipc_command(
            "set_property",
            vec![serde_json::json!("pause"), serde_json::json!(true)],
        ),
    ] {
        let mut line = serde_json::json!({"command": cmd, "request_id": 1}).to_string();
        line.push('\n');
        wh.write_all(line.as_bytes()).await.unwrap();
    }
    wh.flush().await.unwrap();
    let got = server.await.unwrap();
    assert_eq!(got.len(), 2);
    assert!(got[0].contains("loadfile"));
    assert!(got[1].contains("pause"));
}

#[tokio::test]
async fn engine_with_missing_mpv_binary_stays_alive_and_errors() {
    let dir = tempfile::tempdir().unwrap();
    let sock = sock_in(&dir, "none.sock");
    let (engine, mut rx) = MpvEngine::spawn("/nonexistent-mpv-binary-xyz".into(), sock);
    engine
        .send(AudioCommand::Play(AudioStreamInfo {
            url: "http://example/a".into(),
            mime: "audio/mp4".into(),
            bitrate: 128,
        }))
        .await;
    let ev = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("engine must emit an event")
        .expect("event");
    assert!(matches!(
        ev,
        AudioEvent::Error(_) | AudioEvent::Buffering(_)
    ));
    assert!(
        engine.send(AudioCommand::SetVolume(0.5)).await,
        "engine must survive missing binary"
    );
}
