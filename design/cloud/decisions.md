# Cloud Machines: design decisions

Picked on the design board (`design/cloud/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Where providers live in Settings: C. Machines grouped by provider

*Settings*

**Today:** Settings › Machines lists This Mac and the machines saved by hand, each reached over SSH: its icon, its name over “SSH devbox1 · Connected”, then Edit, Remove and its switch. Add Machine asks for an SSH target and a name. A cloud computer (proton-cloud is a Proton computer) is just an SSH target: agentZ doesn’t know it’s Proton’s, can’t start or stop it, and doesn’t know what it costs.

**C. Machines grouped by provider** (from new): Machines keeps This Mac and SSH machines as today, then one group per provider account. A group’s header has the provider’s mark and name, the account, its usage (Proton’s balance and hours, Instabox’s counts against its limits), the provider’s page (Add Credit, Dashboard) and New Machine. Its machines are its rows; providers not set up are listed under the groups (topic 2).

## 2. Providers that aren’t set up: A. Rows under the providers, as agents to install

*Settings*

**Today:** agentZ knows no providers. On this Mac, Proton’s CLI (`proton`, in `~/.local/bin`) is installed and signed in as your account, and Instabox’s isn’t installed. Neither provider has a referral program today; Instabox needs an invite code for a new account (6 digits, from its admins; `instabox login --invite-code`), so a code they give agentZ would show here. Settings › Agents lists the registry’s agents that aren’t installed, each with Install.

**A. Rows under the providers, as agents to install** (from Settings › Agents): Under the provider groups, More Providers lists each provider whose CLI agentZ didn’t find: its mark, name and “Not installed”, one line on what it is and its price, agentZ’s code when there is one, then Copy Install Command and Learn More. A provider whose CLI is installed but signed out shows “Signed out” with Sign In… (topic 3). Once signed in, it becomes a group with its machines.

## 3. Signing in with the provider’s CLI: None of these

*Settings*

**Today:** An agent that logs in from a terminal (Settings › Agents) runs its login command in a terminal shown under its login buttons, and agentZ checks the login when it ends. Instabox’s CLI signs in with `instabox login` (the browser, GitHub or Google; `--device` without a browser; `--invite-code` for a new account), Proton’s with `proton login` (a code to approve in the browser). The mock’s output is illustrative.

**Note:** we don't need sign ins, they just set it up, and in app it just shows up, maybe we should check availablity button or refresh button or soething though

## 4. Adding a cloud machine: C. A menu on Add Machine

*Machines*

**Today:** Add Machine is one dialog: an SSH target (“user@host, or a Host from ~/.ssh/config”) and a name, with “agentZ installs its server in ~/.agentz there, and nothing else.” A cloud machine has to be made on the vendor’s site or CLI first, then added by its SSH host.

**C. A menu on Add Machine** (from new): Add Machine opens a menu: SSH Machine…, New Instabox Machine…, New Proton Computer…, then Add Existing… with each account’s machines not in agentZ yet. Each item opens a dialog of its own. A provider group’s New Machine opens its dialog directly.

## 5. Starting something that costs money: C. Ask the first time per account

*Machines*

**Today:** Nothing in agentZ costs money by itself. Proton bills $20 per 730 running hours from the moment a computer is created or started; its CLI asks first: “This computer costs $20 per 730 running hours (about one month), billed per running second from your shared Cube balance. Stopping it stops the charge. Continue?” (skipped with `--yes`). Instabox is free during its beta.

**C. Ask the first time per account** (from new): As B, with “Don’t ask again for this account”. Once ticked, the price shows on the buttons as in A. Settings › Machines › the account’s menu has Ask Before Charging to turn it back on.

## 6. A cloud machine’s row: B. A state badge, one action, a menu

*Machines*

**Today:** A row is the machine’s icon, its name over “SSH proton · Connected” (or Connecting, Uploading agentz-server · 40%, Reconnecting: the error), then Update Server when there’s one, Retry Now while it’s offline, Edit, Remove and its switch. A stopped Proton computer reads “Reconnecting” with an SSH error.

**B. A state badge, one action, a menu** (from t3code EnvironmentRow): The provider’s mark, the name over what it is (template and size), then a badge with the provider’s state (Running, Paused, Stopped, Starting, Suspended), one button for the likely action (Pause, Resume, Start, Stop; Add Credit when Proton suspended it), and a menu with the rest: Stop or Restart, Open Desktop, Copy SSH Command, Save Snapshot…, Publish Port…, Machine Settings…, Remove from agentZ, Delete Machine…. Connecting shows under the name as today.

## 7. Everything else a provider offers: A. A page per machine, with tabs

*Machines*

**Today:** A saved machine has only Edit (its target and name) and Remove. Instabox also has snapshots, public ports, a desktop with share links, a Vault of secrets, SSH tokens for other terminals, CPU and memory samples, running time, timeouts, idle pause and labels. Proton has its SSH keys and billing. Today they’re all on the vendors’ dashboards and CLIs.

**A. A page per machine, with tabs** (from the agent page (render_agent_page)): Clicking a cloud machine opens its page, as an agent’s does: back to Machines, its mark, name and state badge, its id, Pause and Open Desktop, and tabs for what its provider has. Overview: size, CPU and memory, running time, timeout, idle pause, Vault and labels. Snapshots: those saved from it, Save Snapshot. Ports: published URLs, Publish Port. Desktop: Open Desktop (a control link) and Copy View Link (view only, for an hour). Vault: what it gets, Push Now. SSH: Copy SSH Command (a 7-day token for any terminal), Revoke All. A Proton computer’s page has Overview (size, running time, cost) and SSH Keys. Desktop links and CPU and memory come only from Instabox’s API, not its CLI, so they’re in only if the plan’s open question says so.

## 8. Instabox snapshots: A. On the machine they came from

*Machines*

**Today:** Not in agentZ. An Instabox snapshot saves a machine’s software and its `/data` (home, workspace, saved sign-ins), or its software only; new machines start from it. Snapshots belong to the account (10 of them), not to the machine they came from, and stay when it’s deleted. Saving stops a running machine first, unless the server can save it while it runs.

**A. On the machine they came from** (from Instabox’s machine menu): The machine’s page (topic 6 A) has a Snapshots tab: those saved from it, Save Snapshot… (name, software only or with home and workspace), and per snapshot New Machine and Delete. Add Machine’s Start from › Snapshot lists all of them.

## 9. Removing or deleting a cloud machine: B. One Remove, with a box to delete

*Machines*

**Today:** Remove… takes a machine out of agentZ; the machine itself and agentZ’s server there stay. Proton’s own Delete asks you to type the computer’s name (“This permanently deletes this computer and its disk. Its files cannot be recovered.”).

**B. One Remove, with a box to delete** (from new): One Remove…: “Remove agents-1 from agentZ?” with “Also delete it at Instabox, with its disk” unticked. Ticking it asks for the name.

## 10. Machines an account already has: B. All of them, always

*Machines*

**Today:** proton-cloud is saved as the SSH target `proton`, an alias in `~/.ssh/config` for `ssh.proton.computer:22021`: that’s the Proton computer devbox-3. Machines made on a vendor’s site or CLI aren’t in agentZ until they’re added by hand.

**B. All of them, always** (from new): An account’s group lists all its machines, as the provider has them, refreshed every minute while Settings is open; new ones appear by themselves, and the one saved over SSH is linked and moves into the group. A machine’s switch (off for new ones) decides whether agentZ connects to it.

## 11. Opening something on a stopped machine: C. Start free ones, ask for paid ones

*Everywhere*

**Today:** A machine that’s unreachable shows as offline everywhere: dimmed, its input disabled, “Reconnecting” with an SSH error, and agentZ keeps retrying. A Proton computer that’s stopped stays that way until someone starts it on Proton’s site. An Instabox machine that paused for being idle wakes by itself when agentZ reconnects over SSH; one paused by hand or by its timeout waits for Resume.

**C. Start free ones, ask for paid ones** (from new): B where it costs nothing (resuming or starting an Instabox machine), A where it costs money (starting a Proton computer, with the price, and topic 4’s confirmation if picked).

**Comment on C:** we gotta talk about this, fundamentally we gotta store their threads on the main machine too, so new message should start these or ask you to start you know

## 12. Idle machines and running costs: A. Keep Instabox awake while agents work + B. Stop or pause machines left idle + C. Warn before a balance runs out + D. Say when a provider stops a machine

*Everywhere · pick any*

**Today:** agentZ keeps a machine awake while its agents work (`caffeinate`, `systemd-inhibit`), and stops idle agents after 30 minutes. Cloud providers have their own rules: an Instabox machine pauses after 30 idle minutes (“no agent working, no terminal use, little CPU”), and turning that off may need “manual lifecycle” on the account; a Proton computer runs, and bills, until it’s stopped, and is suspended when the balance runs out.

**A. Keep Instabox awake while agents work** (from agentZ’s keep-awake): While an agent on an Instabox machine works, this Mac’s server holds off the idle pause, as it holds off sleep: it runs `instabox auto-suspend <machine> 0`, and sets it back when the work ends. If the account refuses that (it may need manual lifecycle), agentZ says on the machine’s page that turns may stop when Instabox pauses it.

**B. Stop or pause machines left idle** (from new): Per machine, Stop when idle (Proton) or Pause when idle (Instabox): after no agent has worked and no thread, terminal or workspace on it has been open for the time picked (Never, 30 min, 1 hour, 4 hours). Never is the default.

**C. Warn before a balance runs out** (from Proton billing): A notification when Proton’s balance would run out within a day at the current rate, and when Proton suspends a computer for it, with Add Credit.

**D. Say when a provider stops a machine** (from new): A notification when a machine pauses or stops without agentZ asking (Instabox’s idle pause or timeout, Proton’s suspension), naming the threads that were working.

## 13. Cost, usage and limits: C. Both

*Everywhere*

**Today:** Settings › Usage shows each agent account’s limits (5-hour, weekly…). Nothing shows a cloud account: Proton’s balance, hours and charges, or how many of Instabox’s 5 machines, 10 snapshots and 200 GB are used.

**C. Both** (from A and B): A’s header line in Machines, and B’s cards on the Usage page.
