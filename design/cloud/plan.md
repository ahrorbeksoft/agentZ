# Cloud machines: plan

Status: researched, nothing built. The UI goes on the design board first (this folder's
`index.html`); its `decisions.md` will be the spec. Researched on 2026-10-18 from Instabox's
docs (bundled in its site's script), its SDK (`@insforge/instabox-sdk` 0.5.3) and installer,
and Proton's site, installer and CLI (`proton` 0.1.2, a Python file). Neither provider was
used: no account was signed in, no machine was made. Anything not tested is marked
*(unverified)*.

## What the user asked for

- **Cloud providers in agentZ**, starting with [Instabox](https://instabox.sh) and
  [Proton](https://proton.computer), and more after them, so a framework rather than two
  features.
- **Everything each provider offers**, not only connecting.
- **Agents run on the cloud machine.** Keeping agents on this Mac and sending only their tools to
  the cloud doesn't sandbox anything: whatever the agent runs (`python`, `node`, `sed`, a test
  run) runs where its process does, and Claude Agent runs all its tools itself (it uses neither
  ACP's file system nor its terminals). The cloud machine is a machine like `devbox1`, with
  its own server, agents and logins.

## How it works

### A provider gives a machine and a way in

Everything agentZ does on a machine reached over SSH (uploading `agentz-server`, `proxy`,
reconnecting, handoff, updates, browser logins through forwards) stays as it is. A provider
only has to give agentZ a running machine and an SSH endpoint for it, and to show and change
what the provider knows about the machine: its state, size, cost and the provider's own
features. A cloud machine that's connected is an SSH machine.

Both providers have the same shape, as most do (Hetzner, DigitalOcean, Fly, Daytona
*(unverified)*): an account, machines, and slow changes answered with an **operation** that is
polled until it succeeds or fails, sent with an `Idempotency-Key` so a retry doesn't make a
second machine.

### Where it runs

The providers are a plain Rust crate, `cloud` (no GPUI, `http_client` like the accounts'
readers), one module per provider, tested against recorded answers. **This Mac's server runs
them** and owns the provider accounts, as it owns agent accounts: work that has to go on with
the app closed (an Instabox certificate renewed before it runs out, a machine kept from pausing
while its agents work, an idle machine stopped) needs a process that stays. The app shows them
through the protocol, like agent accounts.

- `cloud.json` in the data directory: the provider accounts (id, provider, label, the last
  identity and usage read). Each account's credential is a file of its own under
  `cloud/<account id>/`, mode 0600, as API-key accounts keep their keys.
- agentZ's own SSH key per account, `cloud/<account id>/id_ed25519` (made with `ssh-keygen`),
  and the host keys the providers report in `cloud/<account id>/known_hosts`. agentZ never
  edits `~/.ssh/config`.
- A saved machine (`MachineProfile`) gets a source: `Ssh { target }` as today, or
  `Cloud { account, machine }`. A cloud machine's SSH options come from the provider each time
  it connects: `-p`, `-l`, `-i`, `CertificateFile`, `UserKnownHostsFile`, `IdentitiesOnly`.
  `agentz_client::ssh::Ssh` takes such an endpoint where it takes a target string today.
- The machine's status learns the provider's states. A stopped or paused machine isn't
  "Reconnecting": the app doesn't dial it until it runs, and says what it is.
- An SSH machine already saved can be linked to the provider machine with the same host and
  port: `proton-cloud` (`proton`, `ssh.proton.computer:22021`) is the Proton computer
  `devbox-3`.

### The framework

```rust
trait Provider {
    fn description(&self) -> &ProviderDescription; // name, icon, site, sign-in kind, capabilities
    async fn sign_in(&self, …) -> Result<Credential>;
    async fn sign_out(&self, credential: &Credential) -> Result<()>;
    async fn identity(&self) -> Result<Identity>;
    async fn machines(&self) -> Result<Vec<CloudMachine>>;  // id, name, state, size, created
    async fn create(&self, options: CreateOptions) -> Result<Operation>;
    async fn change(&self, machine: &str, change: Lifecycle) -> Result<Operation>; // start, stop, …
    async fn operation(&self, id: &str) -> Result<Operation>;
    async fn ssh_endpoint(&self, machine: &str) -> Result<SshEndpoint>;
    async fn usage(&self) -> Result<Usage>;   // balance and hours, or counts and limits
    // Optional parts, by capability:
    fn snapshots(&self) -> Option<&dyn Snapshots>;
    fn ports(&self) -> Option<&dyn PublicPorts>;
    fn desktop(&self) -> Option<&dyn Desktop>;
    fn vault(&self) -> Option<&dyn Vault>;
    fn ssh_tokens(&self) -> Option<&dyn SshTokens>;
    fn metrics(&self) -> Option<&dyn Metrics>;
}
```

`Lifecycle` is `Start`, `Stop`, `Restart`, `Pause`, `Resume`, `Delete`; a provider lists the
ones it has. The UI shows only what a provider supports, so a third provider adds a module and
a description, not UI.

## Instabox

InsForge's "Cloud Computers for AI Agents", free for now ("Free until we figure it out"): 5
machines, 10 snapshots, 200 GB stored. Each machine is a Linux VM (x86_64) with its own kernel,
root and Docker: 4 vCPUs, 8 GB, a 20 GB `/data` disk (home is `/data/home`, so `~/.agentz`
lives across stops). API at `https://api.instabox.sh`, key in `X-API-Key`.

| What it offers | Its API | In agentZ |
|---|---|---|
| Sign in | API key (`ibk_…`, scopes `machines:*`, `images:*`), made on its dashboard or with `instabox keys create`. Its CLI also signs in through the browser *(endpoint unverified)* | Paste a key; a link to its API Keys page |
| Who | `GET /v1/me` | The account's row |
| Templates | `GET /v1/templates` (`agent-browser`, `universal`, `herdr`, `orca`) | Create |
| Machines | `GET/POST /v1/machines`, `GET/PATCH/DELETE /v1/machines/{id}` | List, create, rename, delete |
| Lifecycle | `POST …/start`, `/stop`, `/suspend`, `/resume`; `GET /v1/operations/{id}?wait=25s` (long poll) | Start, Stop, Pause, Resume |
| Timeout | `timeoutSeconds` and `onTimeout` (`pause` or `kill`), 1 min to 24 h (4 h without "manual lifecycle") | Create, machine settings |
| Idle pause | `autoSuspendAfterSeconds` (30 min; changeable only with manual lifecycle, an admin setting) | Machine settings; see Idle |
| Labels | `metadata`, 32 per machine | Machine settings *(maybe not)* |
| SSH | `POST …/connections` with a public key: a certificate (1 s to 24 h), host, port, user (the machine id) and a `@cert-authority` known_hosts line. The gateway does sessions, exec, env, signals and local forwards; no agent forwarding, X11 or remote forwards | agentZ's key, a fresh certificate before each connection and before it runs out |
| SSH tokens | `GET/POST/DELETE …/ssh-tokens` (`ssh -p 2222 ibssh_…@host`, 7 days by default, no key) | Copy SSH Command, for the user's own terminal; Revoke All |
| Snapshots | `POST …/snapshots` (`rootOnly` for software only), `GET /v1/snapshots`, `GET/DELETE /v1/snapshots/{id}`, `…/files` | Save, list, delete, start a machine from one |
| Public ports | `GET/POST …/ports`, `DELETE …/ports/{port}`, `GET /v1/ports` (`https://<port>-<id>.<domain>/`, up to 16; a visit wakes an idle-paused machine) | Publish, list, unpublish |
| Desktop | `POST …/desktop` (start), `…/desktop/stream` (a link, view-only or not, 1 min to 24 h; `DELETE` revokes them all) | Open Desktop in the browser |
| Vault | Account items (env and files) written into every machine at boot: CLI only *(no documented endpoint)*; per machine `…/vault` (what it gets, push now, keep it out) | Per machine: on or off, push |
| Metrics | `GET …/metrics` (CPU and memory, every 30 s) | The machine's page |
| Usage | `GET …/usage` (running seconds this month), `GET /v1/usage` (counts, storage, limits) | The account's row and the machine's page |
| Commands and files | `POST …/exec`, `GET …/files` | Not needed: agentZ has its own server there |

Catches:

- **Idle pause.** A machine pauses after 30 idle minutes, and what counts as idle isn't
  documented. Paused mid-turn, its agents' connections to their APIs drop. Turning it off needs
  "manual lifecycle", an admin setting, though the Concepts page says `instabox auto-suspend
  <machine> 0` does it. SSH, commands and a visit to a port resume a machine that paused for
  being idle, so the app's reconnecting wakes it.
- **20 GB.** Enough for most projects, not for big Rust builds.
- **5 machines in any state**, paused and stopped ones included.

## Proton

Collaborators Inc.'s "One Proton. $20 a month.", billed by the running second from the "Cube"
balance it shares with their other product ($20 is 730 running hours; a stopped computer costs
nothing; with no balance left, computers are suspended and their disks kept). Each computer:
4 shared vCPUs (the landing page also says 8), 2 GB of memory guaranteed and up to 64 GB as the
host allows, a 50 GB disk, Ubuntu 24.04 with Node, Bun, Python, Docker, gh, Claude Code, Codex,
Chromium and Playwright. API at `https://app.proton.computer/api`, undocumented: what its CLI
sends is all there is.

| What it offers | Its API (from the CLI) | In agentZ |
|---|---|---|
| Sign in | A device login: `POST /cli/device {client}` gives a code and a link, then `POST /cli/token {device_code}` until `approved` gives a bearer token; `POST /cli/logout` revokes it | Sign In opens the page; the code shown to compare |
| Who | `GET /me` | The account's row |
| Machines | `GET /machines`, `GET /machines/{id}` (with its last operations), `POST /machines {name, ssh_keys, billing_accepted}` | List, create |
| Lifecycle | `POST /machines/{id}/start`, `/stop`, `/restart` (start and restart with `billing_accepted`), `/delete {confirm_name, purge_disk}`; `GET /operations/{id}` | Start, Stop, Restart, Delete |
| SSH | `public_ssh`: host `ssh.proton.computer`, its port, `host_key`, fingerprint; user `ubuntu`; the keys given at creation | agentZ's key at creation, the host key pinned |
| Keys | `POST /machines/{id}/authorize-key {ssh_keys}` *replaces* the keys of a running computer | Add agentZ's key to a linked computer, sent with the user's keys so theirs stay |
| Billing | `GET /billing`: balance, runtime, amount charged, whether billing is on | The account's row; Add Credit opens its billing page |

Catches:

- **It costs money while it runs.** Starting or creating one is a purchase: Proton's own CLI
  asks first ("This computer costs $20 per 730 running hours… Continue?").
- **The API may change** without notice; its CLI is at 0.1.2.
- **`authorize-key` replaces keys**, so adding agentZ's key must send the user's keys too, or
  the user's own `ssh proton` stops working.

## Decided here (the board can change them)

- **Built in over the vendors' APIs, not their CLIs.** Most of Instabox's features exist only in
  its API, Proton's CLI is a thin wrapper over its API, and nothing has to be installed.
- **agentZ signs in itself**: Instabox with an API key, Proton with its device login. It doesn't
  read the vendors' CLI logins (`~/.config/proton/credentials.json`).
- **Agents' logins on a cloud machine work as on any SSH machine**: logged in there, through
  the browser logins agentZ already forwards. Copying logins from this Mac is left out: two
  copies of a login whose refresh token works once log each other out.

## Open questions (the board's topics)

1. Where providers live: in Settings › Machines, or a section of their own.
2. Signing in to each provider.
3. Creating a machine: provider, template or snapshot, name, size, timeout; what it costs.
4. A cloud machine's row: its state and actions.
5. A stopped or paused machine elsewhere (sidebar, New Thread, a thread on it): start it when
   it's opened, or ask.
6. Idle: keep Instabox from pausing while agents work; stop or pause a machine whose agents have
   all been idle a while, to save money.
7. A machine's page: snapshots, ports, desktop, vault, SSH tokens, metrics, timeout, labels.
8. Cost and usage: Proton's balance, Instabox's limits.
9. Deleting a machine (its disk and threads go with it).
10. Linking an SSH machine already saved.

## Order of work

1. The `cloud` crate with the framework and both providers against recorded answers; endpoints
   in `agentz_client::ssh`.
2. Provider accounts in this Mac's server, and the protocol.
3. Cloud machines connecting, with their states.
4. The UI picked on the board.
5. Each provider's extras: snapshots, ports, desktop, vault, tokens, metrics, billing.
6. Idle handling, as picked.
