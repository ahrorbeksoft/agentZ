# Model Visibility and Favorites: design decisions

Picked on the design board (`design/model-visibility/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Where you choose the models the picker shows: A. A Models tab on the agent’s page

*Hiding models*

**Today:** The model chip beside Send (“Opus 5.5 · Extra High ⌄”) opens a picker of every model the agent offers, in the agent’s order: here Factory Droid’s 66, with a “Fast Mode” twin for many and three “[Deprecated]” ones. OpenCode offers 111 (“OpenCode Zen/…”, “GitLab Duo/…”), Claude Agent 4. “Search models…” narrows the list, and the effort’s levels follow under it. Settings › Agents › Factory Droid › Defaults has the same 66 in its Model menu, which runs off the screen. Nothing hides a model. In the mocks you use 16 of Droid’s 66.

**A. A Models tab on the agent’s page** (from t3code Settings › Providers › Models): The agent’s page gets a Models tab after Defaults. It lists every model the agent offers, each with a switch: on, it’s in the picker (“Shown in picker”); off, it’s hidden (“Hidden from picker”) and its name dims. The list’s tools are topic 7, its star topic 9. The picker ends its models with Manage Models…, as the agent picker ends with Manage Agents…, which opens this tab.

## 2. What the picker does with hidden models: A. Gone, even from search

*Hiding models*

**Today:** Every model is in the picker, and search matches any of them. In every option the hidden models are the 50 you don’t use, chosen as topic 1 says; the mocks search for “gpt”, which matches 5 shown models and 15 hidden ones. These mocks leave favorites out.

**A. Gone, even from search** (from t3code model picker): Hidden models aren’t listed and search doesn’t find them, as t3code’s picker leaves them out. To use one, show it again in Settings (Manage Models…).

## 3. When the model in use is hidden: A. Listed while it’s in use

*Hiding models*

**Today:** Can’t happen today. With hiding, a thread can be on a model you hide later (here Opus 4.8, from before you hid the 4.x models), and the default for new threads (Defaults’ Model) can be one you hid. t3code moves its composer to the first shown model when its selection is hidden, and keeps custom models always shown.

**A. Listed while it’s in use** (from new): The thread’s model stays in the picker in its place, dimmed, tagged “Hidden”, with its check, until you pick another. A hidden default works the same in Defaults’ Model menu. The chip names it as always.

## 4. Models the agent adds later: A. Shown

*Hiding models*

**Today:** Agents change their lists: Droid adds a model every few weeks, and a plan can offer more. Once some are hidden, a model that wasn’t there when you chose is either shown or hidden by default. In the mocks Droid has just added Opus 5.6.

**A. Shown** (from t3code (keeps the hidden ones)): agentZ keeps the models you hid, as t3code keeps hiddenModels; anything new is in the picker until you hide it.

## 5. Across an agent’s accounts: A. Shared by all accounts, an account can keep its own

*Whose list it is*

**Today:** In the mocks Factory Droid has two accounts here, Personal and Work. Its Defaults and Environment tabs open with an Account menu: Shared first (the layers icon), what every account uses, then each account, whose tab starts with “Use shared settings”. Zed and t3code have no accounts: Zed keeps favorites per agent, t3code hidden models per provider. Favorites (topic 9) follow the same choice. The mocks filter the list to “opus”.

**A. Shared by all accounts, an account can keep its own** (from agentZ Defaults and Environment (Shared)): The Models tab gets Defaults’ Account menu, Shared first: Shared’s list is every account’s, and beside the menu a note names the accounts that keep their own. On an account, the tab starts with “Use shared settings”: on, it says what Shared has (“16 of 66 shown · 3 favorites”) and nothing more; off (“Work sets its own.”), the account gets a copy of Shared’s list to change, and its threads’ picker uses that copy. Turned back on, the copy goes.

**Note:** Picked in chat: one list shared by all accounts, with an account able to opt out and keep its own.

## 6. Across machines: A. Shared’s list is every machine’s

*Whose list it is*

**Today:** An agent’s page is one machine’s: the header’s machine picker opens Factory Droid on This Mac or Devbox 1. Accounts live on their machine, so Defaults’ Shared is that machine’s accounts’. Thread titles, Skills, MCP Servers and Storage have a Shared every machine follows until it sets its own. t3code syncs hidden models to every device. The mocks show topic 5’s A, on Shared.

**A. Shared’s list is every machine’s** (from t3code (synced)): agentZ keeps Shared’s list in the app and sends it to every machine, as it sends Thread titles’ Shared, so Shared on Devbox 1 is the same list. An account that keeps its own lives on one machine, so its list is that machine’s. A machine whose agent lacks a model just doesn’t list it.

## 7. Finding and changing many at once: A. Filter models + B. Counts + D. Groups: Favorites, All, Hidden from picker + E. Hide deprecated

*The Models list · pick any*

**Today:** There’s no list yet. t3code’s has a Filter models field once there are more than eight, a count line (“66 models · 3 favorites · 50 hidden”), one Disable all / Enable all button, and the rows in three groups: Favorites, All, and Hidden from picker. Pick any; each mock shows its tool alone.

**A. Filter models** (from t3code): A field over the list narrows it as you type, from nine models up; “No models match.” when nothing does. The rows keep their switches and stars.

**B. Counts** (from t3code): Beside the heading: “66 models · 3 favorites · 50 hidden”.

**D. Groups: Favorites, All, Hidden from picker** (from t3code): Favorites first, then the other shown models (All), then Hidden from picker, each in the agent’s order. A row moves to its group when you flip its switch or star.

**E. Hide deprecated** (from new): A Hide deprecated button hides every model the agent names “[Deprecated]” (Droid’s three).

## 8. The order of the models: C. Drag to reorder

*The Models list*

**Today:** The picker lists models in the agent’s order: Droid’s goes Auto Model, Fable, Opus, Sonnet, Haiku, GPT, Gemini, then the rest. t3code lets you move a model up or down within its group, in Settings. The mocks use topic 7’s groups and filter to “opus”.

**C. Drag to reorder** (from agentZ role chips): A grip at each row’s start drags it within its group, as the subthread roles’ chips are dragged.

## 9. How you mark a favorite: C. Only in Settings

*Favorites*

**Today:** There are no favorites. Zed’s model picker shows a star at a row’s end while you hover it (“Favorite Model”, filled in the accent color for “Unfavorite Model”). t3code’s picker has a star on every row (“Add to favorites”), and its Settings list starts each row with one. In every option but E, the Models list (topic 1’s A) has t3code’s star too. The mocks use Zed’s Favorite section (topic 10’s A) and hover Fable 5.1.

**C. Only in Settings** (from t3code Settings list): The picker shows the favorites, but you star them in the Models list.

## 10. Where favorites show in the picker: B. On top, once

*Favorites*

**Today:** There are none yet. Zed’s picker starts with a Favorite heading over them, then All (or the agent’s own groups) with every model, the favorites again among them. t3code’s picker opens on its Favorites view whenever you have some. The mocks use topic 2’s A (hidden models gone) and stars on hover (topic 9’s A).

**B. On top, once** (from t3code Settings groups): As A, but the favorites leave the list below.

## 11. Other agents’ favorites: A. Each agent’s own

*Favorites*

**Today:** A thread can switch agents between turns from the agent picker; the conversation is handed to the new agent. Zed keeps favorites per agent (for an ACP agent, per setting and value) and lists only the current agent’s. t3code’s favorites are global: its picker’s Favorites lists them from every provider, and picking one switches the provider. In the mocks Claude Agent’s favorites are Opus 5.5 and Sonnet 5.5, Codex’s is 6.1 Sol.

**A. Each agent’s own** (from Zed): The picker lists only this agent’s favorites. Changing agents stays in the agent picker.

## 12. Keys for the model picker: A. Both of Zed’s

*Favorites*

**Today:** No key opens the model picker or changes the model; hovering the chip shows the setting’s name and description. In Zed, ⌥⌘/ opens the model selector, ⌥⇥ moves to the next favorite, and the chip’s tooltip names both (the second only with favorites).

**A. Both of Zed’s** (from Zed agent::ToggleModelSelector, agent::CycleFavoriteModels): ⌥⌘/ opens the model picker; ⌥⇥ moves the thread to the next favorite, in their order, wrapping around. The chip’s tooltip lists both, as Zed’s does.

## 13. Reasoning Effort in a submenu: A. One row at the bottom, its levels beside it

*The picker*

**Today:** The picker lists the effort’s levels under the models, under a “Reasoning Effort” heading (composer-2, topic 2 A), so five more rows follow the models every time it opens. You asked for them in a submenu. agentZ’s agent picker already has submenus: an agent with accounts has a chevron at its row’s end, and its accounts open beside it on hover. Zed has the effort in a chip of its own beside the model’s (“Change Thinking Effort”), t3code in a menu of its own. Each mock opens the submenu to the left, where there’s room beside the composer.

**A. One row at the bottom, its levels beside it** (from agentZ agent picker’s account submenus): The levels give way to one row at the picker’s end: “Reasoning Effort”, then the current level and a chevron (“Extra High ›”). Hovering it, or → on it, opens the levels beside the picker, the current one checked; picking one closes both. The model’s other settings, such as Claude Agent’s Fast mode switch, stay rows of their own.

## 14. How tall the picker is: A. Eight rows

*The picker*

**Today:** The models list is at most 18rem tall, nine rows, and then scrolls. The effort’s levels under it add six rows more. With the effort in a submenu (topic 13’s A) the picker is already about 130px shorter. You asked for it a little smaller. The mocks show the whole picker, with topic 13’s A and Manage Models….

**A. Eight rows** (from new): The list is 16rem: eight models, then it scrolls. The picker is about 345px tall.

## 15. Where else the list applies: A. Defaults’ Model menu + B. The subthread roles’ Model menus + C. What agents are told

*Beyond the composer · pick any*

**Today:** Models are picked in two more places: Settings › Agents › an agent › Defaults (its Model menu, “Agent’s choice” first, then all 66) and Settings › Subthreads (each role’s pick has a Model submenu). Agents also learn every model an agent offers from agentZ’s orchestrator_capabilities, and pick one for the subthreads they start. The composer’s picker on the new thread screen is the same picker, so it follows anyway. Pick any.

**A. Defaults’ Model menu** (from t3code (hidden means hidden from pickers)): The menu lists Agent’s choice, then only the shown models. A hidden default stays listed as topic 3 decides.

**B. The subthread roles’ Model menus** (from new): A role’s pick offers Default, then the agent’s shown models; a pick already on a hidden model keeps it.

**C. What agents are told** (from new): orchestrator_capabilities lists only an agent’s shown models, so agents choose among those for their subthreads.
