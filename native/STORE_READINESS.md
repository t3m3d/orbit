# Orbit Microsoft Store readiness

This is an engineering checklist, not a claim of Microsoft approval.

## Implemented in the application

- Local native Rust application with embedded `librqbit` core
- True pre-consent Offline mode with no torrent-engine session or network activity
- Explicit Offline, Download Only, and Standard network modes
- Engine-level upload blocking in Download Only mode
- DHT setting and plain-language network disclosure
- Accurate PEX disclosure without claiming an unavailable PEX switch
- Opt-in incoming peer listener, validated port range, and separately opt-in UPnP
- User-selected completion policy
- Automatic seed ratio and seed time limits
- Persistent default download directory
- Destination validation and disk-space preflight
- Accurate Starting, Online, Offline, and Error core presentation
- Normalized transfer statuses
- Numeric speed and added-time sorting
- Connected, connecting, queued, seen, and failed peer counts
- Copyable per-transfer diagnostics and in-app version/privacy/license details
- Watched-folder pending deduplication and retry backoff
- User review before transfer start
- Legitimate-use reminder before transfer start
- Opt-in close-to-tray behavior with visible live tray state and transfer speeds
- Scrollable settings and explicit accessible labels for major text-entry controls
- Automated privacy-plan, malformed-input, destination, source-scheme, migration, and completion-policy tests
- Krypton-themed first-run setup with an offline-until-confirmed network gate
- Ask-every-start, remembered-choice, and editable Windows-profile network policies
- First-run download destination and hash-protected source `.torrent` retention choice
- Local-only search with no content-index integration

## Required before submission

- Reconfirm the recorded Partner Center identity before packaging
- Produce and validate the MSIX package
- Generate all Store tile and logo assets
- Verify packaged `.torrent` and `magnet:` activation
- Confirm single-instance forwarding from packaged activation
- Add a final publisher/support contact to `PRIVACY.md`
- Publish the final privacy statement at the Partner Center URL
- Generate complete transitive third-party notices
- Complete GPLv3 corresponding-source distribution review
- Audit dependencies and known security advisories
- Test install, update, uninstall, offline mode, permissions, disk full, malformed input, recovery, and upload blocking
- Run the Windows App Certification Kit
- Test clean Windows 10 and Windows 11 installations
- Prepare accurate screenshots, description, accessibility declaration, and IARC age rating

## Deliberately deferred

- PEX switch until the embedded engine exposes a verified control
- uTP claims until verified in the shipping engine
- Remote blocklist subscriptions
- ARM64 packaging
- Cross-platform packages

## Permanently excluded

- Torrent-index or media search
- Curated, trending, recommended, or preloaded content
- Piracy-site integrations
- Website scraping or automatic extraction
- DRM, paywall, geographic, or service circumvention
- Streaming-platform downloaders
- Hidden upload behavior
