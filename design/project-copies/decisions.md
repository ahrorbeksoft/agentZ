# Project copies: design decisions

Picked on the design board (`design/project-copies/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. One name for a combined project: Not decided yet

*One project*

**Today:** Copies with the same primary remote are already one project in the project switcher (one row under “On several machines”, its machines after the name), the title bar, the All projects count, New Thread’s project list (it starts in the copy used last; the composer’s machine picker moves it), the sidebar’s project filter (every copy’s threads) and Settings (one page with a machine dropdown, from the settings-projects round). What still differs by machine: the switcher names a project whose copies share the repository’s name by its owner and repository, “ahrorbeksoft/ielts-today”, while each thread card, draft row and Go To entry names its own copy, “ielts-today”, with that copy’s icon. Threads, workspaces and terminals each run on one machine, so they keep its icon. A change in progress (not committed) shows the switcher’s machines as icons instead of names.

## 2. Where you see whether the copies are in sync: Not decided yet

*In sync*

**Today:** Nowhere. Each machine’s server reads the branch checked out in each project every 5 seconds and sends it with the projects; cards show their thread’s branch. Nothing reads or shows a copy’s commit, how it stands against its remote, uncommitted changes or stashes, or compares one machine’s copy with another’s. The Workspaces view has a git popover for one workspace (branch → upstream with ↑↓, uncommitted files and lines, the last commit, the path, its worktrees), which the options below borrow from.

## 3. What each copy is compared with: Not decided yet

*In sync*

**Today:** Nothing is compared. Each server knows only its own copy; the app gets each copy’s branch name and nothing else. Two copies can only be compared by commit where one machine has the other’s commit: after it fetched what the other pushed.

## 4. How the differences are listed: Not decided yet

*In sync*

**Today:** Not shown anywhere. The Workspaces view’s git popover writes one checkout as a few lines: the branch → its upstream with ↑ and ↓, uncommitted files with +added −removed, the last commit with its time, and the path.

## 5. How it’s kept current: Not decided yet

*In sync*

**Today:** Each server reads the branch of its projects’ folders, worktrees and pastures every 5 seconds and when a new one appears (`Server::refresh_git_heads`, `read_git_head`, which reads `.git/HEAD` without running git), and sends it with the projects. It looks each project’s repository up again every 15 minutes. The Workspaces view’s workspaces also get ahead/behind, uncommitted changes and the last commit (`SpaceGit`), read with git. Nothing fetches; ahead and behind are against whatever the machine fetched last.
