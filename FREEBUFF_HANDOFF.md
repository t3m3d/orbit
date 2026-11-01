# Orbit handoff — 2026-11-01

Continue Orbit in `/Users/t3m3d/Documents/GitHub/orbit` on Apple Silicon macOS.
The user's language stack is **Krypton / KryptScript / Objective-K**. Use the
existing local Krypton checkout at `/Users/t3m3d/Documents/GitHub/krypton` and
the local `../brain` app as references. Do not confuse Krypton with the .NET
WinForms toolkit, and do not replace the Mac UI with web, Swift, or Objective-C.

## Architecture and completed cleanup

- `macos/orbit.ks`: native AppKit frontend written in KryptScript/Objective-K.
- `macos/build.ks`: KryptScript app-bundle builder.
- `native/src/bin/orbit-engine.rs`: bundled private Rust/librqbit child process.
- Shared Rust engine/settings/storage remain because replacing the BitTorrent
  implementation would be a separate project. The user accepted keeping this
  backend while removing other languages where practical.
- Mac builds use `--no-default-features`. The optional `rust-ui` feature holds
  eframe/egui, rfd, and tray-icon; none appear in the Mac engine dependency tree.
  Engine wakeups now use a UI-independent callback.
- Windows retains its existing Rust frontend and PowerShell registration helper.
  Keep OS resource files, plist/MSIX manifests, and artwork.
- Removed the unused React/TypeScript/CSS website, Node manifests/lockfile,
  Cloudflare/Drizzle scaffolding, web test, and obsolete public assets.
- Replaced the Python bridge tests with `native/tests/bridge.rs`. No Node or
  Python installation is required to build or test the remaining application.

## User-visible work

- Purple body and matching title bar, purple app dialogs, lighter purple text
  fields with lavender text. Preserve these choices.
- Main window has **Add Link…** (⌘L); the paste field lives in its dialog.
- **Open Torrent…** (⌘O), magnet URL events, and `.torrent` document events.
- Links arriving Offline queue until the user explicitly enables networking.
- **Use Orbit for Browser Links…** requests macOS default associations after
  confirmation. It now detects a build-folder or temporary location and says so
  before changing anything, offering **Reveal Orbit.app** instead; after the
  request it re-reads LaunchServices and reports the real result, including
  another copy of Orbit that owns the association. HTTP torrent links still
  download a descriptor in the browser; opening that file invokes Orbit. No
  default-handler change was made during development tests.
- The window opens centered and the fixed design survives resizing: header
  pinned to the top, footer to the bottom, transfers table takes the extra
  height. Minimum size is 1080x660. Double-clicking a transfer reveals it in
  Finder.
- Primary transfer workflow, previews/file selection, destinations, pause/resume,
  completion choices, safe removal, Finder actions, and speed/network controls.
- Offline-first behavior, engine-level piece-upload blocking in Download Only,
  explicit Standard sharing confirmation, optional remembered mode.
- Closing the window or ⌘Q stops the child. An inherited stdin pipe carries JSON
  commands; atomic snapshots live in an owner-only temporary directory. No local
  HTTP service. An exclusive engine lock protects the resume database.

## Build

Run from the Orbit root:

```sh
export KRYPTON_ROOT=/Users/t3m3d/Documents/GitHub/krypton
"$KRYPTON_ROOT/kcc-native" macos/build.ks -o native/target/build-macos
native/target/build-macos
open native/target/Orbit.app
```

`--debug` on the builder selects a debug engine. Local `kcc-native` accepts the
source directly; do not prepend `--native` to this invocation. Builds stay in
`native/target`. Do not install, publish, notarize, or replace installed copies
without the user's request. Krypton supplies its normal local ad-hoc executable
signature; this is not a notarized release.

## Verification

```sh
cargo fmt --manifest-path native/Cargo.toml --all -- --check
cargo clippy --manifest-path native/Cargo.toml --all-targets -- -D warnings
cargo clippy --manifest-path native/Cargo.toml --no-default-features --all-targets -- -D warnings
cargo test --manifest-path native/Cargo.toml --all-targets
cargo test --manifest-path native/Cargo.toml --no-default-features --test bridge
ORBIT_DATA_DIR="$(mktemp -d)/data" native/target/Orbit.app/Contents/MacOS/Orbit --smoke
```

Latest checks passed: format, both strict Clippy configurations, 27 unit tests
(23 original frontend tests plus 4 engine-binary tests), 3 Rust bridge integration
tests, the no-default-features bridge run, release app build, and packaged
Objective-K UI smoke test. The integration suite downloads and verifies generated
bytes using only a localhost tracker/peer, checks zero uploads, pause/resume,
restart recovery, keep-files removal, Offline validation, persistence, duplicate
engine rejection, and EOF shutdown.

## Latest session update (2026-11-01)

Browser handoff and Mac usability were the focus. Changes in `macos/orbit.ks`:

- `isInstalledPath()` treats `native/target`, `/tmp`, `/private/tmp`, and
  `/var/folders` as volatile. The handoff dialog explains that macOS records the
  exact bundle path and offers **Reveal Orbit.app** / **Use This Build Anyway**;
  it never moves or installs the app itself.
- `handlerReport()` and `handlesBoth()` read `URLForApplicationToOpenURL:` for a
  sample magnet and for an empty `.torrent` probe in the owner-only temp dir.
  The command reports "this copy of Orbit", another app with its path, or no
  handler, and the button title becomes **Browser Links: Orbit** only when this
  copy owns both. A request is re-checked over roughly two seconds; failure is
  reported honestly instead of assumed.
- `layout()` pins the header to the top and the footer to the bottom and sizes
  the transfers table to the remaining height; the window is centered and its
  minimum is 1080x660. The transfers table also opens Finder on double-click.

The smoke test now also checks bundle-path detection, the read-only handler
report and label, and that a 1200x900 resize grows the table. It still never
touches system default handlers. Browser confirmation dialogs on an installed
copy remain a manual check.

Two implementation lessons: keep `funcptr` wiring directly in
`cocoaOnClickKeyed(...)` calls; passing raw callback pointers through the old
button helper caused an ARM alignment crash. On macOS/BSD, accepted test sockets
must explicitly clear inherited `O_NONBLOCK` before blocking reads.

## Remaining work and guardrails

Mac UI is not yet at Windows feature parity: watched folders, tray/background
operation, search/tags, Turtle Mode, advanced incoming/UPnP configuration, and
editing policies/file selection on existing transfers are not exposed. Resizing
is now covered by the layout pass and the smoke test. Still manual: real browser
handoff on the final installed path (including the macOS/browser confirmation
dialogs), denied-folder permissions, disk-full recovery, and real-world torrent
behavior before release. Do not claim those checks passed. Windows was not built
on this Mac.

Data: `~/Library/Application Support/Orbit`; default downloads:
`~/Downloads/Orbit`. Use isolated `ORBIT_DATA_DIR` for tests. Preserve Offline
defaults, consent, source-descriptor identity checks, and explicit delete-data
confirmation. No accounts, telemetry, hosted backend, or arbitrary public torrent
testing. Read `macos/README.md` and inspect `git status` first; current changes
include uncommitted work, and the user committed an earlier checkpoint during
the session. Do not revert or overwrite unrelated changes.
