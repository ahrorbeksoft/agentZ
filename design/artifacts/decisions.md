# Artifacts: design decisions

Picked on the design board (`design/artifacts/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. A published page in the thread: Not decided yet

*In the thread*

**Today:** There are no artifacts yet. agentZ’s own tools say what they did on one line with the agentZ mark, as the Tool calls round picked: “Ran cargo test -p limiter”, “Started a terminal: npm run dev”, and rows that made a subthread, thread or terminal end in Open (screenshot, with the mock agent). Design boards are pages an agent writes in `design/` and serves with `design/server.py`; it writes the link in its reply, and you open it. Claude Code prints the page’s link after each publish; t3code shows its HTML page inside the thread, with “Open in panel” on hover.

## 2. When it’s published again: Not decided yet

*In the thread*

**Today:** Nothing yet. In Claude Code each publish is a new version at the same link, and a page that’s open updates; the link in an earlier message opens the latest. The mocks use topic 1’s card B.

## 3. Opening the browser by itself: Not decided yet

*In the thread*

**Today:** Nothing yet. Claude Code opens your browser on a page’s first publish (`CLAUDE_CODE_ARTIFACT_AUTO_OPEN=0` turns that off), and not when it’s published again. agentZ never brings windows to the front on its own today: an agent waiting shows in the sidebar and in a notification.

## 4. What they’re called: Not decided yet

*In the app*

**Today:** Not named yet; the backlog says the name isn’t decided. Claude Code and claude.ai call them artifacts, the word the backlog uses; t3code calls its pages HTML renders.

## 5. Where they’re listed: Not decided yet

*In the app · pick any*

**Today:** Nothing lists them yet. The sidebar has shelves under the cards: Workspaces (threads started in panes) and Archived, each closed at first with its count. Claude Code shows a ⧉ pill under the prompt with the session’s artifact (or their count) and lists all of yours with `/artifacts`, this session’s first; claude.ai has a gallery of every one. The mocks say “artifacts”, the name topic’s A.

## 6. Narrowing the list: Not decided yet

*In the app · pick any*

**Today:** The sidebar shows the project picked in the title bar (or All projects), and its search finds threads by title, archived ones and Workspaces threads included. Artifacts belong to no project, only to the thread or chat that published them. Claude Code’s `/artifacts` puts “This session” first. The mocks use the shelf from the topic before.

## 7. The page’s header: Not decided yet

*The page in the browser*

**Today:** There are no artifact pages yet. A design board, the nearest thing, has its own top bar: the round’s title, how many topics are decided, “Saved”, Only undecided and Summary (screenshot). Claude Code’s page has a thin bar over the page: a button to all your artifacts, the title with a menu, Share and your avatar; Share has the version picker. Claude Design’s Export menu holds every format and “Send to Claude Code”.

## 8. Earlier versions: Not decided yet

*The page in the browser*

**Today:** Nothing yet. Each publish of an artifact is a version (the backlog). Claude Code keeps each publish as a version at the same link, picked from its Share menu (“Sharing version 2”), with version history to restore one. The mocks use the header topic’s A.

## 9. Export, and the agent’s files: Not decided yet

*The page in the browser*

**Today:** Nothing yet. The backlog: save the HTML or Markdown, print or save as PDF with the browser’s print, copy the source, and files the agent made with its own tools (Word, PowerPoint, CSV) offered as downloads. Claude Design’s Export menu lists each format; Claude Code’s pages offer a file only through a button the page itself has. The mocks use the header topic’s A.

## 10. The app’s theme on the page: Not decided yet

*The page in the browser*

**Today:** Nothing yet. The backlog: pages use the app’s theme, its colors and fonts. agentZ has System, Light and Dark, with one theme for each (yours is JetBrains Dark). t3code gives its pages the theme as CSS variables (`--background`, `--foreground`, `--border`, `--accent`, `--font-sans`, chart colors…) before the first paint, and sends them again when you change it; a page opened outside the app follows the system’s light or dark. Claude Code’s design skill picks colors itself, or a design system written in CLAUDE.md.

## 11. Sending back to the thread: Not decided yet

*The page in the browser*

**Today:** A design board, the page this generalizes, saves your picks and comments as you go into `choices.json` and `decisions.md` beside it; then you tell the agent in its thread to read them. Claude Code’s pages can only give you text to paste (“Copy as prompt”). Claude Design sends a design to Claude Code from its Export menu. t3code’s pages talk to the app (the theme, their height, links) but send nothing to the agent.

## 12. When it’s published again while open: Not decided yet

*The page in the browser*

**Today:** Nothing yet. Claude Code’s open pages update in place when the agent publishes again, for a page the agent keeps up to date as it works (a checklist, a timeline). A design board shows a round’s latest when you reload it.

## 13. Markdown pages: Not decided yet

*The page in the browser*

**Today:** Nothing yet. An artifact can be a .md file. Claude Code shows one as “a styled document page with syntax-highlighted code”. agentZ draws Markdown in threads with Zed’s markdown: 14 px text, its headings, code blocks with the theme’s colors. The mocks use the header topic’s A.

## 14. Deleting the thread or chat that published it: Not decided yet

*When the thread goes*

**Today:** Undecided in the backlog. Deleting a thread asks first, “Delete “Rate limiter tests”? The thread and its conversation will be removed…” (screenshot); archiving keeps everything. An artifact records only the thread or chat that published it. Claude Code’s artifacts live in your claude.ai account apart from sessions, so they stay; t3code keeps its pages with the thread, so they go with it.
