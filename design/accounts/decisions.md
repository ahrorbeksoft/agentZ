# Accounts: design decisions

Picked on the design board (`design/accounts/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Accounts on the agent’s page: A. A card per account, limits inside

*Agent page*

**Today:** One Account card on the agent’s Account tab: avatar, email, plan, Change Account and Log Out, with a note when the login came from outside agentZ. Change Account replaces the login; there is no second one.

**A. A card per account, limits inside** (from today’s Account card, t3code’s limit rows): The Account tab lists every account as today’s card, with its limits under the email: each window’s bar, % left and reset. “Add Account” sits over the list, as “Add Agent” does over the agents. Each card has a ⋯ menu (topic 5). The External account comes first, then agentZ’s in the order they were added.

## 2. How an account’s limits read: A. Bars with % left and the reset

*Agent page*

**Today:** agentZ shows no usage. The agents say it differently: Claude’s `/usage` says “38% used · resets 4:10pm”, Codex says “62% left (resets 16:10)”, Droid draws bars per window, t3code shows “62% left” with a bar and “resets in 2h 10m”. The mocks show the External account; the windows come from each agent (5-hour and weekly here; Droid adds monthly).

**A. Bars with % left and the reset** (from t3code LimitWindows): A row per window: its name and “62% left”, a bar of what’s left, and “resets in 2h 10m”. A hairline on the bar marks where even spending would be; the tooltip gives the exact time. The bar turns yellow under 15% and red when used up.

## 3. Adding an account: A. A new card with today’s login rows

*Agent page*

**Today:** Not possible. Logging in from the Account card (the rows below, as today for a logged-out agent) or Change Account always uses the agent’s one login.

**A. A new card with today’s login rows** (from today’s login rows): Add Account puts a “New account” card at the end of the list with the agent’s login methods, as a logged-out agent shows them today. agentZ makes the account’s folder and runs the login there. Once logged in, the card becomes the account, named by its email; Rename can give it a shorter name later. Cancel removes the empty folder.

## 4. The agent’s own login: A. First in the list, tagged

*Agent page*

**Today:** The card shows the agent’s own login, with a note under it: “Logged in outside agentZ. … Every thread with it uses this login, and logging out here logs out the CLI too.” In the new design this is the External account: listed only while the agent is logged in outside agentZ, never removed by agentZ, and the one existing threads stay on.

**A. First in the list, tagged** (from today’s note, as a tag): The External account comes first with an “Outside agentZ” tag. Today’s note moves into the tag’s tooltip. Its menu has no Remove; Log Out asks first and says it logs out the CLI too.

## 5. What each account’s menu offers: A. Rename + B. Use for New Threads + C. Refresh Usage + D. Open Usage Page + E. Show in Finder + F. Log Out + G. Remove Account

*Agent page · pick any*

**Today:** The agent’s ⋯ menu has Uninstall; the card has Change Account and Log Out. Pick any for the account’s ⋯ menu. The limit actions have their own topics under “When limits run out”.

**A. Rename** (from t3code’s instance name): A short name shown instead of the email, here and in pickers (“Work”). Edited in place on the card.

**B. Use for New Threads** (from new): Marks the account new threads start on, with a “Default” tag. Without it, new threads take the External account, or else the first one.

**C. Refresh Usage** (from t3code’s Refresh): Reads the limits now; the item says when they were last read. They’re also read every 5 minutes and after each turn.

**D. Open Usage Page** (from t3code’s Manage usage): Opens the vendor’s usage or billing page in the browser: claude.ai/settings/usage, chatgpt.com/codex/settings/usage, app.factory.ai/settings/billing, app.devin.ai/settings/usage.

**E. Show in Finder** (from new): Opens the account’s folder (`accounts/claude-acp/work` in the data folder).

**F. Log Out** (from today): Logs the account out and keeps it in the list as logged out, with Log In on its card. Its threads stay and ask to log in again.

**G. Remove Account** (from new): Asks first, then deletes the account’s folder: its login, sessions and history. Its threads stay in the sidebar but can’t continue (Continue on another account still works). Not offered for the External account.

## 6. Each account’s defaults and environment: A. An account picker on the tabs

*Agent page*

**Today:** One set per agent: the Defaults tab (“Defaults for New Threads”: a menu per option the agent offers, “Agent’s choice” until you pick) and the Environment tab (variables passed to the agent when it starts). Picking a model in a thread also makes it the default. With accounts, each has its own set: accounts of one agent offer different models (a Team plan whose organization allows fewer, a Pro plan that bills Fable to usage credits, an API key), and some need their own variables (a work proxy). In the mocks, Work offers three models where alex@hey.com offers five.

**A. An account picker on the tabs** (from today’s tabs): The Defaults, Environment and Threads tabs stay, each with an account menu over it that picks whose settings (or sessions) it shows. It opens on the default account. The model menu lists only what that account offers.

## 7. What a new account’s settings start as: B. Copied from an account you pick

*Agent page*

**Today:** Today there’s one set of defaults and variables per agent; the External account keeps it. A new account’s folder also starts without the agent’s own settings file (Claude’s `settings.json`, Codex’s `config.toml`), since agentZ doesn’t copy the agent’s files (an open question in the plan). Here Side is the new account.

**B. Copied from an account you pick** (from new): The new account’s card (“Adding an account”) gets “Copy settings from”: an existing account, the default one first, or Nothing. Its variables and defaults are copied once; defaults the new account doesn’t offer are dropped when its first session lists its models. After that each account’s settings are its own.

## 8. Choosing what happens when a limit is reached: B. Pools as tabs, as /limits shows them + also C. Asked when it happens

*When limits run out*

**Today:** Only in each agent’s own terminal app. Droid’s `/limits` has a “When limit is reached” choice: “Switch to Droid Core” (cheaper models, no extra cost) or “Enable Extra Usage” (billed). It’s saved on Factory’s server, so it’s per account and applies to the CLI too; enterprise orgs set it on Factory’s dashboard. Claude’s limit menu offers “Switch to usage credits”, Devin bills “overage” once its org allows it, and Codex spends purchased credits after the plan’s limits. The mocks show a Droid account; agents without such a choice show none.

**B. Pools as tabs, as /limits shows them** (from Droid’s /limits tabs): Tabs over the windows switch between Standard, Droid Core and Extra usage, as in Droid’s panel; the same “When a limit is reached” setting sits under them.

**Also take from C. Asked when it happens** (from Claude’s rate-limit options): No setting on the card. When a thread hits the limit, its notice offers the choices as buttons, as Claude’s terminal app does: “Switch to Droid Core”, “Use Extra Usage”. Picking one saves it on Factory’s server and sends the message again.

## 9. Using a limit reset: A. A line under the limits, with a confirm

*When limits run out*

**Today:** Not shown. Codex grants occasional limit resets: its `/usage` offers “Use this reset?”, and its app-server reports them (`rateLimitResetCredits`, one on the test account) and uses one by request. t3code shows “1 reset credit banked · Use reset” with a confirm. Claude has a hidden `/limit-reset` (once a week, terminal only); agentZ would run it in a hidden terminal, which is untested. The mocks show a Codex account whose 5-hour window is used up.

**A. A line under the limits, with a confirm** (from t3code ResetCredits): When the account has resets, a line under its limits says how many and when the next expires, with Use Reset. It always asks first, since a reset can’t be given back. The thread’s limit notice offers the same button (see “When a thread’s account runs out”).

## 10. Extra usage and credits: B. A switch to turn it on

*When limits run out*

**Today:** Not shown. Every subscription agent can bill past its plan: Claude’s usage credits (`extra_usage` in its usage data, turned on in `/usage-credits`), Codex’s purchased credits (`credits.balance`), Droid’s extra usage balance, Devin’s overage, Grok’s prepaid balance. Turning paid usage on happens on the vendor’s site or in its terminal app.

**B. A switch to turn it on** (from Claude’s /usage-credits, Droid’s preference): A switch per account: “Use extra usage when limits run out”, asking first because it bills the card on file. agentZ changes it through the agent (Droid’s preference; Claude’s only through its terminal app, untested).

## 11. Waiting for the reset: B. A setting per account

*When limits run out*

**Today:** A thread that hits a limit stops with the agent’s error, and nothing happens at the reset. Claude’s terminal app offers “Continue automatically at reset”, and t3code’s limit banner has “Resume at reset”; no ACP agent offers it. agentZ already queues messages (sent when the turn ends), so it can send one at the reset time for any agent.

**B. A setting per account** (from Claude’s rate-limit options): A “When a limit is reached” setting on the account: Stop (today) or Continue at reset, for every thread on it. agentZ never moves a thread to another account by itself.

## 12. Picking the account for a new thread: E. In the strip under the composer

*Threads*

**Today:** The new thread’s composer has an agent chip; its menu lists the installed agents, then Terminal and Manage Agents…. A thread keeps its agent once it starts, and would keep its account too: accounts share no sessions. New threads start on the agent’s default account: the one marked “Use for New Threads”, else the External one, else the first.

**E. In the strip under the composer** (from today’s checkout and machine pickers): Beside Local and the machine, under the composer: the account’s avatar and name, with the same menu as A.

## 13. Seeing a thread’s account: D. A color per account

*Threads*

**Today:** A thread row shows its project, title, branch, machine and the agent’s icon; the details popover and the composer name the agent. Nothing would say which account. In every option nothing changes for agents with one account.

**D. A color per account** (from t3code accent colors): Each account gets a color, picked on its card, that tints the agent’s icon wherever the thread shows.

## 14. When a thread’s account runs out: A. A notice over the composer

*Threads*

**Today:** The turn ends with the agent’s error text in the thread (“You’ve hit your limit · resets 4:10pm”, “Usage limit reached”), worded differently by each agent. agentZ would recognize it from the limits it reads, and the buttons depend on what the agent offers: another account with room, waiting for the reset, a limit reset (Codex), Droid Core or extra usage (Droid).

**A. A notice over the composer** (from t3code ThreadErrorBanner): A yellow notice above the composer: which account ran out, when it resets, and the ways on. “Continue on Side” offers the account with the most left; its arrow lists the others.

## 15. Continuing on another account: A. Accounts in Continue with Another Agent

*Threads*

**Today:** A thread’s title menu has Continue with Another Agent ▸, which starts a new thread with the conversation handed over as a transcript. Accounts share no sessions, so moving to another account works the same way: a new thread on that account, carrying the conversation, linked to the old one.

**A. Accounts in Continue with Another Agent** (from today’s submenu): The submenu lists the agent’s other accounts under it (the thread’s own one greyed), then the other agents.

## 16. Usage at a glance: A. A Usage page in Settings + B. A gauge in the composer + C. In the account picker

*Threads · pick any*

**Today:** Nothing. Each agent’s page will show its accounts’ limits (topic 1). Pick any other places.

**A. A Usage page in Settings** (from t3code’s Usage page): Settings › Usage lists every agent with accounts. Each window is one card: how much is left across all accounts, and a bar split into one segment per account with its % and reset. Clicking a segment opens the account on its agent’s page.

**B. A gauge in the composer** (from t3code ComposerUsageLimits): Beside the agent chip, a small gauge with the thread’s account’s tightest window (“62%”). Clicking it shows that account’s windows, with Use Reset and Usage when they apply.

**C. In the account picker** (from the picker topic): Already in the account picker (“Picking the account for a new thread”, options A–C): each account with its tightest window. Pick this to keep that and nothing more.

## 17. Where agentZ’s skills live: A. agentZ’s own folder

*Skills and MCP servers*

**Today:** agentZ has no skills of its own. Each agent loads skills from its own folders (`~/.claude/skills`, `~/.factory/skills`, …), and many also read the shared `~/.agents/skills`, where you have `find-skills`. Either way agentZ links each skill into every account’s skills folder, one link per skill, so the agents’ own skills stay beside them, and skips a skill when the agent has its own of the same name. The plan picked A; Zed does B.

**A. agentZ’s own folder** (from the plan): Skills live in agentZ’s data folder and are linked into every account, External included. The agents’ CLIs see them in a terminal only through those links.

## 18. Settings › Skills: A. Zed’s Skills page

*Skills and MCP servers*

**Today:** There’s no Skills page. The mocks put Skills and MCP Servers after Agents in the settings list.

**A. Zed’s Skills page** (from Zed skills_setup.rs): Each skill as a row: its name and description from `SKILL.md`, a warning when an agent skips it (and why), a delete button that asks first, and Open ↗ for its `SKILL.md`. Add Skill offers Add from Folder… and Zed’s Create a Skill (a folder with a new `SKILL.md`). Empty, it says Zed’s “No global skills installed.”

## 19. Settings › MCP Servers: A. Zed’s MCP Servers page

*Skills and MCP servers*

**Today:** There’s no MCP Servers page. agentZ gives every thread’s agent only its own `agentz` server. The servers added here go to every agent and account in agentZ threads (not to the CLIs in a terminal); remote ones only to agents that take them, and agents that ignore ACP’s servers (Cline, Cortex Code) get none.

**A. Zed’s MCP Servers page** (from Zed mcp_servers_page.rs): Servers as rows: name, Local or Remote, the command or URL, configure and delete buttons, and a switch to turn one off. A line says which agents can’t take a server. Add Server offers Add Local Server and Add Remote Server, each a dialog in Zed’s words: Server Name, Command, Arguments, Environment Variables; or URL and Headers.

## 20. Keeping one to some accounts: B. An accounts menu on each row

*Skills and MCP servers*

**Today:** Everything agentZ manages loads everywhere: skills are linked into every account’s skills folder, and MCP servers go to every session’s agent. A work-only server (the company’s docs, a database) then also loads on a personal account, and its tools fill every thread’s tool list.

**B. An accounts menu on each row** (from new): Each skill and server gets a menu beside its controls: “Every account” at first, or the accounts it loads on, grouped by agent, with a check each. A new account starts checked, as Zed’s new threads follow the default profile.
