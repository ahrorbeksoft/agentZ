//! Settings › MCP Servers: Zed's MCP Servers page for agentZ's own MCP servers on a machine
//! (design/accounts decisions.md §19), which every agent and account gets in agentZ threads.
//! Add Server offers Zed's Add Local Server and Add Remote Server, each a form like Zed's.

use std::rc::Rc;

use agent_client_protocol::schema::v1 as acp;
use agentz_protocol::Request;
use agentz_protocol::accounts::AgentAccount;
use agentz_protocol::agents::{AgentId, InstallState};
use agentz_protocol::mcp_servers::{IGNORES_MCP_SERVERS, McpServer, McpTransport};
use gpui::{AnyElement, App, Context, Entity, Focusable, Task, Window};
use text_input::TextInput;
use ui::{ContextMenu, ContextMenuEntry, PopoverMenu, Switch, ToggleState, Tooltip, prelude::*};
use util::ResultExt as _;

use super::accounts_menu::{KeptOffItem, account_groups, render_accounts_menu};
use super::skills::{join_with_and, render_error};
use super::{
    EnvRow, SettingsPage, account_tag, new_text_input, new_variable_row, on_page, render_row,
    render_section, render_section_with_actions,
};
use crate::controls::{ActionButton, ActionStyle, text_field};
use crate::server_client::ServerClient;

/// What Settings › MCP Servers shows.
pub(super) enum McpServersPage {
    List,
    /// Zed's form, adding a server or configuring one.
    Form(McpServerForm),
}

pub(super) struct McpServerForm {
    remote: bool,
    /// The server being configured; `None` adds one.
    replacing: Option<String>,
    /// Kept as they were when one is configured.
    enabled: bool,
    kept_off: Vec<AgentAccount>,
    name: Entity<TextInput>,
    command: Entity<TextInput>,
    /// Split on spaces, as Zed's form does.
    args: Entity<TextInput>,
    env: Vec<EnvRow>,
    url: Entity<TextInput>,
    headers: Vec<EnvRow>,
    error: Option<SharedString>,
    saving: Option<Task<()>>,
}

#[derive(Clone, Copy)]
enum Pairs {
    Env,
    Headers,
}

impl Pairs {
    fn rows(self, form: &mut McpServerForm) -> &mut Vec<EnvRow> {
        match self {
            Pairs::Env => &mut form.env,
            Pairs::Headers => &mut form.headers,
        }
    }
}

impl McpServerForm {
    /// What the form says, as the server keeps it. Rows without a name are left out, as Zed's
    /// form leaves them out.
    fn server(&self, cx: &App) -> McpServer {
        let text = |input: &Entity<TextInput>| input.read(cx).text().trim().to_string();
        let pairs = |rows: &[EnvRow]| {
            rows.iter()
                .map(|row| (text(&row.key), row.value.read(cx).text().to_string()))
                .filter(|(key, _)| !key.is_empty())
                .collect()
        };
        let transport = if self.remote {
            McpTransport::Remote {
                url: text(&self.url),
                headers: pairs(&self.headers),
            }
        } else {
            McpTransport::Local {
                command: text(&self.command),
                args: self
                    .args
                    .read(cx)
                    .text()
                    .split_whitespace()
                    .map(str::to_string)
                    .collect(),
                env: pairs(&self.env),
            }
        };
        McpServer {
            name: text(&self.name),
            enabled: self.enabled,
            transport,
            kept_off: self.kept_off.clone(),
        }
    }
}

impl SettingsPage {
    /// The page's title (a breadcrumb in the form) and the machine whose servers it shows.
    pub(super) fn render_mcp_servers_header(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let heading = match &self.mcp_servers_page {
            McpServersPage::Form(form) => self.render_sub_page_heading(
                "mcp-servers-back",
                "MCP Servers",
                match (&form.replacing, form.remote) {
                    (Some(_), _) => "Configure MCP Server",
                    (None, false) => "Add Local MCP Server",
                    (None, true) => "Add Remote MCP Server",
                },
                |this, _, cx| this.close_mcp_server_form(cx),
                cx,
            ),
            McpServersPage::List => Headline::new("MCP Servers")
                .size(HeadlineSize::Small)
                .into_any_element(),
        };
        let machine = if !self.machines.read(cx).has_remotes() {
            None
        } else if let McpServersPage::Form(_) = self.mcp_servers_page {
            // The form belongs to the machine it was opened on.
            Some(self.render_machine_label(cx))
        } else {
            Some(self.render_agents_machine_picker(window, cx))
        };
        h_flex()
            .h(px(28.))
            .gap_4()
            .justify_between()
            .child(heading)
            .children(machine)
            .into_any_element()
    }

    pub(super) fn render_mcp_servers(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        match &self.mcp_servers_page {
            McpServersPage::List => self.render_mcp_server_list(window, cx),
            McpServersPage::Form(form) => self.render_mcp_server_form(form, window, cx),
        }
    }

    fn render_mcp_server_list(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let servers = self.agents_client(cx).read(cx).mcp_servers().to_vec();
        let rows = if servers.is_empty() {
            vec![
                h_flex()
                    .debug_selector(|| "mcp-servers-empty".into())
                    .p_4()
                    .justify_center()
                    .child(
                        Label::new(
                            "No MCP servers added yet. Click \"Add Server\" to get started.",
                        )
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                    )
                    .into_any_element(),
            ]
        } else {
            let agents = self.mcp_agents(cx);
            servers
                .iter()
                .enumerate()
                .map(|(index, server)| {
                    self.render_mcp_server_row(index, server, &agents, window, cx)
                })
                .collect()
        };
        let list = render_section_with_actions(
            "Every agent and account gets these in agentZ threads.",
            rows,
            self.render_add_mcp_server_menu(cx),
            cx,
        );
        let error = self
            .mcp_server_error
            .clone()
            .map(|error| render_error(error, "mcp-server-error"));
        std::iter::once(list).chain(error).collect()
    }

    /// The installed agents, with what each said it takes when it last started.
    fn mcp_agents(&self, cx: &App) -> Vec<McpAgent> {
        let client = self.agents_client(cx);
        self.registry(cx)
            .read(cx)
            .agents()
            .iter()
            .filter(|agent| matches!(agent.install_state, InstallState::Installed { .. }))
            .map(|agent| McpAgent {
                id: agent.id().clone(),
                name: agent.name().to_string(),
                capabilities: client
                    .read(cx)
                    .agent_settings(&agent.id().0)
                    .mcp_capabilities,
            })
            .collect()
    }

    /// Zed's row: the name, Local or Remote, the command or URL, and the agents that don't get
    /// it; then configure, delete and its switch.
    fn render_mcp_server_row(
        &self,
        index: usize,
        server: &McpServer,
        agents: &[McpAgent],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let client = self.agents_client(cx);
        let groups = account_groups(
            self.registry(cx).read(cx).agents(),
            client.read(cx),
            |agent| {
                agents
                    .iter()
                    .find(|other| other.id == *agent.id())
                    .is_some_and(|agent| agent.gets(server))
            },
        );
        let page = cx.weak_entity();
        let accounts_menu = render_accounts_menu(
            KeptOffItem::McpServer(server.name.clone()),
            groups,
            client,
            Rc::new(move |request, cx| {
                page.update(cx, |page, cx| {
                    page.send_mcp_server_request(request, "Couldn't change the server", cx)
                })
                .log_err();
            }),
            window,
            cx,
        );
        let selector = format!("mcp-server-{}", server.name);
        let name = server.name.clone();
        let configured = server.clone();
        // agentZ's own server is only switched, as Zed lists an extension's without Configure.
        let editable = !server.is_agentz();
        h_flex()
            .debug_selector(move || selector)
            .px_4()
            .py_3()
            .gap_4()
            .justify_between()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(Label::new(server.name.clone()))
                            .child(account_tag(
                                if server.is_agentz() {
                                    "Built-in"
                                } else if server.is_remote() {
                                    "Remote"
                                } else {
                                    "Local"
                                },
                                Color::Muted,
                                cx,
                            )),
                    )
                    .child(
                        div().min_w_0().overflow_hidden().child(
                            Label::new(server.detail())
                                .size(LabelSize::Small)
                                .color(Color::Muted)
                                .when(editable, |label| label.buffer_font(cx))
                                .truncate(),
                        ),
                    )
                    .children(not_given_notes(server, agents).into_iter().map(|note| {
                        Label::new(note)
                            .size(LabelSize::XSmall)
                            .color(Color::Placeholder)
                    })),
            )
            .child(
                h_flex()
                    .flex_none()
                    .gap_1()
                    .children(accounts_menu)
                    .when(editable, |actions| {
                        actions
                            .child(
                                div()
                                    .debug_selector(move || format!("mcp-server-configure-{index}"))
                                    .child(
                                        IconButton::new(
                                            ("mcp-server-configure", index),
                                            IconName::Settings,
                                        )
                                        .icon_size(IconSize::Small)
                                        .tooltip(Tooltip::text("Configure MCP Server"))
                                        .on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                this.open_mcp_server_form(
                                                    configured.is_remote(),
                                                    Some(&configured),
                                                    window,
                                                    cx,
                                                )
                                            }),
                                        ),
                                    ),
                            )
                            .child(
                                div()
                                    .debug_selector(move || format!("mcp-server-delete-{index}"))
                                    .child({
                                        let name = name.clone();
                                        IconButton::new(
                                            ("mcp-server-delete", index),
                                            IconName::Trash,
                                        )
                                        .icon_size(IconSize::Small)
                                        .tooltip(Tooltip::text("Uninstall MCP Server"))
                                        .on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                this.send_mcp_server_request(
                                                    Request::DeleteMcpServer(name.clone()),
                                                    "Couldn't delete the server",
                                                    cx,
                                                )
                                            }),
                                        )
                                    }),
                            )
                    })
                    .child(
                        div()
                            .debug_selector(move || format!("mcp-server-switch-{index}"))
                            .child(
                                Switch::new(("mcp-server-switch", index), server.enabled.into())
                                    .on_click(cx.listener(move |this, state, _, cx| {
                                        this.send_mcp_server_request(
                                            Request::SetMcpServerEnabled {
                                                name: name.clone(),
                                                enabled: *state == ToggleState::Selected,
                                            },
                                            "Couldn't change the server",
                                            cx,
                                        )
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_add_mcp_server_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let page = cx.weak_entity();
        div()
            .debug_selector(|| "add-mcp-server".into())
            .child(
                PopoverMenu::new("add-mcp-server-menu")
                    .menu(move |window, cx| {
                        let page = page.clone();
                        Some(ContextMenu::build(window, cx, move |menu, _, _| {
                            menu.item(
                                ContextMenuEntry::new("Add Local Server")
                                    .icon(IconName::Terminal)
                                    .icon_color(Color::Muted)
                                    .handler(on_page(&page, |page, window, cx| {
                                        page.open_mcp_server_form(false, None, window, cx)
                                    })),
                            )
                            .item(
                                ContextMenuEntry::new("Add Remote Server")
                                    .icon(IconName::ToolWeb)
                                    .icon_color(Color::Muted)
                                    .handler(on_page(&page, |page, window, cx| {
                                        page.open_mcp_server_form(true, None, window, cx)
                                    })),
                            )
                        }))
                    })
                    .trigger(
                        Button::new("add-mcp-server", "Add Server")
                            .style(ButtonStyle::Subtle)
                            .label_size(LabelSize::Small)
                            .color(Color::Muted)
                            .start_icon(
                                Icon::new(IconName::Plus)
                                    .size(IconSize::XSmall)
                                    .color(Color::Muted),
                            )
                            .end_icon(
                                Icon::new(IconName::ChevronDown)
                                    .size(IconSize::XSmall)
                                    .color(Color::Muted),
                            ),
                    )
                    .anchor(gpui::Anchor::TopRight)
                    .offset(gpui::point(px(0.), px(4.))),
            )
            .into_any_element()
    }

    /// Sends a change to the machine the list is of, to say why if it fails.
    fn send_mcp_server_request(
        &mut self,
        request: Request,
        failure: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.mcp_server_error = None;
        let response = self.agents_client(cx).read(cx).request(request);
        cx.spawn(async move |this, cx| {
            let Err(error) = response.await else {
                return;
            };
            this.update(cx, |this, cx| {
                this.mcp_server_error = Some(format!("{failure}: {error:#}").into());
                cx.notify();
            })
            .log_err();
        })
        .detach();
        cx.notify();
    }

    pub(super) fn open_mcp_server_form(
        &mut self,
        remote: bool,
        server: Option<&McpServer>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (command, args, env, url, headers) = match server.map(|server| &server.transport) {
            Some(McpTransport::Local { command, args, env }) => (
                command.clone(),
                args.join(" "),
                env.clone(),
                String::new(),
                Vec::new(),
            ),
            Some(McpTransport::Remote { url, headers }) => (
                String::new(),
                String::new(),
                Vec::new(),
                url.clone(),
                headers.clone(),
            ),
            Some(McpTransport::Agentz) | None => Default::default(),
        };
        let rows = |kind: Pairs, pairs: Vec<(String, String)>, cx: &mut Context<Self>| {
            pairs
                .iter()
                .map(|(key, value)| pair_row(kind, key, value, cx))
                .collect::<Vec<_>>()
        };
        let name = new_text_input(
            "my-mcp-server",
            server.map_or("", |server| server.name.as_str()),
            cx,
        );
        window.focus(&name.focus_handle(cx), cx);
        self.mcp_servers_page = McpServersPage::Form(McpServerForm {
            remote,
            replacing: server.map(|server| server.name.clone()),
            enabled: server.is_none_or(|server| server.enabled),
            kept_off: server
                .map(|server| server.kept_off.clone())
                .unwrap_or_default(),
            name,
            command: new_text_input("/path/to/server", &command, cx),
            args: new_text_input("--flag value", &args, cx),
            env: rows(Pairs::Env, env, cx),
            url: new_text_input("https://example.com/mcp", &url, cx),
            headers: rows(Pairs::Headers, headers, cx),
            error: None,
            saving: None,
        });
        self.mcp_server_error = None;
        cx.notify();
    }

    pub(super) fn close_mcp_server_form(&mut self, cx: &mut Context<Self>) {
        self.mcp_servers_page = McpServersPage::List;
        cx.notify();
    }

    fn save_mcp_server_form(&mut self, cx: &mut Context<Self>) {
        let client: Entity<ServerClient> = self.agents_client(cx);
        let McpServersPage::Form(form) = &mut self.mcp_servers_page else {
            return;
        };
        if form.saving.is_some() {
            return;
        }
        let server = form.server(cx);
        let others = client
            .read(cx)
            .mcp_servers()
            .iter()
            .map(|other| other.name.as_str())
            .filter(|other| Some(*other) != form.replacing.as_deref());
        if let Err(error) = server.validate(others) {
            form.error = Some(error.into());
            cx.notify();
            return;
        }
        let request = client.read(cx).request(Request::SaveMcpServer {
            replacing: form.replacing.clone(),
            server,
        });
        form.error = None;
        form.saving = Some(cx.spawn(async move |this, cx| {
            let saved = request.await;
            this.update(cx, |this, cx| {
                let McpServersPage::Form(form) = &mut this.mcp_servers_page else {
                    return;
                };
                form.saving = None;
                match saved {
                    Ok(_) => this.mcp_servers_page = McpServersPage::List,
                    Err(error) => form.error = Some(format!("{error:#}").into()),
                }
                cx.notify();
            })
            .log_err();
        }));
        cx.notify();
    }

    /// Zed's form in its words: Server Name, then Command, Arguments and Environment Variables,
    /// or URL and Headers; then Cancel and Save. Zed's timeout and OAuth client ID are left
    /// out: ACP gives agents neither.
    fn render_mcp_server_form(
        &self,
        form: &McpServerForm,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let field = |input: &Entity<TextInput>, monospace: bool, cx: &App| {
            div()
                .w(px(320.))
                .child(
                    text_field(input, false, window, cx).when(monospace, |field| {
                        field.font_buffer(cx).text_size(rems_from_px(12_f32))
                    }),
                )
                .into_any_element()
        };
        let mut rows = vec![render_row(
            "Server Name",
            "Required. A unique name used to identify this MCP server.",
            field(&form.name, false, cx),
            cx,
        )];
        if form.remote {
            rows.push(render_row(
                "URL",
                "Required. The base URL of the remote MCP server.",
                field(&form.url, true, cx),
                cx,
            ));
        } else {
            rows.push(render_row(
                "Command",
                "Required. Path to the executable that launches the server.",
                field(&form.command, true, cx),
                cx,
            ));
            rows.push(render_row(
                "Arguments",
                "Space-separated arguments passed to the command.",
                field(&form.args, true, cx),
                cx,
            ));
        }
        let server = render_section("Server", rows, cx);
        let pairs = if form.remote {
            self.render_mcp_pairs(
                form,
                Pairs::Headers,
                "Headers",
                "HTTP headers sent with each request to the server.",
                window,
                cx,
            )
        } else {
            self.render_mcp_pairs(
                form,
                Pairs::Env,
                "Environment Variables",
                "Environment variables provided to the server process.",
                window,
                cx,
            )
        };
        let error = form
            .error
            .clone()
            .map(|error| render_error(error, "mcp-server-form-error"));
        let is_saving = form.saving.is_some();
        let actions = h_flex()
            .justify_end()
            .gap_2()
            .child(
                div().debug_selector(|| "mcp-server-cancel".into()).child(
                    ActionButton::new("mcp-server-cancel", "Cancel")
                        .style(ActionStyle::Ghost)
                        .on_click(cx.listener(|this, _, _, cx| this.close_mcp_server_form(cx))),
                ),
            )
            .child(
                div().debug_selector(|| "mcp-server-save".into()).child(
                    ActionButton::new(
                        "mcp-server-save",
                        if is_saving { "Saving…" } else { "Save" },
                    )
                    .style(ActionStyle::Primary)
                    .disabled(is_saving)
                    .on_click(cx.listener(|this, _, _, cx| this.save_mcp_server_form(cx))),
                ),
            )
            .into_any_element();
        [server, pairs]
            .into_iter()
            .chain(error)
            .chain([actions])
            .collect()
    }

    /// The environment variables or headers, a name and a value a row, as the custom agent
    /// form's.
    fn render_mcp_pairs(
        &self,
        form: &McpServerForm,
        kind: Pairs,
        title: &'static str,
        description: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (rows, add_id, remove_id) = match kind {
            Pairs::Env => (&form.env, "mcp-server-add-env", "mcp-server-remove-env"),
            Pairs::Headers => (
                &form.headers,
                "mcp-server-add-header",
                "mcp-server-remove-header",
            ),
        };
        let field = |input: &Entity<TextInput>, cx: &App| {
            text_field(input, false, window, cx)
                .font_buffer(cx)
                .text_size(rems_from_px(12_f32))
        };
        let mut items: Vec<AnyElement> = rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                h_flex()
                    .px_4()
                    .py_2()
                    .gap_2()
                    .child(div().w(px(180.)).child(field(&row.key, cx)))
                    .child(Label::new("=").color(Color::Muted))
                    .child(div().flex_1().min_w_0().child(field(&row.value, cx)))
                    .child(
                        IconButton::new((remove_id, index), IconName::Close)
                            .icon_size(IconSize::Small)
                            .tooltip(Tooltip::text("Remove"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let McpServersPage::Form(form) = &mut this.mcp_servers_page {
                                    let rows = kind.rows(form);
                                    if index < rows.len() {
                                        rows.remove(index);
                                    }
                                }
                                cx.notify();
                            })),
                    )
                    .into_any_element()
            })
            .collect();
        if items.is_empty() {
            items.push(
                div()
                    .px_4()
                    .py_3()
                    .child(Label::new("None.").color(Color::Muted))
                    .into_any_element(),
            );
        }
        let add = div()
            .debug_selector(move || add_id.into())
            .child(
                Button::new(add_id, "Add")
                    .style(ButtonStyle::Subtle)
                    .label_size(LabelSize::Small)
                    .color(Color::Muted)
                    .start_icon(
                        Icon::new(IconName::Plus)
                            .size(IconSize::XSmall)
                            .color(Color::Muted),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let row = pair_row(kind, "", "", cx);
                        // So the new name can be typed straight away.
                        window.focus(&row.key.focus_handle(cx), cx);
                        if let McpServersPage::Form(form) = &mut this.mcp_servers_page {
                            kind.rows(form).push(row);
                        }
                        cx.notify();
                    })),
            )
            .into_any_element();
        v_flex()
            .gap_2()
            .child(render_section_with_actions(title, items, add, cx))
            .child(
                Label::new(description)
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            )
            .into_any_element()
    }
}

/// An installed agent, for the agents a server doesn't reach.
struct McpAgent {
    id: AgentId,
    name: String,
    /// `None` until it has started.
    capabilities: Option<acp::McpCapabilities>,
}

impl McpAgent {
    fn ignores_servers(&self) -> bool {
        IGNORES_MCP_SERVERS.contains(&self.id.0.as_ref())
    }

    /// Whether it's given the server: it doesn't ignore ACP's servers, and for a remote one,
    /// didn't say it takes local ones only.
    fn gets(&self, server: &McpServer) -> bool {
        !self.ignores_servers() && !(server.is_remote() && self.takes_local_only())
    }

    fn takes_local_only(&self) -> bool {
        self.capabilities
            .as_ref()
            .is_some_and(|capabilities| !capabilities.http)
    }
}

/// Which agents don't get the server, and why: those that ignore ACP's MCP servers, and for a
/// remote one, those that said they only take local ones. Agents that haven't started yet
/// aren't named.
fn not_given_notes(server: &McpServer, agents: &[McpAgent]) -> Vec<String> {
    let names = |which: &dyn Fn(&McpAgent) -> bool| -> Vec<String> {
        agents
            .iter()
            .filter(|agent| which(agent))
            .map(|agent| agent.name.clone())
            .collect()
    };
    let mut notes = Vec::new();
    if server.is_remote() {
        let local_only = names(&|agent| !agent.ignores_servers() && agent.takes_local_only());
        if !local_only.is_empty() {
            let verb = if local_only.len() == 1 {
                "takes"
            } else {
                "take"
            };
            notes.push(format!(
                "Not given to {}, which {verb} local servers only.",
                join_with_and(local_only)
            ));
        }
    }
    let ignoring = names(&McpAgent::ignores_servers);
    if !ignoring.is_empty() {
        let verb = if ignoring.len() == 1 {
            "ignores"
        } else {
            "ignore"
        };
        notes.push(format!(
            "Not given to {}, which {verb} the MCP servers agentZ gives agents.",
            join_with_and(ignoring)
        ));
    }
    notes
}

/// A row of the environment variables or headers.
fn pair_row(kind: Pairs, key: &str, value: &str, cx: &mut App) -> EnvRow {
    let row = new_variable_row(key, value, cx);
    if let Pairs::Headers = kind {
        row.key
            .update(cx, |input, cx| input.set_placeholder("Name", cx));
    }
    row
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use agentz_protocol::Response;
    use agentz_protocol::agents::{AgentSettings, RegistrySnapshot};
    use agentz_protocol::spaces::SpacesSnapshot;
    use gpui::TestAppContext;

    use super::super::Section;
    use super::super::tests::listing;
    use super::*;
    use crate::machines::MachineId;

    fn installed(id: &str, name: &str) -> agentz_protocol::agents::AgentListing {
        listing(
            id,
            name,
            InstallState::Installed {
                version: "1.0.0".into(),
                update_available: false,
            },
        )
    }

    fn github() -> McpServer {
        McpServer {
            name: "github".into(),
            enabled: true,
            transport: McpTransport::Local {
                command: "npx".into(),
                args: vec!["-y".into(), "@modelcontextprotocol/server-github".into()],
                env: vec![("GITHUB_TOKEN".into(), "secret".into())],
            },
            kept_off: Vec::new(),
        }
    }

    fn linear() -> McpServer {
        McpServer {
            name: "linear".into(),
            enabled: true,
            transport: McpTransport::Remote {
                url: "https://mcp.linear.app/mcp".into(),
                headers: Vec::new(),
            },
            kept_off: Vec::new(),
        }
    }

    #[test]
    fn names_the_agents_that_dont_get_a_server() {
        let agent = |id: &str, name: &str, http: Option<bool>| McpAgent {
            id: AgentId::new(id),
            name: name.into(),
            capabilities: http.map(|http| {
                let mut capabilities = acp::McpCapabilities::default();
                capabilities.http = http;
                capabilities
            }),
        };
        let agents = [
            agent("factory-droid", "Factory Droid", Some(false)),
            agent("claude-acp", "Claude Agent", Some(true)),
            agent("grok-build", "Grok Build", None),
        ];
        assert!(not_given_notes(&github(), &agents).is_empty());
        assert_eq!(
            not_given_notes(&linear(), &agents),
            ["Not given to Factory Droid, which takes local servers only."]
        );
        let agents = [
            agent("factory-droid", "Factory Droid", Some(false)),
            agent("cursor", "Cursor", Some(false)),
            agent("cline", "Cline", Some(true)),
        ];
        assert_eq!(
            not_given_notes(&linear(), &agents),
            [
                "Not given to Factory Droid and Cursor, which take local servers only.",
                "Not given to Cline, which ignores the MCP servers agentZ gives agents.",
            ]
        );
    }

    #[gpui::test]
    fn servers_are_added_configured_switched_and_deleted(cx: &mut TestAppContext) {
        let requests: Rc<RefCell<Vec<Request>>> = Rc::default();
        let client = cx.update(|cx| {
            crate::init_for_test(cx);
            super::super::init(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            let registry = client.read(cx).registry().clone();
            registry.update(cx, |registry, cx| {
                registry.set_snapshot(
                    RegistrySnapshot {
                        agents: vec![installed("factory-droid", "Factory Droid")],
                        is_fetching: false,
                        fetch_error: None,
                    },
                    cx,
                )
            });
            let requests = requests.clone();
            client.update(cx, |client, cx| {
                client.answer_for_test(move |request| {
                    requests.borrow_mut().push(request.clone());
                    match request {
                        Request::SaveMcpServer { .. }
                        | Request::DeleteMcpServer(_)
                        | Request::SetMcpServerEnabled { .. } => Some(Response::Ok),
                        _ => None,
                    }
                });
                client.set_agent_settings_for_test(
                    [(
                        AgentId::new("factory-droid"),
                        AgentSettings {
                            mcp_capabilities: Some(acp::McpCapabilities::default()),
                            ..AgentSettings::default()
                        },
                    )]
                    .into(),
                    cx,
                );
            });
            crate::machines::init_for_test(vec![client.clone()], cx);
            client
        });
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        page.update_in(cx, |page, window, cx| {
            page.select(Section::McpServers, window, cx)
        });
        cx.run_until_parked();
        let click = |selector: &'static str, cx: &mut gpui::VisualTestContext| {
            let bounds = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("{selector} is shown"));
            cx.simulate_click(bounds.center(), gpui::Modifiers::none());
            cx.run_until_parked();
        };
        let sent = |request: &Request| requests.borrow().contains(request);
        let form = |cx: &mut gpui::VisualTestContext| {
            page.read_with(cx, |page, cx| match &page.mcp_servers_page {
                McpServersPage::Form(form) => Some((form.server(cx), form.error.clone())),
                McpServersPage::List => None,
            })
        };
        assert!(cx.debug_bounds("mcp-servers-empty").is_some());

        // Add Local Server, with Zed's checks before anything is sent.
        page.update_in(cx, |page, window, cx| {
            page.open_mcp_server_form(false, None, window, cx)
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("mcp-servers-back").is_some());
        click("mcp-server-save", cx);
        assert_eq!(
            form(cx).and_then(|(_, error)| error).as_deref(),
            Some("Server name is required.")
        );
        page.update_in(cx, |page, window, cx| {
            if let McpServersPage::Form(form) = &page.mcp_servers_page {
                window.focus(&form.name.focus_handle(cx), cx);
            }
        });
        cx.simulate_input("github");
        page.update(cx, |page, cx| {
            if let McpServersPage::Form(form) = &page.mcp_servers_page {
                form.command
                    .update(cx, |input, cx| input.set_text("npx", cx));
                form.args.update(cx, |input, cx| {
                    input.set_text("-y  @modelcontextprotocol/server-github", cx)
                });
            }
        });
        click("mcp-server-add-env", cx);
        cx.simulate_input("GITHUB_TOKEN");
        page.update(cx, |page, cx| {
            if let McpServersPage::Form(form) = &page.mcp_servers_page {
                form.env[0]
                    .value
                    .update(cx, |input, cx| input.set_text("secret", cx));
            }
        });
        click("mcp-server-save", cx);
        assert!(sent(&Request::SaveMcpServer {
            replacing: None,
            server: github(),
        }));
        assert!(form(cx).is_none());

        // Listed, a remote one names the agents that only take local ones.
        client.update(cx, |client, cx| {
            client.set_mcp_servers_for_test(vec![github(), linear()], cx)
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("mcp-servers-empty").is_none());
        let first = cx
            .debug_bounds("mcp-server-github")
            .expect("github is listed");
        let second = cx
            .debug_bounds("mcp-server-linear")
            .expect("linear is listed");
        assert!(first.top() < second.top());

        // Configure opens it as it is, and a name another server has is refused.
        click("mcp-server-configure-1", cx);
        assert_eq!(form(cx).map(|(server, _)| server), Some(linear()));
        page.update(cx, |page, cx| {
            if let McpServersPage::Form(form) = &page.mcp_servers_page {
                form.name
                    .update(cx, |input, cx| input.set_text("github", cx));
            }
        });
        click("mcp-server-save", cx);
        assert_eq!(
            form(cx).and_then(|(_, error)| error).as_deref(),
            Some("A server named \"github\" already exists.")
        );
        page.update(cx, |page, cx| {
            if let McpServersPage::Form(form) = &page.mcp_servers_page {
                form.name
                    .update(cx, |input, cx| input.set_text("linear", cx));
            }
        });
        click("mcp-server-add-header", cx);
        cx.simulate_input("Authorization");
        click("mcp-server-save", cx);
        assert!(sent(&Request::SaveMcpServer {
            replacing: Some("linear".into()),
            server: McpServer {
                transport: McpTransport::Remote {
                    url: "https://mcp.linear.app/mcp".into(),
                    headers: vec![("Authorization".into(), String::new())],
                },
                ..linear()
            },
        }));

        click("mcp-server-switch-0", cx);
        assert!(sent(&Request::SetMcpServerEnabled {
            name: "github".into(),
            enabled: false,
        }));
        click("mcp-server-delete-1", cx);
        assert!(sent(&Request::DeleteMcpServer("linear".into())));
    }

    #[gpui::test]
    fn agentz_is_only_switched(cx: &mut TestAppContext) {
        let requests: Rc<RefCell<Vec<Request>>> = Rc::default();
        let client = cx.update(|cx| {
            crate::init_for_test(cx);
            super::super::init(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            let requests = requests.clone();
            client.update(cx, |client, cx| {
                client.answer_for_test(move |request| {
                    requests.borrow_mut().push(request.clone());
                    Some(Response::Ok)
                });
                client.set_mcp_servers_for_test(vec![McpServer::agentz(), github()], cx);
            });
            crate::machines::init_for_test(vec![client.clone()], cx);
            client
        });
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        page.update_in(cx, |page, window, cx| {
            page.select(Section::McpServers, window, cx)
        });
        cx.run_until_parked();
        let agentz = cx
            .debug_bounds("mcp-server-agentz")
            .expect("agentz is listed");
        let github = cx
            .debug_bounds("mcp-server-github")
            .expect("github is listed");
        assert!(agentz.top() < github.top());
        assert!(cx.debug_bounds("mcp-server-configure-0").is_none());
        assert!(cx.debug_bounds("mcp-server-delete-0").is_none());
        assert!(cx.debug_bounds("mcp-server-configure-1").is_some());

        let switch = cx
            .debug_bounds("mcp-server-switch-0")
            .expect("agentz has a switch");
        cx.simulate_click(switch.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        assert!(requests.borrow().contains(&Request::SetMcpServerEnabled {
            name: "agentz".into(),
            enabled: false,
        }));
        drop(client);
    }

    #[gpui::test]
    fn servers_are_kept_off_accounts_from_their_menu(cx: &mut TestAppContext) {
        let requests: Rc<RefCell<Vec<Request>>> = Rc::default();
        let droid = AgentId::new("factory-droid");
        let claude = AgentId::new("claude-acp");
        let client = cx.update(|cx| {
            crate::init_for_test(cx);
            super::super::init(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            let registry = client.read(cx).registry().clone();
            registry.update(cx, |registry, cx| {
                registry.set_snapshot(
                    RegistrySnapshot {
                        agents: vec![
                            installed("factory-droid", "Factory Droid"),
                            installed("claude-acp", "Claude Agent"),
                        ],
                        is_fetching: false,
                        fetch_error: None,
                    },
                    cx,
                )
            });
            let requests = requests.clone();
            client.update(cx, |client, cx| {
                client.answer_for_test(move |request| {
                    requests.borrow_mut().push(request.clone());
                    Some(Response::Ok)
                });
                // Droid takes local servers only, and has an agentZ account beside its own.
                client.set_agent_settings_for_test(
                    [(
                        droid.clone(),
                        AgentSettings {
                            mcp_capabilities: Some(acp::McpCapabilities::default()),
                            ..AgentSettings::default()
                        },
                    )]
                    .into(),
                    cx,
                );
                client.set_accounts_for_test(
                    [(
                        droid.clone(),
                        agentz_protocol::accounts::AgentAccounts {
                            accounts: vec![super::super::tests::account(1, Some("Work"), None)],
                            last_id: 1,
                            ..Default::default()
                        },
                    )]
                    .into(),
                    cx,
                );
                client.set_mcp_servers_for_test(vec![github(), linear()], cx);
            });
            crate::machines::init_for_test(vec![client.clone()], cx);
            client
        });
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        page.update_in(cx, |page, window, cx| {
            page.select(Section::McpServers, window, cx)
        });
        cx.run_until_parked();
        let click = |selector: &'static str, cx: &mut gpui::VisualTestContext| {
            let bounds = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("{selector} is shown"));
            cx.simulate_click(bounds.center(), gpui::Modifiers::none());
            cx.run_until_parked();
        };
        let kept_off = |accounts: &[(&AgentId, Option<u64>)]| -> Vec<AgentAccount> {
            accounts
                .iter()
                .map(|(agent_id, account)| AgentAccount {
                    agent_id: (*agent_id).clone(),
                    account: account.map(agentz_protocol::accounts::AccountId),
                })
                .collect()
        };
        let sent = |kept_off: Vec<AgentAccount>| {
            requests.borrow().contains(&Request::SetMcpServerKeptOff {
                name: "github".into(),
                kept_off,
            })
        };

        // Linear reaches only Claude's one account, so there's nothing to choose.
        assert!(cx.debug_bounds("mcp-server-accounts-linear").is_none());

        // Github's menu has Droid's two accounts and Claude's, and stays open as they're
        // unchecked and checked again.
        click("mcp-server-accounts-github", cx);
        let work = "mcp-server-accounts-github-factory-droid-1";
        assert!(
            cx.debug_bounds("mcp-server-accounts-github-claude-acp-external")
                .is_some()
        );
        click(work, cx);
        assert!(sent(kept_off(&[(&droid, Some(1))])));
        assert_eq!(
            client.read_with(cx, |client, _| client.mcp_servers()[0].kept_off.clone()),
            kept_off(&[(&droid, Some(1))])
        );
        click("mcp-server-accounts-github-claude-acp-external", cx);
        assert!(sent(kept_off(&[(&claude, None), (&droid, Some(1))])));
        click(work, cx);
        assert!(sent(kept_off(&[(&claude, None)])));
    }
}
