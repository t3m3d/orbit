# Orbit for Linux x86-64

Linux uses the existing native Rust/egui desktop interface and embedded librqbit
engine with Orbit's purple theme. The Mac UI remains KryptScript/Objective-K.
Krypton's current Linux Wayland layer has drawing/input primitives but no ready
text fields, tables, file dialogs, or widget controls; it is not a drop-in port of
the AppKit interface. The native Linux build script is KryptScript.

## Run

Extract `orbit-linux-x86_64.tar.gz`, then:

```sh
cd orbit-linux-x86_64
./orbit
```

Target: x86-64 Linux with glibc 2.36 or newer (Debian 12, Ubuntu 24.04, or newer),
OpenSSL 3, OpenGL, libxkbcommon, and X11 or Wayland. This is not a musl/Alpine
binary. File/folder dialogs use an XDG desktop portal. Install `xdg-utils`,
`xdg-desktop-portal`, and your desktop's portal backend (GNOME, KDE, or GTK).
No compiler, Rust toolchain, or Krypton runtime is required by end users.

Linux supports the existing transfer workflow, metadata preview/file selection,
pause/resume, persistence, queue controls, watched folders, speed limits, and
sharing/completion policies. Fresh settings remain Offline until the user makes
a network choice. Close-to-tray is disabled on Linux; closing quits Orbit.
System public/private/metered network classification is unavailable and resolves
to the conservative unclassified mapping.

Settings/resume data live at `$XDG_DATA_HOME/Orbit` (normally
`~/.local/share/Orbit`). Downloads default to the desktop's Downloads/Orbit
folder. `ORBIT_DATA_DIR` and `ORBIT_DOWNLOAD_DIR` override these for testing.

## Desktop launcher and default torrent app

```sh
./orbit --install-desktop
./orbit --make-default
```

Installation is per user: the executable is copied to `~/.local/lib/orbit/orbit`;
the desktop launcher/icon use your XDG data directory. No sudo is needed.
`--install-desktop` creates a launcher without changing defaults.
`--make-default` installs the launcher, requests both `.torrent` and `magnet:`
associations through `xdg-mime`, then checks both results. The same action is in
Settings → Desktop & automation. Browsers may still ask before opening an app.
Protocol/file activation forwards to an already running Orbit instance.

## Build on Linux

Debian/Ubuntu build packages:

```sh
sudo apt-get install build-essential pkg-config libssl-dev libx11-dev libxi-dev \
  libxrandr-dev libxcursor-dev libxkbcommon-dev libwayland-dev \
  libgl1-mesa-dev libegl1-mesa-dev
```

Use a Rust toolchain compatible with the locked dependencies and a complete
Linux x86-64 Krypton checkout. From the Orbit repository:

```sh
export KRYPTON_ROOT=/path/to/krypton
"$KRYPTON_ROOT/bootstrap/kcc_driver_linux_x86_64" --x64 linux/build.ks -o /tmp/orbit-build
/tmp/orbit-build
```

The explicit `--x64` keeps the helper on the requested architecture, including
when building in an emulated container on an ARM Mac.

Output: `native/target/linux/orbit-linux-x86_64.tar.gz` and `SHA256SUMS`.
To build only the executable without Krypton:

```sh
cargo build --locked --manifest-path native/Cargo.toml --release --bin orbit-native
```

## Build and verify from a Mac or Linux with Docker

```sh
docker buildx build --platform linux/amd64 -f linux/Dockerfile \
  --output type=local,dest=native/target/linux .
```

The image builds the AMD64 executable, runs strict Clippy and all Rust tests,
renders the real UI using Xvfb/software OpenGL, and checks the desktop launcher
and both default associations in a disposable container home. Output includes
the archive, checksum, and extracted application folder. It does not change the
host's default apps or install Orbit on the host.

Manual verification on a real GNOME/KDE session remains necessary for Wayland,
portal dialogs, browser consent, and desktop-specific default-app behavior.
An Xvfb smoke test validates X11 rendering, not every desktop environment.
