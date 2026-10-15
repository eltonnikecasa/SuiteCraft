# SuiteCraft

SuiteCraft is a desktop launcher and manager for open-source applications published on GitHub. It is designed to present independent applications as a cohesive software suite without limiting the catalog to the Craft family.

> **Project status:** early development. The current prototype targets Fedora and Linux on x86-64. Interfaces, manifests, installation behavior, and supported applications may change before the first stable release.

Official repository: [github.com/eltonnikecasa/SuiteCraft](https://github.com/eltonnikecasa/SuiteCraft)

## Goals

SuiteCraft aims to provide:

- A clear application catalog with **Installed**, **Available**, and **Updates** views.
- Muted application icons when an app is not installed.
- Direct launching of installed applications.
- An unobtrusive settings panel for adding GitHub repositories manually.
- GitHub Releases checks when the launcher opens, with clear network and compatibility errors.
- Updates that occur only after explicit user confirmation—never silently.
- Safe application installation and removal without deleting user-created data.
- Desktop menu entries for installed applications, generated without shell interpolation.

The repository currently contains a Tauri 2 application structure with a React and TypeScript interface and a Rust system layer. The prototype includes catalog views, custom repository entries, GitHub release discovery, conservative AppImage selection, and user-directed installation. It is not yet production-ready.

### Linux Alpha limitations

The Alpha installs per user in `~/.local/opt/suitecraft/apps/<app-id>/`. Global `/opt` installation remains disabled until an audited privilege helper is available; the graphical application is never run as root. SuiteCraft can reuse a validated raster icon from an existing desktop entry whose `Exec` resolves to the installed AppImage. Safe extraction of icons embedded inside an AppImage is not implemented yet, and SuiteCraft deliberately does not invoke the untrusted AppImage runtime with `--appimage-extract`; when no trusted icon is available, the UI uses an empty icon slot and the generated desktop entry omits `Icon=`.

## Initial catalog

The initial planned catalog contains:

- DeckCraft
- SoundCraft
- GridCraft
- FilmCraft
- WordCraft
- PhotoCraft
- VectorCraft
- LightCraft
- EffectCraft
- DesignCraft
- PdfCraft
- CADCraft
- SolidCraft

These are catalog candidates, not compatibility guarantees. Each entry is subject to validation before installation can be enabled. SuiteCraft does not assume that a project has a verified repository, a Linux release, an AppImage asset, or compatibility with a particular distribution or architecture.

The catalog is intended to grow beyond these initial applications and beyond the Craft naming family.

## Installation model

The planned system-wide AppImage layout is:

```text
/opt/suitecraft/apps/<app-id>/
```

Writing to `/opt` requires narrowly scoped, safe privilege elevation. SuiteCraft must not run the entire graphical application as root, invoke embedded interactive `sudo`, or grant broad filesystem permissions. Until a reviewed privilege helper is available, users should select an explicitly configured, writable directory in their home directory.

Icons and application manifests may be managed separately from AppImage binaries. Desktop entries must be generated safely in the user's application menu and only for installed applications. Applications installed through SuiteCraft must remain independently runnable; the launcher must not become a runtime dependency.

AppImage is the initial Linux package format. Support for additional Linux formats and Windows is planned, but is not implemented in this early version.

## Security principles

- No silent or automatic installation of updates.
- No execution of arbitrary install scripts supplied by catalog entries or releases.
- Release assets must match explicit platform, package, and architecture rules.
- Downloads must be checked for successful HTTP responses, expected format and size, and checksum when one is provided.
- A working installed version must remain protected until its replacement has downloaded and passed validation.
- External data must never be interpolated into shell commands.

## Development

The project uses Rust, Tauri 2, React, and TypeScript. Install the native Tauri prerequisites for your Fedora version, along with a current Rust toolchain and Node.js 20 or newer.

Install project dependencies and start the desktop development build:

```bash
npm install
npm run tauri dev
```

Available project checks:

```bash
npm test
npm run build
CARGO_BUILD_JOBS=15 cargo test --manifest-path src-tauri/Cargo.toml -j 15
CARGO_BUILD_JOBS=15 cargo check --manifest-path src-tauri/Cargo.toml -j 15
```

The Rust commands may need to download crates on their first run. Distribution packaging is still forthcoming.

## Contributing

SuiteCraft is in an exploratory stage. Issues and focused pull requests are welcome in the [official repository](https://github.com/eltonnikecasa/SuiteCraft). Before proposing a catalog entry, verify the repository ownership, release source, supported platforms, package format, and architecture; do not infer compatibility from an application name or release filename alone.

Please keep changes small, document security-sensitive behavior, and include tests for release selection, version comparison, and installation-state changes where applicable.

## License

A project license has not yet been defined. Until a `LICENSE` file is added, no open-source license should be assumed for this repository's own source code. Third-party applications listed or installed by SuiteCraft retain their respective licenses and ownership.
