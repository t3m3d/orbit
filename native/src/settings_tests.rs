use crate::settings::{
    AppSettings, NetworkMode, SharePolicy, StartupNetworkPolicy, SystemNetworkProfile,
    TransferPolicy,
};

#[test]
fn settings_round_trip_preserves_privacy_controls() {
    let settings = AppSettings {
        sharing_consent_recorded: true,
        block_all_uploads: true,
        default_share_policy: SharePolicy::StopWhenComplete,
        auto_delete_torrent: true,
        max_active_downloads: 2,
        ..Default::default()
    };
    let json = serde_json::to_string(&settings).expect("settings serialize");
    let restored: AppSettings = serde_json::from_str(&json).expect("settings deserialize");
    assert_eq!(restored, settings);
}

#[test]
fn legacy_transfer_policy_loads_without_a_source_fingerprint() {
    let json = r#"{
        "share_policy": "stop_when_complete",
        "source_torrent": "C:\\Downloads\\legal.torrent",
        "delete_source_when_complete": true,
        "completion_notified": false,
        "source_deleted": false,
        "tag": "linux"
    }"#;
    let policy: TransferPolicy = serde_json::from_str(json).expect("legacy policy deserialize");
    assert_eq!(policy.share_policy, SharePolicy::StopWhenComplete);
    assert!(policy.source_torrent_sha256.is_none());
}

#[test]
fn fresh_install_defaults_to_consent_first_upload_blocking() {
    let settings = AppSettings::default();
    assert!(!settings.network_setup_completed);
    assert_eq!(
        settings.startup_network_policy,
        StartupNetworkPolicy::AskEveryStart
    );
    assert!(!settings.sharing_consent_recorded);
    assert!(settings.block_all_uploads);
    assert_eq!(settings.effective_network_mode(), NetworkMode::Offline);
    assert_eq!(settings.default_share_policy, SharePolicy::StopWhenComplete);
}

#[test]
fn system_network_defaults_are_conservative() {
    let settings = AppSettings::default();
    assert_eq!(
        settings.mode_for_system_profile(SystemNetworkProfile::Private),
        NetworkMode::Standard
    );
    assert_eq!(
        settings.mode_for_system_profile(SystemNetworkProfile::Public),
        NetworkMode::DownloadOnly
    );
    assert_eq!(
        settings.mode_for_system_profile(SystemNetworkProfile::Metered),
        NetworkMode::DownloadOnly
    );
    assert_eq!(
        settings.mode_for_system_profile(SystemNetworkProfile::Unknown),
        NetworkMode::Offline
    );
    assert_eq!(
        settings.mode_for_system_profile(SystemNetworkProfile::Disconnected),
        NetworkMode::Offline
    );
    assert!(!settings.auto_delete_torrent);
}
#[test]

fn legacy_settings_require_a_new_sharing_choice() {
    let json = r#"{
        "block_all_uploads": false,
        "default_share_policy": "seed",
        "watch_enabled": false
    }"#;
    let settings: AppSettings = serde_json::from_str(json).expect("legacy settings deserialize");
    assert!(!settings.sharing_consent_recorded);
    assert_eq!(settings.effective_network_mode(), NetworkMode::Offline);
}

#[test]
fn legacy_consent_preserves_the_previous_upload_choice() {
    let download_only: AppSettings =
        serde_json::from_str(r#"{"sharing_consent_recorded":true,"block_all_uploads":true}"#)
            .expect("legacy download-only settings");
    assert_eq!(
        download_only.effective_network_mode(),
        NetworkMode::DownloadOnly
    );

    let standard: AppSettings =
        serde_json::from_str(r#"{"sharing_consent_recorded":true,"block_all_uploads":false}"#)
            .expect("legacy standard settings");
    assert_eq!(standard.effective_network_mode(), NetworkMode::Standard);
}

#[test]
fn an_explicit_offline_mode_overrides_legacy_upload_settings() {
    let settings = AppSettings {
        sharing_consent_recorded: true,
        network_mode: Some(NetworkMode::Offline),
        block_all_uploads: false,
        ..Default::default()
    };
    assert_eq!(settings.effective_network_mode(), NetworkMode::Offline);
    assert!(settings.engine_network_plan().disable_upload);
}

#[test]
fn incoming_connections_are_opt_in_and_use_an_inclusive_ui_range() {
    let defaults = AppSettings::default();
    assert!(defaults.incoming_port_range().is_none());

    let settings = AppSettings {
        incoming_connections_enabled: true,
        listen_port_start: 49_152,
        listen_port_end: 49_162,
        ..Default::default()
    };
    assert_eq!(settings.incoming_port_range(), Some(49_152..49_163));
}

#[test]
fn incoming_port_settings_are_clamped_to_safe_values() {
    let settings = AppSettings {
        incoming_connections_enabled: true,
        listen_port_start: 80,
        listen_port_end: u16::MAX,
        ..Default::default()
    };
    assert_eq!(settings.incoming_port_range(), Some(1_024..65_535));
}

#[test]
fn offline_mode_has_no_torrent_session_or_network_services() {
    let settings = AppSettings::default();
    let plan = settings.engine_network_plan();
    assert!(!plan.start_session);
    assert!(plan.disable_upload);
    assert!(plan.disable_dht);
    assert!(plan.incoming_port_range.is_none());
    assert!(!plan.enable_upnp);
}

#[test]
fn download_only_starts_with_the_engine_upload_block() {
    let settings = AppSettings {
        sharing_consent_recorded: true,
        network_mode: Some(NetworkMode::DownloadOnly),
        dht_enabled: true,
        incoming_connections_enabled: true,
        upnp_enabled: true,
        ..Default::default()
    };
    let plan = settings.engine_network_plan();
    assert!(plan.start_session);
    assert!(plan.disable_upload);
    assert!(!plan.disable_dht);
    assert!(plan.incoming_port_range.is_some());
    assert!(plan.enable_upnp);
}

#[test]
fn standard_mode_allows_uploads_but_respects_discovery_settings() {
    let settings = AppSettings {
        sharing_consent_recorded: true,
        network_mode: Some(NetworkMode::Standard),
        dht_enabled: false,
        ..Default::default()
    };
    let plan = settings.engine_network_plan();
    assert!(plan.start_session);
    assert!(!plan.disable_upload);
    assert!(plan.disable_dht);
}

#[test]
fn upnp_cannot_activate_without_an_incoming_listener() {
    let settings = AppSettings {
        sharing_consent_recorded: true,
        network_mode: Some(NetworkMode::Standard),
        incoming_connections_enabled: false,
        upnp_enabled: true,
        ..Default::default()
    };
    let plan = settings.engine_network_plan();
    assert!(plan.incoming_port_range.is_none());
    assert!(!plan.enable_upnp);
}

#[test]
fn background_operation_is_opt_in() {
    assert!(!AppSettings::default().close_to_tray);
}
