# User guide

## Find the right screen

Workspace contains Repositories, Git Profiles and SSH Hosts. Security contains SSH Keys and Trusted Hosts. Tools contains Diagnostics and Settings. Each entity has one editing owner; shortcuts such as Diagnose open the shared tool with context.

## Keys and the agent

Use **Generate Key** or **Import Key** in SSH Keys. Ed25519 is the default. A passphrase protects the stored private key; it is different from agent confirmation.

Import first inspects the file and shows its fingerprint, encryption state and duplicate status. Review the destination filename before importing. The source remains unchanged. Missing OpenSSH public companions can be recovered; encrypted legacy PEM has additional limitations described in [Behavior](BEHAVIOR.md).

**Copy Pub Key** copies the shareable public part. **Copy to Server** opens a terminal to install it in the destination's `authorized_keys`. It does not upload a private key. Finish authentication in the terminal; the app cannot infer installation success merely from opening that window. Repeating installation can append duplicates.

**Add to Agent** chooses a lifetime and optional per-use confirmation. For encrypted keys, enter the passphrase in the terminal and return to the app. Confirmation requires a working agent-side `SSH_ASKPASS` helper. Without one, confirmation-protected signing is refused. **Remove from Agent** unloads the key without deleting its files.

Deletion reviews known profile, host and scanned-repository references and checks the key fingerprint. Its coverage excludes unscanned projects and remote installations. Keep your own secure backups before deleting keys you might need later.

## Key details, labels and local audit

Open **Details** beside a key to see its fingerprint, file paths, encryption state, agent membership, copies with the same fingerprint and references to the selected file. The file timestamp is its modification time, not proof of when the identity was created. Each copy remains a separate file row because deletion and references are path-specific.

**Labels & notes** contains comma-separated tags, purpose, notes and an optional planned replacement date. **Save annotations** updates all copies of the same identity. Search matches tags and purpose as well as names, comments and fingerprints; **All tags** narrows the library by a selected tag. Tags are trimmed and deduplicated case-insensitively. Notes are plain local text: do not put secrets in them.

The replacement date is a reminder, not an SSH expiration or revocation mechanism. Due dates appear as warnings in the audit. A stale-save error keeps your draft intact: copy important edits, discard the draft, reload annotations and reapply them. Leaving the detail view with Back to keys prompts before discarding edits.

**Audit keys** checks the local library on demand. Filter errors, warnings or informational notes, open **Local references and scan coverage** for evidence, or open a key's details. It reports malformed or mismatched pairs, missing companions, duplicate identities, unencrypted private files, excess permissions and missing local links. An encrypted legacy pair may remain unverified without unlocking. Absence of local references never proves a key is unused remotely. The audit makes no network connections and does not modify keys; permission repair remains in the existing permissions panel. Run it again after changing files or configuration.

## Multiple Git accounts

1. Create/import a separate key for the account and register its public key with your provider.
2. In Git Profiles, choose **New profile**. Set a display name, account label, unique SSH alias, service hostname, SSH user/port and existing key.
3. Save, then enter a repository path such as `team/service` to obtain an account-specific clone URL.
4. Use that URL in your normal Git workflow. Existing remotes are not rewritten.

The account label is descriptive; many Git providers use the SSH user `git` for every account. Generated profile entries appear read-only in SSH Hosts. Edit them through Git Profiles. Deleting a profile retains its key but breaks remotes that still reference its removed alias.

## Local repositories

In Repositories choose **Add project folder**. The scanner discovers Git remotes, including supported worktree layouts, and maps configured aliases/profiles/key paths. Search narrows the displayed results; **Rescan** refreshes them. **Stop scanning** removes a root from the inventory without deleting files.

Scan warnings matter: traversal is bounded and dependency/build directories are skipped. This is a read-only inventory, not a remote editor or an authentication result.

## Server connections

Use SSH Hosts to manage aliases and configuration rules. Select an existing key, or supply IdentityFile paths when configuring advanced cases. Included files outside the managed SSH directory and generated Git sections are read-only. Reordering is unavailable where OpenSSH ordering semantics make it unsafe.

**Connect** opens the selected alias in your configured terminal. The displayed launch history is not a record of successful logins. Set the terminal in **Settings → Terminal**.

## Server trust and diagnostics

Trusted Hosts manages server identities stored in `known_hosts`, not your personal credentials. **Compare key** makes a network comparison; agreement alone is not independent verification of the server's identity. Hashed names and special marker entries cannot be compared this way. Removing an entry means SSH may ask for trust again.

In Diagnostics, start with **Check configuration**. It evaluates effective host, user, port, identities and agent state; hostname resolution may occur. **Test authentication** is a separate network action with an eight-second bound. It requires existing host trust and does not open a remote shell or accept new host keys. Encrypted keys must already be available through the agent.

## Recovery and troubleshooting

- **Stale edit:** reload the current data before editing again; another process changed the file.
- **Wrong Git identity:** check the profile alias in the remote URL, then inspect effective configuration in Diagnostics. Wildcard rules, Git overrides and agent identities can change behavior.
- **Agent unavailable:** launch the app with the intended `SSH_AUTH_SOCK`; use a dedicated agent for isolated tests.
- **Unknown server key:** verify the fingerprint independently and establish trust through your normal SSH workflow before retrying diagnostics.
- **Permission issue:** inspect the listed changes on SSH Keys before using the repair action. It removes excess access and does not widen stricter permissions.
- **Restore configuration:** open Settings → File recovery, expand Backups and recovery, review the file preview, then confirm restoration. This can affect both host rules and Git profiles. Only one previous version is retained; private keys are not included.

Closing the window hides the app to the menu bar. Reopen it from the menu bar or Dock; use Quit to exit.
