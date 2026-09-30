# Screen ownership

Every mutation has one owning screen. A contextual shortcut is useful when it opens the same workflow with the selected entity; a second implementation of that workflow is not.

| Screen | Entity / responsibility | Owns | Does not own |
| --- | --- | --- | --- |
| Repositories | Discovered local Git remotes and their SSH relationships | Scan roots, read-only discovery, mapping to profiles/keys | Editing remotes, managing keys or profile copies |
| Git Profiles | Named Git accounts backed by managed SSH aliases | Profile CRUD, identity assignment, account-specific clone URLs | Key generation, server trust, shell connections |
| SSH Hosts | OpenSSH Host blocks, aliases and connection rules | Host CRUD, ordering, duplication, terminal connection launches | Editing generated Git profile aliases or global application preferences |
| SSH Keys | Local private/public key pairs and agent enrollment | Generate/import, recover public part, copy public part, install public key, agent policy, file permissions, dependency-aware deletion | Git accounts, known_hosts server identities |
| Trusted Hosts | Server public identities in known_hosts | Compare server keys, remove local trust records | Client keys, account authentication or deleting connection aliases |
| Diagnostics | Effective configuration and authentication evidence | One shared diagnostic workflow, explicit network authentication test | Editing entities or implicitly opening sessions |
| Settings | Application-wide preferences and file recovery | Terminal preference and one backup review/restore workflow | Another host list or key backup claims |

## Changes from the previous layout

- Removed Launcher, which duplicated SSH Hosts and Connect. Terminal selection now belongs to Settings. Existing saved Launcher navigation resolves to SSH Hosts.
- Moved the shared cross-file backup inventory out of both SSH Config and Known Hosts into Settings. Both entity screens link to it. Restoring config can affect Git profiles; the recovery screen states this explicitly.
- Renamed SSH Config to SSH Hosts, and Known Hosts to Trusted Hosts, while retaining their actual OpenSSH file terminology in descriptions.
- Grouped navigation into Workspace, Security and Tools.
- Retained Diagnose shortcuts from profiles and repositories: they prefill the single Diagnostics screen, rather than introducing another diagnostic implementation.
- Retained key selection in both profile and host editors: they reference the same key library, but configure different owning entities. Profile-generated blocks remain visible and read-only in SSH Hosts so the effective configuration is not concealed.
- Retained Compare key in Trusted Hosts: comparing a server's identity is distinct from testing client authentication in Diagnostics.
- Retained per-key Copy to Server: installing a public key is different from copying it to the clipboard, selecting an IdentityFile or authenticating.

## Future features

Place a new action on the screen that owns the changed entity. Cross-screen actions should navigate with context or reuse a shared dialog. Keep recovery labels explicit about which file and entities a restore affects. Do not hide raw SSH rules simply to avoid visual repetition: they affect effective configuration and ordering.
