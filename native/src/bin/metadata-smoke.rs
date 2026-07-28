use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use librqbit::{AddTorrent, AddTorrentOptions, Api, Session, SessionOptions};

#[tokio::main]
async fn main() -> Result<()> {
    let source = std::env::args()
        .nth(1)
        .context("pass a legal local .torrent path or magnet link")?;
    let output = std::env::var_os("ORBIT_SMOKE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\tmp\orbit-metadata-smoke"));
    tokio::fs::create_dir_all(&output).await?;

    let session = Session::new_with_opts(
        output,
        SessionOptions {
            disable_dht: true,
            disable_dht_persistence: true,
            disable_upload: true,
            ..Default::default()
        },
    )
    .await?;
    let api = Api::new(session, None);
    let response = api
        .api_add_torrent(
            AddTorrent::from_cli_argument(&source)?,
            Some(AddTorrentOptions {
                list_only: true,
                ..Default::default()
            }),
        )
        .await?;

    ensure!(
        response.id.is_none(),
        "preview unexpectedly started a transfer"
    );
    let files = response.details.files.unwrap_or_default();
    ensure!(!files.is_empty(), "preview returned no files");
    let total: u64 = files.iter().map(|file| file.length).sum();
    ensure!(total > 0, "preview returned an empty torrent");
    println!(
        "PASS: {} files, {} bytes, info hash {}",
        files.len(),
        total,
        response.details.info_hash
    );
    Ok(())
}
