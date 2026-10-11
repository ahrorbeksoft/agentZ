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

## Features

### Boards: design boards that agentZ draws from an agent's designs

Agents asked for designs publish five artifacts, one per design, or write a whole board page
themselves. Extend artifacts so the agent gives only the designs (a picture or a built page
each, with a name, description and the like) and agentZ draws the board around them, as the
local design board is: an intro, topics, picks, comments, a summary. Picks are kept on the
server, and the agent can read them whenever it likes. In design: `design/boards/`.
