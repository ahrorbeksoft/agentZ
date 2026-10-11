# Artifacts 2: design decisions

Picked on the design board (`design/artifacts-2/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. The button by the project picker: B. “Artifacts” and the count

*The Artifacts button*

**Today:** Artifacts are listed in an Artifacts shelf in the sidebar, above Archived, closed at first with its count: the picked project’s artifacts and every chat’s, or every one under All projects (screenshot). Open, it has a row per artifact (its kind, title, version, where it came from and when), then All artifacts…. The title bar has the sidebar button, the project picker, and Agents · Workspaces. Decided: the shelf goes, and a button by the picker takes its place, counting every artifact under All projects and the picked project’s otherwise (chats’ only under All projects).

**B. “Artifacts” and the count** (from new, a button like the picker beside it): The icon, the word Artifacts and the count, drawn as the picker is (no border, a hover), right after it.

## 2. What the button opens: A. A list under it + also C. The artifacts page

*The Artifacts button*

**Today:** The shelf opens in place in the sidebar, as a list (screenshot: under All projects); a click on a row opens it in the browser, and All artifacts… at the end opens the All artifacts page. The mocks have storefront picked: 7 artifacts from its threads, two of them archived.

**A. A list under it** (from agentZ’s menus (the project picker’s)): A popover under the button: a search, All · Documents · Designs, then rows grouped by the thread they came from (an archived thread says so), each with its kind, title and a design’s state (Waiting for you, 2 of 3 decided, Decided), or when. A click opens it in the browser; “Open the artifacts page” ends the list. The arrow keys and Enter work, as in the picker.

**Also take from C. The artifacts page** (from the first round’s All artifacts): The button opens the artifacts page in the browser, at the picked project (topic 4); nothing opens in the app.

## 3. A thread’s artifacts: A. The header’s pill, only this thread’s

*The Artifacts button*

**Today:** Once a thread has published, its header shows a pill before the branch: the artifact’s title, or with several their count, “2 artifacts”, which opens a menu of the thread’s artifacts and All artifacts… (screenshot). Each publish is also a card in the conversation. Decided: a thread lists only its own artifacts.

**A. The header’s pill, only this thread’s** (from the first round (topic 5 B)): As today, but the menu lists only this thread’s artifacts, each with a design’s state, and no All artifacts… (the Artifacts button has the rest). With one artifact, the pill is its title and opens it.

## 4. The page’s layout: A. Places on the left, cards by day

*The artifacts page*

**Today:** All artifacts is a page the server serves at `/`: a card per artifact with its kind’s icon, title, “Page · v1 · 5m ago” and where it came from, newest first, with All · Threads · Chats and a search (screenshot). No pictures, no places, every machine’s artifacts in one grid. Decided: a new page, the homepage, in the app’s theme.

**A. Places on the left, cards by day** (from claude.ai’s artifacts page, with the app’s sidebar): The page’s bar (agentZ’s mark, Artifacts, the search), then places on the left with their counts: All artifacts, Waiting for you (designs not submitted), each project and Chats. On the right, a card per artifact with its picture (a design’s variants, a document’s first lines), its state over it, grouped by day: Today, Yesterday, This week, Earlier.

## 5. Finding one among many: A. Search their words too

*The artifacts page · pick any*

**Today:** All artifacts’ search matches titles, and All · Threads · Chats narrows by where they came from. The shelf has no search. The mocks use topic 4’s A.

**A. Search their words too** (from new): The search finds words inside documents and designs (their topics’ and variants’ words, and your comments), showing the matching line under each title with the words marked.

## 6. From an archived thread: A. An Archived tag

*The artifacts page*

**Today:** Archiving a thread keeps its artifacts, and nothing marks them: the shelf and All artifacts list them like the rest. Deleting a thread keeps them too, “from a deleted thread” (the first round’s topic 14). Decided: an archived thread’s artifacts stay in the lists, marked as from an archived thread. The mocks have storefront picked: Write the 0.9 release notes and Add the cart drawer are archived.

**A. An Archived tag** (from agentZ’s Archived shelf): The card says Archived beside where it came from, with the archive icon; otherwise it’s like the rest.

## 7. Other machines’ artifacts: A. The machine on its cards

*The artifacts page*

**Today:** Each machine’s server keeps its own artifacts; the app gathers every connected machine’s into the shelf and All artifacts, and opens another machine’s through this Mac (`/m/devbox1/a/…`). Neither says which machine one is on, and an offline machine’s are simply missing. Decided, so not shown here: an artifact moves with its thread when the thread goes on on another machine, and the same artifact opens from any machine. In the mocks, api is on Devbox 1.

**A. The machine on its cards** (from agentZ’s thread cards (the machine icon)): A card from another machine names it beside where it came from, as thread cards show theirs. When a machine isn’t connected, a line over the cards says so, and its artifacts aren’t listed.

## 8. A document’s page: A. Today’s column, without Send to thread

*Documents*

**Today:** A Markdown artifact opens as a document page (the first round’s topic 13 B): 15 px text with room between lines, a larger title, ruled tables, in a 720 px column, under the page’s bar with home, the title, the version menu, where it came from, Files, Export and Send to thread (screenshot). Decided: a document is Markdown only, and read-only: no comments.

**A. Today’s column, without Send to thread** (from the first round (topic 13 B)): As today, but the bar has only the version menu, where it came from, and Export (Markdown, Print or Save as PDF, Copy Source). To talk about it, mention it in the thread (topic 20).

## 9. A design with several topics: A. A list on the left

*Designs*

**Today:** There are no designs yet: an agent that wants you to pick writes a whole page itself, or publishes each variant as its own artifact. The design board, the model here, lists a round’s topics on the left by section, each with what you picked (a green letter) and a dot when there’s a comment; ← → move between them, and Summary and How this works end the list. Most designs will have one topic: a few variants of one thing. The mocks’ bigger design, Pricing, has three: the plans page, the plan picker in checkout, and the yearly discount.

**A. A list on the left** (from the design board): Topics on the left with what you picked, Summary at the end, on every design, even one with a single topic.

## 10. How the variants are laid out: A. Cards in a grid

*Designs*

**Today:** The design board puts a topic’s options in cards, in columns as wide as the topic asks: each with its letter, name, description, Good and Cost, the mock, Pick, + Also and a comment (screenshot: the first artifacts round). The mocks have five variants at 1280×800 in a 1280 px browser.

**A. Cards in a grid** (from the design board): Three cards across (two in a narrow window), each with everything the design board’s cards have. A click on a picture shows it up close (topic 12).

## 11. Today’s version: B. Today as a variant to keep

*Designs*

**Today:** An agent says what’s there today in its own words, if at all. The design board opens each topic with a Today box: what’s there now, with a screenshot and its problems (topic 10’s screenshot). Decided: including today’s version is one of the design defaults (topic 21). In the mocks, today’s plans page is a plain table.

**B. Today as a variant to keep** (from new): Today’s version is the first card, marked Today, with its words, Good and Cost; its button says Keep today’s, and + Also on a variant takes parts of it into today’s.

## 12. One variant up close: A. Full size over the design

*Designs*

**Today:** On the design board, a click on a mock opens it full size over the board with its caption; Esc or a click closes it. There are no arrows to the next one, and you pick and comment only on the card. A variant published as its own artifact today opens as a whole page, with nothing to pick.

**A. Full size over the design** (from the design board): A click shows the picture (or runs the page) at full size over the design, its name under it. ← → go to the next variant; Esc closes.

## 13. Phone and desktop: B. A switch for the whole design

*Designs*

**Today:** Nothing for sizes. The five variants are desktop pages, 1280 px wide; a phone version would be another page. The design board draws each mock at one size, and a round that cares about narrow windows mocks them as separate options. Decided: phone, desktop or both is one of the design defaults (topic 21). In the mocks, the agent made phone versions of A and D.

**B. A switch for the whole design** (from new, like the browser’s device toolbar): With phone versions in the design, its bar has Desktop · Phone, which swaps every card; phone pictures are laid out five across. A variant without one says “No phone version”.

## 14. Pictures and live pages: C. A picture, a page, or both

*Designs*

**Today:** An artifact is one self-contained file, a page or a document, 16 MiB at most, with its images embedded. Each of the five demo variants is a whole page built with the site’s own styles (screenshot: variant A published as its own page). On the design board, an option is a mock drawn in HTML at its real size and zoomed to fit its card. Decided: the agent gives pictures or live pages, and which is one of the design defaults (topic 21).

**C. A picture, a page, or both** (from new): Each variant is a picture, a live page, or both. The card shows the picture (or the page, if that’s all there is); a variant with a page has “Open page”, which runs it full size up close (topic 12) to scroll and click through.

## 15. What the page says about saving: A. Saved

*Picks and Submit*

**Today:** There are no designs yet. The design board’s server writes your picks to `choices.json` and `decisions.md` as you type, and its bar says “Saved”. An artifact page runs sandboxed with no storage, so a design an agent writes as a page keeps your picks only in its tab (“Kept until you reload”, screenshot), and `agentz_artifact_read` gives the agent the page’s source, not your picks. Decided: picks and comments are saved on the server with the design, and the agent can read them whenever it likes.

**A. Saved** (from the design board): The server keeps the picks with the design as you go, and the bar says “Saved”. A reload, another browser, or the design opened from another machine shows them.

## 16. Submit: A. Submit in the bar

*Picks and Submit*

**Today:** Every artifact page has Send to thread in its bar: a box with what the page gave (its `window.agentzPicks()`) and a note, and Send puts both in the thread as your message (screenshot: a design page, which gives nothing). Nothing marks a page as done. Decided: Submit, which you don’t have to use, marks a design decided and can also send it to the thread.

**A. Submit in the bar** (from artifacts (Send to thread)): Submit in the bar opens a box with your picks, “Also send to Design the plans page” (on at first) and a note. Afterwards the design is Decided: in the bar, on its card in the thread, in the Artifacts button’s list and on the artifacts page. Changing a pick brings Submit back.

## 17. What Submit sends to the thread: B. A short message; the agent reads the rest

*Picks and Submit*

**Today:** Send to thread puts the page’s text and your note in the thread as your message, behind a chip naming the page and its version; the agent gets the text as it is. With “Also send” on, Submit (topic 16) does the sending here.

**B. A short message; the agent reads the rest** (from new): Your message says what you picked and your note, under the chip. The agent gets a short reference to the design, reads the picks and comments with `agentz_artifact_read` (topic 24), and the thread shows it did.

## 18. The design in the thread: A. The card a document has

*Picks and Submit*

**Today:** A publish shows as a card where the agent published: its kind’s tile, the title, “Page · v1 · published 2m ago”, Open and a menu (the first round’s topic 1 B). The newest publish of an artifact is the card; earlier ones become lines. The thread’s header lists the thread’s artifacts (topic 3).

**A. The card a document has** (from the first round (topic 1 B)): The same card with a design’s tile: “Design · 5 variants · v1 · published 2m ago”.

## 19. Published again: A. Picks carry over

*Picks and Submit*

**Today:** Each publish is a new version at the same link, and an open page gets a bar, “v2 was just published. Show v2” (the first round’s topic 12 B); the version menu has the earlier ones. In the mocks, Claude Agent published v2 after you submitted: B changed after your comment, and D is gone.

**A. Picks carry over** (from new): Picks and comments move to the new version by variant name. A changed variant is marked Changed and a new one New; a variant that’s gone takes its pick with it, and the banner says so. A submitted design waits for you again: Submit sends v2’s picks.

## 20. Artifacts in the @ menu: A. One Artifacts group

*Mentions*

**Today:** In a thread, @ lists Threads and Files (in a chat, Projects and Threads), each row with its icon, name and a hint (screenshot). A thread mention sends that thread’s conversation with your message, behind a chip. Artifacts can’t be mentioned; an agent finds one with `agentz_artifact_list`. Decided: artifacts can be mentioned, this thread’s first, then the project’s, then anywhere’s; a mention is a short reference, and the agent reads the artifact with its tools, from any machine.

**A. One Artifacts group** (from agentZ’s @ menu (a group per kind)): An Artifacts group before Threads and Files: this thread’s artifacts, then the project’s, then anywhere’s, each row’s hint saying where it’s from. Typing filters it, as the other groups.

## 21. Design defaults: B. A, and a project’s own

*Settings and the agent*

**Today:** Settings › General › Artifacts has one row, Open in the browser: when a page an agent publishes opens by itself (screenshot). Nothing about designs. Decided: the design defaults are how many variants, whether today’s version is one of them, phone and/or desktop, and pictures and/or live pages. They’re given to the agent as defaults, which it may change for a project, not as rules.

**B. A, and a project’s own** (from agentZ’s Settings › Projects): As A, and each project’s page in Settings › Projects has Design defaults: “Use General’s” on at first; off, the same four rows for that project.

## 22. The agent’s tools: A. A publish tool per kind

*Settings and the agent*

**Today:** Every thread’s and chat’s agent has three: `agentz_artifact_publish` (one `.html` page or `.md` document by its path, a title, files to offer as downloads, and the artifact to add a version to), `agentz_artifact_list` (its project’s or the chats’, or every one with `all`) and `agentz_artifact_read` (an artifact’s source). They see only their machine’s. Decided: two kinds, documents and designs; the agent lists and reads every machine’s artifacts, and an artifact moves with its thread.

**A. A publish tool per kind** (from agentZ’s tools (a tool per thing it does)): `agentz_document_publish` takes a Markdown file; `agentz_design_publish` takes the design (topic 23). `agentz_artifact_list` lists the thread’s, the project’s or all, every machine’s, with a design’s state; `agentz_artifact_read` reads any machine’s.

## 23. How the agent hands over a design: B. The tool’s arguments

*Settings and the agent*

**Today:** An agent makes a design one of two ways today. It writes the whole thing as one page (topics, cards, picks, comments, a summary, and `window.agentzPicks` for Send to thread) and publishes it with `agentz_artifact_publish` by its path: hundreds of lines each time, different every time. Or it publishes each variant as its own artifact, as an agent in another project did with five: five unrelated pages in All artifacts (screenshot). Whichever is picked, the tool’s description ends with the design defaults (topic 21).

**B. The tool’s arguments** (from agentZ’s tools (as agentz_artifact_publish)): `agentz_design_publish` takes the design itself: the title, an intro, and topics, each with today’s version and its variants (a name, a picture and/or a page, a description, Good and Cost), and the one the agent recommends. Its schema lists every field, so the agent fills it in right the first time, and agentZ checks it before publishing. The answer is the design’s link.

## 24. What the agent reads back: B. As text, like decisions.md

*Settings and the agent*

**Today:** `agentz_artifact_read` gives an artifact’s source by its id and version: a page’s HTML or a document’s Markdown. A page’s picks live only in its tab, so the agent can’t read them. On the design board, the agent reads `decisions.md`, made from `choices.json`: a section per topic with the pick, its description and the comments. The mocks read Pricing, a design of three topics with two decided.

**B. As text, like decisions.md** (from the design board’s decisions.md): A heading per topic with the pick by letter and name, each picked variant’s description, and the comments under it; an undecided topic says so. The top says whether it’s submitted.

**Comment on B:** how how. can they read the design then?
