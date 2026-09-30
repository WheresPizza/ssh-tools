# Changelog

## 0.2.0

### Added

- Git account profiles with dedicated aliases, key selection, `IdentitiesOnly` and clone URLs.
- Read-only local repository discovery, remote/profile/key mappings and key-deletion impact previews.
- Effective OpenSSH configuration diagnostics and explicit bounded authentication tests.
- Existing-key import with source revision and duplicate fingerprint checks; missing public-key recovery.
- Agent lifetimes, end-of-workday enrollment and per-use signing confirmation.
- Nested key discovery, permissions review/repair, source-aware Include editing and previous-file recovery.
- Isolated workspaces and production IPC acceptance coverage, including temporary agent and localhost SSH scenarios.

### Changed

- Compact terminal-inspired interface with system monospace typography, graphite/teal colors, consistent controls and outline icons.
- Navigation grouped into Workspace, Security and Tools, with documented screen ownership.
- SSH Config renamed to SSH Hosts; Known Hosts renamed to Trusted Hosts.
- Removed the duplicate Launcher screen. Connections live in SSH Hosts; terminal preferences and shared file recovery live in Settings. Existing Launcher navigation migrates to SSH Hosts.
- New terminal-and-key application icon, monochrome tray mark and matching browser icon.
- Reorganized README and added user, development, release and security documentation.
- Release automation validates matching versions/tags and creates a draft after tests.

### Fixed and hardened

- Tauri IPC argument naming, key algorithm decoding, event cleanup and background blocking work.
- Configuration preservation, stale-edit protection, atomic replacement and hidden backups.
- Symlink/path mutation checks, existing-key overwrite prevention and fingerprint-checked deletion.
- Passphrase-protected key generation without subprocess argument exposure; encrypted agent enrollment through a terminal.
- Terminal command quoting, macOS automation error reporting and selected agent socket propagation.
- Known-host parsing, nonstandard ports, expected-record bulk deletion and network key comparison.
- `ssh-copy-id` no longer skips a new key simply because another identity already authenticates.
- Dock/window reopening, tray configuration, frontend capabilities and Content Security Policy.
- Updated dependencies; remaining target-specific Rust advisories are documented rather than suppressed.

### Release scope

macOS only. Builds are unsigned and not notarized with the current workflow. Scanning reports configured relationships, not proven authentication; recovery does not back up private keys. See [behavior](docs/BEHAVIOR.md) and [release guide](docs/RELEASING.md).
