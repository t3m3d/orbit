use std::{path::PathBuf, time::Duration};

use anyhow::{Context, Result, bail};
use librqbit::{
    AddTorrent, Api, Session,
    api::{ApiTorrentListOpts, TorrentIdOrHash},
};

#[tokio::main]
async fn main() -> Result<()> {
    let source = std::env::args()
        .nth(1)
        .context("pass a legal .torrent URL as the first argument")?;
    let output = std::env::var_os("ORBIT_SMOKE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\tmp\orbit-smoke"));
    tokio::fs::create_dir_all(&output).await?;

    println!("Starting Orbit engine in {}", output.display());
    let session = Session::new(output).await?;
    let api = Api::new(session, None);
    let added = api
        .api_add_torrent(AddTorrent::from_url(&source), None)
        .await?;
    let id = added.id.context("torrent metadata returned no id")?;
    println!("Added legal test torrent as id {id}");

    for _ in 0..90 {
        let list = api.api_torrent_list_ext(ApiTorrentListOpts { with_stats: true });
        if let Some(torrent) = list
            .torrents
            .into_iter()
            .find(|torrent| torrent.id == Some(id))
            && let Some(stats) = torrent.stats
        {
            println!(
                "state={:?} downloaded={} total={}",
                stats.state, stats.progress_bytes, stats.total_bytes
            );
            if let Some(live) = stats.live {
                let received = live.snapshot.downloaded_and_checked_bytes;
                if received > 0 {
                    api.api_torrent_action_pause(TorrentIdOrHash::Id(id))
                        .await?;
                    api.api_torrent_action_forget(TorrentIdOrHash::Id(id))
                        .await?;
                    println!(
                        "PASS: embedded engine downloaded and verified {received} payload bytes"
                    );
                    return Ok(());
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    let _ = api.api_torrent_action_forget(TorrentIdOrHash::Id(id)).await;
    bail!("torrent was added but no payload bytes arrived within 90 seconds")
}
