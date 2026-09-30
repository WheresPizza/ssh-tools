# Behavior and security boundaries

Reference for SSH GUI 0.3.0. Read alongside the [user guide](USER_GUIDE.md).

## Data handling

SSH data stays in `~/.ssh`. App preferences live in the platform config directory under `ssh-gui`; launch history is stored locally in the webview.

Configuration updates preserve unchanged text and refuse stale edits. Before changing a config file or `known_hosts`, the app saves the previous contents as `.config.ssh-gui.bak` or `.known_hosts.ssh-gui.bak` (one previous version). Writes use private temporary files and atomic replacement. Key creation refuses existing private **or** public filenames. Deleting a key removes it from the connected agent when present; it does not revoke keys already installed on servers.

The key library manages regular private-key files beneath the SSH directory (up to 32 directory levels). Symbolic links are not followed for key mutations or permission repairs. OpenSSH keys expose their public portion even when encrypted; the app can copy it without a `.pub` companion. Encrypted legacy PEM keys require their companion. Malformed keys are shown with an error and cannot be mutated through the app.

Generated keys are encrypted in Rust using the supplied passphrase; the passphrase is not passed in subprocess arguments. Encrypted agent enrollment prompts in the terminal and uses the app's selected agent socket. Key deletion checks the displayed fingerprint before removing a pair.

The editor displays physical Host blocks, including source paths; it does not calculate effective inherited options. Included files outside the SSH workspace are read-only. Duplicate aliases are addressed by source and line. Include/Match sections and unknown directives are preserved; reordering files with Include, Match, duplicate aliases or wildcard rules is disabled because ordering affects SSH behavior. Hidden backups avoid matching ordinary Include globs. Include traversal is limited to 16 levels, 256 files and 2 MiB per file.

A network key comparison reports whether the scanned key matches the stored key; it is not independent proof of the server's identity. Hashed names and certificate-authority/revocation records cannot be compared this way.

Copy to Server uses `ssh-copy-id -f` so an existing configured identity cannot incorrectly cause the new key to be skipped. Repeating the operation can append a duplicate authorized-key entry. The terminal reports authentication and installation results.

## Key annotations and audit

Annotations live in `key-metadata.json` under the application config directory (`settings` in isolated mode), keyed by SHA256 fingerprint. Copies share tags, purpose, notes and a replacement date. Deleting or moving a key file does not delete its annotations; importing the same identity restores that association. Metadata does not contain private key bytes and is not encrypted; do not store secrets in notes. Writes use the common private atomic-write mechanism, retain one previous metadata version and reject stale revisions, symlink destinations and changed key fingerprints. The Settings recovery inventory covers SSH files, not this metadata file.

Limits: 20 tags, 64 UTF-8 bytes per tag, 256 bytes for purpose, 4096 bytes for notes and a 1 MiB metadata file. Tags cannot contain commas or control characters. Notes allow newlines and tabs. Dates must be real YYYY-MM-DD dates. A planned date does not expire the key; the UI compares it with the local calendar date during audit rendering.

Audit results are snapshots, not continuous monitoring or proof of successful authentication. The audit reads key files and current permissions, checks supported keypair identities, and scans configured repository folders once for the whole library. It never connects to servers, executes repository SSH commands, unlocks encrypted keys or applies fixes. Failed permission/relationship checks produce incomplete-check warnings. No local references means only that none were found in the checked data. Permission repair remains a separate explicit action. Details list references for the selected file and identify other local copies; they do not combine different paths into an assumed effective SSH identity.

## Profile, repository and diagnostic behavior

Git profiles occupy a generated section at the start of the main SSH config. Profile metadata and directives are saved in one atomic write. The original configuration follows an explicit `Host *` boundary and is preserved byte-for-byte when the last profile is removed. The generated section is read-only on the SSH Hosts screen; use Git Profiles to edit it. External changes to that generated section are rejected rather than overwritten. Register the public key with your Git provider yourself; saving a profile does not register or verify an account. Other existing wildcard `IdentityFile` directives may add further candidates; Diagnostics shows the effective configuration.

Repository scans are read-only and limited to selected folders: up to 32 roots, 12 directory levels, 20,000 visited directories and 2,000 repositories. Hidden folders, symlinks and common dependency/build folders are skipped. Scan warnings identify incomplete results. Relationships represent declared key candidates, not proof of actual use: SSH Match rules, agent keys, Git includes, `core.sshCommand`, environment overrides and global URL rewrites can alter authentication. Git configuration includes and repository SSH commands are never executed by the scanner. Remote URL credentials are hidden in the UI. Removing a scan root does not remove files.

Deletion previews inspect current SSH config, profiles and the selected repository folders. Unscanned projects and remote installations are outside that check. Removing a profile keeps its key and does not rewrite Git remotes.

Diagnostics evaluates configuration with `ssh -G`. Automatic evaluation refuses `Match exec` and dynamically expanded Include paths. Configuration evaluation can perform hostname resolution. Authentication is a separate explicit action using public keys, batch mode, strict existing host trust, no remote shell/command, no port forwards and an eight-second process-group timeout. Proxy configuration is honored. The app does not accept unknown host keys during diagnostics.

Import copies a validated source into a private staging directory, checks the reviewed source revision and fingerprint duplicates, and publishes without overwriting either destination file. OpenSSH encrypted keys keep their original bytes. Encrypted legacy PEM requires a public companion and displays that its pairing cannot be verified without unlocking; use OpenSSH format for self-contained public-key recovery.

Agent lifetime and confirmation constraints are enforced by `ssh-agent`, including after the app closes. Per-use confirmation requires a working agent-side `SSH_ASKPASS` helper; without confirmation, signing is refused. The UI refreshes actual agent membership on focus and every 15 seconds. OpenSSH does not expose current lifetime/confirmation constraints through key listing, so the UI labels settings as the last enrollment requested by this app session and does not invent policies for externally loaded keys.

## Dependency audit (2026-09-30)

`npm audit` reports zero vulnerabilities. RustSec prompted updates to `plist` (1.10.0, using quick-xml 0.41.0), `anyhow` (1.0.104), and `rand` 0.8 (0.8.8).

The full cross-platform Cargo lockfile still reports three vulnerability findings: quick-xml 0.38.4 (two XML DoS advisories, through Linux Wayland) and optional rsa 0.9.10 (Marvin timing advisory). Neither package is in the current macOS dependency tree (`cargo tree -i ...` reports no entries). The app generates/signs RSA keys through system OpenSSH; ssh-key's RSA arithmetic feature is not enabled. The glib warning is also outside the macOS tree.

RustSec additionally flags older unmaintained transitive Tauri libraries (fxhash, proc-macro-error and unic packages), and rand 0.7.3 used by PHF code generation. The rand advisory requires a custom reentrant logger and the log feature; that feature is absent from this dependency tree. These upstream maintenance findings remain visible; the project does not suppress them or claim an entirely clean cross-platform Rust audit. Linux/Windows releases are not validated by this iteration.

