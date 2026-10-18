# Live page preview: design decisions

Picked on the design board (`design/live-preview/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. How a live page is resized: D. Handles on both edges, and presets

*Resizing*

**Today:** A live page opens up close (Open page on a card): an interactive frame, 1280 px wide or the window less 128 px, and the window’s height less 64 px; with Phone chosen in the bar, 390 px wide. ← → step through the variants, Esc closes. The frame’s width can’t be changed: to see a phone layout the bar’s Desktop · Phone swaps in the variant’s separate phone page.

**D. Handles on both edges, and presets** (from Chrome DevTools’ device mode (both handles) and its presets): A grip on each side of the frame, which moves both at once so the page stays centred, with the width beside the grip while dragging; and a small row of presets over the frame (Phone · Tablet · Laptop · Fit) that sets the width in one click. No typed fields. Presets are chips with their width as a faint number; the active one is lit while the width equals it.

## 2. Where the preview resizes: A. Only in the close-up

*Resizing*

**Today:** Nowhere. A card draws the live page at a fixed 1280 × 800 (or 390 × 844 in Phone), scaled down to the card’s width, non-interactive, with Open page over its corner. The close-up has the one width. To compare how three variants look on a phone you switch the whole page to Phone and read narrow cards, five across.

**A. Only in the close-up** (from new): The cards stay as they are (1280 px scaled down). Resizing is in the close-up only, by whichever control topic 1 picks (drawn here with a drag handle). Closing and opening another variant starts at full width again.

## 3. What becomes of Desktop · Phone and the phone versions: A. Live pages ignore Desktop · Phone; the toggle is for pictures

*Resizing*

**Today:** A variant can have a picture and a live page, and a phone picture and a phone page. The bar has Desktop · Phone when the design has both kinds; Phone redraws every card from its phone picture or phone page (a variant with neither says “No phone version”) in a five-column grid. Settings › General › Artifacts › Sizes (Desktop · Phone · Both) tells agents which to make; with Both and Variants as › Live pages, every variant is two HTML files.

**A. Live pages ignore Desktop · Phone; the toggle is for pictures** (from new): A live page is one page: agents publish only `page`; `phone_page` goes away. The bar’s toggle shows only when some variant has a phone picture, and then swaps pictures as today; live pages don’t change with it (resize them as topic 1 and 2 say). Settings: Sizes stays and applies to pictures only; with Variants as › Live pages it’s greyed, its line saying “Pictures only: a live page is one responsive page.” With Both, Sizes still sizes the pictures, one page per variant.
