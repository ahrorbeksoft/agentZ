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
- Login checks (item 5) are an empty session (`LoginCheck::Session`, Droid's) or the agent's
  status command (`LoginCheck::Command`). The other two in plan.md › Login checks come with
  the agents that need them: an empty model list with Devin (17), a key agentZ holds with GLM
  (wave 2). As the server starts, it checks only the External account, and only of
  agents that have agentZ accounts: without any, nothing depends on it, and every agent would
  be started at each server start. The app ignores `Event::Accounts` until item 8 shows them.
- The server takes an account for login sessions (`OpenLoginSession`) and for listing and
  importing sessions, but the app still sends the External account (`None`) for both, and its
  settings tabs edit the External account's settings, until items 8 and 9 add the menus.
- Only agents with a description can have more accounts (`AddAccount` refuses the rest).
  Droid's is `accounts/droid.rs`; wave 1 and later agents each add one beside it.

| Order | § | Item | Status |
|---|---|---|---|
| 1 | | Rename `Account` and `ConnectionId::Account` to `LoginSession` | done |
| 2 | | Accounts data: `accounts.json`, `AccountId`, a thread's account, the External account from the normal home's login check | done |
| 3 | | Settings per account: `AgentSettings` keyed by account, the External account keeping today's | done |
| 4 | | Agent descriptions and each account's environment; an agent process per (agent, account); the mock agent's `MOCK_HOME`; Droid's description | done |
| 5 | | Login checks from the description | done |
| 6 | | Identity and quota readers, refresh (5 minutes, after each turn, on demand), failed reads keeping the last numbers; Droid's `/status` and `/limits` reader | |
| 7 | | API-key accounts (Droid with a Factory API key) | |
| 8 | 1–5, 13 | Account tab: a card per account with limit bars, Add Account, the External account tagged, the ⋯ menu, the account's color | |
| 9 | 6, 7 | Account menu on the Defaults, Environment and Threads tabs; Copy settings from | |
| 10 | 12 | The account in the strip under the composer; the default account for new threads | |
| 11 | 13 | The account's color on its threads' agent icon | |
| 12 | 15 | Accounts in Continue with Another Agent; agent control's accounts (plan › Settings per account: each account's models in the agent listing, an `account` argument on launch and delegate) | |
| 13 | 14 | The limit notice over the composer (Continue on another account first) | |
| 14 | 16 | Settings › Usage, the composer gauge, limits in the account picker | |
| 15 | | Wave 1: Claude | |
| 16 | | Wave 1: Codex | |
| 17 | | Wave 1: Devin | |
| 18 | 11 | Stop or Continue at reset, per account | |
| 19 | 8 | Droid: pools as tabs, When limit is reached, its buttons in the notice | |
| 20 | 9 | Limit resets (Codex) | |
| 21 | 10 | Extra usage switch | |
| 22 | 17, 18 | agentZ's skills folder, linking into accounts, Settings › Skills | |
| 23 | 17 | Importing the skills already in the agents' homes and `~/.agents/skills` | |
| 24 | 19 | Settings › MCP Servers, passed to every session | |
| 25 | 20 | The accounts menu on each skill and server | |
| 26 | | Importing skills and servers from another machine (board topic first) | |
| 27 | | Wave 2, one agent per commit | |
| 28 | | Wave 3, one agent per commit | |
