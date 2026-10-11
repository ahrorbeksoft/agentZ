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

### Artifacts 2: the Artifacts button, a new artifacts page, documents and designs

Agents asked for designs publish five artifacts, one per design, or write a whole board page
themselves, and the artifacts list is hard to use with many artifacts across projects. In
design: `design/artifacts-2/` (it replaces the boards round). The user's requirements:

- No Artifacts shelf in the sidebar: an Artifacts button by the project picker, with every
  artifact and their count under All projects, the picked project's otherwise (chats' only
  under All projects). A thread lists only its own artifacts. Chats can publish.
- Two kinds for now: documents (Markdown only, read-only, no comments) and designs. Plain HTML
  pages stop being a kind.
- A better artifacts page, opened in the browser and styled like the app.
- Designs: agentZ draws the page; the agent gives only the variants (pictures or live pages)
  and the words. A design holds one question or several topics. Picks and comments are saved
  on the server with the design, and the agent can read them whenever it likes. Submit
  (optional) marks a design decided, and can send it to the thread.
- Design defaults in Settings (how many variants, today's version included, phone and/or
  desktop, pictures and/or live pages), given to the agent as defaults it may change for a
  project.
- Artifacts can be @-mentioned, this thread's first, then the project's, then anywhere's; a
  mention is a short reference, and the agent reads the artifact with its tools.
- Across machines, like threads: one list of every machine's artifacts, fetched when needed;
  agents read and mention other machines' artifacts; an artifact moves with its thread, and
  opens from any machine. Archived threads' artifacts stay in the lists, marked.
- The agent's tools are reshaped to match.
