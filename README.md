# Orbit

Orbit is the working title for a universal torrent-management app: one responsive interface for desktop, tablet, mobile, and the web.

## What exists now

- responsive transfer dashboard with desktop and mobile navigation
- searchable and filterable torrent library
- magnet-link and `.torrent` file intake UI
- pause, resume, select, and remove interactions
- device-local queue persistence
- live rqbit node pairing, polling, magnet/file intake, and transfer controls
- installable web-app manifest and offline shell
- social sharing metadata and artwork

Orbit runs in demo mode by default. When paired with an rqbit server, the same interface reads the real queue and can add, pause, resume, and forget transfers while the node downloads and stores the payloads.

## Cross-platform architecture

A browser cannot safely reach the full BitTorrent network because standard peers use TCP/uTP while browser networking is restricted. The production architecture should keep this interface shared and move torrent traffic into a small local engine:

1. **Shared UI** — this Next/Vinext app runs as a PWA and inside native shells.
2. **Torrent engine** — the first working adapter targets rqbit 8.x over its HTTP API; a bundled librqbit core is the native packaging path.
3. **Native packaging** — Tauri packages the UI and engine for Windows, macOS, Linux, iOS, and Android.
4. **Web access** — the hosted PWA connects over an encrypted, authenticated channel to the user's paired home node.
5. **Local-first state** — transfers and files stay on the user's device unless they explicitly pair another device.

This avoids a limited WebRTC-only torrent client and gives every platform the same queue and controls.

## Development

Requirements: Node.js 22.13+ and pnpm 11.

```bash
pnpm install
pnpm dev
pnpm build
pnpm test
```

The working title is intentionally isolated to `app/page.tsx`, `app/layout.tsx`, `package.json`, `public/manifest.webmanifest`, and this README so it can be changed cleanly later.

Use torrents only for content you are authorized to download and share.