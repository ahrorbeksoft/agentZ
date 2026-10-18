# Cloud machines: plan

Status: researched, nothing built. The UI goes on the design board first (this folder's
`index.html`); its `decisions.md` will be the spec. Researched on 2026-10-18 from Instabox's
docs (bundled in its site's script), its SDK (`@insforge/instabox-sdk` 0.5.3) and its CLI's help
(read from the `instabox-darwin-arm64` binary, not run), and Proton's site, installer and CLI
(`proton` 0.1.2, a Python file). No provider was used through agentZ: nothing was signed in to
or made. `proton list` was run once, read-only. Anything not tested is marked *(unverified)*.

## What the user asked for

- **Cloud providers in agentZ**, starting with [Instabox](https://instabox.sh) and
  [Proton](https://proton.computer), and more after them, so a framework rather than two
  features.
- **Everything each provider offers**, not only connecting.
- **The providers' own CLIs.** The user installs a provider's CLI and signs in with it; agentZ
  finds it and works through it. agentZ has no sign-in of its own and keeps no provider
  credentials.
- **Providers that aren't set up are shown too**, with a Learn More link, and with agentZ's
  promo, invite or referral code once a provider offers one.
- **Agents run on the cloud machine.** Keeping agents on this Mac and sending only their tools to
  the cloud doesn't sandbox anything: whatever the agent runs (`python`, `node`, `sed`, a test
  run) runs where its process does, and Claude Agent runs all its tools itself (it uses neither
  ACP's file system nor its terminals). The cloud machine is a machine like `devbox1`, with
  its own server, agents and logins.

## How it works

### A provider gives a machine and a way in

Everything agentZ does on a machine reached over SSH (uploading `agentz-server`, `proxy`,
reconnecting, handoff, updates, browser logins through forwards) stays as it is. A provider
only has to give agentZ a running machine and SSH options for it, and to show and change what
the provider knows about the machine: its state, size, cost and the provider's own features. A
cloud machine that's connected is an SSH machine.

### Through the provider's CLI

Each provider is described by its CLI: the program's name, how to install it, how to tell
whether it's signed in and as whom, and the command for each thing it can do, with how to read
its output. agentZ runs those commands; it never reads the CLI's login.

- **Finding it.** This Mac's server looks the program up on the login shell's `PATH` (it loads
  that environment already) when it starts, when Settings › Machines opens, and every few
  minutes while it's open. A provider is then one of:
  - **Not installed**: shown with what it is, its price, Learn More, the install command to
    copy, and agentZ's code when there is one.
  - **Signed out**: the CLI is there (`--version` answers) but `whoami` fails. agentZ offers
    Sign In, which opens a terminal running the CLI's login (`instabox login`, `proton login`),
    as agents' terminal logins run.
  - **Signed in**: `whoami` names the account; its machines are listed.
- **Commands.** Run with the login shell's environment, in the background, one at a time per
  machine (both CLIs wait on their own operations). Output is read as JSON where the CLI has
  `--json`, otherwise as text, with captured real outputs as test fixtures so a new CLI version
  that changes its output fails a test instead of showing wrong numbers (as the agent accounts'
  readers do). A command that fails shows its message.
- **SSH options from the CLI**: `instabox ssh-config <machine>` and `proton ssh-config <name>`
  print a Host block; agentZ passes its options to `ssh` (`-o HostName=…` and so on) and never
  edits `~/.ssh/config`. Instabox's certificate lasts a short time, so agentZ runs `instabox
  ssh-cert <machine>` before each connection and before it runs out. Proton's computers take the
  user's own key, given when the computer is made (`~/.ssh/id_ed25519.pub` by default), and its
  CLI pins the host key it reports in `~/.config/proton/known_hosts`.
- **Where it runs.** This Mac's server runs the CLIs and keeps what it learned in
  `cloud.json` (the providers' last state and the machines' last listing), so a certificate is
  renewed and an idle machine handled with the app closed. A saved machine (`MachineProfile`)
  gets a source: `Ssh { target }` as today, or `Cloud { provider, machine }`.
  `agentz_client::ssh::Ssh` takes the SSH options where it takes a target string today.
- **States.** The machine's status learns the provider's states. A stopped or paused machine
  isn't "Reconnecting": the app doesn't dial it until it runs, and says what it is.
- **An SSH machine already saved** can be linked to the provider machine with the same host and
  port: `proton-cloud` (`proton`, `ssh.proton.computer:22021`) is the Proton computer
  `devbox-3`.

### Learn More and codes

A provider's description has its site, a Learn More page, the install command, a one-line pitch
and its price. It can also carry agentZ's code: a promo, invite or referral code with its words
("Invite code 482913: new accounts need one") and, where the provider takes one, how to pass it
(a link parameter, or `instabox login --invite-code <code>`). Neither provider has a referral
program today. Instabox does need an **invite code** to activate a new account (6 digits, from
its admins; `login --invite-code`), so a code InsForge gives agentZ would go there. The code
lives in the description, so a new one ships with an update.

### The framework

```rust
struct ProviderDescription {
    id: &'static str,                  // "instabox", "proton"
    name: &'static str,
    program: &'static str,             // "instabox", "proton"
    install: &'static str,             // the command shown to copy
    site: &'static str,
    learn_more: &'static str,
    pitch: &'static str,
    price: &'static str,
    code: Option<ProviderCode>,        // agentZ's promo, invite or referral code, when there is one
    capabilities: Capabilities,        // pause, snapshots, ports, vault, tokens, billing, …
}

trait Provider {
    fn description(&self) -> &ProviderDescription;
    async fn status(&self) -> ProviderStatus;             // NotInstalled, SignedOut, SignedIn { account }
    fn sign_in_command(&self) -> Vec<String>;             // run in a terminal
    async fn machines(&self) -> Result<Vec<CloudMachine>>; // id, name, state, size
    async fn create(&self, options: CreateOptions) -> Result<CloudMachine>;
    async fn change(&self, machine: &str, change: Lifecycle) -> Result<()>;
    async fn ssh_options(&self, machine: &str) -> Result<SshOptions>;
    async fn usage(&self) -> Result<Usage>;              // balance and hours, or counts and limits
    // Optional parts, by capability: snapshots, ports, vault, ssh_tokens.
}
```

`Lifecycle` is `Start`, `Stop`, `Restart`, `Pause`, `Resume`, `Delete`; a provider lists the
ones it has. The UI shows only what a provider supports, so a third provider adds a module and
a description, not UI.

## Instabox

InsForge's "Cloud Computers for AI Agents", free for now ("Free until we figure it out"): 5
machines, 10 snapshots, 200 GB stored; its CLI also shows "Free credit left" and this month's
usage. Each machine is a Linux VM (x86_64) with its own kernel, root and Docker: 4 vCPUs, 8 GB,
a 20 GB `/data` disk (home is `/data/home`, so `~/.agentz` lives across stops). Its CLI
(`instabox`, a native binary; `curl -fsSL https://api.instabox.sh/install.sh | sh` or `npx
instabox@latest`) signs in through the browser (GitHub or Google, `--device` without a
browser) and keeps its login in a `credentials.json` in its config folder.

| What it offers | Its CLI | In agentZ |
|---|---|---|
| Sign in | `login [--device] [--provider github\|google] [--invite-code CODE]`, `whoami`, `logout` | Detected; Sign In runs `instabox login` in a terminal |
| Templates | `templates --json` (`agent-browser`, `universal`, `herdr`, `orca`) | Create |
| Machines | `ls --json`, `get <m> --json`, `create --name N [--template T \| --snapshot S] [--timeout D --on-timeout pause\|kill] [--auto-suspend D\|0] [--meta K=V] [--no-vault]`, `rm` | List, create, delete |
| Lifecycle | `start`, `stop`, `suspend`, `resume`, `wake` | Start, Stop, Pause, Resume |
| Timeout | `timeout <m> <d>\|0 [--on-timeout pause\|kill]` | Create, machine settings |
| Idle pause | `auto-suspend <m> <d>\|0` (idle is "no agent working, no terminal use, little CPU"; 30 min by default; may need manual lifecycle on the account *(unverified)*) | Machine settings; see Idle |
| Labels | `--meta K=V` on create; `ls --meta` | Create *(maybe not)* |
| SSH | `ssh-config <m>`, `ssh-cert <m>` | agentZ's connection |
| SSH tokens | `ssh-token <m> [--ttl 7d] --json`, `ssh-token --revoke <m>` | Copy SSH Command, for the user's own terminal; Revoke All |
| Snapshots | `snapshot save <m> --name N [--root-only] --json`, `snapshot ls --json`, `snapshot files`, `snapshot rm` | Save, list, delete, start a machine from one |
| Public ports | `port publish <m> <port> [--name] --json`, `port ls --json`, `port unpublish` | Publish, list, unpublish |
| Vault | `vault set`, `set-file`, `import`, `ls --json`, `rm`, `show <m> --json`, `push <m> --json`; `--no-vault` | Per machine: what it gets, push; the items stay the CLI's |
| Usage | `usage --json` (counts, storage, limits), `org get <org> --json` (plan, this month's usage, credit) | The account's header |
| Desktop | **No CLI command.** The API has `…/desktop` and `…/desktop/stream` (view-only or control links) | See open questions |
| Metrics | **No CLI command.** The API has `…/metrics` (CPU and memory every 30 s) and per-machine usage | See open questions |
| Commands and files | `exec`, `files` | Not needed: agentZ has its own server there |

Catches:

- **Idle pause.** A machine pauses after 30 idle minutes ("no agent working, no terminal use,
  little CPU"). Paused mid-turn, its agents' connections to their APIs drop. SSH, commands and
  a visit to a port resume a machine that paused for being idle, so the app's reconnecting
  wakes it.
- **Telemetry.** Its CLI sends the names of the commands it runs (never their arguments) to
  InsForge unless `INSTABOX_NO_TELEMETRY=1`. agentZ would run it often; it leaves that to the
  user's environment.
- **20 GB.** Enough for most projects, not for big Rust builds.
- **5 machines in any state**, paused and stopped ones included.

## Proton

Collaborators Inc.'s "One Proton. $20 a month.", billed by the running second from the "Cube"
balance it shares with their other product ($20 is 730 running hours; a stopped computer costs
nothing; with no balance left, computers are suspended and their disks kept). Each computer:
4 shared vCPUs (the landing page also says 8), 2 GB of memory guaranteed and up to 64 GB as the
host allows, a 50 GB disk, Ubuntu 24.04 with Node, Bun, Python, Docker, gh, Claude Code, Codex,
Chromium and Playwright. Its CLI (`proton`, one Python file needing Python 3.8+; `curl -fsSL
https://app.proton.computer/install.sh | sh`) signs in with a code approved in the browser and
keeps its token in `~/.config/proton/credentials.json`. It's installed and signed in on this
Mac.

| What it offers | Its CLI | In agentZ |
|---|---|---|
| Sign in | `login [--no-browser]`, `whoami`, `logout` | Detected; Sign In runs `proton login` in a terminal |
| Machines | `list --json`, `show <name> --json` (with its last operations), `create <name> [--key FILE] --yes` | List, create |
| Lifecycle | `start`, `stop`, `restart` (`--yes` skips its price question), `delete <name> --yes` | Start, Stop, Restart, Delete |
| SSH | `ssh-config <name>` (host `ssh.proton.computer`, its port, user `ubuntu`, the pinned host key) | agentZ's connection |
| Keys | `keys <name> --key FILE…` *replaces* the keys of a running computer | SSH Keys…, sent with every key to keep |
| Billing | `balance` (text: the Cube balance, hours running, amount charged) | The account's header; Add Credit opens its billing page |

Catches:

- **It costs money while it runs.** Starting or creating one is a purchase: Proton's own CLI
  asks first ("This computer costs $20 per 730 running hours… Continue?"); agentZ asks in its
  own UI, then runs the command with `--yes`.
- **Its output may change**: the CLI is at 0.1.2, and `balance` has no `--json`.
- **`keys` replaces keys**, so changing them must send every key to keep.

## Decided

- **The user's own CLIs, signed in by the user** (the user's choice). agentZ detects them, runs
  them, and keeps no credentials; it never reads their logins.
- **Providers not set up are listed** with Learn More, their install command, and agentZ's code
  once a provider offers one (the user's choice).
- **Agents' logins on a cloud machine work as on any SSH machine**: logged in there, through
  the browser logins agentZ already forwards. Copying logins from this Mac is left out: two
  copies of a login whose refresh token works once log each other out.

## Open questions

For the user:

- **What only Instabox's API has**: desktop links and CPU and memory. Leave them out, or call
  the API with the CLI's stored login for just those (agentZ would read `credentials.json`,
  which it otherwise doesn't)?

On the design board:

1. Where providers live: in Settings › Machines, or a section of their own.
2. Providers that aren't set up, and signing in.
3. Creating a machine: provider, template or snapshot, name, size, timeout; what it costs.
4. Starting something that costs money.
5. A cloud machine's row: its state and actions.
6. A machine's page: snapshots, ports, desktop, vault, SSH tokens, timeout.
7. Instabox snapshots.
8. Deleting a machine (its disk and threads go with it).
9. Machines an account already has, and linking an SSH machine already saved.
10. A stopped or paused machine elsewhere (sidebar, New Thread, a thread on it).
11. Idle: keep Instabox from pausing while agents work; stop or pause a machine whose agents have
    all been idle a while; balance warnings.
12. Cost and usage: Proton's balance, Instabox's limits.

## Order of work

1. The `cloud` crate: descriptions, detection, and both providers' commands and parsers against
   captured outputs; SSH options in `agentz_client::ssh`.
2. Providers in this Mac's server, and the protocol.
3. Cloud machines connecting, with their states.
4. The UI picked on the board.
5. Each provider's extras: snapshots, ports, vault, tokens, billing.
6. Idle handling, as picked.
