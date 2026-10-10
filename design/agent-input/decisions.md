# Agents' requests for input: design decisions

Picked on the design board (`design/agent-input/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. A permission request: None of these

*Permission to use a tool*

**Today:** When an agent asks before it uses a tool, the tool’s row stays, with its spinner, and the agent’s choices list under it as plain lines: a check for once, a double check for always, a red × for no. The agent names them, so there are three (Claude Agent: “Yes”, “Yes, and don’t ask again for npm test commands”, “No”), four (Codex) or six (Factory Droid: “Allow”, “Allow always”, three “Allow & auto-run (… risk)”, “Cancel”). Under the turn, “Awaiting Confirmation” shows before “Working for 23s”.

## 2. What a request shows: None of these

*Permission to use a tool · pick any*

**Today:** The tool’s row is the same as a running one’s: a spinner, and the agent’s title. Codex’s edit says “Edited src/cart/total.ts +3 −2”, past tense, with its diff open under it. A command opens like any command, an MCP tool shows its name and server, and its input (the JSON) is behind “Input”. These mocks use A’s card from the topic before.

**Note:** keep it as is

## 3. Saying no, and why: D. Only the choice

*Permission to use a tool*

**Today:** A No is sent as the agent’s choice and nothing more. Some agents offer a No that waits for your words: Codex’s “No, and tell Codex what to do differently” and Claude Agent’s plan “No, keep planning”. agentZ sends it and the agent ends its turn; you then write in the composer, which has no hint that it’s waiting for that.

**D. Only the choice** (from Zed): As today and as Zed: the No is sent, the row says “Denied”, and whatever you want to say goes in the composer as usual.

## 4. A plan to approve: D. Rendered in the row, today’s choices

*Plans*

**Today:** Claude Agent’s plan comes as a permission request on a tool called “Approve Plan”: its row shows the plan as plain text in the code font, behind “Input”, and five choices under it: “Yes, clear context (34% used) and use auto mode”, “Yes, and use auto mode”, “Yes, and bypass permissions”, “Yes, manually approve edits”, “No, keep planning”. Once answered the row says “Exited Plan Mode” and the plan is folded away. Codex asks “Implement this plan?”, and Factory Droid’s spec mode “Proceed with implementation” in three levels.

**D. Rendered in the row, today’s choices** (from Zed): As Zed shows a tool’s content: the plan rendered as markdown under the “Approve Plan” row (not in the code font, not behind “Input”), and the choices as the permission topic picks. After you answer it folds into the row, which opens to it again.

## 5. A question with choices: B. Today’s card, tidied

*Questions and forms*

**Today:** Claude Agent’s questions come as a form: a row “Asking for your input” with a spinner, then a card “Claude Agent is asking” with a ×. In it the question, its header (“Approach”) as a small label, each choice as a radio with its description under it (“(Recommended)” is part of the label), an “Other” field with a long hint, and “⏎ to submit”, Decline and Submit at the foot.

**B. Today’s card, tidied** (from Zed (its elicitation card)): The card stays as it is, with radios and Submit, as Zed’s. The “Asking for your input” row goes, so the card is the one title. “Recommended” becomes a tag, and Other is a closed “Other…” line that opens its field when clicked.

## 6. Several questions at once: A. One at a time

*Questions and forms*

**Today:** When the agent asks several questions in one go (Claude Agent asks up to four), the card lists them one after another: each one’s header, question, choices (radios for one answer, checkboxes for several) and its own Other field with its hint. Submit at the very end sends them all.

**A. One at a time** (from t3code (its question steps)): The card shows one question, with “1 of 2” and Back at its top. A one-answer question goes on to the next when clicked (A in the topic before); several answers take Next. The last one’s button is Submit, which sends them all.

## 7. A form to fill in: B. Required fields marked

*Questions and forms · pick any*

**Today:** An MCP server can ask for details through the agent (an elicitation), as github does here before it creates an issue. Its tool’s row, “Create issue github”, spins over a card titled “Mock is asking”: the server’s message, then each field with its label above it (a checkbox, a number, checkboxes and radios in a row, text with its description). Submitting with a problem marks the field red with the reason under it.

**B. Required fields marked** (from Zed (its required fields)): A required field’s label ends in a red *, and Submit stays dim until each one is filled. A wrong value shows its reason when you leave the field, not only when you submit.

## 8. A page to open: A. The host first, the address folded

*Pages and logins*

**Today:** When a server needs you in the browser (linear’s sign-in here), its row spins over a card “Mock wants you to open a page”: the server’s message, “Opens linear.app” with a lock, the whole address in a code box, and Decline and “Open linear.app ↗”. Once opened, the card stays the same until the server goes on.

**A. The host first, the address folded** (from Zed (its URL elicitation), shortened): The card says who asks and why in one line (“linear needs you to sign in”), then the host big with its lock. The full address is folded behind “Show address”. Open, Copy link and Decline at the foot. Zed’s warning for a look-alike host stays.

## 9. When the agent needs a login: A. The ways to log in, in the bar

*Pages and logins*

**Today:** When the agent answers a message with “authentication required”, the message is marked “Not sent” with Retry, a bar over the composer says “Mock needs a login” with “Log in…”, and the composer says “Log in to Mock to send a message”. Log in… opens the agent’s login panel, with its ways to log in.

**A. The ways to log in, in the bar** (from Zed (its authentication callout)): The bar names the agent and offers each of its ways to log in as a button, as Zed’s “Authentication Required” callout does (“Log in with Claude”, “API key…”). Once logged in, your message is sent by itself.

## 10. After you answer: A. A line with your answer

*Every request*

**Today:** Once you answer, the card or the choices go away. A permission’s row carries on as the tool runs (“Ran npm test”); a question’s row stays “Asking for your input”, and a plan’s says “Exited Plan Mode”. Only the agent’s next words, if it writes them, say what you chose (“You chose “Yes”.”).

**A. A line with your answer** (from new): Under the row, one dim line says what you did, with its mark: “You allowed it once”, “You allowed npm test commands from now on”, “You denied it”, “You answered: Design round first”. A question’s row turns to “Asked: Approach”.

## 11. Requests from subagents, and with no row: A. At the end of the thread, saying who asks

*Every request*

**Today:** A subagent’s request shows inside its row, which opens by itself to it: the subagent’s step (“Edit .env”) and its choices as lines. A request for a tool call the agent never showed gets a row of its own, with its choices under it. Under the turn, “Awaiting Confirmation” shows either way.

**A. At the end of the thread, saying who asks** (from new): A subagent’s request shows at the end of the thread, as the agent’s would, in the same card, its header naming the subagent (“Find where the login view is drawn wants to edit a file”). The subagent’s row shows “Waiting for you” in place of its step.

## 12. Answering from the keyboard: A. Zed’s keys for permissions

*Every request · pick any*

**Today:** A form submits with ⏎ while one of its fields has focus (“⏎ to submit”). A permission’s choices and a question’s have no keys; nothing takes focus when a request comes, so the composer keeps it, and answering takes the mouse.

**A. Zed’s keys for permissions** (from Zed (its permission keys)): While the thread has focus, ⌘Y allows once, ⌘⌥Y picks the first “always” choice and ⌘⌥Z the first No, as in Zed. Each shows on its button.

## 13. Showing that the agent waits: A. A pill over the composer

*Every request · pick any*

**Today:** A thread waiting on you says so on its sidebar card: “Pending Approval” in yellow for a permission, “Awaiting Input” in purple for a question or form. In the thread, “Awaiting Confirmation” shows before “Working for 23s” under the turn, and the request is wherever it is in the conversation, maybe scrolled out of view.

**A. A pill over the composer** (from new): While a request is out of view, a pill over the composer says “Claude Agent is waiting for you” with a ↓; clicking it scrolls to the request.
