# Accounts: plan

Status: designed, nothing built. The UI was picked on the design board: `decisions.md` is its
spec (§ numbers below are its sections), and `progress.md` tracks the build. The research was
done on 2026-10-06 against the ACP Registry of
that day (41 agents). Factory Droid 0.234.0, Claude Code, Codex and Devin 3000.11.3 were tested
in depth on real logins (read-only, no prompts). The other agents were tested in throwaway
homes, or read from their source and docs. Anything not tested is marked *(unverified)*.

## What the user asked for

- **Several accounts per agent.** More than one login to the same agent (two Factory accounts,
  two Claude subscriptions, …). Accounts share nothing: each has its own login, its own
  sessions and its own history. A thread can't move between accounts.
- **Accounts run side by side.** Threads on different accounts of the same agent run at once.
- **Quota per account.** Each account shows its plan's usage windows (5-hour, weekly,
  monthly, credits, …): how much is used and when each resets. agentZ reads it from the agent,
  using the agent's own terminal commands where ACP has nothing.
- **Settings per account.** Accounts of the same agent can offer different models, so each
  account has its own defaults for new threads, its own environment variables and its own list
  of models and options.
- **Who the account is.** The account's email (or name) and plan are read automatically.
- **Switching.** The user picks the account for a new thread. There is no automatic switching
  (t3code has none either).
- **One generic account manager.** It works for any agent that can keep its login in a folder
  agentZ chooses. Adding an agent means describing it, not writing a new feature.
- **Skills and MCP servers managed in agentZ**, loaded by every agent and every account unless
  kept to some accounts (§20). The agents' own skills and MCP servers stay as they are.
- **Every agent where it makes sense**, mostly the ones with subscriptions.

## Which agents it makes sense for

A second account is worth having when a login carries its own allowance: a subscription with
usage windows (Claude, ChatGPT, Factory, Devin, Grok, Copilot, Cursor, …), monthly
credits (Kilo Pass, Augment, Amp), or a vendor's coding plan sold as a key with windows (GLM,
MiniMax, Kimi). A pay-per-token API key doesn't need it: a second key just bills the same way.

Accounts also work for agents with no quota to show. The home folder, login, skills and MCP
parts are the same; the quota part is optional.

## How it works

### The home folder

Every supported agent has an environment variable (or a few) that moves its config, login and
sessions into another folder. Each account is one such folder:

```
<data dir>/accounts/<agent id>/<account id>/
```

There are two kinds of account (the user's decision):

- **External:** the login the user made outside agentZ, in the agent's own CLI or app. It uses
  the agent's normal home (no variables set), and it's only listed while the agent is logged
  in there. agentZ runs the agent's login check against the normal home when the server starts,
  when the agent's settings open, and with each quota refresh. Threads on it put their sessions
  in the CLI's history, like today.
- **agentZ accounts:** every login made from agentZ goes into a new home under the data
  directory. They start empty and log in through agentZ's existing login flow, run inside that
  home. If the agent isn't logged in outside agentZ, agentZ never uses its normal home, so
  agentZ's sessions never fill the CLI's or app's history.

New threads start on the account marked Use for New Threads (§5), else the External account
when it's listed, else the first agentZ account. The strip under the composer changes it (§12).
agentZ doesn't share the external login with its own homes: accounts share nothing, so a login
is never linked or copied from one home to another. (t3code's Codex "shadow home" does the
opposite: extra accounts share the main home's sessions and keep only `auth.json` separate.)

Existing threads ran in the normal home, so they stay on the External account. If that login
goes away, they show as logged out, and logging in from one of them logs in the normal home,
because that's where their sessions are. It's the only time agentZ logs in the normal home.

The External account can't be removed; Log Out on it also logs out the CLI, as the confirm
dialog says today. It replaces the "Logged in outside agentZ" note under the Account card, and
`AgentSettings` becomes per account (see Settings per account).

Agents with no home variable (DeepAgents, crow-cli, Agoragentic) and agents without a
description keep today's behavior: one login, in the normal home.

Two Droid daemons with different homes ran at once and stayed fully separate (each asked for
its own pairing code). Claude's keychain entry is named after its folder, so homes don't share
a login either.

What an account's environment needs, per agent:

- **Home variables.** For example `CLAUDE_CONFIG_DIR=<home>`, or Devin's
  `XDG_DATA_HOME=<home>/data` and `XDG_CONFIG_HOME=<home>/config`.
- **File storage switches.** Some agents keep the login in the macOS keychain under a fixed
  name, which every home would share. Most have a switch to keep it in a file in the home
  instead: `AGY_ACP_FORCE_FILE_STORAGE=1` (Antigravity), `AGENT_CLI_CREDENTIAL_STORE=file`
  (Cursor), `VIBE_TEST_DISABLE_KEYRING=1` (Mistral Vibe), `GOOSE_DISABLE_KEYRING` (goose).
- **Login variables removed.** A login in the environment overrides the account's own. The
  user's environment has `GITHUB_TOKEN`, which Copilot, OpenCode and Kilo take as a login. For
  every agentZ account, the server removes the agent's login variables
  (`FACTORY_API_KEY`, `XAI_API_KEY`, `CURSOR_API_KEY`, `GITHUB_TOKEN`, `GH_TOKEN`, …).
- **HOME itself**, for agents with no other variable (Cursor, Auggie, MiniMax Code, Kimchi,
  Codebuddy). This is the weakest kind: the agent's shell commands may then see the account
  folder as their home and miss the user's `.gitconfig`, `.ssh` and `.config`. A moved HOME
  also moves the macOS keychain lookup, and keychain logins break (t3code's Claude driver
  exports only `CLAUDE_CONFIG_DIR` for this reason: with HOME moved, Claude reports "Not
  logged in"). Those agents come last. Each one is tested first for whether it passes the real
  HOME to its shell. If it doesn't, the account folder gets symlinks to the user's `.gitconfig`,
  `.ssh` and `.config` (the user accepted this).

### API-key accounts

Some accounts are just a key: Droid with a Factory API key, GLM, MiniMax with a key, Mistral.
For those, agentZ keeps the key in the account's folder (mode 0600, as the agents keep their
own logins) and passes it in the agent's key variable (`FACTORY_API_KEY`, `Z_AI_API_KEY`, …).
Because agentZ holds the key, it can call the vendor's quota API itself.

### Data

- `accounts.json` in the data directory: for each agent, its accounts in order. Each has an id,
  a label (Rename, §5; the email until then), a color (§13: it tints the agent's icon wherever
  the account's threads show), what happens when a limit is reached (Stop or Continue at reset,
  §11), and the last identity and quota read, with when they were read. Each agent also names
  its default account for new threads (Use for New Threads, §5), if any. New fields get
  `#[serde(default)]`.
- Each thread records its account (`account: Option<AccountId>`, where `None` is External), so
  every existing thread stays on the External account.
- The External account is listed whenever the normal home is logged in. Its color and limit
  setting are kept under its agent in `accounts.json`, so they survive while it isn't listed.
- Accounts are per machine, like logins and agent settings today: the homes live where the
  agent runs.

### Settings per account

Accounts of one agent don't get the same models, so what agentZ keeps per agent today moves to
each account. What decides an account's models, per agent:

- **Claude:** the Default model depends on the account type (Pro, Max, Team, Enterprise, the
  API, Bedrock, …). Organizations restrict models (`availableModels` and organization model
  restrictions) and cap effort per role. Fable bills to usage credits on some plans, and access
  to Opus 4.6 with 1M context depends on the plan
  ([model configuration](https://code.claude.com/docs/en/model-config)).
- **Codex:** models follow the sign-in. With ChatGPT, the workspace, seat and role decide;
  Enterprise admins turn GPT-6 Sol, Luna and Astra on per workspace. With an API key, the key's
  organization and project decide. GPT-5.4 left Codex for ChatGPT sign-ins on August 31, 2026,
  but not for API keys
  ([workspace model availability](https://learn.chatgpt.com/docs/enterprise/workspace-model-availability)).
- **Factory Droid:** the organization's `modelPolicy` allows and blocks models, per user too,
  and its `customModels` add more. The user's own custom models (BYOK) are in the home's
  `settings.json`
  ([enterprise controls](https://docs.factory.ai/enterprise/hierarchical-settings-and-org-control)).
- **Devin:** Enterprise teams restrict models in Team Settings
  ([models](https://docs.devin.ai/cli/models)).

What changes:

- Everything in `AgentSettings` (`agents/settings.json`) becomes per account: the environment
  (the Environment tab), the defaults for new threads (`default_mode` and
  `default_config_options`, the Defaults tab), the options and modes the agent last offered
  (`known_config_options` and `known_modes`, which hold the model list), and the login method
  and identity. Nothing is shared between accounts, as in t3code, where every provider instance
  has its own environment, home and models.
- The External account keeps today's settings, so nothing the user set is lost and existing
  threads keep their defaults.
- A thread takes its account's defaults, and a choice made in a thread becomes that account's
  default (as Zed does per agent). Values the account doesn't offer are skipped, as
  `AgentThread::apply_defaults` does already.
- A new thread's model, mode and other selectors come from its account's session. Changing the
  account opens the session on the other account, as changing the agent does today.
- The environment is built in this order: the server's, without the agent's login variables;
  the account's Environment; then the account's home variables and file storage switches. A
  login variable the user sets in the account's Environment on purpose stays.
- The agent's own settings files are in the home, so they're per account already: Claude's
  `settings.json` (model, permissions, hooks, `env`), Codex's `config.toml` (model, profiles,
  providers), Droid's `settings.json` (custom models, session defaults) and Devin's
  `config.json`. "Copy settings from" (below) copies them too, from the picked account's home
  (the normal home for the External account), but never the login. They can hold API keys
  (Droid's custom models); the user accepted that.
- The agent's Threads tab (importing its sessions) lists one account's sessions, since each
  account has its own.
- Agent control: `agentz_thread_launch` and `delegate_task` list each account with its models
  and modes, take an optional account (the default account otherwise), and check a model
  against that account's options.
- The Defaults, Environment and Threads tabs each get an account menu over them that picks
  whose settings (or sessions) they show, opening on the default account. The model menu lists
  only what that account offers (§6).
- A new account's card has "Copy settings from": an existing account (the default one first)
  or Nothing. Its Environment, defaults and the agent's settings files are copied once; defaults
  it doesn't offer are dropped when its first session lists its models. After that each
  account's settings are its own (§7).

### The server

- `Server::agent_command` (`crates/agentz_server/src/server.rs`) takes the account and builds
  its environment (above). Each (agent, account) pair is its own agent process, so accounts run
  in parallel.
- Login, logout and the agent's settings page work per account. The existing `Account` struct
  and `ConnectionId::Account` (the agent started from Settings to log in or out) are renamed
  to `LoginSession` and `ConnectionId::LoginSession`, freeing the word "account".
- Removing an agentZ account asks first, then deletes its home folder. It doesn't log out first,
  because for some agents (Copilot, Cursor's keychain store) that could end the same login
  elsewhere.

### Agent descriptions

Each supported agent gets a small description in `crates/agentz_server/src/accounts/`:

- **Environment:** home variables, file storage switches, login variables to remove.
- **Skills folders** to link into, relative to the home, and the folders outside the home the
  agent reads anyway (for clash checks).
- **Settings files** that "Copy settings from" copies, relative to the home.
- **Login check:** how to tell the account is logged out.
- **Identity reader** and **quota reader**.

Readers come in seven kinds:

1. **Command:** run a program with the account's env and parse its output, JSON or text
   (`claude auth status --json`, `devin auth status`, `cursor-agent status --format json`,
   `kilo profile --json`, `auggie account status --json`).
2. **JSON-RPC:** start the agent's own server and send requests (`codex app-server` with
   `account/read` and `account/rateLimits/read`; `copilot --headless --stdio` with
   `auth.getStatus` and `account.getQuota`).
3. **ACP extension request** on the account's agent connection (Grok's
   `_x.ai/auth/check_subscription` and `_x.ai/billing`).
4. **Local HTTP server** started by the agent (`kilo serve`, then
   `GET /kilocode/provider-usage`).
5. **Hidden terminal script:** start the agent's terminal UI in a PTY nobody sees, wait for it
   to be ready, type a slash command, press Enter only once the command menu shows the
   expected entry, read the screen, then close it with Esc and quit. The server already runs
   terminals (`alacritty_terminal`), so this reuses them. Only for Droid's `/status` and
   `/limits`: the user allows no other agent's terminal UI to be read (Devin's `/usage` was
   read this way at first). An agent whose quota is only in its terminal UI has no quota
   reader, unless its vendor's API gives it (kind 7).
6. **HTTP with a key agentZ holds** (API-key accounts, above).
7. **HTTP with the agent's stored login:** read the token from where the agent keeps it in the
   account's home, and call the vendor's usage API with it. Only where nothing else gives the
   quota: Devin (`GetUserStatus`), Antigravity (Google's APIs), Cline, OpenCode's provider
   logins, Kimi's API, Cursor's dashboard API. OpenUsage (`references/openusage`) reads most
   of these vendors' APIs: take the requests from it, sent as it sends them, never redirected.
   The user lifted the rule against reading agents' credentials for this.

   agentZ only reads the login; it never changes, refreshes or copies it (the user's choice).
   Many vendors' refresh tokens work once, so a refresh by agentZ would log the agent out,
   and writing the new tokens back would race the agent. A login whose token has expired
   keeps its last numbers until the agent renews it. ACP's `initialize` doesn't renew it; an
   agent renews its token when it uses it, so an empty session (no prompt) may *(check per
   agent)*. API keys (Devin's, Factory's, OpenCode Go's, Z.ai's) don't expire this way.

   The one exception is Antigravity (the user's choice): its ACP server keeps only the
   refresh token, getting a new access token each time it starts, and Google keeps refresh
   tokens as they are. So agentZ gets an access token from it for each read, and writes
   nothing back.

Readers return one shape: `AccountStatus { logged_in, email, name, plan, windows, credits }`,
with each window `{ label, used_percent, resets_at }`.

Parsers keep captured real outputs, with personal data replaced, as test fixtures. A new agent
version that changes its output then fails a test instead of showing wrong numbers.

### Login checks

agentZ treats "a session opened" as "logged in". That holds for most agents: their
`session/new` fails with "Authentication required" when logged out. It doesn't for Claude,
Devin, Kilo, OpenCode, Amp, Cortex Code and GLM Agent, whose sessions open while logged out.
For those, the description names another check:

- **The agent's own status command:** `claude auth status --json`, `devin auth status`,
  `kilo profile --json`, `opencode auth list --format json`.
- **An empty model list** in the session (Devin, Cortex Code).
- **A key agentZ holds** (GLM).

This changes the AGENTS.md pitfall "Stay within ACP for agent status": agentZ also runs the
agent's own status commands, and reads a stored login where that's the only quota source
(reader kind 7). AGENTS.md is updated with the first reader that does. Claude and Codex keep
sending `_auth/status_update` during a session (`agentz_protocol::thread::AuthStatus`), and
that is still used.

### Quota

- Shown like t3code: each window as a bar with "n% left" and "resets in …", the plan beside
  the email. When a window is used up, threads on that account show when it resets.
- Refreshed every 5 minutes while the app is open (t3code's interval), after each turn ends on
  that account, and on demand. Reads of different accounts are staggered.
- A read that fails keeps the account's last numbers, with when they were read. A login that
  has no usage to read (the External account on an API key or Bedrock, say) shows none.
  (t3code separates the two: a failed probe keeps the last bars, an unsupported login clears
  them.)
- Some readers leave files behind, which the server deletes after each read:
  - Droid's terminal UI writes an empty session file (~250 bytes) under
    `<home>/.factory/sessions/<folder>/` each time it starts, and it must start in a trusted
    folder. The reader runs it in a fixed folder of agentZ's (`accounts/<agent id>/reader/`,
    trusted once in each home) and deletes the sessions opened there.
  - `claude -p "/usage"` writes a transcript under `<config dir>/projects/<folder>/`. It gets
    the same cleanup, unless Claude has a flag that skips saving the session.
  - Codex's `app-server`, Devin's status command and its `GetUserStatus` leave nothing.
- Hidden terminal readers have to handle first-run prompts: Droid's folder trust and Codex's
  update prompt (`-c check_for_update_on_startup=false`).

### Actions when a limit runs out

The subscription agents let the user do something about a used-up limit, mostly only in their
own terminal app. None of these was used on a real account: each one spends a reset or can
bill money.

- **Droid: what happens at the limit.** `/limits` has a "When limit is reached" choice:
  "Switch to Droid Core" (cheaper models, no extra cost) or "Enable Extra Usage" (billed from
  the extra usage balance). It's saved on Factory's server, so it's per account and the CLI
  follows it too: `POST /api/organization/subscription/set-overage-preference` with
  `droidCore` or `extraUsage` *(read in the binary, not called)*. Enterprise orgs set it on
  Factory's dashboard. `/limits` shows the Droid Core models' windows beside the standard ones.
- **Codex: limit resets and credits.** The app-server reports `rateLimitResetCredits` with the
  limits (one on the test account), and `account/rateLimitResetCredit/consume` uses one.
  `/usage` offers "Use this reset?", and t3code shows "1 reset credit banked · Use reset" with a
  confirm. Purchased credits (`credits.balance`) are spent after the plan's limits.
- **Claude: credits, resets and waiting.** Only in the terminal app: `/usage-credits` turns on
  usage credits (reported as `extra_usage`), the hidden `/limit-reset` resets the limits once a
  week, and at the limit `/rate-limit-options` offers switching to usage credits or "Stop and
  wait for limit to reset", with an automatic resume at the reset. agentZ could only reach
  these through a hidden terminal, which the user doesn't allow for Claude.
- **Devin:** overage billing, once the org allows it, set on Devin's site.
- **Grok:** a prepaid balance and an on-demand cap, read from `_x.ai/billing`.
- **Waiting for the reset** works for every agent without its help: the server queues a message
  in the thread and sends it when the account's window resets, even with the app closed.
  t3code does this: at a limit its banner offers "Resume at reset" and "Snooze until reset"
  (`UsageLimitRecoveryBanner`), and it never moves a thread to another account.

Any action that spends a reset or can bill money asks first. What the design round picked:

- **Droid** (§8): tabs over the windows switch between Standard, Droid Core and Extra usage, as
  in `/limits`, with the "When limit is reached" setting under them. The thread's limit notice
  also offers the choices as buttons; picking one saves it on Factory's server and sends the
  message again.
- **Limit resets** (§9): a line under the limits says how many resets the account has and when
  the next expires, with Use Reset, which always asks first. The thread's notice offers it too.
- **Extra usage** (§10): for accounts that report one, a line under the limits says what's left
  of the extra usage or credits, with Manage opening the vendor's page. agentZ never turns paid
  usage on itself. Droid gets no separate line: its balance is on its Extra usage tab, and its
  "When limit is reached" choice (§8) is already its extra usage setting.
- **Waiting** (§11): a setting per account, Stop (today) or Continue at reset, for every thread
  on it. agentZ never moves a thread to another account by itself. On Droid it applies only
  when Droid itself stops, since its own choice at the limit comes first.
- **The thread** (§14): a yellow notice over the composer says which account ran out, when it
  resets, and the ways on: "Continue on <the account with the most left>", whose arrow lists
  the others, and the agent's own actions (§8, §9). Continue with Another Agent ▸ lists the
  agent's other accounts first, the thread's own greyed (§15).

### Skills

- agentZ keeps one skills folder per machine: `<data dir>/skills/<name>/SKILL.md` (§17).
  Settings › Skills is Zed's page (§18): a row per skill with its description, a warning when an
  agent skips it, delete (asks first) and Open ↗; Add Skill offers Add from Folder… and Create a
  Skill.
- The server symlinks each skill into every account's skills folders, including the agent's
  normal home while its External account is listed, so every agent loads them. Symlinked skills
  were checked in Droid, Claude, Codex, Devin, Gemini, Grok, OpenCode and Kilo.
- It links skill by skill, never the whole folder, so the agent's own skills stay beside ours.
  It only ever removes links that point into `<data dir>/skills`. If the agent already has a
  skill with the same name, in its home or in a folder it reads anyway, ours is skipped and
  Settings shows why.
- Links are synced when a skill is added or removed, when an account is created, and when the
  server starts. New sessions pick them up.
- Skills already in the agents' homes or in `~/.agents/skills` aren't imported (the user's
  answer): they stay the agent's own, and may only suit that agent. agentZ's page only adds
  skills of its own, for every agent.
- Skills are per machine, like agent settings. Settings can import them from another of the
  user's machines, as a copy.
- Each skill and MCP server has an accounts menu on its row: "Every account" at first, or the
  accounts it loads on, grouped by agent (§20). It's stored as the accounts it's kept off, so a
  new account starts checked. A skill is linked only into the accounts it loads on, and a
  server is passed only to their sessions.
- agentZ doesn't write into the shared `~/.agents/skills`. Many agents read it whatever their
  home: Codex, Devin, Grok, OpenCode, Kilo, Qoder, Amp, Cline and others. Skills already there
  show up in every account of those agents, and not in Droid, Claude and Cursor accounts,
  which don't read it.

### MCP servers

- Settings › MCP Servers is Zed's page (§19): a row per server (name, Local or Remote, the
  command or URL) with configure, delete and a switch to turn it off, and a line naming the
  agents that can't take it. Add Server offers Add Local Server (name, command, arguments,
  environment variables) and Add Remote Server (URL and headers), each a dialog.
- MCP servers are per machine too, and can be imported from another machine, as skills can.
  The servers in the agents' own configs aren't imported (the user's answer): those stay the
  agent's own.
- The server adds them to the `mcpServers` of every `session/new` and `session/load`, beside
  agentZ's own `agentz` server (`server.rs`, where `agent_control` is added). This is how Zed
  passes its context servers. It covers every agent and account without editing their config
  files.
- Remote servers go only to agents that announce `mcpCapabilities.http`. Droid announces none,
  so it only gets local servers.
- The existing fallback stays: if the agent rejects the MCP servers, the session opens without
  them. It was added for Droid 0.233.0. Droid 0.234.0 was checked: it accepts them and starts
  the server.
- Some agents ignore `mcpServers` from ACP (Cline, Cortex Code, the pi and Autohand adapters).
  They get no app-managed MCP servers, and Settings says so.
- These servers only load in agentZ threads, not when the user runs the agent's CLI in a
  terminal.

### UI

Decided in the design round; `decisions.md` is the spec. In short:

- The agent's Account tab lists a card per account with its limits as bars, "n% left" and
  "resets in …" (§1, §2). Add Account adds a "New account" card with the agent's login rows
  (§3). The External account comes first, tagged "Outside agentZ" (§4). Each card's ⋯ menu has
  Rename, Use for New Threads, Refresh Usage, Open Usage Page, Show in Finder, Log Out and
  Remove Account (§5); the card also picks the account's color (§13).
- A new thread's account sits in the strip under the composer, beside Local and the machine
  (§12). A thread shows its account by the color of its agent's icon (§13).
- Usage shows in a Settings › Usage page across accounts, a gauge in the composer, and the
  account picker (§16).
- Nothing changes for agents with one account.

### Tests

- The mock agent (`crates/agent_thread/test_support/mock_agent.py`) gets a home variable
  (`MOCK_HOME`), keeps its login file there, and offers a status command and a quota command.
  Server tests can then run two accounts at once and check they stay separate.
- The mock agent offers different models per home (from a file in it), so a server test can
  check that each account keeps its own model list and defaults, and that a default the
  account doesn't offer is skipped.
- Reader parsers are unit-tested on the captured fixtures.
- Skills sync runs on temporary folders: links are created, the agent's own skills stay
  untouched, clashes are skipped, and stale links are removed.
- MCP: the mock agent reports the `mcpServers` it got, and a server test checks that the user's
  servers arrive beside `agentz`, and not on an account the server is kept off.
- A new account copies the settings and the agent's settings files of the account picked in
  "Copy settings from" (but no login), and drops a copied default its first session doesn't
  offer.
- Continue at reset: a thread stopped by a limit gets its message sent when the window resets
  (the clock is advanced in the test).

## Order of work

`progress.md` breaks these into commits and tracks them.

1. **Core, with Droid:** rename `LoginSession`, then build accounts data, the External account,
   per-account settings and environments, agent descriptions, readers and login checks. Droid
   works end to end, both with a login and with an API key.
2. **The agent page and threads** (§1–7, §12–16), on Droid.
3. **Claude, Codex, Devin** (wave 1).
4. **Limit actions** (§8–11), once the agents that offer them are in.
5. **Skills and MCP servers** (§17–20).
6. **Wave 2**, one agent per commit.
7. **Wave 3**, one agent per commit, each tested first for its catch.
8. **Later** agents when asked for.

## Agents

### Overview

| Wave | Agents | Why |
|---|---|---|
| 1 | Factory Droid, Claude, Codex, Devin | Researched in depth; the user's main agents |
| 2 | Grok Build, GitHub Copilot, Kilo, GLM Agent, Qoder | A clean home variable, and the identity and quota can be read without a prompt |
| 3 | Cursor, Google Antigravity, Kimi CLI, Auggie, MiniMax Code, Amp, Junie, OpenCode | Each has a catch: HOME has to move, there's no quota source, or nothing was tested |
| Later | Cline, Codebuddy Code, Cortex Code, Mistral Vibe, Qwen Code, Kimchi, pi, Stakpak, Dirac, fast-agent, goose, VT Code, Corust Agent, siGit Code | Weak fit: no quota, ACP ignores MCP, the login is only through other vendors, or there are no docs |
| Skip | Gemini CLI, Nova, Autohand, Poolside, DimCode, Harn, DeepAgents, Minion Code, crow-cli, Agoragentic | Deprecated, keys only, nothing to show, or not a coding agent |

### Wave 1

| | Factory Droid | Claude | Codex | Devin |
|---|---|---|---|---|
| Home | `FACTORY_HOME_OVERRIDE` | `CLAUDE_CONFIG_DIR` | `CODEX_HOME` | `XDG_DATA_HOME` + `XDG_CONFIG_HOME` |
| Login stored | `<home>/.factory/auth.v2.loginkeychain`, encrypted with one shared keychain key | Keychain entry named after the folder | `<home>/auth.json` | `<data>/devin/credentials.toml` |
| Logged out | `session/new` fails | Session opens; `claude auth status --json` | `session/new` fails | Session opens with no models; `devin auth status` |
| Identity | Terminal `/status` *(unverified, read in code)*; API key: `GET /api/cli/whoami` | `claude auth status --json`: email, org, plan, method | `account/read`: email, plan | `devin auth status`: name, email, plan |
| Quota | Terminal `/limits`: 5-hour, weekly, monthly, extra usage; API key: `GET /api/billing/limits` | `claude -p "/usage"`: session and week, % used, resets | `account/rateLimits/read`: windows with `usedPercent`, `windowDurationMins`, `resetsAt`, credits | `GetUserStatus` with `windsurf_api_key` from `credentials.toml` (OpenUsage's): daily and weekly, extra usage balance |
| Skills | `<home>/.factory/skills`, `<home>/.agents/skills` | `<home>/skills` | `<home>/skills`, plus real `~/.agents/skills` | `<config>/devin/skills`, plus real `~/.agents/skills` and `~/.claude/skills` |
| MCP | Stdio only; accepted and started | Accepted | Accepted | Accepted |

Notes:

- **Droid:**
  - Logins are device pairing or an API key. `FACTORY_API_KEY` overrides a stored login.
  - The `/limits` panel has a "When limit is reached" choice: close it with Esc, never Enter.
  - For API-key accounts, both endpoints take `Authorization: Bearer <key>`. Both exist (an
    invalid key gets 401); the response fields weren't seen.
  - `droid doctor --auth --json` shows the email only masked.
- **Claude:** `/usage` returns plain text: "Current session: n% used · resets …" and "Current
  week (all models): n% used · resets …". During turns the ACP adapter also sends
  `usage_update._meta["_claude/rateLimit"]`.
- **Codex:** `codex-acp` runs `codex app-server`, which the reader starts directly with the
  same `CODEX_HOME`. Nothing is left behind.
- **Devin:**
  - `XDG_CONFIG_HOME` also reaches the tools Devin runs (`gh` and others). Test whether
    `XDG_DATA_HOME` alone, which holds the login, is enough. If it isn't, link the user's other
    `~/.config` entries into the account.

### Wave 2

- **Grok Build** (`grok agent stdio`):
  - Home: `GROK_HOME`; the login is a plain `auth.json` there (verified). Remove `XAI_API_KEY`.
  - Logged out: `session/new` fails, and `_x.ai/auth/check_subscription` returns
    `authenticated: false`.
  - Identity: `_x.ai/auth/check_subscription` returns the email, tier and team, sent right
    after `initialize`.
  - Quota: `_x.ai/billing` returns the current period (for example weekly), on-demand cap and
    used, prepaid balance and billing dates.
  - Skills: `$GROK_HOME/skills` (symlinks verified). It also reads the real
    `~/.claude/skills`, `~/.agents/skills` and `~/.cursor/skills`; turn off the first and last
    with `GROK_CLAUDE_SKILLS_ENABLED=false` and `GROK_CURSOR_SKILLS_ENABLED=false`.
  - MCP: http and sse.
  - The probe on the real login refreshed its token; harmless.
- **GitHub Copilot** (`copilot --acp`):
  - Home: `COPILOT_HOME` (verified).
  - The token is in the keychain (service `copilot-cli`), keyed by user and host. Different
    GitHub users don't collide, but the same user in two homes would share it.
  - Use `--no-auto-login`, and remove `COPILOT_GITHUB_TOKEN`, `GH_TOKEN` and `GITHUB_TOKEN`,
    or the `gh` CLI's login and the environment leak in.
  - Logged out: `session/new` fails.
  - Identity: `copilot --headless --stdio` (Content-Length framing), `auth.getStatus`, gives the
    login.
  - Quota: `account.getQuota` returns chat, completions and premium-request snapshots, each
    with entitlement, used, remaining percentage and reset date *(fields from source)*.
  - Skills: `$COPILOT_HOME/skills`. MCP: http and sse.
- **Kilo** (`kilo acp`, an OpenCode fork):
  - Home: `XDG_DATA_HOME`, `XDG_CONFIG_HOME`, `XDG_STATE_HOME` and `XDG_CACHE_HOME` (each gets
    a `kilo/` folder); the login is `auth.json`. Remove `GITHUB_TOKEN`.
  - Logins: Kilo account (Kilo Pass credits, coding plans), ChatGPT, Copilot, SuperGrok,
    provider keys.
  - Logged out: the session opens with free models; `kilo profile --json` exits 1.
  - Identity: `kilo profile --json` gives the name, email, team and balance *(from source)*.
  - Quota: `kilo serve`, then `GET /kilocode/provider-usage`. Each window has used, remaining,
    limit, period and `resetAt`, covering Kilo coding plans, ChatGPT and MiniMax.
  - Skills: `$XDG_CONFIG_HOME/kilo/skills` (symlinks verified), plus the real
    `~/.claude/skills` and `~/.agents/skills` unless `KILO_DISABLE_EXTERNAL_SKILLS` is set.
  - MCP: http and sse, accepted.
- **GLM Agent** (`glm-acp-agent`, a community package for Z.AI's GLM Coding Plan):
  - An account is a key: pass `Z_AI_API_KEY`, and move sessions with `XDG_STATE_HOME`.
  - Logged out: the session opens without a key, so agentZ checks its own key.
  - Quota: `GET https://api.z.ai/api/monitor/usage/quota/limit` with `Authorization: <key>`
    (no "Bearer"). It returns `data.limits[]` with a type, window, percentage and
    `nextResetTime`, plus the plan level. The endpoint is undocumented;
    `zai-org/zai-coding-plugins` uses it.
  - Skills: it only reads `.claude` folders as slash commands, so app skills don't reach it.
  - MCP: http.
- **Qoder** (`qodercli --acp`):
  - Home: `QODER_CONFIG_DIR`. It also edits the real `~/.config/git/ignore`.
  - Whether it uses the keychain is unknown; `QODER_FORCE_ENCRYPTED_FILE_STORAGE` exists.
  - Logged out: `session/new` fails.
  - Identity: `qodercli status -o json` gives `logged_in`, plus the username, email, plan and
    org when logged in *(from source)*.
  - Quota: only in its terminal `/usage` *(unverified)*, which isn't read: no quota reader.
  - Skills: `$QODER_CONFIG_DIR/skills`, plus the real `~/.agents/skills`.
  - MCP: http and sse; whether it's honored is unverified.

### Wave 3

- **Cursor** (`cursor-agent acp`):
  - Login stays shared unless HOME moves: `CURSOR_CONFIG_DIR` moves only the config, and the
    keychain tokens are shared, so a config-only home still reported logged in.
  - Use `HOME=<home>` with `AGENT_CLI_CREDENTIAL_STORE=file` (verified as logged out in a new
    home). Remove `CURSOR_API_KEY` and `CURSOR_AUTH_TOKEN`.
  - With the default keychain store, `agent logout` in any home logs out the user's real
    account.
  - Identity: `cursor-agent status --format json` gives the email and name;
    `about --format json` gives the tier.
  - Quota: its terminal `/usage` (the monthly allowance for Auto, API and total, plus
    on-demand spend) isn't read. Cursor's dashboard API (`api2.cursor.sh`, `DashboardService`'s
    `GetCurrentPeriodUsage` and `GetPlanInfo`, as OpenUsage reads it) with the stored access
    token (kind 7), if `cursor-agent`'s file store keeps one *(unverified)*.
  - Skills: `$HOME/.cursor/skills`, `$HOME/.agents/skills`. MCP: http and sse.
- **Google Antigravity** (`agy-acp-server`):
  - Home: `GEMINI_HOME` plus `AGY_ACP_FORCE_FILE_STORAGE=1`. Without the switch, the login
    goes to one fixed keychain entry that every home shares, and that's where the user's
    current login is.
  - Logged out: `session/new` fails.
  - Identity and quota: nothing over ACP. Google's APIs give them, as OpenUsage reads them:
    Cloud Code's `retrieveUserQuotaSummary` (each model group's 5-hour and weekly windows)
    and `loadCodeAssist` (the tier), and Google's user info, sent an access token got from
    the stored refresh token (kind 7, its exception). The External account's ACP login is in
    a keychain entry only the ACP server may read, so the `agy` CLI's login stands in for it,
    which `agy -p /usage` renews.
  - Skills: `<home>/config/skills`, `<home>/antigravity-cli/skills`. MCP: http and sse,
    accepted.
- **Kimi CLI** (`kimi acp`):
  - Home: `KIMI_SHARE_DIR` plus HOME, because two paths ignore it. The login is a file.
  - Plans have 5-hour and weekly windows.
  - Logged out: `session/new` fails.
  - Quota: the API with the stored OAuth token (kind 7); its terminal `/usage` isn't read.
  - Skills: the first of `~/.kimi/skills`, `~/.claude/skills`, `~/.codex/skills` that exists
    (under the moved HOME), plus `~/.agents/skills`. MCP: http.
- **Auggie** (`auggie --acp`):
  - Home: HOME plus `--augment-cache-dir`. Credit-based plans.
  - Logged out: `session/new` fails. It offers its login only when the client sends
    `_meta["terminal-auth"]`.
  - Quota: `auggie account status --json` gives the plan, amount remaining, included per
    cycle and cycle end *(from source)*.
  - Identity: only the terminal's `/account` *(unverified)*.
  - MCP: converts `mcpServers` though it doesn't announce it.
- **MiniMax Code** (`mcode acp`):
  - Home: `MINIMAX_DATA_DIR` plus HOME. Without HOME it appends a `PATH` export to the real
    `~/.bashrc` and `~/.zshrc` (verified).
  - 5-hour and weekly windows.
  - Logged out: `session/new` fails.
  - Quota with a key: `GET https://api.minimax.io/v1/api/openplatform/coding_plan/remains`
    with `Bearer <key>` *(from a GitHub issue)*. The subscription login has no reader.
  - MCP: http and sse; whether it's honored is unverified.
- **Amp** (`amp-acp`, a third-party wrapper around Sourcegraph's `amp`):
  - Home: `XDG_DATA_HOME` and `XDG_CONFIG_HOME`; logins are files.
  - Logged out: the session opens, and logged-out commands open a browser.
  - Identity and quota: `amp usage` gives "Signed in as …" and the credits remaining *(from
    docs)*.
  - MCP: passed to `amp` on each prompt.
- **Junie** (JetBrains):
  - Home: `JUNIE_HOME`. Keychain use is unclear.
  - Quota: only its terminal `/usage` (remaining balance), which isn't read: no quota reader.
  - Everything is from docs only (the download is 334 MB). Test it first.
- **OpenCode** (installed by the user):
  - Home: `XDG_DATA_HOME`, `XDG_CONFIG_HOME`, `XDG_STATE_HOME`, `XDG_CACHE_HOME`; the login is
    in `opencode.db` (v2). Remove `GITHUB_TOKEN`.
  - Subscription logins: OpenCode Go (5-hour, weekly, monthly), ChatGPT, Copilot, SuperGrok.
  - Logged out: the session opens with free models; `opencode auth list --format json` is
    empty.
  - Identity: only the login's label (org name or "API key").
  - Quota: only through the vendors' APIs with its stored tokens (reader kind 7).
  - Skills: `$XDG_CONFIG_HOME/opencode/skills` (symlinks verified), plus the real
    `~/.claude/skills` and `~/.agents/skills`.
  - MCP: http, accepted.

### Later

| Agent | Home | Why later |
|---|---|---|
| Cline | `CLINE_DIR`, `CLINE_DATA_DIR` | ACP ignores `mcpServers`; identity and quota need the stored token; one fixed hub port (25463) for every home |
| Codebuddy Code | HOME + `CODEBUDDY_CONFIG_DIR` | China-focused logins; quota only in its terminal UI, which isn't read |
| Cortex Code | `SNOWFLAKE_HOME`, or named connections (`-c`) | Snowflake accounts; no quota source; ignores ACP MCP; session opens without a connection |
| Mistral Vibe | `VIBE_HOME` + `VIBE_TEST_DISABLE_KEYRING=1` | Identity through its API with the key, but no quota |
| Qwen Code | `QWEN_HOME` | Coding Plan keys have windows, but no quota API was found; the free login ended 2026-04-15 |
| Kimchi | HOME only | Credits, no LLM quota |
| pi | `PI_CODING_AGENT_DIR` | Logs in to other vendors' subscriptions; the adapter ignores `mcpServers` |
| Stakpak | HOME only | Has its own profiles; account pricing unknown |
| Dirac | `DIRAC_DIR` | ChatGPT login (could read ChatGPT usage); no MCP |
| fast-agent | `FAST_AGENT_HOME` | ChatGPT plan login, kept in the keychain |
| goose | `GOOSE_PATH_ROOT` + `GOOSE_DISABLE_KEYRING` | Mostly keys; ChatGPT, Copilot and Gemini logins |
| VT Code | `VTCODE_CONFIG`, `VTCODE_DATA` | Keychain; already supports several keys per provider |
| Corust Agent | `CORUST_HOME` | Own plan with a quota, but no docs: only binary strings |
| siGit Code | `SIGIT_CONFIG_DIR` | Local models first; whether the cloud tier has a quota is unknown |

### Skip

Gemini CLI (deprecated, the user said), Nova (obfuscated, 462 MB of dependencies, keychain),
Autohand (two packages, ignores MCP, no quota), Poolside (no identity or quota), DimCode (keys
only; its coding plan is "coming soon"), Harn, DeepAgents, Minion Code and crow-cli (keys
only), Agoragentic (a paid-services marketplace, not a coding agent; its README warns against
the npm package the registry installs).

## Open questions

Decided by the user:

- The agent's own login is the External account, listed only while it's logged in; everything
  else lives in agentZ's homes (see The home folder).
- Quota refreshes every 5 minutes, after each turn ends, and on demand (§5).
- The agents' own skills and MCP servers aren't imported: their configs stay as they are, and
  may be agent-specific. agentZ only adds skills and servers of its own, for every agent.
- Agents that need HOME moved get links to the user's `.gitconfig`, `.ssh` and `.config`.
- agentZ may read an agent's stored login where that's the only quota source (reader kind 7),
  and never refreshes it, but for Antigravity, whose access token it gets from the stored
  refresh token. For Antigravity's External account, it may run the `agy` CLI to renew the
  CLI's own login.
- No agent's terminal UI is read but Droid's (reader kind 5).
- Skills and MCP servers are per machine, and can be imported from another machine.
- "Copy settings from" also copies the agent's own settings files.
- On Droid, Stop or Continue at reset applies only when Droid itself stops, and there's no
  separate extra usage line.
- Extra usage is shown, never turned on by agentZ (§10 A, after B was first picked).
- The waves are in the right order. Gemini CLI is deprecated, so it's skipped.

Nothing is open.
