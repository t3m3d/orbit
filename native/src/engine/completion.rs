use std::{
    fs,
    io::ErrorKind,
    time::{SystemTime, UNIX_EPOCH},
};

use eframe::egui;
use librqbit::{Api, api::TorrentIdOrHash};

use crate::settings::{SharePolicy, TransferPolicyStore};

use super::{EngineEvent, TorrentRow, emit, is_paused, torrent_source_sha256};

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn seed_limit_reason(
    uploaded: u64,
    downloaded: u64,
    completed_at_unix: Option<u64>,
    ratio_percent: u32,
    time_minutes: u32,
    now_unix: u64,
) -> Option<&'static str> {
    let ratio_reached = ratio_percent > 0
        && downloaded > 0
        && (uploaded as u128).saturating_mul(100)
            >= (downloaded as u128).saturating_mul(ratio_percent as u128);
    if ratio_reached {
        return Some("ratio target reached");
    }

    let time_reached = time_minutes > 0
        && completed_at_unix.is_some_and(|completed_at| {
            now_unix.saturating_sub(completed_at) >= u64::from(time_minutes).saturating_mul(60)
        });
    time_reached.then_some("time target reached")
}

pub(super) async fn apply_completion_policies(
    api: &Api,
    rows: &[TorrentRow],
    policies: &mut TransferPolicyStore,
    event_tx: &std::sync::mpsc::Sender<EngineEvent>,
    repaint: &egui::Context,
) -> bool {
    let mut changed = false;
    let now = unix_now();

    for row in rows.iter().filter(|row| row.finished) {
        let Some(policy) = policies.get_mut(&row.info_hash) else {
            continue;
        };

        if policy.completed_at_unix.is_none() {
            policy.completed_at_unix = Some(now);
            changed = true;
        }

        // The stop policy is continuously enforced, including after restore or
        // a user changing the policy on an already-complete transfer.
        if policy.share_policy == SharePolicy::StopWhenComplete
            && !is_paused(row)
            && let Err(error) = api
                .api_torrent_action_pause(TorrentIdOrHash::Id(row.id))
                .await
        {
            emit(
                event_tx,
                repaint,
                EngineEvent::Error(format!(
                    "could not stop sharing {}: {error:#}; Orbit will retry",
                    row.name
                )),
            );
            continue;
        }

        if policy.share_policy == SharePolicy::Seed
            && let Some(reason) = seed_limit_reason(
                row.uploaded,
                row.downloaded,
                policy.completed_at_unix,
                policy.seed_ratio_percent,
                policy.seed_time_minutes,
                now,
            )
        {
            if !is_paused(row)
                && let Err(error) = api
                    .api_torrent_action_pause(TorrentIdOrHash::Id(row.id))
                    .await
            {
                emit(
                    event_tx,
                    repaint,
                    EngineEvent::Error(format!(
                        "could not stop seeding {} after its {reason}: {error:#}; Orbit will retry",
                        row.name
                    )),
                );
                continue;
            }
            if !policy.seed_limit_reached {
                policy.seed_limit_reached = true;
                changed = true;
                emit(
                    event_tx,
                    repaint,
                    EngineEvent::Notice(format!("{} · seeding stopped: {reason}", row.name)),
                );
            }
        }

        if policy.completion_notified {
            continue;
        }

        if policy.delete_source_when_complete && !policy.source_deleted {
            let Some(path) = policy.source_torrent.as_deref() else {
                emit(
                    event_tx,
                    repaint,
                    EngineEvent::Error(format!(
                        "{} finished, but no local source .torrent was recorded",
                        row.name
                    )),
                );
                continue;
            };
            if !path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("torrent"))
            {
                emit(
                    event_tx,
                    repaint,
                    EngineEvent::Error(format!(
                        "refused to delete non-.torrent source {}",
                        path.display()
                    )),
                );
                continue;
            }
            let Some(expected_fingerprint) = policy.source_torrent_sha256.as_deref() else {
                emit(
                    event_tx,
                    repaint,
                    EngineEvent::Error(format!(
                        "refused to delete {} because its original fingerprint is unavailable",
                        path.display()
                    )),
                );
                continue;
            };
            match torrent_source_sha256(path) {
                Ok(actual) if actual == expected_fingerprint => {}
                Ok(_) => {
                    emit(
                        event_tx,
                        repaint,
                        EngineEvent::Error(format!(
                            "refused to delete {} because the file changed after it was added",
                            path.display()
                        )),
                    );
                    continue;
                }
                Err(error) if !path.exists() => {
                    policy.source_deleted = true;
                    changed = true;
                    emit(
                        event_tx,
                        repaint,
                        EngineEvent::Notice(format!(
                            "Source .torrent for {} was already removed",
                            row.name
                        )),
                    );
                    let _ = error;
                }
                Err(error) => {
                    emit(
                        event_tx,
                        repaint,
                        EngineEvent::Error(format!(
                            "could not verify {} before deletion: {error:#}",
                            path.display()
                        )),
                    );
                    continue;
                }
            }
            if !policy.source_deleted {
                match fs::remove_file(path) {
                    Ok(()) => {
                        policy.source_deleted = true;
                        changed = true;
                    }
                    Err(error) if error.kind() == ErrorKind::NotFound => {
                        policy.source_deleted = true;
                        changed = true;
                    }
                    Err(error) => {
                        emit(
                            event_tx,
                            repaint,
                            EngineEvent::Error(format!(
                                "{} finished, but {} could not be deleted: {error}; Orbit will retry",
                                row.name,
                                path.display()
                            )),
                        );
                        continue;
                    }
                }
            }
        }

        let action = match policy.share_policy {
            SharePolicy::Seed => "kept in Orbit".to_owned(),
            SharePolicy::StopWhenComplete => "sharing stopped".to_owned(),
            SharePolicy::RemoveWhenComplete => {
                if !is_paused(row)
                    && let Err(error) = api
                        .api_torrent_action_pause(TorrentIdOrHash::Id(row.id))
                        .await
                {
                    emit(
                        event_tx,
                        repaint,
                        EngineEvent::Error(format!(
                            "could not pause {} before removal: {error:#}; Orbit will retry",
                            row.name
                        )),
                    );
                    continue;
                }
                if let Err(error) = api
                    .api_torrent_action_forget(TorrentIdOrHash::Id(row.id))
                    .await
                {
                    emit(
                        event_tx,
                        repaint,
                        EngineEvent::Error(format!(
                            "could not remove {} after completion: {error:#}; Orbit will retry",
                            row.name
                        )),
                    );
                    continue;
                }
                "removed from Orbit; downloaded files kept".to_owned()
            }
        };

        policy.completion_notified = true;
        changed = true;
        emit(
            event_tx,
            repaint,
            EngineEvent::Completion {
                name: row.name.clone(),
                action: if policy.source_deleted && policy.delete_source_when_complete {
                    format!("{action} · source .torrent deleted")
                } else {
                    action
                },
            },
        );
    }

    changed
}

#[cfg(test)]
mod tests {
    use super::seed_limit_reason;

    #[test]
    fn ratio_limit_uses_uploaded_over_downloaded() {
        assert_eq!(
            seed_limit_reason(2_000, 1_000, Some(10), 200, 0, 20),
            Some("ratio target reached")
        );
        assert_eq!(seed_limit_reason(1_999, 1_000, Some(10), 200, 0, 20), None);
    }

    #[test]
    fn time_limit_starts_when_completion_is_observed() {
        assert_eq!(
            seed_limit_reason(0, 1_000, Some(100), 0, 10, 700),
            Some("time target reached")
        );
        assert_eq!(seed_limit_reason(0, 1_000, Some(100), 0, 10, 699), None);
    }

    #[test]
    fn zero_seed_limits_mean_unlimited() {
        assert_eq!(
            seed_limit_reason(u64::MAX, 1, Some(0), 0, 0, u64::MAX),
            None
        );
    }
}
