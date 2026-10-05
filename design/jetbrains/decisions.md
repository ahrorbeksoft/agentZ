# JetBrains Dark: design decisions

Picked on the design board (`design/jetbrains/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Changed lines: A. Zed's diff colors

*JetBrains Dark*

**Today:** Added and removed lines are filled with the theme's solid created and deleted backgrounds (#447152, #8f5247). One Dark sets those at 10%, so they only look heavy here.

**A. Zed's diff colors** (from Zed editor): Use the colors Zed's own editor uses for changed lines (`editor.diff_hunk.*.background`): in JetBrains Dark, the theme's green at 12% and its dark red #2b2322. For every theme, so diffs look as they do in Zed; One Dark barely changes.

## 2. Agents | Workspaces: B. Gray, no blue

*JetBrains Dark*

**Today:** The selected side is Zed's toggle button: the theme's info background (gray #393b41 here) with its accent text (saturated blue #3474f0). The same accent colors links and other highlights.

**B. Gray, no blue** (from JetBrains segmented buttons): The selected side is a lighter gray with the normal text color. A change to the switch itself, so in every theme.
