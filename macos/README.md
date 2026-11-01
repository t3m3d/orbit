# Orbit for macOS

The Mac frontend is written in KryptScript and Objective-K, using the local
Krypton checkout's Cocoa bindings. It creates AppKit windows, controls, menus,
dialogs, and document/protocol delegates directly. There is no web frontend,
Swift, or Objective-C application source in this port.

The existing Rust/librqbit transfer engine runs as a bundled child process.
Mac builds disable the optional `rust-ui` feature, excluding eframe/egui, Rust
file dialogs, and tray widgets from the Mac engine dependency graph.
Commands use inherited stdin, not an HTTP server. State snapshots are written
atomically inside a unique, owner-only temporary directory. Closing Orbit closes
the pipe and stops the engine. An exclusive data-directory lock prevents two
engine children from corrupting the same resume database.

## Build and run

Requires Apple Silicon macOS, Rust, Xcode command-line tools, and the sibling
`../krypton` checkout. Set `KRYPTON_ROOT` to use a different Krypton checkout.
Run from the Orbit repository:

```sh
export KRYPTON_ROOT="$(cd ../krypton && pwd)"
"$KRYPTON_ROOT/kcc-native" macos/build.ks -o native/target/build-macos
native/target/build-macos
open native/target/Orbit.app
```

Use `native/target/build-macos --debug` for a debug engine. The frontend always
compiles to native arm64 Mach-O. Builds stay in this checkout; the builder does
not install the application, change default handlers, or publish anything.
Krypton's compiler supplies its normal local ad-hoc executable signature. This
is not a Developer ID signed or notarized distribution.

## Mac behavior

- Purple AppKit interface and title bar, matching dialogs and text inputs.
- Fixed 1120x800 design that stays usable when resized: the header stays at the
  top, the footer stays at the bottom, and the transfers table takes the extra
  height. The window opens centered and cannot be shrunk below 1080x660.
- **Add Link…** (⌘L) opens the paste field; it is hidden from the main window.
- **Open Torrent…** (⌘O), `.torrent` document events, and `magnet:` URL events.
- Browser links queue while Offline. Explicit network selection is still required.
- Metadata preview, individual file toggles, destination selection, start-paused,
  completion policies, transfer speeds/progress, pause/resume, safe removal,
  Finder actions, double-click a transfer to reveal it, and copy-magnet.
- Offline, Download Only, and Standard; DHT control, global speed limits, and an
  optional remembered mode. Standard requires an explicit sharing confirmation.
- Settings and resume state: `~/Library/Application Support/Orbit`.
- Default downloads: `~/Downloads/Orbit`.
- Close window or ⌘Q quits and stops transfers; this frontend does not hide in
  the menu bar or continue transfers after closing.
- Free-space checks use the destination volume, including a new folder's nearest
  existing ancestor.

To opt into browser handoff, click **Use Orbit for Browser Links…**, then accept
any macOS and browser confirmations. Do this after placing the app at the path
you intend to keep. The bundle declares both `magnet:` and `.torrent` support.
An HTTP(S) link to a torrent normally downloads its descriptor in the browser;
open that downloaded file with Orbit. Orbit does not intercept arbitrary browser
HTTP requests. Default-handler changes are asynchronous and may require macOS
confirmation; the UI reports a request, not an assumed successful registration.

macOS records the app's exact path as the handler. Orbit therefore detects when
it is running from a build folder (for example `native/target`) or a temporary
directory and says so before changing anything, offering **Reveal Orbit.app**
instead of registering a copy that the next build replaces. After a request it
re-reads LaunchServices and reports the result: the button title becomes
**Browser Links: Orbit** when this copy handles both magnet links and `.torrent`
files, and its tooltip otherwise names the app or copy that currently does. The
app never moves or installs itself.

The Mac frontend currently exposes the primary transfer workflow. Windows UI
features such as tray operation, watched-folder automation, search/tags, Turtle
Mode, advanced incoming/UPnP configuration, and editing policies/file selection
on existing transfers remain in the Rust frontend. The shared engine retains
its completion and persistence behavior. This is not yet full UI parity.

## Verify

```sh
cargo fmt --manifest-path native/Cargo.toml --all -- --check
cargo clippy --manifest-path native/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path native/Cargo.toml --all-targets
cargo test --manifest-path native/Cargo.toml --no-default-features --test bridge
```

The Rust integration suite (`native/tests/bridge.rs`) uses a generated torrent, a localhost tracker, and a tiny
localhost BitTorrent peer. It verifies real downloaded bytes, pause/resume,
Download Only reporting zero uploaded bytes, restart recovery, keep-files
removal, Offline validation, settings persistence, duplicate-engine exclusion,
and shutdown when the input pipe closes. It uses no public torrent or tracker.

To test the packaged Objective-K callbacks and bridge in isolation:

```sh
ORBIT_DATA_DIR="$(mktemp -d)/data" native/target/Orbit.app/Contents/MacOS/Orbit --smoke
```

The UI smoke test requires an explicit test data directory. It exercises a native
button callback, a magnet Apple event, the hidden paste field, Offline
settings application, bundle-path detection, the read-only LaunchServices
handler report, and a resize layout pass before exiting. It never changes the
system default handlers. Browser-specific external-app prompts and system
default-handler selection still need a manual check on the final installed path.
