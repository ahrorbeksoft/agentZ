# Pinned threads: design decisions

Picked on the design board (`design/pins/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. The pin on a pinned card: A. A muted pin before the time

*Cards*

**Today:** Nothing is pinned today. The cards follow the thread order setting (Newest first, or Latest activity), and the shelves for shells, Workspaces threads and archived threads rest at the bottom. In these, Add the checkout page and Fix flaky login test are pinned.

**A. A muted pin before the time** (from t3code pinned card): A small pin in a dim gray on the project line, just before the time or the state. Clicking it unpins the thread (“Unpin thread” on hover). With one project selected, which drops the project line, it sits before the time on the title line.

## 2. The pinned cards at rest: A. Nothing between them

*Cards*

**Today:** There’s no pinned block today. The drafts already sit above the cards, with a faint line under them. Each option here uses A’s pin.

**A. Nothing between them** (from t3code (the pinned block has no header)): The pinned cards are simply first. Nothing marks where they end; the pins do.

## 3. Pinning from the card: A. The menu and dragging only

*Cards*

**Today:** A card shows Archive over its time while the mouse is on it (and Discard over a draft’s). Here the mouse is on Write tests for the rate limiter.

**A. The menu and dragging only** (from t3code (pinning lives in the menu)): Hover stays as it is, Archive alone. Pinning is in the menu or by dragging; the pin on a pinned card unpins it.

## 4. Where a dragged card lands: A. Pinned and Active labels open up

*Dragging*

**Today:** Cards don’t drag today. They will lift as workspace rows do: the card is raised and held where you grabbed it, and the cards it passes slide over to leave its place open where it would land. Here Write tests for the rate limiter is dragged up among the pinned cards, which pins it. The options differ in what shows where the pinned cards end; each shows the next topic’s badge.

**A. Pinned and Active labels open up** (from t3code drag boundaries): While a card is held, “Pinned” opens above the pinned cards and “Active” under them, each a small label with a rule, 24 points tall; the cards move down to make room. The section the card would land in takes the accent. They close when it’s dropped. With nothing pinned yet, “Pinned” and an empty slot open at the top, so there’s somewhere to drop the first.

## 5. The dragged card: A. Raised, saying what letting go does

*Dragging*

**Today:** Workspace rows drag this way already: the row is raised, opaque in the selected color with a shadow, and held where you grabbed it. The same drag as before, with the previous topic’s A.

**A. Raised, saying what letting go does** (from t3code drop badge, agentZ workspace rows): The card raised as workspace rows are. Over another section, its time or state gives way to a small badge in the accent: Pin, Unpin, Archive or Unarchive. Over its own section, nothing changes (a pinned card keeps its pin).

## 6. Archiving by dragging: A. The Archived header takes the accent

*Dragging*

**Today:** A card archives from its Archive button on hover or its menu; an archived row comes back from its Unarchive button or menu. Here Write tests for the rate limiter is dragged down to Archived. The other way, an archived row dragged up into the list says Unarchive, and lands where it’s let go: among the pinned cards, it’s pinned too.

**A. The Archived header takes the accent** (from t3code Settled header): While a card is held, the Archived header reads at full strength; with the card over it (or over its rows), it turns accent, and the card’s badge says Archive. Letting go archives the thread, and its card leaves the list; a folded shelf stays folded. With nothing archived yet, the header shows while a card is held.

## 7. Pin in the menus: B. First

*Menus*

**Today:** A card’s menu (right-click) has Rename, Archive, Project Settings and Delete…; the thread’s title menu has Rename, Continue with Another Agent, Archive and Delete…. Pin goes in both, and reads Unpin on a pinned thread. Shells, drafts, Workspaces threads and agent CLI cards don’t get it. Here the menu of Write tests for the rate limiter.

**B. First** (from t3code (Pin near the top)): Pin, Rename, Archive, … in the card’s menu, and first in the title menu.
