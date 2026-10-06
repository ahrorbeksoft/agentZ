# Accounts: design decisions

Picked on the design board (`design/accounts/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Accounts on the agent’s page: Not decided yet

*Agent page*

**Today:** One Account card on the agent’s Account tab: avatar, email, plan, Change Account and Log Out, with a note when the login came from outside agentZ. Change Account replaces the login; there is no second one.

## 2. How an account’s limits read: Not decided yet

*Agent page*

**Today:** agentZ shows no usage. The agents say it differently: Claude’s `/usage` says “38% used · resets 4:10pm”, Codex says “62% left (resets 16:10)”, Droid draws bars per window, t3code shows “62% left” with a bar and “resets in 2h 10m”. The mocks show the External account; the windows come from each agent (5-hour and weekly here; Droid adds monthly).

## 3. Adding an account: Not decided yet

*Agent page*

**Today:** Not possible. Logging in from the Account card (the rows below, as today for a logged-out agent) or Change Account always uses the agent’s one login.

## 4. The agent’s own login: Not decided yet

*Agent page*

**Today:** The card shows the agent’s own login, with a note under it: “Logged in outside agentZ. … Every thread with it uses this login, and logging out here logs out the CLI too.” In the new design this is the External account: listed only while the agent is logged in outside agentZ, never removed by agentZ, and the one existing threads stay on.

## 5. What each account’s menu offers: Not decided yet

*Agent page · pick any*

**Today:** The agent’s ⋯ menu has Uninstall; the card has Change Account and Log Out. Pick any for the account’s ⋯ menu. The limit actions have their own topics under “When limits run out”.

## 6. Each account’s defaults and environment: Not decided yet

*Agent page*

**Today:** One set per agent: the Defaults tab (“Defaults for New Threads”: a menu per option the agent offers, “Agent’s choice” until you pick) and the Environment tab (variables passed to the agent when it starts). Picking a model in a thread also makes it the default. With accounts, each has its own set: accounts of one agent offer different models (a Team plan whose organization allows fewer, a Pro plan that bills Fable to usage credits, an API key), and some need their own variables (a work proxy). In the mocks, Work offers three models where alex@hey.com offers five.

## 7. What a new account’s settings start as: Not decided yet

*Agent page*

**Today:** Today there’s one set of defaults and variables per agent; the External account keeps it. A new account’s folder also starts without the agent’s own settings file (Claude’s `settings.json`, Codex’s `config.toml`), since agentZ doesn’t copy the agent’s files (an open question in the plan). Here Side is the new account.

## 8. Choosing what happens when a limit is reached: Not decided yet

*When limits run out*

**Today:** Only in each agent’s own terminal app. Droid’s `/limits` has a “When limit is reached” choice: “Switch to Droid Core” (cheaper models, no extra cost) or “Enable Extra Usage” (billed). It’s saved on Factory’s server, so it’s per account and applies to the CLI too; enterprise orgs set it on Factory’s dashboard. Claude’s limit menu offers “Switch to usage credits”, Devin bills “overage” once its org allows it, and Codex spends purchased credits after the plan’s limits. The mocks show a Droid account; agents without such a choice show none.

## 9. Using a limit reset: Not decided yet

*When limits run out*

**Today:** Not shown. Codex grants occasional limit resets: its `/usage` offers “Use this reset?”, and its app-server reports them (`rateLimitResetCredits`, one on the test account) and uses one by request. t3code shows “1 reset credit banked · Use reset” with a confirm. Claude has a hidden `/limit-reset` (once a week, terminal only); agentZ would run it in a hidden terminal, which is untested. The mocks show a Codex account whose 5-hour window is used up.

## 10. Extra usage and credits: Not decided yet

*When limits run out*

**Today:** Not shown. Every subscription agent can bill past its plan: Claude’s usage credits (`extra_usage` in its usage data, turned on in `/usage-credits`), Codex’s purchased credits (`credits.balance`), Droid’s extra usage balance, Devin’s overage, Grok’s prepaid balance. Turning paid usage on happens on the vendor’s site or in its terminal app.

## 11. Waiting for the reset: Not decided yet

*When limits run out*

**Today:** A thread that hits a limit stops with the agent’s error, and nothing happens at the reset. Claude’s terminal app offers “Continue automatically at reset”, and t3code’s limit banner has “Resume at reset”; no ACP agent offers it. agentZ already queues messages (sent when the turn ends), so it can send one at the reset time for any agent.

## 12. Picking the account for a new thread: Not decided yet

*Threads*

**Today:** The new thread’s composer has an agent chip; its menu lists the installed agents, then Terminal and Manage Agents…. A thread keeps its agent once it starts, and would keep its account too: accounts share no sessions. New threads start on the agent’s default account: the one marked “Use for New Threads”, else the External one, else the first.

## 13. Seeing a thread’s account: Not decided yet

*Threads*

**Today:** A thread row shows its project, title, branch, machine and the agent’s icon; the details popover and the composer name the agent. Nothing would say which account. In every option nothing changes for agents with one account.

## 14. When a thread’s account runs out: Not decided yet

*Threads*

**Today:** The turn ends with the agent’s error text in the thread (“You’ve hit your limit · resets 4:10pm”, “Usage limit reached”), worded differently by each agent. agentZ would recognize it from the limits it reads, and the buttons depend on what the agent offers: another account with room, waiting for the reset, a limit reset (Codex), Droid Core or extra usage (Droid).

## 15. Continuing on another account: Not decided yet

*Threads*

**Today:** A thread’s title menu has Continue with Another Agent ▸, which starts a new thread with the conversation handed over as a transcript. Accounts share no sessions, so moving to another account works the same way: a new thread on that account, carrying the conversation, linked to the old one.

## 16. Usage at a glance: Not decided yet

*Threads · pick any*

**Today:** Nothing. Each agent’s page will show its accounts’ limits (topic 1). Pick any other places.

## 17. Where agentZ’s skills live: Not decided yet

*Skills and MCP servers*

**Today:** agentZ has no skills of its own. Each agent loads skills from its own folders (`~/.claude/skills`, `~/.factory/skills`, …), and many also read the shared `~/.agents/skills`, where you have `find-skills`. Either way agentZ links each skill into every account’s skills folder, one link per skill, so the agents’ own skills stay beside them, and skips a skill when the agent has its own of the same name. The plan picked A; Zed does B.

## 18. Settings › Skills: Not decided yet

*Skills and MCP servers*

**Today:** There’s no Skills page. The mocks put Skills and MCP Servers after Agents in the settings list.

## 19. Settings › MCP Servers: Not decided yet

*Skills and MCP servers*

**Today:** There’s no MCP Servers page. agentZ gives every thread’s agent only its own `agentz` server. The servers added here go to every agent and account in agentZ threads (not to the CLIs in a terminal); remote ones only to agents that take them, and agents that ignore ACP’s servers (Cline, Cortex Code) get none.

## 20. Keeping one to some accounts: Not decided yet

*Skills and MCP servers*

**Today:** Everything agentZ manages loads everywhere: skills are linked into every account’s skills folder, and MCP servers go to every session’s agent. A work-only server (the company’s docs, a database) then also loads on a personal account, and its tools fill every thread’s tool list.
