# Dragging: design decisions

Picked on the design board (`design/dragging/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. What follows the pointer when you drag a pane: Not decided yet

*Panes*

**Today:** Dragging a pane's header carries a small label with its title ("Shell"). It used to stay where the label started, far from the pointer when you grabbed the header by its far end (the screenshot); since the last fix it sits just past the pointer. The half it would split off, or all of the pane for a swap, is shaded.

## 2. The pane you're moving, while you drag: Not decided yet

*Panes*

**Today:** It stays exactly as it was, focused (clicking its header to start the drag focuses it). Shown here with the small preview (B above).

## 3. Where the pane will land: Not decided yet

*Panes*

**Today:** As in Zed: near an edge (a fifth of the pane's shorter side), the half the dragged pane would take is shaded gray; in the middle, all of the pane is, and dropping swaps the two. Shown here with the small preview.

## 4. What follows the pointer when you drag a tab: Not decided yet

*Tabs*

**Today:** The same small label as for panes follows the pointer, and the tab it's over is shaded; dropping puts the dragged tab in that tab's place. Here server (docker compose up beside a shell) is dragged onto agents.

## 5. Where the tab will land: Not decided yet

*Tabs*

**Today:** The tab under the pointer is shaded gray, and the dragged tab takes its place. The tab you're moving stays as it is. Shown here with the small preview of the tab.

## 6. Dragging a workspace row: Not decided yet

*Sidebar*

**Today:** The same small label follows the pointer, and the row it's over is shaded; dropping puts the dragged workspace in that row's place. Here api is dragged onto storefront.
