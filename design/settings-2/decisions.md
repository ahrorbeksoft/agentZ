# Settings: design decisions

Picked on the design board (`design/settings-2/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. How Settings is organized: A. Headings in the sidebar, General split up

*Layout*

**Today:** The settings sidebar lists nine pages (General, Appearance, Notifications, Agents, Usage, Skills, MCP Servers, Machines, Storage), then “Projects” with a row per project. General is one long page of seven sections and 23 rows: Threads (Thread order, Use modifier to send, Show thinking, Group working threads, Auto-archive threads, Subthreads at once, Usage shown in the composer), Chats, Artifacts (Open in the browser and four design defaults), Thread titles (its own machine picker, then Generate thread titles, Provider, Model, Reasoning effort), Projects (three rows on combining), Server (Start at login) and Experimental (ACP v2). Agents, Usage, Skills, MCP Servers and Storage each have the machine picker in their header. Nothing searches. “How this works” lists every setting with where it applies.

**A. Headings in the sidebar, General split up** (from new, with t3code’s one-topic pages): The sidebar gets small muted headings, as it has for Projects: agentZ (General, Appearance, Notifications), Threads (Threads, Subthreads, Thread titles, Artifacts), Agents (Agents, Usage, Skills, MCP Servers), Machines (Machines, Storage), then Projects. General’s big sections become pages: Threads holds its seven rows less Subthreads at once, plus Chats; Subthreads holds Subthreads at once and the roles (topic 4); Thread titles takes the machine picker into its header, like Agents; Artifacts holds its five rows. General keeps Projects, Server and Experimental.

## 2. A setting for all machines: A. “All machines” first in the machine picker + also B. One view, “Set on Devbox 1” on the rows that differ + also C. A “Use All machines’” switch per section

*Machines and accounts*

**Today:** What each machine’s server keeps is shown one machine at a time. Agents, Usage, Skills, MCP Servers and Storage share a machine picker in their header: Agents shows which agents that machine has installed, and an agent’s page belongs to the machine it was opened on, with Defaults for New Threads and Environment Variables for each of its accounts there; Usage is read only; Skills and MCP Servers are each machine’s lists; Storage has that machine’s Cleaning up rules. General › Thread titles has a picker of its own as its first row, for Generate thread titles, Provider, Model and Reasoning effort. Start at login is This Mac’s alone. What the app sends to every server (Auto-archive, Subthreads at once, the design defaults, ACP v2, the subthread roles) is the same everywhere and has no picker. So a cleanup rule, a skill or the titles’ model, wanted on three machines, takes three visits, and nothing says Devbox 1 differs. The mocks use Thread titles as a page with the picker in its header, as topic 1’s A makes it.

**A. “All machines” first in the machine picker** (from t3code’s “All environments”): The machine picker gets All machines at the top, over a line, as t3code’s scope menu has “All environments”, and pages open on it. A change there applies to every machine that doesn’t set its own, and to machines added later. On All machines, a row some machine sets differently says so under its description, “Devbox 1 sets its own: Opus 5.5”. On one machine, a row it sets itself has Zed’s ↺ beside its title (“Reset to All machines’: Sonnet 5.5”); the rest look as they do now. Skills and MCP servers added on All machines go to every machine; a machine’s view lists those and its own. Agents and Usage keep a machine’s view only, since installs and reads are each machine’s.

**Also take from B. One view, “Set on Devbox 1” on the rows that differ** (from Zed’s “Modified in”, t3code’s inheritance popover): Pages that can share show one value, every machine’s, with no picker. A row a machine sets differently says so after its title, as Zed’s rows say “— Modified in settings.json”. Clicking it opens t3code’s popover of where the value comes from: each machine with its value, its own ones with ↺. Changing one machine’s value happens there. Agents, Usage and Storage’s sizes keep the picker, since what they show is each machine’s.

**Also take from C. A “Use All machines’” switch per section** (from agentZ’s project “Use General’s”): As a project’s Design defaults follow General’s: the picker gets All machines, and on one machine each section starts with a Use All machines’ switch, on, with what that means (“On · Claude · Sonnet 5.5 · Low”). Turning it off copies All machines’ values for that machine and shows its rows to change; turning it back on drops them. On All machines, a section a machine has its own of says “Devbox 1 sets its own.”

**Comment on C:** maybe use defaults or something use general settings or something "use all machines" feel awkward

## 3. A setting for all accounts: C. A “Use All accounts’” switch per tab

*Machines and accounts*

**Today:** “Profile” isn’t a word agentZ uses. I read it as an agent’s accounts (Claude Agent’s Personal and Work), whose settings are each account’s own. On an agent’s page, Defaults for New Threads (model, effort, mode) and Environment Variables are per account, through the “Account” menu over those tabs, which opens on the account for new threads; choosing a model in a thread on an account makes it that account’s default. The Account tab’s cards each have When a limit is reached (Stop or Continue at reset), and Factory Droid’s own one, which Factory keeps for each login. A new account starts from Copy settings from (another account, or Nothing), copied once. Skills and MCP servers already have an accounts menu (“Every account”, “2 of 3”). If you meant something else, say so in a comment. Two other readings: a project’s own settings (its Design defaults, with “Use General’s”), or Zed’s settings profiles (named sets of settings laid over yours, which agentZ doesn’t have). The options use the same letters as the topic before.

**C. A “Use All accounts’” switch per tab** (from agentZ’s project “Use General’s”): The Account menu gets All accounts. On one account, Defaults and Environment each start with a Use All accounts’ switch, on, with what that means (“Opus 5.5 · High · Default”). Off, the account gets a copy to change; back on, the copy goes. All accounts says which accounts have their own.

**Comment on C:** here too as i've said i nthe settings "All accounts" feel awkward, maybe default settings or osmething like that

## 4. What each subthread role runs on: D. A role per row, its picks as chips

*Subthreads*

**Today:** Built without a screen (3b4bf5a). Agents delegate by four roles: researcher (finds things out), reviewer (checks work), worker (does it) and other (the default). For each, the user keeps an ordered list of picks, each an agent with a model and effort (both may be left to the default). A task tries its role’s picks in order: the first whose agent is installed on the task’s machine and has an account logged in, not used up and offering the model runs it; of that agent’s accounts, the one with the most usage to spare goes first. An empty list leaves the choice to the agent that delegates. The picks are an app setting the app sends every server, so a task started on Devbox 1 uses Devbox 1’s accounts. Agents see the picks in their tools. There’s no row for them in Settings yet; the backlog suggests one beside Subthreads at once in General › Threads. Neither Zed nor t3code has anything like it. The mocks use topic 1’s A (a Subthreads page) where they need a page.

**D. A role per row, its picks as chips** (from new): Each role is one row: its name, then its picks as chips in order (“1 Codex · GPT-5.5 · High”), dragged to reorder, and a + chip to add one. Clicking a chip opens a small popover with the agent, Model and Effort menus and Remove.

**Comment on D:** if agent has multiple accounts, show an account selector below "Agent" but we should also have "all" to select all.

## 5. Which account a pick would use: None of these

*Subthreads*

**Today:** The server chooses the account when a task starts: of the pick’s agent’s accounts on the task’s machine, logged in, not used up and offering the model, the one with the most usage to spare per hour until its limit resets (60% left resetting in an hour beats 80% left resetting in six days), then the account for new threads. Settings doesn’t say which that would be. In the mocks, on This Mac: Claude Agent’s Work is used up (resets in 1h 52m), so Personal (62% left); Codex has one account (81% left); Factory Droid’s login is used up until 4:10 PM, so its picks are skipped. Devbox 1 has only Claude Agent, with its own login. The mocks use this round’s A for the list.

**Note:** i've told you what to do in the topic 4
