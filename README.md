# Orbit

Orbit is a local-first native BitTorrent client with the Krypton/Cortex visual theme. The macOS frontend is written in **KryptScript / Objective-K** in [`macos/`](macos/), backed by the existing Rust `librqbit` engine in [`native/`](native/). The Rust frontend remains available for Windows. There is no server account, hosted backend, or required website.

For Apple Silicon builds, supported Mac workflows, and verification, see [`macos/README.md`](macos/README.md). The feature list below describes the original Rust frontend; the Mac document records the current differences.

## Source layout

- `macos/` — KryptScript / Objective-K frontend and app-bundle builder.
- `native/src/engine.rs`, `native/src/engine/`, `native/src/settings.rs`, and `native/src/storage.rs` — shared Rust torrent core, persistence, and storage checks.
- `native/src/bin/orbit-engine.rs` — private process bridge used by the Mac app.
- `native/src/main.rs`, `native/src/tray.rs`, and `native/src/network_profile.rs` — original Rust desktop frontend and Windows integration.
- `native/tests/bridge.rs` — Rust integration tests with a localhost tracker and peer.

The retired React/TypeScript website, Cloudflare/database scaffolding, Node toolchain, and Python test harness have been removed. No Node or Python installation is needed to build or test Orbit. macOS users run the bundled native app without installing Rust or Krypton. Windows PowerShell registration and OS resource/package manifests remain because they provide platform integration.

## Current native features

- magnet links, local `.torrent` files, direct torrent URLs, drag-and-drop, and a watched folder
- metadata preview before download, individual file selection, destination selection, and start-paused
- session persistence and fast resume across restarts
- pause/resume, pause/resume all, search, filters, sorting, tags, queue limits, and Turtle Mode
- persistent default download directory and per-transfer destinations
- Krypton-themed first-run setup for network behavior, download location, and source `.torrent` retention
- startup choices to ask every launch, remember a mode, or map Windows network profiles to Orbit modes
- global and per-add download/upload limits
- destination validation and free-space preflight before a transfer starts
- transfer details, file progress, ratio, ETA, expanded peer health, copyable diagnostics, info hash, copy-magnet, and open-folder actions
- safe removal choices: keep downloaded data or explicitly delete it
- completion policies: keep for seeding, stop when complete, or remove from Orbit while keeping files
- automatic seeding stop targets by ratio or elapsed time
- optional deletion of the original local `.torrent` after completion, protected by a SHA-256 identity check
- explicit Offline, Download Only, and Standard network modes, plus a DHT privacy control
- opt-in incoming peer connections, validated listen-port range, and separately opt-in UPnP forwarding
- explicit PEX disclosure; the embedded engine manages PEX and does not currently expose a separate switch
- reliable watched-folder imports with pending-state deduplication and failed-preview backoff
- opt-in close-to-tray operation with live transfer state and speed in the tray menu and tooltip
- Windows default-app registration, single-instance forwarding, and Krypton icon
- in-app version, privacy state, local data locations, system diagnostics, and license information
- device-local JSON settings, policy state, and torrent fast-resume data

### Sharing controls

`Offline` is the strongest local control: Orbit does not create a torrent-engine session, restore transfers, or contact DHT, trackers, or peers. Fresh installations stay Offline until the user saves a network choice.

`Download Only` starts the engine but blocks BitTorrent piece uploads at the engine level. `Standard` allows piece sharing according to each transfer's completion policy. Existing settings migrate to the equivalent mode selected in the previous build.

DHT and PEX can expose the user's IP address to other participants and do not provide anonymity. DHT has a user control; PEX is managed by the embedded engine and has no separate switch. Incoming listening, UPnP, and close-to-tray operation are off by default.

The per-transfer `Stop when complete` policy prevents post-download seeding, but normal BitTorrent piece sharing can still occur while that transfer is downloading. Orbit does not disguise a 0-byte rate limit as an upload block: `0` means unlimited.

Store packaging groundwork and remaining release gates are documented in [`native/STORE_READINESS.md`](native/STORE_READINESS.md). The draft local privacy statement is [`native/PRIVACY.md`](native/PRIVACY.md).

## Build locally

Windows requirements: a current Rust toolchain and the Windows 10/11 SDK. For macOS, follow [`macos/README.md`](macos/README.md).

```powershell
cd native
cargo test --all-targets
cargo build --release --bin orbit-native
```

The optimized executable is created at:

```text
native\target\release\orbit-native.exe
```

The checked-in local convenience copy is `native\Orbit.exe`. The registration helper is:

```powershell
powershell -ExecutionPolicy Bypass -File .\register-orbit.ps1 -ExecutablePath .\Orbit.exe
```

Windows may still require the user to select Orbit in **Settings → Apps → Default apps** because Windows protects the existing per-user `.torrent` association.

## Verification

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo run --bin metadata-smoke -- C:\path\to\legal-content.torrent
```

The smoke test reads metadata in preview-only mode with DHT and uploading disabled; it does not start the download.

Use torrents only for content you are authorized to download and share.
