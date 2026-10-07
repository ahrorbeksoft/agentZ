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
  the agents that need them: an empty model list with Cortex Code (later; Devin's status
  command was enough), a key agentZ holds with GLM (wave 2). As the server starts, it checks only the External account, and only of
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
  title-cased. The in-turn `_claude/rateLimit` updates aren't read. Logging
  a new account in (Claude's terminal login with `CLAUDE_CONFIG_DIR` set) wasn't tried: it
  needs the user's browser.
- Item 16 (Codex, `accounts/codex.rs`): the adapter (codex-acp 2.1.1) runs Codex itself with
  `cli`, so the reader is `cli app-server` after the adapter's script: `initialize`, then
  `account/read` (`refreshToken: false`) and `account/rateLimits/read`, and it quits at the
  end of its input (about 4.5 seconds through the adapter). The login check is a session:
  the adapter fails `session/new` with "Authentication required" while Codex has no
  account, as it did in a new home. Checked against the real Codex 0.160.0: a new home reads
  logged out, and the user's own login (Free) read the captured fixture's one monthly
  window. A new home's login is `auth.json` there; an API key login (tried with a fake key
  in a temporary home) has no limits to read, and its read shows none. The API Key method
  takes the key in `authenticate` and Codex keeps it, so Codex has no key login. Window
  names and lengths are t3code's, as are the plans, without "ChatGPT … Subscription". Not
  read: credits (none on the test account, and agentZ shows none yet), other limits than `codex` (a model's own) and the `-c check_for_update_on_startup`
  switch plan.md names, which only the terminal UI needs. Logging a new account in wasn't
  tried: it needs the user's browser.
- Item 17 (Devin, `accounts/devin.rs`): both home variables move, since `XDG_CONFIG_HOME`'s
  `devin/config.json` holds the organization `/org` picked (`devin.org_id`), which belongs to
  the login; "Copy settings from" leaves it out (`login_settings`). The tools Devin runs see
  the same variables, so the account's `.config` and `.local/share` link every entry of the
  user's own except `devin` (`shared_folders`, relinked each time the agent starts; links to
  entries the user removed go). The login check is `auth status`, which always exits 0, so
  it's read by how it starts ("Logged in", `LoggedIn::Prefix`); it reads only the stored
  login, so a `WINDSURF_API_KEY` set on purpose in the Environment shows logged out. The
  reader runs Devin's terminal UI (`devin` with `--respect-workspace-trust false`) in
  `accounts/devin/reader/` with a config folder of its own there (first-run questions
  answered, auto-update off), so it starts none of the user's MCP servers or hooks; its data
  folder is the account's, for the login. It types `/usage`, presses Enter once the menu
  offers it first, and quits with Ctrl+C twice. Until Devin has learned how the account is
  billed (about 8 seconds after start in the real one), `/usage` answers "No credits or ACUs
  consumed yet in this session." without the quota, so the reader asks again each second for
  up to 30 seconds, then reads no windows (a login billed by credits or ACUs). Each run leaves
  a session lock holding the ACP child's process id, which the reader removes once that
  process is gone; nothing else is kept (no session, no prompt history), and Devin rotates
  its own logs. Windows are Devin's: Daily and Weekly, with resets "in 16h 21m" or "Oct 11,
  1:00 PM (UTC+5)". Checked against the real Devin 3000.11.3: logged out in empty folders,
  and the user's own login (Pro) read both windows in about 11 seconds with no lock left. The
  status line's "Pro · 100% remaining" isn't read. Logging a new account in wasn't tried: it
  needs the user's browser.
- Item 18 (`server/limit_waits.rs`): "When a limit is reached" sits under the card's limits,
  past the avatar, as in the mock, and only for agents that read usage, since waiting needs
  the reset from a read; an agent with one account has it too, as it has the notice. The
  notice also gets §14's "Continue at 16:10" (item 13 left it for this), for one thread; once
  the thread waits, its body says agentZ sends "Continue." when the limit resets, and Don't
  Continue cancels it (t3code's "Cancel auto-resume"; the name is my choice). A limit is a
  failed turn with a window used up in the read after it, as for the notice. The server keeps
  the time in `Thread::continues_at` and queues "Continue." then, which starts a stopped
  agent; it looks at the clock at least every minute, since a sleeping Mac stops timers, and
  waits again after a restart. Any turn that starts cancels the wait, an archived thread
  doesn't continue, and a subthread never waits. The time is the read's reset: later reads
  don't move it, and a continue that comes too early (Droid's whole-day resets) fails and,
  on a Continue at reset account, waits again. Switching an account to Stop leaves threads
  already waiting (each notice can cancel). The mock fails prompts once its window is full
  and resets at `resets_at` in its home. Not tried at a real agent's limit, which would
  spend usage.
- Item 19 (`accounts/droid.rs`, `accounts/readers.rs`, `server/usage_reads.rs`,
  `SettingsPage::render_limits` and `render_overage`, the notice in `agent_view.rs`). What
  Droid 0.235.0 does, read in the strings of `~/.local/bin/droid`:
  - `/limits`' "When limit is reached" rows are "Switch to Droid Core" (always) and "Enable
    Extra Usage in settings (opens browser)" (only when the login `canManageOverage`;
    disabled during a free trial). Enter on the first POSTs `set-overage-preference` with
    `{"overagePreference": "droidCore"}` (skipped when it can't manage) and moves that
    session to the recommended Droid Core model. The second only opens
    `app.factory.ai/settings/usage`: nothing in Droid posts `extraUsage`, which is turned on
    on Factory's site. So "Use extra usage" opens that page (the user was told).
  - A row is marked `●` when it's the preference (or Droid Core's when none is set and the
    session's model is a Droid Core one), but the cursor (`>`, blinking between "> " and
    " >") hides the mark of its row, and starts on the first row. Tab goes Standard → Droid
    Core → Extra Usage (the last only when it can manage). Notes are lines starting `●  `
    outside its boxes.
  - Over ACP, at the limit Droid moves to a Droid Core model by itself when the preference is
    `droidCore`, goes on billing when it's `extraUsage`, and otherwise the turn fails.

  Built: the terminal reader reads Standard's tab, then Tab for Droid Core's windows, then ↓
  (waiting up to 5 seconds for the cursor) so the first row's mark shows. A login with one
  row (it can't change the choice) gets no `overage`, since that row's mark stays hidden.
  While extra usage can't be turned on (a free trial), the cursor can't move, so Droid Core
  reads as not chosen even when it is, and a switch then fails its check. A switch through
  the terminal moves the reads' own session to a Droid Core model, which would show Droid
  Core as chosen if the preference were ever cleared on Factory's site. Factory's API reader
  takes `limits.core`, `overagePreference`, `canManageOverage` and `extraUsageAllowed`, and
  switches by the same POST. The server answers `SwitchToDroidCore` once the read after it
  says Droid Core; reads and switches of one account hold one lock. The card's tabs are
  Standard | Droid Core | Extra usage (the last only when the login can change the choice,
  "$0.00 remaining" when it reads no balance), then Droid's row, then item 18's, titled
  "When <agent> stops at a limit". The notice's Switch to Droid Core shows unless it's
  chosen, the login can't change it, or Droid Core's pool is used up too; Use Extra Usage
  shows when extra usage is allowed. The mock's `overage` file makes it Droid-like.
  Not verified: nothing was run against the real Droid (the new tabs are parsed from the
  two screens captured from the user's login, `limits-droid-core.txt` and
  `limits-moved.txt`); switching on a real account (it writes to Factory, and the user's is
  already `droidCore`); and the look of the card and notice, since this session couldn't
  take screenshots (no Screen Recording permission from inside agentZ's terminal). The
  layout is checked in headless tests only.
- Item 20 (`accounts/codex.rs`, `Reader::use_limit_reset`, `server/usage_reads.rs`,
  `usage_limits::render_limit_resets`, `ConfirmRequest::use_limit_reset`): Codex's
  `account/rateLimits/read` has `rateLimitResetCredits`; the reader counts them as t3code
  does (none unless the count is above 0; the next expiry is the earliest `expiresAt` of
  the available ones). Use Reset sends `account/rateLimitResetCredit/consume` with an
  idempotency key and no `creditId` (Codex picks, as in t3code), then reads the account
  again in the same app-server. The server keeps one attempt per account until Codex
  answers it (t3code's `ResetCreditCoordinator`), so a try that timed out and is tried again
  can't spend a second reset; `alreadyRedeemed` counts as done, and `nothingToReset` and
  `noCredit` are errors in t3code's words ("nothing to reset right now", "no reset credit
  left"). A reset and a read of one account hold the same lock. The card's
  line sits under its limits ("1 limit reset available · expires in 27d 23h", t3code's
  duration), with `RotateCcw` for the mock's ticket, which agentZ's icons don't have. Use
  Reset asks in agentZ's confirm dialog with the mock's text ("This clears Work's 5-hour and
  weekly limits now (alex@acme.co, Codex). It uses your only reset and can't be undone.").
  The notice offers Use Reset whenever the account has resets, and then sends the thread's
  last message again, as Switch to Droid Core does. The gauge's popover has the card's line;
  it closes for the question, and nothing goes again unless the thread is at its limit. The
  mock's `limit_resets` file gives it resets, and it has nothing to reset until its window
  is full. Checked against the real Codex 0.160.0 (the user's Free login, which the user let
  this use): the read found its one reset (expiring Oct 29) beside the Monthly window at 3%,
  and consume answered `{"outcome":"nothingToReset"}` twice with one key, leaving the reset
  there: Codex resets only an account at a limit (as Claude's `not_limited` in t3code).
  Not verified: an actual reset, which needs the account at its limit (that spends usage),
  and so `reset` and `alreadyRedeemed` from the real Codex; and the look of the line and
  dialog, checked in headless tests only (no screenshots).
- `tests/browser.rs`'s `remote_agents_hand_their_login_pages_to_the_clients` failed once in
  a full run and passes alone: `agent_settings::write_json` writes `agents/settings.json` in
  place, and the test read it empty mid-write.
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
- Item 21 (`AccountStatus::extra_usage`, `usage_limits::render_extra_usage`): §10 is A now,
  the user's pick when B came to be built. Claude's line is Claude Code's "Usage credits" in
  `/usage`, read from `extra_usage` in the same `get_usage` answer: amounts in cents, the
  currency written as Claude Code writes it (`$`, `€`, `CA$`, …, else the code; yen, won and
  dong in whole units). As there, a Pro or Max login always has the line ("Off" while
  they're off, "Unlimited" without a limit), a team or enterprise one only once they're on
  ("$12.00 spent" without a limit), and other logins none. Codex's is "Credits", from the
  main limit's `credits` (`hasCredits`, `unlimited`, `balance` as text), shown only while
  there are some, as a number ("1,240 left"), since credits aren't money. Droid gets no line:
  its balance is already on its Extra usage tab. Manage ↗ opens the description's usage
  page. Not verified: an account with credits on (the user's Claude Pro login has them off,
  as in the captured fixture, and the Codex Free login has none), and the line's look,
  checked in a headless test only (no screenshots).
- Skills (item 22): agentZ never links into `~/.agents/skills`, which Codex, Devin and others
  read whatever their home, so a skill there stays the user's own; each agent gets agentZ's in
  its own folder instead (Droid's `.factory/skills`, Codex's `$CODEX_HOME/skills`). Devin
  also reads `~/.claude/skills`, so it skips a skill agentZ linked there for Claude's External
  account rather than load it twice. Choices of mine, not on the board: Add from Folder…
  sends at most 32 MB and leaves out `.git` and `.DS_Store`; Open ↗ is off for another
  machine's skills, whose `SKILL.md` isn't on this Mac.

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
| 16 | | Wave 1: Codex | done |
| 17 | | Wave 1: Devin | done |
| 18 | 11 | Stop or Continue at reset, per account | done |
| 19 | 8 | Droid: pools as tabs, When limit is reached, its buttons in the notice | done |
| 20 | 9 | Limit resets (Codex) | done |
| 21 | 10 | Extra usage: the balance, with Manage | done |
| 22 | 17, 18 | agentZ's skills folder, linking into accounts, Settings › Skills | done |
| 23 | 17 | Importing the skills already in the agents' homes and `~/.agents/skills` | dropped |
| 24 | 19 | Settings › MCP Servers, passed to every session | |
| 25 | 20 | The accounts menu on each skill and server | |
| 26 | | Importing skills and servers from another machine (board topic first) | |
| 27 | | Wave 2, one agent per commit | |
| 28 | | Wave 3, one agent per commit | |
