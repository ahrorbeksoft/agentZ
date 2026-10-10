# Backlog

Features and bugs to implement, in the order they were given. Each entry is the user's request
as given, not yet researched. Remove an entry once it's built.

## Bugs

### Linux: the fallback delete prompt cuts off its detail text

On Linux, GPUI's fallback prompt (the one shown for Delete… confirmations) cuts off its detail
text.

### Linux: Ctrl-, doesn't open Settings while a terminal has focus

On Linux, Ctrl-, does nothing while a terminal has focus; it should open Settings as it does
elsewhere.

### Stop doesn't stop some ACP agents' turns

Stop doesn't work on some ACP agents: Factory Droid's turn, for one, doesn't stop at all. Find a
more reliable way to stop a session's turn.

### The thread header shrinks the project's name too much

With a long thread title, the header's project name shrinks to a few letters ("agentZ" shows
as "age…"). Project names shouldn't shrink this much.
