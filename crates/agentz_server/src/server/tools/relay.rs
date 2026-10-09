//! Agent control across machines. A server only knows its own machine, so calls naming
//! another one go through the app, which reaches every machine: it sends each server its other
//! machines and the projects it combines with the server's ([`Peers`]), and runs relayed calls
//! there for the combined project's checkout ([`RelayToolCall`]). Without the app open, other
//! machines are out of reach.
//!
//! Threads keep living on their own machine. A task delegated to another machine is an
//! ordinary thread there, followed with `machine` on the thread tools, since its parent's
//! lineage can't span two servers.
//!
//! The Workspaces view's terminals and adding a project need no project, so they reach every
//! machine the app does: an agent can clone a repository there and add it, and once the app
//! combines it with the caller's project, the rest of the tools work there too.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use agentz_protocol::{Event, PeerMachine, Peers, RelayToolCall, ServerMessage, ToolResult};
use collections::HashMap;
use futures::FutureExt as _;
use futures::channel::oneshot;
use futures::future::{BoxFuture, join_all};
use projects::ProjectId;
use serde_json::{Map, Value, json};

use super::{Arguments, Caller, Continuation, Failure, Outcome, Server, Step, failure, invalid};
use crate::server::ClientId;

/// The tools that take `machine`, run on that machine's checkout of the project, or for
/// [`MACHINE_TOOLS`], on the machine.
pub(super) const RELAYED_TOOLS: [&str; 19] = [
    "orchestrator_capabilities",
    "agentz_thread_list",
    "agentz_thread_read",
    "agentz_thread_launch",
    "create_threads",
    "agentz_thread_send",
    "agentz_thread_wait",
    "agentz_thread_interrupt",
    "agentz_thread_update",
    "agentz_thread_organize",
    "agentz_thread_diff",
    "delegate_task",
    "agentz_workspace_list",
    "agentz_terminal_list",
    "agentz_terminal_start",
    "agentz_terminal_send",
    "agentz_terminal_read",
    "agentz_terminal_wait",
    "agentz_project_add",
];

/// The relayed tools that also work on a machine without the caller's project: the
/// Workspaces view's terminals, and adding a project. The project's checkout goes along when
/// there is one, for its terminals.
pub(super) const MACHINE_TOOLS: [&str; 6] = [
    "agentz_terminal_list",
    "agentz_terminal_start",
    "agentz_terminal_send",
    "agentz_terminal_read",
    "agentz_terminal_wait",
    "agentz_project_add",
];

/// How long adding a project on another machine waits for the app to combine it with the
/// caller's, when it's the same repository.
#[cfg(not(test))]
const COMBINE_WAIT: Duration = Duration::from_secs(10);
#[cfg(test)]
const COMBINE_WAIT: Duration = Duration::from_secs(2);

/// The list asks machines the caller didn't name, so one that doesn't answer is left out
/// rather than holding up the rest.
#[cfg(not(test))]
const LIST_RELAY_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(test)]
const LIST_RELAY_TIMEOUT: Duration = Duration::from_secs(2);

/// Codes a relayed failure keeps; others become `orchestration_error`.
const FAILURE_CODES: [&str; 19] = [
    "account_unavailable",
    "invalid_request",
    "operation_failed",
    "orchestration_error",
    "project_not_found",
    "thread_not_found",
    "already_in_workspace",
    "capability_denied",
    "handoff_in_progress",
    "model_unavailable",
    "not_in_pasture",
    "parent_not_active",
    "provider_unavailable",
    "task_not_found",
    "terminal_exited",
    "thread_not_interruptible",
    "thread_not_sendable",
    "machine_unavailable",
    "no_active_run",
];

#[derive(Default)]
pub(in crate::server) struct Relays {
    /// What each app client sent, the latest last; that one relays.
    peers: Vec<(ClientId, Peers)>,
    pending: HashMap<u64, (ClientId, oneshot::Sender<ToolResult>)>,
    next_id: u64,
    /// Told when the peers change.
    peer_waiters: Vec<oneshot::Sender<()>>,
}

impl Relays {
    pub(in crate::server) fn set_peers(&mut self, client: ClientId, peers: Peers) {
        self.peers.retain(|(other, _)| *other != client);
        self.peers.push((client, peers));
        for waiter in self.peer_waiters.drain(..) {
            // The call waiting may have gone meanwhile.
            waiter.send(()).ok();
        }
    }

    fn peers_changed(&mut self) -> oneshot::Receiver<()> {
        self.peer_waiters.retain(|waiter| !waiter.is_canceled());
        let (sender, receiver) = oneshot::channel();
        self.peer_waiters.push(sender);
        receiver
    }

    /// The project's checkouts the app combines with it.
    fn checkouts(&self, project_id: Option<ProjectId>) -> Option<&agentz_protocol::PeerCheckouts> {
        let project_id = project_id?;
        self.latest()?
            .1
            .checkouts
            .iter()
            .find(|checkouts| checkouts.project_id == project_id)
    }

    pub(in crate::server) fn finish(&mut self, relay_id: u64, result: ToolResult) {
        if let Some((_, sender)) = self.pending.remove(&relay_id) {
            // The call's caller may have gone meanwhile.
            sender.send(result).ok();
        }
    }

    /// Dropping the client's relays fails the calls waiting on them.
    pub(in crate::server) fn client_gone(&mut self, client: ClientId) {
        self.peers.retain(|(other, _)| *other != client);
        self.pending.retain(|_, (owner, _)| *owner != client);
    }

    fn latest(&self) -> Option<(ClientId, &Peers)> {
        self.peers.last().map(|(client, peers)| (*client, peers))
    }

    fn is_this_machine(&self, machine: &str) -> bool {
        self.latest()
            .is_some_and(|(_, peers)| peers.this_machine.eq_ignore_ascii_case(machine))
    }
}

/// Where a relayed call runs.
struct Target {
    client: ClientId,
    machine: String,
    /// The project's checkout there; `None` for [`MACHINE_TOOLS`] on a machine without it.
    path: Option<PathBuf>,
}

impl Server {
    /// Runs the call on the machine it names, when that's another one.
    pub(super) fn relay_if_elsewhere(
        &mut self,
        caller: Caller,
        name: &str,
        arguments: &Arguments,
    ) -> Option<Outcome> {
        let machine = match arguments.string("machine", 256) {
            Ok(Some(machine)) => machine,
            Ok(None) => return None,
            Err(failure) => return Some(Err(failure)),
        };
        if self.relays.is_this_machine(machine) || machine.eq_ignore_ascii_case("this machine") {
            return None;
        }
        if !RELAYED_TOOLS.contains(&name) {
            return Some(Err(invalid(format!("{name} only works on this machine."))));
        }
        let target = match self.relay_target(caller, machine, !MACHINE_TOOLS.contains(&name)) {
            Ok(target) => target,
            Err(failure) => return Some(Err(failure)),
        };
        let mut arguments = arguments.0.clone();
        arguments.remove("machine");
        // The other machine has no calling thread to take the agent from.
        if matches!(name, "agentz_thread_launch" | "delegate_task")
            && !arguments.contains_key("agentId")
            && let Some(agent_id) = caller
                .thread_id
                .and_then(|thread_id| self.projects.thread(thread_id))
                .and_then(|thread| thread.agent_id.clone())
        {
            arguments.insert("agentId".into(), json!(agent_id));
        }
        Some(if name == "delegate_task" {
            self.delegate_elsewhere(target, arguments)
        } else if name == "agentz_project_add" {
            Ok(self.add_project_elsewhere(caller, target, arguments))
        } else {
            Ok(Step::Background(self.relay(
                target,
                name,
                Value::Object(arguments),
            )))
        })
    }

    /// The machine the call names, and the caller's project's checkout there, which only
    /// `needs_project` calls must have.
    fn relay_target(
        &self,
        caller: Caller,
        machine: &str,
        needs_project: bool,
    ) -> Result<Target, Failure> {
        let Some((client, peers)) = self.relays.latest() else {
            return Err(failure(
                "machine_unavailable",
                "Other machines are reached through the agentZ app, which isn't connected.",
            ));
        };
        let Some(known) = peers
            .machines
            .iter()
            .find(|known| known.name.eq_ignore_ascii_case(machine))
        else {
            let names: Vec<&str> = std::iter::once(peers.this_machine.as_str())
                .chain(peers.machines.iter().map(|known| known.name.as_str()))
                .collect();
            return Err(invalid(format!(
                "There's no machine named {machine}. Machines: {}.",
                names.join(", ")
            )));
        };
        if !known.online {
            return Err(failure(
                "machine_unavailable",
                format!("{} isn't connected.", known.name),
            ));
        }
        let path = self
            .relays
            .checkouts(caller.project_id)
            .and_then(|checkouts| {
                checkouts
                    .checkouts
                    .iter()
                    .find(|checkout| checkout.machine.eq_ignore_ascii_case(&known.name))
            })
            .map(|checkout| checkout.path.clone());
        if path.is_none() && needs_project {
            return Err(failure(
                "capability_denied",
                format!(
                    "This project isn't on {}. Clone its repository there in a Workspaces \
                     terminal (agentz_terminal_start with machine and folder) and add it with \
                     agentz_project_add first.",
                    known.name
                ),
            ));
        }
        Ok(Target {
            client,
            machine: known.name.clone(),
            path,
        })
    }

    /// Asks the app to run the call there; the answer is tagged with the machine.
    fn relay(
        &mut self,
        target: Target,
        name: &str,
        arguments: Value,
    ) -> BoxFuture<'static, Result<Value, Failure>> {
        let relay_id = self.relays.next_id;
        self.relays.next_id += 1;
        let (sender, receiver) = oneshot::channel();
        self.relays
            .pending
            .insert(relay_id, (target.client, sender));
        self.send(
            target.client,
            ServerMessage::Event(Event::RelayToolCall(RelayToolCall {
                relay_id,
                machine: target.machine.clone(),
                path: target.path,
                name: name.to_string(),
                arguments,
            })),
        );
        let machine = target.machine;
        async move {
            let result = receiver.await.map_err(|_| {
                failure(
                    "machine_unavailable",
                    format!("The agentZ app stopped relaying to {machine}."),
                )
            })?;
            if result.is_error {
                return Err(relayed_failure(&result.value, &machine));
            }
            Ok(with_machine(result.value, &machine))
        }
        .boxed()
    }

    /// A task for another machine: an ordinary thread there with the task as its prompt, waited
    /// for in `wait` mode.
    fn delegate_elsewhere(&mut self, target: Target, arguments: Map<String, Value>) -> Outcome {
        let wait = match arguments.get("mode").and_then(Value::as_str) {
            None | Some("async") => false,
            Some("wait") => true,
            Some(mode) => return Err(invalid(format!("Unknown mode {mode}."))),
        };
        let mut launch = Map::new();
        for (from, to) in [
            ("task", "prompt"),
            ("title", "title"),
            ("agentId", "agentId"),
            ("model", "model"),
            ("workspaceStrategy", "workspaceStrategy"),
            ("clientRequestId", "clientRequestId"),
        ] {
            if let Some(value) = arguments.get(from) {
                launch.insert(to.into(), value.clone());
            }
        }
        if !launch.contains_key("prompt") {
            return Err(invalid("task is required."));
        }
        let note = format!(
            "The task runs as an ordinary thread on {}. Follow it with agentz_thread_wait or \
             agentz_thread_read, passing machine and threadId.",
            target.machine
        );
        let timeout = arguments.get("timeoutMs").cloned();
        let (client, machine, path) = (target.client, target.machine.clone(), target.path.clone());
        let launched = self.relay(target, "agentz_thread_launch", Value::Object(launch));
        if !wait {
            return Ok(Step::Background(
                async move {
                    let mut launched = launched.await?;
                    launched["note"] = json!(note);
                    Ok(launched)
                }
                .boxed(),
            ));
        }
        Ok(Step::Then(
            async move {
                let launched = launched.await;
                Box::new(move |server: &mut Server| -> Outcome {
                    let launched = launched?;
                    let mut wait = json!({"threadId": launched["threadId"].clone()});
                    if let Some(timeout) = timeout {
                        wait["timeoutMs"] = timeout;
                    }
                    let target = Target {
                        client,
                        machine,
                        path,
                    };
                    let waited = server.relay(target, "agentz_thread_wait", wait);
                    Ok(Step::Background(
                        async move {
                            let mut waited = waited.await?;
                            waited["note"] = json!(note);
                            Ok(waited)
                        }
                        .boxed(),
                    ))
                }) as Continuation
            }
            .boxed(),
        ))
    }

    /// A project added on another machine, and whether the app combined it with the caller's,
    /// which takes a moment: the app hears of the project, then tells this server.
    fn add_project_elsewhere(
        &mut self,
        caller: Caller,
        target: Target,
        arguments: Map<String, Value>,
    ) -> Step {
        let machine = target.machine.clone();
        let added = self.relay(target, "agentz_project_add", Value::Object(arguments));
        let repository = caller
            .project_id
            .and_then(|project_id| self.projects.project(project_id))
            .and_then(|project| project.repository.as_ref())
            .map(|repository| repository.canonical_key.clone());
        Step::Then(
            async move {
                let added = added.await;
                Box::new(move |server: &mut Server| {
                    let added = added?;
                    let same_repository = repository.is_some()
                        && added["repository"].as_str() == repository.as_deref();
                    if !same_repository {
                        return Ok(Step::Done(combined(added, false, false)));
                    }
                    server.until_combined(caller, machine, added, Instant::now() + COMBINE_WAIT)
                }) as Continuation
            }
            .boxed(),
        )
    }

    fn until_combined(
        &mut self,
        caller: Caller,
        machine: String,
        added: Value,
        deadline: Instant,
    ) -> Outcome {
        let path = added["path"].as_str().map(PathBuf::from);
        let is_combined = self
            .relays
            .checkouts(caller.project_id)
            .zip(path)
            .is_some_and(|(checkouts, path)| {
                checkouts.checkouts.iter().any(|checkout| {
                    checkout.machine.eq_ignore_ascii_case(&machine) && checkout.path == path
                })
            });
        if is_combined || Instant::now() >= deadline {
            return Ok(Step::Done(combined(added, true, is_combined)));
        }
        let changed = self.relays.peers_changed();
        Ok(Step::Then(
            async move {
                tokio::time::timeout_at(deadline.into(), changed).await.ok();
                Box::new(move |server: &mut Server| {
                    server.until_combined(caller, machine, added, deadline)
                }) as Continuation
            }
            .boxed(),
        ))
    }

    /// The list from this machine, with the threads of the project's checkouts on the others
    /// when it's the first page. Each thread names its machine.
    pub(super) fn list_everywhere(
        &mut self,
        caller: Caller,
        arguments: &Arguments,
        local: Value,
    ) -> Step {
        if caller.relayed {
            return Step::Done(local);
        }
        let this_machine = self
            .relays
            .latest()
            .map(|(_, peers)| peers.this_machine.clone());
        let Some(this_machine) = this_machine else {
            return Step::Done(local);
        };
        let local = with_machine(local, &this_machine);
        let is_first_page = arguments.0.get("cursor").is_none_or(|cursor| cursor == 0);
        let others: Vec<String> = self
            .relays
            .checkouts(caller.project_id)
            .map(|checkouts| {
                let mut machines: Vec<String> = Vec::new();
                for checkout in &checkouts.checkouts {
                    if !machines.contains(&checkout.machine) {
                        machines.push(checkout.machine.clone());
                    }
                }
                machines
            })
            .unwrap_or_default();
        if !is_first_page || others.is_empty() {
            return Step::Done(local);
        }
        let mut arguments = arguments.0.clone();
        arguments.remove("machine");
        let limit = arguments
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(super::DEFAULT_LIST_LIMIT)
            .clamp(1, 100) as usize;
        let mut lists = Vec::new();
        for machine in others {
            let list = match self.relay_target(caller, &machine, true) {
                Ok(target) => self.relay(
                    target,
                    "agentz_thread_list",
                    Value::Object(arguments.clone()),
                ),
                Err(failure) => async move { Err(failure) }.boxed(),
            };
            lists.push(async move {
                let list = tokio::time::timeout(LIST_RELAY_TIMEOUT, list)
                    .await
                    .unwrap_or_else(|_| {
                        Err(failure(
                            "machine_unavailable",
                            format!(
                                "{machine} didn't answer within {} seconds.",
                                LIST_RELAY_TIMEOUT.as_secs()
                            ),
                        ))
                    });
                (machine, list)
            });
        }
        Step::Background(
            async move {
                let mut merged = local;
                let mut threads: Vec<Value> = merged["threads"].as_array().cloned().unwrap_or_default();
                let mut total = merged["total"].as_u64().unwrap_or(0);
                let mut machines = vec![json!({"machine": this_machine, "nextCursor": merged["nextCursor"].clone()})];
                for (machine, list) in join_all(lists).await {
                    match list {
                        Ok(list) => {
                            threads.extend(list["threads"].as_array().cloned().unwrap_or_default());
                            total += list["total"].as_u64().unwrap_or(0);
                            machines.push(json!({"machine": machine, "nextCursor": list["nextCursor"].clone()}));
                        }
                        Err(failure) => machines.push(
                            json!({"machine": machine, "error": failure.message}),
                        ),
                    }
                }
                // RFC 3339 times in UTC sort as text; threads without activity go last.
                threads.sort_by(|a, b| {
                    let time = |thread: &Value| thread["lastActivityAt"].as_str().map(str::to_string);
                    time(b).cmp(&time(a))
                });
                threads.truncate(limit);
                merged["threads"] = Value::Array(threads);
                merged["total"] = json!(total);
                merged["machines"] = Value::Array(machines);
                Ok(merged)
            }
            .boxed(),
        )
    }

    /// The other machines, for `orchestrator_capabilities`.
    pub(super) fn add_machines(&self, caller: Caller, capabilities: &mut Value) {
        let Some((_, peers)) = self.relays.latest() else {
            return;
        };
        let checkouts = self.relays.checkouts(caller.project_id);
        capabilities["machine"]["name"] = json!(peers.this_machine);
        capabilities["otherMachines"] = peers
            .machines
            .iter()
            .map(|PeerMachine { name, online }| {
                let has_project = checkouts.is_some_and(|checkouts| {
                    checkouts
                        .checkouts
                        .iter()
                        .any(|checkout| &checkout.machine == name)
                });
                json!({
                    "name": name,
                    "status": if *online { "connected" } else { "disconnected" },
                    "hasThisProject": has_project,
                })
            })
            .collect();
        capabilities["features"]["otherMachines"] = json!(
            "Tools that take machine run on that machine's checkout of this project. Pass machine \
             to orchestrator_capabilities for its agents. The terminal tools and \
             agentz_project_add also work on a machine without this project: open a Workspaces \
             terminal there with agentz_terminal_start (machine and folder), clone the \
             repository, then add the clone with agentz_project_add, and the app combines it \
             with this project."
        );
    }
}

/// Says whether a project added on another machine joined the caller's.
fn combined(mut added: Value, same_repository: bool, is_combined: bool) -> Value {
    added["combinedWithThisProject"] = json!(is_combined);
    added["note"] = json!(match (same_repository, is_combined) {
        (_, true) => {
            "The app combined it with this project, so the tools that take machine now work in \
             this checkout."
        }
        (true, false) => {
            "It has this project's repository, but the app hasn't combined them: its project \
             grouping settings may keep them apart."
        }
        (false, false) => "It isn't this project's repository, so it's a project of its own.",
    });
    added
}

/// Names the machine on the result and on each thread in it.
fn with_machine(mut value: Value, machine: &str) -> Value {
    if let Value::Object(object) = &mut value {
        if let Some(Value::Array(threads)) = object.get_mut("threads") {
            for thread in threads {
                if let Value::Object(thread) = thread {
                    thread.insert("machine".into(), json!(machine));
                }
            }
        }
        object.entry("machine").or_insert_with(|| json!(machine));
    }
    value
}

fn relayed_failure(value: &Value, machine: &str) -> Failure {
    let code = value["code"].as_str().unwrap_or_default();
    let code = FAILURE_CODES
        .iter()
        .find(|known| **known == code)
        .copied()
        .unwrap_or("orchestration_error");
    let message = value["message"].as_str().unwrap_or("The call failed.");
    failure(code, format!("On {machine}: {message}"))
}
