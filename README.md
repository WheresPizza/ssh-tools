# SSH GUI

A macOS desktop app for managing SSH configurations, keys, and connections — built with [Tauri 2](https://tauri.app), React, and TypeScript.

<!-- screenshot here -->

## Download

Get the latest `.dmg` from the [Releases page](../../releases/latest), open it, and drag **SSH GUI** to Applications.

> **First launch:** Right-click the app → **Open** to bypass the Gatekeeper unsigned-app warning.

## Features

- **SSH Config** — view, create, edit, and delete hosts in `~/.ssh/config`
- **SSH Keys** — generate (Ed25519, RSA, ECDSA), inspect fingerprints, and delete key pairs
- **Known Hosts** — browse and remove entries from `~/.ssh/known_hosts`
- **Connection History** — track recently used hosts
- **SSH Agent** — add / remove keys from the running SSH agent
- **Permissions audit** — detect and fix unsafe file permissions on SSH files

## Requirements

- **Rust** 1.70 or later
- **Node.js** 18 or later
- **macOS** (Tauri shell permissions use macOS-specific paths)

## Quick Start

```bash
npm install
npm run tauri dev
```

## Build

```bash
# TypeScript / Vite
npm run build

# Rust (no warnings)
cd src-tauri && cargo check

# Distributable DMG (output: src-tauri/target/release/bundle/dmg/)
npm run release
```

## Testing

```bash
npm test
```

## License

MIT — see [LICENSE](LICENSE)
