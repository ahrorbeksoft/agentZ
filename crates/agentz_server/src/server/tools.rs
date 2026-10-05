//! The agent-control tools, after t3code's orchestrator MCP server: agents list, read, start,
//! message, wait for, interrupt, rename and archive the threads of their own project, read
//! their changes, and delegate tasks to subthreads. The MCP bridge and the CLI both call them with
//! [`Request::CallTool`](agentz_protocol::Request).
//!
//! A delegated task runs in a subthread that gets only the task as its prompt. It ends once
//! its agent is idle and its own tasks have ended and been announced to it; the result is the
//! agent's last message, or the error. The parent hears of the end through `delegate_task`'s
//! wait, `task_status`, or a message sent to it once it's idle. Both the outcome and whether
//! the parent has it are saved, so a restart loses neither.
//!
//! The policy is t3code's: a caller only reaches threads in its own project, can't message,
//! wait for or interrupt itself, and can't delete threads or answer permission requests, which
//! stay with the user. Mutations take an optional `clientRequestId`, so a retry returns the
//! first answer instead of doing the work again.

mod relay;
mod terminals;
mod workspaces;

pub(super) use relay::Relays;

use std::collections::VecDeque;
use std::time::{Duration, Instant, SystemTime};

use agent_client_protocol::schema::v1 as acp;
use agentz_protocol::agents::{AgentId, InstallState};
use agentz_protocol::diff::{DiffScope, DiffStatus, FileChange};
use agentz_protocol::thread::{ConnectionStatus, Entry};
use agentz_protocol::{ConnectionId, ServerMessage, ToolCaller, ToolResult};
use collections::HashMap;
use futures::FutureExt as _;
use futures::future::BoxFuture;
use projects::{
    ProjectId, ProjectStore, Task, TaskEnd, TaskOutcome, Thread, ThreadCreator, ThreadId,
};
use serde_json::{Map, Value, json};
use util::ResultExt as _;

use super::{ClientId, FollowUp, Input, Server};
use workspaces::{Folder, Placement, workspace_strategy};

const MAX_BATCH_THREADS: usize = 20;
const MAX_PROMPT_CHARS: usize = 120_000;
const MAX_TITLE_CHARS: usize = 512;
const DEFAULT_WAIT: Duration = Duration::from_secs(10 * 60);
const MAX_WAIT: Duration = Duration::from_secs(60 * 60);
/// How long reading a thread whose agent wasn't running waits for its history to load.
const LOAD_WAIT: Duration = Duration::from_secs(30);
const DEFAULT_LIST_LIMIT: u64 = 50;
const DEFAULT_READ_LIMIT: u64 = 50;
const DEFAULT_CHARS_PER_ITEM: u64 = 4_000;
const MAX_CHARS_PER_ITEM: u64 = 50_000;
const MAX_LAST_MESSAGE_CHARS: usize = 8_000;
const MAX_TOOL_RESULTS_KEPT: usize = 1_000;
const DEFAULT_PATCH_CHARS: u64 = 50_000;
const MAX_PATCH_CHARS: u64 = 1_000_000;
const TASK_ROLES: [&str; 6] = [
    "implementation",
    "research",
    "review",
    "design",
    "test",
    "general",
];

/// A call waiting for a thread, retried after every change until it's done or its time is up.
pub(super) struct PendingToolCall {
    client: ClientId,
    id: u64,
    caller: Caller,
    name: String,
    arguments: Value,
    /// Set once `delegate_task` has created the task it waits for, so it isn't created again.
    task: Option<ThreadId>,
    deadline: Instant,
}

/// The answers to mutations by `clientRequestId`, oldest dropped first.
#[derive(Default)]
pub(super) struct ToolResults {
    results: HashMap<String, Value>,
    order: VecDeque<String>,
}

impl ToolResults {
    fn get(&self, key: &str) -> Option<&Value> {
        self.results.get(key)
    }

    fn insert(&mut self, key: String, value: Value) {
        if self.results.insert(key.clone(), value).is_none() {
            self.order.push_back(key);
            if self.order.len() > MAX_TOOL_RESULTS_KEPT
                && let Some(oldest) = self.order.pop_front()
            {
                self.results.remove(&oldest);
            }
        }
    }
}

/// The project a call may manage, and the thread making it unless it came from the CLI run
/// outside a thread.
#[derive(Clone, Copy, Debug)]
struct Caller {
    project_id: ProjectId,
    thread_id: Option<ThreadId>,
}

impl Caller {
    fn creator(self) -> ThreadCreator {
        match self.thread_id {
            Some(thread_id) => ThreadCreator::Thread(thread_id),
            None => ThreadCreator::Command,
        }
    }
}

/// A typed failure, with t3code's `OrchestratorMcpFailure` codes.
#[derive(Debug)]
struct Failure {
    code: &'static str,
    message: String,
}

fn failure(code: &'static str, message: impl Into<String>) -> Failure {
    Failure {
        code,
        message: message.into(),
    }
}

fn invalid(message: impl Into<String>) -> Failure {
    failure("invalid_request", message)
}

enum Step {
    Done(Value),
    /// Try again once something changes, for at most this long.
    Wait(Duration),
    /// Answer once the delegated task ends, or with `waitTimedOut` after this long.
    WaitForTask(ThreadId, Duration),
    /// Answer with this work's result, done off the server's task.
    Background(BoxFuture<'static, Result<Value, Failure>>),
    /// Do this work off the server's task, then go on with its result on the server, as when
    /// a launched thread's worktree has to be made first.
    Then(BoxFuture<'static, Continuation>),
}

type Outcome = Result<Step, Failure>;
type Continuation = Box<dyn FnOnce(&mut Server) -> Outcome + Send>;

impl Server {
    /// Runs a tool and answers, now or once the thread it waits for is ready.
    pub(super) fn call_tool(
        &mut self,
        client: ClientId,
        id: u64,
        caller: ToolCaller,
        name: String,
        arguments: Value,
    ) {
        let caller = match self.resolve_caller(&caller) {
            Ok(caller) => caller,
            Err(failure) => {
                self.send_changes();
                return self.send_tool_result(client, id, tool_result(Err(failure)));
            }
        };
        let outcome = self.run_tool(caller, &name, &arguments, false);
        self.settle_tool_call(client, id, caller, name, arguments, outcome);
    }

    /// Answers a call with its outcome, or has it wait, retry or go on in the background.
    fn settle_tool_call(
        &mut self,
        client: ClientId,
        id: u64,
        caller: Caller,
        name: String,
        arguments: Value,
        outcome: Outcome,
    ) {
        let result = match outcome {
            Ok(step @ (Step::Wait(_) | Step::WaitForTask(..))) => {
                let (task, timeout) = match step {
                    Step::WaitForTask(task, timeout) => (Some(task), timeout),
                    Step::Wait(timeout) => (None, timeout),
                    Step::Done(_) | Step::Background(_) | Step::Then(_) => (None, Duration::ZERO),
                };
                let deadline = Instant::now() + timeout;
                self.pending_tool_calls.push(PendingToolCall {
                    client,
                    id,
                    caller,
                    name,
                    arguments,
                    task,
                    deadline,
                });
                self.wake_at(deadline);
                None
            }
            Ok(Step::Done(value)) => Some(tool_result(Ok(value))),
            Ok(Step::Background(work)) => {
                self.answer_in_background(client, id, work);
                None
            }
            Ok(Step::Then(work)) => {
                self.continue_in_background(client, id, caller, name, arguments, work);
                None
            }
            Err(failure) => Some(tool_result(Err(failure))),
        };
        self.send_changes();
        if let Some(result) = result {
            self.send_tool_result(client, id, result);
        }
    }

    fn send_tool_result(&self, client: ClientId, id: u64, result: ToolResult) {
        self.send(
            client,
            ServerMessage::Response {
                id,
                result: Ok(agentz_protocol::Response::ToolResult(result)),
            },
        );
    }

    fn continue_in_background(
        &self,
        client: ClientId,
        id: u64,
        caller: Caller,
        name: String,
        arguments: Value,
        work: BoxFuture<'static, Continuation>,
    ) {
        self.spawn_then(work, move |server, then| {
            let outcome = then(server);
            server.settle_tool_call(client, id, caller, name, arguments, outcome);
        });
    }

    /// Retries the waiting calls, and returns the answers of those that are done.
    pub(super) fn answer_waiting_tool_calls(&mut self) -> Vec<(ClientId, u64, ToolResult)> {
        let now = Instant::now();
        let mut answers = Vec::new();
        for call in std::mem::take(&mut self.pending_tool_calls) {
            if !self.clients.contains_key(&call.client) {
                continue;
            }
            let timed_out = now >= call.deadline;
            let outcome = match call.task {
                Some(task) => self.wait_for_task(call.caller, task, timed_out),
                None => self.run_tool(call.caller, &call.name, &call.arguments, timed_out),
            };
            match outcome {
                Ok(Step::Wait(_) | Step::WaitForTask(..)) if !timed_out => {
                    self.pending_tool_calls.push(call)
                }
                Ok(Step::Wait(_) | Step::WaitForTask(..)) => answers.push((
                    call.client,
                    call.id,
                    tool_result(Err(failure("orchestration_error", "timed out"))),
                )),
                Ok(Step::Done(value)) => {
                    answers.push((call.client, call.id, tool_result(Ok(value))))
                }
                Ok(Step::Background(work)) => self.answer_in_background(call.client, call.id, work),
                Ok(Step::Then(work)) => self.continue_in_background(
                    call.client,
                    call.id,
                    call.caller,
                    call.name,
                    call.arguments,
                    work,
                ),
                Err(failure) => answers.push((call.client, call.id, tool_result(Err(failure)))),
            }
        }
        answers
    }

    /// Sends agents' queued messages to threads whose turn has ended.
    pub(super) fn send_follow_ups(&mut self) {
        let waiting: Vec<ThreadId> = self.follow_ups.keys().copied().collect();
        for thread_id in waiting {
            let Some(thread) = self.threads.get_mut(&thread_id) else {
                self.follow_ups.remove(&thread_id);
                continue;
            };
            // Handed to the next server with the thread.
            if thread.is_paused() {
                continue;
            }
            match thread.status() {
                ConnectionStatus::Failed(_) => {
                    self.follow_ups.remove(&thread_id);
                    continue;
                }
                ConnectionStatus::Ready if !thread.is_working() => {}
                _ => continue,
            }
            let Some(queue) = self.follow_ups.get_mut(&thread_id) else {
                continue;
            };
            let Some(follow_up) = queue.pop_front() else {
                continue;
            };
            let (text, from, ended_tasks) = match follow_up.task {
                None => (follow_up.text, follow_up.from, Vec::new()),
                // Every task that has ended goes in one message, as in t3code.
                Some(task) => {
                    let mut tasks = vec![task];
                    queue.retain(|other| match other.task {
                        Some(task) => {
                            tasks.push(task);
                            false
                        }
                        None => true,
                    });
                    let text = task_ended_message(&tasks);
                    let from = match tasks.as_slice() {
                        [task] => ThreadCreator::Thread(*task),
                        _ => follow_up.from,
                    };
                    (text, from, tasks)
                }
            };
            thread.send_from(text, from);
            for task in ended_tasks {
                self.projects
                    .update_task(task, |task| task.delivered = true);
            }
            self.thread_changed(ConnectionId::Thread(thread_id));
        }
        self.follow_ups.retain(|_, queue| !queue.is_empty());
    }

    /// Ends the tasks whose subthread is done, as its state shows: idle, with its own tasks
    /// ended and announced to it.
    pub(super) fn finish_tasks(&mut self) {
        let unfinished: Vec<ThreadId> = self
            .projects
            .threads()
            .iter()
            .filter(|thread| {
                thread
                    .task
                    .as_ref()
                    .is_some_and(|task| task.outcome.is_none())
            })
            .map(|thread| thread.id)
            .collect();
        for task in unfinished {
            let outcome = match self.threads.get(&task) {
                None => Some((
                    TaskEnd::Interrupted,
                    Some("The task's agent stopped.".to_string()),
                )),
                Some(thread) => match thread.status() {
                    ConnectionStatus::Failed(error) => {
                        Some((TaskEnd::Failed, Some(error.to_string())))
                    }
                    ConnectionStatus::Ready
                        if !self.is_busy(task) && !self.has_unannounced_tasks(task) =>
                    {
                        Some(if let Some(error) = thread.turn_error() {
                            (TaskEnd::Failed, Some(error.to_string()))
                        } else if thread.last_stop_reason() == Some(&acp::StopReason::Cancelled) {
                            (
                                TaskEnd::Interrupted,
                                Some("The task's turn was stopped.".to_string()),
                            )
                        } else {
                            (
                                TaskEnd::Completed,
                                last_agent_message(thread.entries())
                                    .map(|message| truncate(message, MAX_LAST_MESSAGE_CHARS).0),
                            )
                        })
                    }
                    _ => None,
                },
            };
            if let Some((end, summary)) = outcome {
                self.projects.update_task(task, |task| {
                    task.outcome = Some(TaskOutcome {
                        end,
                        summary,
                        ended_at: SystemTime::now(),
                    });
                });
            }
        }
    }

    /// Queues word of each ended task the parent doesn't have yet, starting the parent's agent
    /// if needed.
    pub(super) fn announce_finished_tasks(&mut self) {
        let ended: Vec<(ThreadId, ThreadId)> = self
            .projects
            .threads()
            .iter()
            .filter_map(|thread| {
                let task = thread.task.as_ref()?;
                (task.outcome.is_some() && !task.delivered).then_some((thread.id, task.parent))
            })
            .collect();
        for (task, parent) in ended {
            let queued = self
                .follow_ups
                .get(&parent)
                .is_some_and(|queue| queue.iter().any(|follow_up| follow_up.task == Some(task)));
            if queued {
                continue;
            }
            // A parent that has itself ended, or is gone, has nobody left to tell.
            let parent_ended = self.projects.thread(parent).is_none_or(|thread| {
                thread
                    .task
                    .as_ref()
                    .is_some_and(|task| task.outcome.is_some())
            });
            if parent_ended || self.start(parent).is_err() {
                self.projects
                    .update_task(task, |task| task.delivered = true);
                continue;
            }
            self.follow_ups
                .entry(parent)
                .or_default()
                .push_back(FollowUp {
                    text: String::new(),
                    from: ThreadCreator::Thread(task),
                    task: Some(task),
                });
        }
    }

    fn answer_in_background(
        &self,
        client: ClientId,
        id: u64,
        work: BoxFuture<'static, Result<Value, Failure>>,
    ) {
        let inputs = self.inputs.clone();
        self.runtime.spawn(async move {
            let result = tool_result(work.await);
            inputs
                .unbounded_send(Input::Respond {
                    client,
                    id,
                    result: Ok(agentz_protocol::Response::ToolResult(result)),
                })
                .ok();
        });
    }

    pub(super) fn wake_at(&self, deadline: Instant) {
        let inputs = self.inputs.clone();
        self.runtime.spawn(async move {
            tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
            inputs.unbounded_send(Input::ToolDeadline).ok();
        });
    }

    fn resolve_caller(&self, caller: &ToolCaller) -> Result<Caller, Failure> {
        let thread_id = match caller {
            ToolCaller::Session(token) => *self.tool_sessions.get(token).ok_or_else(|| {
                failure(
                    "capability_denied",
                    "This agent's agentZ credential is no longer valid.",
                )
            })?,
            ToolCaller::Thread(thread_id) => *thread_id,
            ToolCaller::Directory(path) => {
                let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.clone());
                // A project's worktrees and pastures count as part of it.
                let (project_id, _) = self
                    .projects
                    .projects()
                    .iter()
                    .flat_map(|project| {
                        std::iter::once(&project.path)
                            .chain(project.workspaces.iter().map(|workspace| &workspace.path))
                            .map(move |folder| (project.id, folder))
                    })
                    .filter(|(_, folder)| {
                        path.starts_with(folder)
                            || std::fs::canonicalize(folder)
                                .is_ok_and(|folder| path.starts_with(folder))
                    })
                    .max_by_key(|(_, folder)| folder.components().count())
                    .ok_or_else(|| {
                        failure(
                            "capability_denied",
                            format!("{} isn't inside an agentZ project.", path.display()),
                        )
                    })?;
                return Ok(Caller {
                    project_id,
                    thread_id: None,
                });
            }
        };
        let thread = self.projects.thread(thread_id).ok_or_else(|| {
            failure(
                "thread_not_found",
                format!("Thread {} doesn't exist.", thread_id.0),
            )
        })?;
        Ok(Caller {
            project_id: thread.project_id,
            thread_id: Some(thread_id),
        })
    }

    fn run_tool(
        &mut self,
        caller: Caller,
        name: &str,
        arguments: &Value,
        timed_out: bool,
    ) -> Outcome {
        let empty = Map::new();
        let arguments = Arguments(match arguments {
            Value::Object(arguments) => arguments,
            Value::Null => &empty,
            _ => return Err(invalid("The arguments must be an object.")),
        });
        if let Some(outcome) = self.relay_if_elsewhere(caller, name, &arguments) {
            return outcome;
        }
        let request_key = arguments.string("clientRequestId", 256)?.map(|request_id| {
            format!(
                "{}:{:?}:{name}:{request_id}",
                caller.project_id.0,
                caller.thread_id.map(|thread_id| thread_id.0)
            )
        });
        // A delegation finds its own earlier task, whose status may have changed since.
        let request_key = request_key.filter(|_| name != "delegate_task");
        if let Some(key) = &request_key
            && let Some(value) = self.tool_results.get(key)
        {
            return Ok(Step::Done(value.clone()));
        }
        let step = match name {
            "orchestrator_capabilities" => self.capabilities(caller).map(|step| match step {
                Step::Done(mut capabilities) => {
                    self.add_machines(caller, &mut capabilities);
                    Step::Done(capabilities)
                }
                step => step,
            }),
            "agentz_thread_list" => match self.thread_list(caller, &arguments)? {
                Step::Done(local) => Ok(self.list_everywhere(caller, &arguments, local)),
                step => Ok(step),
            },
            "agentz_thread_read" => self.thread_read(caller, &arguments, timed_out),
            "agentz_thread_launch" => {
                let mut spec = self.launch_spec(caller, &arguments, "prompt")?;
                let placement = std::mem::take(&mut spec.placement);
                self.with_folders(caller, vec![placement], move |server, folders| {
                    let folder = folders.into_iter().next().unwrap_or(Folder::Default);
                    Ok(Step::Done(server.launch(caller, spec, folder)))
                })
            }
            "create_threads" => self.create_threads(caller, &arguments),
            "agentz_thread_send" => self.thread_send(caller, &arguments),
            "agentz_thread_wait" => self.thread_wait(caller, &arguments, timed_out),
            "agentz_thread_interrupt" => self.thread_interrupt(caller, &arguments),
            "agentz_thread_update" => self.thread_update(caller, &arguments),
            "agentz_thread_organize" => self.thread_organize(caller, &arguments),
            "delegate_task" => self.delegate_task(caller, &arguments),
            "task_status" => {
                let task = self.task(caller, arguments.thread_id("taskId")?)?;
                Ok(Step::Done(self.task_result(task, false, true)))
            }
            "task_cancel" => self.task_cancel(caller, &arguments),
            "agentz_thread_diff" => self.thread_diff_tool(caller, &arguments),
            "agentz_workspace_status" => self.workspace_status(caller),
            "agentz_workspace_list" => self.workspace_list(caller, &arguments),
            "agentz_workspace_handoff" => self.workspace_handoff(caller, &arguments),
            "agentz_workspace_sync" => self.workspace_sync(caller, &arguments),
            "agentz_workspace_bring_back" => self.workspace_bring_back(caller, &arguments),
            "agentz_terminal_list" => self.terminal_list(caller),
            "agentz_terminal_start" => self.terminal_start(caller, &arguments),
            "agentz_terminal_send" => self.terminal_send(caller, &arguments),
            "agentz_terminal_read" => self.terminal_read(caller, &arguments),
            "agentz_terminal_wait" => self.terminal_wait(caller, &arguments, timed_out),
            _ => Err(invalid(format!("There is no tool named {name}."))),
        }?;
        Ok(match (request_key, step) {
            (Some(key), Step::Done(value)) => {
                self.tool_results.insert(key, value.clone());
                Step::Done(value)
            }
            // Remembered once the work is done.
            (Some(key), Step::Then(work)) => Step::Then(
                async move {
                    let then = work.await;
                    Box::new(move |server: &mut Server| {
                        let outcome = then(server);
                        if let Ok(Step::Done(value)) = &outcome {
                            server.tool_results.insert(key, value.clone());
                        }
                        outcome
                    }) as Continuation
                }
                .boxed(),
            ),
            (_, step) => step,
        })
    }

    fn capabilities(&mut self, caller: Caller) -> Outcome {
        // A thread started in a workspace pane belongs to no project, and works in its folder.
        let (project_id, project_name, project_path) =
            match self.projects.project(caller.project_id) {
                Some(project) => (
                    Some(project.id.0),
                    Some(project.name().to_string()),
                    project.path.clone(),
                ),
                None if caller.project_id == ProjectId::WORKSPACES => {
                    let folder = caller
                        .thread_id
                        .and_then(|thread_id| self.projects.thread_folder(thread_id))
                        .ok_or_else(|| failure("thread_not_found", "This thread was deleted."))?;
                    (None, None, folder)
                }
                None => return Err(failure("orchestration_error", "The project was removed.")),
            };
        let caller_thread = caller
            .thread_id
            .and_then(|thread_id| self.projects.thread(thread_id));
        let caller_agent = caller_thread.and_then(|thread| thread.agent_id.clone());
        let caller_agent_name = caller_agent
            .as_ref()
            .map(|id| self.agent_name(&AgentId::new(id.clone())).to_string());
        let agents: Vec<Value> = self
            .registry_snapshot()
            .agents
            .iter()
            .filter(|agent| matches!(agent.install_state, InstallState::Installed { .. }))
            .map(|agent| {
                let settings = self.agent_settings.get(agent.id());
                let models = model_choices(&settings.known_config_options)
                    .map(|(_, choices, _)| {
                        choices
                            .into_iter()
                            .map(|(id, name)| json!({"id": id, "name": name}))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let modes = settings
                    .known_modes
                    .as_ref()
                    .map(|modes| {
                        modes
                            .available_modes
                            .iter()
                            .map(|mode| json!({"id": mode.id.0.as_ref(), "name": mode.name}))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                json!({
                    "agentId": agent.id().0.as_ref(),
                    "name": agent.name().as_ref(),
                    "models": models,
                    "modes": modes,
                    "canRunChildTask": true,
                })
            })
            .collect();
        Ok(Step::Done(json!({
            "currentThreadId": caller.thread_id.map(|thread_id| thread_id.0),
            "projectId": project_id,
            "projectName": project_name,
            "projectPath": project_path,
            "agentId": caller_agent,
            "agentName": caller_agent_name,
            "model": caller_thread.and_then(|thread| thread.model.clone()),
            "machine": {
                "id": self.machine.id,
                "hostname": self.machine.hostname,
                "os": self.machine.os,
                "arch": self.machine.arch,
                "status": "connected",
            },
            "agents": agents,
            "features": {
                "threadManagement": true,
                "batchThreadCreation": true,
                "maxBatchThreads": MAX_BATCH_THREADS,
                "incrementalThreadRead": true,
                "appOwnedSubagents": true,
                "diffs": true,
                "workspaces": true,
                "terminals": true,
            },
        })))
    }

    fn thread_list(&self, caller: Caller, arguments: &Arguments) -> Outcome {
        let statuses = match arguments.array("statuses")? {
            Some(statuses) => Some(
                statuses
                    .iter()
                    .map(|status| {
                        status
                            .as_str()
                            .ok_or_else(|| invalid("statuses must be strings."))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            None => None,
        };
        let title_contains = arguments
            .string("titleContains", 256)?
            .map(|title| title.to_lowercase());
        let archived = arguments.bool("archived")?.unwrap_or(false);
        let cursor = arguments.number("cursor")?.unwrap_or(0) as usize;
        let limit = arguments
            .number("limit")?
            .unwrap_or(DEFAULT_LIST_LIMIT)
            .clamp(1, 100) as usize;

        let mut threads: Vec<&projects::Thread> = self
            .projects
            .threads()
            .iter()
            .filter(|thread| thread.project_id == caller.project_id && thread.task.is_none())
            .filter(|thread| thread.archived_at.is_some() == archived)
            .filter(|thread| {
                title_contains
                    .as_ref()
                    .is_none_or(|text| thread.title.to_lowercase().contains(text))
            })
            .filter(|thread| {
                statuses
                    .as_ref()
                    .is_none_or(|statuses| statuses.contains(&self.thread_status(thread.id)))
            })
            .collect();
        threads.sort_by_key(|thread| std::cmp::Reverse(thread.id));
        let total = threads.len();
        let page: Vec<Value> = threads
            .iter()
            .skip(cursor)
            .take(limit)
            .map(|thread| self.thread_summary(caller, thread))
            .collect();
        let next_cursor = (cursor + page.len() < total).then_some(cursor + page.len());
        Ok(Step::Done(json!({
            "projectId": caller.project_id.0,
            "currentThreadId": caller.thread_id.map(|thread_id| thread_id.0),
            "threads": page,
            "nextCursor": next_cursor,
            "total": total,
        })))
    }

    fn thread_read(&mut self, caller: Caller, arguments: &Arguments, timed_out: bool) -> Outcome {
        let thread_id = self.target(caller, arguments.thread_id("threadId")?)?;
        let activity = match arguments.string("view", 16)? {
            None | Some("messages") => false,
            Some("activity") => true,
            Some(view) => return Err(invalid(format!("Unknown view {view}."))),
        };
        let after = arguments.number("afterPosition")?;
        let limit = arguments
            .number("limit")?
            .unwrap_or(DEFAULT_READ_LIMIT)
            .clamp(1, 100) as usize;
        let max_chars = arguments
            .number("maxCharsPerItem")?
            .unwrap_or(DEFAULT_CHARS_PER_ITEM)
            .clamp(1, MAX_CHARS_PER_ITEM) as usize;

        // The conversation lives with the agent, so a thread that isn't running is started
        // and read once its session has loaded, as opening it in the app would.
        if !timed_out {
            if !self.threads.contains_key(&thread_id) {
                self.start(thread_id)?;
                return Ok(Step::Wait(LOAD_WAIT));
            }
            if self.threads.get(&thread_id).is_some_and(|thread| {
                *thread.status() == ConnectionStatus::Connecting
                    && thread.session_restore().is_none()
            }) {
                return Ok(Step::Wait(LOAD_WAIT));
            }
        }

        let thread = self
            .projects
            .thread(thread_id)
            .ok_or_else(|| failure("thread_not_found", "The thread was deleted."))?;
        let mut detail = self.thread_summary(caller, thread);
        let Some(agent_thread) = self.threads.get(&thread_id) else {
            return Err(failure(
                "orchestration_error",
                "The thread's agent stopped.",
            ));
        };
        let start = after.map_or(0, |after| after as usize + 1);
        let mut items = Vec::new();
        let mut has_more = false;
        for (position, entry) in agent_thread.entries().iter().enumerate().skip(start) {
            let item = match entry {
                Entry::UserMessage(text) => {
                    let from = agent_thread.prompt_sender(position);
                    let mut item = text_item(position, "user_message", text, max_chars);
                    item["createdBy"] = json!(if from.is_some() { "agent" } else { "user" });
                    if let Some(ThreadCreator::Thread(thread_id)) = from {
                        item["createdByThreadId"] = json!(thread_id.0);
                    }
                    item
                }
                Entry::AgentMessage(text) => text_item(position, "agent_message", text, max_chars),
                Entry::Plan => {
                    let plan: Vec<String> = agent_thread
                        .plan()
                        .iter()
                        .map(|item| format!("- [{}] {}", plan_status(&item.status), item.content))
                        .collect();
                    text_item(position, "plan", &plan.join("\n"), max_chars)
                }
                Entry::AgentThought(text) if activity => {
                    text_item(position, "thought", text, max_chars)
                }
                Entry::ToolCall(tool_call) if activity => {
                    let mut item =
                        text_item(position, "tool_call", &tool_call.text.join("\n"), max_chars);
                    item["title"] = json!(tool_call.title);
                    item["status"] = serde_json::to_value(&tool_call.status).unwrap_or_default();
                    item["kind"] = serde_json::to_value(&tool_call.kind).unwrap_or_default();
                    item
                }
                Entry::AgentThought(_) | Entry::ToolCall(_) => continue,
            };
            if items.len() == limit {
                has_more = true;
                break;
            }
            items.push(item);
        }
        let next_position = items.last().and_then(|item| item["position"].as_u64());
        detail["itemCount"] = json!(agent_thread.entries().len());
        detail["pendingApprovalCount"] = json!(agent_thread.state.permission_requests.len());
        detail["turnError"] = json!(agent_thread.turn_error().map(|error| error.to_string()));
        if let ConnectionStatus::Failed(error) = agent_thread.status() {
            detail["error"] = json!(error.to_string());
        }
        Ok(Step::Done(json!({
            "thread": detail,
            "items": items,
            "nextPosition": next_position,
            "hasMore": has_more,
        })))
    }

    fn create_threads(&mut self, caller: Caller, arguments: &Arguments) -> Outcome {
        let requests = arguments
            .array("threads")?
            .ok_or_else(|| invalid("threads is required."))?;
        if requests.is_empty() || requests.len() > MAX_BATCH_THREADS {
            return Err(invalid(format!(
                "threads must have 1 to {MAX_BATCH_THREADS} entries."
            )));
        }
        // All are checked before any is created.
        let mut specs = requests
            .iter()
            .map(|request| match request {
                Value::Object(request) => self.launch_spec(caller, &Arguments(request), "prompt"),
                _ => Err(invalid("Each thread must be an object.")),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let placements = specs
            .iter_mut()
            .map(|spec| std::mem::take(&mut spec.placement))
            .collect();
        self.with_folders(caller, placements, move |server, folders| {
            let threads: Vec<Value> = specs
                .into_iter()
                .zip(folders)
                .map(|(spec, folder)| server.launch(caller, spec, folder))
                .collect();
            Ok(Step::Done(json!({ "threads": threads })))
        })
    }

    fn launch_spec(
        &self,
        caller: Caller,
        arguments: &Arguments,
        prompt_key: &str,
    ) -> Result<LaunchSpec, Failure> {
        let prompt = arguments
            .string(prompt_key, MAX_PROMPT_CHARS)?
            .map(String::from);
        let title = arguments
            .string("title", MAX_TITLE_CHARS)?
            .map(String::from);
        let caller_thread = caller
            .thread_id
            .and_then(|thread_id| self.projects.thread(thread_id));
        let agent_id = match arguments.string("agentId", 256)? {
            Some(agent_id) => AgentId::new(agent_id.to_string()),
            None => caller_thread
                .and_then(|thread| thread.agent_id.clone())
                .map(AgentId::new)
                .ok_or_else(|| invalid("agentId is required outside a thread."))?,
        };
        let registry = self.registry_snapshot();
        if !matches!(
            registry.install_state(&agent_id),
            InstallState::Installed { .. }
        ) {
            let installed: Vec<String> = registry
                .agents
                .iter()
                .filter(|agent| matches!(agent.install_state, InstallState::Installed { .. }))
                .map(|agent| agent.id().0.to_string())
                .collect();
            return Err(failure(
                "provider_unavailable",
                format!(
                    "{} isn't installed. Installed agents: {}.",
                    agent_id.0,
                    installed.join(", ")
                ),
            ));
        }

        let same_agent_as_caller = caller_thread
            .is_some_and(|thread| thread.agent_id.as_deref() == Some(agent_id.0.as_ref()));
        let caller_agent_thread = caller
            .thread_id
            .and_then(|thread_id| self.threads.get(&thread_id))
            .filter(|_| same_agent_as_caller);
        let caller_options = caller_agent_thread
            .map(|thread| thread.config_options().to_vec())
            .unwrap_or_default();
        let mode = caller_agent_thread
            .and_then(|thread| thread.modes())
            .map(|modes| modes.current_mode_id.clone());
        // Agents that put their permission mode in a setting rather than an ACP mode.
        let mode_option = select_choices(&caller_options, acp::SessionConfigOptionCategory::Mode)
            .map(|(config_id, _, current)| (config_id, current));
        let known_options = if caller_options.is_empty() {
            self.agent_settings.get(&agent_id).known_config_options
        } else {
            caller_options
        };
        let model = match arguments.string("model", 256)? {
            Some(requested) => {
                let Some((config_id, choices, _)) = model_choices(&known_options) else {
                    return Err(failure(
                        "model_unavailable",
                        format!(
                            "{} hasn't reported its models yet. Leave model out to use its default.",
                            agent_id.0
                        ),
                    ));
                };
                let Some((value, _)) = choices.iter().find(|(id, name)| {
                    id.eq_ignore_ascii_case(requested) || name.eq_ignore_ascii_case(requested)
                }) else {
                    let names: Vec<&str> = choices.iter().map(|(id, _)| id.as_str()).collect();
                    return Err(failure(
                        "model_unavailable",
                        format!(
                            "{} has no model {requested}. Its models: {}.",
                            agent_id.0,
                            names.join(", ")
                        ),
                    ));
                };
                Some((config_id, value.clone()))
            }
            // t3code's rule: the caller's model carries over to a thread of the same agent.
            None if same_agent_as_caller => {
                model_choices(&known_options).map(|(config_id, _, current)| (config_id, current))
            }
            None => None,
        };
        Ok(LaunchSpec {
            prompt,
            title,
            agent_id,
            model,
            mode,
            mode_option,
            placement: workspace_strategy(arguments)?,
        })
    }

    fn launch(&mut self, caller: Caller, spec: LaunchSpec, folder: Folder) -> Value {
        let Some(thread_id) = self.projects.add_thread(
            caller.project_id,
            projects::NEW_THREAD_TITLE,
            Some(spec.agent_id.0.to_string()),
        ) else {
            return json!({"error": "The project was removed."});
        };
        // A launched thread doesn't inherit the caller's workspace, as in t3code. Outside every
        // project there's no checkout to start in, so it works where the caller does.
        let folder = match folder {
            Folder::Chosen(folder) => Some(folder),
            Folder::Default if caller.project_id == ProjectId::WORKSPACES => Some(
                caller
                    .thread_id
                    .and_then(|thread_id| self.projects.thread_folder(thread_id)),
            ),
            Folder::Default => None,
        };
        if let Some(folder) = folder {
            self.projects.set_thread_workspace(thread_id, folder);
        }
        self.projects
            .set_thread_creator(thread_id, caller.creator());
        let model = spec.model.as_ref().map(|(_, value)| value.clone());
        self.start_launched(thread_id, caller.creator(), spec);
        let mut summary = self
            .projects
            .thread(thread_id)
            .map(|thread| self.thread_summary(caller, thread))
            .unwrap_or_default();
        if let Some(model) = model {
            summary["model"] = json!(model);
        }
        summary
    }

    /// Starts a new thread's agent with the spec's title, settings and prompt.
    fn start_launched(&mut self, thread_id: ThreadId, creator: ThreadCreator, spec: LaunchSpec) {
        if let Some(title) = spec.title {
            self.projects.set_custom_title(thread_id, title);
        }
        let mut defaults = self.agent_settings.get(&spec.agent_id).session_defaults();
        for (config_id, value) in spec.model.iter().chain(&spec.mode_option) {
            defaults.config_options.retain(|(id, _)| id != config_id);
            defaults.config_options.push((
                config_id.clone(),
                acp::SessionConfigOptionValue::value_id(value.clone()),
            ));
        }
        if spec.mode.is_some() {
            defaults.mode = spec.mode;
        }
        if let Err(error) = self.update_thread(ConnectionId::Thread(thread_id), |thread| {
            thread.set_defaults(defaults);
            if let Some(prompt) = spec.prompt {
                thread.send_from(prompt, creator);
            }
        }) {
            log::error!("failed to start a launched thread: {error:#}");
        }
    }

    fn delegate_task(&mut self, caller: Caller, arguments: &Arguments) -> Outcome {
        let Some(parent) = caller.thread_id else {
            return Err(failure(
                "capability_denied",
                "Only a thread's agent can delegate tasks. From a shell, pass --thread.",
            ));
        };
        if self
            .projects
            .thread(parent)
            .and_then(|thread| thread.task.as_ref())
            .is_some_and(|task| task.outcome.is_some())
        {
            return Err(failure(
                "parent_not_active",
                "This task has ended, so it can't delegate more.",
            ));
        }
        let wait = match arguments.string("mode", 16)? {
            None | Some("async") => false,
            Some("wait") => true,
            Some(mode) => return Err(invalid(format!("Unknown mode {mode}."))),
        };
        let timeout = arguments
            .number("timeoutMs")?
            .map_or(DEFAULT_WAIT, Duration::from_millis)
            .min(MAX_WAIT);
        let role = match arguments.string("role", 32)? {
            Some(role) if TASK_ROLES.contains(&role) => Some(role.to_string()),
            Some(role) => {
                return Err(invalid(format!(
                    "Unknown role {role}. Roles: {}.",
                    TASK_ROLES.join(", ")
                )));
            }
            None => None,
        };
        let client_request_id = arguments.string("clientRequestId", 256)?.map(String::from);
        let existing = client_request_id.as_ref().and_then(|request_id| {
            self.projects
                .subthreads(parent)
                .into_iter()
                .find(|thread| {
                    thread
                        .task
                        .as_ref()
                        .and_then(|task| task.client_request_id.as_ref())
                        == Some(request_id)
                })
                .map(|thread| thread.id)
        });
        let answer = move |server: &mut Server, task: ThreadId| -> Outcome {
            if wait {
                server
                    .wait_for_task(caller, task, false)
                    .map(|step| match step {
                        Step::Wait(_) => Step::WaitForTask(task, timeout),
                        step => step,
                    })
            } else {
                Ok(Step::Done(server.task_result(task, false, false)))
            }
        };
        if let Some(task) = existing {
            return answer(self, task);
        }
        let mut spec = self.launch_spec(caller, arguments, "task")?;
        let Some(prompt) = spec.prompt.clone() else {
            return Err(invalid("task is required."));
        };
        let placement = std::mem::take(&mut spec.placement);
        self.with_folders(caller, vec![placement], move |server, folders| {
            let agent_id = spec.agent_id.0.to_string();
            let task = server
                .projects
                .add_subthread(
                    Task {
                        parent,
                        prompt,
                        role,
                        client_request_id,
                        outcome: None,
                        delivered: false,
                    },
                    Some(agent_id),
                )
                .ok_or_else(|| failure("thread_not_found", "This thread was deleted."))?;
            // Otherwise the task works where its parent does.
            if let Some(Folder::Chosen(folder)) = folders.into_iter().next() {
                server.projects.set_thread_workspace(task, folder);
            }
            server.start_launched(task, ThreadCreator::Thread(parent), spec);
            answer(server, task)
        })
    }

    /// `delegate_task`'s wait: the result once the task ends, or `waitTimedOut`.
    fn wait_for_task(&mut self, caller: Caller, task: ThreadId, timed_out: bool) -> Outcome {
        let task = self.task(caller, Some(task))?;
        let ended = self
            .projects
            .thread(task)
            .and_then(|thread| thread.task.as_ref())
            .is_some_and(|task| task.outcome.is_some());
        if !ended && !timed_out {
            return Ok(Step::Wait(MAX_WAIT));
        }
        Ok(Step::Done(self.task_result(task, !ended, true)))
    }

    fn task_cancel(&mut self, caller: Caller, arguments: &Arguments) -> Outcome {
        let task = self.task(caller, arguments.thread_id("taskId")?)?;
        let reason = arguments.string("reason", 2_000)?.map(String::from);
        if let Some(outcome) = self
            .projects
            .thread(task)
            .and_then(|thread| thread.task.as_ref())
            .and_then(|task| task.outcome.as_ref())
        {
            return Ok(Step::Done(json!({
                "taskId": task.0,
                "status": outcome.end.as_str(),
            })));
        }
        self.cancel_task(task, reason);
        Ok(Step::Done(json!({
            "taskId": task.0,
            "status": "cancel_requested",
        })))
    }

    /// Ends the task as cancelled, stops its agent's turn, and cancels its own tasks.
    fn cancel_task(&mut self, task: ThreadId, reason: Option<String>) {
        for thread_id in self.projects.thread_and_subthreads(task) {
            let ended = self
                .projects
                .thread(thread_id)
                .and_then(|thread| thread.task.as_ref())
                .is_none_or(|task| task.outcome.is_some());
            if ended {
                continue;
            }
            let summary = match (&reason, thread_id == task) {
                (Some(reason), true) => reason.clone(),
                (None, true) => "Cancelled by the parent.".to_string(),
                (_, false) => "Cancelled with the task that delegated it.".to_string(),
            };
            self.projects.update_task(thread_id, |task| {
                task.outcome = Some(TaskOutcome {
                    end: TaskEnd::Cancelled,
                    summary: Some(summary),
                    ended_at: SystemTime::now(),
                });
                // The parent asked, so it isn't told again.
                task.delivered = true;
            });
            self.follow_ups.remove(&thread_id);
            if self
                .threads
                .get(&thread_id)
                .is_some_and(|thread| thread.is_working())
            {
                self.update_thread(ConnectionId::Thread(thread_id), |thread| thread.cancel())
                    .log_err();
            }
        }
    }

    /// A task the caller delegated.
    fn task(&self, caller: Caller, task: Option<ThreadId>) -> Result<ThreadId, Failure> {
        let task = task.ok_or_else(|| invalid("taskId is required."))?;
        match self
            .projects
            .thread(task)
            .and_then(|thread| thread.task.as_ref())
        {
            Some(delegated) if caller.thread_id == Some(delegated.parent) => Ok(task),
            _ => Err(failure(
                "task_not_found",
                format!("This thread has no task {}.", task.0),
            )),
        }
    }

    /// The task as `delegate_task` and `task_status` describe it. With `acknowledge`, an
    /// ended task's result counts as delivered to the parent.
    fn task_result(&mut self, task: ThreadId, wait_timed_out: bool, acknowledge: bool) -> Value {
        let (status, work_state) = self.task_status(task);
        let Some(thread) = self.projects.thread(task) else {
            return json!({"taskId": task.0, "status": "cancelled"});
        };
        let Some(delegated) = thread.task.as_ref() else {
            return json!({"taskId": task.0});
        };
        let has_pending_tasks = self.projects.subthreads(task).iter().any(|thread| {
            thread
                .task
                .as_ref()
                .is_some_and(|task| task.outcome.is_none())
        });
        let result = json!({
            "taskId": task.0,
            "childThreadId": task.0,
            "title": thread.title,
            "role": delegated.role,
            "status": status,
            "workState": work_state,
            "hasPendingChildTasks": has_pending_tasks,
            "agentId": thread.agent_id,
            "agentName": thread
                .agent_id
                .as_ref()
                .map(|id| self.agent_name(&AgentId::new(id.clone())).to_string()),
            "model": thread.model,
            "summary": delegated.outcome.as_ref().and_then(|outcome| outcome.summary.clone()),
            "endedAt": delegated.outcome.as_ref().map(|outcome| timestamp(outcome.ended_at)),
            "waitTimedOut": wait_timed_out,
        });
        let parent = delegated.parent;
        if acknowledge && delegated.outcome.is_some() && !delegated.delivered {
            self.projects
                .update_task(task, |task| task.delivered = true);
            if let Some(queue) = self.follow_ups.get_mut(&parent) {
                queue.retain(|follow_up| follow_up.task != Some(task));
            }
        }
        result
    }

    /// t3code's delegated task status and work state.
    fn task_status(&self, task: ThreadId) -> (&'static str, &'static str) {
        if let Some(outcome) = self
            .projects
            .thread(task)
            .and_then(|thread| thread.task.as_ref())
            .and_then(|task| task.outcome.as_ref())
        {
            return (outcome.end.as_str(), "result_available");
        }
        let status = match self.thread_status(task) {
            "starting" => "queued",
            "waiting_for_approval" | "waiting_for_input" | "needs_login" => "waiting",
            _ => "running",
        };
        let work_state = if !self.is_busy(task) && self.has_unannounced_tasks(task) {
            "waiting_for_children"
        } else {
            "working"
        };
        (status, work_state)
    }

    /// The thread has delegated tasks that are running, or have ended without it hearing.
    pub(super) fn has_unannounced_tasks(&self, thread_id: ThreadId) -> bool {
        self.projects.subthreads(thread_id).iter().any(|thread| {
            thread
                .task
                .as_ref()
                .is_some_and(|task| task.outcome.is_none() || !task.delivered)
        })
    }

    fn thread_send(&mut self, caller: Caller, arguments: &Arguments) -> Outcome {
        let thread_id = self.target(caller, arguments.thread_id("threadId")?)?;
        let message = arguments
            .string("message", MAX_PROMPT_CHARS)?
            .ok_or_else(|| invalid("message is required."))?
            .to_string();
        let restart = match arguments.string("mode", 16)? {
            None | Some("auto") | Some("queue") => false,
            Some("restart") => true,
            Some(mode) => return Err(invalid(format!("Unknown mode {mode}."))),
        };
        if caller.thread_id == Some(thread_id) {
            return Err(failure(
                "thread_not_sendable",
                "A thread can't send messages to itself.",
            ));
        }
        if self
            .projects
            .thread(thread_id)
            .is_some_and(|thread| thread.task.is_some())
        {
            return Err(failure(
                "thread_not_sendable",
                "The thread is a delegated task. Use task_status or task_cancel.",
            ));
        }
        self.start(thread_id)?;
        match self.thread_status(thread_id) {
            "failed" => {
                return Err(failure(
                    "thread_not_sendable",
                    "The thread's agent failed to start.",
                ));
            }
            "needs_login" => {
                return Err(failure(
                    "thread_not_sendable",
                    "The thread's agent needs the user to log in.",
                ));
            }
            _ => {}
        }
        let from = caller.creator();
        let delivery = if !self.is_busy(thread_id) {
            self.update_thread(ConnectionId::Thread(thread_id), |thread| {
                thread.send_from(message, from)
            })
            .map_err(|error| failure("orchestration_error", format!("{error:#}")))?;
            "started"
        } else if restart {
            // A restart cancels the turn and sends the message once it has stopped (t3code's
            // interrupt-and-restart).
            self.follow_ups
                .entry(thread_id)
                .or_default()
                .push_front(FollowUp {
                    text: message,
                    from,
                    task: None,
                });
            self.update_thread(ConnectionId::Thread(thread_id), |thread| thread.cancel())
                .map_err(|error| failure("orchestration_error", format!("{error:#}")))?;
            "restarted"
        } else {
            self.follow_ups
                .entry(thread_id)
                .or_default()
                .push_back(FollowUp {
                    text: message,
                    from,
                    task: None,
                });
            "queued"
        };
        Ok(Step::Done(json!({
            "threadId": thread_id.0,
            "delivery": delivery,
            "status": self.thread_status(thread_id),
        })))
    }

    fn thread_wait(&mut self, caller: Caller, arguments: &Arguments, timed_out: bool) -> Outcome {
        let thread_id = self.target(caller, arguments.thread_id("threadId")?)?;
        if caller.thread_id == Some(thread_id) {
            return Err(invalid("A thread can't wait for itself."));
        }
        let timeout = arguments
            .number("timeoutMs")?
            .map_or(DEFAULT_WAIT, Duration::from_millis)
            .min(MAX_WAIT);
        let busy = self.is_busy(thread_id);
        if busy && !timed_out {
            return Ok(Step::Wait(timeout));
        }
        let (last_message, truncated) = self
            .threads
            .get(&thread_id)
            .and_then(|thread| last_agent_message(thread.entries()))
            .map(|message| truncate(message, MAX_LAST_MESSAGE_CHARS))
            .map_or((None, false), |(message, truncated)| {
                (Some(message), truncated)
            });
        Ok(Step::Done(json!({
            "threadId": thread_id.0,
            "status": self.thread_status(thread_id),
            "timedOut": busy,
            "lastAgentMessage": last_message,
            "lastAgentMessageTruncated": truncated,
            "turnError": self
                .threads
                .get(&thread_id)
                .and_then(|thread| thread.turn_error())
                .map(|error| error.to_string()),
        })))
    }

    fn thread_interrupt(&mut self, caller: Caller, arguments: &Arguments) -> Outcome {
        let thread_id = self.target(caller, arguments.thread_id("threadId")?)?;
        if caller.thread_id == Some(thread_id) {
            return Err(failure(
                "thread_not_interruptible",
                "A thread can't interrupt itself.",
            ));
        }
        let dropped = self
            .follow_ups
            .remove(&thread_id)
            .map_or(0, |queue| queue.len());
        let working = self
            .threads
            .get(&thread_id)
            .is_some_and(|thread| thread.is_working());
        if working {
            self.update_thread(ConnectionId::Thread(thread_id), |thread| thread.cancel())
                .map_err(|error| failure("orchestration_error", format!("{error:#}")))?;
        }
        Ok(Step::Done(json!({
            "threadId": thread_id.0,
            "status": if working { "interrupt_requested" } else { "no_active_run" },
            "droppedQueuedMessages": dropped,
        })))
    }

    fn thread_update(&mut self, caller: Caller, arguments: &Arguments) -> Outcome {
        let thread_id = match arguments.thread_id("threadId")? {
            Some(thread_id) => self.target(caller, Some(thread_id))?,
            None => self.target(caller, caller.thread_id)?,
        };
        match arguments.string("action", 32)? {
            None | Some("rename") => {}
            Some(action) => return Err(invalid(format!("Unknown action {action}."))),
        }
        let title = arguments
            .string("title", MAX_TITLE_CHARS)?
            .ok_or_else(|| invalid("title is required to rename."))?
            .to_string();
        self.projects.set_custom_title(thread_id, title.clone());
        Ok(Step::Done(json!({
            "threadId": thread_id.0,
            "title": title,
        })))
    }

    fn thread_organize(&mut self, caller: Caller, arguments: &Arguments) -> Outcome {
        let thread_id = match arguments.thread_id("threadId")? {
            Some(thread_id) => self.target(caller, Some(thread_id))?,
            None => self.target(caller, caller.thread_id)?,
        };
        match arguments.string("action", 32)? {
            Some("pin") => self
                .projects
                .pin_thread(thread_id, None)
                .map_err(|error| invalid(error.to_string()))?,
            Some("unpin") => self.projects.unpin_thread(thread_id),
            Some("archive") => self.projects.archive_thread(thread_id),
            Some("unarchive") => self.projects.unarchive_thread(thread_id),
            Some(action) => return Err(invalid(format!("Unknown action {action}."))),
            None => {
                return Err(invalid(
                    "action is required: pin, unpin, archive or unarchive.",
                ));
            }
        }
        let thread = self.projects.thread(thread_id);
        Ok(Step::Done(json!({
            "threadId": thread_id.0,
            "pinned": thread.is_some_and(Thread::is_pinned),
            "archived": thread.is_some_and(|thread| thread.archived_at.is_some()),
        })))
    }

    fn thread_diff_tool(&mut self, caller: Caller, arguments: &Arguments) -> Outcome {
        let thread_id = self.target(
            caller,
            arguments.thread_id("threadId")?.or(caller.thread_id),
        )?;
        let scope = match arguments.string("scope", 16)? {
            None | Some("all") => DiffScope::All,
            Some("latest_turn") => DiffScope::LatestTurn,
            Some(scope) => return Err(invalid(format!("Unknown scope {scope}."))),
        };
        let include_patch = match arguments.string("format", 16)? {
            None | Some("patch") => true,
            Some("files") => false,
            Some(format) => return Err(invalid(format!("Unknown format {format}."))),
        };
        let max_chars = arguments
            .number("maxChars")?
            .unwrap_or(DEFAULT_PATCH_CHARS)
            .clamp(1, MAX_PATCH_CHARS) as usize;
        let checkpoints = self
            .checkpoints(thread_id)
            .ok_or_else(|| failure("thread_not_found", "The thread was deleted."))?;
        Ok(Step::Background(
            async move {
                let diff = checkpoints
                    .diff(scope)
                    .await
                    .map_err(|error| failure("orchestration_error", format!("{error:#}")))?;
                let files: Vec<Value> = agentz_protocol::diff::parse_patch(&diff.patch)
                    .into_iter()
                    .map(|file| {
                        json!({
                            "path": file.path,
                            "oldPath": file.old_path,
                            "change": match file.change {
                                FileChange::Added => "added",
                                FileChange::Deleted => "deleted",
                                FileChange::Modified => "modified",
                                FileChange::Renamed => "renamed",
                            },
                            "binary": file.binary,
                            "additions": file.additions,
                            "deletions": file.deletions,
                        })
                    })
                    .collect();
                let status = match diff.status {
                    DiffStatus::Ready => "ready",
                    DiffStatus::NotRepository => "not_repository",
                    DiffStatus::NoTurns | DiffStatus::Unknown(_) => "no_turns",
                };
                let mut result = json!({
                    "threadId": thread_id.0,
                    "status": status,
                    "scope": match scope {
                        DiffScope::LatestTurn => "latest_turn",
                        DiffScope::All
                        | DiffScope::Turn(_)
                        | DiffScope::WorkingTree
                        | DiffScope::Branch => "all",
                    },
                    "turns": diff.turns,
                    "files": files,
                    "truncated": diff.truncated,
                });
                if include_patch {
                    let (patch, cut) = truncate(&diff.patch, max_chars);
                    result["patch"] = json!(patch);
                    result["truncated"] = json!(diff.truncated || cut);
                }
                Ok(result)
            }
            .boxed(),
        ))
    }

    /// A thread the caller may manage: one in its project.
    fn target(&self, caller: Caller, thread_id: Option<ThreadId>) -> Result<ThreadId, Failure> {
        let thread_id = thread_id.ok_or_else(|| invalid("threadId is required."))?;
        // Threads of other projects are reported as missing, so they can't be probed for.
        match self.projects.thread(thread_id) {
            Some(thread) if thread.project_id == caller.project_id => Ok(thread_id),
            _ => Err(failure(
                "thread_not_found",
                format!("There is no thread {} in this project.", thread_id.0),
            )),
        }
    }

    /// Starts the thread's agent if it isn't running.
    fn start(&mut self, thread_id: ThreadId) -> Result<(), Failure> {
        if self.threads.contains_key(&thread_id) {
            return Ok(());
        }
        self.update_thread(ConnectionId::Thread(thread_id), |_| {})
            .map_err(|error| failure("orchestration_error", format!("{error:#}")))
    }

    fn thread_status(&self, thread_id: ThreadId) -> &'static str {
        let Some(thread) = self.threads.get(&thread_id) else {
            return "idle";
        };
        let has_follow_ups = self.follow_ups.contains_key(&thread_id);
        match thread.status() {
            ConnectionStatus::Failed(_) => "failed",
            ConnectionStatus::AuthRequired => "needs_login",
            _ if !thread.state.permission_requests.is_empty() => "waiting_for_approval",
            _ if thread.is_awaiting_input() => "waiting_for_input",
            _ if thread.is_working() => "running",
            _ if has_follow_ups => "queued",
            ConnectionStatus::Connecting => "starting",
            _ => "idle",
        }
    }

    /// Whether the thread has a turn running, or messages from agents waiting to start one.
    pub(super) fn is_busy(&self, thread_id: ThreadId) -> bool {
        matches!(
            self.thread_status(thread_id),
            "running" | "waiting_for_approval" | "waiting_for_input" | "queued"
        )
    }

    fn thread_summary(&self, caller: Caller, thread: &projects::Thread) -> Value {
        json!({
            "threadId": thread.id.0,
            "title": thread.title,
            "status": self.thread_status(thread.id),
            "agentId": thread.agent_id,
            "agentName": thread
                .agent_id
                .as_ref()
                .map(|id| self.agent_name(&AgentId::new(id.clone())).to_string()),
            "model": thread.model,
            "pinned": thread.is_pinned(),
            "archived": thread.archived_at.is_some(),
            "createdBy": if thread.created_by.is_some() { "agent" } else { "user" },
            "createdByThreadId": match thread.created_by {
                Some(ThreadCreator::Thread(thread_id)) => Some(thread_id.0),
                _ => None,
            },
            "parentThreadId": thread.parent().map(|parent| parent.0),
            "isCaller": caller.thread_id == Some(thread.id),
            "lastActivityAt": thread.last_activity_at.map(timestamp),
            "completedAt": thread.completed_at.map(timestamp),
        })
    }
}

struct LaunchSpec {
    prompt: Option<String>,
    title: Option<String>,
    agent_id: AgentId,
    model: Option<(acp::SessionConfigId, String)>,
    /// The caller's mode, for a thread of the same agent: a delegated task never gets more
    /// room than its parent.
    mode: Option<acp::SessionModeId>,
    mode_option: Option<(acp::SessionConfigId, String)>,
    placement: Placement,
}

/// Ends the tasks that were running when the server last stopped. Their parents are told once
/// it's running again.
pub(super) fn interrupt_unfinished_tasks(projects: &mut ProjectStore) {
    let unfinished: Vec<ThreadId> = projects
        .threads()
        .iter()
        .filter(|thread| {
            thread
                .task
                .as_ref()
                .is_some_and(|task| task.outcome.is_none())
        })
        .map(|thread| thread.id)
        .collect();
    for task in unfinished {
        projects.update_task(task, |task| {
            task.outcome = Some(TaskOutcome {
                end: TaskEnd::Interrupted,
                summary: Some("agentZ's server stopped while the task was running.".to_string()),
                ended_at: SystemTime::now(),
            });
        });
    }
}

fn task_ended_message(tasks: &[ThreadId]) -> String {
    let ids: Vec<String> = tasks.iter().map(|task| task.0.to_string()).collect();
    match ids.as_slice() {
        [id] => format!(
            "Delegated task {id} reached a terminal state. Use task_status with taskId {id} to read the result."
        ),
        _ => format!(
            "Delegated tasks {} reached terminal states. Use task_status with each taskId to read the results.",
            ids.join(", ")
        ),
    }
}

struct Arguments<'a>(&'a Map<String, Value>);

impl Arguments<'_> {
    /// A trimmed, non-empty string of at most `max_chars`.
    fn string(&self, key: &str, max_chars: usize) -> Result<Option<&str>, Failure> {
        match self.0.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(text)) => {
                let text = text.trim();
                if text.is_empty() {
                    return Err(invalid(format!("{key} can't be empty.")));
                }
                if text.chars().count() > max_chars {
                    return Err(invalid(format!(
                        "{key} is over {max_chars} characters long."
                    )));
                }
                Ok(Some(text))
            }
            Some(_) => Err(invalid(format!("{key} must be a string."))),
        }
    }

    fn bool(&self, key: &str) -> Result<Option<bool>, Failure> {
        match self.0.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Bool(value)) => Ok(Some(*value)),
            Some(_) => Err(invalid(format!("{key} must be true or false."))),
        }
    }

    fn number(&self, key: &str) -> Result<Option<u64>, Failure> {
        match self.0.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(value) => value
                .as_u64()
                .or_else(|| value.as_f64().filter(|n| *n >= 0.0).map(|n| n as u64))
                .map(Some)
                .ok_or_else(|| invalid(format!("{key} must be a non-negative number."))),
        }
    }

    /// Thread ids are numbers, but agents often quote them.
    fn thread_id(&self, key: &str) -> Result<Option<ThreadId>, Failure> {
        match self.0.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Number(number)) => number
                .as_u64()
                .map(|id| Some(ThreadId(id)))
                .ok_or_else(|| invalid(format!("{key} must be a thread id."))),
            Some(Value::String(text)) => text
                .trim()
                .parse()
                .map(|id| Some(ThreadId(id)))
                .map_err(|_| invalid(format!("{key} must be a thread id."))),
            Some(_) => Err(invalid(format!("{key} must be a thread id."))),
        }
    }

    fn array(&self, key: &str) -> Result<Option<&Vec<Value>>, Failure> {
        match self.0.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Array(values)) => Ok(Some(values)),
            Some(_) => Err(invalid(format!("{key} must be an array."))),
        }
    }
}

fn tool_result(result: Result<Value, Failure>) -> ToolResult {
    match result {
        Ok(value) => ToolResult {
            value,
            is_error: false,
        },
        Err(failure) => ToolResult {
            value: json!({"code": failure.code, "message": failure.message}),
            is_error: true,
        },
    }
}

/// The model selector among the options: its id, its choices (value id and name), and the
/// current value.
fn model_choices(
    options: &[acp::SessionConfigOption],
) -> Option<(acp::SessionConfigId, Vec<(String, String)>, String)> {
    select_choices(options, acp::SessionConfigOptionCategory::Model)
}

/// The first selector of the category among the options, as for [`model_choices`].
fn select_choices(
    options: &[acp::SessionConfigOption],
    category: acp::SessionConfigOptionCategory,
) -> Option<(acp::SessionConfigId, Vec<(String, String)>, String)> {
    options.iter().find_map(|option| {
        if option.category.as_ref() != Some(&category) {
            return None;
        }
        let acp::SessionConfigKind::Select(select) = &option.kind else {
            return None;
        };
        let choices: Vec<(String, String)> = match &select.options {
            acp::SessionConfigSelectOptions::Ungrouped(choices) => choices
                .iter()
                .map(|choice| (choice.value.0.to_string(), choice.name.clone()))
                .collect(),
            acp::SessionConfigSelectOptions::Grouped(groups) => groups
                .iter()
                .flat_map(|group| &group.options)
                .map(|choice| (choice.value.0.to_string(), choice.name.clone()))
                .collect(),
            _ => Vec::new(),
        };
        Some((
            option.id.clone(),
            choices,
            select.current_value.0.to_string(),
        ))
    })
}

fn text_item(position: usize, kind: &str, text: &str, max_chars: usize) -> Value {
    let (text, truncated) = truncate(text, max_chars);
    json!({
        "position": position,
        "type": kind,
        "text": text,
        "textTruncated": truncated,
    })
}

fn truncate(text: &str, max_chars: usize) -> (String, bool) {
    match text.char_indices().nth(max_chars) {
        Some((end, _)) => (text[..end].to_string(), true),
        None => (text.to_string(), false),
    }
}

/// The agent's answer to the last user message.
fn last_agent_message(entries: &[Entry]) -> Option<&str> {
    entries
        .iter()
        .rev()
        .take_while(|entry| !matches!(entry, Entry::UserMessage(_)))
        .find_map(|entry| match entry {
            Entry::AgentMessage(text) => Some(text.as_str()),
            _ => None,
        })
}

fn plan_status(status: &acp::PlanEntryStatus) -> &'static str {
    match status {
        acp::PlanEntryStatus::Completed => "x",
        acp::PlanEntryStatus::InProgress => "~",
        _ => " ",
    }
}

fn timestamp(time: SystemTime) -> String {
    chrono::DateTime::<chrono::Utc>::from(time).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// The tools as MCP tool definitions, for `tools/list`. Names, descriptions and behavior follow
/// t3code's orchestrator tools, with `agentz_` for `t3_`.
pub(super) fn definitions() -> Value {
    let thread_id =
        json!({"type": "integer", "description": "A thread id from agentz_thread_list."});
    let client_request_id = json!({
        "type": "string",
        "maxLength": 256,
        "description": "Stable idempotency key to reuse when retrying this mutation.",
    });
    let workspace_strategy = |default: &str| {
        json!({
            "type": "object",
            "description": format!("Where the thread works. {default} type=worktree or pasture makes a new one on a new branch (branch, default agentz/<short id>) from baseRef (default: the checkout's current branch); a pasture is a copy-on-write clone of the whole project folder, so dependencies, .env and caches come along. type=existing reuses one of the project's worktrees or pastures by path, from agentz_workspace_list. type=root is the project's own checkout."),
            "properties": {
                "type": {"type": "string", "enum": ["root", "worktree", "pasture", "existing"]},
                "baseRef": {"type": "string", "maxLength": 256},
                "branch": {"type": "string", "maxLength": 256},
                "path": {"type": "string", "maxLength": 4096},
            },
            "required": ["type"],
            "additionalProperties": false,
        })
    };
    let launch = json!({
        "type": "object",
        "properties": {
            "prompt": {"type": "string", "maxLength": MAX_PROMPT_CHARS, "description": "The first message, a complete task for the new thread's agent. Without it the thread starts idle."},
            "title": {"type": "string", "maxLength": MAX_TITLE_CHARS, "description": "Optional concise title. Without it, the prompt names the thread."},
            "agentId": {"type": "string", "description": "An installed agent from orchestrator_capabilities. Defaults to this thread's agent."},
            "model": {"type": "string", "description": "A model id or name the agent advertises in orchestrator_capabilities. Defaults to this thread's model for the same agent, or the agent's default."},
            "workspaceStrategy": workspace_strategy("Defaults to the project's own checkout, whatever this thread's workspace."),
        },
        "additionalProperties": false,
    });
    let mut launch_with_request_id = launch.clone();
    launch_with_request_id["properties"]["clientRequestId"] = client_request_id.clone();
    let mut tools = json!([
        {
            "name": "orchestrator_capabilities",
            "title": "Get orchestration capabilities",
            "description": "List the agents installed on this machine with the models and modes each advertises, this thread's agent and model, the machine, and the agentZ thread-management features available to this thread.",
            "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false},
            "annotations": {"readOnlyHint": true, "destructiveHint": false, "idempotentHint": true},
        },
        {
            "name": "agentz_thread_list",
            "title": "List agentZ threads",
            "description": "List agentZ threads in the calling thread's project, newest first. Filter by status (idle, starting, running, waiting_for_approval, waiting_for_input, queued, needs_login, failed), title, or archived state, and paginate with the returned cursor. Threads from other projects are never exposed.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "statuses": {"type": "array", "items": {"type": "string", "enum": ["idle", "starting", "running", "waiting_for_approval", "waiting_for_input", "queued", "needs_login", "failed"]}, "maxItems": 10},
                    "titleContains": {"type": "string", "maxLength": 256},
                    "archived": {"type": "boolean", "description": "true lists only archived threads; the default lists only active ones."},
                    "cursor": {"type": "integer", "minimum": 0},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 100},
                },
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": true, "destructiveHint": false, "idempotentHint": true},
        },
        {
            "name": "agentz_thread_read",
            "title": "Read an agentZ thread",
            "description": "Read a thread's state and a paginated timeline, from a thread in the calling project. The default messages view returns user messages, agent messages and plans; activity also returns thoughts and summarized tool calls. Continue with afterPosition=nextPosition while hasMore is true. Long item text is cut at maxCharsPerItem and marked textTruncated.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "threadId": thread_id,
                    "view": {"type": "string", "enum": ["messages", "activity"]},
                    "afterPosition": {"type": "integer", "minimum": 0, "description": "Return items after this position."},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 100},
                    "maxCharsPerItem": {"type": "integer", "minimum": 1, "maximum": MAX_CHARS_PER_ITEM},
                },
                "required": ["threadId"],
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": true, "destructiveHint": false, "idempotentHint": true},
        },
        {
            "name": "agentz_thread_launch",
            "title": "Launch an agentZ thread",
            "description": "Create one ORDINARY TOP-LEVEL agentZ thread in this project, optionally with a first prompt, agent and model. Use it only when the user asks for a separate or new thread or conversation. The new thread is marked as created by this thread's agent. Use agentz_thread_wait or agentz_thread_read to follow its work.",
            "inputSchema": launch_with_request_id,
            "annotations": {"readOnlyHint": false, "destructiveHint": true, "openWorldHint": true},
        },
        {
            "name": "create_threads",
            "title": "Create agentZ threads",
            "description": "Create one or more ORDINARY TOP-LEVEL agentZ threads in this project, sharing this checkout. This is not delegation. Prefer agentz_thread_launch for a single thread. Both require the user to request separate, new or top-level threads or conversations. Each entry may choose its agent and model; omitted settings inherit.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "threads": {"type": "array", "items": launch, "minItems": 1, "maxItems": MAX_BATCH_THREADS},
                    "clientRequestId": client_request_id,
                },
                "required": ["threads"],
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": true, "openWorldHint": true},
        },
        {
            "name": "agentz_thread_send",
            "title": "Send to an agentZ thread",
            "description": "Send a message to another agentZ thread in the calling project. mode='auto' (the default) starts an idle thread's turn or queues behind a running one; queue always waits for the current turn; restart interrupts the running turn and sends the message once it has stopped. The message is marked as sent by this thread's agent. clientRequestId makes retries idempotent.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "threadId": thread_id,
                    "message": {"type": "string", "maxLength": MAX_PROMPT_CHARS},
                    "mode": {"type": "string", "enum": ["auto", "queue", "restart"]},
                    "clientRequestId": client_request_id,
                },
                "required": ["threadId", "message"],
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": true, "openWorldHint": true},
        },
        {
            "name": "agentz_thread_wait",
            "title": "Wait for an agentZ thread",
            "description": "Wait until another thread in the calling project finishes its current turn and the messages queued for it, then return its status and last agent message. An idle thread returns immediately. timeoutMs defaults to 10 minutes; a timeout returns timedOut=true and doesn't interrupt the work, so call again or read the thread later.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "threadId": thread_id,
                    "timeoutMs": {"type": "integer", "minimum": 0, "maximum": MAX_WAIT.as_millis() as u64},
                },
                "required": ["threadId"],
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": true, "destructiveHint": false, "idempotentHint": true},
        },
        {
            "name": "agentz_thread_interrupt",
            "title": "Interrupt an agentZ thread",
            "description": "Stop the running turn of another thread in the calling project, and drop the messages agents queued for it. A thread without a running turn returns no_active_run. clientRequestId makes retries idempotent.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "threadId": thread_id,
                    "reason": {"type": "string", "maxLength": 2000},
                    "clientRequestId": client_request_id,
                },
                "required": ["threadId"],
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": true},
        },
        {
            "name": "agentz_thread_update",
            "title": "Rename an agentZ thread",
            "description": "Rename a thread in the calling project with action='rename' and title. Omit threadId to rename this thread. The new title replaces automatic titles, as a rename by the user does.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "threadId": thread_id,
                    "action": {"type": "string", "enum": ["rename"]},
                    "title": {"type": "string", "maxLength": MAX_TITLE_CHARS},
                    "clientRequestId": client_request_id,
                },
                "required": ["title"],
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": true, "idempotentHint": true},
        },
        {
            "name": "agentz_thread_organize",
            "title": "Organize an agentZ thread",
            "description": "Pin, unpin, archive or unarchive a thread in the calling project. Omit threadId for this thread. Pinned threads list first in the sidebar; pinning an archived thread brings it back. Archiving doesn't stop a turn in progress; archived threads move to Thread History, unpinned. Deleting threads is left to the user.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "threadId": thread_id,
                    "action": {"type": "string", "enum": ["pin", "unpin", "archive", "unarchive"]},
                    "clientRequestId": client_request_id,
                },
                "required": ["action"],
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": false, "idempotentHint": true},
        },
        {
            "name": "agentz_thread_diff",
            "title": "Read an agentZ thread's changes",
            "description": "Read the file changes a thread in the calling project made, from the git checkpoints agentZ takes before its first turn and after each turn. Omit threadId for this thread. scope='all' (the default) covers every finished turn; latest_turn only the last one. Changes are those of the thread's folder, so they include edits by anyone else working there at the same time. A turn still running isn't included yet. format='files' returns only the changed files with line counts; the default patch also returns the unified diff, cut at maxChars. status is not_repository outside git and no_turns before a turn has finished.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "threadId": thread_id,
                    "scope": {"type": "string", "enum": ["all", "latest_turn"]},
                    "format": {"type": "string", "enum": ["patch", "files"]},
                    "maxChars": {"type": "integer", "minimum": 1, "maximum": MAX_PATCH_CHARS},
                },
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": true, "destructiveHint": false, "idempotentHint": true},
        },
        {
            "name": "delegate_task",
            "title": "Delegate a child task",
            "description": "Delegate one task to an agentZ-owned child agent (a subagent) of THIS thread, which runs it with only the supplied task prompt, without the parent's conversation. Choose agents and models from orchestrator_capabilities. Prefer your own native subagent tools for same-agent work when they support the chosen model; use this for other agents or models, or for explicitly agentZ-owned child tasks. The agent and model inherit from this thread unless given, and the child keeps this thread's mode. The child is not an ordinary top-level thread: it shows in this thread's Agents control. Prefer mode='async' for long work: when the task ends, this thread gets a message saying so, queued until its turn ends, so end the turn instead of polling. mode='wait' blocks until the task ends or timeoutMs (default 10 minutes) passes; a timeout returns waitTimedOut=true and doesn't cancel the task. Keep the taskId for task_status and task_cancel.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task": {"type": "string", "maxLength": MAX_PROMPT_CHARS, "description": "A self-contained task for one delegated child agent."},
                    "title": {"type": "string", "maxLength": MAX_TITLE_CHARS},
                    "role": {"type": "string", "enum": TASK_ROLES},
                    "agentId": {"type": "string", "description": "An installed agent from orchestrator_capabilities. Defaults to this thread's agent."},
                    "model": {"type": "string", "description": "A model id or name the agent advertises. Defaults to this thread's model for the same agent, or the agent's default."},
                    "mode": {"type": "string", "enum": ["async", "wait"], "description": "Defaults to async. Use wait only when this turn needs the result before it can continue."},
                    "timeoutMs": {"type": "integer", "minimum": 0, "maximum": MAX_WAIT.as_millis() as u64, "description": "How long mode=wait waits. It doesn't cancel the task."},
                    "workspaceStrategy": workspace_strategy("Defaults to this thread's own folder, so parallel tasks that edit files should each get a worktree or pasture."),
                    "clientRequestId": client_request_id,
                },
                "required": ["task"],
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": true, "openWorldHint": true},
        },
        {
            "name": "agentz_workspace_status",
            "title": "Get this thread's workspace",
            "description": "Report where this thread works: kind (checkout, worktree or pasture), whether it's attached to a worktree or pasture, its folder and branch, the branch it started from, and the project's own checkout. Call this before agentz_workspace_handoff to see whether a handoff is possible.",
            "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false},
            "annotations": {"readOnlyHint": true, "destructiveHint": false, "idempotentHint": true},
        },
        {
            "name": "agentz_workspace_list",
            "title": "List branches and workspaces",
            "description": "List the project repository's local branches, most recently committed first, with the checkouts each is checked out in, and the project's worktrees and pastures with the threads working in each. pastures tells whether new pastures are copy_on_write, a full_copy (slow and as big as the project) or unsupported here.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "maxLength": 256, "description": "Only branches whose name contains this."},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 1000},
                },
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": true, "destructiveHint": false, "idempotentHint": true},
        },
        {
            "name": "agentz_workspace_handoff",
            "title": "Hand off this thread to a new workspace",
            "description": "Move this thread into a new git worktree or pasture on a new branch (branch, default agentz/<short id>) from baseRef (default: the checkout's current branch). A worktree has only tracked files; a pasture is a copy-on-write clone of the whole project folder, with dependencies, .env and caches. To launch a separate agent in a new or existing workspace, use agentz_thread_launch or delegate_task with workspaceStrategy instead. The agent restarts in the new folder once this turn ends, with the conversation kept when the agent can load sessions, so call this as the last action of the turn. To keep working, pass continuationPrompt with the remaining work: it starts the next turn there. Uncommitted changes in the checkout don't come along to a worktree. Fails if this thread already works in a worktree or pasture.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "type": {"type": "string", "enum": ["worktree", "pasture"]},
                    "branch": {"type": "string", "maxLength": 256},
                    "baseRef": {"type": "string", "maxLength": 256},
                    "continuationPrompt": {"type": "string", "maxLength": MAX_PROMPT_CHARS},
                    "clientRequestId": client_request_id,
                },
                "required": ["type"],
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": true, "idempotentHint": false, "openWorldHint": true},
        },
        {
            "name": "agentz_workspace_sync",
            "title": "Sync this pasture from the project",
            "description": "For a thread in a pasture: fetch a branch from the project's checkout (default: the branch the pasture started from) and rebase the pasture's branch onto it, or merge with strategy=merge. Commit first: uncommitted changes to tracked files stop it. On conflicts the rebase or merge is undone and the conflicting files are named. Worktrees share the project's branches, so they don't need this.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "branch": {"type": "string", "maxLength": 256},
                    "strategy": {"type": "string", "enum": ["rebase", "merge"]},
                },
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": true},
        },
        {
            "name": "agentz_workspace_bring_back",
            "title": "Bring this pasture's branch to the project",
            "description": "For a thread in a pasture: create a branch in the project's checkout (default: the pasture's branch) at the pasture's HEAD, or fast-forward it there, ready to review and push from there. Only committed work comes along. A branch the checkout has checked out can't be updated; sync first if the branch has moved on.",
            "inputSchema": {
                "type": "object",
                "properties": {"branch": {"type": "string", "maxLength": 256}},
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": true},
        },
        {
            "name": "task_status",
            "title": "Get delegated task status",
            "description": "Read a task this thread delegated. status is queued, running, waiting (for the user's approval, input or login), completed, failed, cancelled or interrupted. workState tells working, waiting_for_children (its turn ended but its own tasks are still running) and result_available apart. summary is the task's result once it has ended: the child's last message, or the error. Reading an ended task's result means this thread isn't sent a message about it.",
            "inputSchema": {
                "type": "object",
                "properties": {"taskId": {"type": "integer", "description": "The taskId from delegate_task."}},
                "required": ["taskId"],
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": false, "idempotentHint": true},
        },
        {
            "name": "task_cancel",
            "title": "Cancel delegated task",
            "description": "Stop a task this thread delegated, and the tasks it delegated in turn. This thread isn't sent a message about it. An ended task's result stays available.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "taskId": {"type": "integer"},
                    "reason": {"type": "string", "maxLength": 2000},
                    "clientRequestId": client_request_id,
                },
                "required": ["taskId"],
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": true},
        },
    ]);
    if let Value::Array(tools) = &mut tools {
        tools.extend(terminals::definitions());
        let machine = json!({
            "type": "string",
            "maxLength": 256,
            "description": "Another machine with this project, by its name in orchestrator_capabilities' otherMachines. Thread ids are per machine, so pass it again for that machine's threads. Omit for this machine.",
        });
        for tool in tools {
            let is_relayed = tool["name"]
                .as_str()
                .is_some_and(|name| relay::RELAYED_TOOLS.contains(&name));
            if is_relayed
                && let Some(properties) = tool["inputSchema"]["properties"].as_object_mut()
            {
                properties.insert("machine".into(), machine.clone());
            }
        }
    }
    tools
}
