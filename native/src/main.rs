#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod engine;
mod network_profile;
mod settings;
#[cfg(test)]
mod settings_tests;
mod storage;
mod tray;
#[cfg(test)]
mod ui_tests;

use std::{
    collections::{HashMap, HashSet, VecDeque},
    io::{ErrorKind, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender},
    },
    time::{Duration, Instant},
};

use eframe::egui::{
    self, Align, Color32, ComboBox, CornerRadius, FontId, Frame, Layout, Margin, RichText,
    ScrollArea, Stroke, Vec2,
};
use engine::{AddDraft, AddRequest, EngineCommand, EngineEvent, TorrentRow, start_engine};
use network_profile::detect_system_network_profile;
use settings::{
    AppSettings, NetworkMode, SharePolicy, StartupNetworkPolicy, SystemNetworkProfile,
    TransferPolicy, data_dir, default_download_dir, load_settings, save_settings,
};
use storage::{format_bytes, storage_preflight};
use tray::{OrbitTray, TrayAction};

const KRYPTON_BG: Color32 = Color32::from_rgb(22, 16, 31);
const KRYPTON_CHROME: Color32 = Color32::from_rgb(30, 21, 48);
const KRYPTON_SUBTLE: Color32 = Color32::from_rgb(18, 11, 27);
const KRYPTON_CARD: Color32 = Color32::from_rgb(36, 25, 52);
const KRYPTON_ACTIVE: Color32 = Color32::from_rgb(29, 20, 40);
const KRYPTON_HOVER: Color32 = Color32::from_rgb(50, 35, 74);
const KRYPTON_PRESSED: Color32 = Color32::from_rgb(63, 43, 92);
const KRYPTON_SELECTED: Color32 = Color32::from_rgb(85, 50, 168);
const KRYPTON_STROKE: Color32 = Color32::from_rgb(50, 32, 70);
const KRYPTON_STROKE_SUBTLE: Color32 = Color32::from_rgb(37, 23, 53);
const KRYPTON_PURPLE: Color32 = Color32::from_rgb(157, 124, 255);
const KRYPTON_PURPLE_HOVER: Color32 = Color32::from_rgb(181, 156, 255);
const TEXT_PRIMARY: Color32 = Color32::from_rgb(242, 234, 255);
const TEXT_SECONDARY: Color32 = Color32::from_rgb(192, 176, 218);
const TEXT_TERTIARY: Color32 = Color32::from_rgb(126, 108, 157);
const RADIOACTIVE: Color32 = Color32::from_rgb(183, 255, 60);
const RADIOACTIVE_SOFT: Color32 = Color32::from_rgb(215, 255, 146);
const RADIOACTIVE_DARK: Color32 = Color32::from_rgb(24, 35, 8);
const DANGER: Color32 = Color32::from_rgb(255, 107, 149);
const WARNING: Color32 = Color32::from_rgb(255, 184, 76);
const ACTIVATION_ADDRESS: &str = "127.0.0.1:47618";

fn activation_address() -> String {
    std::env::var("ORBIT_ACTIVATION_ADDRESS").unwrap_or_else(|_| ACTIVATION_ADDRESS.to_owned())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum TransferFilter {
    #[default]
    All,
    Downloading,
    Completed,
    Paused,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SortMode {
    #[default]
    Added,
    Name,
    Progress,
    Size,
    Speed,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum CoreState {
    #[default]
    Starting,
    Online,
    Locked,
    Offline,
}

#[derive(Clone, Debug)]
struct RemovePrompt {
    id: usize,
    name: String,
    output_folder: String,
    total: u64,
}

fn network_mode_combo(ui: &mut egui::Ui, id: &'static str, mode: &mut NetworkMode) {
    ComboBox::from_id_salt(id)
        .selected_text(mode.label())
        .show_ui(ui, |ui| {
            for value in [
                NetworkMode::Offline,
                NetworkMode::DownloadOnly,
                NetworkMode::Standard,
            ] {
                ui.selectable_value(mode, value, value.label());
            }
        });
}

fn transfer_bar_color(torrent: &TorrentRow) -> Color32 {
    if torrent.error.is_some() {
        DANGER
    } else if torrent.finished {
        RADIOACTIVE
    } else if is_paused(torrent) {
        KRYPTON_SELECTED
    } else if torrent.status.eq_ignore_ascii_case("seeding") {
        RADIOACTIVE_SOFT
    } else {
        KRYPTON_PURPLE
    }
}

fn krypton_progress_bar(
    ui: &mut egui::Ui,
    progress: f32,
    fill: Color32,
    label: &str,
) -> egui::Response {
    let desired = Vec2::new(ui.available_width(), 20.0);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let painter = ui.painter();
    let progress = progress.clamp(0.0, 1.0);
    let track = rect.shrink(1.0);

    painter.rect_filled(track, CornerRadius::same(8), KRYPTON_SUBTLE);
    painter.rect_stroke(
        track,
        CornerRadius::same(8),
        Stroke::new(1.0, KRYPTON_STROKE),
        egui::StrokeKind::Inside,
    );

    if progress > 0.0 {
        let inner = track.shrink(2.0);
        let filled_width = (inner.width() * progress).max(3.0).min(inner.width());
        let filled = egui::Rect::from_min_size(inner.min, Vec2::new(filled_width, inner.height()));
        painter.rect_filled(filled, CornerRadius::same(6), fill);
        painter.line_segment(
            [
                egui::pos2(filled.left() + 4.0, filled.top() + 2.0),
                egui::pos2(
                    (filled.right() - 3.0).max(filled.left() + 4.0),
                    filled.top() + 2.0,
                ),
            ],
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 80)),
        );
        if progress < 1.0 {
            painter.line_segment(
                [
                    egui::pos2(filled.right(), filled.top() + 2.0),
                    egui::pos2(filled.right(), filled.bottom() - 2.0),
                ],
                Stroke::new(2.0, RADIOACTIVE),
            );
        }
    }

    let text_color = if progress >= 0.5 && (fill == RADIOACTIVE || fill == RADIOACTIVE_SOFT) {
        RADIOACTIVE_DARK
    } else {
        TEXT_PRIMARY
    };
    painter.text(
        rect.center() + egui::vec2(0.0, 1.0),
        egui::Align2::CENTER_CENTER,
        label,
        FontId::proportional(8.5),
        text_color,
    );
    response
}

fn krypton_status_strip(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 5.0), egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(3), KRYPTON_SUBTLE);
    ui.painter()
        .rect_filled(rect.shrink(1.0), CornerRadius::same(2), color);
}

fn is_supported_source(source: &str) -> bool {
    let lower = source.to_ascii_lowercase();
    if lower.starts_with("magnet:") || lower.starts_with("http://") || lower.starts_with("https://")
    {
        return true;
    }
    !lower.contains("://")
        && Path::new(source)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("torrent"))
}

fn normalized_local_path(source: &str) -> PathBuf {
    let path = PathBuf::from(source);
    std::fs::canonicalize(&path).unwrap_or(path)
}

fn collect_startup_sources() -> Vec<String> {
    std::env::args_os()
        .skip(1)
        .map(|argument| argument.to_string_lossy().into_owned())
        .filter(|source| is_supported_source(source))
        .collect()
}

fn forward_to_running_instance(sources: &[String]) {
    if let Ok(mut stream) = TcpStream::connect(activation_address()) {
        let _ = stream.write_all(sources.join("\0").as_bytes());
    }
}

fn acquire_activation_listener(sources: &[String]) -> (Option<TcpListener>, bool) {
    match TcpListener::bind(activation_address()) {
        Ok(listener) => (Some(listener), false),
        Err(error) if error.kind() == ErrorKind::AddrInUse => {
            forward_to_running_instance(sources);
            (None, true)
        }
        Err(_) => (None, false),
    }
}

fn start_activation_listener(
    listener: TcpListener,
    command_router: Arc<Mutex<Sender<EngineCommand>>>,
    repaint: egui::Context,
) {
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let mut payload = String::new();
            if stream.take(64 * 1024).read_to_string(&mut payload).is_err() {
                continue;
            }
            for source in payload
                .split('\0')
                .filter(|source| is_supported_source(source))
            {
                if let Ok(command_tx) = command_router.lock() {
                    let _ = command_tx.send(EngineCommand::Preview(source.to_owned()));
                }
            }
            repaint.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            repaint.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            repaint.send_viewport_cmd(egui::ViewportCommand::Focus);
            repaint.request_repaint();
        }
    });
}

fn is_paused(torrent: &TorrentRow) -> bool {
    torrent.state.eq_ignore_ascii_case("paused")
}

fn is_active_transfer(torrent: &TorrentRow) -> bool {
    !is_paused(torrent) && (!torrent.finished || torrent.status.eq_ignore_ascii_case("seeding"))
}

fn open_folder(path: &str) -> anyhow::Result<()> {
    #[cfg(windows)]
    let program = "explorer.exe";
    #[cfg(target_os = "macos")]
    let program = "/usr/bin/open";
    #[cfg(not(any(windows, target_os = "macos")))]
    let program = "xdg-open";
    Command::new(program).arg(path).spawn()?;
    Ok(())
}

#[cfg(windows)]
fn open_default_apps() -> anyhow::Result<()> {
    Command::new("explorer.exe")
        .arg("ms-settings:defaultapps?registeredAppUser=Orbit")
        .spawn()?;
    Ok(())
}

fn collect_watch_files(folder: &Path, depth: usize, output: &mut Vec<PathBuf>) {
    if depth > 8 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_watch_files(&path, depth + 1, output);
        } else if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("torrent"))
        {
            output.push(path);
        }
    }
}

fn style_krypton(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = Some(TEXT_PRIMARY);
    visuals.weak_text_color = Some(TEXT_TERTIARY);
    visuals.panel_fill = KRYPTON_BG;
    visuals.window_fill = KRYPTON_CARD;
    visuals.window_stroke = Stroke::new(1.0, KRYPTON_STROKE);
    visuals.faint_bg_color = KRYPTON_ACTIVE;
    visuals.extreme_bg_color = KRYPTON_SUBTLE;
    visuals.text_edit_bg_color = Some(KRYPTON_ACTIVE);
    visuals.code_bg_color = KRYPTON_SUBTLE;
    visuals.selection.bg_fill = KRYPTON_SELECTED;
    visuals.selection.stroke = Stroke::new(1.0, TEXT_PRIMARY);
    visuals.hyperlink_color = RADIOACTIVE;
    visuals.warn_fg_color = WARNING;
    visuals.error_fg_color = DANGER;
    visuals.widgets.noninteractive.bg_fill = KRYPTON_CARD;
    visuals.widgets.noninteractive.weak_bg_fill = KRYPTON_CARD;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, KRYPTON_STROKE_SUBTLE);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_SECONDARY);
    visuals.widgets.inactive.bg_fill = KRYPTON_ACTIVE;
    visuals.widgets.inactive.weak_bg_fill = KRYPTON_ACTIVE;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, KRYPTON_STROKE);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_SECONDARY);
    visuals.widgets.hovered.bg_fill = KRYPTON_HOVER;
    visuals.widgets.hovered.weak_bg_fill = KRYPTON_HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, KRYPTON_PURPLE_HOVER);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    visuals.widgets.active.bg_fill = KRYPTON_PRESSED;
    visuals.widgets.active.weak_bg_fill = KRYPTON_PRESSED;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, KRYPTON_PURPLE);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    visuals.widgets.open = visuals.widgets.active;
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = CornerRadius::same(7);
    }
    ctx.set_visuals(visuals);
}

struct OrbitApp {
    command_tx: Sender<EngineCommand>,
    command_router: Arc<Mutex<Sender<EngineCommand>>>,
    event_rx: Receiver<EngineEvent>,
    torrents: Vec<TorrentRow>,
    selected: Option<usize>,
    editor_for: Option<usize>,
    policy_editor: Option<TransferPolicy>,
    file_selection: HashSet<usize>,
    source_input: String,
    engine_status: String,
    core_state: CoreState,
    notice: String,
    error: Option<String>,
    download_dir: PathBuf,
    previews: VecDeque<AddDraft>,
    remove_prompt: Option<RemovePrompt>,
    settings: AppSettings,
    settings_editor: AppSettings,
    show_settings: bool,
    show_startup_setup: bool,
    first_run_setup: bool,
    system_network_profile: SystemNetworkProfile,
    pending_startup_sources: Vec<String>,
    show_about: bool,
    uploads_blocked_active: bool,
    search: String,
    filter: TransferFilter,
    sort: SortMode,
    turtle_mode: bool,
    tray: Option<OrbitTray>,
    quitting: bool,
    watch_seen: HashSet<PathBuf>,
    watch_pending: HashSet<PathBuf>,
    watch_retry_after: HashMap<PathBuf, Instant>,
    last_watch_scan: Instant,
}

impl OrbitApp {
    fn new(
        cc: &eframe::CreationContext<'_>,
        startup_sources: Vec<String>,
        activation_listener: Option<TcpListener>,
        mut settings: AppSettings,
        icon: egui::IconData,
    ) -> Self {
        style_krypton(&cc.egui_ctx);
        let stored_settings = settings.clone();
        let system_network_profile = detect_system_network_profile();
        let first_run_setup = !settings.network_setup_completed;
        let needs_startup_setup = first_run_setup
            || settings.startup_network_policy == StartupNetworkPolicy::AskEveryStart;
        if needs_startup_setup {
            settings.sharing_consent_recorded = false;
            settings.network_mode = Some(NetworkMode::Offline);
            settings.block_all_uploads = true;
        } else if settings.startup_network_policy == StartupNetworkPolicy::FollowSystem {
            let mode = settings.mode_for_system_profile(system_network_profile);
            settings.network_mode = Some(mode);
            settings.sharing_consent_recorded = true;
            settings.block_all_uploads = mode != NetworkMode::Standard;
        }
        if !settings.sharing_consent_recorded {
            settings.block_all_uploads = true;
            settings.default_share_policy = SharePolicy::StopWhenComplete;
        }
        let download_dir = std::env::var_os("ORBIT_DOWNLOAD_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let configured = settings.default_download_dir.trim();
                if configured.is_empty() {
                    default_download_dir()
                } else {
                    PathBuf::from(configured)
                }
            });
        let (command_tx, event_rx) =
            start_engine(download_dir.clone(), data_dir(), settings.clone(), {
                let context = cc.egui_ctx.clone();
                Arc::new(move || context.request_repaint())
            });
        let command_router = Arc::new(Mutex::new(command_tx.clone()));
        if let Some(listener) = activation_listener {
            start_activation_listener(listener, command_router.clone(), cc.egui_ctx.clone());
        }
        let pending_startup_sources = if needs_startup_setup {
            startup_sources
        } else {
            for source in startup_sources {
                let _ = command_tx.send(EngineCommand::Preview(source));
            }
            Vec::new()
        };
        let tray = OrbitTray::new(&icon).ok();
        Self {
            command_tx,
            command_router,
            event_rx,
            torrents: Vec::new(),
            selected: None,
            editor_for: None,
            policy_editor: None,
            file_selection: HashSet::new(),
            source_input: String::new(),
            engine_status: "Starting embedded engine…".to_owned(),
            core_state: CoreState::Starting,
            notice: if needs_startup_setup {
                "Complete Network Setup · torrent networking is off".to_owned()
            } else if settings.startup_network_policy == StartupNetworkPolicy::FollowSystem {
                format!(
                    "{} detected · {}",
                    system_network_profile.label(),
                    settings.effective_network_mode().label()
                )
            } else {
                format!(
                    "{} mode selected",
                    settings.effective_network_mode().label()
                )
            },
            error: None,
            download_dir,
            previews: VecDeque::new(),
            remove_prompt: None,
            settings_editor: stored_settings,
            settings,
            show_settings: false,
            show_startup_setup: needs_startup_setup,
            first_run_setup,
            system_network_profile,
            pending_startup_sources,
            show_about: false,
            uploads_blocked_active: false,
            search: String::new(),
            filter: TransferFilter::All,
            sort: SortMode::Added,
            turtle_mode: false,
            tray,
            quitting: false,
            watch_seen: HashSet::new(),
            watch_pending: HashSet::new(),
            watch_retry_after: HashMap::new(),
            last_watch_scan: Instant::now(),
        }
    }

    fn send(&mut self, command: EngineCommand) {
        if self.command_tx.send(command).is_err() {
            self.core_state = CoreState::Offline;
            self.engine_status = "Embedded core unavailable".to_owned();
            self.error = Some("The embedded torrent engine stopped responding.".to_owned());
        }
    }

    fn restart_engine(
        &mut self,
        ctx: &egui::Context,
        runtime_settings: AppSettings,
    ) -> Result<(), String> {
        let mut stopped = self.command_tx.send(EngineCommand::Shutdown).is_err();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !stopped && Instant::now() < deadline {
            match self.event_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(EngineEvent::Stopped) => stopped = true,
                Ok(_) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => stopped = true,
            }
        }
        if !stopped {
            return Err(
                "The previous torrent core did not stop safely. Restart Orbit to apply the change."
                    .to_owned(),
            );
        }

        let download_dir = std::env::var_os("ORBIT_DOWNLOAD_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let configured = runtime_settings.default_download_dir.trim();
                if configured.is_empty() {
                    default_download_dir()
                } else {
                    PathBuf::from(configured)
                }
            });
        let (command_tx, event_rx) = start_engine(
            download_dir.clone(),
            data_dir(),
            runtime_settings.clone(),
            {
                let context = ctx.clone();
                Arc::new(move || context.request_repaint())
            },
        );
        let mut router = self
            .command_router
            .lock()
            .map_err(|_| "The activation command router is unavailable.".to_owned())?;
        *router = command_tx.clone();
        drop(router);

        self.command_tx = command_tx;
        self.event_rx = event_rx;
        self.settings = runtime_settings;
        self.download_dir = download_dir;
        self.torrents.clear();
        self.previews.clear();
        self.core_state = CoreState::Starting;
        self.engine_status = "Starting embedded engine…".to_owned();
        for source in std::mem::take(&mut self.pending_startup_sources) {
            let _ = self.command_tx.send(EngineCommand::Preview(source));
        }
        Ok(())
    }

    fn request_preview(&mut self, source: String) {
        let source = source.trim().to_owned();
        if source.is_empty() {
            self.error = Some("Choose a .torrent file or paste a magnet link first.".to_owned());
        } else if !is_supported_source(&source) {
            self.error = Some(
                "Orbit accepts magnet links, torrent URLs, and local .torrent files.".to_owned(),
            );
        } else {
            self.send(EngineCommand::Preview(source));
            self.notice = "Reading torrent metadata…".to_owned();
            self.error = None;
        }
    }

    fn drain_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                EngineEvent::Starting(path) => {
                    self.core_state = CoreState::Starting;
                    self.engine_status = format!("Starting · {}", path.display());
                }
                EngineEvent::NetworkLocked => {
                    self.core_state = CoreState::Locked;
                    self.uploads_blocked_active = true;
                    self.engine_status = "Offline mode · no torrent network activity".to_owned();
                    self.notice = "Choose a network mode in Settings when you are ready".to_owned();
                    self.error = None;
                }
                EngineEvent::Ready {
                    uploads_blocked,
                    listen_port,
                } => {
                    self.core_state = CoreState::Online;
                    self.uploads_blocked_active = uploads_blocked;
                    let connection = if let Some(port) = listen_port {
                        format!(" · listening on {port}")
                    } else {
                        " · outbound connections only".to_owned()
                    };
                    self.engine_status = if uploads_blocked {
                        format!("Embedded core ready · Download Only{connection}")
                    } else {
                        format!("Embedded core ready · Standard{connection}")
                    };
                    self.notice = if self.settings.sharing_consent_recorded {
                        "Listening for magnets and torrent files".to_owned()
                    } else {
                        "Choose sharing behavior · uploads remain blocked".to_owned()
                    };
                    self.error = None;
                }
                EngineEvent::Snapshot(rows) => {
                    self.torrents = rows;
                    if self
                        .selected
                        .is_some_and(|id| !self.torrents.iter().any(|row| row.id == id))
                    {
                        self.selected = None;
                        self.editor_for = None;
                    }
                    if self.selected.is_none() {
                        self.selected = self.torrents.first().map(|torrent| torrent.id);
                    }
                    self.sync_editors();
                }
                EngineEvent::PreviewReady(draft) => {
                    let local_source = normalized_local_path(&draft.source);
                    if local_source.is_file() {
                        self.watch_pending.remove(&local_source);
                        self.watch_retry_after.remove(&local_source);
                        self.watch_seen.insert(local_source);
                    }
                    self.previews.push_back(draft);
                    self.notice = "Review the transfer before starting".to_owned();
                    self.error = None;
                }
                EngineEvent::PreviewFailed { source, message } => {
                    let local_source = normalized_local_path(&source);
                    self.watch_pending.remove(&local_source);
                    if local_source
                        .extension()
                        .is_some_and(|value| value.eq_ignore_ascii_case("torrent"))
                    {
                        self.watch_retry_after
                            .insert(local_source, Instant::now() + Duration::from_secs(60));
                    }
                    self.error = Some(message);
                }
                EngineEvent::Completion { name, action } => {
                    self.notice = format!("{name} complete · {action}");
                    self.error = None;
                    ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                        egui::UserAttentionType::Informational,
                    ));
                }
                EngineEvent::Notice(message) => {
                    self.notice = message;
                    self.error = None;
                }
                EngineEvent::Error(message) => self.error = Some(message),
                EngineEvent::Fatal(message) => {
                    self.core_state = CoreState::Offline;
                    self.engine_status = "Embedded core unavailable".to_owned();
                    self.error = Some(message);
                }
                EngineEvent::Stopped => {
                    self.core_state = CoreState::Offline;
                    self.engine_status = "Embedded core stopped".to_owned();
                }
            }
        }
    }

    fn sync_editors(&mut self) {
        if self.editor_for == self.selected {
            return;
        }
        let row = self
            .selected
            .and_then(|id| self.torrents.iter().find(|row| row.id == id));
        self.policy_editor = row.map(|row| row.policy.clone());
        self.file_selection = row
            .map(|row| {
                row.files
                    .iter()
                    .filter(|file| file.included)
                    .map(|file| file.index)
                    .collect()
            })
            .unwrap_or_default();
        self.editor_for = self.selected;
    }

    fn handle_tray(&mut self, ctx: &egui::Context) {
        let actions = self
            .tray
            .as_ref()
            .map(OrbitTray::drain_actions)
            .unwrap_or_default();
        for action in actions {
            match action {
                TrayAction::Show => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                TrayAction::PauseAll => self.send(EngineCommand::PauseAll),
                TrayAction::ResumeAll => self.send(EngineCommand::ResumeAll),
                TrayAction::Quit => {
                    self.quitting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn update_tray_status(&self) {
        let Some(tray) = &self.tray else {
            return;
        };
        let status = match self.core_state {
            CoreState::Starting => "Core starting".to_owned(),
            CoreState::Locked => "Network offline".to_owned(),
            CoreState::Offline => "Core unavailable".to_owned(),
            CoreState::Online => {
                let active = self
                    .torrents
                    .iter()
                    .filter(|torrent| is_active_transfer(torrent))
                    .count();
                if active == 0 {
                    "No active transfers".to_owned()
                } else {
                    let down: f64 = self.torrents.iter().map(|row| row.down_speed_mib).sum();
                    let up: f64 = self.torrents.iter().map(|row| row.up_speed_mib).sum();
                    format!("{active} active · ↓ {down:.2} MiB/s · ↑ {up:.2} MiB/s")
                }
            }
        };
        tray.set_status(&status);
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let paths = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .filter_map(|file| file.path.clone())
                .collect::<Vec<_>>()
        });
        for path in paths {
            let source = path.to_string_lossy().into_owned();
            if is_supported_source(&source) {
                self.request_preview(source);
            }
        }
    }

    fn scan_watch_folder(&mut self) {
        if !self.settings.watch_enabled || self.last_watch_scan.elapsed() < Duration::from_secs(3) {
            return;
        }
        self.last_watch_scan = Instant::now();
        let folder = PathBuf::from(&self.settings.watch_folder);
        let mut files = Vec::new();
        collect_watch_files(&folder, 0, &mut files);
        files.sort();
        for file in files {
            let resolved = normalized_local_path(&file.to_string_lossy());
            let retry_ready = self
                .watch_retry_after
                .get(&resolved)
                .is_none_or(|retry_at| Instant::now() >= *retry_at);
            if !self.watch_seen.contains(&resolved)
                && retry_ready
                && self.watch_pending.insert(resolved.clone())
            {
                self.request_preview(resolved.to_string_lossy().into_owned());
            }
        }
    }

    fn sidebar(&mut self, root_ui: &mut egui::Ui) {
        egui::Panel::left("sidebar")
            .exact_size(224.0)
            .frame(
                Frame::new()
                    .fill(KRYPTON_SUBTLE)
                    .inner_margin(Margin::same(20)),
            )
            .show(root_ui, |ui| {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(28.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(rect, CornerRadius::same(8), KRYPTON_SELECTED);
                    ui.painter()
                        .circle_stroke(rect.center(), 8.0, Stroke::new(1.2, RADIOACTIVE));
                    ui.painter().circle_filled(rect.center(), 3.5, RADIOACTIVE);
                    ui.label(RichText::new("ORBIT").size(19.0).strong());
                });
                ui.label(
                    RichText::new("KRYPTON NETWORK")
                        .size(8.0)
                        .strong()
                        .color(RADIOACTIVE),
                );
                ui.add_space(25.0);
                ui.label(
                    RichText::new("LOCAL LIBRARY")
                        .size(9.0)
                        .strong()
                        .color(TEXT_TERTIARY),
                );
                ui.add_space(8.0);
                let downloading = self
                    .torrents
                    .iter()
                    .filter(|row| !row.finished && !is_paused(row))
                    .count();
                let complete = self.torrents.iter().filter(|row| row.finished).count();
                let paused = self.torrents.iter().filter(|row| is_paused(row)).count();
                for (filter, label, count) in [
                    (TransferFilter::All, "●  All transfers", self.torrents.len()),
                    (TransferFilter::Downloading, "↓  Downloading", downloading),
                    (TransferFilter::Completed, "✓  Completed", complete),
                    (TransferFilter::Paused, "Ⅱ  Paused", paused),
                ] {
                    if ui
                        .selectable_label(
                            self.filter == filter,
                            RichText::new(format!("{label}     {count}")).size(12.0),
                        )
                        .clicked()
                    {
                        self.filter = filter;
                    }
                    ui.add_space(6.0);
                }

                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    Frame::new()
                        .fill(KRYPTON_CARD)
                        .stroke(Stroke::new(1.0, KRYPTON_PURPLE))
                        .corner_radius(CornerRadius::same(10))
                        .inner_margin(Margin::same(12))
                        .show(ui, |ui| {
                            let (core_label, core_color) = match self.core_state {
                                CoreState::Starting => ("●  CORE STARTING", WARNING),
                                CoreState::Online if self.uploads_blocked_active => {
                                    ("●  PRIVACY LOCK", RADIOACTIVE)
                                }
                                CoreState::Online => ("●  CORE ONLINE", RADIOACTIVE),
                                CoreState::Locked => ("●  NETWORK OFFLINE", WARNING),
                                CoreState::Offline => ("●  CORE OFFLINE", DANGER),
                            };
                            ui.label(
                                RichText::new(core_label)
                                    .size(9.0)
                                    .strong()
                                    .color(core_color),
                            );
                            krypton_status_strip(ui, core_color);
                            ui.add_space(2.0);
                            ui.label(
                                RichText::new(&self.engine_status)
                                    .size(9.5)
                                    .color(TEXT_PRIMARY),
                            );
                            ui.label(
                                RichText::new(self.download_dir.display().to_string())
                                    .size(8.0)
                                    .color(TEXT_TERTIARY),
                            );
                        });
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new("LOCAL CORE  //  ZERO ACCOUNT")
                            .size(8.0)
                            .strong()
                            .color(TEXT_TERTIARY),
                    );
                });
            });
    }

    fn top_bar(&mut self, root_ui: &mut egui::Ui) {
        egui::Panel::top("topbar")
            .exact_size(72.0)
            .frame(
                Frame::new()
                    .fill(KRYPTON_CHROME)
                    .stroke(Stroke::new(1.0, KRYPTON_STROKE))
                    .inner_margin(Margin::symmetric(26, 14)),
            )
            .show(root_ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("KRYPTON  //  NATIVE TORRENT CLIENT")
                                .size(9.0)
                                .strong()
                                .color(KRYPTON_PURPLE),
                        );
                        ui.label(RichText::new("Transfers").size(22.0).strong());
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Settings").clicked() {
                            self.settings_editor = self.settings.clone();
                            self.show_settings = true;
                        }
                        if ui.button("About").clicked() {
                            self.show_about = true;
                        }
                        if self.settings.close_to_tray {
                            ui.label(
                                RichText::new("●  BACKGROUND OPT-IN")
                                    .size(8.5)
                                    .strong()
                                    .color(RADIOACTIVE),
                            )
                            .on_hover_text("Closing the window keeps Orbit running in the tray.");
                        }
                        let turtle_label = if self.turtle_mode {
                            "Turtle: ON"
                        } else {
                            "Turtle"
                        };
                        if ui
                            .button(turtle_label)
                            .on_hover_text(
                                "Toggle the configured reduced download and upload limits.",
                            )
                            .clicked()
                        {
                            self.turtle_mode = !self.turtle_mode;
                            let (down_kib, up_kib) = if self.turtle_mode {
                                (
                                    self.settings.turtle_download_kib,
                                    self.settings.turtle_upload_kib,
                                )
                            } else {
                                (
                                    self.settings.download_limit_kib,
                                    self.settings.upload_limit_kib,
                                )
                            };
                            self.send(EngineCommand::SetGlobalLimits { down_kib, up_kib });
                        }
                        if ui
                            .button("Resume all")
                            .on_hover_text("Resume every transfer in the queue.")
                            .clicked()
                        {
                            self.send(EngineCommand::ResumeAll);
                        }
                        if ui
                            .button("Pause all")
                            .on_hover_text("Pause every transfer in the queue.")
                            .clicked()
                        {
                            self.send(EngineCommand::PauseAll);
                        }
                        let status = self.error.as_deref().unwrap_or(self.notice.as_str());
                        ui.add_sized(
                            [300.0, 18.0],
                            egui::Label::new(
                                RichText::new(format!("●  {status}")).size(9.0).color(
                                    if self.error.is_some() {
                                        DANGER
                                    } else {
                                        RADIOACTIVE
                                    },
                                ),
                            )
                            .truncate(),
                        );
                    });
                });
            });
    }

    fn add_bar(&mut self, ui: &mut egui::Ui) {
        Frame::new()
            .fill(KRYPTON_CARD)
            .stroke(Stroke::new(1.0, KRYPTON_STROKE))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin::same(14))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("SOURCE")
                            .size(9.0)
                            .strong()
                            .color(RADIOACTIVE),
                    );
                    let response = ui.add_sized(
                        [(ui.available_width() - 225.0).max(180.0), 38.0],
                        egui::TextEdit::singleline(&mut self.source_input)
                            .hint_text("Paste magnet, torrent URL, or local .torrent path"),
                    );
                    let enter = response.lost_focus()
                        && ui.input(|input| input.key_pressed(egui::Key::Enter));
                    if ui.button("Browse…").clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter("Torrent file", &["torrent"])
                            .pick_file()
                    {
                        self.source_input = path.to_string_lossy().into_owned();
                    }
                    if ui
                        .add(
                            egui::Button::new(RichText::new("Review & add").strong())
                                .fill(KRYPTON_SELECTED),
                        )
                        .clicked()
                        || enter
                    {
                        let source = std::mem::take(&mut self.source_input);
                        self.request_preview(source);
                    }
                });
            });
        if let Some(error) = &self.error {
            ui.add_space(7.0);
            ui.label(RichText::new(error).size(10.0).strong().color(DANGER));
        }
    }

    fn filter_and_sort_rows(&self) -> Vec<TorrentRow> {
        let search = self.search.trim().to_ascii_lowercase();
        let mut rows = self
            .torrents
            .iter()
            .filter(|row| match self.filter {
                TransferFilter::All => true,
                TransferFilter::Downloading => !row.finished && !is_paused(row),
                TransferFilter::Completed => row.finished,
                TransferFilter::Paused => is_paused(row),
            })
            .filter(|row| {
                search.is_empty()
                    || row.name.to_ascii_lowercase().contains(&search)
                    || row.policy.tag.to_ascii_lowercase().contains(&search)
                    || row.info_hash.to_ascii_lowercase().contains(&search)
            })
            .cloned()
            .collect::<Vec<_>>();
        match self.sort {
            SortMode::Added => {
                rows.sort_by_key(|row| std::cmp::Reverse((row.policy.added_at_unix, row.id)))
            }
            SortMode::Name => rows.sort_by_key(|row| row.name.to_ascii_lowercase()),
            SortMode::Progress => rows.sort_by(|a, b| {
                b.progress
                    .partial_cmp(&a.progress)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            SortMode::Size => rows.sort_by_key(|row| std::cmp::Reverse(row.total)),
            SortMode::Speed => rows.sort_by(|a, b| {
                b.down_speed_mib
                    .max(b.up_speed_mib)
                    .partial_cmp(&a.down_speed_mib.max(a.up_speed_mib))
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
        }
        rows
    }

    fn transfer_table(&mut self, ui: &mut egui::Ui) {
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.heading(RichText::new("Your transfers").size(18.0));
            ui.label(
                RichText::new(format!("{} total", self.torrents.len()))
                    .size(9.0)
                    .color(TEXT_TERTIARY),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ComboBox::from_id_salt("sort-transfers")
                    .selected_text(match self.sort {
                        SortMode::Added => "Added",
                        SortMode::Name => "Name",
                        SortMode::Progress => "Progress",
                        SortMode::Size => "Size",
                        SortMode::Speed => "Speed",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.sort, SortMode::Added, "Added");
                        ui.selectable_value(&mut self.sort, SortMode::Name, "Name");
                        ui.selectable_value(&mut self.sort, SortMode::Progress, "Progress");
                        ui.selectable_value(&mut self.sort, SortMode::Size, "Size");
                        ui.selectable_value(&mut self.sort, SortMode::Speed, "Speed");
                    });
                let search_label = ui.label("Search");
                ui.add_sized(
                    [220.0, 30.0],
                    egui::TextEdit::singleline(&mut self.search).hint_text("Name, tag, or hash"),
                )
                .labelled_by(search_label.id);
            });
        });
        ui.add_space(9.0);
        let rows = self.filter_and_sort_rows();
        if rows.is_empty() {
            Frame::new()
                .fill(KRYPTON_CARD)
                .stroke(Stroke::new(1.0, KRYPTON_STROKE))
                .corner_radius(CornerRadius::same(11))
                .inner_margin(Margin::same(30))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(if self.torrents.is_empty() {
                                "Your local queue is empty."
                            } else {
                                "No transfers match this view."
                            })
                            .size(16.0)
                            .strong(),
                        );
                        ui.label(
                            RichText::new(
                                "Drop a .torrent anywhere in this window or review one above.",
                            )
                            .size(10.0)
                            .color(TEXT_SECONDARY),
                        );
                    });
                });
            return;
        }

        for torrent in rows {
            let selected = self.selected == Some(torrent.id);
            let border = if selected {
                KRYPTON_PURPLE
            } else if torrent.finished {
                RADIOACTIVE_SOFT
            } else {
                KRYPTON_STROKE
            };
            let response = Frame::new()
                .fill(if selected {
                    KRYPTON_HOVER
                } else {
                    KRYPTON_CARD
                })
                .stroke(Stroke::new(if selected { 1.5 } else { 1.0 }, border))
                .corner_radius(CornerRadius::same(9))
                .inner_margin(Margin::symmetric(13, 11))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (icon, _) =
                            ui.allocate_exact_size(Vec2::new(38.0, 43.0), egui::Sense::hover());
                        ui.painter().rect_filled(
                            icon,
                            CornerRadius::same(6),
                            if torrent.finished {
                                RADIOACTIVE
                            } else {
                                KRYPTON_SELECTED
                            },
                        );
                        ui.painter().text(
                            icon.center(),
                            egui::Align2::CENTER_CENTER,
                            if torrent.finished { "OK" } else { "TOR" },
                            FontId::proportional(8.0),
                            if torrent.finished {
                                RADIOACTIVE_DARK
                            } else {
                                TEXT_PRIMARY
                            },
                        );
                        ui.add_space(6.0);
                        ui.vertical(|ui| {
                            ui.set_width((ui.available_width() - 305.0).max(160.0));
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&torrent.name).size(12.0).strong());
                                if !torrent.policy.tag.trim().is_empty() {
                                    ui.label(
                                        RichText::new(format!("  #{}", torrent.policy.tag))
                                            .size(9.0)
                                            .color(RADIOACTIVE),
                                    );
                                }
                            });
                            let bar_label = format!(
                                "{:>3.0}%  //  {}",
                                torrent.progress * 100.0,
                                torrent.status.to_uppercase()
                            );
                            krypton_progress_bar(
                                ui,
                                torrent.progress,
                                transfer_bar_color(&torrent),
                                &bar_label,
                            )
                            .on_hover_text("Transfer completion and current engine state");
                            ui.label(
                                RichText::new(format!(
                                    "{} of {} · {} · {}",
                                    format_bytes(torrent.downloaded),
                                    format_bytes(torrent.total),
                                    torrent.status,
                                    torrent.policy.share_policy.label()
                                ))
                                .size(8.5)
                                .color(TEXT_TERTIARY),
                            );
                        });
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(format!("↓ {}", torrent.down_speed))
                                    .size(9.5)
                                    .strong()
                                    .color(RADIOACTIVE),
                            );
                            ui.label(
                                RichText::new(format!("↑ {}", torrent.up_speed))
                                    .size(9.0)
                                    .color(KRYPTON_PURPLE_HOVER),
                            );
                            if !torrent.eta.is_empty() {
                                ui.label(
                                    RichText::new(format!("ETA {}", torrent.eta))
                                        .size(8.0)
                                        .color(TEXT_TERTIARY),
                                );
                            }
                        });
                        let paused = is_paused(&torrent);
                        if ui
                            .add(
                                egui::Button::new(if paused { "Resume" } else { "Pause" })
                                    .fill(if paused { RADIOACTIVE } else { KRYPTON_ACTIVE }),
                            )
                            .clicked()
                        {
                            self.send(if paused {
                                EngineCommand::Resume(torrent.id)
                            } else {
                                EngineCommand::Pause(torrent.id)
                            });
                        }
                        if ui
                            .add(
                                egui::Button::new(RichText::new("Remove").color(DANGER))
                                    .fill(KRYPTON_ACTIVE),
                            )
                            .clicked()
                        {
                            self.remove_prompt = Some(RemovePrompt {
                                id: torrent.id,
                                name: torrent.name.clone(),
                                output_folder: torrent.output_folder.clone(),
                                total: torrent.total,
                            });
                        }
                    });
                });
            if response.response.clicked() {
                self.selected = Some(torrent.id);
                self.editor_for = None;
                self.sync_editors();
            }
            ui.add_space(7.0);
        }
    }

    fn details_panel(&mut self, ui: &mut egui::Ui) {
        let Some(row) = self
            .selected
            .and_then(|id| self.torrents.iter().find(|row| row.id == id))
            .cloned()
        else {
            return;
        };
        ui.add_space(12.0);
        Frame::new()
            .fill(KRYPTON_CARD)
            .stroke(Stroke::new(1.0, KRYPTON_STROKE))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::same(15))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(RichText::new("Transfer details").size(16.0));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Open folder").clicked()
                            && let Err(error) = open_folder(&row.output_folder)
                        {
                            self.error = Some(format!("Could not open folder: {error}"));
                        }
                        if ui.button("Copy magnet").clicked() {
                            ui.ctx()
                                .copy_text(format!("magnet:?xt=urn:btih:{}", row.info_hash));
                            self.notice = "Magnet link copied".to_owned();
                        }
                        if ui.button("Copy diagnostics").clicked() {
                            ui.ctx().copy_text(format!(
                                "Orbit transfer diagnostics\nName: {}\nInfo hash: {}\nStatus: {}\nEngine state: {}\nProgress: {:.2}%\nDownloaded: {}\nUploaded: {}\nDownload speed: {}\nUpload speed: {}\nPeers: {} connected, {} connecting, {} queued, {} seen, {} failed\nDestination: {}\nError: {}",
                                row.name,
                                row.info_hash,
                                row.status,
                                row.state,
                                row.progress * 100.0,
                                format_bytes(row.downloaded),
                                format_bytes(row.uploaded),
                                row.down_speed,
                                row.up_speed,
                                row.connected_peers,
                                row.connecting_peers,
                                row.queued_peers,
                                row.discovered_peers,
                                row.failed_peers,
                                row.output_folder,
                                row.error.as_deref().unwrap_or("none"),
                            ));
                            self.notice = "Transfer diagnostics copied".to_owned();
                        }
                    });
                });
                ui.label(
                    RichText::new(format!("Hash  {}", row.info_hash))
                        .size(9.0)
                        .color(TEXT_TERTIARY),
                );
                ui.label(
                    RichText::new(format!("Save path  {}", row.output_folder))
                        .size(9.0)
                        .color(TEXT_TERTIARY),
                );
                let ratio = if row.downloaded == 0 {
                    0.0
                } else {
                    row.uploaded as f64 / row.downloaded as f64
                };
                ui.label(
                    RichText::new(format!(
                        "Uploaded {} · Ratio {:.2} · {} files",
                        format_bytes(row.uploaded),
                        ratio,
                        row.files.len()
                    ))
                    .size(9.0)
                    .color(TEXT_SECONDARY),
                );
                ui.label(
                    RichText::new(format!(
                        "State {} · Engine {} · Down {} · Up {}{}",
                        row.status,
                        row.state,
                        row.down_speed,
                        row.up_speed,
                        if row.eta.is_empty() {
                            String::new()
                        } else {
                            format!(" · ETA {}", row.eta)
                        }
                    ))
                    .size(9.0)
                    .color(TEXT_SECONDARY),
                );
                ui.label(
                    RichText::new(format!(
                        "Peers {} connected · {} connecting · {} queued · {} seen · {} failed",
                        row.connected_peers,
                        row.connecting_peers,
                        row.queued_peers,
                        row.discovered_peers,
                        row.failed_peers
                    ))
                    .size(9.0)
                    .color(TEXT_SECONDARY),
                );
                if let Some(error) = &row.error {
                    ui.label(RichText::new(error).color(DANGER));
                }
                ui.separator();

                let command_tx = self.command_tx.clone();
                if let Some(policy) = self.policy_editor.as_mut() {
                    ui.horizontal(|ui| {
                        ui.label("Completion:");
                        ComboBox::from_id_salt("selected-share-policy")
                            .selected_text(policy.share_policy.label())
                            .show_ui(ui, |ui| {
                                for value in [
                                    SharePolicy::Seed,
                                    SharePolicy::StopWhenComplete,
                                    SharePolicy::RemoveWhenComplete,
                                ] {
                                    ui.selectable_value(
                                        &mut policy.share_policy,
                                        value,
                                        value.label(),
                                    );
                                }
                            });
                        let tag_label = ui.label("Tag:");
                        ui.add_sized(
                            [130.0, 26.0],
                            egui::TextEdit::singleline(&mut policy.tag).hint_text("optional"),
                        )
                        .labelled_by(tag_label.id);
                        ui.add_enabled_ui(policy.source_torrent.is_some(), |ui| {
                            ui.checkbox(
                                &mut policy.delete_source_when_complete,
                                "Delete source .torrent at completion",
                            );
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.add_enabled_ui(policy.share_policy == SharePolicy::Seed, |ui| {
                            ui.label("Seed ratio target");
                            ui.add(
                                egui::DragValue::new(&mut policy.seed_ratio_percent)
                                    .range(0..=100_000)
                                    .suffix("%"),
                            );
                            ui.label("Seed time");
                            ui.add(
                                egui::DragValue::new(&mut policy.seed_time_minutes)
                                    .range(0..=525_600)
                                    .suffix(" min"),
                            );
                            ui.label(
                                RichText::new("0 = unlimited")
                                    .size(8.5)
                                    .color(TEXT_TERTIARY),
                            );
                        });
                        if ui.button("Apply policy").clicked() {
                            let _ = command_tx.send(EngineCommand::UpdatePolicy {
                                info_hash: row.info_hash.clone(),
                                policy: policy.clone(),
                            });
                        }
                    });
                }

                if !row.files.is_empty() {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Files").strong());
                        if ui.small_button("All").clicked() {
                            self.file_selection = row.files.iter().map(|file| file.index).collect();
                        }
                        if ui.small_button("None").clicked() {
                            self.file_selection.clear();
                        }
                        if ui.button("Apply file selection").clicked() {
                            if self.file_selection.is_empty() {
                                self.error = Some("Select at least one file.".to_owned());
                            } else {
                                let mut only_files =
                                    self.file_selection.iter().copied().collect::<Vec<_>>();
                                only_files.sort_unstable();
                                self.send(EngineCommand::UpdateFiles {
                                    id: row.id,
                                    only_files,
                                });
                            }
                        }
                    });
                    ScrollArea::vertical()
                        .id_salt("details-files")
                        .max_height(155.0)
                        .show(ui, |ui| {
                            for file in &row.files {
                                ui.horizontal(|ui| {
                                    let mut selected = self.file_selection.contains(&file.index);
                                    if ui
                                        .checkbox(
                                            &mut selected,
                                            RichText::new(&file.name).size(9.5),
                                        )
                                        .changed()
                                    {
                                        if selected {
                                            self.file_selection.insert(file.index);
                                        } else {
                                            self.file_selection.remove(&file.index);
                                        }
                                    }
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        ui.label(
                                            RichText::new(format!(
                                                "{} / {}",
                                                format_bytes(file.downloaded),
                                                format_bytes(file.length)
                                            ))
                                            .size(8.5)
                                            .color(TEXT_TERTIARY),
                                        );
                                    });
                                });
                            }
                        });
                }
            });
    }

    fn preview_modal(&mut self, ctx: &egui::Context) {
        let mut cancel = false;
        let mut add = false;
        if let Some(draft) = self.previews.front_mut() {
            egui::Window::new("Review transfer")
                .id(egui::Id::new("review-transfer"))
                .collapsible(false)
                .resizable(true)
                .default_width(650.0)
                .show(ctx, |ui| {
                    ui.label(RichText::new(&draft.name).size(18.0).strong());
                    ui.label(
                        RichText::new(format!(
                            "{} · {} files · {}",
                            draft.info_hash,
                            draft.files.len(),
                            format_bytes(draft.total)
                        ))
                        .size(9.0)
                        .color(TEXT_TERTIARY),
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let destination_label = ui.label("Destination:");
                        ui.add_sized(
                            [430.0, 28.0],
                            egui::TextEdit::singleline(&mut draft.output_folder),
                        )
                        .labelled_by(destination_label.id);
                        if ui.button("Browse…").clicked()
                            && let Some(folder) = rfd::FileDialog::new().pick_folder()
                        {
                            draft.output_folder = folder.to_string_lossy().into_owned();
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut draft.start_paused, "Start paused");
                        ComboBox::from_id_salt("new-share-policy")
                            .selected_text(draft.share_policy.label())
                            .show_ui(ui, |ui| {
                                for value in [
                                    SharePolicy::Seed,
                                    SharePolicy::StopWhenComplete,
                                    SharePolicy::RemoveWhenComplete,
                                ] {
                                    ui.selectable_value(
                                        &mut draft.share_policy,
                                        value,
                                        value.label(),
                                    );
                                }
                            });
                        let local_source = Path::new(&draft.source).is_file();
                        ui.add_enabled_ui(local_source, |ui| {
                            ui.checkbox(
                                &mut draft.delete_source_when_complete,
                                "Delete source .torrent when complete",
                            );
                        });
                    });
                    ui.add_enabled_ui(draft.share_policy == SharePolicy::Seed, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Seed ratio target");
                            ui.add(
                                egui::DragValue::new(&mut draft.seed_ratio_percent)
                                    .range(0..=100_000)
                                    .suffix("%"),
                            );
                            ui.label("Seed time");
                            ui.add(
                                egui::DragValue::new(&mut draft.seed_time_minutes)
                                    .range(0..=525_600)
                                    .suffix(" min"),
                            );
                            ui.label(
                                RichText::new("0 = unlimited")
                                    .size(8.5)
                                    .color(TEXT_TERTIARY),
                            );
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.label("Download KiB/s (0 unlimited)");
                        ui.add(egui::DragValue::new(&mut draft.download_limit_kib));
                        ui.label("Upload KiB/s");
                        ui.add(egui::DragValue::new(&mut draft.upload_limit_kib));
                        let tag_label = ui.label("Tag");
                        ui.add_sized([120.0, 26.0], egui::TextEdit::singleline(&mut draft.tag))
                            .labelled_by(tag_label.id);
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Choose files").strong());
                        if ui.small_button("Select all").clicked() {
                            for file in &mut draft.files {
                                file.selected = true;
                            }
                        }
                        if ui.small_button("Select none").clicked() {
                            for file in &mut draft.files {
                                file.selected = false;
                            }
                        }
                    });
                    ScrollArea::vertical()
                        .id_salt("preview-files")
                        .max_height(240.0)
                        .show(ui, |ui| {
                            for file in &mut draft.files {
                                ui.horizontal(|ui| {
                                    ui.checkbox(&mut file.selected, &file.name);
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        ui.label(
                                            RichText::new(format_bytes(file.length))
                                                .size(8.5)
                                                .color(TEXT_TERTIARY),
                                        );
                                    });
                                });
                            }
                        });
                    let selected_count = draft.files.iter().filter(|file| file.selected).count();
                    let selected_size: u64 = draft
                        .files
                        .iter()
                        .filter(|file| file.selected)
                        .map(|file| file.length)
                        .sum();
                    let storage = storage_preflight(&draft.output_folder, selected_size);
                    ui.label(
                        RichText::new(format!(
                            "{selected_count} selected · {}",
                            format_bytes(selected_size)
                        ))
                        .size(9.0)
                        .color(RADIOACTIVE),
                    );
                    match &storage {
                        Ok(Some(available)) => {
                            ui.label(
                                RichText::new(format!(
                                    "Storage preflight passed · {} available after reserving {}",
                                    format_bytes(available.saturating_sub(selected_size)),
                                    format_bytes(selected_size)
                                ))
                                .size(9.0)
                                .color(RADIOACTIVE_SOFT),
                            );
                        }
                        Ok(None) => {
                            ui.label(
                                RichText::new("Destination path is valid")
                                    .size(9.0)
                                    .color(RADIOACTIVE_SOFT),
                            );
                        }
                        Err(message) => {
                            ui.label(RichText::new(message).size(9.0).color(DANGER));
                        }
                    }
                    ui.label(
                        RichText::new(
                            "Start only content you are authorized to download and share.",
                        )
                        .size(9.0)
                        .color(TEXT_SECONDARY),
                    );
                    ui.label(
                        RichText::new(
                            "Peer Exchange (PEX) may be used by the embedded engine and has no \
                             separate switch in this build. Disabling DHT does not disable PEX.",
                        )
                        .size(9.0)
                        .color(TEXT_TERTIARY),
                    );
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            cancel = true;
                        }
                        if ui
                            .add_enabled(
                                selected_count > 0 && storage.is_ok(),
                                egui::Button::new(RichText::new("Start transfer").strong())
                                    .fill(KRYPTON_SELECTED),
                            )
                            .clicked()
                        {
                            add = true;
                        }
                    });
                });
        }
        if cancel {
            self.previews.pop_front();
        } else if add && let Some(draft) = self.previews.pop_front() {
            let selected_files = draft
                .files
                .iter()
                .filter(|file| file.selected)
                .map(|file| file.index)
                .collect();
            self.send(EngineCommand::Add(AddRequest {
                source: draft.source,
                info_hash: draft.info_hash,
                name: draft.name,
                output_folder: draft.output_folder,
                selected_files,
                start_paused: draft.start_paused,
                share_policy: draft.share_policy,
                seed_ratio_percent: draft.seed_ratio_percent,
                seed_time_minutes: draft.seed_time_minutes,
                delete_source_when_complete: draft.delete_source_when_complete,
                upload_limit_kib: draft.upload_limit_kib,
                download_limit_kib: draft.download_limit_kib,
                tag: draft.tag,
            }));
        }
    }

    fn remove_modal(&mut self, ctx: &egui::Context) {
        let Some(prompt) = self.remove_prompt.clone() else {
            return;
        };
        let mut close = false;
        let mut keep = false;
        let mut delete = false;
        egui::Window::new("Remove transfer?")
            .id(egui::Id::new("remove-transfer"))
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(RichText::new(&prompt.name).size(16.0).strong());
                ui.label(
                    RichText::new(format!(
                        "{} · {}",
                        format_bytes(prompt.total),
                        prompt.output_folder
                    ))
                    .size(9.0)
                    .color(TEXT_TERTIARY),
                );
                ui.add_space(8.0);
                ui.label("Choose carefully. Deleting data cannot be undone by Orbit.");
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                    if ui.button("Remove, keep files").clicked() {
                        keep = true;
                    }
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("Remove and delete data").color(Color32::WHITE),
                            )
                            .fill(DANGER),
                        )
                        .clicked()
                    {
                        delete = true;
                    }
                });
            });
        if keep {
            self.send(EngineCommand::Forget(prompt.id));
            self.remove_prompt = None;
        } else if delete {
            self.send(EngineCommand::DeleteData(prompt.id));
            self.remove_prompt = None;
        } else if close {
            self.remove_prompt = None;
        }
    }

    fn about_modal(&mut self, ctx: &egui::Context) {
        if !self.show_about {
            return;
        }
        let mut close = false;
        egui::Window::new("About Orbit")
            .id(egui::Id::new("orbit-about"))
            .collapsible(false)
            .resizable(false)
            .default_width(540.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(42.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(rect, CornerRadius::same(11), KRYPTON_SELECTED);
                    ui.painter()
                        .circle_stroke(rect.center(), 12.0, Stroke::new(1.6, RADIOACTIVE));
                    ui.painter().circle_filled(rect.center(), 5.0, RADIOACTIVE);
                    ui.vertical(|ui| {
                        ui.heading(RichText::new("Orbit Torrent Manager").size(20.0));
                        ui.label(
                            RichText::new(format!(
                                "Version {} · Native Rust client",
                                env!("CARGO_PKG_VERSION")
                            ))
                            .size(9.5)
                            .color(TEXT_TERTIARY),
                        );
                    });
                });
                ui.add_space(8.0);
                ui.label(
                    RichText::new(
                        "Local-first BitTorrent transfers with no Orbit account, advertising, \
                         analytics, telemetry, cloud sync, or required hosted backend.",
                    )
                    .size(10.0)
                    .color(TEXT_SECONDARY),
                );
                ui.separator();
                ui.label(RichText::new("Current privacy state").strong());
                ui.label(
                    RichText::new(self.settings.effective_network_mode().label())
                        .color(RADIOACTIVE_SOFT),
                );
                ui.label(
                    RichText::new(format!(
                        "DHT {} · Incoming peers {} · UPnP {}",
                        if self.settings.effective_network_mode() == NetworkMode::Offline {
                            "inactive"
                        } else if self.settings.dht_enabled {
                            "enabled"
                        } else {
                            "disabled"
                        },
                        if self.settings.effective_network_mode() != NetworkMode::Offline
                            && self.settings.incoming_connections_enabled
                        {
                            "enabled"
                        } else {
                            "disabled"
                        },
                        if self.settings.effective_network_mode() != NetworkMode::Offline
                            && self.settings.incoming_connections_enabled
                            && self.settings.upnp_enabled
                        {
                            "enabled"
                        } else {
                            "disabled"
                        },
                    ))
                    .size(9.0)
                    .color(TEXT_TERTIARY),
                );
                ui.label(
                    RichText::new(
                        "PEX: engine-managed. Peer Exchange may operate for supported torrents; \
                         this build does not expose a separate PEX switch.",
                    )
                    .size(9.0)
                    .color(TEXT_TERTIARY),
                );
                ui.label(
                    RichText::new(format!(
                        "Background transfers: {}",
                        if self.settings.close_to_tray {
                            "opted in — closing the window keeps Orbit in the tray"
                        } else {
                            "off — closing the window exits Orbit"
                        }
                    ))
                    .size(9.0)
                    .color(TEXT_TERTIARY),
                );
                ui.separator();
                ui.label(RichText::new("Device-local data").strong());
                ui.label(
                    RichText::new(data_dir().display().to_string())
                        .size(9.0)
                        .color(TEXT_TERTIARY),
                );
                ui.horizontal(|ui| {
                    if ui.button("Open app data").clicked()
                        && let Err(error) = open_folder(&data_dir().to_string_lossy())
                    {
                        self.error = Some(format!("Could not open app data: {error}"));
                    }
                    if ui.button("Open downloads").clicked()
                        && let Err(error) = open_folder(&self.download_dir.to_string_lossy())
                    {
                        self.error = Some(format!("Could not open downloads: {error}"));
                    }
                    if ui.button("Copy system info").clicked() {
                        ui.ctx().copy_text(format!(
                            "Orbit {}\nOS: {} {}\nMode: {}\nCore: {}",
                            env!("CARGO_PKG_VERSION"),
                            std::env::consts::OS,
                            std::env::consts::ARCH,
                            self.settings.effective_network_mode().label(),
                            self.engine_status,
                        ));
                        self.notice = "Orbit system info copied".to_owned();
                    }
                });
                ui.separator();
                ui.label(
                    RichText::new(
                        "Orbit is licensed under GNU GPL version 3. Bundled open-source \
                         components retain their respective licenses.",
                    )
                    .size(9.0)
                    .color(TEXT_SECONDARY),
                );
                ui.label(
                    RichText::new(
                        "Use Orbit only for content you are authorized to download and share.",
                    )
                    .size(9.0)
                    .color(WARNING),
                );
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        if close {
            self.show_about = false;
        }
    }

    fn startup_setup_modal(&mut self, ctx: &egui::Context) {
        if !self.show_startup_setup {
            return;
        }

        let mut apply = false;
        egui::Window::new(if self.first_run_setup {
            "Welcome to Orbit"
        } else {
            "Choose this session's network mode"
        })
        .id(egui::Id::new("orbit-startup-setup"))
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .collapsible(false)
        .resizable(true)
        .default_width(700.0)
        .show(ctx, |ui| {
            ScrollArea::vertical()
                .id_salt("startup-setup-scroll")
                .max_height(680.0)
                .show(ui, |ui| {
                    Frame::new()
                        .fill(KRYPTON_CARD)
                        .stroke(Stroke::new(1.0, KRYPTON_PURPLE))
                        .corner_radius(CornerRadius::same(12))
                        .inner_margin(Margin::same(18))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("KRYPTON  //  FIRST-RUN CONTROL")
                                    .size(9.0)
                                    .strong()
                                    .color(RADIOACTIVE),
                            );
                            ui.heading(
                                RichText::new("Your network. Your files. Your rules.")
                                    .size(22.0)
                                    .color(TEXT_PRIMARY),
                            );
                            ui.label(
                                RichText::new(
                                    "Orbit remains offline until you confirm these settings.",
                                )
                                .color(TEXT_SECONDARY),
                            );
                        });

                    ui.add_space(12.0);
                    ui.heading(RichText::new("1  Network mode").size(16.0));
                    let mut network_mode = self.settings_editor.configured_network_mode();
                    ui.horizontal(|ui| {
                        ui.label("Default mode");
                        network_mode_combo(ui, "setup-default-network-mode", &mut network_mode);
                    });
                    self.settings_editor.network_mode = Some(network_mode);
                    self.settings_editor.block_all_uploads = network_mode != NetworkMode::Standard;
                    ui.label(
                        RichText::new(match network_mode {
                            NetworkMode::Offline => {
                                "No torrent session, trackers, DHT, PEX, or peer connections."
                            }
                            NetworkMode::DownloadOnly => {
                                "Downloads are allowed; BitTorrent piece uploads are blocked by the engine."
                            }
                            NetworkMode::Standard => {
                                "Downloads and sharing are allowed under each transfer's policy."
                            }
                        })
                        .size(9.0)
                        .color(if network_mode == NetworkMode::Standard {
                            WARNING
                        } else {
                            TEXT_TERTIARY
                        }),
                    );

                    ui.add_space(10.0);
                    ui.heading(RichText::new("2  When should Orbit ask?").size(16.0));
                    for policy in [
                        StartupNetworkPolicy::AskEveryStart,
                        StartupNetworkPolicy::RememberChoice,
                        StartupNetworkPolicy::FollowSystem,
                    ] {
                        ui.radio_value(
                            &mut self.settings_editor.startup_network_policy,
                            policy,
                            policy.label(),
                        );
                    }

                    if self.settings_editor.startup_network_policy
                        == StartupNetworkPolicy::FollowSystem
                    {
                        ui.add_space(8.0);
                        Frame::new()
                            .fill(KRYPTON_SUBTLE)
                            .stroke(Stroke::new(1.0, KRYPTON_STROKE))
                            .corner_radius(CornerRadius::same(9))
                            .inner_margin(Margin::same(12))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "●  WINDOWS REPORTS: {}",
                                        self.system_network_profile.label().to_uppercase()
                                    ))
                                    .size(9.0)
                                    .strong()
                                    .color(RADIOACTIVE),
                                );
                                for (label, id, mode) in [
                                    (
                                        "Private or domain",
                                        "setup-system-private",
                                        &mut self.settings_editor.system_private_mode,
                                    ),
                                    (
                                        "Public",
                                        "setup-system-public",
                                        &mut self.settings_editor.system_public_mode,
                                    ),
                                    (
                                        "Metered or cellular",
                                        "setup-system-metered",
                                        &mut self.settings_editor.system_metered_mode,
                                    ),
                                    (
                                        "Unclassified",
                                        "setup-system-unknown",
                                        &mut self.settings_editor.system_unknown_mode,
                                    ),
                                ] {
                                    ui.horizontal(|ui| {
                                        ui.add_sized([150.0, 22.0], egui::Label::new(label));
                                        network_mode_combo(ui, id, mode);
                                    });
                                }
                                ui.label(
                                    RichText::new(
                                        "Disconnected always resolves to Offline. System detection never starts a torrent by itself.",
                                    )
                                    .size(9.0)
                                    .color(TEXT_TERTIARY),
                                );
                            });
                    }

                    if self.first_run_setup {
                        ui.add_space(10.0);
                        ui.heading(RichText::new("3  File defaults").size(16.0));
                        let destination_label = ui.label("Save downloads to");
                        ui.horizontal(|ui| {
                            ui.add_sized(
                                [540.0, 28.0],
                                egui::TextEdit::singleline(
                                    &mut self.settings_editor.default_download_dir,
                                ),
                            )
                            .labelled_by(destination_label.id);
                            if ui.button("Browse…").clicked()
                                && let Some(folder) = rfd::FileDialog::new().pick_folder()
                            {
                                self.settings_editor.default_download_dir =
                                    folder.to_string_lossy().into_owned();
                            }
                        });
                        ui.checkbox(
                            &mut self.settings_editor.auto_delete_torrent,
                            "Delete an imported local .torrent after its download completes",
                        );
                        ui.label(
                            RichText::new(
                                "Downloaded content is kept. Orbit deletes only the original descriptor and only when its SHA-256 identity still matches.",
                            )
                            .size(9.0)
                            .color(TEXT_TERTIARY),
                        );
                    }

                    ui.add_space(14.0);
                    let resolved_mode = if self.settings_editor.startup_network_policy
                        == StartupNetworkPolicy::FollowSystem
                    {
                        self.settings_editor
                            .mode_for_system_profile(self.system_network_profile)
                    } else {
                        self.settings_editor.configured_network_mode()
                    };
                    if ui
                        .add_sized(
                            [260.0, 38.0],
                            egui::Button::new(
                                RichText::new(format!("Save and start {}", resolved_mode.label()))
                                    .strong(),
                            )
                            .fill(KRYPTON_SELECTED),
                        )
                        .clicked()
                    {
                        apply = true;
                    }
                    ui.label(
                        RichText::new("You can change every option later in Settings.")
                            .size(9.0)
                            .color(TEXT_TERTIARY),
                    );
                });
        });

        if apply {
            if self.settings_editor.default_download_dir.trim().is_empty() {
                self.error = Some("Choose a default download folder.".to_owned());
                return;
            }
            if let Err(error) = std::fs::create_dir_all(&self.settings_editor.default_download_dir)
            {
                self.error = Some(format!("Could not create the download folder: {error}"));
                return;
            }

            let mut persisted = self.settings_editor.clone();
            persisted.network_setup_completed = true;
            persisted.sharing_consent_recorded = true;
            let configured_mode = persisted.configured_network_mode();
            persisted.block_all_uploads = configured_mode != NetworkMode::Standard;
            if let Err(error) = save_settings(&persisted) {
                self.error = Some(format!("Could not save Network Setup: {error:#}"));
                return;
            }

            let mut runtime_settings = persisted.clone();
            let runtime_mode =
                if persisted.startup_network_policy == StartupNetworkPolicy::FollowSystem {
                    persisted.mode_for_system_profile(self.system_network_profile)
                } else {
                    persisted.configured_network_mode()
                };
            runtime_settings.network_mode = Some(runtime_mode);
            runtime_settings.sharing_consent_recorded = true;
            runtime_settings.block_all_uploads = runtime_mode != NetworkMode::Standard;
            match self.restart_engine(ctx, runtime_settings) {
                Ok(()) => {
                    self.settings_editor = persisted;
                    self.show_startup_setup = false;
                    self.first_run_setup = false;
                    self.notice = format!("Network Setup saved · {}", runtime_mode.label());
                    self.error = None;
                }
                Err(error) => self.error = Some(error),
            }
        }
    }

    fn settings_modal(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }
        let mut cancel = false;
        let mut save = false;
        let mut exit = false;
        egui::Window::new("Orbit settings")
            .id(egui::Id::new("orbit-settings"))
            .collapsible(false)
            .resizable(true)
            .default_width(620.0)
            .show(ctx, |ui| {
                ScrollArea::vertical()
                    .id_salt("settings-scroll")
                    .max_height(620.0)
                    .show(ui, |ui| {
                ui.heading(RichText::new("Privacy & sharing").size(16.0));
                if !self.settings.sharing_consent_recorded {
                    ui.label(
                        RichText::new(
                            "Network choice required. Orbit is fully offline until you save a mode.",
                        )
                        .strong()
                        .color(WARNING),
                    );
                }
                let mut network_mode = self.settings_editor.configured_network_mode();
                ui.horizontal(|ui| {
                    ui.label("Torrent network mode");
                    ComboBox::from_id_salt("network-mode")
                        .selected_text(network_mode.label())
                        .show_ui(ui, |ui| {
                            for value in [
                                NetworkMode::Offline,
                                NetworkMode::DownloadOnly,
                                NetworkMode::Standard,
                            ] {
                                ui.selectable_value(&mut network_mode, value, value.label());
                            }
                        });
                });
                self.settings_editor.network_mode = Some(network_mode);
                self.settings_editor.block_all_uploads =
                    network_mode != NetworkMode::Standard;
                ui.label(
                    RichText::new(
                        match network_mode {
                            NetworkMode::Offline => {
                                "No torrent engine session is created: no DHT, trackers, peers, restores, downloads, or uploads."
                            }
                            NetworkMode::DownloadOnly => {
                                "The engine may contact DHT, trackers, and peers, but its upload path is blocked."
                            }
                            NetworkMode::Standard => {
                                "Transfers may upload pieces. Per-transfer completion and seeding limits still apply."
                            }
                        },
                    )
                    .size(9.0)
                    .color(TEXT_TERTIARY),
                );
                ui.add_enabled_ui(network_mode != NetworkMode::Offline, |ui| {
                    ui.checkbox(
                        &mut self.settings_editor.dht_enabled,
                        "DHT peer discovery (needed for many trackerless magnets)",
                    );
                });
                ui.label(
                    RichText::new(
                        "DHT and peer discovery reveal your IP address to other participants. \
                         Download Only blocks file uploads; it does not provide anonymity. \
                         Peer Exchange (PEX) may be used by the engine and has no separate switch \
                         in this build. Disabling DHT does not disable PEX. \
                         Saving authorizes Orbit to use and immediately apply the selected mode.",
                    )
                    .size(9.0)
                    .color(TEXT_TERTIARY),
                );
                ui.add_space(8.0);
                ui.heading(RichText::new("Startup network selection").size(16.0));
                for policy in [
                    StartupNetworkPolicy::AskEveryStart,
                    StartupNetworkPolicy::RememberChoice,
                    StartupNetworkPolicy::FollowSystem,
                ] {
                    ui.radio_value(
                        &mut self.settings_editor.startup_network_policy,
                        policy,
                        policy.label(),
                    );
                }
                if self.settings_editor.startup_network_policy
                    == StartupNetworkPolicy::FollowSystem
                {
                    Frame::new()
                        .fill(KRYPTON_SUBTLE)
                        .stroke(Stroke::new(1.0, KRYPTON_STROKE))
                        .corner_radius(CornerRadius::same(9))
                        .inner_margin(Margin::same(10))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(format!(
                                    "Current network profile: {}",
                                    self.system_network_profile.label()
                                ))
                                .strong()
                                .color(RADIOACTIVE_SOFT),
                            );
                            for (label, id, mode) in [
                                (
                                    "Private or domain",
                                    "settings-system-private",
                                    &mut self.settings_editor.system_private_mode,
                                ),
                                (
                                    "Public",
                                    "settings-system-public",
                                    &mut self.settings_editor.system_public_mode,
                                ),
                                (
                                    "Metered or cellular",
                                    "settings-system-metered",
                                    &mut self.settings_editor.system_metered_mode,
                                ),
                                (
                                    "Unclassified",
                                    "settings-system-unknown",
                                    &mut self.settings_editor.system_unknown_mode,
                                ),
                            ] {
                                ui.horizontal(|ui| {
                                    ui.add_sized([150.0, 22.0], egui::Label::new(label));
                                    network_mode_combo(ui, id, mode);
                                });
                            }
                            ui.label(
                                RichText::new("Disconnected always uses Offline.")
                                    .size(9.0)
                                    .color(TEXT_TERTIARY),
                            );
                        });
                }
                ui.add_space(4.0);
                ui.heading(RichText::new("Connectivity").size(16.0));
                ui.add_enabled_ui(network_mode != NetworkMode::Offline, |ui| {
                    ui.checkbox(
                        &mut self.settings_editor.incoming_connections_enabled,
                        "Accept incoming peer connections",
                    );
                    ui.add_enabled_ui(
                        self.settings_editor.incoming_connections_enabled,
                        |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Listen ports");
                                ui.add(
                                    egui::DragValue::new(
                                        &mut self.settings_editor.listen_port_start,
                                    )
                                    .range(1_024..=65_534),
                                );
                                ui.label("through");
                                ui.add(
                                    egui::DragValue::new(
                                        &mut self.settings_editor.listen_port_end,
                                    )
                                    .range(1_024..=65_534),
                                );
                            });
                            ui.checkbox(
                                &mut self.settings_editor.upnp_enabled,
                                "Request automatic router port forwarding with UPnP",
                            );
                        },
                    );
                });
                ui.label(
                    RichText::new(
                        "Incoming connections can improve reachability. UPnP changes the router's \
                         port mapping while Orbit runs, so it is off by default. Saving safely \
                         restarts the embedded core when these settings change.",
                    )
                    .size(9.0)
                    .color(TEXT_TERTIARY),
                );
                if self.settings_editor.incoming_connections_enabled
                    && self.settings_editor.listen_port_end
                        < self.settings_editor.listen_port_start
                {
                    ui.label(
                        RichText::new("The ending port must be equal to or above the starting port.")
                            .size(9.0)
                            .color(DANGER),
                    );
                }
                ui.horizontal(|ui| {
                    ui.label("Default completion policy");
                    ComboBox::from_id_salt("default-share-policy")
                        .selected_text(self.settings_editor.default_share_policy.label())
                        .show_ui(ui, |ui| {
                            for value in [
                                SharePolicy::Seed,
                                SharePolicy::StopWhenComplete,
                                SharePolicy::RemoveWhenComplete,
                            ] {
                                ui.selectable_value(
                                    &mut self.settings_editor.default_share_policy,
                                    value,
                                    value.label(),
                                );
                            }
                        });
                });
                ui.add_enabled_ui(
                    self.settings_editor.default_share_policy == SharePolicy::Seed,
                    |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Default seed ratio target");
                            ui.add(
                                egui::DragValue::new(
                                    &mut self.settings_editor.default_seed_ratio_percent,
                                )
                                .range(0..=100_000)
                                .suffix("%"),
                            );
                            ui.label("Time");
                            ui.add(
                                egui::DragValue::new(
                                    &mut self.settings_editor.default_seed_time_minutes,
                                )
                                .range(0..=525_600)
                                .suffix(" min"),
                            );
                        });
                    },
                );
                ui.checkbox(
                    &mut self.settings_editor.auto_delete_torrent,
                    "Delete local source .torrent after a completed download",
                );
                ui.checkbox(
                    &mut self.settings_editor.start_paused,
                    "Add transfers paused",
                );
                ui.separator();
                ui.heading(RichText::new("Speed & queue").size(16.0));
                ui.horizontal(|ui| {
                    ui.label("Global download KiB/s");
                    ui.add(egui::DragValue::new(
                        &mut self.settings_editor.download_limit_kib,
                    ));
                    ui.label("Upload KiB/s");
                    ui.add(egui::DragValue::new(
                        &mut self.settings_editor.upload_limit_kib,
                    ));
                });
                ui.label(
                    RichText::new("0 means unlimited; it never means blocked.")
                        .size(8.5)
                        .color(TEXT_TERTIARY),
                );
                ui.horizontal(|ui| {
                    ui.label("Turtle download KiB/s");
                    ui.add(egui::DragValue::new(
                        &mut self.settings_editor.turtle_download_kib,
                    ));
                    ui.label("Turtle upload KiB/s");
                    ui.add(egui::DragValue::new(
                        &mut self.settings_editor.turtle_upload_kib,
                    ));
                });
                ui.horizontal(|ui| {
                    ui.label("Maximum active downloads (0 unlimited)");
                    ui.add(
                        egui::DragValue::new(&mut self.settings_editor.max_active_downloads)
                            .range(0..=100),
                    );
                });
                ui.separator();
                ui.heading(RichText::new("Storage").size(16.0));
                let default_dir_label = ui.label("Default download directory");
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [470.0, 27.0],
                        egui::TextEdit::singleline(
                            &mut self.settings_editor.default_download_dir,
                        ),
                    )
                    .labelled_by(default_dir_label.id);
                    if ui.button("Browse…").clicked()
                        && let Some(folder) = rfd::FileDialog::new().pick_folder()
                    {
                        self.settings_editor.default_download_dir =
                            folder.to_string_lossy().into_owned();
                    }
                });
                ui.label(
                    RichText::new("Default directory changes apply after Orbit restarts.")
                        .size(8.5)
                        .color(TEXT_TERTIARY),
                );
                ui.separator();
                ui.heading(RichText::new("Desktop & automation").size(16.0));
                ui.checkbox(
                    &mut self.settings_editor.close_to_tray,
                    "Allow transfers to continue after closing the window (keep Orbit in tray)",
                );
                ui.label(
                    RichText::new(
                        "Off by default. When enabled, closing hides the window but leaves the \
                         full Orbit process running. The tray menu shows transfer activity and \
                         provides Pause all, Resume all, and Exit Orbit.",
                    )
                    .size(9.0)
                    .color(if self.settings_editor.close_to_tray {
                        WARNING
                    } else {
                        TEXT_TERTIARY
                    }),
                );
                ui.checkbox(
                    &mut self.settings_editor.watch_enabled,
                    "Watch a folder for .torrent files",
                );
                let watch_dir_label = ui.label("Watched folder");
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [470.0, 27.0],
                        egui::TextEdit::singleline(&mut self.settings_editor.watch_folder),
                    )
                    .labelled_by(watch_dir_label.id);
                    if ui.button("Browse…").clicked()
                        && let Some(folder) = rfd::FileDialog::new().pick_folder()
                    {
                        self.settings_editor.watch_folder = folder.to_string_lossy().into_owned();
                    }
                });
                ui.horizontal(|ui| {
                    #[cfg(windows)]
                    if ui.button("Windows default-app settings").clicked()
                        && let Err(error) = open_default_apps()
                    {
                        self.error = Some(format!("Could not open Windows settings: {error}"));
                    }
                    if ui.button("Open download folder").clicked()
                        && let Err(error) = open_folder(&self.download_dir.to_string_lossy())
                    {
                        self.error = Some(format!("Could not open folder: {error}"));
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                    let settings_valid = !self.settings_editor.incoming_connections_enabled
                        || self.settings_editor.listen_port_end
                            >= self.settings_editor.listen_port_start;
                    if ui
                        .add_enabled(
                            settings_valid,
                            egui::Button::new(RichText::new("Save settings").strong())
                                .fill(KRYPTON_SELECTED),
                        )
                        .clicked()
                    {
                        save = true;
                    }
                    if ui
                        .add(egui::Button::new(RichText::new("Exit Orbit").color(DANGER)))
                        .clicked()
                    {
                        exit = true;
                    }
                });
                    });
            });
        if cancel {
            self.show_settings = false;
        } else if save {
            let watch_changed = self.settings.watch_folder != self.settings_editor.watch_folder
                || self.settings.watch_enabled != self.settings_editor.watch_enabled;
            if self.settings_editor.default_download_dir.trim().is_empty() {
                self.settings_editor.default_download_dir =
                    default_download_dir().to_string_lossy().into_owned();
            }
            if let Err(error) = std::fs::create_dir_all(&self.settings_editor.default_download_dir)
            {
                self.error = Some(format!("Could not create the download folder: {error}"));
                return;
            }

            self.settings_editor.network_setup_completed = true;
            self.settings_editor.sharing_consent_recorded = true;
            let configured_mode = self.settings_editor.configured_network_mode();
            self.settings_editor.block_all_uploads = configured_mode != NetworkMode::Standard;
            let persisted = self.settings_editor.clone();
            if let Err(error) = save_settings(&persisted) {
                self.error = Some(format!("Could not save settings: {error:#}"));
                return;
            }

            let mut runtime_settings = persisted.clone();
            let runtime_mode =
                if persisted.startup_network_policy == StartupNetworkPolicy::FollowSystem {
                    persisted.mode_for_system_profile(self.system_network_profile)
                } else {
                    persisted.configured_network_mode()
                };
            runtime_settings.network_mode = Some(runtime_mode);
            runtime_settings.block_all_uploads = runtime_mode != NetworkMode::Standard;
            let restart_required = self.settings.effective_network_mode() != runtime_mode
                || self.settings.dht_enabled != runtime_settings.dht_enabled
                || self.settings.incoming_connections_enabled
                    != runtime_settings.incoming_connections_enabled
                || self.settings.listen_port_start != runtime_settings.listen_port_start
                || self.settings.listen_port_end != runtime_settings.listen_port_end
                || self.settings.upnp_enabled != runtime_settings.upnp_enabled
                || self.settings.default_download_dir != runtime_settings.default_download_dir;

            let apply_result = if restart_required {
                self.restart_engine(ctx, runtime_settings)
            } else {
                self.settings = runtime_settings;
                self.send(EngineCommand::SetGlobalLimits {
                    down_kib: self.settings.download_limit_kib,
                    up_kib: self.settings.upload_limit_kib,
                });
                self.send(EngineCommand::SetMaxActive(
                    self.settings.max_active_downloads,
                ));
                Ok(())
            };
            match apply_result {
                Ok(()) => {
                    self.settings_editor = persisted;
                    if watch_changed {
                        self.watch_seen.clear();
                        self.watch_pending.clear();
                        self.watch_retry_after.clear();
                    }
                    self.notice = "Settings saved and applied".to_owned();
                    self.error = None;
                    self.show_settings = false;
                }
                Err(error) => self.error = Some(error),
            }
        } else if exit {
            self.quitting = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl eframe::App for OrbitApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        #[cfg(target_os = "macos")]
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::Q)) {
            self.quitting = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        self.drain_events(&ctx);
        self.handle_tray(&ctx);
        self.update_tray_status();
        self.handle_dropped_files(&ctx);
        self.scan_watch_folder();

        if ctx.input(|input| input.viewport().close_requested())
            && self.settings.close_to_tray
            && self.tray.is_some()
            && !self.quitting
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            let active = self
                .torrents
                .iter()
                .filter(|torrent| is_active_transfer(torrent))
                .count();
            self.notice = format!(
                "Orbit is running in the system tray · {active} active transfer{}",
                if active == 1 { "" } else { "s" }
            );
        }

        self.sidebar(ui);
        self.top_bar(ui);
        egui::CentralPanel::default()
            .frame(Frame::new().fill(KRYPTON_BG).inner_margin(Margin::same(24)))
            .show(ui, |ui| {
                self.add_bar(ui);
                ScrollArea::vertical()
                    .id_salt("main-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.transfer_table(ui);
                        self.details_panel(ui);
                    });
            });

        self.preview_modal(&ctx);
        self.remove_modal(&ctx);
        self.about_modal(&ctx);
        self.startup_setup_modal(&ctx);
        self.settings_modal(&ctx);
        ctx.request_repaint_after(Duration::from_millis(250));
    }
}

fn main() -> eframe::Result {
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/orbit-icon-final.png"))
        .expect("embedded Orbit icon is valid PNG data");
    let startup_sources = collect_startup_sources();
    let (activation_listener, forwarded_to_running_instance) =
        acquire_activation_listener(&startup_sources);
    if forwarded_to_running_instance {
        return Ok(());
    }
    let settings = load_settings().unwrap_or_else(|_| AppSettings {
        block_all_uploads: true,
        ..Default::default()
    });
    let tray_icon = icon.clone();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Orbit — Krypton Torrent Client")
            .with_icon(icon)
            .with_inner_size([1260.0, 820.0])
            .with_min_inner_size([900.0, 620.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Orbit",
        options,
        Box::new(move |cc| {
            Ok(Box::new(OrbitApp::new(
                cc,
                startup_sources,
                activation_listener,
                settings,
                tray_icon,
            )))
        }),
    )
}
