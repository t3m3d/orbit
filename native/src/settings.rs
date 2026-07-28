use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SharePolicy {
    Seed,
    #[default]
    StopWhenComplete,
    RemoveWhenComplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkMode {
    Offline,
    DownloadOnly,
    Standard,
}

impl NetworkMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Offline => "Offline - no torrent networking",
            Self::DownloadOnly => "Download only - block all uploads",
            Self::Standard => "Standard - sharing policies apply",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupNetworkPolicy {
    #[default]
    AskEveryStart,
    RememberChoice,
    FollowSystem,
}

impl StartupNetworkPolicy {
    pub const fn label(self) -> &'static str {
        match self {
            Self::AskEveryStart => "Ask every time Orbit starts",
            Self::RememberChoice => "Keep this choice until I change it",
            Self::FollowSystem => "Follow the current Windows network",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SystemNetworkProfile {
    Disconnected,
    Metered,
    Public,
    Private,
    #[default]
    Unknown,
}

impl SystemNetworkProfile {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Disconnected => "Disconnected",
            Self::Metered => "Metered or cellular",
            Self::Public => "Public network",
            Self::Private => "Private or domain network",
            Self::Unknown => "Unclassified network",
        }
    }
}

impl SharePolicy {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Seed => "Seed after completion",
            Self::StopWhenComplete => "Stop when complete",
            Self::RemoveWhenComplete => "Remove from Orbit when complete",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub sharing_consent_recorded: bool,
    // None preserves the behavior selected by older Orbit builds.
    pub network_mode: Option<NetworkMode>,
    pub block_all_uploads: bool,
    pub default_share_policy: SharePolicy,
    pub dht_enabled: bool,
    pub incoming_connections_enabled: bool,
    pub listen_port_start: u16,
    pub listen_port_end: u16,
    pub upnp_enabled: bool,
    pub auto_delete_torrent: bool,
    pub start_paused: bool,
    pub network_setup_completed: bool,
    pub startup_network_policy: StartupNetworkPolicy,
    pub system_private_mode: NetworkMode,
    pub system_public_mode: NetworkMode,
    pub system_metered_mode: NetworkMode,
    pub system_unknown_mode: NetworkMode,
    pub default_download_dir: String,
    pub default_seed_ratio_percent: u32,
    pub default_seed_time_minutes: u32,
    pub download_limit_kib: u32,
    pub upload_limit_kib: u32,
    pub turtle_download_kib: u32,
    pub turtle_upload_kib: u32,
    pub max_active_downloads: usize,
    pub watch_folder: String,
    pub watch_enabled: bool,
    pub close_to_tray: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineNetworkPlan {
    pub start_session: bool,
    pub disable_upload: bool,
    pub disable_dht: bool,
    pub incoming_port_range: Option<std::ops::Range<u16>>,
    pub enable_upnp: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        let downloads = default_download_dir();
        let watch_folder = downloads
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(data_dir)
            .join("Orbit Watch")
            .to_string_lossy()
            .into_owned();

        Self {
            sharing_consent_recorded: false,
            network_mode: None,
            block_all_uploads: true,
            default_share_policy: SharePolicy::StopWhenComplete,
            dht_enabled: true,
            incoming_connections_enabled: false,
            listen_port_start: 49_152,
            listen_port_end: 49_162,
            upnp_enabled: false,
            auto_delete_torrent: false,
            start_paused: false,
            default_download_dir: downloads.to_string_lossy().into_owned(),
            default_seed_ratio_percent: 200,
            network_setup_completed: false,
            startup_network_policy: StartupNetworkPolicy::AskEveryStart,
            system_private_mode: NetworkMode::Standard,
            system_public_mode: NetworkMode::DownloadOnly,
            system_metered_mode: NetworkMode::DownloadOnly,
            system_unknown_mode: NetworkMode::Offline,
            default_seed_time_minutes: 0,
            download_limit_kib: 0,
            upload_limit_kib: 0,
            turtle_download_kib: 1_024,
            turtle_upload_kib: 128,
            max_active_downloads: 3,
            watch_folder,
            watch_enabled: false,
            close_to_tray: false,
        }
    }
}

impl AppSettings {
    pub fn configured_network_mode(&self) -> NetworkMode {
        self.network_mode
            .unwrap_or(if !self.sharing_consent_recorded {
                NetworkMode::Offline
            } else if self.block_all_uploads {
                NetworkMode::DownloadOnly
            } else {
                NetworkMode::Standard
            })
    }

    pub fn effective_network_mode(&self) -> NetworkMode {
        if self.sharing_consent_recorded {
            self.configured_network_mode()
        } else {
            NetworkMode::Offline
        }
    }

    pub fn mode_for_system_profile(&self, profile: SystemNetworkProfile) -> NetworkMode {
        match profile {
            SystemNetworkProfile::Disconnected => NetworkMode::Offline,
            SystemNetworkProfile::Metered => self.system_metered_mode,
            SystemNetworkProfile::Public => self.system_public_mode,
            SystemNetworkProfile::Private => self.system_private_mode,
            SystemNetworkProfile::Unknown => self.system_unknown_mode,
        }
    }

    pub fn incoming_port_range(&self) -> Option<std::ops::Range<u16>> {
        if !self.incoming_connections_enabled {
            return None;
        }
        let start = self.listen_port_start.clamp(1_024, 65_534);
        let end = self.listen_port_end.clamp(start, 65_534);
        Some(start..end + 1)
    }

    pub fn engine_network_plan(&self) -> EngineNetworkPlan {
        let mode = self.effective_network_mode();
        let start_session = mode != NetworkMode::Offline;
        let incoming_port_range = start_session.then(|| self.incoming_port_range()).flatten();
        EngineNetworkPlan {
            start_session,
            disable_upload: mode != NetworkMode::Standard,
            disable_dht: !start_session || !self.dht_enabled,
            enable_upnp: incoming_port_range.is_some() && self.upnp_enabled,
            incoming_port_range,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TransferPolicy {
    pub share_policy: SharePolicy,
    pub seed_ratio_percent: u32,
    pub seed_time_minutes: u32,
    pub completed_at_unix: Option<u64>,
    pub seed_limit_reached: bool,
    pub added_at_unix: u64,
    pub source_torrent: Option<PathBuf>,
    pub source_torrent_sha256: Option<String>,
    pub delete_source_when_complete: bool,
    pub completion_notified: bool,
    pub source_deleted: bool,
    pub tag: String,
}

pub type TransferPolicyStore = HashMap<String, TransferPolicy>;

pub fn default_download_dir() -> PathBuf {
    dirs::download_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join("Orbit")
}

pub fn data_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("ORBIT_DATA_DIR") {
        return PathBuf::from(path);
    }
    dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("Orbit")
}

pub fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

pub fn persistence_dir() -> PathBuf {
    data_dir().join("session")
}

pub fn policies_path() -> PathBuf {
    data_dir().join("transfer-policies.json")
}

pub fn load_settings() -> Result<AppSettings> {
    load_json_or_default(&settings_path())
}

pub fn save_settings(settings: &AppSettings) -> Result<()> {
    save_json(&settings_path(), settings)
}

pub fn load_policies() -> Result<TransferPolicyStore> {
    load_json_or_default(&policies_path())
}

pub fn save_policies(policies: &TransferPolicyStore) -> Result<()> {
    save_json(&policies_path(), policies)
}

fn newest_backup(path: &Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    let filename = path.file_name()?.to_str()?;
    let prefix = format!(".{filename}.");
    fs::read_dir(parent)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|candidate| {
            candidate
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&prefix) && name.ends_with(".bak"))
        })
        .max_by_key(|candidate| {
            fs::metadata(candidate)
                .and_then(|meta| meta.modified())
                .ok()
        })
}
fn load_json_or_default<T>(path: &Path) -> Result<T>
where
    T: DeserializeOwned + Default,
{
    fs::create_dir_all(
        path.parent()
            .context("Orbit state path does not have a parent directory")?,
    )
    .with_context(|| {
        format!(
            "failed to create Orbit state directory for {}",
            path.display()
        )
    })?;

    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let Some(backup) = newest_backup(path) else {
                return Ok(T::default());
            };
            fs::read(&backup).with_context(|| format!("failed to recover {}", backup.display()))?
        }
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()));
        }
    };

    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(T::default());
    }

    serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse JSON from {}", path.display()))
}

fn save_json<T>(path: &Path, value: &T) -> Result<()>
where
    T: Serialize + ?Sized,
{
    let parent = path
        .parent()
        .context("Orbit state path does not have a parent directory")?;
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create Orbit state directory {}",
            parent.display()
        )
    })?;

    let bytes = serde_json::to_vec_pretty(value).context("failed to serialize Orbit state")?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("orbit-state");
    let temp_path = parent.join(format!(".{file_name}.{}.{}.tmp", std::process::id(), nonce));

    let write_result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .with_context(|| format!("failed to create {}", temp_path.display()))?;
        file.write_all(&bytes)
            .with_context(|| format!("failed to write {}", temp_path.display()))?;
        file.write_all(b"\n")
            .with_context(|| format!("failed to finish {}", temp_path.display()))?;
        file.sync_all()
            .with_context(|| format!("failed to flush {}", temp_path.display()))?;
        Ok(())
    })();

    if let Err(error) = write_result {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }

    match fs::rename(&temp_path, path) {
        Ok(()) => Ok(()),
        Err(initial_error) if path.exists() => {
            // Windows cannot rename over an existing file. Move the old state aside,
            // install the fully-written replacement, then discard the backup.
            let backup_path =
                parent.join(format!(".{file_name}.{}.{}.bak", std::process::id(), nonce));
            fs::rename(path, &backup_path).with_context(|| {
                format!(
                    "failed to prepare {} for replacement after: {initial_error}",
                    path.display()
                )
            })?;

            if let Err(replace_error) = fs::rename(&temp_path, path) {
                let restore_error = fs::rename(&backup_path, path).err();
                let _ = fs::remove_file(&temp_path);
                return Err(anyhow!(
                    "failed to replace {}: {replace_error}; restore result: {}",
                    path.display(),
                    restore_error
                        .map(|error| error.to_string())
                        .unwrap_or_else(|| "previous file restored".to_owned())
                ));
            }

            let _ = fs::remove_file(backup_path);
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_file(&temp_path);
            Err(error).with_context(|| format!("failed to install {}", path.display()))
        }
    }
}
