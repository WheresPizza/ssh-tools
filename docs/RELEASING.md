# Release guide

## Release state

Version **0.3.0** targets macOS. Local builds are unsigned and not notarized. The checked-in release workflow does not configure signing credentials. Do not describe these builds as signed, notarized, universal, or validated on Linux/Windows.

Preparing files is not publishing: create/push a tag and publish a GitHub release only when the maintainer intends to release.

## Before tagging

1. Keep versions aligned in `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` and `src-tauri/tauri.conf.json`.
2. Update CHANGELOG, README, user/development guides, dependency findings and UI documentation as applicable.
3. Run `npm ci`, `npm test`, `npm run build`, `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`, `cargo test --locked --manifest-path src-tauri/Cargo.toml` and `npm run release:check`.
4. Review `npm audit` and `cargo audit --file src-tauri/Cargo.lock`. Record remaining findings and their actual target/feature exposure; do not silently suppress them.
5. Run the optional localhost acceptance test from the development guide. Review populated/empty pages and dialogs in an isolated native workspace.
6. Run `npm run icons` if icon sources changed, then `npm run release`. Open the DMG, verify its contents and inspect the app's icon and architecture. Confirm signing/notarization status explicitly.
7. Review the final Git diff. Do not ship temporary keys, credentials, private config, screenshots containing personal data or local build directories.

`npm run release:check` checks version consistency, the matching changelog heading, required docs/assets and versioned release notes and (when `RELEASE_TAG` is set) the exact `v<version>` tag. It does not replace tests, audits, visual review or signing validation.

## GitHub release workflow

A pushed `v*.*.*` tag triggers `.github/workflows/release.yml`. It validates the tag/version relationship, runs frontend and Rust checks, then builds native `.app`/`.dmg` assets with the checked-in Tauri configuration. It creates a **draft**, not a public release. The build architecture follows the hosted macOS runner; inspect the artifact filename and executable before publishing.

For 0.3.0, the tag is `v0.3.0`. Before publishing the draft:

- Confirm CI succeeded and assets open correctly on the target architecture.
- Review the versioned notes in `docs/releases/<version>.md`, which the workflow inserts into the draft.
- State architecture, macOS scope, and unsigned/not-notarized status clearly.
- Attach checksums if distributing assets outside GitHub.
- Review remaining dependency advisories in `docs/BEHAVIOR.md`.

Signing and notarization require a separate, deliberate credential setup and release validation. Never commit certificates, passwords or signing secrets.

## Local outputs

- `src-tauri/target/release/bundle/macos/SSH GUI.app`
- `src-tauri/target/release/bundle/dmg/*.dmg`

Record actual verification results when preparing a release. Do not present a local test pass as evidence that a GitHub workflow has run or that a public release exists.

## macOS packaging locale

`npm run release` sets `en_US.UTF-8` for macOS packaging. The system Perl invoked by the DMG builder fails under an inherited `C.UTF-8` locale. For direct `tauri bundle` invocations, use `LANG=en_US.UTF-8 LC_ALL=en_US.UTF-8 LC_CTYPE=en_US.UTF-8` as well.

Current local report: [0.3.0 verification](releases/0.3.0-verification.md). Previous release: [0.2.0 verification](releases/0.2.0-verification.md).
