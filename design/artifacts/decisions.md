# Artifacts: design decisions

Picked on the design board (`design/artifacts/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. A published page in the thread: B. A card with its title, version and Open

*In the thread*

**Today:** There are no artifacts yet. agentZ’s own tools say what they did on one line with the agentZ mark, as the Tool calls round picked: “Ran cargo test -p limiter”, “Started a terminal: npm run dev”, and rows that made a subthread, thread or terminal end in Open (screenshot, with the mock agent). Design boards are pages an agent writes in `design/` and serves with `design/server.py`; it writes the link in its reply, and you open it. Claude Code prints the page’s link after each publish; t3code shows its HTML page inside the thread, with “Open in panel” on hover.

**B. A card with its title, version and Open** (from Claude Code (the link it prints after each publish), as a card): A card in the conversation where the agent published: a tile by kind (a page, a document), the title, then “Page · v1 · published 2m ago”, an Open button, and a ⋯ menu (Copy Link, Export, Show in List). Clicking anywhere on it opens it too.

## 2. When it’s published again: D. The card moves to the latest

*In the thread*

**Today:** Nothing yet. In Claude Code each publish is a new version at the same link, and a page that’s open updates; the link in an earlier message opens the latest. The mocks use topic 1’s card B.

**D. The card moves to the latest** (from new): Only the newest publish of a page shows as the card; each earlier one becomes a line, “Published an artifact: Checkout layouts · v1”, which opens that version.

## 3. Opening the browser by itself: C. Only when you’re in that thread

*In the thread*

**Today:** Nothing yet. Claude Code opens your browser on a page’s first publish (`CLAUDE_CODE_ARTIFACT_AUTO_OPEN=0` turns that off), and not when it’s published again. agentZ never brings windows to the front on its own today: an agent waiting shows in the sidebar and in a notification.

**C. Only when you’re in that thread** (from new): As A, but only when the thread is on screen and agentZ is in front, so you were watching it. Otherwise it waits for Open, as B.

**Comment on C:** make it optional in the settings with all the choices

## 4. What they’re called: A. Artifacts

*In the app*

**Today:** Not named yet; the backlog says the name isn’t decided. Claude Code and claude.ai call them artifacts, the word the backlog uses; t3code calls its pages HTML renders.

**A. Artifacts** (from Claude Code): “Artifacts”, “Published an artifact”, “All artifacts”: Claude’s word, which many already know from Claude Code and claude.ai.

## 5. Where they’re listed: B. A pill in the thread’s header + A. A shelf in the sidebar + D. All of them on a page in the browser

*In the app · pick any*

**Today:** Nothing lists them yet. The sidebar has shelves under the cards: Workspaces (threads started in panes) and Archived, each closed at first with its count. Claude Code shows a ⧉ pill under the prompt with the session’s artifact (or their count) and lists all of yours with `/artifacts`, this session’s first; claude.ai has a gallery of every one. The mocks say “artifacts”, the name topic’s A.

**B. A pill in the thread’s header** (from Claude Code (the ⧉ pill under the prompt)): Once a thread has published, its header shows the page (“Checkout layouts”) or the count (“2 artifacts”) before the branch. A click opens it, or with several, a list of the thread’s pages with “All artifacts…” at the end.

**A. A shelf in the sidebar** (from agentZ’s Workspaces and Archived shelves): An “Artifacts (4)” shelf above Archived, closed at first, its state kept like the Workspaces shelf’s. Open, a row per page, newest first: its kind, title, version, where it came from (“storefront › Add the checkout page”, or “Chat · …”) and when. A click opens it in the browser; its menu has Open, Show Thread, Copy Link and Delete….

**D. All of them on a page in the browser** (from Claude Code (the gallery on claude.ai)): agentz-server serves an “All artifacts” page, reached from each page’s header (its first button) and from the app (the command palette’s “artifacts: show all”, and “All artifacts…” in B). Cards with a picture (topic 1 C’s), title, version, where from and when; All / Threads / Chats and a search.

## 6. Narrowing the list: A. Follows the project picker

*In the app · pick any*

**Today:** The sidebar shows the project picked in the title bar (or All projects), and its search finds threads by title, archived ones and Workspaces threads included. Artifacts belong to no project, only to the thread or chat that published them. Claude Code’s `/artifacts` puts “This session” first. The mocks use the shelf from the topic before.

**A. Follows the project picker** (from agentZ’s sidebar): With a project picked in the title bar, the list shows the pages its threads published, and chats’ pages, which belong to no project. All projects shows every one.

## 7. The page’s header: A. A thin bar with everything on it

*The page in the browser*

**Today:** There are no artifact pages yet. A design board, the nearest thing, has its own top bar: the round’s title, how many topics are decided, “Saved”, Only undecided and Summary (screenshot). Claude Code’s page has a thin bar over the page: a button to all your artifacts, the title with a menu, Share and your avatar; Share has the version picker. Claude Design’s Export menu holds every format and “Send to Claude Code”.

**A. A thin bar with everything on it** (from Claude Code (its page header)): A 44 px bar in the app’s colors: All artifacts (home), the title, its version with a menu (topic 8), the agent’s icon and where it came from (“storefront › Add the checkout page”, a link that opens the thread in agentZ), then Export (topic 9) and Send to thread (topic 11). The page is below it.

## 8. Earlier versions: A. A menu on the version

*The page in the browser*

**Today:** Nothing yet. Each publish of an artifact is a version (the backlog). Claude Code keeps each publish as a version at the same link, picked from its Share menu (“Sharing version 2”), with version history to restore one. The mocks use the header topic’s A.

**A. A menu on the version** (from Claude Code (its version picker)): The version in the header (“v3”) opens a menu of every version with when it was published, the latest first. An earlier one shows under a banner, “This is v2, from 14 minutes ago. The latest is v3. Show v3”, and the header’s version reads “v2 of 3” in the accent. Each version has its own link.

## 9. Export, and the agent’s files: B. Export, and Files apart

*The page in the browser*

**Today:** Nothing yet. The backlog: save the HTML or Markdown, print or save as PDF with the browser’s print, copy the source, and files the agent made with its own tools (Word, PowerPoint, CSV) offered as downloads. Claude Design’s Export menu lists each format; Claude Code’s pages offer a file only through a button the page itself has. The mocks use the header topic’s A.

**B. Export, and Files apart** (from new): Export holds A’s first three; a “Files 2” button beside it, shown only when the agent attached files, lists them.

## 10. The app’s theme on the page: A. The app’s theme, live

*The page in the browser*

**Today:** Nothing yet. The backlog: pages use the app’s theme, its colors and fonts. agentZ has System, Light and Dark, with one theme for each (yours is JetBrains Dark). t3code gives its pages the theme as CSS variables (`--background`, `--foreground`, `--border`, `--accent`, `--font-sans`, chart colors…) before the first paint, and sends them again when you change it; a page opened outside the app follows the system’s light or dark. Claude Code’s design skill picks colors itself, or a design system written in CLAUDE.md.

**A. The app’s theme, live** (from t3code (its theme as CSS variables)): The server puts the app’s theme on each page as CSS variables, which the header and Markdown pages use and the agent’s skill tells it to use. Changing the theme or the mode in agentZ changes open pages at once. A page that sets its own colors keeps them.

## 11. Sending back to the thread: A. Send to thread, with a look first

*The page in the browser*

**Today:** A design board, the page this generalizes, saves your picks and comments as you go into `choices.json` and `decisions.md` beside it; then you tell the agent in its thread to read them. Claude Code’s pages can only give you text to paste (“Copy as prompt”). Claude Design sends a design to Claude Code from its Export menu. t3code’s pages talk to the app (the theme, their height, links) but send nothing to the agent.

**A. Send to thread, with a look first** (from the design board’s saving, and Claude Design’s Send to Claude Code): The page gives what you picked or wrote (the agent’s page says what, through agentZ’s script on it). Send to thread opens a box with that text and a note field; Send puts it in the thread as your message, with a chip naming the page and version. If the agent is working, it joins the thread’s queue. The page says “Sent 2m ago”.

## 12. When it’s published again while open: B. A bar to show it

*The page in the browser*

**Today:** Nothing yet. Claude Code’s open pages update in place when the agent publishes again, for a page the agent keeps up to date as it works (a checklist, a timeline). A design board shows a round’s latest when you reload it.

**B. A bar to show it** (from new): The page stays on v3 with a bar over it: “v4 was just published. Show v4”.

## 13. Markdown pages: B. A document page

*The page in the browser*

**Today:** Nothing yet. An artifact can be a .md file. Claude Code shows one as “a styled document page with syntax-highlighted code”. agentZ draws Markdown in threads with Zed’s markdown: 14 px text, its headings, code blocks with the theme’s colors. The mocks use the header topic’s A.

**B. A document page** (from Claude Code (its styled document pages)): A page for reading: 15 px text with more space between lines, a larger title, code in the theme’s syntax colors, ruled tables, in a 720 px column. Prints well.

## 14. Deleting the thread or chat that published it: A. Kept

*When the thread goes*

**Today:** Undecided in the backlog. Deleting a thread asks first, “Delete “Rate limiter tests”? The thread and its conversation will be removed…” (screenshot); archiving keeps everything. An artifact records only the thread or chat that published it. Claude Code’s artifacts live in your claude.ai account apart from sessions, so they stay; t3code keeps its pages with the thread, so they go with it.

**A. Kept** (from Claude Code (artifacts apart from sessions)): The thread goes, its pages stay. The list says “from a deleted thread” where the thread was, and the header no longer links to it. Delete one from the list’s menu.
