# Cloud machines: plan

> **Instabox was dropped** (the user, 2026-10-19): its account pauses machines after 30 idle
> minutes whatever they ask, so agentZ supports Proton only. What only Instabox had isn't
> built: topic 8 (snapshots), 12 A (keeping it awake), its tabs in topic 7 (Snapshots, Ports,
> Desktop, Vault, SSH), its templates and timeouts in topic 4, Pause when idle in 12 B and its
> counts in 13. Topic 11 C's free start went with it, and a thread on a stopped computer
> doesn't offer Start either: its offline notice says it's stopped, and it's started from
> Settings › Machines. `progress.md` (item 10) has the details.

Status: designed, being built. The UI was picked on the design board: `decisions.md` is its
spec (topic numbers below are its sections), and `progress.md` tracks the build. Researched on 2026-10-18 from Instabox's
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
  finds it and works through it. agentZ has no sign-in of its own; the one credential it
  keeps is an Instabox API key it makes through the CLI, for what only Instabox's API does.
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
  - **Signed out**: the CLI is there (`--version` answers) but `whoami` fails. agentZ says to
    run its login (`instabox login`, `proton login`) in a terminal, with Copy; it doesn't sign
    anyone in (the user's note on topic 3).
  - **Signed in**: `whoami` names the account; its machines are listed.

  More Providers' header has **Check Again**, which looks for the CLIs at once, so a provider
  set up while Settings is open shows up without waiting.
- **Commands.** Run with the login shell's environment, in the background, one at a time per
  machine (both CLIs wait on their own operations). Output is read as JSON where the CLI has
  `--json`, otherwise as text, with captured real outputs as test fixtures so a new CLI version
  that changes its output fails a test instead of showing wrong numbers (as the agent accounts'
  readers do). A command that fails shows its message.
- **SSH options from the CLI**: `instabox ssh-config <machine>` and `proton ssh-config <name>`
  print a Host block; agentZ passes its options to `ssh` (`-o HostName=…` and so on) and never
  edits `~/.ssh/config`. Instabox's certificate lasts a short time, so agentZ runs `instabox
  ssh-cert <machine>` before each connection and before it runs out. Proton's computers take one
  public key when they're made (its CLI's default is `~/.ssh/id_ed25519.pub`, `id_ecdsa.pub` or
  `id_rsa.pub`, which many Macs don't have): agentZ gives each its own key, made in the data
  folder (`cloud/id_ed25519`) when first needed, keeps it when the keys are replaced, and adds
  it to the SSH options (`IdentityFile`, then `ssh`'s defaults, which the option replaces).
  Its CLI pins the host key it reports in `~/.config/proton/known_hosts`.
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

### Instabox's API, for what its CLI doesn't do

Desktop links and CPU and memory exist only in Instabox's API. agentZ may use it where that
needs nothing from the user (the user's answer), so it makes its own API key once, through the
signed-in CLI: `instabox keys create --name agentZ --scope machines:read --scope machines:write
--json`, Instabox's documented way for programs. The key is kept in `cloud/instabox/api-key` in
the data directory (mode 0600) and sent as `X-API-Key`. When it stops working (revoked, or the
CLI signed in to another account), agentZ makes a new one and removes the old with `instabox
keys rm`. It never reads the CLI's own login.

### Threads of a machine that's off

Today the app keeps each machine's projects and threads in memory only: an offline machine's
threads stay listed while the app runs, and after a restart a machine that's off shows nothing
until it connects. So a stopped cloud computer's threads would disappear.

- **This Mac keeps the list** of every remote machine (cloud and SSH): its projects and threads
  (title, agent, state, last activity), written each time it changes, and shown while the
  machine is off. Only the list, not the conversations (the user's answer): opening a thread
  of a machine that's off says so until it connects.
- **A message to a thread of a stopped machine** is held on this Mac (across restarts) and
  shown in the thread as "Sent when proton-cloud is running", with Cancel. agentZ starts the
  machine at once when that's free (Instabox), or after topic 5's question when it costs money
  (Proton), and sends the message once it's connected (topic 11 C). New Thread on a stopped
  machine works the same way. On an SSH machine, which agentZ can't start, the message waits
  until the machine is back.

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

- **The user's own CLIs, signed in by the user** (the user's choice). agentZ detects them and
  runs them, and never reads their logins. The one credential it keeps is the Instabox API key
  it makes through the CLI (see Instabox's API above).
- **Providers not set up are listed** with Learn More, their install command, and agentZ's code
  once a provider offers one (the user's choice).
- **Agents' logins on a cloud machine work as on any SSH machine**: logged in there, through
  the browser logins agentZ already forwards. Copying logins from this Mac is left out: two
  copies of a login whose refresh token works once log each other out.
- **No sign-in in agentZ** (the user's note on topic 3): a signed-out CLI shows the command to
  run, and Check Again looks again.
- **Desktop links and CPU and memory through Instabox's API**, with a key agentZ makes through
  the CLI (see Instabox's API above).
- **Only the list of a machine that's off** is kept on this Mac, not its conversations; a
  message to it is held and starts the machine (see Threads of a machine that's off).
- **The board's picks** (`decisions.md`): 1 C machines grouped by provider; 2 A providers not
  set up as rows under the groups; 4 C a menu on Add Machine; 5 C ask before charging the first
  time per account; 6 B a state badge, one action and a menu; 7 A a page per machine with tabs;
  8 A snapshots on the machine they came from; 9 B one Remove with a box to delete; 10 B all
  of an account's machines, always; 11 C start free ones, ask for paid ones; 12 A, B, C and D
  for idle machines; 13 C cost in the group headers and on the Usage page.

Nothing is open.

## Order of work

`progress.md` breaks these into commits and tracks them.

1. The `cloud` crate: descriptions, detection, and both providers' commands and parsers against
   captured outputs.
2. SSH options in `agentz_client::ssh`, so a machine can be reached without `~/.ssh/config`.
3. Providers in this Mac's server, and the protocol.
4. Cloud machines connecting, with their states, and the Machines page (topics 1, 2, 3, 6, 10,
   13 A).
5. Adding, starting and removing (topics 4, 5, 9, 11's start).
6. The machine's page and each provider's extras (topics 7, 8), with Instabox's API.
7. The list of a machine that's off, and held messages (topic 11).
8. Idle handling (topic 12) and the Usage page (topic 13 B).
