# Appearance: design decisions

Picked on the design board (`design/appearance/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. What's behind the window: E. Your picture + also F. Your picture, softened

*Window*

**Today:** Solid. GPUI can draw a window clear or frosted (on macOS an NSVisualEffectView behind it), and Zed's themes turn that on with "background.appearance": "blurred" and colors with some transparency. agentZ has the code from Zed but never applies it to the window, so it's always solid, whatever the theme says.

**E. Your picture** (from new (as Warp and iTerm2 do)): A picture you choose fills the window behind everything, and each surface is the theme's color over it, partly see-through. Works the same on every desktop, since agentZ draws it.

**Also take from F. Your picture, softened** (from new (as macOS's lock screen does)): Like E, but the picture is blurred once when you pick it, so it reads as color and light rather than detail. GPUI can't blur at draw time, so agentZ keeps a blurred copy.

## 2. How much shows through: D. Your choice, with a slider + also A. A little

*Window*

**Today:** Nothing does.

**D. Your choice, with a slider** (from new): Settings has a slider from solid to see-through, starting at B.

**Also take from A. A little** (from new): Surfaces are 88% the theme's color: a hint of the wallpaper's color, mostly the theme.

## 3. What stays solid: None of these

*Window · pick any*

**Today:** Everything is solid.

## 4. Where you turn it on: B. A Window section for any theme

*Settings*

**Today:** Settings › Appearance has the mode and a light and a dark theme. A theme can say "background.appearance", but agentZ ignores it.

**B. A Window section for any theme** (from new): A Window section under Theme: Background (Solid, Frosted, or Picture), how much shows through, and for Picture, the picture and whether to soften it. It applies to whichever theme is on, light or dark, by making its surfaces partly transparent.
