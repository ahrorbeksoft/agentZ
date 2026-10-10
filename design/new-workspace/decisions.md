# New worktrees and pastures: design decisions

Picked on the design board (`design/new-workspace/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Where a new worktree’s choices are: Not decided yet

*Choosing*

**Today:** The checkout chip under a new thread’s composer (“Local”) opens Where It Works: Local checkout, New worktree, New pasture, then the project’s existing worktrees and pastures by branch. Picking New worktree at once replaces the draft with one in a new worktree (“Making a worktree…” pulses in the strip): git worktree add on a new branch, agentz/ and 8 hex digits, from whatever the project’s folder has checked out. A pasture copies the folder the same way. The branch shows at the strip’s right end, as plain text. Nothing else can be chosen.

## 2. What a new worktree starts from by default: Not decided yet

*Choosing*

**Today:** What the project’s folder has checked out (its branch, or its commit when it’s detached).

## 3. What the branch list shows: Not decided yet

*Choosing · pick any*

**Today:** Only the Workspaces view’s New Worktree… dialog lists branches: its From menu has the local branches, without search or marks.

## 4. Starting from what’s on origin: Not decided yet

*Choosing*

**Today:** Nothing fetches. A worktree starts from the local branch, however far behind origin it is.

## 5. Checking out an existing branch: Not decided yet

*Choosing*

**Today:** A new worktree always gets a new branch. An existing branch can only be worked on in a worktree that already has it (the menu’s Existing list), or in the project’s folder. The Workspaces dialog refuses a name that exists: “the branch fix-login already exists; choose another name”.

## 6. The new branch’s name: Not decided yet

*Choosing*

**Today:** agentz/ and 8 hex digits (agentz/3f671a7e) for a thread’s worktree; herdr’s words (agentz/green-valley-86fb) in the Workspaces dialog, where it can be edited. A thread’s branch can’t be named before or after.

## 7. Starting from a pull request: Not decided yet

*Choosing*

**Today:** agentZ has no GitHub or GitLab integration; a pull request’s branch has to be fetched by hand.

## 8. When the worktree is made: Not decided yet

*Making it*

**Today:** At once, when New worktree is picked: the draft is replaced by one whose agent starts in the new worktree. “Making a worktree…” pulses in the strip meanwhile. Switching back to Local leaves the worktree behind until drafts are swept.

## 9. A new pasture’s choices: Not decided yet

*Making it*

**Today:** A pasture copies the project’s whole folder (uncommitted and ignored files too), then makes its branch from what’s checked out (cow’s create). It gets the same kind of name, agentz/ and an id.

## 10. Defaults in Settings: Not decided yet

*Making it · pick any*

**Today:** There are none: new threads start in the project’s folder (an open question in the architecture asks whether they should default to a worktree or pasture), worktrees branch from the checked-out branch, and submodules are always fetched recursively.

## 11. The Workspaces view’s New Worktree dialog: Not decided yet

*Making it*

**Today:** A workspace row’s New Worktree… has the branch name (herdr’s words), From (a plain list of local branches), where it’s made, and Worktree or Pasture.
