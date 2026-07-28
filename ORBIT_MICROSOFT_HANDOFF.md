# Orbit Torrent Client — Microsoft Scope and Store Handoff

**Date:** July 26, 2026  
**Status:** Working local Windows application; not yet submitted to or certified by Microsoft.

## Current product

Orbit is a local-first native Windows torrent client.

The current working application is:

- Written in Rust using eframe/egui and librqbit.
- Installed and operated locally on the user's computer.
- Independent of any website, backend, cloud service, account, or hosted API.
- Branded with the Krypton purple and radioactive-green visual theme.
- Not currently written in the Krypton programming language.

A future Krypton-language implementation is a separate engineering track. It will not be presented as the current architecture or block the first Windows Store candidate.

An earlier browser-based prototype is not part of the product and will not be included in the Store package.

## Already implemented

Orbit currently supports:

- User-provided magnet links.
- Local `.torrent` files.
- Direct HTTP/HTTPS links to `.torrent` files.
- Drag-and-drop torrent intake.
- Optional watched-folder intake.
- Add, pause, resume, remove, pause-all, and resume-all.
- Queue management and maximum active downloads.
- Turtle mode and configurable bandwidth limits.
- Per-transfer download directories and bandwidth options.
- Filters for all, downloading, completed, and paused transfers.
- Search by name, tag, or info hash.
- Sorting by name, date, progress, size, and speed.
- Progress, download/upload speed, ETA, ratio, size, file details, and info hash.
- Tags and persisted settings.
- Fast resume and recovery after restarting.
- Tray operation and single-instance handling.
- Local `.torrent` file-association support in the development build.
- Completion notification inside Orbit plus Windows taskbar attention.
- Genuine engine-level upload blocking through Privacy Lock.
- Per-transfer completion behavior:
  - Continue seeding.
  - Stop when complete.
  - Remove the transfer when complete.
- Optional deletion of the original `.torrent` descriptor after completion. The downloaded content is not deleted.
- Safety handling for duplicate torrents, failed policy persistence, interrupted completion actions, and recovered settings.

## Work planned before the first Store submission

The following will be completed or formally resolved before submission:

- Produce a supported Store package, preferably MSIX, with the correct identity, icons, versioning, capabilities, activation, installation, update, and uninstall behavior.
- Move `.torrent` file association and magnet protocol activation into the supported package manifest rather than relying only on unpackaged registry registration.
- Add an editable default download directory.
- Add automatic seeding stop controls based on ratio and elapsed seeding time.
- Display peer and seed counts when librqbit exposes reliable values. Orbit will not advertise unavailable information.
- Improve notification preferences and ensure notification behavior respects Windows settings.
- Clearly disclose seeding, DHT, PEX, tracker communication, and Privacy Lock behavior.
- Add an About/legal section, privacy statement, third-party notices, and license information.
- Resolve GPLv3 distribution obligations, corresponding-source delivery, and dependency-license compatibility before Store distribution.
- Audit dependencies and known security advisories.
- Test clean installation, upgrade, uninstall, offline behavior, invalid torrents, disk-full conditions, permissions, crash recovery, path traversal, and upload-blocking behavior.
- Run the Windows App Certification Kit and test on clean Windows 10 and Windows 11 environments.
- Complete an accurate Partner Center listing, privacy-policy link, accessibility declaration, and IARC age rating.
- Use only legally redistributable material for screenshots or testing. No torrent or magnet content will be bundled with the release.

Local development tests are not being treated as Store certification.

## Accepted but conditional or later work

These features remain accepted possibilities but are not being promised until engine and platform support are verified:

- User-facing DHT and PEX switches.
- A separate maximum-active-seeding limit.
- IP filtering and blocklist support.
- Advanced blocklist subscription management.
- uTP support.
- Rich Windows toast actions.
- Additional packaging targets such as ARM64.
- Cross-platform releases outside the Windows Store project.

DHT, PEX, tracker, and transport capabilities will be documented only after the exact librqbit version has been audited. Unsupported protocol claims will not be placed in the Store listing.

## Permanent product boundaries

Orbit will not include:

- A built-in torrent or media search engine.
- A public content catalog or discovery service.
- Trending, featured, recommended, or curated downloads.
- Integration with piracy-oriented indexes or websites.
- Website scraping, crawling, or automatic torrent extraction.
- Automatic downloading from third-party webpages.
- Preloaded torrents, magnet links, media, or "starter content."
- DRM, paywall, geographic, or service-restriction circumvention.
- YouTube, Netflix, Hulu, streaming-service, or social-media download integrations.
- Claims that users can obtain "anything," "free media," or copyrighted material without authorization.
- Hidden upload behavior or a cosmetic upload-blocking switch that does not stop protocol uploads.

Orbit will accept only user-initiated transfer inputs and will use neutral wording focused on legitimate distribution, backups, public datasets, open-source software, and material the user is authorized to transfer.

The first Store release will also have no accounts, advertising, cloud synchronization, or telemetry.

## Content and UGC position

Orbit does not host, publish, curate, recommend, or operate a public content catalog. It displays torrent metadata supplied by the user or the BitTorrent protocol.

We request Microsoft's confirmation on whether displaying peer-supplied torrent metadata causes any part of Store policy 11.12 concerning user-generated content to apply to a local transfer utility.

## Clarification concerning Store approval

These boundaries are intended to reduce legal and certification risk. They are not a claim that Microsoft has already approved Orbit, nor that every excluded feature is explicitly named as an automatic rejection.

Microsoft's current policies require appropriate rights for accessed content and prohibit products that facilitate or encourage illegal activity. Final acceptance remains Microsoft's decision during certification. See the [Microsoft Store Policies 7.19](https://learn.microsoft.com/en-us/windows/apps/publish/store-policies) and [Microsoft certification guidance](https://learn.microsoft.com/en-us/windows/apps/publish/faq/get-your-app-certified).

## Confirmation requested from Microsoft

Please confirm:

1. Whether explicit DHT and PEX switches are expected, or whether clear disclosure and a functional global upload block are sufficient.
2. Requirements for continued seeding or tray operation after the main window closes.
3. The recommended capabilities and manifest declarations for magnet activation and `.torrent` file association.
4. Whether any torrent metadata shown locally invokes the UGC-policy requirements.
5. The preferred Store packaging model for the existing native Rust application.
6. Any Store-specific restrictions affecting GPLv3 distribution and corresponding-source delivery.
7. Whether Microsoft recommends any additional wording, testing evidence, or certification notes for a neutral-purpose BitTorrent client.

The Windows publishing process and packaging options will follow Microsoft's current [Windows application publishing guidance](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/publish-first-app).
