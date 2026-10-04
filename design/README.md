# Design boards

Before changing agentZ's UI, the user picks from designs on a local board: every part of the
change as a topic with a few mocked options, a comment box on each, and a note per topic. Their
picks are saved here and become the spec. The boards are plain HTML and JavaScript served by a
small Python server; nothing is published anywhere.

```sh
python3 design/server.py        # http://127.0.0.1:4477 lists every round
```

Start it detached from your session so it outlives it, and give the user the round's link
(`http://127.0.0.1:4477/<round>/`). Don't open it yourself: that would steal focus.

## Reading a round's decisions

Each round is a folder (`workspaces/`, …). As the user picks and comments, the server writes:

- `decisions.md`: the spec. For each topic, what it does today, the option(s) picked with their
  description and source, and the user's comments and notes. Build from this.
- `choices.json`: the raw picks, which the board loads back.

Comments refine a pick ("A, but hide it like herdr does"); read them as part of the spec. After
editing a round's topics, run `node design/board/decisions.js <round>` to rewrite `decisions.md`.

## Making a new round

1. Screenshot what's there now (see Testing in `AGENTS.md`) into `<round>/img/`.
2. Copy `workspaces/index.html` to `<round>/index.html`; set `data-area` (the folder's name),
   `data-title`, the title bar text, `window.ROUND_INTRO`, and its topic scripts.
3. Write the topics, one `TOPICS.push({...})` each:

   ```js
   TOPICS.push({
     id: 'rows', section: 'Sidebar', title: 'Workspace rows',
     size: 'narrow',          // card width: narrow (≈300px mocks), medium, wide
     type: 'multi',           // only for "pick any" feature lists; omit for "pick one"
     rec: 'A',                // your recommendation(s)
     now: 'What it does today…', nowImg: 'img/now-rows.png', issues: ['…'],
     options: [
       { key: 'A', name: 'Tidied two lines', from: 'herdr rows', desc: '…', good: '…', cost: '…',
         mock: () => frame(sidebar({ list: currentList() }), { w: 290, h: 470 }) },
       // … usually five
     ],
   });
   ```

   Say where each option comes from (Zed, t3code, herdr, cow) or mark it `new`.
4. Mocks are HTML strings built from `board/mock.js` (One Dark tokens at the app's sizes: the
   sidebar, rows, tabs, panes, terminals, menus, popovers, windows) and styled by
   `board/mock.css`. Mocks keep their real size and are zoomed to fit their card; a click shows
   one full size.
5. Check every mock renders: `node -e` with `vm` over `board/mock.js` and the topic files, calling
   each `mock()` (see `board/decisions.js`), then headless Chrome screenshots of each topic
   (`--headless=new --screenshot --timeout=4000`).
