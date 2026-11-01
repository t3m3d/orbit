//! Private child-process bridge for the Krypton macOS application.
//! Commands arrive over inherited stdin, never a network socket.
#[allow(dead_code)]
#[path = "../engine.rs"]
mod engine;
#[allow(dead_code)]
#[path = "../settings.rs"]
mod settings;
#[path = "../storage.rs"]
mod storage;

use anyhow::{Context, Result, bail};
use engine::{AddDraft, AddRequest, EngineCommand, EngineEvent, TorrentRow};
use serde::{Deserialize, Serialize};
use settings::{AppSettings, NetworkMode, StartupNetworkPolicy};
use std::{
    fs,
    io::{self, BufRead},
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    Preview {
        source: String,
    },
    Add {
        request: AddRequest,
    },
    DismissPreview,
    Pause {
        id: usize,
    },
    Resume {
        id: usize,
    },
    Forget {
        id: usize,
    },
    DeleteData {
        id: usize,
    },
    PauseAll,
    ResumeAll,
    Configure {
        mode: NetworkMode,
        destination: String,
        dht: bool,
        remember: bool,
        down_kib: u32,
        up_kib: u32,
    },
    Shutdown,
}

#[derive(Serialize)]
struct State {
    protocol: u32,
    revision: u64,
    status: String,
    error: Option<String>,
    settings: AppSettings,
    torrents: Vec<TorrentRow>,
    preview: Option<AddDraft>,
}

fn supported_source(source: &str) -> bool {
    let lower = source.to_ascii_lowercase();
    lower.starts_with("magnet:")
        || lower.starts_with("https://")
        || lower.starts_with("http://")
        || (!lower.contains("://")
            && std::path::Path::new(source)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("torrent")))
}

fn stop(tx: &mpsc::Sender<EngineCommand>, rx: &mpsc::Receiver<EngineEvent>) -> Result<()> {
    if tx.send(EngineCommand::Shutdown).is_err() {
        return Ok(());
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(EngineEvent::Stopped) | Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            _ => {}
        }
    }
    bail!("The torrent core is busy. Quit and reopen Orbit before changing network mode.")
}

fn main() -> Result<()> {
    let state_path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .context("missing private state-file path")?,
    );
    fs::create_dir_all(settings::data_dir())?;
    // A second frontend must not write the same resume database concurrently.
    #[cfg(unix)]
    let _lock = {
        use std::os::fd::AsRawFd;
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(settings::data_dir().join("engine.lock"))?;
        // flock holds an advisory lock for this file descriptor's lifetime.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            bail!("Orbit is already running with this data directory.");
        }
        file
    };
    let mut saved = settings::load_settings().context("could not read Orbit settings")?;
    if !saved.network_setup_completed
        || saved.startup_network_policy != StartupNetworkPolicy::RememberChoice
    {
        saved.network_mode = Some(NetworkMode::Offline);
        saved.sharing_consent_recorded = false;
        saved.block_all_uploads = true;
    }
    let mut state = State {
        protocol: 1,
        revision: 0,
        status: "Starting torrent core".into(),
        error: None,
        settings: saved,
        torrents: vec![],
        preview: None,
    };
    let context = eframe::egui::Context::default();
    let (mut tx, mut rx) = engine::start_engine(
        PathBuf::from(&state.settings.default_download_dir),
        settings::data_dir(),
        state.settings.clone(),
        context.clone(),
    );
    let (input_tx, input_rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            match line {
                Ok(line) if line.len() <= 1024 * 1024 => {
                    if input_tx
                        .send(serde_json::from_str::<Command>(&line).map_err(|e| e.to_string()))
                        .is_err()
                    {
                        break;
                    }
                }
                _ => break,
            }
        }
        let _ = input_tx.send(Ok(Command::Shutdown));
    });
    let mut running = true;
    while running {
        while let Ok(event) = rx.try_recv() {
            match event {
                EngineEvent::Starting(_) => state.status = "Starting torrent core".into(),
                EngineEvent::NetworkLocked => {
                    state.status = "Offline — torrent networking is off".into()
                }
                EngineEvent::Ready {
                    uploads_blocked, ..
                } => {
                    state.status = if uploads_blocked {
                        "Download Only — piece uploads blocked"
                    } else {
                        "Standard — sharing enabled"
                    }
                    .into()
                }
                EngineEvent::Snapshot(rows) => state.torrents = rows,
                EngineEvent::PreviewReady(draft) => {
                    state.preview = Some(draft);
                    state.error = None;
                }
                EngineEvent::Notice(message) => state.status = message,
                EngineEvent::Completion { name, action } => {
                    state.status = format!("{name}: {action}")
                }
                EngineEvent::Error(message)
                | EngineEvent::Fatal(message)
                | EngineEvent::PreviewFailed { message, .. } => state.error = Some(message),
                EngineEvent::Stopped => {}
            }
        }
        while let Ok(command) = input_rx.try_recv() {
            let result = (|| -> Result<()> {
                let command = command.map_err(anyhow::Error::msg)?;
                match command {
                    Command::Shutdown => {
                        running = false;
                    }
                    Command::Preview { source } => {
                        if state.settings.effective_network_mode() == NetworkMode::Offline {
                            bail!("Choose Download Only or Standard before importing a torrent.");
                        }
                        if !supported_source(source.trim()) {
                            bail!("Choose a .torrent file, magnet link, or HTTP(S) torrent URL.");
                        }
                        state.preview = None;
                        state.status = "Reading torrent metadata…".into();
                        tx.send(EngineCommand::Preview(source.trim().to_owned()))?;
                    }
                    Command::Add { request } => {
                        let draft = state
                            .preview
                            .as_ref()
                            .context("Preview this torrent before adding it")?;
                        if request.info_hash != draft.info_hash || request.source != draft.source {
                            bail!("The torrent preview changed. Preview it again.");
                        }
                        if request.selected_files.is_empty()
                            || request
                                .selected_files
                                .iter()
                                .any(|i| !draft.files.iter().any(|f| f.index == *i))
                        {
                            bail!("Choose at least one valid file.");
                        }
                        let bytes = draft
                            .files
                            .iter()
                            .filter(|f| request.selected_files.contains(&f.index))
                            .map(|f| f.length)
                            .sum();
                        storage::storage_preflight(&request.output_folder, bytes)
                            .map_err(anyhow::Error::msg)?;
                        tx.send(EngineCommand::Add(request))?;
                        state.preview = None;
                    }
                    Command::DismissPreview => state.preview = None,
                    Command::Pause { id } => tx.send(EngineCommand::Pause(id))?,
                    Command::Resume { id } => tx.send(EngineCommand::Resume(id))?,
                    Command::Forget { id } => tx.send(EngineCommand::Forget(id))?,
                    Command::DeleteData { id } => tx.send(EngineCommand::DeleteData(id))?,
                    Command::PauseAll => tx.send(EngineCommand::PauseAll)?,
                    Command::ResumeAll => tx.send(EngineCommand::ResumeAll)?,
                    Command::Configure {
                        mode,
                        destination,
                        dht,
                        remember,
                        down_kib,
                        up_kib,
                    } => {
                        storage::storage_preflight(&destination, 0).map_err(anyhow::Error::msg)?;
                        let mut settings = state.settings.clone();
                        settings.network_mode = Some(mode);
                        settings.sharing_consent_recorded = true;
                        settings.network_setup_completed = true;
                        settings.block_all_uploads = mode != NetworkMode::Standard;
                        settings.startup_network_policy = if remember {
                            StartupNetworkPolicy::RememberChoice
                        } else {
                            StartupNetworkPolicy::AskEveryStart
                        };
                        settings.default_download_dir = destination;
                        settings.dht_enabled = dht;
                        settings.download_limit_kib = down_kib;
                        settings.upload_limit_kib = up_kib;
                        // This frontend has no background controls; always quit with its window.
                        settings.close_to_tray = false;
                        stop(&tx, &rx)?;
                        // A save failure leaves the old core stopped, never starts unpersisted networking.
                        settings::save_settings(&settings)?;
                        (tx, rx) = engine::start_engine(
                            PathBuf::from(&settings.default_download_dir),
                            settings::data_dir(),
                            settings.clone(),
                            context.clone(),
                        );
                        state.settings = settings;
                        state.torrents.clear();
                        state.preview = None;
                        state.status = "Applying network settings…".into();
                    }
                }
                state.error = None;
                Ok(())
            })();
            if let Err(error) = result {
                state.error = Some(format!("{error:#}"));
            }
        }
        state.revision += 1;
        let temporary = state_path.with_extension("next");
        fs::write(&temporary, serde_json::to_vec(&state)?)?;
        fs::rename(temporary, &state_path)?;
        std::thread::sleep(Duration::from_millis(150));
    }
    stop(&tx, &rx)?;
    Ok(())
}
