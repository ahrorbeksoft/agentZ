# Open in Editor: design decisions

Picked on the design board (`design/open-in-editor/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. The button and where it goes: A. Split button before Changes

*Thread header*

**Today:** The open thread’s header (`render_toolbar` in agent_view.rs) has the project crumb, the title with its menu, a fork’s source pill, the artifacts pill, the branch, the changes button (+added −removed), Terminal and “⋯” (the agent’s options). Under 480 px the branch, artifacts, changes and Terminal move into “⋯”. Nothing opens the thread’s folder (the project’s checkout, a worktree or a pasture) in an editor. The pictures are recreated from the code, not captured from the running app.

**A. Split button before Changes** (from t3code): t3code’s OpenInPicker: an outlined split button before the changes button. The left part shows the last-used editor’s icon and “Open” and opens the folder there with one click; the chevron lists the installed editors. Narrow, “Open in VS Code ⌘⇧O” and “Open in…” (a submenu of the editors) head the compact “⋯”, as t3code’s compact menu does. The key is ⌘⇧O here: t3code’s ⌘O is Open Folder in agentZ.

## 2. What the menu lists: B. Last used first, Finder and Copy Path

*Thread header*

**Today:** There is no menu. agentZ lists no editors, and the server on a machine can’t say which are installed.

**B. Last used first, Finder and Copy Path** (from t3code): A, changed in three ways (t3code does none of them): the last-used editor on top with the key, in its own group; the rest after it; then “Show in Finder” (“Show in Files” on Linux) and Copy Path below a line. With no editor found the menu opens anyway: “No editor found” (disabled) over Show in Finder and Copy Path, and the button shows the plain “Open” with an outward arrow, which opens the menu.

## 3. A thread on another machine: C. Open over SSH, in the editors that can

*Thread header*

**Today:** A thread’s folder is on the machine of its project: this Mac, or an SSH machine (Devbox 1) whose server runs the agent. agentZ shows its conversation, changes and terminals everywhere, but nothing opens its folder on the machine that runs it. An editor installed on this Mac can’t read that folder except through its own SSH support: VS Code and Cursor’s Remote - SSH, Zed’s `ssh://` projects. The app knows the SSH host it dials for a machine (the one it was added with) and the folder’s absolute path.

**C. Open over SSH, in the editors that can** (from t3code): t3code’s “remote-links”: the menu lists only the editors on this Mac that open remote folders, VS Code, Cursor and VSCodium (`vscode://vscode-remote/ssh-remote+devbox1/home/ubuntu/api`) and Zed (`zed://ssh/devbox1/home/ubuntu/api`); the app hands the link to the OS, and the editor does the SSH. A first line says “Opens over SSH. Needs your key on Devbox 1” until one has worked. WebStorm and the file manager aren’t listed. A link the OS can’t open does nothing; the editor’s window reports its own failure.

## 4. Open a changed file from the Changes panel: B. A button on the file header

*Changes panel*

**Today:** The Changes panel (Cmd-D) lists the changed files as headers over their hunks, each with a Viewed button. Nothing opens a file in an editor. t3code’s diff panel opens a file in the editor, and its launch commands take a line (`code --goto path:line`, Zed’s `path:line`, JetBrains’ `--line`), which it uses for the file links in chat.

**B. A button on the file header** (from t3code): The last-used editor’s icon beside Viewed on each file’s header, with the tooltip “Open in VS Code”, opening the file itself at its first change. It follows the same editor choice as the header’s button, and is hidden when there is none.
