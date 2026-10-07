# Accounts: build progress

The UI spec is `decisions.md` (numbers below are its sections); everything else is in `plan.md`.
Agents building this: take the first item that isn't done, mark it in progress here in the same
commit as your first change, and mark it done with its commit when it's finished. Each item is
committed and pushed on its own, so `git log` and `git status` show where work stopped.

Notes for whoever continues:
- Test with the mock agent only (`MOCK_HOME` and friends, see plan.md › Tests). Droid is the
  first real agent, and only `initialize` and empty sessions may be sent to it; its quota
  reader is checked against captured screens, not by spending usage.
- An agent with one account looks and behaves as today everywhere.
- §20 is stored as the accounts a skill or server is kept off, so a new account starts with it.
- Droid (the user's answer): §8 is its limit choice, §11 applies only when Droid itself stops,
  and §10 has no Droid switch.
- §7 also copies the agent's own settings files (never the login), as the user decided.
- The user lifted the rule against reading agents' stored logins (reader kind 7). AGENTS.md's
  pitfall now allows status commands and points at kind 7, which comes with wave 3.
- Importing skills and servers from another machine isn't on the design board yet: add a topic
  and let the user pick before building item 26.
- The agents' own skills and MCP servers are never imported (the user's answer, which dropped
  item 23): they stay as the agent has them, and may be agent-specific. agentZ only adds its
  own, for every agent (items 22, 24, 25).
- Login checks (item 5) are an empty session (`LoginCheck::Session`, Droid's) or the agent's
  status command (`LoginCheck::Command`). The other two in plan.md › Login checks come with
  the agents that need them: an empty model list with Devin (17), a key agentZ holds with GLM
  (wave 2). As the server starts, it checks only the External account, and only of
  agents that have agentZ accounts: without any, nothing depends on it, and every agent would
  be started at each server start.
- The server takes an account for login sessions (`OpenLoginSession`) and for listing and
  importing sessions. Since item 8, the Account tab opens a login session per account.
- Item 15 (Claude, `accounts/claude.rs`): the adapter runs Claude Code itself with `--cli`
  (as its terminal logins do), so the login check is `--cli auth status --json` after the
  adapter's script, by `loggedIn` (true on an API key or Bedrock too, as checked on 2.1.287).
  The reader isn't plan.md's `claude -p "/usage"`, which goes in as a prompt and prints text
  to parse: it's `get_usage`, the control request the adapter's and t3code's `/usage` send
  (numbers and ISO reset times), to Claude Code
  started with no prompt (`--no-session-persistence`, `--strict-mcp-config`,
  `disableAllHooks`: the user's SessionStart hooks report to herdr and Orca, and would take
  each read for a session). It leaves nothing behind (no transcript, no project entry), and
  takes about 6 seconds with `auth status`. Checked against the real Claude: a new home is
  logged out, and the user's own login read the same windows as the captured fixture. Window
  names are t3code's (Session, Weekly, "Weekly · <model>"); the plan is `subscriptionType`
  title-cased. Extra usage and the in-turn `_claude/rateLimit` updates aren't read. Logging
  a new account in (Claude's terminal login with `CLAUDE_CONFIG_DIR` set) wasn't tried: it
  needs the user's browser.
- Item 14: Settings › Usage lists the installed agents that read their accounts' limits
  (`reads_usage`), an agent with one account included, as the mock's Codex is; "across N
  accounts" shows only with more than one. Accounts found logged out are left out, as their
  reads are out of date. Pools are by window name, what's left being 100 less the mean used
  (t3code's). A segment's click opens the account's Account tab. The gauge is only in a
  started thread's composer (a new thread's account picker already has the limits), and
  isn't shown while the account is found logged out. Its popover has the account's windows
  and Usage ↗; Use Reset comes with limit resets (20).
- Item 13: the notice shows when a turn ends with an error while the last read of the
  thread's account has a window used up that hasn't reset; agentZ doesn't parse the agents'
  error texts. A failed turn is read again, so the notice can follow the error by a few
  seconds. It's Zed's warning `Callout` (so its icon is the warning triangle, not the mock's
  hourglass), the body's tooltip has the agent's error, and its × closes it until the next
  turn. With one account listed it says "Your account" and offers no Continue; the Continue
  button is a split button only when there's more than one other account. The Usage link is
  the description's `usage_page`. Continue at the reset (18), Droid's choices (19) and limit
  resets (20) add their buttons to it later.
- Item 12: the thread's agent heads the submenu as a row that isn't picked itself (its
  accounts beneath it are), and its accounts are drawn as in the new thread's account picker.
  With one account listed, the submenu is as before. Agent control lists `accounts` (key: the
  id, or "external"; name; isDefault; models; modes) only for an agent with more than one, and
  `account` takes a key or a name, failing with `account_unavailable` otherwise. A logged-out
  account can be launched on, as in the picker; its thread asks to log in. The caller's model
  carries over to a thread on another account only when that account offers it.
- Item 11: the mock's tinted glyph is a tile, but the app's rows draw the agent's icon bare,
  so the icon itself takes the account's color (its theme shade, at full strength where it
  was muted and faint). That's on the sidebar card and its details popover, Go To, Workspaces'
  pane headers, rows, agents list and save-layout list, the started thread's composer and
  subthread bar, and the continued-from, continued-in and handoff cards
  (`AgentAccounts::thread_color`). Only while the agent lists more than one account, and only
  for an account given a color: none has one until it's picked on its card. A new thread's
  agent chip stays as it was; its account shows in the strip.
- Item 10: the account chip sits after Local and the machine (after the folder in a
  Workspaces draft), only while the agent lists more than one account. Its menu is §12 A's
  with §16 C's limits already in it (the window closest to running out, red "Used up · 1h 9m"
  when spent), so item 14 has only the Usage page and the gauge left. An account found logged
  out shows "Logged out" instead of its plan; it's still offered, as on the Defaults tabs, and
  its thread asks to log in. Picking an account makes the draft again (ACP can't move a
  session); another checkout keeps the account, another agent or machine takes its account
  for new threads. `ContinueThread` takes an `account` now (the draft of a continuation keeps
  its account picker; item 12 adds the accounts to Continue with Another Agent). Add Account…
  and Manage Accounts… open the agent's Account tab (Add Account… also adds a card there);
  a page already open on that agent keeps its login sessions.
- Item 9: the Defaults, Environment and Threads tabs share one account menu
  (`AgentPanel::picked_account`). It opens on the account for new threads and follows it
  until one is picked; a picked account that's no longer listed gives way to it again. With
  one account listed there's no menu. "Copy settings from" is `CopyAccountSettings`;
  `AddAccount` copies from the account for new threads first (`default_settings_source`). It
  copies the Environment (without the description's login variables, which are a login),
  the defaults, and the description's `settings_files` from that account's home (the normal
  home for the External account), with the keys `home_files` starts a home with written over
  them (Droid's `cloudSessionSync: false`). Copied defaults wait for the account's first
  session (`AgentSettings::copied_defaults`), which drops those it doesn't offer. The row
  shows only on a New account card while more than one account is listed, and copying starts
  its login session again, with the copied variables.
- Item 8: the app keeps every machine's accounts from `Event::Accounts`
  (`ServerClient::accounts`). Rename saves on Enter or a click elsewhere, Escape cancels, and
  an empty name shows the email again. Cancel on a New account card removes it without asking,
  since its folder has nothing yet; Remove Account asks. The menu's icons are muted, as the
  sidebar's, Remove Account included (Zed has no red items). Changes that fail say so under
  "Accounts".
- Only agents with a description can have more accounts (`AddAccount` refuses the rest).
  Droid's is `accounts/droid.rs`; wave 1 and later agents each add one beside it.
- Readers (item 6): the mock's kind is `Reader::Command` (printing agentZ's own JSON); each
  other kind comes with the first agent that needs it. Droid's (`Reader::DroidTerminal`, the
  hidden terminal, kind 5) reads `/status` and `/limits` by symbols and numbers only, as the
  user asked, since Droid is translated. Droid's own code makes that hold: it writes reset
  times in English in every language ("2 days", "1h 5min", "39min"), `/limits` always opens
  on Standard Usage, and its windows are always drawn 5-hour, weekly, monthly, so they're
  named by position. It runs in `accounts/factory-droid/reader/`, answering the trust
  question once per home (for the External account, an entry in the user's own
  `~/.factory/settings.json`), with cloud session sync off by `--settings`. It resumes one
  session per home instead of opening one each time: Droid keeps every session it opened in
  `cache/session-discovery-index.json` and `cache/session-index/index.db` even after its files
  are deleted, so deleting them after each read (the first version) grew those by one entry
  every 5 minutes. Checked against the real Droid (a new key home and the user's own login):
  the same numbers as Factory's API, in 3 to 8 seconds (a resumed read about 3, with no new
  session or index entry); weekly and monthly resets are only as exact as Droid's whole
  days. The limit choice stayed `droidCore`.
- New Droid accounts start with `cloudSessionSync: false` in their `.factory/settings.json`
  (the user's request; Droid's `/settings` turns it back on). It's a description's
  `home_files`, written at `AddAccount`.
- API-key accounts (item 7): a description's key login is the method that reads a key from a
  variable. agentZ keeps the key in the account's folder and restarts the agent with it to log
  in. Droid takes any key (checked over ACP: a made-up one passes `authenticate` and opens
  sessions), so agentZ checks the key with its reader first. Droid's key reader
  (`Reader::FactoryApi`) parses `/api/billing/limits`; its test answer is a real one, with
  other numbers. A key account has no email (Factory's `whoami` gives only ids), and only
  Factory's US address is used, not its EU one. An agentZ account's login rows have the key
  field; a key account's card is titled by its login method, having no email.

| Order | § | Item | Status |
|---|---|---|---|
| 1 | | Rename `Account` and `ConnectionId::Account` to `LoginSession` | done |
| 2 | | Accounts data: `accounts.json`, `AccountId`, a thread's account, the External account from the normal home's login check | done |
| 3 | | Settings per account: `AgentSettings` keyed by account, the External account keeping today's | done |
| 4 | | Agent descriptions and each account's environment; an agent process per (agent, account); the mock agent's `MOCK_HOME`; Droid's description | done |
| 5 | | Login checks from the description | done |
| 6 | | Identity and quota readers, refresh (5 minutes, after each turn, on demand), failed reads keeping the last numbers; Droid's `/status` and `/limits` reader | done |
| 7 | | API-key accounts (Droid with a Factory API key) | done |
| 8 | 1–5, 13 | Account tab: a card per account with limit bars, Add Account, the External account tagged, the ⋯ menu, the account's color | done |
| 9 | 6, 7 | Account menu on the Defaults, Environment and Threads tabs; Copy settings from | done |
| 10 | 12 | The account in the strip under the composer; the default account for new threads | done |
| 11 | 13 | The account's color on its threads' agent icon | done |
| 12 | 15 | Accounts in Continue with Another Agent; agent control's accounts (plan › Settings per account: each account's models in the agent listing, an `account` argument on launch and delegate) | done |
| 13 | 14 | The limit notice over the composer (Continue on another account first) | done |
| 14 | 16 | Settings › Usage, the composer gauge (the picker's limits came with item 10) | done |
| 15 | | Wave 1: Claude | done |
| 16 | | Wave 1: Codex | |
| 17 | | Wave 1: Devin | |
| 18 | 11 | Stop or Continue at reset, per account | |
| 19 | 8 | Droid: pools as tabs, When limit is reached, its buttons in the notice | |
| 20 | 9 | Limit resets (Codex) | |
| 21 | 10 | Extra usage switch | |
| 22 | 17, 18 | agentZ's skills folder, linking into accounts, Settings › Skills | |
| 23 | 17 | Importing the skills already in the agents' homes and `~/.agents/skills` | dropped |
| 24 | 19 | Settings › MCP Servers, passed to every session | |
| 25 | 20 | The accounts menu on each skill and server | |
| 26 | | Importing skills and servers from another machine (board topic first) | |
| 27 | | Wave 2, one agent per commit | |
| 28 | | Wave 3, one agent per commit | |
