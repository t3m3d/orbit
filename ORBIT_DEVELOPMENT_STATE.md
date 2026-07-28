# Orbit Development State

**Saved:** July 26, 2026  
**Workspace:** `C:\Users\brian\Documents\GitHub\current`  
**Native application:** `C:\Users\brian\Documents\GitHub\current\native`

## Ground rules

- Orbit is a local native application, not a hosted website or cloud service.
- Keep source code and development builds on this computer.
- Do not publish, upload, submit, install, sign, or replace the installed Orbit copy unless the user explicitly requests it.
- The local test executable is `native\Orbit.exe`.
- Use torrents only for content the user is authorized to download and share.

## Current architecture

- Rust 2024 desktop application
- `eframe` / `egui` Krypton-themed native interface
- Embedded `librqbit 8.1.1` BitTorrent engine
- Local JSON settings, transfer policies, and fast-resume data
- No account, telemetry, advertising, cloud synchronization, or required backend
- Windows tray integration, single-instance activation, `.torrent` handling, and `magnet:` handling

## Product identity

- Product: `Orbit Torrent Manager`
- Package name: `t3m3d.OrbitTorrentManager`
- Publisher: `CN=97613709-C254-4F66-AB6B-1EE4BA3D003F`
- Publisher display name: `t3m3d`
- Real manifest: `native\store\Package.appxmanifest`
- Only `runFullTrust` is declared; do not add unnecessary network capabilities.

## Recently completed

### First-run Network Setup

- Krypton-themed first-run setup keeps Orbit offline until confirmed.
- User selects a default Orbit network mode:
  - Offline
  - Download Only
  - Standard
- User selects when Orbit asks:
  - Ask every startup
  - Remember the choice until changed
  - Follow the current Windows network
- System-aware defaults:
  - Private/domain: Standard
  - Public: Download Only
  - Metered/cellular: Download Only
  - Unclassified: Offline
  - Disconnected: always Offline
- All system mappings except disconnected are editable.
- Windows network classification is detected locally and is not sent anywhere.
- Saving privacy/network changes safely restarts the embedded engine without restarting the full application.

### First-run file defaults

- User chooses the default download directory.
- User chooses whether imported local `.torrent` descriptors are retained or deleted after completion.
- Source deletion keeps downloaded content and deletes only the descriptor.
- Deletion remains protected by the existing SHA-256 identity check.

### Network and privacy controls

- Fresh installations are fully Offline before consent.
- Offline mode creates no torrent session and performs no DHT, tracker, peer, restore, download, or upload activity.
- Download Only starts the engine with BitTorrent piece uploads blocked at engine level.
- Standard mode permits sharing according to transfer policy.
- DHT has a user control.
- PEX is accurately disclosed as engine-managed with no separate switch.
- Incoming listening and UPnP are separate opt-in controls.
- Close-to-tray/background transfers are opt-in and expose live state and speeds in the tray.

### Krypton visual system

- Deep violet backgrounds and panels
- Electric-purple active accents
- Radioactive-green status and completion accents
- Pink failure/danger accents
- Custom transfer status bars:
  - Purple active fill
  - Green completion/seeding fill
  - Green moving-edge indicator
  - Violet paused state
  - Pink error state
  - Dark recessed track
  - Integrated percentage and uppercase engine-state text
- Matching illuminated core-status strip in the sidebar
- Scrollable settings and accessible labels for major text inputs

## Existing transfer features

- Magnet links, local `.torrent` files, direct torrent URLs, drag-and-drop, and watched-folder intake
- Metadata preview before download
- File selection, destination selection, and start-paused
- Fast resume and session persistence
- Pause/resume, queue controls, filters, sorting, tags, speed limits, and Turtle Mode
- Transfer details, peer health, diagnostics, info hash, copy magnet, and open-folder actions
- Safe removal with keep-data or explicit delete-data choices
- Completion policies: seed, stop, or remove from Orbit while retaining downloaded files
- Seed ratio and seed-time limits
- Destination validation and disk-space preflight
- Malformed source and unsupported URL-scheme rejection

## Verification state

Last successful commands:

```powershell
cd C:\Users\brian\Documents\GitHub\current\native
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release --bin orbit-native
```

- Automated tests: **22 passed**
- Strict Clippy: **passed**
- Release build: **passed**
- Workspace executable SHA-256:
  `3E5B030367C7478F7E3BC38CD7C369282576B337CA44521C9572C9BBB3B21F68`
- At the time this state was saved, no active `Orbit.exe` process was detected.

## Important source files

- `native\src\main.rs` — application lifecycle and interface
- `native\src\engine.rs` — embedded torrent engine and commands
- `native\src\settings.rs` — persisted settings and network plans
- `native\src\network_profile.rs` — local Windows network detection
- `native\src\tray.rs` — tray controls and live state
- `native\src\settings_tests.rs` — privacy and settings regression tests
- `native\src\ui_tests.rs` — source and destination validation tests
- `native\PRIVACY.md` — draft privacy statement
- `native\STORE_READINESS.md` — release gate checklist
- `README.md` — user/developer overview

## Work still required before release

- Run a real end-to-end test with a known legal torrent.
- Verify Standard and Download Only behavior, including observed upload blocking.
- Test pause/resume, restart recovery, file selection, completion policies, tray behavior, and source deletion.
- Test disk-full, denied-permission, lost-network, and recovery scenarios.
- Complete transitive dependency security and license audits.
- Complete GPLv3 corresponding-source distribution planning.
- Produce final Store assets and a signed MSIX.
- Verify packaged file/protocol activation and single-instance forwarding.
- Run WACK and clean Windows 10/11 install, update, and uninstall tests.
- Publish the final privacy-policy URL and add the final support contact only when preparing submission.

## Pickup instructions

When development resumes:

1. Read this file and `native\STORE_READINESS.md`.
2. Inspect current source and working-tree state before editing; preserve unrelated user changes.
3. Ask for or follow the user's next requested feature.
4. Keep the implementation in the native Rust application and match the Krypton theme.
5. Run formatting, strict Clippy, all tests, and a release build.
6. Replace only `native\Orbit.exe` for local testing.
7. Do not touch the installed Programs-folder copy or publish anything without explicit approval.
