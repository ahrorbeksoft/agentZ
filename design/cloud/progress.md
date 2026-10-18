# Cloud machines: build progress

The UI spec is `decisions.md` (topic numbers below are its sections); everything else is in
`plan.md`. Agents building this: take the first item that isn't done, mark it in progress here
in the same commit as your first change, and mark it done with its commit when it's finished.
Each item is committed on its own, so `git log` and `git status` show where work stopped.

Notes for whoever continues:
- **Don't push** this work: the user asked for commits only, for now (2026-10-18).
- Proton's CLI is installed and signed in on this Mac (`proton`, in `~/.local/bin`), with one
  computer, `devbox-3`, running: the user's `proton-cloud` machine. Only read-only commands
  may be run against it (`whoami`, `list`, `show`, `balance`, `ssh-config`): starting,
  stopping, creating or deleting a computer costs money or loses work. Test those with a fake
  `proton` on `PATH`.
- Instabox's CLI isn't installed here, and there's no account. Its parsers follow its docs and
  `--help`; their fixtures are *(unverified)* until someone captures real outputs.
- Never sign in or out of a provider's CLI: the user does that (topic 3's note).

## Items

1. **The `cloud` crate**: provider descriptions (Instabox, Proton), detection (on `PATH`,
   `--version`, `whoami`), and each CLI's commands with parsers against captured outputs.
   Status: done in its own commit ("Add the cloud crate…"). Proton's fixtures are captured
   from its CLI on this Mac; Instabox's follow its API reference.
2. **SSH options** in `agentz_client::ssh`: a machine reached by options from a provider's
   `ssh-config` rather than a target in `~/.ssh/config`. Status: done in its own commit
   ("Reach machines with a provider's SSH options"): `Ssh::with_options`, and hints that name
   the command that works in a terminal.
3. **Providers in this Mac's server**: `cloud.json`, detection on demand and every few minutes
   while an app is open, the protocol (a snapshot, events, requests for lifecycle changes and
   SSH options). Status: done in its own commit ("Find the cloud providers…"):
   `server/cloud_requests.rs`, `Request::CheckCloud` and friends, `ServerConfig::cloud_path`
   for tests. Only this Mac's app asks, so other machines never run the CLIs.
4. **Cloud machines in the app**: `MachineProfile`'s source, connecting with the provider's SSH
   options (Instabox's certificate renewed first), the provider's states (a stopped machine
   isn't dialed), and Settings › Machines grouped by provider with More Providers and Check
   Again (topics 1 C, 2 A, 3's note, 6 B, 10 B, 13 A). Status: done in its own commit ("Group
   Settings › Machines by cloud provider…"): `MachineProfile::cloud`, `Transport::Cloud` and
   `MachineStatus::NotRunning` in `server_client.rs`, SSH machines linked by address in
   `machines.rs`, `cloud_machines.rs` for the words and marks, and
   `settings_page/cloud_providers.rs`. The server renews Instabox's certificate (item 3).
   Starting asks first (topic 5 C) from the row's button already; item 5 adds it to creating.
5. **Adding, starting, removing**: Add Machine's menu and each provider's dialog (topic 4 C),
   asking before charging the first time per account (topic 5 C), Remove with a box to delete
   (topic 9 B). Status: done in its own commit ("Add, create and remove cloud machines…"):
   `Request::CloudSources` (Instabox's templates and snapshots; Proton has none) and
   `Response::CloudMachineCreated` in the server, `settings_page/new_cloud_machine.rs` for the
   dialog, Add Machine's menu and Remove in `settings_page/cloud_providers.rs`, and
   `ConfirmCheckbox::type_to_confirm` for the name to type. Instabox's `templates` and
   `snapshot ls` fixtures are *(unverified)*; creating a Proton computer is tested with a fake
   `proton` only.
6. **A machine's page** with its provider's tabs (topic 7 A), snapshots on it (topic 8 A),
   ports, Vault, SSH tokens and Proton's keys; Instabox's API key through its CLI, for desktop
   links and CPU and memory. Status: not started.
7. **Machines that are off**: the list of every remote machine's projects and threads kept on
   this Mac, and messages held for a stopped machine, which start it (topic 11 C). Status: not
   started.
8. **Idle machines** (topic 12 A–D) and the Usage page's Cloud section (topic 13 C). Status:
   not started.
9. **Docs**: `docs/architecture.md`'s feature section and `AGENTS.md`'s testing notes. Status:
   not started.
