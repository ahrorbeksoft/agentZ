//! Agents' icons, shared by every machine. They all read the same ACP Registry, so each icon
//! is fetched once, from the first machine to list it (this Mac's server, which connects
//! first), and shows for that agent on every machine, even one that couldn't download it.

use agentz_protocol::agents::{AgentIcon, AgentId, IconId, RegistrySnapshot};
use agentz_protocol::{Request, Response};
use anyhow::{Result, anyhow};
use collections::{HashMap, HashSet};
use futures::future::BoxFuture;
use gpui::{App, AppContext as _, Context, Entity, Global, SharedString};

#[derive(Default)]
pub struct AgentIconStore {
    /// SVG markup, by id.
    icons: HashMap<IconId, SharedString>,
    /// Each agent's icon, as the last machine to list it named it.
    agent_icons: HashMap<AgentId, IconId>,
    /// Asked for and not answered yet.
    requested: HashSet<IconId>,
}

struct GlobalAgentIcons(Entity<AgentIconStore>);

impl Global for GlobalAgentIcons {}

/// Call before `machines::init`, whose clients report their registries here.
pub fn init(cx: &mut App) {
    let store = cx.new(|_| AgentIconStore::default());
    cx.set_global(GlobalAgentIcons(store));
}

/// The agent's icon as SVG markup, from whichever machine had it.
pub fn agent_icon(agent: &AgentId, cx: &App) -> Option<SharedString> {
    let store = cx.try_global::<GlobalAgentIcons>()?.0.read(cx);
    store.icons.get(store.agent_icons.get(agent)?).cloned()
}

impl AgentIconStore {
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalAgentIcons>().0.clone()
    }

    /// Notes the icons a machine's registry names, and fetches those no machine has sent yet
    /// with `fetch` (a request to that machine).
    pub(crate) fn learn(
        &mut self,
        registry: &RegistrySnapshot,
        fetch: impl FnOnce(Request) -> BoxFuture<'static, Result<Response>>,
        cx: &mut Context<Self>,
    ) {
        let mut missing = Vec::new();
        for agent in registry.agents() {
            let Some(id) = agent.icon() else {
                continue;
            };
            self.agent_icons.insert(agent.id().clone(), id.clone());
            if !self.icons.contains_key(id) && self.requested.insert(id.clone()) {
                missing.push(id.clone());
            }
        }
        if missing.is_empty() {
            return;
        }
        let response = fetch(Request::AgentIcons(missing.clone()));
        cx.spawn(async move |this, cx| {
            let icons = match response.await {
                Ok(Response::AgentIcons(icons)) => Ok(icons),
                Ok(response) => Err(anyhow!("unexpected response: {response:?}")),
                Err(error) => Err(error),
            };
            this.update(cx, |this, cx| this.received(&missing, icons, cx))
                .ok();
        })
        .detach();
    }

    fn received(
        &mut self,
        requested: &[IconId],
        icons: Result<Vec<AgentIcon>>,
        cx: &mut Context<Self>,
    ) {
        // Whatever didn't arrive is asked for again when a machine next lists it.
        for id in requested {
            self.requested.remove(id);
        }
        match icons {
            Ok(icons) if !icons.is_empty() => {
                self.icons
                    .extend(icons.into_iter().map(|icon| (icon.id, icon.svg)));
                cx.notify();
                // Icons show in many views; they arrive about once a session.
                cx.refresh_windows();
            }
            Ok(_) => {}
            Err(error) => log::error!("fetching agent icons failed: {error:#}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use agentz_protocol::agents::{AgentListing, InstallState, RegistryAgentMetadata};
    use futures::FutureExt as _;
    use gpui::TestAppContext;

    use super::*;

    fn registry(agents: &[(&str, Option<&str>)]) -> RegistrySnapshot {
        RegistrySnapshot {
            agents: agents
                .iter()
                .map(|(id, icon)| AgentListing {
                    metadata: RegistryAgentMetadata {
                        id: AgentId::new(id.to_string()),
                        name: id.to_string().into(),
                        description: "".into(),
                        version: "1.0.0".into(),
                        repository: None,
                        website: None,
                        license_url: None,
                        icon: icon.map(|icon| IconId(icon.to_string().into())),
                    },
                    supports_current_platform: true,
                    install_state: InstallState::NotInstalled,
                    custom_command: None,
                    accounts: None,
                })
                .collect(),
            ..Default::default()
        }
    }

    #[gpui::test]
    fn icons_are_fetched_once_for_every_machine(cx: &mut TestAppContext) {
        cx.update(init);
        let requests = Rc::new(RefCell::new(Vec::new()));
        let fetch = |requests: &Rc<RefCell<Vec<Request>>>| {
            let requests = requests.clone();
            move |request: Request| {
                requests.borrow_mut().push(request.clone());
                let Request::AgentIcons(ids) = request else {
                    unreachable!("only icons are asked for");
                };
                let icons = ids
                    .into_iter()
                    .map(|id| AgentIcon {
                        svg: format!("<svg>{}</svg>", id.0).into(),
                        id,
                    })
                    .collect();
                futures::future::ready(Ok(Response::AgentIcons(icons))).boxed()
            }
        };

        // This Mac lists both agents, so it's asked for both icons.
        let store = cx.update(|cx| AgentIconStore::global(cx));
        store.update(cx, |store, cx| {
            store.learn(
                &registry(&[("claude", Some("a")), ("codex", Some("b"))]),
                fetch(&requests),
                cx,
            )
        });
        cx.run_until_parked();
        assert_eq!(
            *requests.borrow(),
            vec![Request::AgentIcons(vec![
                IconId("a".into()),
                IconId("b".into())
            ])]
        );

        // Another machine lists the same icons, and one it couldn't download: nothing is
        // fetched from it, and the agent without an icon there still shows one.
        store.update(cx, |store, cx| {
            store.learn(
                &registry(&[("claude", Some("a")), ("codex", None)]),
                fetch(&requests),
                cx,
            )
        });
        cx.run_until_parked();
        assert_eq!(requests.borrow().len(), 1);
        cx.update(|cx| {
            assert_eq!(
                agent_icon(&AgentId::new("codex"), cx).as_deref(),
                Some("<svg>b</svg>")
            );
            assert_eq!(agent_icon(&AgentId::new("gemini"), cx), None);
        });
    }
}
