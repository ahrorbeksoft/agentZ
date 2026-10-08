# Projects in Settings: design decisions

Picked on the design board (`design/settings-projects/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. How the sidebar lists a combined project: A. Once, by its name only

*Settings sidebar*

**Today:** Under “Projects”, one row per copy: “ielts-today”, then “ielts-today · Ahrorbek’s Laptop”, and “fluency.uz” three times. This Mac’s copy has the plain name; every other machine’s copy adds “ · ” and the machine’s name. Each row opens that one copy’s page.

**A. Once, by its name only** (from t3code): One row per project, as t3code’s settings list them. Nothing shows which machines it’s on; the page says that. A project that’s only on another machine keeps “ · Devbox 1” after its name.

## 2. Choosing the machine: A. A dropdown in the top right, as on Usage

*Project page*

**Today:** No choice on the page: each copy has its own page, opened from its own row in the settings sidebar. The page’s Repository section lists the other copies in a “Combined with” row (“Ahrorbek’s Laptop: /home/ahrorbek/projects/ielts-today. Name and icon changes apply to all of them.”). The Agents and Usage pages pick their machine with a dropdown in the top right (“This Mac”), its menu listing each machine with a check on the one shown.

**A. A dropdown in the top right, as on Usage** (from agentZ’s Usage and Agents pages): The machine dropdown from Usage, beside the page’s title. Its menu lists each copy’s machine with its icon, and a check on the one shown. The page opens on the machine you came from (a thread’s Project Settings), else This Mac. Two copies on one machine are told apart by folder, as New Thread’s machine menu does (“This Mac · ~/projects/agentZ-2”). A project on one machine shows no dropdown.

## 3. Shared settings and the copy’s own: A. Sections named after the machine

*Project page*

**Today:** One list for one copy: Project (Name, Icon, Monogram, Folder), Repository (Repository, Grouping, Combined with), Checkouts and Danger. Name, Icon and Monogram already change every copy (an icon file only This Mac’s copies, since it’s a file on this Mac). Folder, Grouping, Checkouts and Remove change only the copy whose page it is. Only “Combined with” says which is which.

**A. Sections named after the machine** (from new): Project (Name, Icon, Monogram) and Repository come first and apply to every copy. Then a section titled with the chosen machine, “Ahrorbek’s Laptop”, holds Folder and Grouping, followed by its Checkouts and Danger. “Combined with” goes away, since the machine picker lists the copies. The picker is the one chosen in “Choosing the machine”.

## 4. What Remove does for a combined project: A. Only the chosen machine’s copy

*Project page*

**Today:** Danger has one row, “Remove project”: “Removes the project and its threads from agentZ. Files on disk are not touched.” Its Remove Project button asks “Remove “ielts-today” from agentZ?” (“Its threads are removed too. Nothing on disk is touched.”) and then removes only the copy whose page is open. The other machines keep theirs, though nothing says so.

**A. Only the chosen machine’s copy** (from t3code): Today’s behavior, named for what it does: “Remove from Ahrorbek’s Laptop”, as t3code’s “Remove checkout”. Other machines keep their copies. After it, the page shows the next copy. A project on one machine keeps today’s “Remove Project”.
