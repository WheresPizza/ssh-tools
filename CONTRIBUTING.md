# Contributing

Read [Development](docs/DEVELOPMENT.md) for setup and verification, and [Screen ownership](docs/SCREEN_OWNERSHIP.md) before introducing a new UI workflow.

Use disposable fixtures and isolated workspaces. Never commit private keys, passphrases, real host configuration, access tokens or personal screenshots. Keep filesystem mutation, stale-edit checks, confirmation behavior and OpenSSH semantics covered by relevant tests.

For UI changes, use the shared [interface guidelines](docs/INTERFACE_GUIDELINES.md), check keyboard focus and long paths, and review light/dark behavior. For behavior changes, update the user guide and CHANGELOG. State what changed and how it was verified in the pull request.

Report bugs with the app version, macOS/architecture, steps and sanitized error text. Remove private paths, usernames, hostnames and credentials as appropriate. See [SECURITY.md](SECURITY.md) for sensitive reports.
