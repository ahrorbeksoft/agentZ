//! The Workspaces view's spaces: their requests, their pane terminals, and their folders' git
//! status.

use std::path::{Path, PathBuf};
use std::time::Duration;

use agentz_protocol::Response;
use agentz_protocol::layout::PaneId;
use agentz_protocol::spaces::{
    Pane, PaneAgent, PaneAgentState, PaneContent, PaneTerminal, SpaceFolder, SpaceId, SpaceRequest,
};
use agentz_protocol::terminal::TerminalKey;
use anyhow::{Context as _, Result, anyhow};
use projects::ThreadId;
use util::ResultExt as _;

use super::terminal_requests::home_relative;
use super::{Input, Server};
use crate::detect::{Agent, AgentState};
use crate::spaces;
use crate::terminal_programs;
use crate::terminals::{TerminalSize, TerminalSpawn};

/// How often the spaces' branches are looked up again.
const GIT_REFRESH_INTERVAL: Duration = Duration::from_secs(5);

impl Server {
    pub(super) fn space_request(&mut self, request: SpaceRequest) -> Result<Response> {
        // Which folder most tabs are in changes with the tabs and their first panes.
        let moves_tabs = matches!(
            request,
            SpaceRequest::CreateTab { .. }
                | SpaceRequest::CloseTab(_)
                | SpaceRequest::MoveTab { .. }
                | SpaceRequest::ClosePane(_)
                | SpaceRequest::SwapPanes(..)
                | SpaceRequest::MovePane { .. }
                | SpaceRequest::SetPaneContent { .. }
        );
        let response = self.handle_space_request(request)?;
        if moves_tabs {
            let spaces: Vec<SpaceId> = self.spaces.spaces().iter().map(|space| space.id).collect();
            for space in spaces {
                self.refresh_space_git(space);
            }
        }
        Ok(response)
    }

    fn handle_space_request(&mut self, request: SpaceRequest) -> Result<Response> {
        match request {
            SpaceRequest::CreateSpace {
                folder,
                project_id,
                content,
            } => {
                // Collecting the components drops a trailing `/` left by completion.
                let folder: PathBuf = crate::directories::expand_home(&folder)
                    .components()
                    .collect();
                anyhow::ensure!(folder.is_dir(), "{} isn't a folder here", folder.display());
                if let Some(project_id) = project_id {
                    self.projects
                        .project(project_id)
                        .context("no such project")?;
                }
                let content = self.checked_content(content, &folder)?;
                let location = self.spaces.create_space(folder, project_id, content);
                self.start_pane(location.pane);
                self.refresh_space_git(location.space);
                Ok(Response::SpacePane(location))
            }
            SpaceRequest::RenameSpace { space, name } => {
                self.spaces.rename_space(space, name)?;
                Ok(Response::Ok)
            }
            SpaceRequest::CloseSpace(space) => {
                let panes = self.spaces.close_space(space)?;
                self.end_panes(panes);
                Ok(Response::Ok)
            }
            SpaceRequest::MoveSpace { space, index } => {
                self.spaces.move_space(space, index)?;
                Ok(Response::Ok)
            }
            SpaceRequest::CreateTab { space, content } => {
                let folder = self.space_folder(space)?;
                let content = self.checked_content(content, &folder)?;
                let location = self.spaces.create_tab(space, content)?;
                self.start_pane(location.pane);
                Ok(Response::SpacePane(location))
            }
            SpaceRequest::RenameTab { tab, name } => {
                self.spaces.rename_tab(tab, name)?;
                Ok(Response::Ok)
            }
            SpaceRequest::CloseTab(tab) => {
                let panes = self.spaces.close_tab(tab)?;
                self.end_panes(panes);
                Ok(Response::Ok)
            }
            SpaceRequest::MoveTab { tab, index } => {
                self.spaces.move_tab(tab, index)?;
                Ok(Response::Ok)
            }
            SpaceRequest::SplitPane {
                pane,
                direction,
                content,
            } => {
                let folder = self.pane_space_folder(pane)?;
                let content = self.checked_content(content, &folder)?;
                let location = self.spaces.split_pane(pane, direction, content)?;
                self.start_pane(location.pane);
                Ok(Response::SpacePane(location))
            }
            SpaceRequest::ClosePane(pane) => {
                let pane = self.spaces.close_pane(pane)?;
                self.end_panes(vec![pane]);
                Ok(Response::Ok)
            }
            SpaceRequest::SwapPanes(first, second) => {
                self.spaces.swap_panes(first, second)?;
                Ok(Response::Ok)
            }
            SpaceRequest::MovePane { pane, target, edge } => {
                self.spaces.move_pane(pane, target, edge)?;
                Ok(Response::Ok)
            }
            SpaceRequest::SetSplitRatio { tab, path, ratio } => {
                self.spaces.set_split_ratio(tab, &path, ratio)?;
                Ok(Response::Ok)
            }
            SpaceRequest::SetPaneContent { pane, content } => {
                let folder = self.pane_space_folder(pane)?;
                let content = self.checked_content(content, &folder)?;
                self.spaces.set_pane_content(pane, content)?;
                self.close_terminal(&TerminalKey::Pane(pane));
                self.start_pane(pane);
                Ok(Response::Ok)
            }
            SpaceRequest::Unknown(request) => Err(anyhow!("unsupported request: {request}")),
        }
    }

    /// Refuses what a pane can't show, and fills in a terminal's folder.
    fn checked_content(&self, content: PaneContent, space_folder: &Path) -> Result<PaneContent> {
        match content {
            PaneContent::Terminal(terminal) => {
                let folder = if terminal.folder.as_os_str().is_empty() {
                    space_folder.to_path_buf()
                } else {
                    crate::directories::expand_home(&terminal.folder)
                };
                Ok(PaneContent::Terminal(PaneTerminal {
                    folder,
                    command: terminal
                        .command
                        .filter(|command| !command.trim().is_empty()),
                }))
            }
            PaneContent::Thread(thread_id) => {
                self.projects.thread(thread_id).context("no such thread")?;
                Ok(content)
            }
            PaneContent::Unknown(content) => Err(anyhow!("unsupported pane content: {content}")),
        }
    }

    fn space_folder(&self, space: SpaceId) -> Result<PathBuf> {
        self.spaces
            .spaces()
            .iter()
            .find(|candidate| candidate.id == space)
            .map(|space| space.folder.clone())
            .context("no such workspace")
    }

    fn pane_space_folder(&self, pane: PaneId) -> Result<PathBuf> {
        self.spaces
            .panes()
            .find(|(_, candidate)| candidate.id == pane)
            .map(|(space, _)| space.folder.clone())
            .context("no such pane")
    }

    /// Starts a terminal pane's terminal. Thread panes show their thread's.
    fn start_pane(&mut self, pane: PaneId) {
        let key = TerminalKey::Pane(pane);
        let Ok(spawn) = self.pane_terminal_spawn(pane) else {
            return;
        };
        self.start_terminal(key, spawn, TerminalSize::default())
            .log_err();
    }

    /// What a terminal pane runs: its command or the shell, in its folder. A folder that's
    /// gone since falls back to its space's, then the home folder.
    pub(super) fn pane_terminal_spawn(&self, pane: PaneId) -> Result<TerminalSpawn> {
        let (space, pane) = self
            .spaces
            .panes()
            .find(|(_, candidate)| candidate.id == pane)
            .context("no such pane")?;
        let PaneContent::Terminal(terminal) = &pane.content else {
            return Err(anyhow!("this pane shows a thread"));
        };
        let cwd = [terminal.folder.clone(), space.folder.clone()]
            .into_iter()
            .chain([util::paths::home_dir().clone()])
            .find(|folder| folder.is_dir())
            .unwrap_or_else(|| terminal.folder.clone());
        Ok(TerminalSpawn {
            program: self.terminal_program(terminal.command.clone()),
            cwd,
            env: self.terminal_env(None),
        })
    }

    /// Ends the terminals of closed panes.
    fn end_panes(&mut self, panes: Vec<Pane>) {
        for pane in panes {
            self.close_terminal(&TerminalKey::Pane(pane.id));
        }
    }

    /// Closes a pane whose terminal ended, as herdr does.
    pub(super) fn close_space_pane(&mut self, pane: PaneId) {
        if let Some(pane) = self.spaces.close_pane(pane).log_err() {
            self.end_panes(vec![pane]);
        }
    }

    /// herdr's snapshot restore: terminal panes start again as new shells (or their commands)
    /// in their folders, unless the server before handed them over. Thread panes show their threads, which are already back.
    pub(super) fn restore_spaces(&mut self) {
        let panes: Vec<PaneId> = self
            .spaces
            .panes()
            .filter(|(_, pane)| matches!(pane.content, PaneContent::Terminal(_)))
            .map(|(_, pane)| pane.id)
            // Handed over by the server before, still running.
            .filter(|pane| {
                !self
                    .terminals
                    .running
                    .contains_key(&TerminalKey::Pane(*pane))
            })
            .collect();
        for pane in panes {
            self.start_pane(pane);
        }
        self.close_orphaned_panes();
        self.refresh_spaces_git();
        let inputs = self.inputs.clone();
        self.runtime.spawn(async move {
            loop {
                tokio::time::sleep(GIT_REFRESH_INTERVAL).await;
                let refresh = Input::Run(Box::new(Server::refresh_spaces_git));
                if inputs.unbounded_send(refresh).is_err() {
                    break;
                }
            }
        });
    }

    /// Closes the panes of threads that were deleted.
    pub(super) fn close_orphaned_panes(&mut self) {
        let projects = &self.projects;
        let closed = self
            .spaces
            .close_thread_panes(|thread_id| projects.thread(thread_id).is_some());
        self.end_panes(closed);
    }

    /// Also the branches of the folders terminal threads are in, which change without the
    /// terminal noticing (a checkout in another window).
    fn refresh_spaces_git(&mut self) {
        let spaces: Vec<SpaceId> = self.spaces.spaces().iter().map(|space| space.id).collect();
        for space in spaces {
            self.refresh_space_git(space);
        }
        let folders: Vec<(ThreadId, PathBuf)> = self
            .projects
            .terminal_folders()
            .map(|(thread_id, folder)| (thread_id, folder.path.clone()))
            .collect();
        for (thread_id, path) in folders {
            self.refresh_terminal_folder(thread_id, path);
        }
    }

    /// Looks up where a workspace is now, the folder most of its tabs are in, and that
    /// folder's branch.
    fn refresh_space_git(&mut self, space: SpaceId) {
        let Some(folder) = self.space_current_folder(space) else {
            return;
        };
        self.spawn_then(
            async move {
                let git = spaces::space_git(&folder).await;
                (folder, git)
            },
            move |server, (folder, git)| {
                server.spaces.set_current(
                    space,
                    Some(SpaceFolder {
                        display_path: home_relative(&folder),
                        path: folder,
                    }),
                );
                server.spaces.set_git(space, git);
            },
        );
    }

    pub(super) fn refresh_space_of_pane(&mut self, pane: PaneId) {
        let space = self
            .spaces
            .panes()
            .find(|(_, candidate)| candidate.id == pane)
            .map(|(space, _)| space.id);
        if let Some(space) = space {
            self.refresh_space_git(space);
        }
    }

    /// The folder most of a workspace's tabs are in, each tab by its top-left pane. A workspace
    /// whose tabs have no folder yet is still where it was opened.
    fn space_current_folder(&self, space: SpaceId) -> Option<PathBuf> {
        let space = self
            .spaces
            .spaces()
            .iter()
            .find(|candidate| candidate.id == space)?;
        let tab_folders: Vec<PathBuf> = space
            .tabs
            .iter()
            .filter_map(|tab| {
                let pane = tab.pane(tab.root.first_pane())?;
                self.pane_current_folder(&pane.content, pane.id)
            })
            .collect();
        Some(
            spaces::majority_folder(&tab_folders)
                .unwrap_or(&space.folder)
                .clone(),
        )
    }

    fn pane_current_folder(&self, content: &PaneContent, pane: PaneId) -> Option<PathBuf> {
        match content {
            PaneContent::Terminal(terminal) => self
                .terminals
                .running
                .get(&TerminalKey::Pane(pane))
                .and_then(|running| running.folder.clone())
                .or_else(|| Some(terminal.folder.clone()))
                .filter(|folder| !folder.as_os_str().is_empty()),
            PaneContent::Thread(thread_id) => {
                if let Some(folder) = self.projects.terminal_folder(*thread_id) {
                    return Some(folder.path.clone());
                }
                let thread = self.projects.thread(*thread_id)?;
                thread
                    .workspace
                    .clone()
                    .or_else(|| Some(self.projects.project(thread.project_id)?.path.clone()))
            }
            PaneContent::Unknown(_) => None,
        }
    }

    /// A pane terminal's agent and its state, from agent detection.
    pub(super) fn publish_pane_agent(
        &mut self,
        pane: PaneId,
        agent: Option<Agent>,
        state: AgentState,
    ) {
        let agent = agent.map(|agent| PaneAgent {
            registry_agent: terminal_programs::registry_agent(agent.label()).map(Into::into),
            name: terminal_programs::label(agent.label())
                .unwrap_or(agent.label())
                .to_string(),
            state: match state {
                AgentState::Idle => PaneAgentState::Idle,
                AgentState::Working => PaneAgentState::Working,
                AgentState::Blocked => PaneAgentState::Blocked,
                AgentState::Unknown => PaneAgentState::Unknown,
            },
        });
        self.spaces.set_pane_agent(pane, agent);
    }
}
