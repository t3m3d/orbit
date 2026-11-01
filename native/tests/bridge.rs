//! Real bridge and BitTorrent tests. All traffic stays on localhost.
#![cfg(unix)]

use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Bridge {
    root: PathBuf,
    child: Option<Child>,
}

impl Bridge {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("orbit-bridge-{}-{unique}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let mut bridge = Self { root, child: None };
        bridge.start();
        bridge
    }

    fn start(&mut self) {
        // A previous snapshot must not satisfy a restarted child's readiness check.
        let _ = fs::remove_file(self.root.join("state.json"));
        self.child = Some(
            Command::new(env!("CARGO_BIN_EXE_orbit-engine"))
                .arg(self.root.join("state.json"))
                .env("ORBIT_DATA_DIR", self.root.join("data"))
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::inherit())
                .spawn()
                .unwrap(),
        );
        self.wait(|s| text(s, "status").starts_with("Offline"));
    }

    fn send(&mut self, command: Value) {
        let input = self.child.as_mut().unwrap().stdin.as_mut().unwrap();
        serde_json::to_writer(&mut *input, &command).unwrap();
        input.write_all(b"\n").unwrap();
        input.flush().unwrap();
    }

    fn wait(&mut self, predicate: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut last = Value::Null;
        while Instant::now() < deadline {
            assert!(
                self.child.as_mut().unwrap().try_wait().unwrap().is_none(),
                "core exited"
            );
            if let Ok(bytes) = fs::read(self.root.join("state.json"))
                && let Ok(state) = serde_json::from_slice::<Value>(&bytes)
            {
                if predicate(&state) {
                    return state;
                }
                last = state;
            }
            thread::sleep(Duration::from_millis(40));
        }
        panic!("Timed out; last state: {last}");
    }

    fn configure(&mut self, mode: &str, remember: bool) -> Value {
        self.send(json!({"action":"configure", "mode":mode,
            "destination":self.root.join("downloads"), "dht":false,
            "remember":remember, "down_kib":0, "up_kib":0}));
        self.wait(|s| {
            s["settings"]["network_mode"] == mode
                && s["settings"]["network_setup_completed"] == true
                && s["settings"]["startup_network_policy"]
                    == if remember {
                        "remember_choice"
                    } else {
                        "ask_every_start"
                    }
                && text(s, "status").starts_with(if mode == "offline" {
                    "Offline"
                } else {
                    "Download Only"
                })
        })
    }

    fn finish(&mut self) {
        let child = self.child.as_mut().unwrap();
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "core exited with {status}");
                self.child = None;
                return;
            }
            assert!(Instant::now() < deadline, "core did not shut down");
            thread::sleep(Duration::from_millis(40));
        }
    }

    fn stop(&mut self) {
        self.send(json!({"action":"shutdown"}));
        self.finish();
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            child.stdin.take();
            let deadline = Instant::now() + Duration::from_secs(8);
            while Instant::now() < deadline {
                if child.try_wait().ok().flatten().is_some() {
                    break;
                }
                thread::sleep(Duration::from_millis(40));
            }
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

#[test]
fn offline_validation_persistence_and_eof_shutdown() {
    let mut bridge = Bridge::new();
    let state = bridge.wait(|s| text(s, "status").starts_with("Offline"));
    assert_eq!(state["protocol"], 1);
    assert_eq!(state["torrents"], json!([]));
    assert!(!bridge.root.join("data/session").exists());
    bridge.send(json!({"action":"preview", "source":"magnet:?xt=urn:btih:abc"}));
    bridge.wait(|s| text(s, "error").contains("Choose Download Only"));
    bridge.send(
        json!({"action":"configure", "mode":"offline", "destination":"relative",
        "dht":false, "remember":false, "down_kib":0, "up_kib":0}),
    );
    bridge.wait(|s| text(s, "error").contains("absolute path"));
    bridge.configure("offline", true);
    let saved: Value =
        serde_json::from_slice(&fs::read(bridge.root.join("data/settings.json")).unwrap()).unwrap();
    assert_eq!(saved["network_mode"], "offline");
    assert_eq!(saved["dht_enabled"], false);
    assert_eq!(saved["close_to_tray"], false);
    bridge.child.as_mut().unwrap().stdin.take();
    bridge.finish();
}

#[test]
fn second_core_cannot_share_resume_database() {
    let bridge = Bridge::new();
    let mut other = Command::new(env!("CARGO_BIN_EXE_orbit-engine"))
        .arg(bridge.root.join("other.json"))
        .env("ORBIT_DATA_DIR", bridge.root.join("data"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = other.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = other.kill();
            panic!("second core did not reject lock");
        }
        thread::sleep(Duration::from_millis(40));
    };
    assert!(!status.success());
    let mut error = String::new();
    other
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut error)
        .unwrap();
    assert!(error.contains("already running"));
}

struct LocalSwarm {
    payload: Arc<Vec<u8>>,
    tracker_port: u16,
    stopped: Arc<AtomicBool>,
    workers: Vec<JoinHandle<()>>,
}

impl LocalSwarm {
    fn new() -> Self {
        let payload = Arc::new(b"Orbit localhost integration fixture.\n".repeat(900));
        let peer = TcpListener::bind("127.0.0.1:0").unwrap();
        let tracker = TcpListener::bind("127.0.0.1:0").unwrap();
        peer.set_nonblocking(true).unwrap();
        tracker.set_nonblocking(true).unwrap();
        let peer_port = peer.local_addr().unwrap().port();
        let tracker_port = tracker.local_addr().unwrap().port();
        let stopped = Arc::new(AtomicBool::new(false));
        let mut workers = Vec::new();
        let flag = stopped.clone();
        let data = payload.clone();
        workers.push(thread::spawn(move || {
            while !flag.load(Ordering::Relaxed) {
                if let Ok((mut stream, _)) = peer.accept() {
                    // BSD accept can inherit O_NONBLOCK from the listener.
                    let _ = stream.set_nonblocking(false);
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
                    let _ = serve_peer(&mut stream, &data, &flag);
                } else {
                    thread::sleep(Duration::from_millis(10));
                }
            }
        }));
        let flag = stopped.clone();
        workers.push(thread::spawn(move || {
            while !flag.load(Ordering::Relaxed) {
                if let Ok((mut stream, _)) = tracker.accept() {
                    let _ = stream.set_nonblocking(false);
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
                    let _ = serve_tracker(&mut stream, peer_port);
                } else {
                    thread::sleep(Duration::from_millis(10));
                }
            }
        }));
        Self {
            payload,
            tracker_port,
            stopped,
            workers,
        }
    }

    fn torrent(&self) -> Vec<u8> {
        let announce = format!("http://127.0.0.1:{}/announce", self.tracker_port);
        let mut bytes = format!("d8:announce{}:{announce}4:infod6:lengthi{}e4:name11:fixture.txt12:piece lengthi65536e6:pieces20:", announce.len(), self.payload.len()).into_bytes();
        // SHA-1 of the fixed payload above, one piece. No extra hash dependency.
        bytes.extend_from_slice(&[
            105, 14, 172, 245, 134, 154, 253, 210, 37, 215, 112, 247, 12, 180, 143, 164, 36, 201,
            234, 99,
        ]);
        bytes.extend_from_slice(b"7:privatei1eee");
        bytes
    }
}

impl Drop for LocalSwarm {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

fn serve_tracker(stream: &mut TcpStream, peer_port: u16) -> std::io::Result<()> {
    let mut request = Vec::new();
    let mut byte = [0];
    while !request.ends_with(b"\r\n\r\n") && request.len() < 16384 {
        stream.read_exact(&mut byte)?;
        request.push(byte[0]);
    }
    let mut body = b"d8:completei1e10:incompletei0e8:intervali1e5:peers6:".to_vec();
    body.extend_from_slice(&[127, 0, 0, 1]);
    body.extend_from_slice(&peer_port.to_be_bytes());
    body.push(b'e');
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(&body)
}

fn serve_peer(stream: &mut TcpStream, payload: &[u8], stopped: &AtomicBool) -> std::io::Result<()> {
    let mut handshake = [0; 68];
    stream.read_exact(&mut handshake)?;
    handshake[48..].copy_from_slice(b"-ORTEST-123456789012");
    stream.write_all(&handshake)?;
    stream.write_all(&[0, 0, 0, 2, 5, 128])?; // bitfield: one complete piece
    stream.write_all(&[0, 0, 0, 1, 1])?; // unchoke
    while !stopped.load(Ordering::Relaxed) {
        let mut length = [0; 4];
        stream.read_exact(&mut length)?;
        let length = u32::from_be_bytes(length) as usize;
        if length == 0 {
            continue;
        }
        if length > 65536 {
            return Err(std::io::Error::other("oversized peer message"));
        }
        let mut message = vec![0; length];
        stream.read_exact(&mut message)?;
        if message[0] == 6 && message.len() == 13 {
            let piece = u32::from_be_bytes(message[1..5].try_into().unwrap());
            let offset = u32::from_be_bytes(message[5..9].try_into().unwrap()) as usize;
            let count = u32::from_be_bytes(message[9..13].try_into().unwrap()) as usize;
            let Some(block) = payload
                .get(offset..offset.saturating_add(count))
                .filter(|_| piece == 0)
            else {
                return Err(std::io::Error::other("invalid fixture piece request"));
            };
            stream.write_all(&((9 + block.len()) as u32).to_be_bytes())?;
            stream.write_all(&[7])?;
            stream.write_all(&message[1..9])?;
            stream.write_all(block)?;
        }
    }
    Ok(())
}

#[test]
fn local_transfer_pause_recovery_and_keep_data() {
    let mut bridge = Bridge::new();
    let swarm = LocalSwarm::new();
    let source = bridge.root.join("fixture.torrent");
    fs::write(&source, swarm.torrent()).unwrap();
    bridge.configure("download_only", false);
    bridge.send(json!({"action":"preview", "source":"ftp://example.invalid/a.torrent"}));
    bridge.wait(|s| text(s, "error").contains("HTTP(S)"));
    bridge.send(json!({"action":"preview", "source":source}));
    let state = bridge.wait(|s| s["preview"].is_object());
    let mut request = state["preview"].as_object().unwrap().clone();
    assert_eq!(request.remove("total").unwrap(), swarm.payload.len());
    let files = request.remove("files").unwrap();
    request.insert(
        "selected_files".into(),
        files
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["index"].clone())
            .collect(),
    );
    request.insert("start_paused".into(), json!(true));
    bridge.send(json!({"action":"add", "request":request}));
    let state = bridge.wait(|s| text(&s["torrents"][0], "state").eq_ignore_ascii_case("paused"));
    bridge.send(json!({"action":"resume", "id":state["torrents"][0]["id"]}));
    let state = bridge.wait(|s| s["torrents"][0]["finished"] == true);
    let row = &state["torrents"][0];
    assert_eq!(row["uploaded"], 0);
    let output = PathBuf::from(text(row, "output_folder")).join("fixture.txt");
    assert_eq!(fs::read(&output).unwrap(), *swarm.payload);
    bridge.stop();
    bridge.start();
    let state = bridge.wait(|s| text(s, "status").starts_with("Offline"));
    assert_eq!(state["torrents"], json!([]));
    bridge.configure("download_only", false);
    let state = bridge.wait(|s| s["torrents"][0]["finished"] == true);
    bridge.send(json!({"action":"forget", "id":state["torrents"][0]["id"]}));
    bridge.wait(|s| s["torrents"] == json!([]) && s["preview"].is_null());
    assert_eq!(fs::read(output).unwrap(), *swarm.payload);
    bridge.stop();
}
