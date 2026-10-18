# Cloud machines: build progress

The UI spec is `decisions.md` (topic numbers below are its sections); everything else is in
`plan.md`. Agents building this: take the first item that isn't done, mark it in progress here
in the same commit as your first change, and mark it done with its commit when it's finished.
Each item is committed on its own, so `git log` and `git status` show where work stopped.

Notes for whoever continues:
- Items 1–10 are merged into `main` and pushed (the user, 2026-10-19).
- Proton's CLI is installed and signed in on this Mac (`proton`, in `~/.local/bin`); the
  account has no computers now (2026-10-19). Only read-only commands may be run against it
  (`whoami`, `list`, `show`, `balance`, `ssh-config`): starting, stopping, creating or
  deleting a computer costs money or loses work. Test those with a fake `proton` on `PATH`.
- **Instabox was dropped** (the user, 2026-10-19; item 10): its account pauses machines after
  30 idle minutes whatever they ask (`fixedAutoSuspendSeconds` in `whoami --json`, and
  `auto-suspend <machine> 0` doesn't change it), and allows timeouts up to 4 hours. On three
  test machines, only a process printing to a terminal of its own on the machine held the
  pause off; an open SSH session, web requests and `instabox get` didn't. agentZ supports
  Proton only. Its CLI is still installed on this Mac, with `devbox5` paused, but agentZ
  doesn't run it.
- Never sign in or out of a provider's CLI: the user does that (topic 3's note).

## Items

Items 1–9 were built for Instabox and Proton. Item 10 removed Instabox and what only it used,
so the Instabox parts they name are gone.
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
   links and CPU and memory. Status: done in two commits. The server's ("Read a cloud
   machine's details…"): `Request::CloudMachineDetails` and `CloudMachineAction`,
   `cloud::InstaboxApi` (its key in `cloud/instabox/api-key`, made with `instabox keys
   create`), Instabox's `get`, `vault show` and `port ls` fixtures. The app's ("Open a cloud
   machine's page…"): `settings_page/cloud_machine_page.rs` for the page and its tabs,
   `settings_page/cloud_machine_dialog.rs` for Save Snapshot… and Publish Port… (in the
   shell's modal layer), and in `settings_page/cloud_providers.rs` the row's mark and name,
   which open it, and the row menu's Open Desktop, Save Snapshot…, Publish Port…, SSH Keys…
   and Machine Settings…. The page reads the machine again after each listing, and closes
   once the provider doesn't list it. Its actions are tested against a test server only.
7. **Machines that are off** (topic 11 C): opening a thread on a cloud machine that isn't
   running starts it where that's free (resuming or starting an Instabox machine), with
   "Resuming review-box…" and Cancel, which puts it back once the provider is done; one that
   charges (a Proton computer) says "proton-cloud is stopped" with Start and its price, which
   asks topic 5's question first. Only the first look counts: a machine found running when
   the thread opened isn't started again once it stops. The list of every remote machine's
   projects and threads kept on this Mac, and messages held for a stopped machine, are put
   off for later (the user, 2026-10-19). Status: done in its own commit ("Start a stopped
   cloud machine from its thread…"). `ServerClient::start_cloud_machine`
   (the start is under way until the provider lists its machines again after its CLI is
   done), the thread's notice in `agent_view.rs`, and `cloud_machines::charge_question`,
   shared with Settings › Machines.
8. **Idle machines** (topic 12 A–D) and the Usage page's Cloud section (topic 13 C). Status:
   done in two commits. The Usage page's Cloud section is in its own ("Show each cloud
   account's usage on Settings › Usage"): `cloud_providers::render_cloud_usage`, with
   `cloud_machines::limit_uses` shared with Machines' header line, which turns yellow from 80%
   of a limit, as Instabox's Account page warns from 80% of storage. Instabox counts storage
   in GiB (`maxStoredGiB`). Proton's `balance` doesn't say over what period its runtime is
   counted, so the page calls it "Run time", not "This month". The rest is in the other
   ("Hold off idle pauses, stop idle machines…"): the app tells this Mac's server what works
   on each provider machine and whether anything of it is open
   (`Request::ReportCloudActivity`, from `Machines::sync_cloud_activity` and
   `Shell::sync_open_machines`), and the server (`server/cloud_requests.rs`), with the app
   closed too, holds off Instabox's idle pause while agents work (`auto-suspend <machine> 0`,
   the pause set back after, `ProviderState::held_awake`), stops a Proton computer left idle
   as long as its page's "When idle" says (`ProviderState::stop_when_idle`), and keeps notices
   of a stop it didn't ask for and of a balance that runs out within a day
   (`CloudSnapshot::notices`). The app shows each as a system notification, with Resume,
   Start or Add Credit where that's free (`cloud_machines::notification`, the shell's
   notification handler), whatever `notify_when_unfocused` says, and dismisses them
   (`Request::DismissCloudNotices`). Instabox's idle pause (topic 12 A) is honoured only for
   accounts an admin gave manual lifecycle (`manualLifecycle` in `whoami --json`); the user's
   account has none, so it's fixed at 30 minutes there, holding it off is tested with a fake
   `instabox` only, and the machine's page says turns may stop when Instabox pauses it.
9. **Docs**: `docs/architecture.md`'s feature section and `AGENTS.md`'s testing notes. Status:
   done in its own commit ("Document cloud machines…"): the Cloud machines feature section, the
   `cloud` crate, `cloud.json` and the Instabox key in the data table, and the fake CLIs.
10. **Drop Instabox** (the user, 2026-10-19), and what only it used: its CLI, API and fixtures
    in the `cloud` crate; templates, snapshots, timeouts and idle pause in the protocol and the
    New Machine dialog; holding off its idle pause in the server; the Snapshots, Ports,
    Desktop, Vault and SSH tabs and their dialogs on a machine's page; its limits on the Usage
    page; and the thread's free start with Cancel (topic 11 C). With the user, the thread's
    Start went too, since Proton's start charges and a stopped computer's threads are only
    reachable while the app stays open: a thread on a stopped computer has the usual offline
    notice, which says it's stopped, and a stopped computer's notification has no button.
    Status: done in its own commit ("Drop Instabox…").
11. **agentZ's own SSH key** (the user, 2026-10-19): making a computer failed with "no SSH key
    found" on this Mac, whose keys in `~/.ssh` have other names than the three Proton's CLI
    looks for, and agentZ reached computers only with `ssh`'s defaults. The user chose a key
    of agentZ's own over picking one in New Computer: `cloud::OwnKey`, in `cloud/id_ed25519`,
    made when first needed, given to `create`, kept by Replace Keys, and the first
    `IdentityFile` of a computer's SSH options. Status: done in its own commit ("Make cloud
    computers with agentZ's own SSH key").
