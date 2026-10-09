# Release guide

## Release state

Version **0.3.3** is a local stabilization candidate targeting macOS; **0.3.2** remains the published baseline. Local 0.3.3 builds use an ad-hoc bundle signature with Hardened Runtime and the Apple Events automation entitlement. They are not Developer ID signed or notarized. The checked-in release workflow does not configure signing credentials. Do not describe these builds as Developer ID signed, notarized, universal, or validated on Linux/Windows.

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

For the proposed 0.3.3 release, the tag would be `v0.3.3`; do not create or push it until acceptance and publication are authorized. Before publishing the draft:

- Confirm CI succeeded and assets open correctly on the target architecture.
- Review the versioned notes in `docs/releases/<version>.md`, which the workflow inserts into the draft.
- State architecture, macOS scope, and ad-hoc/not-notarized status clearly.
- Attach checksums if distributing assets outside GitHub.
- Review remaining dependency advisories in `docs/BEHAVIOR.md`.

Developer ID signing and notarization require a separate, deliberate credential setup and release validation. Never commit certificates, passwords or signing secrets.

## Bundle signature verification

The macOS configuration uses `signingIdentity: "-"` and keeps Hardened Runtime enabled. `src-tauri/Entitlements.plist` grants only `com.apple.security.automation.apple-events`; the existing Info.plist explains terminal automation. macOS still controls permission to automate each terminal. This entitlement does not grant access automatically.

After packaging, run `codesign --verify --deep --strict "SSH GUI.app"` on both the mounted DMG and the extracted archive. Inspect `codesign -dv --verbose=4` for `adhoc,runtime`, the expected bundle identifier and sealed resources; use `codesign -d --entitlements -` to confirm the single entitlement. Verification of an ad-hoc signature establishes bundle integrity, not a trusted publisher or notarization. Repeat native terminal acceptance on this exact build; a valid signature alone does not prove Apple Events or SSH authentication works.

## Local outputs

- `src-tauri/target/release/bundle/macos/SSH GUI.app`
- `src-tauri/target/release/bundle/dmg/*.dmg`

Record actual verification results when preparing a release. Do not present a local test pass as evidence that a GitHub workflow has run or that a public release exists.

## macOS packaging locale

`npm run release` sets `en_US.UTF-8` for macOS packaging. The system Perl invoked by the DMG builder fails under an inherited `C.UTF-8` locale. For direct `tauri bundle` invocations, use `LANG=en_US.UTF-8 LC_ALL=en_US.UTF-8 LC_CTYPE=en_US.UTF-8` as well.

Current local report: [0.3.3 verification](releases/0.3.3-verification.md). Published release baseline: [0.3.2 verification](releases/0.3.2-verification.md).

## Stabilization acceptance gate for 0.3.3

Do not label 0.3.3 stable or publish it on the strength of unit, IPC, PTY or loopback tests alone. Complete the [action matrix and native gates](releases/0.3.3-verification.md), including protected/unprotected agent workflows, wrong input/cancel/restart, native pickers, minimum window sizes, every advertised macOS terminal adapter, clean installation and copied-data upgrades from 0.2.0/0.3.2. Verify the final DMG and archive, embedded version, architecture, signing limitations and SHA-256 hashes. A blocked native UI test is a failed readiness gate, not a pass or optional omission.

Development, packaging and acceptance remain local. Publishing needs a subsequent user request. Keep repository-local author `wherespizza <wherespizza@gmail.com>`; never change global Git identity. Batch pushes and request one final review only when the final state is ready.
