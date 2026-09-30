<p align="center"><img src="public/app-icon.svg" width="112" height="112" alt="SSH GUI — terminal and key" /></p>
<h1 align="center">SSH GUI</h1>
<p align="center">Your SSH keys, Git identities and connections. One local workspace.</p>
<p align="center"><a href="https://github.com/WheresPizza/ssh-tools/actions/workflows/ci.yml"><img src="https://github.com/WheresPizza/ssh-tools/actions/workflows/ci.yml/badge.svg" alt="CI" /></a> <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT license" /></a> <img src="https://img.shields.io/badge/platform-macOS-lightgrey.svg" alt="macOS" /></p>

SSH GUI is a compact desktop app for developers who juggle personal, work and client SSH identities. Create or import keys, give each Git account its own alias, find which repositories reference those aliases, and investigate connection problems without jumping between configuration files.

Built with **Tauri, Rust, React and TypeScript**, with a terminal-inspired interface. It works with your local OpenSSH files and tools; no SSH GUI account or hosted service is required.

## What you can do

| Screen | What it does |
| --- | --- |
| **Repositories** | Discover Git repositories in selected folders and inspect remote → SSH alias → profile/key relationships. Scanning is read-only. |
| **Git Profiles** | Separate work, personal and client accounts with named profiles, dedicated SSH aliases, key selection and account-specific clone URLs. |
| **SSH Hosts** | Add, edit, duplicate, search and reorder configuration blocks, inspect included files, and open connections in your terminal. |
| **SSH Keys** | Generate Ed25519, RSA or ECDSA keys; import existing keys; copy or recover public keys; inspect permissions and references before deletion. |
| **Trusted Hosts** | Inspect server identities in `known_hosts`, compare stored keys with a network scan, and remove selected trust records. |
| **Diagnostics** | Inspect effective OpenSSH settings and agent availability, then explicitly test public-key authentication with a bounded timeout. |
| **Settings** | Choose your terminal and review or restore the previous version of supported SSH configuration/trust files. |

Key management also includes:

- **Agent policies:** one hour, eight hours, a time today or the agent default; optional confirmation for every use.
- **Encrypted keys:** passphrase-protected generation and imports that retain their original protection. Agent enrollment prompts in the terminal.
- **Duplicate-aware import:** fingerprint checks, reviewed-source validation and no silent overwriting of existing files.
- **Public-key installation:** open `ssh-copy-id` in your terminal to add a key to a server.
- **Local safeguards:** stale-edit detection, atomic configuration updates, a previous-version backup, and checks against unsafe path mutations.

## Get started

Download a build from [GitHub Releases](https://github.com/WheresPizza/ssh-tools/releases), when available. Choose the artifact matching your Mac's architecture, open the `.dmg`, and drag **SSH GUI.app** to Applications. This release targets **macOS**; Linux and Windows are not validated release targets.

Current builds are **unsigned and not notarized**. macOS may block opening them. Release assets should state their signing status; do not expect a notarized installer. If no compatible asset is available, [build from source](docs/DEVELOPMENT.md).

The app uses the OpenSSH tools on your machine. Key installation additionally needs `ssh-copy-id`. Terminal, iTerm2, Alacritty and kitty are supported in their standard macOS locations; Terminal is the fallback.

### A typical workflow

1. Open **SSH Keys** and generate or import a key.
2. Register its **public** key with your Git provider, then create a **Git Profile** that selects it.
3. Enter `owner/repository` in Git Profiles and copy the clone URL for the account you want.
4. Add your project folder in **Repositories** to inspect local relationships.
5. Use **Diagnose** to review configuration. Run **Test authentication** when you want to contact the server.

For servers, create a connection in **SSH Hosts** and use **Connect**. Configure your preferred terminal in Settings.

## Clear boundaries

- SSH configuration and keys normally remain in `~/.ssh`; preferences and UI history are stored locally.
- Repository scans show **configured key candidates**, not proof that a particular key authenticated successfully.
- Saving a Git profile does not create a provider account, upload a key or rewrite Git remotes.
- Removing a local key does not revoke copies already installed on servers. Removing a trusted-host entry does not delete a connection alias.
- Recovery keeps **one previous version** of supported configuration files. It is not a private-key backup system.
- Diagnostics does not accept unknown server keys. Network operations may use your existing proxy configuration.
- OpenSSH does not report current agent lifetime/confirmation policy. The app displays actual membership and the last policy requested during this app session.

See [behavior and security boundaries](docs/BEHAVIOR.md) for format support, scan limits, configuration semantics and known dependency findings.

## Development

Install Node.js supported by `package.json`, stable Rust, Xcode Command Line Tools and the required OpenSSH tools, then:

```sh
git clone https://github.com/WheresPizza/ssh-tools.git
cd ssh-tools
npm ci
npm run tauri dev
```

```sh
npm test
npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml
npm run release:check
npm run release
```

The final command produces native bundles under `src-tauri/target/release/bundle/`. Builds use the current machine's architecture. See the [development guide](docs/DEVELOPMENT.md) for isolated testing and the optional localhost acceptance test.

## Documentation

- [User guide](docs/USER_GUIDE.md) — common workflows and troubleshooting.
- [Development](docs/DEVELOPMENT.md) — environment, tests, isolated workspaces and builds.
- [Behavior and security boundaries](docs/BEHAVIOR.md) — what the app guarantees and what it cannot infer.
- [Release guide](docs/RELEASING.md) — validation, packaging, signing status and draft releases.
- [Interface guidelines](docs/INTERFACE_GUIDELINES.md) and [screen ownership](docs/SCREEN_OWNERSHIP.md).
- [Changelog](CHANGELOG.md), [contributing](CONTRIBUTING.md), [security reporting](SECURITY.md).

## License

[MIT](LICENSE).
