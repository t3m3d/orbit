# Orbit MSIX groundwork

This folder contains Store-packaging groundwork, not a signed or submission-ready package.

## Before packaging

1. Reconfirm the identity recorded in `PRODUCT_IDENTITY.md` against Partner Center.
2. Use `Package.appxmanifest` for the real package; `Package.appxmanifest.template` remains an identity-neutral reference.
3. Build the release executable and stage it as `Orbit.exe`.
4. Create all PNG assets referenced by the manifest at their required dimensions.
5. Verify `.torrent` and `magnet:` activation against the packaged executable, including forwarding into an already-running Orbit instance.
6. Validate the manifest and package using the Windows SDK tools.
7. Install the package on clean Windows 10 and Windows 11 virtual machines.
8. Run the Windows App Certification Kit.

The current unpackaged `register-orbit.ps1` helper is for local development only. A Store build must use package-manifest associations.

The manifest intentionally requests `runFullTrust` because Orbit is a native desktop process. Do not add restricted capabilities without a verified implementation need and a matching Partner Center explanation.
