//! The Workspaces view's workspaces, herdr's model: a folder on the server's machine, with
//! tabs, each a tree of split panes holding terminals and threads. They're called spaces here,
//! since a thread's workspace is the checkout it works in (see [`crate::workspace`]).
//!
//! Spaces are shared session state (herdr's runtime/client rule): the server keeps, saves and
//! restores them. Which tab is shown, which pane is focused, and zoom belong to each client.

use std::path::PathBuf;

use projects::{ProjectId, ThreadId};
use serde::{Deserialize, Serialize};

use crate::layout::{Direction, Node, PaneId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SpaceId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TabId(pub u64);

/// Every space on the server, in the sidebar's order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SpacesSnapshot {
    pub spaces: Vec<Space>,
}

impl SpacesSnapshot {
    pub fn space(&self, id: SpaceId) -> Option<&Space> {
        self.spaces.iter().find(|space| space.id == id)
    }

    /// The space and tab holding a pane.
    pub fn pane(&self, id: PaneId) -> Option<(&Space, &Tab, &Pane)> {
        self.spaces.iter().find_map(|space| {
            space.tabs.iter().find_map(|tab| {
                tab.panes
                    .iter()
                    .find(|pane| pane.id == id)
                    .map(|pane| (space, tab, pane))
            })
        })
    }

    pub fn tab(&self, id: TabId) -> Option<(&Space, &Tab)> {
        self.spaces.iter().find_map(|space| {
            space
                .tabs
                .iter()
                .find(|tab| tab.id == id)
                .map(|tab| (space, tab))
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Space {
    pub id: SpaceId,
    /// The user's name for it. Without one, clients name it after its folder.
    #[serde(default)]
    pub name: Option<String>,
    pub folder: PathBuf,
    /// The project it was opened from, if any.
    #[serde(default)]
    pub project_id: Option<ProjectId>,
    /// Never empty: closing the last tab closes the space.
    pub tabs: Vec<Tab>,
    /// The folder's branch, looked up by the server from time to time. Not saved.
    #[serde(default)]
    pub git: Option<SpaceGit>,
}

impl Space {
    /// The name the sidebar shows: the user's, or the folder's.
    pub fn label(&self) -> String {
        if let Some(name) = &self.name {
            return name.clone();
        }
        self.folder
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.folder.to_string_lossy().into_owned())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tab {
    pub id: TabId,
    /// The user's name for it. Without one, clients number it, as herdr does.
    #[serde(default)]
    pub name: Option<String>,
    pub root: Node,
    /// Every pane in `root`, in no particular order.
    pub panes: Vec<Pane>,
}

impl Tab {
    pub fn pane(&self, id: PaneId) -> Option<&Pane> {
        self.panes.iter().find(|pane| pane.id == id)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pane {
    pub id: PaneId,
    pub content: PaneContent,
    /// The agent CLI detected in a terminal pane, and its state. Not saved.
    #[serde(default)]
    pub agent: Option<PaneAgent>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PaneContent {
    /// A terminal of its own, [`crate::terminal::TerminalKey::Pane`]. After a server restart
    /// it starts again in its folder.
    Terminal(PaneTerminal),
    /// A thread: an agent's conversation, or a terminal thread's terminal. The same thread
    /// the Agents view shows.
    Thread(ThreadId),
    /// From a newer version.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaneTerminal {
    pub folder: PathBuf,
    /// Run by the user's shell; `None` is the shell itself.
    #[serde(default)]
    pub command: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneAgent {
    /// As herdr names it, such as "Claude Code".
    pub name: String,
    pub state: PaneAgentState,
}

/// herdr's agent states.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaneAgentState {
    /// Finished, waiting for the next task.
    Idle,
    Working,
    /// Waiting for the user, such as at a permission prompt.
    Blocked,
    /// Its screen says nothing either way.
    Unknown,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpaceGit {
    /// `None` when detached.
    pub branch: Option<String>,
    /// Commits ahead of and behind the branch's upstream; zero without one.
    pub ahead: u32,
    pub behind: u32,
}

/// Where a request put a pane: [`crate::Response::SpacePane`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneLocation {
    pub space: SpaceId,
    pub tab: TabId,
    pub pane: PaneId,
}

/// The spaces' requests, [`crate::Request::Spaces`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SpaceRequest {
    /// A space with one tab holding `content`: [`crate::Response::SpacePane`].
    CreateSpace {
        folder: PathBuf,
        #[serde(default)]
        project_id: Option<ProjectId>,
        content: PaneContent,
    },
    /// `None` names it after its folder again.
    RenameSpace {
        space: SpaceId,
        name: Option<String>,
    },
    /// Ends its terminals. Its threads stay.
    CloseSpace(SpaceId),
    MoveSpace {
        space: SpaceId,
        index: usize,
    },
    /// A tab after the space's others: [`crate::Response::SpacePane`].
    CreateTab {
        space: SpaceId,
        content: PaneContent,
    },
    RenameTab {
        tab: TabId,
        name: Option<String>,
    },
    /// Closing a space's last tab closes the space.
    CloseTab(TabId),
    MoveTab {
        tab: TabId,
        index: usize,
    },
    /// Splits `pane`, putting `content` to its right or below it:
    /// [`crate::Response::SpacePane`].
    SplitPane {
        pane: PaneId,
        direction: Direction,
        content: PaneContent,
    },
    /// Closing a tab's last pane closes the tab.
    ClosePane(PaneId),
    /// Swaps two panes of one tab.
    SwapPanes(PaneId, PaneId),
    /// Sets the first child's share of the split at `path` (see
    /// [`crate::layout::SplitBorder::path`]).
    SetSplitRatio {
        tab: TabId,
        path: Vec<bool>,
        ratio: f32,
    },
    /// Shows something else in a pane, ending its terminal if it had one.
    SetPaneContent {
        pane: PaneId,
        content: PaneContent,
    },
    /// From a newer version.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}
