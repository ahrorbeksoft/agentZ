//! The Workspaces view's spaces as the server keeps them: tabs of pane trees, saved after
//! every change and loaded when the server starts again (herdr's session snapshot).

use std::path::{Path, PathBuf};

use agentz_protocol::layout::{Direction, Node, PaneId, TileLayout};
use agentz_protocol::spaces::{
    Pane, PaneAgent, PaneContent, PaneLocation, Space, SpaceFolder, SpaceGit, SpaceId,
    SpacesSnapshot, Tab, TabId,
};
use anyhow::{Context as _, Result, anyhow};
use projects::{ProjectId, Saver, ThreadId};
use serde::{Deserialize, Serialize};
use util::ResultExt as _;

#[derive(Default, Serialize, Deserialize)]
struct SavedSpaces {
    next_id: u64,
    spaces: Vec<Space>,
}

pub(crate) struct SpaceStore {
    /// Spaces, tabs and panes share one counter, so ids are never reused.
    next_id: u64,
    spaces: Vec<Space>,
    revision: u64,
    saver: Option<Saver<SavedSpaces>>,
}

impl SpaceStore {
    /// Loads the spaces saved at `path`, and saves every change there. With `None`, nothing
    /// is read or saved.
    pub(crate) fn load(path: Option<PathBuf>) -> Self {
        let saved = path
            .as_deref()
            .and_then(|path| projects::read_state::<SavedSpaces>(path).log_err())
            .flatten()
            .unwrap_or_default();
        let mut this = Self {
            next_id: saved.next_id.max(1),
            spaces: saved.spaces,
            revision: 0,
            saver: path.map(|path| Saver::new(path, "spaces-saver")),
        };
        for space in &mut this.spaces {
            space.git = None;
            space.current = None;
            // Ending a rename unchanged once saved the folder's own name as the user's, which
            // stopped the name from following the workspace. It reads the same either way.
            if space.name.as_deref().is_some_and(|name| {
                space
                    .folder
                    .file_name()
                    .is_some_and(|folder| folder == name)
            }) {
                space.name = None;
            }
            space.tabs.retain_mut(|tab| {
                for pane in &mut tab.panes {
                    pane.clear_runtime();
                }
                // A tree and pane list that disagree came from a bad write; keep what's
                // consistent.
                let ids = TileLayout::from_saved(tab.root.clone(), PaneId(0)).pane_ids();
                tab.panes.retain(|pane| ids.contains(&pane.id));
                tab.panes.len() == ids.len()
            });
        }
        this.spaces.retain(|space| !space.tabs.is_empty());
        let highest_id = this
            .spaces
            .iter()
            .flat_map(|space| {
                std::iter::once(space.id.0).chain(space.tabs.iter().flat_map(|tab| {
                    std::iter::once(tab.id.0).chain(tab.panes.iter().map(|pane| pane.id.0))
                }))
            })
            .max();
        if let Some(highest_id) = highest_id {
            this.next_id = this.next_id.max(highest_id + 1);
        }
        this
    }

    /// Goes up by at least one with every change.
    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn snapshot(&self) -> SpacesSnapshot {
        SpacesSnapshot {
            spaces: self.spaces.clone(),
        }
    }

    pub(crate) fn spaces(&self) -> &[Space] {
        &self.spaces
    }

    pub(crate) fn pane(&self, id: PaneId) -> Option<&Pane> {
        self.spaces
            .iter()
            .flat_map(|space| &space.tabs)
            .flat_map(|tab| &tab.panes)
            .find(|pane| pane.id == id)
    }

    /// Every pane, with its space's folder.
    pub(crate) fn panes(&self) -> impl Iterator<Item = (&Space, &Pane)> {
        self.spaces.iter().flat_map(|space| {
            space
                .tabs
                .iter()
                .flat_map(move |tab| tab.panes.iter().map(move |pane| (space, pane)))
        })
    }

    pub(crate) fn create_space(
        &mut self,
        folder: PathBuf,
        project_id: Option<ProjectId>,
        content: PaneContent,
    ) -> PaneLocation {
        let space = SpaceId(self.allocate_id());
        let (tab, pane) = self.new_tab(content);
        let location = PaneLocation {
            space,
            tab: tab.id,
            pane,
        };
        self.spaces.push(Space {
            id: space,
            name: None,
            folder,
            project_id,
            tabs: vec![tab],
            git: None,
            current: None,
        });
        self.changed();
        location
    }

    pub(crate) fn rename_space(&mut self, id: SpaceId, name: Option<String>) -> Result<()> {
        self.space_mut(id)?.name = clean_name(name);
        self.changed();
        Ok(())
    }

    /// Returns the panes it held.
    pub(crate) fn close_space(&mut self, id: SpaceId) -> Result<Vec<Pane>> {
        let index = self.space_index(id)?;
        let space = self.spaces.remove(index);
        self.changed();
        Ok(space.tabs.into_iter().flat_map(|tab| tab.panes).collect())
    }

    pub(crate) fn move_space(&mut self, id: SpaceId, index: usize) -> Result<()> {
        let from = self.space_index(id)?;
        let space = self.spaces.remove(from);
        self.spaces.insert(index.min(self.spaces.len()), space);
        self.changed();
        Ok(())
    }

    pub(crate) fn create_tab(
        &mut self,
        space: SpaceId,
        content: PaneContent,
    ) -> Result<PaneLocation> {
        self.space_index(space)?;
        let (tab, pane) = self.new_tab(content);
        let location = PaneLocation {
            space,
            tab: tab.id,
            pane,
        };
        self.space_mut(space)?.tabs.push(tab);
        self.changed();
        Ok(location)
    }

    pub(crate) fn rename_tab(&mut self, id: TabId, name: Option<String>) -> Result<()> {
        let (space, tab) = self.tab_index(id)?;
        self.spaces[space].tabs[tab].name = clean_name(name);
        self.changed();
        Ok(())
    }

    /// Returns the panes it held. Closing a space's last tab closes the space.
    pub(crate) fn close_tab(&mut self, id: TabId) -> Result<Vec<Pane>> {
        let (space, tab) = self.tab_index(id)?;
        let tab = self.spaces[space].tabs.remove(tab);
        if self.spaces[space].tabs.is_empty() {
            self.spaces.remove(space);
        }
        self.changed();
        Ok(tab.panes)
    }

    pub(crate) fn move_tab(&mut self, id: TabId, index: usize) -> Result<()> {
        let (space, from) = self.tab_index(id)?;
        let tabs = &mut self.spaces[space].tabs;
        let tab = tabs.remove(from);
        tabs.insert(index.min(tabs.len()), tab);
        self.changed();
        Ok(())
    }

    pub(crate) fn split_pane(
        &mut self,
        target: PaneId,
        direction: Direction,
        content: PaneContent,
    ) -> Result<PaneLocation> {
        let (space, tab, _) = self.pane_index(target)?;
        let pane = PaneId(self.allocate_id());
        let space_id = self.spaces[space].id;
        let tab = &mut self.spaces[space].tabs[tab];
        let mut layout = TileLayout::from_saved(tab.root.clone(), target);
        anyhow::ensure!(
            layout.split_pane(target, direction, pane, 0.5),
            "the pane isn't in its tab"
        );
        tab.root = layout.into_root();
        tab.panes.push(Pane::new(pane, content));
        let location = PaneLocation {
            space: space_id,
            tab: tab.id,
            pane,
        };
        self.changed();
        Ok(location)
    }

    /// Returns the pane. Closing a tab's last pane closes the tab, and maybe its space.
    pub(crate) fn close_pane(&mut self, id: PaneId) -> Result<Pane> {
        let (space, tab_index, pane) = self.pane_index(id)?;
        let tab = &mut self.spaces[space].tabs[tab_index];
        if tab.panes.len() == 1 {
            let tab_id = tab.id;
            let mut panes = self.close_tab(tab_id)?;
            return panes.pop().context("the tab had no pane");
        }
        let mut layout = TileLayout::from_saved(tab.root.clone(), id);
        anyhow::ensure!(layout.close_pane(id), "the pane isn't in its tab");
        tab.root = layout.into_root();
        let pane = tab.panes.remove(pane);
        self.changed();
        Ok(pane)
    }

    pub(crate) fn swap_panes(&mut self, first: PaneId, second: PaneId) -> Result<()> {
        let (space, tab, _) = self.pane_index(first)?;
        let tab = &mut self.spaces[space].tabs[tab];
        let mut layout = TileLayout::from_saved(tab.root.clone(), first);
        anyhow::ensure!(
            layout.swap_panes(first, second),
            "those panes aren't two panes of one tab"
        );
        tab.root = layout.into_root();
        self.changed();
        Ok(())
    }

    pub(crate) fn set_split_ratio(&mut self, id: TabId, path: &[bool], ratio: f32) -> Result<()> {
        anyhow::ensure!(ratio.is_finite(), "the ratio isn't a number");
        let (space, tab) = self.tab_index(id)?;
        let tab = &mut self.spaces[space].tabs[tab];
        let mut layout = TileLayout::from_saved(tab.root.clone(), PaneId(0));
        anyhow::ensure!(layout.set_ratio_at(path, ratio), "there's no split there");
        let root = layout.into_root();
        if root != tab.root {
            tab.root = root;
            self.changed();
        }
        Ok(())
    }

    /// Returns what the pane held before.
    pub(crate) fn set_pane_content(
        &mut self,
        id: PaneId,
        content: PaneContent,
    ) -> Result<PaneContent> {
        let (space, tab, pane) = self.pane_index(id)?;
        let pane = &mut self.spaces[space].tabs[tab].panes[pane];
        pane.clear_runtime();
        let previous = std::mem::replace(&mut pane.content, content);
        self.changed();
        Ok(previous)
    }

    /// Closes the panes showing threads that no longer exist, returning them.
    pub(crate) fn close_thread_panes(&mut self, exists: impl Fn(ThreadId) -> bool) -> Vec<Pane> {
        let gone: Vec<PaneId> = self
            .panes()
            .filter(|(_, pane)| {
                matches!(pane.content, PaneContent::Thread(thread_id) if !exists(thread_id))
            })
            .map(|(_, pane)| pane.id)
            .collect();
        gone.into_iter()
            .filter_map(|pane| self.close_pane(pane).log_err())
            .collect()
    }

    /// Runtime state: not saved, but sent to clients.
    pub(crate) fn set_git(&mut self, id: SpaceId, git: Option<SpaceGit>) {
        let Some(space) = self.spaces.iter_mut().find(|space| space.id == id) else {
            return;
        };
        if space.git != git {
            space.git = git;
            self.revision += 1;
        }
    }

    /// Runtime state: not saved, but sent to clients.
    pub(crate) fn set_current(&mut self, id: SpaceId, current: Option<SpaceFolder>) {
        let Some(space) = self.spaces.iter_mut().find(|space| space.id == id) else {
            return;
        };
        if space.current != current {
            space.current = current;
            self.revision += 1;
        }
    }

    /// Runtime state: not saved, but sent to clients.
    pub(crate) fn set_pane_agent(&mut self, id: PaneId, agent: Option<PaneAgent>) {
        let Ok((space, tab, pane)) = self.pane_index(id) else {
            return;
        };
        let pane = &mut self.spaces[space].tabs[tab].panes[pane];
        if pane.agent != agent {
            pane.agent = agent;
            self.revision += 1;
        }
    }

    /// Runtime state: not saved, but sent to clients.
    pub(crate) fn set_pane_folder(&mut self, id: PaneId, folder: Option<SpaceFolder>) {
        let Ok((space, tab, pane)) = self.pane_index(id) else {
            return;
        };
        let pane = &mut self.spaces[space].tabs[tab].panes[pane];
        if pane.folder != folder {
            pane.folder = folder;
            self.revision += 1;
        }
    }

    /// Runtime state: not saved, but sent to clients.
    pub(crate) fn set_pane_program(&mut self, id: PaneId, program: Option<String>) {
        let Ok((space, tab, pane)) = self.pane_index(id) else {
            return;
        };
        let pane = &mut self.spaces[space].tabs[tab].panes[pane];
        if pane.program != program {
            pane.program = program;
            self.revision += 1;
        }
    }

    fn new_tab(&mut self, content: PaneContent) -> (Tab, PaneId) {
        let tab = TabId(self.allocate_id());
        let pane = PaneId(self.allocate_id());
        (
            Tab {
                id: tab,
                name: None,
                root: Node::Pane(pane),
                panes: vec![Pane::new(pane, content)],
            },
            pane,
        )
    }

    fn allocate_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn space_index(&self, id: SpaceId) -> Result<usize> {
        self.spaces
            .iter()
            .position(|space| space.id == id)
            .context("no such workspace")
    }

    fn space_mut(&mut self, id: SpaceId) -> Result<&mut Space> {
        let index = self.space_index(id)?;
        Ok(&mut self.spaces[index])
    }

    fn tab_index(&self, id: TabId) -> Result<(usize, usize)> {
        self.spaces
            .iter()
            .enumerate()
            .find_map(|(space_index, space)| {
                space
                    .tabs
                    .iter()
                    .position(|tab| tab.id == id)
                    .map(|tab_index| (space_index, tab_index))
            })
            .ok_or_else(|| anyhow!("no such tab"))
    }

    fn pane_index(&self, id: PaneId) -> Result<(usize, usize, usize)> {
        self.spaces
            .iter()
            .enumerate()
            .find_map(|(space_index, space)| {
                space.tabs.iter().enumerate().find_map(|(tab_index, tab)| {
                    tab.panes
                        .iter()
                        .position(|pane| pane.id == id)
                        .map(|pane_index| (space_index, tab_index, pane_index))
                })
            })
            .ok_or_else(|| anyhow!("no such pane"))
    }

    /// Writes the spaces now, and returns once they're written.
    pub(crate) fn flush_saves(&self) {
        if let Some(saver) = &self.saver {
            saver.flush();
        }
    }

    /// Stops writing the spaces: another process owns them now.
    pub(crate) fn stop_saving(&mut self) {
        if let Some(saver) = self.saver.take() {
            saver.discard();
        }
    }

    fn changed(&mut self) {
        self.revision += 1;
        let Some(saver) = &self.saver else {
            return;
        };
        let mut spaces = self.spaces.clone();
        for space in &mut spaces {
            space.git = None;
            space.current = None;
            for pane in space.tabs.iter_mut().flat_map(|tab| &mut tab.panes) {
                pane.clear_runtime();
            }
        }
        saver.save(SavedSpaces {
            next_id: self.next_id,
            spaces,
        });
    }
}

fn clean_name(name: Option<String>) -> Option<String> {
    name.map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
}

/// The folder's branch and how far it is from its upstream, as herdr shows them.
pub(crate) async fn space_git(folder: &Path) -> Option<SpaceGit> {
    if !crate::git::is_repository(folder).await {
        return None;
    }
    let branch = crate::git::git(folder, &["symbolic-ref", "--quiet", "--short", "HEAD"], &[])
        .await
        .ok()
        .map(|branch| branch.trim().to_string())
        .filter(|branch| !branch.is_empty());
    let (ahead, behind) = crate::git::git(
        folder,
        &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"],
        &[],
    )
    .await
    .ok()
    .and_then(|counts| parse_ahead_behind(&counts))
    .unwrap_or_default();
    let checkout = crate::git::git(folder, &["rev-parse", "--show-toplevel"], &[])
        .await
        .ok()
        .map(|top| PathBuf::from(top.trim()))
        .filter(|top| !top.as_os_str().is_empty());
    // Found through the shared `.git`, so a linked worktree is named after the repository
    // rather than itself, and grouped under it.
    let main_checkout = crate::workspaces::main_checkout(folder).await.ok();
    Some(SpaceGit {
        branch,
        ahead,
        behind,
        repository: main_checkout
            .as_ref()
            .and_then(|root| Some(root.file_name()?.to_string_lossy().into_owned())),
        checkout,
        main_checkout,
    })
}

/// The folder most tabs are in. When tabs are split evenly, the earliest tab's wins.
pub(crate) fn majority_folder(tab_folders: &[PathBuf]) -> Option<&PathBuf> {
    let mut best: Option<(&PathBuf, usize)> = None;
    for folder in tab_folders {
        let count = tab_folders.iter().filter(|other| *other == folder).count();
        if best.is_none_or(|(_, best_count)| count > best_count) {
            best = Some((folder, count));
        }
    }
    best.map(|(folder, _)| folder)
}

fn parse_ahead_behind(output: &str) -> Option<(u32, u32)> {
    let mut counts = output.split_whitespace();
    let ahead = counts.next()?.parse().ok()?;
    let behind = counts.next()?.parse().ok()?;
    Some((ahead, behind))
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentz_protocol::spaces::PaneTerminal;

    fn shell(folder: &str) -> PaneContent {
        PaneContent::Terminal(PaneTerminal {
            folder: folder.into(),
            command: None,
        })
    }

    fn pane_ids(store: &SpaceStore, tab: TabId) -> Vec<PaneId> {
        let (space, tab) = store.tab_index(tab).expect("tab");
        TileLayout::from_saved(store.spaces[space].tabs[tab].root.clone(), PaneId(0)).pane_ids()
    }

    #[test]
    fn splitting_and_closing_panes() {
        let mut store = SpaceStore::load(None);
        let first = store.create_space("/w".into(), None, shell("/w"));
        let second = store
            .split_pane(first.pane, Direction::Horizontal, shell("/w"))
            .expect("split");
        let third = store
            .split_pane(
                second.pane,
                Direction::Vertical,
                PaneContent::Thread(ThreadId(9)),
            )
            .expect("split");
        assert_eq!(second.tab, first.tab);
        assert_eq!(
            pane_ids(&store, first.tab),
            vec![first.pane, second.pane, third.pane]
        );

        store.swap_panes(first.pane, third.pane).expect("swap");
        assert_eq!(
            pane_ids(&store, first.tab),
            vec![third.pane, second.pane, first.pane]
        );

        store.set_split_ratio(first.tab, &[], 0.3).expect("ratio");
        let snapshot = store.snapshot();
        let (_, tab) = snapshot.tab(first.tab).expect("tab");
        assert!(matches!(tab.root, Node::Split { ratio, .. } if (ratio - 0.3).abs() < 1e-6));
        assert!(store.set_split_ratio(first.tab, &[false], 0.3).is_err());

        let closed = store.close_pane(second.pane).expect("close");
        assert_eq!(closed.id, second.pane);
        assert_eq!(pane_ids(&store, first.tab), vec![third.pane, first.pane]);

        // Gone threads take their panes with them.
        let closed = store.close_thread_panes(|_| false);
        assert_eq!(closed.len(), 1);
        assert_eq!(pane_ids(&store, first.tab), vec![first.pane]);

        // The last pane takes its tab, and the last tab its space.
        store.close_pane(first.pane).expect("close");
        assert!(store.spaces().is_empty());
    }

    #[test]
    fn tabs_and_spaces() {
        let mut store = SpaceStore::load(None);
        let one = store.create_space("/one".into(), Some(ProjectId(4)), shell("/one"));
        let two = store.create_space("/two".into(), None, shell("/two"));
        let tab = store.create_tab(one.space, shell("/one")).expect("tab");
        assert_eq!(tab.space, one.space);
        assert_eq!(store.spaces()[0].tabs.len(), 2);

        store.move_tab(tab.tab, 0).expect("move");
        assert_eq!(store.spaces()[0].tabs[0].id, tab.tab);
        store
            .rename_tab(tab.tab, Some("  build ".into()))
            .expect("rename");
        assert_eq!(store.spaces()[0].tabs[0].name.as_deref(), Some("build"));
        store.rename_tab(tab.tab, Some(" ".into())).expect("rename");
        assert_eq!(store.spaces()[0].tabs[0].name, None);

        store.move_space(two.space, 0).expect("move");
        assert_eq!(store.spaces()[0].id, two.space);
        store
            .rename_space(one.space, Some("Mine".into()))
            .expect("rename");
        assert_eq!(
            store.snapshot().space(one.space).expect("space").label(),
            "Mine"
        );
        assert_eq!(
            store.snapshot().space(two.space).expect("space").label(),
            "two"
        );

        let closed = store.close_tab(tab.tab).expect("close");
        assert_eq!(closed.len(), 1);
        let closed = store.close_space(one.space).expect("close");
        assert_eq!(closed.len(), 1);
        assert_eq!(store.spaces().len(), 1);
        assert!(store.close_space(one.space).is_err());
    }

    #[test]
    fn runtime_state_counts_as_a_change_but_isnt_saved() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("spaces.json");
        {
            let mut store = SpaceStore::load(Some(path.clone()));
            let location = store.create_space("/w".into(), None, shell("/w"));
            let revision = store.revision();
            store.set_git(
                location.space,
                Some(SpaceGit {
                    branch: Some("main".into()),
                    ahead: 1,
                    behind: 0,
                    repository: Some("w".into()),
                    checkout: Some("/w".into()),
                    main_checkout: Some("/w".into()),
                }),
            );
            store.set_pane_agent(
                location.pane,
                Some(PaneAgent {
                    registry_agent: None,
                    name: "Claude Code".into(),
                    state: agentz_protocol::spaces::PaneAgentState::Working,
                }),
            );
            assert_eq!(store.revision(), revision + 2);
            store
                .split_pane(location.pane, Direction::Vertical, shell("/w"))
                .expect("split");
        }

        let mut store = SpaceStore::load(Some(path));
        let space = &store.spaces()[0];
        assert_eq!(space.git, None);
        assert!(space.tabs[0].panes.iter().all(|pane| pane.agent.is_none()));
        assert_eq!(space.tabs[0].panes.len(), 2);
        // Ids keep counting up from the saved ones.
        let highest = store
            .panes()
            .map(|(space, pane)| space.id.0.max(pane.id.0))
            .max()
            .expect("panes");
        let next = store.create_space("/x".into(), None, shell("/x"));
        assert!(next.space.0 > highest && next.pane.0 > highest);
    }

    #[test]
    fn inconsistent_saved_tabs_are_dropped() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("spaces.json");
        let saved = SavedSpaces {
            next_id: 3,
            spaces: vec![Space {
                id: SpaceId(1),
                name: None,
                folder: "/w".into(),
                project_id: None,
                tabs: vec![Tab {
                    id: TabId(2),
                    name: None,
                    root: Node::Pane(PaneId(7)),
                    panes: Vec::new(),
                }],
                git: None,
                current: None,
            }],
        };
        std::fs::write(&path, serde_json::to_vec(&saved).expect("json")).expect("write");
        let store = SpaceStore::load(Some(path));
        assert!(store.spaces().is_empty());
    }

    #[test]
    fn a_saved_name_equal_to_the_folders_is_automatic() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("spaces.json");
        {
            let mut store = SpaceStore::load(Some(path.clone()));
            let first = store.create_space("/work/app".into(), None, shell("/work/app"));
            let second = store.create_space("/work/lib".into(), None, shell("/work/lib"));
            store
                .rename_space(first.space, Some("app".into()))
                .expect("rename");
            store
                .rename_space(second.space, Some("Library".into()))
                .expect("rename");
        }
        let store = SpaceStore::load(Some(path));
        let names: Vec<Option<&str>> = store
            .spaces()
            .iter()
            .map(|space| space.name.as_deref())
            .collect();
        assert_eq!(names, [None, Some("Library")]);
    }

    #[test]
    fn the_majority_of_tabs_picks_the_folder() {
        let folders = |paths: &[&str]| paths.iter().map(PathBuf::from).collect::<Vec<_>>();
        assert_eq!(majority_folder(&[]), None);
        assert_eq!(
            majority_folder(&folders(&["/a", "/b", "/b"])),
            Some(&PathBuf::from("/b"))
        );
        // A tie goes to the earliest tab.
        assert_eq!(
            majority_folder(&folders(&["/a", "/b", "/b", "/a"])),
            Some(&PathBuf::from("/a"))
        );
        assert_eq!(
            majority_folder(&folders(&["/a", "/b"])),
            Some(&PathBuf::from("/a"))
        );
    }

    #[test]
    fn ahead_behind_parses() {
        assert_eq!(parse_ahead_behind("2\t1\n"), Some((2, 1)));
        assert_eq!(parse_ahead_behind(""), None);
    }

    #[tokio::test]
    async fn git_status_of_a_folder() {
        let directory = tempfile::tempdir().expect("tempdir");
        assert_eq!(space_git(directory.path()).await, None);
        crate::git::git(directory.path(), &["init", "-q", "-b", "trunk"], &[])
            .await
            .expect("init");
        let root = std::fs::canonicalize(directory.path()).expect("canonical path");
        let name = root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
        assert_eq!(
            space_git(directory.path()).await,
            Some(SpaceGit {
                branch: Some("trunk".into()),
                ahead: 0,
                behind: 0,
                repository: name.clone(),
                checkout: Some(root.clone()),
                main_checkout: Some(root.clone()),
            })
        );
        // Inside it, and in a worktree of it, the repository keeps its name.
        let inner = directory.path().join("inner");
        std::fs::create_dir(&inner).expect("inner folder");
        assert_eq!(space_git(&inner).await.and_then(|git| git.repository), name);
        let run = |args: &'static [&'static str]| {
            let folder = directory.path().to_path_buf();
            async move { crate::git::git(&folder, args, &[]).await.expect("git") }
        };
        run(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "first",
        ])
        .await;
        let worktrees = tempfile::tempdir().expect("tempdir");
        let worktree = worktrees.path().join("feature");
        crate::git::git(
            directory.path(),
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "feature",
                &worktree.to_string_lossy(),
            ],
            &[],
        )
        .await
        .expect("worktree");
        let git = space_git(&worktree).await.expect("a repository");
        // A linked worktree, grouped under the main checkout.
        assert!(git.is_linked_worktree());
        assert_eq!(git.main_checkout.as_ref(), Some(&root));
        assert_eq!(
            (git.branch.as_deref(), git.repository),
            (Some("feature"), name)
        );
    }
}
