# Cloud Machines: design decisions

Picked on the design board (`design/cloud/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Where providers live in Settings: Not decided yet

*Settings*

**Today:** Settings › Machines lists This Mac and the machines saved by hand, each reached over SSH: its icon, its name over “SSH devbox1 · Connected”, then Edit, Remove and its switch. Add Machine asks for an SSH target and a name. A cloud computer (proton-cloud is a Proton computer) is just an SSH target: agentZ doesn’t know it’s Proton’s, can’t start or stop it, and doesn’t know what it costs.

## 2. Signing in to a provider: Not decided yet

*Settings*

**Today:** There’s nothing to sign in to: agentZ reaches a cloud computer as an SSH target, with the key in `~/.ssh`. Instabox signs programs in with an API key (`ibk_…`); its CLI signs people in through the browser. Proton has no API keys: its CLI signs in with a code it shows, approved in the browser (`proton login`), and keeps the token in `~/.config/proton/credentials.json`. You’re signed in there already.

## 3. Adding a cloud machine: Not decided yet

*Machines*

**Today:** Add Machine is one dialog: an SSH target (“user@host, or a Host from ~/.ssh/config”) and a name, with “agentZ installs its server in ~/.agentz there, and nothing else.” A cloud machine has to be made on the vendor’s site or CLI first, then added by its SSH host.

## 4. Starting something that costs money: Not decided yet

*Machines*

**Today:** Nothing in agentZ costs money by itself. Proton bills $20 per 730 running hours from the moment a computer is created or started; its CLI asks first: “This computer costs $20 per 730 running hours (about one month), billed per running second from your shared Cube balance. Stopping it stops the charge. Continue?” (skipped with `--yes`). Instabox is free during its beta.

## 5. A cloud machine’s row: Not decided yet

*Machines*

**Today:** A row is the machine’s icon, its name over “SSH proton · Connected” (or Connecting, Uploading agentz-server · 40%, Reconnecting: the error), then Update Server when there’s one, Retry Now while it’s offline, Edit, Remove and its switch. A stopped Proton computer reads “Reconnecting” with an SSH error.

## 6. Everything else a provider offers: Not decided yet

*Machines*

**Today:** A saved machine has only Edit (its target and name) and Remove. Instabox also has snapshots, public ports, a desktop with share links, a Vault of secrets, SSH tokens for other terminals, CPU and memory samples, running time, timeouts, idle pause and labels. Proton has its SSH keys and billing. Today they’re all on the vendors’ dashboards and CLIs.

## 7. Instabox snapshots: Not decided yet

*Machines*

**Today:** Not in agentZ. An Instabox snapshot saves a machine’s software and its `/data` (home, workspace, saved sign-ins), or its software only; new machines start from it. Snapshots belong to the account (10 of them), not to the machine they came from, and stay when it’s deleted. Saving stops a running machine first, unless the server can save it while it runs.

## 8. Removing or deleting a cloud machine: Not decided yet

*Machines*

**Today:** Remove… takes a machine out of agentZ; the machine itself and agentZ’s server there stay. Proton’s own Delete asks you to type the computer’s name (“This permanently deletes this computer and its disk. Its files cannot be recovered.”).

## 9. Machines an account already has: Not decided yet

*Machines*

**Today:** proton-cloud is saved as the SSH target `proton`, an alias in `~/.ssh/config` for `ssh.proton.computer:22021`: that’s the Proton computer devbox-3. Machines made on a vendor’s site or CLI aren’t in agentZ until they’re added by hand.

## 10. Opening something on a stopped machine: Not decided yet

*Everywhere*

**Today:** A machine that’s unreachable shows as offline everywhere: dimmed, its input disabled, “Reconnecting” with an SSH error, and agentZ keeps retrying. A Proton computer that’s stopped stays that way until someone starts it on Proton’s site. An Instabox machine that paused for being idle wakes by itself when agentZ reconnects over SSH; one paused by hand or by its timeout waits for Resume.

## 11. Idle machines and running costs: Not decided yet

*Everywhere · pick any*

**Today:** agentZ keeps a machine awake while its agents work (`caffeinate`, `systemd-inhibit`), and stops idle agents after 30 minutes. Cloud providers have their own rules: an Instabox machine pauses after 30 idle minutes (what counts as idle isn’t documented), and changing that needs “manual lifecycle” on the account; a Proton computer runs, and bills, until it’s stopped, and is suspended when the balance runs out.

## 12. Cost, usage and limits: Not decided yet

*Everywhere*

**Today:** Settings › Usage shows each agent account’s limits (5-hour, weekly…). Nothing shows a cloud account: Proton’s balance, hours and charges, or how many of Instabox’s 5 machines, 10 snapshots and 200 GB are used.
