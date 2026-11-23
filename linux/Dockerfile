# syntax=docker/dockerfile:1
# The default build is the requested Linux x86-64 target; pass --platform too.
FROM rust:bookworm@sha256:ade39a710ef85f3506cffcfec3e8a24ba090badef1dff2fb798f71d878833fea AS build
RUN test "$(uname -m)" = x86_64
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev libx11-dev libxi-dev libxrandr-dev libxcursor-dev \
    libxkbcommon-dev libxkbcommon-x11-0 libwayland-dev libgl1-mesa-dev libegl1-mesa-dev \
    libgl1-mesa-dri xvfb xauth xdg-utils desktop-file-utils dbus-x11 \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /orbit
COPY native/ native/
COPY LICENSE LICENSE
COPY linux/README.md linux/README.md
RUN --mount=type=cache,id=orbit-linux-x86_64-target,target=/orbit/native/target --mount=type=cache,target=/usr/local/cargo/registry cargo build --locked --manifest-path native/Cargo.toml --release --bin orbit-native --bin orbit-engine
RUN rustup component add clippy
RUN --mount=type=cache,id=orbit-linux-x86_64-target,target=/orbit/native/target --mount=type=cache,target=/usr/local/cargo/registry cargo clippy --locked --manifest-path native/Cargo.toml --all-targets -- -D warnings \
    && cargo test --locked --manifest-path native/Cargo.toml --all-targets
# Exercise real X11 rendering with software OpenGL, without a public torrent.
RUN --mount=type=cache,id=orbit-linux-x86_64-target,target=/orbit/native/target --mount=type=cache,target=/usr/local/cargo/registry mkdir -p /tmp/orbit-ui/data && \
    ORBIT_DATA_DIR=/tmp/orbit-ui/data ORBIT_DOWNLOAD_DIR=/tmp/orbit-ui/downloads \
    ORBIT_ACTIVATION_ADDRESS=127.0.0.1:0 LIBGL_ALWAYS_SOFTWARE=1 WINIT_UNIX_BACKEND=x11 \
    timeout 30s xvfb-run -a native/target/release/orbit-native --smoke
# Check per-user installation and both MIME defaults in disposable container HOME.
RUN --mount=type=cache,id=orbit-linux-x86_64-target,target=/orbit/native/target --mount=type=cache,target=/usr/local/cargo/registry mkdir -p /tmp/orbit-home && HOME=/tmp/orbit-home \
    XDG_DATA_HOME=/tmp/orbit-home/.local/share XDG_CONFIG_HOME=/tmp/orbit-home/.config \
    native/target/release/orbit-native --make-default && \
    desktop-file-validate /tmp/orbit-home/.local/share/applications/org.krypton-lang.orbit.desktop
RUN --mount=type=cache,id=orbit-linux-x86_64-target,target=/orbit/native/target --mount=type=cache,target=/usr/local/cargo/registry mkdir -p /out/orbit-linux-x86_64 && \
    cp native/target/release/orbit-native /out/orbit-linux-x86_64/orbit && \
    cp native/assets/orbit-icon-final.png /out/orbit-linux-x86_64/orbit.png && \
    cp LICENSE native/PRIVACY.md linux/README.md /out/orbit-linux-x86_64/ && \
    cd /out && tar -czf orbit-linux-x86_64.tar.gz orbit-linux-x86_64 && \
    sha256sum orbit-linux-x86_64.tar.gz > SHA256SUMS
FROM scratch AS artifact
COPY --from=build /out/ /
