# Orbit Privacy Statement — Draft

Orbit is a local-first BitTorrent client. The initial release has no Orbit account, advertising, analytics, telemetry, cloud synchronization, or hosted Orbit service.

## Information stored locally

Orbit stores settings, transfer policies, and fast-resume state on the device. Downloaded files are written to the directory selected by the user. This local information is not sent to an Orbit-operated service.

## BitTorrent network activity

BitTorrent is a public peer-to-peer protocol. When a transfer is active, trackers, DHT participants, and peers may receive or observe information including the device's public IP address, port, torrent info hash, peer identifier, and transfer activity.

Offline mode does not create a torrent-engine session and does not contact DHT, trackers, or peers. Download Only mode starts the engine but blocks BitTorrent piece uploads at the engine level. Download Only does not make a transfer anonymous and does not prevent peer-discovery or tracker communication.

DHT can be disabled in Orbit's settings. Disabling DHT may prevent trackerless magnet links from finding peers. Peer Exchange (PEX) is managed by the embedded torrent engine and does not currently have a separate Orbit switch; disabling DHT does not disable PEX.

Incoming peer listening is off by default. Users may opt into a listen-port range and may separately request UPnP router port forwarding.

Continuing transfers after the main window closes is off by default. Users may explicitly enable close-to-tray operation; the tray menu and tooltip then show whether the core is offline, unavailable, starting, or actively transferring.

## Sharing controls

Fresh installations stay Offline until the user records a network choice. Users can remain Offline, use Download Only, or select Standard mode. In Standard mode, users can allow sharing while downloading, allow post-download seeding, stop at completion, or remove a completed transfer from Orbit while retaining downloaded files.

Orbit can ask for a network mode on every launch, remember the user's choice, or follow locally reported Windows network properties. The system-aware option uses user-editable mappings for private/domain, public, metered/cellular, and unclassified connections; a disconnected state always resolves to Offline. This detection is performed on the device and is not sent to an Orbit-operated service.

## Source torrent deletion

Orbit can optionally delete the original local `.torrent` descriptor after completion. Orbit verifies that the file still matches the descriptor originally imported. This option does not delete downloaded content.

## User responsibility

Users must download and share only content they are authorized to use. Orbit does not provide a content catalog, search service, recommendations, or bundled downloads.

## Contact and publication

Before Store submission, replace this section with the publisher's support contact and publish the final privacy statement at the URL entered in Partner Center. The privacy-policy page is not required for Orbit to operate and must not become an application backend.
