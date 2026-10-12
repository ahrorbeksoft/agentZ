# Agent switching: design decisions

Picked in agentZ's own design board (the artifact "Switching agents and forking threads",
version 2), with the current app screenshotted beside the mocks. Each section is the spec for
that part: build the picked option as described, with the comments applied.

Decided before the mocks: a thread goes on with another agent, or another account of its
agent, in the same thread and folder, between turns. The new agent's next message brings what
it hasn't seen of the thread as a context handoff (t3code's), also for the same agent on
another account. Any finished reply can be forked into a new thread in the same folder, which
always starts with a handoff, and a fork can be merged back into its source. Continue with
Another Agent goes.

## 1. Changing the agent on a sent thread: C. Agent and account

Two pickers in the composer: the agent, and next to it the account (the account picker new
threads show under the composer). An agent with only one account shows no account picker.

## 2. After switching, before the next message: A. Nothing more

The composer just shows the new agent and its options. The conversation goes along without
any notice; the marker in the conversation (next topic) appears once the message is sent.

## 3. The switch in the conversation: A. Context handoff

t3code's divider: Context handoff, Claude Agent → Codex, with their icons.

## 4. Each reply's footer: C. Icon only, details on hover

Clock and time, then only the icon. Hovering it shows Agent, Account, Model and Effort in
rows.

## 5. Forking from a reply: A. Fork next to Copy

t3code's: a fork icon next to Copy under each finished reply, with the tooltip Fork from This
Response. Earlier replies show it on hover; the last reply always shows it.

**Comment:** instead of "Worked for", put a time icon and show how long it worked, plus a
generic agent icon that shows which agent, which account, which model and which effort.

## 6. The fork and its source: A. t3code's dividers

Pill dividers that open the other thread: Forked from conversation in the fork, Conversation
fork in the source.

## 7. Merge Back, in the fork: D. The source in the header

Like t3code, which lists a fork's source with a merge button: a pill with the source's name
in the fork's header, opening Open Source Thread and Merge Back.

## 8. Merge back, in the source: A. Attachment, then a divider

While waiting: an attachment above the composer, `<fork> · 2 messages`, with ×. Once sent: a
Merged back from `<fork>` divider before that message.

## 9. The limit and lost-history notices: B. Continue at, and Resend with Context

The limit notice keeps only Continue at 13:38: to go on with another account, pick it in the
composer's account picker and send, and its new session gets the conversation, the stopped
turn included. The lost-history notice keeps only its first sentence, and the waiting message
offers Resend with Context, which starts a new session with the conversation and sends that
message.
