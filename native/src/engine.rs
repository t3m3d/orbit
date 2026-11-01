#[path = "engine/completion.rs"]
mod completion;

use std::{
    collections::{HashMap, HashSet},
    fs,
    num::NonZeroU32,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
/// UI-independent wake-up callback. The private bridge uses a no-op.
pub type Repaint = std::sync::Arc<dyn Fn() + Send + Sync>;
use librqbit::{
    AddTorrent, AddTorrentOptions, Api, Session, SessionOptions, SessionPersistenceConfig,
    api::{ApiTorrentListOpts, TorrentIdOrHash},
    limits::LimitsConfig,
};
use sha2::{Digest, Sha256};

use crate::settings::{
    AppSettings, SharePolicy, TransferPolicy, TransferPolicyStore, load_policies, persistence_dir,
    save_policies,
};

#[derive(Clone, Debug, serde::Serialize)]
pub struct TorrentFileRow {
    pub index: usize,
    pub name: String,
    pub length: u64,
    pub included: bool,
    pub downloaded: u64,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct TorrentRow {
    pub id: usize,
    pub info_hash: String,
    pub name: String,
    pub output_folder: String,
    pub progress: f32,
    pub downloaded: u64,
    pub uploaded: u64,
    pub total: u64,
    pub down_speed: String,
    pub up_speed: String,
    pub down_speed_mib: f64,
    pub up_speed_mib: f64,
    pub eta: String,
    pub state: String,
    pub status: String,
    pub connected_peers: usize,
    pub discovered_peers: usize,
    pub connecting_peers: usize,
    pub queued_peers: usize,
    pub failed_peers: usize,
    pub finished: bool,
    pub error: Option<String>,
    pub files: Vec<TorrentFileRow>,
    pub policy: TransferPolicy,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct PreviewFile {
    pub index: usize,
    pub name: String,
    pub length: u64,
    pub selected: bool,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct AddDraft {
    pub source: String,
    pub info_hash: String,
    pub name: String,
    pub output_folder: String,
    pub total: u64,
    pub files: Vec<PreviewFile>,
    pub start_paused: bool,
    pub share_policy: SharePolicy,
    pub seed_ratio_percent: u32,
    pub seed_time_minutes: u32,
    pub delete_source_when_complete: bool,
    pub upload_limit_kib: u32,
    pub download_limit_kib: u32,
    pub tag: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AddRequest {
    pub source: String,
    pub info_hash: String,
    pub name: String,
    pub output_folder: String,
    pub selected_files: Vec<usize>,
    pub start_paused: bool,
    pub share_policy: SharePolicy,
    pub seed_ratio_percent: u32,
    pub seed_time_minutes: u32,
    pub delete_source_when_complete: bool,
    pub upload_limit_kib: u32,
    pub download_limit_kib: u32,
    pub tag: String,
}

#[derive(Clone, Debug)]
pub enum EngineEvent {
    Starting(PathBuf),
    NetworkLocked,
    Ready {
        uploads_blocked: bool,
        listen_port: Option<u16>,
    },
    Snapshot(Vec<TorrentRow>),
    PreviewReady(AddDraft),
    PreviewFailed {
        source: String,
        message: String,
    },
    Completion {
        name: String,
        action: String,
    },
    Notice(String),
    Error(String),
    Fatal(String),
    Stopped,
}

#[derive(Clone, Debug)]
pub enum EngineCommand {
    Preview(String),
    Add(AddRequest),
    Pause(usize),
    Resume(usize),
    Forget(usize),
    DeleteData(usize),
    UpdateFiles {
        id: usize,
        only_files: Vec<usize>,
    },
    UpdatePolicy {
        info_hash: String,
        policy: TransferPolicy,
    },
    SetGlobalLimits {
        down_kib: u32,
        up_kib: u32,
    },
    SetMaxActive(usize),
    PauseAll,
    ResumeAll,
    Shutdown,
}

fn emit(event_tx: &Sender<EngineEvent>, repaint: &Repaint, event: EngineEvent) {
    let _ = event_tx.send(event);
    repaint();
}

fn limit_from_kib(kib: u32) -> Option<NonZeroU32> {
    if kib == 0 {
        None
    } else {
        NonZeroU32::new(kib.saturating_mul(1024))
    }
}

fn limits_from_kib(download_kib: u32, upload_kib: u32) -> LimitsConfig {
    LimitsConfig {
        download_bps: limit_from_kib(download_kib),
        upload_bps: limit_from_kib(upload_kib),
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn normalized_status(
    raw_state: &str,
    finished: bool,
    error: Option<&str>,
    policy: SharePolicy,
    uploads_blocked: bool,
) -> String {
    if error.is_some() || raw_state.eq_ignore_ascii_case("error") {
        "Error"
    } else if raw_state.eq_ignore_ascii_case("initializing") {
        "Starting"
    } else if finished {
        if policy == SharePolicy::Seed
            && !uploads_blocked
            && !raw_state.eq_ignore_ascii_case("paused")
        {
            "Seeding"
        } else {
            "Complete"
        }
    } else if raw_state.eq_ignore_ascii_case("paused") {
        "Paused"
    } else {
        "Downloading"
    }
    .to_owned()
}

fn local_torrent_source(source: &str) -> Option<PathBuf> {
    let path = Path::new(source);
    let is_torrent = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("torrent"));
    if !is_torrent || !path.is_file() {
        return None;
    }

    // Keep one resolved, concrete file path. Completion cleanup never scans a
    // folder, follows a pattern, or guesses which metainfo file was imported.
    Some(fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()))
}

fn torrent_source_sha256(path: &Path) -> Result<String> {
    let bytes = fs::read(path)
        .with_context(|| format!("could not fingerprint source {}", path.display()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn fallback_name(source: &str, info_hash: &str) -> String {
    Path::new(source)
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| info_hash.to_owned())
}

async fn preview_torrent(api: &Api, source: String, settings: &AppSettings) -> Result<AddDraft> {
    let input = AddTorrent::from_cli_argument(&source)
        .with_context(|| format!("could not read torrent source {source:?}"))?;
    let response = api
        .api_add_torrent(
            input,
            Some(AddTorrentOptions {
                list_only: true,
                ..Default::default()
            }),
        )
        .await
        .map_err(anyhow::Error::from)
        .with_context(|| format!("could not preview {source:?}"))?;

    let info_hash = response.details.info_hash;
    let name = response
        .details
        .name
        .unwrap_or_else(|| fallback_name(&source, &info_hash));
    let files = response
        .details
        .files
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(index, file)| PreviewFile {
            index,
            name: file.name,
            length: file.length,
            selected: file.included,
        })
        .collect::<Vec<_>>();
    let total = files.iter().map(|file| file.length).sum();
    let has_local_source = local_torrent_source(&source).is_some();

    Ok(AddDraft {
        source,
        info_hash,
        name,
        output_folder: response.output_folder,
        total,
        files,
        start_paused: settings.start_paused,
        share_policy: settings.default_share_policy,
        seed_ratio_percent: settings.default_seed_ratio_percent,
        seed_time_minutes: settings.default_seed_time_minutes,
        delete_source_when_complete: settings.auto_delete_torrent && has_local_source,
        upload_limit_kib: 0,
        download_limit_kib: 0,
        tag: String::new(),
    })
}

async fn add_torrent(
    api: &Api,
    request: AddRequest,
    policies: &mut TransferPolicyStore,
    user_paused: &mut HashSet<usize>,
) -> Result<String> {
    if request.selected_files.is_empty() {
        bail!("select at least one file before starting the transfer");
    }
    if api
        .api_torrent_list()
        .torrents
        .iter()
        .any(|torrent| torrent.info_hash == request.info_hash)
    {
        bail!("this torrent is already managed by Orbit");
    }

    let preflight_input = AddTorrent::from_cli_argument(&request.source)
        .with_context(|| format!("could not re-read torrent source {:?}", request.source))?;
    let preflight = api
        .api_add_torrent(
            preflight_input,
            Some(AddTorrentOptions {
                list_only: true,
                ..Default::default()
            }),
        )
        .await
        .map_err(anyhow::Error::from)
        .with_context(|| format!("could not verify {:?}", request.source))?;
    if preflight.details.info_hash != request.info_hash {
        bail!("torrent changed after preview; review it again before adding");
    }

    let input = AddTorrent::from_cli_argument(&request.source)
        .with_context(|| format!("could not read torrent source {:?}", request.source))?;
    let response = api
        .api_add_torrent(
            input,
            Some(AddTorrentOptions {
                paused: request.start_paused,
                only_files: Some(request.selected_files),
                output_folder: (!request.output_folder.trim().is_empty())
                    .then(|| request.output_folder.clone()),
                ratelimits: limits_from_kib(request.download_limit_kib, request.upload_limit_kib),
                ..Default::default()
            }),
        )
        .await
        .map_err(anyhow::Error::from)
        .with_context(|| format!("could not add {:?}", request.source))?;

    if response.details.info_hash != request.info_hash {
        bail!(
            "torrent changed after preview (expected {}, received {})",
            request.info_hash,
            response.details.info_hash
        );
    }

    if request.start_paused
        && let Some(id) = response.id
    {
        user_paused.insert(id);
    }

    let source_torrent = local_torrent_source(&request.source);
    let source_torrent_sha256 = source_torrent
        .as_deref()
        .map(torrent_source_sha256)
        .transpose()?;
    let policy = TransferPolicy {
        share_policy: request.share_policy,
        seed_ratio_percent: request.seed_ratio_percent,
        seed_time_minutes: request.seed_time_minutes,
        completed_at_unix: None,
        seed_limit_reached: false,
        added_at_unix: unix_now(),
        delete_source_when_complete: request.delete_source_when_complete
            && source_torrent.is_some(),
        source_torrent,
        source_torrent_sha256,
        completion_notified: false,
        source_deleted: false,
        tag: request.tag,
    };
    policies.insert(response.details.info_hash.clone(), policy);
    if let Err(error) = save_policies(policies) {
        if let Some(id) = response.id {
            let _ = api.api_torrent_action_pause(TorrentIdOrHash::Id(id)).await;
            user_paused.insert(id);
        }
        bail!("transfer was added but its policy could not be saved; transfer paused: {error:#}");
    }

    let display_name = response
        .details
        .name
        .filter(|name| !name.is_empty())
        .unwrap_or(request.name);
    Ok(format!("Added {display_name}"))
}

fn snapshot(
    api: &Api,
    policies: &mut TransferPolicyStore,
    default_share_policy: SharePolicy,
    default_seed_ratio_percent: u32,
    default_seed_time_minutes: u32,
    uploads_blocked: bool,
) -> (Vec<TorrentRow>, bool) {
    let list = api.api_torrent_list_ext(ApiTorrentListOpts { with_stats: true });
    let mut policy_store_changed = false;
    let mut rows = Vec::with_capacity(list.torrents.len());

    for torrent in list.torrents {
        let Some(id) = torrent.id else { continue };
        let info_hash = torrent.info_hash;
        let name = torrent.name.unwrap_or_else(|| format!("Torrent {id}"));
        let output_folder = torrent.output_folder;

        let (
            progress,
            downloaded,
            uploaded,
            total,
            down_speed,
            up_speed,
            down_speed_mib,
            up_speed_mib,
            connected_peers,
            discovered_peers,
            connecting_peers,
            queued_peers,
            failed_peers,
            eta,
            state,
            finished,
            error,
            file_progress,
        ) = if let Some(stats) = torrent.stats {
            let progress = if stats.total_bytes > 0 {
                stats.progress_bytes as f32 / stats.total_bytes as f32
            } else if stats.finished {
                1.0
            } else {
                0.0
            };
            let (
                down_speed,
                up_speed,
                down_speed_mib,
                up_speed_mib,
                connected_peers,
                discovered_peers,
                connecting_peers,
                queued_peers,
                failed_peers,
                eta,
            ) = stats
                .live
                .as_ref()
                .map(|live| {
                    (
                        live.download_speed.to_string(),
                        live.upload_speed.to_string(),
                        live.download_speed.mbps,
                        live.upload_speed.mbps,
                        live.snapshot.peer_stats.live,
                        live.snapshot.peer_stats.seen,
                        live.snapshot.peer_stats.connecting,
                        live.snapshot.peer_stats.queued,
                        live.snapshot.peer_stats.dead,
                        live.time_remaining
                            .as_ref()
                            .map(ToString::to_string)
                            .unwrap_or_default(),
                    )
                })
                .unwrap_or_else(|| {
                    (
                        "0.00 MiB/s".to_owned(),
                        "0.00 MiB/s".to_owned(),
                        0.0,
                        0.0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        String::new(),
                    )
                });
            (
                progress.clamp(0.0, 1.0),
                stats.progress_bytes,
                stats.uploaded_bytes,
                stats.total_bytes,
                down_speed,
                up_speed,
                down_speed_mib,
                up_speed_mib,
                connected_peers,
                discovered_peers,
                connecting_peers,
                queued_peers,
                failed_peers,
                eta,
                stats.state.to_string(),
                stats.finished,
                stats.error,
                stats.file_progress,
            )
        } else {
            (
                0.0,
                0,
                0,
                0,
                "0.00 MiB/s".to_owned(),
                "0.00 MiB/s".to_owned(),
                0.0,
                0.0,
                0,
                0,
                0,
                0,
                0,
                String::new(),
                "initializing".to_owned(),
                false,
                None,
                Vec::new(),
            )
        };

        let files = api
            .api_torrent_details(TorrentIdOrHash::Id(id))
            .ok()
            .and_then(|details| details.files)
            .unwrap_or_default()
            .into_iter()
            .enumerate()
            .map(|(index, file)| TorrentFileRow {
                index,
                name: file.name,
                length: file.length,
                included: file.included,
                downloaded: file_progress
                    .get(index)
                    .copied()
                    .unwrap_or_default()
                    .min(file.length),
            })
            .collect();

        let policy = policies.entry(info_hash.clone()).or_insert_with(|| {
            policy_store_changed = true;
            TransferPolicy {
                share_policy: default_share_policy,
                seed_ratio_percent: default_seed_ratio_percent,
                seed_time_minutes: default_seed_time_minutes,
                added_at_unix: unix_now(),
                ..Default::default()
            }
        });
        if policy.added_at_unix == 0 {
            policy.added_at_unix = unix_now();
            policy_store_changed = true;
        }
        let policy = policy.clone();
        let status = normalized_status(
            &state,
            finished,
            error.as_deref(),
            policy.share_policy,
            uploads_blocked,
        );

        rows.push(TorrentRow {
            id,
            info_hash,
            name,
            output_folder,
            progress,
            downloaded,
            uploaded,
            total,
            down_speed,
            up_speed,
            down_speed_mib,
            up_speed_mib,
            eta,
            state,
            status,
            connected_peers,
            discovered_peers,
            connecting_peers,
            queued_peers,
            failed_peers,
            finished,
            error,
            files,
            policy,
        });
    }
    rows.sort_by_key(|row| row.id);
    (rows, policy_store_changed)
}

fn is_paused(row: &TorrentRow) -> bool {
    row.state.eq_ignore_ascii_case("paused")
}

async fn enforce_queue(
    api: &Api,
    rows: &[TorrentRow],
    max_active: usize,
    queue_paused: &mut HashSet<usize>,
    user_paused: &HashSet<usize>,
) -> Vec<String> {
    let present_unfinished = rows
        .iter()
        .filter(|row| !row.finished)
        .map(|row| row.id)
        .collect::<HashSet<_>>();
    queue_paused.retain(|id| present_unfinished.contains(id));

    if max_active == 0 {
        let mut queued = queue_paused.iter().copied().collect::<Vec<_>>();
        queued.sort_unstable();
        let mut errors = Vec::new();
        for id in queued {
            if user_paused.contains(&id) {
                continue;
            }
            match api.api_torrent_action_start(TorrentIdOrHash::Id(id)).await {
                Ok(_) => {
                    queue_paused.remove(&id);
                }
                Err(error) => {
                    errors.push(format!("could not start queued transfer {id}: {error:#}"))
                }
            }
        }
        return errors;
    }

    let mut active = rows
        .iter()
        .filter(|row| !row.finished && !is_paused(row) && !user_paused.contains(&row.id))
        .map(|row| row.id)
        .collect::<Vec<_>>();
    active.sort_unstable();
    let mut errors = Vec::new();

    while active.len() > max_active {
        let Some(&id) = active.last() else { break };
        match api.api_torrent_action_pause(TorrentIdOrHash::Id(id)).await {
            Ok(_) => {
                active.pop();
                queue_paused.insert(id);
            }
            Err(error) => {
                errors.push(format!("could not queue transfer {id}: {error:#}"));
                break;
            }
        }
    }

    let available = max_active.saturating_sub(active.len());
    if available == 0 {
        return errors;
    }

    let mut queued = rows
        .iter()
        .filter(|row| {
            !row.finished
                && is_paused(row)
                && queue_paused.contains(&row.id)
                && !user_paused.contains(&row.id)
        })
        .map(|row| row.id)
        .collect::<Vec<_>>();
    queued.sort_unstable();
    for id in queued.into_iter().take(available) {
        match api.api_torrent_action_start(TorrentIdOrHash::Id(id)).await {
            Ok(_) => {
                queue_paused.remove(&id);
            }
            Err(error) => errors.push(format!("could not start queued transfer {id}: {error:#}")),
        }
    }
    errors
}

pub fn start_engine(
    download_dir: PathBuf,
    app_data_dir: PathBuf,
    settings: AppSettings,
    repaint: Repaint,
) -> (Sender<EngineCommand>, Receiver<EngineEvent>) {
    let (command_tx, command_rx) = mpsc::channel::<EngineCommand>();
    let (event_tx, event_rx) = mpsc::channel::<EngineEvent>();
    let network_plan = settings.engine_network_plan();

    std::thread::spawn(move || {
        if !network_plan.start_session {
            emit(&event_tx, &repaint, EngineEvent::NetworkLocked);
            while let Ok(command) = command_rx.recv() {
                if matches!(command, EngineCommand::Shutdown) {
                    emit(&event_tx, &repaint, EngineEvent::Stopped);
                    break;
                }
                if matches!(
                    command,
                    EngineCommand::SetGlobalLimits { .. } | EngineCommand::SetMaxActive(_)
                ) {
                    continue;
                }
                emit(
                    &event_tx,
                    &repaint,
                    EngineEvent::Error(
                        "Orbit is in Offline mode. Choose a network mode in Settings and restart Orbit."
                            .to_owned(),
                    ),
                );
            }
            return;
        }

        let runtime = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                emit(
                    &event_tx,
                    &repaint,
                    EngineEvent::Fatal(format!("could not start torrent runtime: {error:#}")),
                );
                return;
            }
        };

        runtime.block_on(async move {
            emit(
                &event_tx,
                &repaint,
                EngineEvent::Starting(download_dir.clone()),
            );

            for (description, directory) in [
                ("download", download_dir.as_path()),
                ("application data", app_data_dir.as_path()),
            ] {
                if let Err(error) = fs::create_dir_all(directory) {
                    emit(
                        &event_tx,
                        &repaint,
                        EngineEvent::Fatal(format!(
                            "could not create {description} folder {}: {error:#}",
                            directory.display()
                        )),
                    );
                    return;
                }
            }

            let session_persistence_dir = persistence_dir();
            if let Err(error) = fs::create_dir_all(&session_persistence_dir) {
                emit(
                    &event_tx,
                    &repaint,
                    EngineEvent::Fatal(format!(
                        "could not create session folder {}: {error:#}",
                        session_persistence_dir.display()
                    )),
                );
                return;
            }

            let policy_load = load_policies();
            let policy_load_failed = policy_load.is_err();
            let effective_upload_block = network_plan.disable_upload || policy_load_failed;
            let recovered_default_policy = if policy_load_failed {
                SharePolicy::StopWhenComplete
            } else { settings.default_share_policy };

            let session = match Session::new_with_opts(
                download_dir,
                SessionOptions {
                    fastresume: true,
                    persistence: Some(SessionPersistenceConfig::Json {
                        folder: Some(session_persistence_dir),
                    }),
                    ratelimits: limits_from_kib(
                        settings.download_limit_kib,
                        settings.upload_limit_kib,
                    ),
                    disable_dht: network_plan.disable_dht,
                    disable_upload: effective_upload_block,
                    listen_port_range: network_plan.incoming_port_range,
                    enable_upnp_port_forwarding: network_plan.enable_upnp,
                    ..Default::default()
                },
            )
            .await
            {
                Ok(session) => session,
                Err(error) => {
                    emit(
                        &event_tx,
                        &repaint,
                        EngineEvent::Fatal(format!(
                            "torrent engine failed to start: {error:#}"
                        )),
                    );
                    return;
                }
            };
            let listen_port = session.tcp_listen_port();

            let api = Api::new(session.clone(), None);
            let mut policies = match policy_load {
                Ok(policies) => policies,
                Err(error) => {
                    emit(
                        &event_tx,
                        &repaint,
                        EngineEvent::Error(format!(
                            "could not load transfer policies: {error:#}"
                        )),
                    );
                    HashMap::new()
                }
            };
            let mut max_active = settings.max_active_downloads;
            let mut queue_paused = HashSet::new();
            let mut user_paused = HashSet::new();
            let mut first_snapshot = true;
            let mut last_rows = Vec::<TorrentRow>::new();

            emit(
                &event_tx,
                &repaint,
                EngineEvent::Ready {
                    uploads_blocked: effective_upload_block,
                    listen_port,
                },
            );

            loop {
                while let Ok(command) = command_rx.try_recv() {
                    match command {
                        EngineCommand::Shutdown => {
                            session.stop().await;
                            emit(&event_tx, &repaint, EngineEvent::Stopped);
                            return;
                        }
                        EngineCommand::Preview(source) => {
                            let attempted_source = source.clone();
                            match preview_torrent(&api, source, &settings).await {
                                Ok(draft) => emit(
                                    &event_tx,
                                    &repaint,
                                    EngineEvent::PreviewReady(draft),
                                ),
                                Err(error) => {
                                    emit(
                                        &event_tx,
                                        &repaint,
                                        EngineEvent::PreviewFailed {
                                            source: attempted_source,
                                            message: format!("{error:#}"),
                                        },
                                    )
                                }
                            }
                        }
                        EngineCommand::Add(request) => {
                            match add_torrent(&api, request, &mut policies, &mut user_paused).await {
                                Ok(message) => {
                                    emit(&event_tx, &repaint, EngineEvent::Notice(message))
                                }
                                Err(error) => emit(
                                    &event_tx,
                                    &repaint,
                                    EngineEvent::Error(format!("{error:#}")),
                                ),
                            }
                        }
                        EngineCommand::Pause(id) => {
                            match api
                                .api_torrent_action_pause(TorrentIdOrHash::Id(id))
                                .await
                            {
                                Ok(_) => {
                                    queue_paused.remove(&id);
                                    user_paused.insert(id);
                                    emit(
                                        &event_tx,
                                        &repaint,
                                        EngineEvent::Notice("Transfer paused".to_owned()),
                                    );
                                }
                                Err(error) => emit(
                                    &event_tx,
                                    &repaint,
                                    EngineEvent::Error(format!(
                                        "could not pause transfer {id}: {error:#}"
                                    )),
                                ),
                            }
                        }
                        EngineCommand::Resume(id) => {
                            let sharing_blocked = last_rows.iter().any(|row| {
                                row.id == id
                                    && row.finished
                                    && (row.policy.share_policy
                                        == SharePolicy::StopWhenComplete
                                        || row.policy.seed_limit_reached)
                            });
                            if sharing_blocked {
                                emit(
                                    &event_tx,
                                    &repaint,
                                    EngineEvent::Error(
                                        "Change the completion policy or raise its seeding limit before resuming it."
                                            .to_owned(),
                                    ),
                                );
                                continue;
                            }
                            match api
                                .api_torrent_action_start(TorrentIdOrHash::Id(id))
                                .await
                            {
                                Ok(_) => {
                                    queue_paused.remove(&id);
                                    user_paused.remove(&id);
                                    emit(
                                        &event_tx,
                                        &repaint,
                                        EngineEvent::Notice("Transfer resumed".to_owned()),
                                    );
                                }
                                Err(error) => emit(
                                    &event_tx,
                                    &repaint,
                                    EngineEvent::Error(format!(
                                        "could not resume transfer {id}: {error:#}"
                                    )),
                                ),
                            }
                        }
                        EngineCommand::Forget(id) | EngineCommand::DeleteData(id) => {
                            let delete_data = matches!(command, EngineCommand::DeleteData(_));
                            let info_hash = last_rows
                                .iter()
                                .find(|row| row.id == id)
                                .map(|row| row.info_hash.clone());
                            let result = if delete_data {
                                api.api_torrent_action_delete(TorrentIdOrHash::Id(id))
                                    .await
                            } else {
                                api.api_torrent_action_forget(TorrentIdOrHash::Id(id))
                                    .await
                            };
                            match result {
                                Ok(_) => {
                                    queue_paused.remove(&id);
                                    user_paused.remove(&id);
                                    if let Some(info_hash) = info_hash {
                                        policies.remove(&info_hash);
                                        if let Err(error) = save_policies(&policies) {
                                            emit(
                                                &event_tx,
                                                &repaint,
                                                EngineEvent::Error(format!(
                                                    "transfer removed, but policy cleanup failed: {error:#}"
                                                )),
                                            );
                                        }
                                    }
                                    emit(
                                        &event_tx,
                                        &repaint,
                                        EngineEvent::Notice(if delete_data {
                                            "Transfer and downloaded data deleted".to_owned()
                                        } else {
                                            "Transfer removed; downloaded files kept".to_owned()
                                        }),
                                    );
                                }
                                Err(error) => emit(
                                    &event_tx,
                                    &repaint,
                                    EngineEvent::Error(format!(
                                        "could not remove transfer {id}: {error:#}"
                                    )),
                                ),
                            }
                        }
                        EngineCommand::UpdateFiles { id, only_files } => {
                            let only_files = only_files.into_iter().collect::<HashSet<_>>();
                            match api
                                .api_torrent_action_update_only_files(
                                    TorrentIdOrHash::Id(id),
                                    &only_files,
                                )
                                .await
                            {
                                Ok(_) => emit(
                                    &event_tx,
                                    &repaint,
                                    EngineEvent::Notice("File selection updated".to_owned()),
                                ),
                                Err(error) => emit(
                                    &event_tx,
                                    &repaint,
                                    EngineEvent::Error(format!(
                                        "could not update files for transfer {id}: {error:#}"
                                    )),
                                ),
                            }
                        }
                        EngineCommand::UpdatePolicy { info_hash, policy } => {
                            let mut policy = policy;
                            if let Some(previous) = policies.get(&info_hash).cloned() {
                                let action_changed = previous.share_policy != policy.share_policy
                                    || previous.delete_source_when_complete
                                        != policy.delete_source_when_complete;
                                let seed_limits_changed = previous.seed_ratio_percent
                                    != policy.seed_ratio_percent
                                    || previous.seed_time_minutes != policy.seed_time_minutes;
                                if action_changed {
                                    policy.completion_notified = false;
                                }
                                if action_changed || seed_limits_changed {
                                    policy.seed_limit_reached = false;
                                }
                                policy.source_torrent = previous.source_torrent;
                                policy.source_torrent_sha256 = previous.source_torrent_sha256;
                                policy.completed_at_unix = previous.completed_at_unix;
                                policy.added_at_unix = previous.added_at_unix;
                                if previous.source_deleted {
                                    policy.source_deleted = true;
                                }
                            }
                            policies.insert(info_hash, policy);
                            match save_policies(&policies) {
                                Ok(()) => emit(
                                    &event_tx,
                                    &repaint,
                                    EngineEvent::Notice("Transfer policy updated".to_owned()),
                                ),
                                Err(error) => emit(
                                    &event_tx,
                                    &repaint,
                                    EngineEvent::Error(format!(
                                        "could not save transfer policy: {error:#}"
                                    )),
                                ),
                            }
                        }
                        EngineCommand::SetGlobalLimits { down_kib, up_kib } => {
                            session
                                .ratelimits
                                .set_download_bps(limit_from_kib(down_kib));
                            session.ratelimits.set_upload_bps(limit_from_kib(up_kib));
                            emit(
                                &event_tx,
                                &repaint,
                                EngineEvent::Notice("Global speed limits updated".to_owned()),
                            );
                        }
                        EngineCommand::SetMaxActive(limit) => {
                            max_active = limit;
                            emit(
                                &event_tx,
                                &repaint,
                                EngineEvent::Notice(if limit == 0 {
                                    "Download queue limit disabled".to_owned()
                                } else {
                                    format!("Download queue limited to {limit} active transfers")
                                }),
                            );
                        }
                        EngineCommand::PauseAll => {
                            let ids = last_rows.iter().map(|row| row.id).collect::<Vec<_>>();
                            let mut paused = 0;
                            for id in ids {
                                match api
                                    .api_torrent_action_pause(TorrentIdOrHash::Id(id))
                                    .await
                                {
                                    Ok(_) => {
                                        queue_paused.remove(&id);
                                        user_paused.insert(id);
                                        paused += 1;
                                    }
                                    Err(error) => emit(
                                        &event_tx,
                                        &repaint,
                                        EngineEvent::Error(format!(
                                            "could not pause transfer {id}: {error:#}"
                                        )),
                                    ),
                                }
                            }
                            emit(
                                &event_tx,
                                &repaint,
                                EngineEvent::Notice(format!("Paused {paused} transfers")),
                            );
                        }
                        EngineCommand::ResumeAll => {
                            let rows = last_rows.clone();
                            let mut resumed = 0;
                            user_paused.clear();
                            for row in rows {
                                if row.finished
                                    && (row.policy.share_policy == SharePolicy::StopWhenComplete
                                        || row.policy.seed_limit_reached)
                                {
                                    continue;
                                }
                                match api
                                    .api_torrent_action_start(TorrentIdOrHash::Id(row.id))
                                    .await
                                {
                                    Ok(_) => {
                                        queue_paused.remove(&row.id);
                                        resumed += 1;
                                    }
                                    Err(error) => emit(
                                        &event_tx,
                                        &repaint,
                                        EngineEvent::Error(format!(
                                            "could not resume transfer {}: {error:#}",
                                            row.id
                                        )),
                                    ),
                                }
                            }
                            emit(
                                &event_tx,
                                &repaint,
                                EngineEvent::Notice(format!("Resumed {resumed} transfers")),
                            );
                        }
                    }
                }

                let (rows, policy_store_changed) = snapshot(
                    &api,
                    &mut policies,
                    recovered_default_policy,
                    settings.default_seed_ratio_percent,
                    settings.default_seed_time_minutes,
                    effective_upload_block,
                );
                if first_snapshot {
                    for row in rows.iter().filter(|row| !row.finished && is_paused(row)) {
                        user_paused.insert(row.id);
                    }
                    first_snapshot = false;
                }

                let completion_changed = completion::apply_completion_policies(
                    &api,
                    &rows,
                    &mut policies,
                    &event_tx,
                    &repaint,
                )
                .await;
                if (policy_store_changed || completion_changed)
                    && let Err(error) = save_policies(&policies)
                {
                    emit(
                        &event_tx,
                        &repaint,
                        EngineEvent::Error(format!(
                            "could not save transfer policies: {error:#}"
                        )),
                    );
                }

                for error in enforce_queue(
                    &api,
                    &rows,
                    max_active,
                    &mut queue_paused,
                    &user_paused,
                )
                .await
                {
                    emit(&event_tx, &repaint, EngineEvent::Error(error));
                }

                last_rows = rows.clone();
                emit(&event_tx, &repaint, EngineEvent::Snapshot(rows));
                tokio::time::sleep(Duration::from_millis(850)).await;
            }
        });
    });

    (command_tx, event_rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_torrent_is_rejected_in_network_disabled_preview() {
        let test_dir = std::env::temp_dir().join(format!(
            "orbit-malformed-test-{}-{}",
            std::process::id(),
            unix_now()
        ));
        std::fs::create_dir_all(&test_dir).expect("create isolated test directory");

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("create test runtime");
        let result = runtime.block_on(async {
            let session = Session::new_with_opts(
                test_dir.clone(),
                SessionOptions {
                    disable_dht: true,
                    disable_dht_persistence: true,
                    disable_upload: true,
                    ..Default::default()
                },
            )
            .await
            .expect("create network-disabled test session");
            let api = Api::new(session, None);
            api.api_add_torrent(
                AddTorrent::from_bytes(b"not a valid bencoded torrent".to_vec()),
                Some(AddTorrentOptions {
                    list_only: true,
                    ..Default::default()
                }),
            )
            .await
        });

        assert!(result.is_err(), "malformed torrent must be rejected");
        let _ = std::fs::remove_dir_all(test_dir);
    }
}
