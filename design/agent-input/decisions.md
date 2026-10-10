# Agents' requests for input: design decisions

Picked on the design board (`design/agent-input/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. A permission request: Not decided yet

*Permission to use a tool*

**Today:** When an agent asks before it uses a tool, the tool’s row stays, with its spinner, and the agent’s choices list under it as plain lines: a check for once, a double check for always, a red × for no. The agent names them, so there are three (Claude Agent: “Yes”, “Yes, and don’t ask again for npm test commands”, “No”), four (Codex) or six (Factory Droid: “Allow”, “Allow always”, three “Allow & auto-run (… risk)”, “Cancel”). Under the turn, “Awaiting Confirmation” shows before “Working for 23s”.

## 2. What a request shows: Not decided yet

*Permission to use a tool · pick any*

**Today:** The tool’s row is the same as a running one’s: a spinner, and the agent’s title. Codex’s edit says “Edited src/cart/total.ts +3 −2”, past tense, with its diff open under it. A command opens like any command, an MCP tool shows its name and server, and its input (the JSON) is behind “Input”. These mocks use A’s card from the topic before.

## 3. Saying no, and why: Not decided yet

*Permission to use a tool*

**Today:** A No is sent as the agent’s choice and nothing more. Some agents offer a No that waits for your words: Codex’s “No, and tell Codex what to do differently” and Claude Agent’s plan “No, keep planning”. agentZ sends it and the agent ends its turn; you then write in the composer, which has no hint that it’s waiting for that.

## 4. A plan to approve: Not decided yet

*Plans*

**Today:** Claude Agent’s plan comes as a permission request on a tool called “Approve Plan”: its row shows the plan as plain text in the code font, behind “Input”, and five choices under it: “Yes, clear context (34% used) and use auto mode”, “Yes, and use auto mode”, “Yes, and bypass permissions”, “Yes, manually approve edits”, “No, keep planning”. Once answered the row says “Exited Plan Mode” and the plan is folded away. Codex asks “Implement this plan?”, and Factory Droid’s spec mode “Proceed with implementation” in three levels.

## 5. A question with choices: Not decided yet

*Questions and forms*

**Today:** Claude Agent’s questions come as a form: a row “Asking for your input” with a spinner, then a card “Claude Agent is asking” with a ×. In it the question, its header (“Approach”) as a small label, each choice as a radio with its description under it (“(Recommended)” is part of the label), an “Other” field with a long hint, and “⏎ to submit”, Decline and Submit at the foot.

## 6. Several questions at once: Not decided yet

*Questions and forms*

**Today:** When the agent asks several questions in one go (Claude Agent asks up to four), the card lists them one after another: each one’s header, question, choices (radios for one answer, checkboxes for several) and its own Other field with its hint. Submit at the very end sends them all.

## 7. A form to fill in: Not decided yet

*Questions and forms · pick any*

**Today:** An MCP server can ask for details through the agent (an elicitation), as github does here before it creates an issue. Its tool’s row, “Create issue github”, spins over a card titled “Mock is asking”: the server’s message, then each field with its label above it (a checkbox, a number, checkboxes and radios in a row, text with its description). Submitting with a problem marks the field red with the reason under it.

## 8. A page to open: Not decided yet

*Pages and logins*

**Today:** When a server needs you in the browser (linear’s sign-in here), its row spins over a card “Mock wants you to open a page”: the server’s message, “Opens linear.app” with a lock, the whole address in a code box, and Decline and “Open linear.app ↗”. Once opened, the card stays the same until the server goes on.

## 9. When the agent needs a login: Not decided yet

*Pages and logins*

**Today:** When the agent answers a message with “authentication required”, the message is marked “Not sent” with Retry, a bar over the composer says “Mock needs a login” with “Log in…”, and the composer says “Log in to Mock to send a message”. Log in… opens the agent’s login panel, with its ways to log in.

## 10. After you answer: Not decided yet

*Every request*

**Today:** Once you answer, the card or the choices go away. A permission’s row carries on as the tool runs (“Ran npm test”); a question’s row stays “Asking for your input”, and a plan’s says “Exited Plan Mode”. Only the agent’s next words, if it writes them, say what you chose (“You chose “Yes”.”).

## 11. Requests from subagents, and with no row: Not decided yet

*Every request*

**Today:** A subagent’s request shows inside its row, which opens by itself to it: the subagent’s step (“Edit .env”) and its choices as lines. A request for a tool call the agent never showed gets a row of its own, with its choices under it. Under the turn, “Awaiting Confirmation” shows either way.

## 12. Answering from the keyboard: Not decided yet

*Every request · pick any*

**Today:** A form submits with ⏎ while one of its fields has focus (“⏎ to submit”). A permission’s choices and a question’s have no keys; nothing takes focus when a request comes, so the composer keeps it, and answering takes the mouse.

## 13. Showing that the agent waits: Not decided yet

*Every request · pick any*

**Today:** A thread waiting on you says so on its sidebar card: “Pending Approval” in yellow for a permission, “Awaiting Input” in purple for a question or form. In the thread, “Awaiting Confirmation” shows before “Working for 23s” under the turn, and the request is wherever it is in the conversation, maybe scrolled out of view.
