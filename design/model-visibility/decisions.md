# Model Visibility and Favorites: design decisions

Picked on the design board (`design/model-visibility/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Where you choose the models the picker shows: Not decided yet

*Hiding models*

**Today:** The model chip beside Send (“Opus 5.5 · Extra High ⌄”) opens a picker of every model the agent offers, in the agent’s order: here Factory Droid’s 66, with a “Fast Mode” twin for many and three “[Deprecated]” ones. OpenCode offers 111 (“OpenCode Zen/…”, “GitLab Duo/…”), Claude Agent 4. “Search models…” narrows the list, and the effort’s levels follow under it. Settings › Agents › Factory Droid › Defaults has the same 66 in its Model menu, which runs off the screen. Nothing hides a model. In the mocks you use 16 of Droid’s 66.

## 2. What the picker does with hidden models: Not decided yet

*Hiding models*

**Today:** Every model is in the picker, and search matches any of them. In every option the hidden models are the 50 you don’t use, chosen as topic 1 says; the mocks search for “gpt”, which matches 5 shown models and 15 hidden ones. These mocks leave favorites out.

## 3. When the model in use is hidden: Not decided yet

*Hiding models*

**Today:** Can’t happen today. With hiding, a thread can be on a model you hide later (here Opus 4.8, from before you hid the 4.x models), and the default for new threads (Defaults’ Model) can be one you hid. t3code moves its composer to the first shown model when its selection is hidden, and keeps custom models always shown.

## 4. Models the agent adds later: Not decided yet

*Hiding models*

**Today:** Agents change their lists: Droid adds a model every few weeks, and a plan can offer more. Once some are hidden, a model that wasn’t there when you chose is either shown or hidden by default. In the mocks Droid has just added Opus 5.6.

## 5. Across an agent’s accounts: A. Shared by all accounts, an account can keep its own

*Whose list it is*

**Today:** In the mocks Factory Droid has two accounts here, Personal and Work. Its Defaults and Environment tabs open with an Account menu: Shared first (the layers icon), what every account uses, then each account, whose tab starts with “Use shared settings”. Zed and t3code have no accounts: Zed keeps favorites per agent, t3code hidden models per provider. Favorites (topic 9) follow the same choice. The mocks filter the list to “opus”.

**A. Shared by all accounts, an account can keep its own** (from agentZ Defaults and Environment (Shared)): The Models tab gets Defaults’ Account menu, Shared first: Shared’s list is every account’s, and beside the menu a note names the accounts that keep their own. On an account, the tab starts with “Use shared settings”: on, it says what Shared has (“16 of 66 shown · 3 favorites”) and nothing more; off (“Work sets its own.”), the account gets a copy of Shared’s list to change, and its threads’ picker uses that copy. Turned back on, the copy goes.

**Note:** Picked in chat: one list shared by all accounts, with an account able to opt out and keep its own.

## 6. Across machines: Not decided yet

*Whose list it is*

**Today:** An agent’s page is one machine’s: the header’s machine picker opens Factory Droid on This Mac or Devbox 1. Accounts live on their machine, so Defaults’ Shared is that machine’s accounts’. Thread titles, Skills, MCP Servers and Storage have a Shared every machine follows until it sets its own. t3code syncs hidden models to every device. The mocks show topic 5’s A, on Shared.

## 7. Finding and changing many at once: Not decided yet

*The Models list · pick any*

**Today:** There’s no list yet. t3code’s has a Filter models field once there are more than eight, a count line (“66 models · 3 favorites · 50 hidden”), one Disable all / Enable all button, and the rows in three groups: Favorites, All, and Hidden from picker. Pick any; each mock shows its tool alone.

## 8. The order of the models: Not decided yet

*The Models list*

**Today:** The picker lists models in the agent’s order: Droid’s goes Auto Model, Fable, Opus, Sonnet, Haiku, GPT, Gemini, then the rest. t3code lets you move a model up or down within its group, in Settings. The mocks use topic 7’s groups and filter to “opus”.

## 9. How you mark a favorite: Not decided yet

*Favorites*

**Today:** There are no favorites. Zed’s model picker shows a star at a row’s end while you hover it (“Favorite Model”, filled in the accent color for “Unfavorite Model”). t3code’s picker has a star on every row (“Add to favorites”), and its Settings list starts each row with one. In every option but E, the Models list (topic 1’s A) has t3code’s star too. The mocks use Zed’s Favorite section (topic 10’s A) and hover Fable 5.1.

## 10. Where favorites show in the picker: Not decided yet

*Favorites*

**Today:** There are none yet. Zed’s picker starts with a Favorite heading over them, then All (or the agent’s own groups) with every model, the favorites again among them. t3code’s picker opens on its Favorites view whenever you have some. The mocks use topic 2’s A (hidden models gone) and stars on hover (topic 9’s A).

## 11. Other agents’ favorites: Not decided yet

*Favorites*

**Today:** A thread can switch agents between turns from the agent picker; the conversation is handed to the new agent. Zed keeps favorites per agent (for an ACP agent, per setting and value) and lists only the current agent’s. t3code’s favorites are global: its picker’s Favorites lists them from every provider, and picking one switches the provider. In the mocks Claude Agent’s favorites are Opus 5.5 and Sonnet 5.5, Codex’s is 6.1 Sol.

## 12. Keys for the model picker: Not decided yet

*Favorites*

**Today:** No key opens the model picker or changes the model; hovering the chip shows the setting’s name and description. In Zed, ⌥⌘/ opens the model selector, ⌥⇥ moves to the next favorite, and the chip’s tooltip names both (the second only with favorites).

## 13. Reasoning Effort in a submenu: Not decided yet

*The picker*

**Today:** The picker lists the effort’s levels under the models, under a “Reasoning Effort” heading (composer-2, topic 2 A), so five more rows follow the models every time it opens. You asked for them in a submenu. agentZ’s agent picker already has submenus: an agent with accounts has a chevron at its row’s end, and its accounts open beside it on hover. Zed has the effort in a chip of its own beside the model’s (“Change Thinking Effort”), t3code in a menu of its own. Each mock opens the submenu to the left, where there’s room beside the composer.

## 14. How tall the picker is: Not decided yet

*The picker*

**Today:** The models list is at most 18rem tall, nine rows, and then scrolls. The effort’s levels under it add six rows more. With the effort in a submenu (topic 13’s A) the picker is already about 130px shorter. You asked for it a little smaller. The mocks show the whole picker, with topic 13’s A and Manage Models….

## 15. Where else the list applies: Not decided yet

*Beyond the composer · pick any*

**Today:** Models are picked in two more places: Settings › Agents › an agent › Defaults (its Model menu, “Agent’s choice” first, then all 66) and Settings › Subthreads (each role’s pick has a Model submenu). Agents also learn every model an agent offers from agentZ’s orchestrator_capabilities, and pick one for the subthreads they start. The composer’s picker on the new thread screen is the same picker, so it follows anyway. Pick any.
