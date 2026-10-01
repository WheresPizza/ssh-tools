# Development

## Prerequisites

- macOS and Xcode Command Line Tools.
- Node.js matching `package.json`: `^22.22.2 || ^24.15.0 || >=26.0.0`.
- Stable Rust via rustup.
- OpenSSH tools: `ssh`, `ssh-keygen`, `ssh-add`, `ssh-agent`, `ssh-keyscan`; `ssh-copy-id` for installation tests.

```sh
npm ci
npm run tauri dev
```

`npm run dev` starts only the Vite frontend; real SSH commands require the Tauri runtime. Rust code lives in `src-tauri/src`, frontend features in `src/features`, shared components/styles in `src/components` and `src/styles`.

## Checks

```sh
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --locked --manifest-path src-tauri/Cargo.toml
cargo check --locked --manifest-path src-tauri/Cargo.toml
npm run release:check
npm audit
cargo audit --file src-tauri/Cargo.lock
```

`cargo audit` requires the separately installed cargo-audit tool. Findings and target-specific limitations are tracked in [Behavior](BEHAVIOR.md); a successful build is not proof of a clean dependency audit.

Rust tests generate temporary keys and exercise production handlers through Tauri IPC, including source revisions, import, profiles, repository relationships and an isolated agent. The 0.3.0 scenario also checks shared annotations across key copies, stale/fingerprint guards, read-only audit byte/permission/timestamp invariants, and incomplete relationship scans. Frontend tests cover payloads, destructive confirmations, navigation migration, shared recovery, agent settings, annotation draft preservation, tag search, audit filtering, replacement reminders and event cleanup.

Optional localhost integration (requires `/usr/sbin/sshd`):

```sh
SSH_GUI_LOCAL_NETWORK=1 cargo test --locked --manifest-path src-tauri/Cargo.toml acceptance::local_workflow -- --nocapture
```

This runs a temporary server bound to `127.0.0.1` and exercises authentication, key installation and key comparison. It uses temporary keys/configuration. The system `ssh-copy-id` creates and removes its own scratch directory under `~/.ssh`; it does not alter existing keys or config.

## Isolated manual testing

After building, create a workspace and run the binary directly:

```sh
preview_dir=$(mktemp -d /private/tmp/ssh-gui-preview.XXXXXX)
SSH_GUI_WORKSPACE="$preview_dir" SSH_AUTH_SOCK="$preview_dir/no-agent.sock" src-tauri/target/release/ssh-gui
```

The visible banner identifies the workspace. SSH files live in its `ssh/` subdirectory; app preferences and UI storage are isolated too. The example intentionally has no agent. `SSH_GUI_WORKSPACE` alone does not isolate an inherited agent: use a separately created `ssh-agent` socket to test enrollment. Keep any keys you want before manually removing a test workspace. Never point a test at a real server unless that test is explicitly intended.

## Build and assets

```sh
npm run icons
npm run release
```

`public/app-icon.svg` is the editable application icon source; `src-tauri/icons/tray-source.svg` is its monochrome menu-bar counterpart. The icon script generates the desktop assets reproducibly with the installed Tauri CLI. Do not hand-edit generated PNG, ICO or ICNS files.

The default release command bundles `.app` and `.dmg` for the current architecture. For a faster application-only build:

```sh
npm run tauri build -- --bundles app
```

See [Release guide](RELEASING.md) before tagging. Follow the [interface guidelines](INTERFACE_GUIDELINES.md) and [screen ownership](SCREEN_OWNERSHIP.md) when adding UI.

The 0.3.1 acceptance scenario covers repository access preview/apply through real Tauri IPC: unchanged preview, stale config/fingerprint rejection, Git locks, forged plans, SSH override/include/push-URL conflicts, exact-folder inspection and previous-config backup. UI tests cover removed navigation migration, folder coverage and explicit URL review/application.

The 0.3.2 key replacement scenario verifies selection of main/included host directives and a Git profile through IPC; unchanged old/new key bytes and metadata; preserved unselected, Match, wildcard and external references; stale config/fingerprint and forged-plan rejection; CRLF/comment preservation; and recovery backup discovery. Unit tests inject a write failure after the first file to verify rollback and preflight refusal before any config change. UI tests cover the four steps, empty initial selection, blocked references, registration acknowledgement, same-identity exclusion, cancel and stale-plan refresh.
