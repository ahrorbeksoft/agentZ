# New worktrees and pastures: design decisions

Picked on the design board (`design/new-workspace/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Where a new worktree’s choices are: A. The branch becomes “From main”

*Choosing*

**Today:** The checkout chip under a new thread’s composer (“Local”) opens Where It Works: Local checkout, New worktree, New pasture, then the project’s existing worktrees and pastures by branch. Picking New worktree at once replaces the draft with one in a new worktree (“Making a worktree…” pulses in the strip): git worktree add on a new branch, agentz/ and 8 hex digits, from whatever the project’s folder has checked out. A pasture copies the folder the same way. The branch shows at the strip’s right end, as plain text. Nothing else can be chosen.

**A. The branch becomes “From main”** (from t3code (BranchToolbar)): With New worktree or New pasture picked, the branch at the strip’s right end reads “From main” and opens a searchable list of branches to start from. The rest of the strip stays as it is.

## 2. What a new worktree starts from by default: A. What the project has checked out

*Choosing*

**Today:** What the project’s folder has checked out (its branch, or its commit when it’s detached).

**A. What the project has checked out** (from today): As now. The list starts on the project folder’s branch.

## 3. What the branch list shows: A. Search + B. Marks for each branch + C. Remote branches too

*Choosing · pick any*

**Today:** Only the Workspaces view’s New Worktree… dialog lists branches: its From menu has the local branches, without search or marks.

**A. Search** (from t3code (“Search refs…”)): A field at the top filters the list as you type.

**B. Marks for each branch** (from t3code (BranchPickerRefItem)): Faint words at the row’s end: current (the project folder’s), default, worktree (checked out in another worktree or pasture), remote.

**C. Remote branches too** (from t3code): origin’s branches that have no local branch follow the local ones, so a teammate’s branch can be a base without fetching it by hand.

## 4. Starting from what’s on origin: B. A fetch button in the list

*Choosing*

**Today:** Nothing fetches. A worktree starts from the local branch, however far behind origin it is.

**B. A fetch button in the list** (from new): A Fetch button beside the search updates the remote branches; picking origin/main (with Remote branches too) starts from it.

## 5. Checking out an existing branch: D. Only through existing worktrees

*Choosing*

**Today:** A new worktree always gets a new branch. An existing branch can only be worked on in a worktree that already has it (the menu’s Existing list), or in the project’s folder. The Workspaces dialog refuses a name that exists: “the branch fix-login already exists; choose another name”.

**D. Only through existing worktrees** (from t3code (Previous worktree, reuse)): As now: an existing branch is worked on in the worktree that has it, from the Existing list. A new worktree is always a new branch.

## 6. The new branch’s name: C. Named after the thread

*Choosing*

**Today:** agentz/ and 8 hex digits (agentz/3f671a7e) for a thread’s worktree; herdr’s words (agentz/green-valley-86fb) in the Workspaces dialog, where it can be edited. A thread’s branch can’t be named before or after.

**C. Named after the thread** (from t3code (Worktree branch naming)): It starts with a temporary name; once the thread has a title (the agent’s, or the title generator’s), the branch is renamed to it: agentz/fix-checkout-rounding. A taken name keeps the temporary one.

## 7. Starting from a pull request: B. Leave it out

*Choosing*

**Today:** agentZ has no GitHub or GitLab integration; a pull request’s branch has to be fetched by hand.

**B. Leave it out** (from today): Not part of this change; a pushed pull request branch shows as a remote branch (with Remote branches too) and can be checked out from there.

## 8. When the worktree is made: B. When the first message is sent

*Making it*

**Today:** At once, when New worktree is picked: the draft is replaced by one whose agent starts in the new worktree. “Making a worktree…” pulses in the strip meanwhile. Switching back to Local leaves the worktree behind until drafts are swept.

**B. When the first message is sent** (from t3code (thread setup)): The strip only records the choice (“New worktree · From main”). Sending makes it, and the thread shows its steps above the reply: fetch (with Start from origin), check out, submodules; then the agent starts there. A failure shows with Retry and Use Local.

## 9. A new pasture’s choices: A. The same choices as a worktree

*Making it*

**Today:** A pasture copies the project’s whole folder (uncommitted and ignored files too), then makes its branch from what’s checked out (cow’s create). It gets the same kind of name, agentz/ and an id.

**A. The same choices as a worktree** (from new): From, Start from origin, Check Out and the name apply to pastures too; the copy switches to the base after copying, and uncommitted changes that don’t fit it are left out.

## 10. Defaults in Settings: B. Start from origin

*Making it · pick any*

**Today:** There are none: new threads start in the project’s folder (an open question in the architecture asks whether they should default to a worktree or pasture), worktrees branch from the checked-out branch, and submodules are always fetched recursively.

**B. Start from origin** (from t3code (Start from origin)): Whether the branch list’s switch starts on.

## 11. The Workspaces view’s New Worktree dialog: A. The same branch list

*Making it*

**Today:** A workspace row’s New Worktree… has the branch name (herdr’s words), From (a plain list of local branches), where it’s made, and Worktree or Pasture.

**A. The same branch list** (from t3code’s list, in herdr’s dialog): Its From opens the branch list picked above (search, marks, remote branches, Start from origin), and an existing branch’s name is handled as picked above.
