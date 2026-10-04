# Dragging: design decisions

Picked on the design board (`design/dragging/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. The tab shows the result as you drag: Not decided yet

*Panes*

**Today:** Today a small label follows the pointer, the pane stays where it is, and the spot it would take is shaded (the screenshot). In all of these, the tab shows the result while you drag instead: the pane leaves its place, and the other panes move to make room for it where it would land, so what you see is what you get when you let go. The small preview of the pane follows the pointer. Pointing at its new place changes nothing; pointing at another pane moves it there (near an edge it splits that pane, in the middle the two swap); back over its own place, everything is as it was. Here npm run dev goes to Claude Code's right edge, swaps with it, swaps back and goes home. The options differ in what fills its place while you drag.

## 2. How the panes move to the new layout: Not decided yet

*Panes*

**Today:** Nothing moves today. The same drag as in the topic before, shown with its D (icon and name); only the way the panes get from one layout to the next differs.

## 3. The tab slides along the bar: B. Slides, raised

*Tabs*

**Today:** Today the tab's name follows the pointer in a small label and the tab under it is shaded; letting go puts it in that tab's place. In all of these, the tab itself moves instead, only sideways: it stays in the bar however far up or down the pointer goes. As its edge passes the middle of the next tab, that tab slides over into its old place, so the order changes while you drag, and letting go leaves it where it is. When there are more tabs than fit, holding it near an end scrolls the bar. Each one moves: server slides left past agents and back.

**B. Slides, raised** (from Zed dragged tab (a copy held where you grabbed it), kept in the bar): As A, and the tab is raised while you hold it: a lighter background, full-strength text and a shadow, so it stands apart from the tabs it passes.

## 4. Dragging a workspace row: Not decided yet

*Sidebar*

**Today:** The same small label follows the pointer, and the row it's over is shaded; dropping puts the dragged workspace in that row's place. Here api is dragged onto storefront.
