//! The accounts menu on each of agentZ's skills and MCP servers (design/accounts decisions.md
//! §20): "Every account" at first, or how many of them load it, over the accounts that could,
//! grouped by agent, with a check each. What's kept is the accounts it's kept off, so an
//! account added later loads it.

use std::rc::Rc;

use agentz_protocol::Request;
use agentz_protocol::accounts::AgentAccount;
use agentz_protocol::agents::{AgentId, AgentListing, InstallState};
use gpui::{AnyElement, App, Entity, Window};
use ui::{ContextMenu, DropdownMenu, DropdownStyle, prelude::*};

use super::{AccountEntry, account_entries, account_selector, render_account_entry};
use crate::server_client::ServerClient;

/// One of agentZ's skills or MCP servers, by its name.
#[derive(Clone, Debug)]
pub(super) enum KeptOffItem {
    Skill(String),
    McpServer(String),
}

impl KeptOffItem {
    /// The accounts it's kept off, as the client last heard.
    fn kept_off(&self, client: &ServerClient) -> Vec<AgentAccount> {
        match self {
            KeptOffItem::Skill(name) => client
                .skills()
                .iter()
                .find(|skill| skill.name == *name)
                .map(|skill| skill.kept_off.clone()),
            KeptOffItem::McpServer(name) => client
                .mcp_servers()
                .iter()
                .find(|server| server.name == *name)
                .map(|server| server.kept_off.clone()),
        }
        .unwrap_or_default()
    }

    fn request(&self, kept_off: Vec<AgentAccount>) -> Request {
        match self {
            KeptOffItem::Skill(name) => Request::SetSkillKeptOff {
                name: name.clone(),
                kept_off,
            },
            KeptOffItem::McpServer(name) => Request::SetMcpServerKeptOff {
                name: name.clone(),
                kept_off,
            },
        }
    }

    fn selector(&self) -> String {
        match self {
            KeptOffItem::Skill(name) => format!("skill-accounts-{name}"),
            KeptOffItem::McpServer(name) => format!("mcp-server-accounts-{name}"),
        }
    }
}

/// An installed agent whose accounts could load an item, with its listed accounts.
pub(super) struct AccountGroup {
    agent_id: AgentId,
    agent_name: SharedString,
    entries: Vec<AccountEntry>,
}

/// The installed agents that `takes` says could load the item, each with its listed accounts.
pub(super) fn account_groups(
    agents: &[AgentListing],
    client: &ServerClient,
    takes: impl Fn(&AgentListing) -> bool,
) -> Vec<AccountGroup> {
    agents
        .iter()
        .filter(|agent| matches!(agent.install_state, InstallState::Installed { .. }))
        .filter(|agent| takes(agent))
        .map(|agent| AccountGroup {
            agent_id: agent.id().clone(),
            agent_name: agent.name().clone(),
            entries: account_entries(&client.accounts(agent.id())),
        })
        .filter(|group| !group.entries.is_empty())
        .collect()
}

/// The menu, beside the row's other controls. With a single account in all, there's nothing
/// to choose, and none.
pub(super) fn render_accounts_menu(
    item: KeptOffItem,
    groups: Vec<AccountGroup>,
    client: Entity<ServerClient>,
    send: Rc<dyn Fn(Request, &mut App)>,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    let accounts: Vec<AgentAccount> = groups
        .iter()
        .flat_map(|group| {
            group.entries.iter().map(|entry| AgentAccount {
                agent_id: group.agent_id.clone(),
                account: entry.account,
            })
        })
        .collect();
    if accounts.len() < 2 {
        return None;
    }
    let kept_off = item.kept_off(client.read(cx));
    let loading = accounts
        .iter()
        .filter(|account| !kept_off.contains(account))
        .count();
    let label = if loading == accounts.len() {
        "Every account".to_string()
    } else {
        format!("{loading} of {}", accounts.len())
    };
    let selector = item.selector();
    let groups = Rc::new(groups);
    let menu_item = item;
    // Rebuilt after each click, from what the client shows, so it stays open on the new checks.
    let menu = ContextMenu::build_persistent(window, cx, move |mut menu, _, cx| {
        menu = menu.keep_open_on_confirm(true);
        let item = menu_item.clone();
        let kept_off = item.kept_off(client.read(cx));
        for group in groups.iter() {
            menu = menu.header(group.agent_name.clone());
            for entry in &group.entries {
                let account = AgentAccount {
                    agent_id: group.agent_id.clone(),
                    account: entry.account,
                };
                let loads = !kept_off.contains(&account);
                let entry = entry.clone();
                let entry_selector = format!(
                    "{}-{}-{}",
                    item.selector(),
                    group.agent_id.0,
                    account_selector(entry.account)
                );
                let (item, client, send) = (item.clone(), client.clone(), send.clone());
                menu = menu.custom_entry(
                    move |_, cx| {
                        let selector = entry_selector.clone();
                        div()
                            .w_full()
                            .debug_selector(move || selector)
                            .child(render_account_entry(&entry, loads, cx))
                            .into_any_element()
                    },
                    move |_, cx| {
                        let mut kept_off = item.kept_off(client.read(cx));
                        if loads {
                            kept_off.push(account.clone());
                            kept_off.sort();
                        } else {
                            kept_off.retain(|other| *other != account);
                        }
                        client.update(cx, |client, cx| match &item {
                            KeptOffItem::Skill(name) => {
                                client.show_skill_kept_off(name, kept_off.clone(), cx)
                            }
                            KeptOffItem::McpServer(name) => {
                                client.show_mcp_server_kept_off(name, kept_off.clone(), cx)
                            }
                        });
                        send(item.request(kept_off), cx);
                    },
                );
            }
        }
        menu
    });
    Some(
        div()
            .debug_selector({
                let selector = selector.clone();
                move || selector
            })
            .child(
                DropdownMenu::new(SharedString::from(selector), label, menu)
                    .style(DropdownStyle::Outlined)
                    .trigger_size(ButtonSize::Compact),
            )
            .into_any_element(),
    )
}
