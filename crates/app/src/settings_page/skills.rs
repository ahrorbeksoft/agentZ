//! Settings › Skills: Zed's Skills page for agentZ's own skills on a machine (design/accounts
//! decisions.md §17, §18), which every agent and account loads. Add Skill offers Add from
//! Folder… and Zed's Create a Skill.

use std::path::{Component, Path, PathBuf};

use agentz_protocol::Request;
use agentz_protocol::accounts::{AccountId, AgentAccounts};
use agentz_protocol::agents::AgentId;
use agentz_protocol::skills::{
    FOLDER_TOO_LARGE, MAX_FOLDER_SIZE, Skill, SkillFile, validate_description, validate_name,
};
use anyhow::{Context as _, Result};
use base64::Engine as _;
use gpui::{
    AnyElement, App, Context, Entity, Focusable, PathPromptOptions, Subscription, Task, Window,
};
use text_input::{TextInput, TextInputEvent};
use ui::{ContextMenu, ContextMenuEntry, Divider, PopoverMenu, Tooltip, prelude::*};
use util::ResultExt as _;

use super::{
    SettingsPage, SettingsPageEvent, account_entry, new_text_input, on_page,
    render_section_with_actions,
};
use crate::confirm_dialog::ConfirmRequest;
use crate::controls::{ActionButton, ActionStyle, field_frame, field_label, text_field};
use crate::machines::MachineId;
use crate::project_switcher::compact_path;
use crate::server_client::ServerClient;

/// The key context of Create a Skill's body, where Enter starts a new line (bound in
/// [`super::init`]).
const SKILL_BODY_KEY_CONTEXT: &str = "SkillBody";
const BODY_MAX_LINES: usize = 24;

/// What Settings › Skills shows.
pub(super) enum SkillsPage {
    List,
    /// Zed's Create a Skill form.
    Create(SkillForm),
}

pub(super) struct SkillForm {
    name: Entity<TextInput>,
    description: Entity<TextInput>,
    body: Entity<TextInput>,
    name_error: Option<&'static str>,
    description_error: Option<&'static str>,
    body_error: Option<&'static str>,
    save_error: Option<SharedString>,
    saving: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

#[derive(Clone, Copy)]
enum SkillField {
    Name,
    Description,
    Body,
}

impl SkillForm {
    /// Zed's checks of a field, as it's edited and on save.
    fn check(&mut self, field: SkillField, cx: &App) {
        match field {
            SkillField::Name => self.name_error = validate_name(self.name.read(cx).text()).err(),
            SkillField::Description => {
                self.description_error =
                    validate_description(self.description.read(cx).text()).err()
            }
            SkillField::Body => {
                self.body_error = self
                    .body
                    .read(cx)
                    .text()
                    .trim()
                    .is_empty()
                    .then_some("Body is required.")
            }
        }
        self.save_error = None;
    }
}

impl SettingsPage {
    /// The page's title (a breadcrumb in Create a Skill) and the machine whose skills it shows.
    pub(super) fn render_skills_header(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let is_creating = matches!(self.skills_page, SkillsPage::Create(_));
        let heading = if is_creating {
            self.render_sub_page_heading(
                "skills-back",
                "Skills",
                "Create Skill",
                |this, _, cx| this.close_skill_form(cx),
                cx,
            )
        } else {
            Headline::new("Skills")
                .size(HeadlineSize::Small)
                .into_any_element()
        };
        let machine = if !self.machines.read(cx).has_remotes() {
            None
        } else if is_creating {
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

    pub(super) fn render_skills(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        match &self.skills_page {
            SkillsPage::List => self.render_skill_list(cx),
            SkillsPage::Create(form) => self.render_skill_form(form, window, cx),
        }
    }

    fn render_skill_list(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let skills = self.agents_client(cx).read(cx).skills().to_vec();
        let rows = if skills.is_empty() {
            vec![
                v_flex()
                    .px_4()
                    .py_6()
                    .items_center()
                    .gap_2()
                    .child(Label::new("No global skills installed.").color(Color::Muted))
                    .child(
                        div().debug_selector(|| "skill-create-empty".into()).child(
                            Button::new("skill-create-empty", "Create a Skill")
                                .style(ButtonStyle::Outlined)
                                .start_icon(
                                    Icon::new(IconName::Plus)
                                        .size(IconSize::Small)
                                        .color(Color::Muted),
                                )
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_skill_form(window, cx)
                                })),
                        ),
                    )
                    .into_any_element(),
            ]
        } else {
            skills
                .iter()
                .enumerate()
                .map(|(index, skill)| self.render_skill_row(index, skill, cx))
                .collect()
        };
        let list = render_section_with_actions(
            "Every agent and account loads these in agentZ threads.",
            rows,
            self.render_add_skill_menu(cx),
            cx,
        );
        let error = self
            .skill_error
            .clone()
            .map(|error| render_error(error, "skill-error"));
        std::iter::once(list).chain(error).collect()
    }

    /// Zed's row: the name, with a warning when an agent skips the skill and why, the
    /// description, then delete and Open ↗.
    fn render_skill_row(&self, index: usize, skill: &Skill, cx: &mut Context<Self>) -> AnyElement {
        let reasons = self.skip_reasons(skill, cx);
        let is_local = self.agents_machine == MachineId::Local;
        let machine = self.machines.read(cx).label(self.agents_machine, cx);
        let skill_path = skill.path.clone();
        let name = skill.name.clone();
        let selector = format!("skill-{}", skill.name);
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
                    .child(h_flex().gap_1().child(Label::new(skill.name.clone())).when(
                        !reasons.is_empty(),
                        |title| {
                            title.child(
                                Icon::new(IconName::Warning)
                                    .size(IconSize::XSmall)
                                    .color(Color::Warning),
                            )
                        },
                    ))
                    .child(
                        Label::new(skill.description.clone())
                            .size(LabelSize::Small)
                            .color(Color::Muted)
                            .line_clamp(5),
                    )
                    .children(reasons.into_iter().map(|reason| {
                        Label::new(reason)
                            .size(LabelSize::XSmall)
                            .color(Color::Warning)
                    })),
            )
            .child(
                h_flex()
                    .flex_none()
                    .gap_2()
                    .child(
                        div()
                            .debug_selector(move || format!("skill-delete-{index}"))
                            .child(
                                IconButton::new(("skill-delete", index), IconName::Trash)
                                    .icon_size(IconSize::Small)
                                    .tooltip(Tooltip::text("Delete Skill"))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.confirm_delete_skill(index, cx)
                                    })),
                            ),
                    )
                    .child(
                        Button::new(("skill-open", index), "Open")
                            .style(ButtonStyle::OutlinedGhost)
                            .size(ButtonSize::Medium)
                            .end_icon(
                                Icon::new(IconName::ArrowUpRight)
                                    .size(IconSize::Small)
                                    .color(Color::Muted),
                            )
                            // Its SKILL.md is on the server's machine.
                            .disabled(!is_local)
                            .when(!is_local, |button| {
                                button.tooltip(Tooltip::text(format!(
                                    "{name}'s SKILL.md is on {machine}."
                                )))
                            })
                            .on_click(move |_, _, cx| cx.open_with_system(&skill_path)),
                    ),
            )
            .into_any_element()
    }

    /// Why accounts skip the skill, a line for each agent.
    fn skip_reasons(&self, skill: &Skill, cx: &App) -> Vec<String> {
        let client = self.agents_client(cx);
        let registry = self.registry(cx);
        let mut agents: Vec<&AgentId> = Vec::new();
        for skipped in &skill.skipped {
            if !agents.contains(&&skipped.agent_id) {
                agents.push(&skipped.agent_id);
            }
        }
        agents
            .into_iter()
            .map(|agent_id| {
                let agent_name = registry
                    .read(cx)
                    .agent(agent_id)
                    .map(|agent| agent.name().to_string())
                    .unwrap_or_else(|| agent_id.0.to_string());
                let skipping: Vec<Option<AccountId>> = skill
                    .skipped
                    .iter()
                    .filter(|skipped| skipped.agent_id == *agent_id)
                    .map(|skipped| skipped.account)
                    .collect();
                skip_reason(
                    &agent_name,
                    &skill.name,
                    &client.read(cx).accounts(agent_id),
                    &skipping,
                )
            })
            .collect()
    }

    fn render_add_skill_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let page = cx.weak_entity();
        div()
            .debug_selector(|| "add-skill".into())
            .child(
                PopoverMenu::new("add-skill-menu")
                    .menu(move |window, cx| {
                        let page = page.clone();
                        Some(ContextMenu::build(window, cx, move |menu, _, _| {
                            menu.item(
                                ContextMenuEntry::new("Add from Folder…")
                                    .icon(IconName::Folder)
                                    .icon_color(Color::Muted)
                                    .handler(on_page(&page, |page, _, cx| {
                                        page.add_skill_from_folder(cx)
                                    })),
                            )
                            .item(
                                ContextMenuEntry::new("Create a Skill")
                                    .icon(IconName::Plus)
                                    .icon_color(Color::Muted)
                                    .handler(on_page(&page, |page, window, cx| {
                                        page.open_skill_form(window, cx)
                                    })),
                            )
                        }))
                    })
                    .trigger(
                        Button::new("add-skill", "Add Skill")
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
                            )
                            .disabled(self.adding_skill.is_some()),
                    )
                    .anchor(gpui::Anchor::TopRight)
                    .offset(gpui::point(px(0.), px(4.))),
            )
            .into_any_element()
    }

    /// Add from Folder…: a folder on this Mac with a `SKILL.md`, copied to the machine.
    fn add_skill_from_folder(&mut self, cx: &mut Context<Self>) {
        if self.adding_skill.is_some() {
            return;
        }
        let client = self.agents_client(cx);
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Add Skill".into()),
        });
        self.skill_error = None;
        self.adding_skill = Some(cx.spawn(async move |this, cx| {
            let folder = match paths.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                Ok(Ok(None)) | Err(_) => None,
                Ok(Err(error)) => {
                    log::error!("failed to pick a skill's folder: {error:#}");
                    None
                }
            };
            let added = match folder {
                Some(folder) => {
                    let files = cx
                        .background_spawn(async move { read_skill_folder(&folder) })
                        .await;
                    match files {
                        Ok(files) => {
                            let request = client.read_with(cx, |client, _| {
                                client.request(Request::AddSkill(files))
                            });
                            request.await.map(|_| ())
                        }
                        Err(error) => Err(error),
                    }
                }
                None => Ok(()),
            };
            this.update(cx, |this, cx| {
                this.adding_skill = None;
                if let Err(error) = added {
                    this.skill_error = Some(format!("Couldn't add the skill: {error:#}").into());
                }
                cx.notify();
            })
            .log_err();
        }));
        cx.notify();
    }

    fn confirm_delete_skill(&mut self, index: usize, cx: &mut Context<Self>) {
        let client = self.agents_client(cx);
        let Some(skill) = client.read(cx).skills().get(index).cloned() else {
            return;
        };
        let folder = skill.path.parent().unwrap_or(&skill.path);
        let folder = match self.agents_machine {
            MachineId::Local => compact_path(folder),
            MachineId::Remote(_) => folder.display().to_string(),
        };
        let page = cx.weak_entity();
        let name = skill.name.clone();
        let request = ConfirmRequest::delete_skill(&skill.name, &folder, move |_, cx| {
            let name = name.clone();
            page.update(cx, |page, cx| page.delete_skill(&client, name, cx))
                .log_err();
        });
        cx.emit(SettingsPageEvent::Confirm(request));
    }

    /// Deletes the skill on the machine it was listed on, to say why if that fails.
    fn delete_skill(
        &mut self,
        client: &Entity<ServerClient>,
        name: String,
        cx: &mut Context<Self>,
    ) {
        self.skill_error = None;
        let response = client.read(cx).request(Request::DeleteSkill(name));
        cx.spawn(async move |this, cx| {
            let Err(error) = response.await else {
                return;
            };
            this.update(cx, |this, cx| {
                this.skill_error = Some(format!("Couldn't delete the skill: {error:#}").into());
                cx.notify();
            })
            .log_err();
        })
        .detach();
        cx.notify();
    }

    pub(super) fn open_skill_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = new_text_input("my-new-skill", "", cx);
        let description = new_text_input(
            "e.g., Fill the PR description following this template.",
            "",
            cx,
        );
        let body = cx.new(|cx| TextInput::new("Add skill content…", cx).multi_line(BODY_MAX_LINES));
        let subscriptions = [
            (&name, SkillField::Name),
            (&description, SkillField::Description),
            (&body, SkillField::Body),
        ]
        .into_iter()
        .map(|(input, field)| {
            cx.subscribe(input, move |this, _, event: &TextInputEvent, cx| {
                if let TextInputEvent::Changed = event
                    && let SkillsPage::Create(form) = &mut this.skills_page
                {
                    form.check(field, cx);
                    cx.notify();
                }
            })
        })
        .collect();
        window.focus(&name.focus_handle(cx), cx);
        self.skills_page = SkillsPage::Create(SkillForm {
            name,
            description,
            body,
            name_error: None,
            description_error: None,
            body_error: None,
            save_error: None,
            saving: None,
            _subscriptions: subscriptions,
        });
        self.skill_error = None;
        cx.notify();
    }

    pub(super) fn close_skill_form(&mut self, cx: &mut Context<Self>) {
        self.skills_page = SkillsPage::List;
        cx.notify();
    }

    fn save_skill_form(&mut self, cx: &mut Context<Self>) {
        let client = self.agents_client(cx);
        let SkillsPage::Create(form) = &mut self.skills_page else {
            return;
        };
        for field in [SkillField::Name, SkillField::Description, SkillField::Body] {
            form.check(field, cx);
        }
        let is_valid = form.name_error.is_none()
            && form.description_error.is_none()
            && form.body_error.is_none();
        if !is_valid || form.saving.is_some() {
            cx.notify();
            return;
        }
        let request = client.read(cx).request(Request::CreateSkill {
            name: form.name.read(cx).text().to_string(),
            description: form.description.read(cx).text().to_string(),
            body: form.body.read(cx).text().to_string(),
        });
        form.saving = Some(cx.spawn(async move |this, cx| {
            let saved = request.await;
            this.update(cx, |this, cx| {
                let SkillsPage::Create(form) = &mut this.skills_page else {
                    return;
                };
                form.saving = None;
                match saved {
                    Ok(_) => this.skills_page = SkillsPage::List,
                    Err(error) => form.save_error = Some(format!("{error:#}").into()),
                }
                cx.notify();
            })
            .log_err();
        }));
        cx.notify();
    }

    /// Zed's Create a Skill: the front-matter's name and description, the skill's content, and
    /// Save Skill.
    fn render_skill_form(
        &self,
        form: &SkillForm,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let field = |label: &'static str,
                     input: &Entity<TextInput>,
                     error: Option<&'static str>,
                     cx: &App| {
            v_flex()
                .gap_1()
                .child(field_label(label))
                .child(text_field(input, error.is_some(), window, cx))
                .children(
                    error.map(|error| Label::new(error).size(LabelSize::Small).color(Color::Error)),
                )
        };
        let front_matter = v_flex()
            .gap_2()
            .child(Label::new("Front-matter"))
            .child(field("Name", &form.name, form.name_error, cx))
            .child(field(
                "Description",
                &form.description,
                form.description_error,
                cx,
            ))
            .into_any_element();
        let body = form.body.clone();
        let is_body_focused = body.read(cx).focus_handle(cx).is_focused(window);
        let content = v_flex()
            .gap_2()
            .child(Label::new("Skill Content"))
            .child(
                div()
                    .key_context(SKILL_BODY_KEY_CONTEXT)
                    .debug_selector(|| "skill-body".into())
                    .child(
                        field_frame(is_body_focused, form.body_error.is_some(), cx)
                            .id("skill-body")
                            .h_auto()
                            .min_h(px(160.))
                            .py_1p5()
                            .items_start()
                            .cursor_text()
                            .on_click(cx.listener(|this, _, window, cx| {
                                if let SkillsPage::Create(form) = &this.skills_page {
                                    window.focus(&form.body.focus_handle(cx), cx);
                                }
                            }))
                            .child(div().flex_1().min_w_0().child(body)),
                    ),
            )
            .children(
                form.body_error
                    .map(|error| Label::new(error).size(LabelSize::Small).color(Color::Error)),
            )
            .into_any_element();
        let is_saving = form.saving.is_some();
        let footer = h_flex()
            .justify_end()
            .child(
                div().debug_selector(|| "skill-save".into()).child(
                    ActionButton::new(
                        "skill-save",
                        if is_saving { "Saving…" } else { "Save Skill" },
                    )
                    .style(ActionStyle::Primary)
                    .disabled(is_saving)
                    .on_click(cx.listener(|this, _, _, cx| this.save_skill_form(cx))),
                ),
            )
            .into_any_element();
        let error = form
            .save_error
            .clone()
            .map(|error| render_error(error, "skill-save-error"));
        [
            front_matter,
            Divider::horizontal().into_any_element(),
            content,
        ]
        .into_iter()
        .chain(error)
        .chain([footer])
        .collect()
    }
}

/// "Claude Agent has its own skill named frontend-design, so it keeps that one.", naming the
/// accounts when only some of the agent's do.
fn skip_reason(
    agent_name: &str,
    skill_name: &str,
    accounts: &AgentAccounts,
    skipping: &[Option<AccountId>],
) -> String {
    let listed = accounts.listed();
    let names: Vec<String> = listed
        .iter()
        .filter(|account| skipping.contains(account))
        .map(|account| account_entry(accounts, *account).name.to_string())
        .collect();
    let who = if names.is_empty() || names.len() == listed.len() {
        agent_name.to_string()
    } else {
        format!("{agent_name} on {}", join_with_and(names))
    };
    format!("{who} has its own skill named {skill_name}, so it keeps that one.")
}

pub(super) fn join_with_and(mut names: Vec<String>) -> String {
    match names.pop() {
        None => String::new(),
        Some(last) if names.is_empty() => last,
        Some(last) => format!("{} and {last}", names.join(", ")),
    }
}

pub(super) fn render_error(error: SharedString, selector: &'static str) -> AnyElement {
    h_flex()
        .debug_selector(move || selector.into())
        .gap_2()
        .items_start()
        .child(
            Icon::new(IconName::XCircle)
                .size(IconSize::Small)
                .color(Color::Error),
        )
        .child(Label::new(error).size(LabelSize::Small).color(Color::Error))
        .into_any_element()
}

/// The files of a skill's folder, as the server takes them. A clone's `.git` and Finder's
/// `.DS_Store` aren't part of the skill, and linked folders aren't followed, so a link back up
/// can't loop.
fn read_skill_folder(folder: &Path) -> Result<Vec<SkillFile>> {
    let mut files = Vec::new();
    let mut size = 0;
    let mut pending = vec![PathBuf::new()];
    while let Some(relative) = pending.pop() {
        let directory = folder.join(&relative);
        let entries = std::fs::read_dir(&directory)
            .with_context(|| format!("reading {}", directory.display()))?;
        for entry in entries {
            let entry = entry.with_context(|| format!("reading {}", directory.display()))?;
            let name = entry.file_name();
            if name == ".git" || name == ".DS_Store" {
                continue;
            }
            let path = entry.path();
            let metadata =
                std::fs::metadata(&path).with_context(|| format!("reading {}", path.display()))?;
            if metadata.is_dir() {
                if !entry.file_type().is_ok_and(|kind| kind.is_symlink()) {
                    pending.push(relative.join(&name));
                }
                continue;
            }
            if !metadata.is_file() {
                continue;
            }
            size += metadata.len() as usize;
            anyhow::ensure!(size <= MAX_FOLDER_SIZE, FOLDER_TOO_LARGE);
            let data =
                std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
            files.push(SkillFile {
                path: slash_path(&relative.join(&name))?,
                data: base64::engine::general_purpose::STANDARD.encode(data),
                executable: is_executable(&metadata),
            });
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

/// `path`'s names with `/` between them, as the server takes them on any machine.
fn slash_path(path: &Path) -> Result<String> {
    let names = path
        .components()
        .map(|component| match component {
            Component::Normal(name) => name
                .to_str()
                .with_context(|| format!("{} isn't a name agentZ can send", path.display())),
            _ => anyhow::bail!("{} isn't a path in the folder", path.display()),
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(names.join("/"))
}

#[cfg(unix)]
fn is_executable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_: &std::fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use agentz_protocol::Response;
    use agentz_protocol::agents::{InstallState, RegistrySnapshot};
    use agentz_protocol::skills::SkippedSkill;
    use agentz_protocol::spaces::SpacesSnapshot;
    use gpui::TestAppContext;

    use super::super::Section;
    use super::super::tests::{account, listing};
    use super::*;

    fn accounts() -> AgentAccounts {
        AgentAccounts {
            accounts: vec![
                account(1, Some("Work"), None),
                account(2, Some("Side"), None),
                account(3, Some("Team"), None),
            ],
            ..AgentAccounts::default()
        }
    }

    #[test]
    fn skip_reasons_name_the_accounts_unless_all_skip() {
        let accounts = accounts();
        let reason = |skipping: &[Option<AccountId>]| {
            skip_reason("Claude Agent", "review", &accounts, skipping)
        };
        assert_eq!(
            reason(&[
                None,
                Some(AccountId(1)),
                Some(AccountId(2)),
                Some(AccountId(3))
            ]),
            "Claude Agent has its own skill named review, so it keeps that one."
        );
        assert_eq!(
            reason(&[Some(AccountId(2))]),
            "Claude Agent on Side has its own skill named review, so it keeps that one."
        );
        assert_eq!(
            reason(&[None, Some(AccountId(1)), Some(AccountId(3))]),
            "Claude Agent on Outside agentZ, Work and Team has its own skill named review, so \
             it keeps that one."
        );
    }

    #[test]
    fn a_folder_is_read_without_git_finder_or_linked_folders() -> Result<()> {
        let folder = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        std::fs::write(folder.path().join("SKILL.md"), "---\nname: x\n---\n")?;
        std::fs::create_dir_all(folder.path().join("scripts"))?;
        std::fs::write(folder.path().join("scripts/run.sh"), "#!/bin/sh\n")?;
        std::fs::create_dir_all(folder.path().join(".git"))?;
        std::fs::write(folder.path().join(".git/config"), "")?;
        std::fs::write(folder.path().join(".DS_Store"), "")?;
        std::fs::write(outside.path().join("secret.md"), "")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(
                folder.path().join("scripts/run.sh"),
                std::fs::Permissions::from_mode(0o755),
            )?;
            std::os::unix::fs::symlink(outside.path(), folder.path().join("linked"))?;
        }

        let files = read_skill_folder(folder.path())?;
        let paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
        assert_eq!(paths, ["SKILL.md", "scripts/run.sh"]);
        assert!(!files[0].executable);
        assert_eq!(files[1].executable, cfg!(unix));
        assert_eq!(files[1].data, "IyEvYmluL3NoCg==");
        Ok(())
    }

    #[gpui::test]
    fn skills_are_listed_created_and_deleted(cx: &mut TestAppContext) {
        let mock = AgentId::new("mock");
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
                        agents: vec![listing(
                            "mock",
                            "Mock",
                            InstallState::Installed {
                                version: "2.0.0".into(),
                                update_available: false,
                            },
                        )],
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
                        Request::CreateSkill { .. } | Request::DeleteSkill(_) => Some(Response::Ok),
                        _ => None,
                    }
                });
                client.set_accounts_for_test([(mock.clone(), accounts())].into(), cx);
            });
            crate::machines::init_for_test(vec![client.clone()], cx);
            crate::project_info::init(cx);
            client
        });
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        let confirms: Rc<RefCell<Vec<ConfirmRequest>>> = Rc::default();
        cx.update(|_, cx| {
            let confirms = confirms.clone();
            cx.subscribe(&page, move |_, event: &SettingsPageEvent, _| {
                if let SettingsPageEvent::Confirm(request) = event {
                    confirms.borrow_mut().push(request.clone());
                }
            })
            .detach();
        });
        page.update_in(cx, |page, window, cx| {
            page.select(Section::Skills, window, cx)
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
        let is_creating = |cx: &mut gpui::VisualTestContext| {
            page.read_with(cx, |page, _| {
                matches!(page.skills_page, SkillsPage::Create(_))
            })
        };

        // With none yet, the page offers to create one.
        click("skill-create-empty", cx);
        assert!(is_creating(cx));
        let is_name_focused = |cx: &mut gpui::VisualTestContext| {
            page.update_in(cx, |page, window, cx| match &page.skills_page {
                SkillsPage::Create(form) => form.name.focus_handle(cx).is_focused(window),
                SkillsPage::List => false,
            })
        };
        assert!(is_name_focused(cx));
        assert!(cx.debug_bounds("skills-back").is_some());

        // Zed's checks: nothing is sent until every field is right.
        click("skill-save", cx);
        let errors = |cx: &mut gpui::VisualTestContext| {
            page.read_with(cx, |page, _| match &page.skills_page {
                SkillsPage::Create(form) => {
                    (form.name_error, form.description_error, form.body_error)
                }
                SkillsPage::List => panic!("the form is open"),
            })
        };
        assert_eq!(
            errors(cx),
            (
                Some("Skill name cannot be empty"),
                Some("Skill description cannot be empty"),
                Some("Body is required."),
            )
        );
        page.update_in(cx, |page, window, cx| {
            if let SkillsPage::Create(form) = &page.skills_page {
                window.focus(&form.name.focus_handle(cx), cx);
            }
        });
        cx.simulate_input("Review");
        assert_eq!(
            errors(cx).0,
            Some("Skill name must contain only lowercase letters, numbers, and hyphens")
        );
        page.update(cx, |page, cx| {
            if let SkillsPage::Create(form) = &page.skills_page {
                form.name
                    .update(cx, |input, cx| input.set_text("review", cx));
                form.description
                    .update(cx, |input, cx| input.set_text("Reviews a change.", cx));
            }
        });
        // Enter starts a new line in the skill's content.
        click("skill-body", cx);
        cx.simulate_input("Read the diff.");
        cx.simulate_keystrokes("enter");
        cx.simulate_input("Say what's wrong.");
        assert_eq!(errors(cx), (None, None, None));
        assert!(
            !requests
                .borrow()
                .iter()
                .any(|request| matches!(request, Request::CreateSkill { .. }))
        );
        click("skill-save", cx);
        assert!(sent(&Request::CreateSkill {
            name: "review".into(),
            description: "Reviews a change.".into(),
            body: "Read the diff.\nSay what's wrong.".into(),
        }));
        assert!(!is_creating(cx));

        // The server lists it, with the account that keeps its own.
        let skills = vec![
            Skill {
                name: "release-notes".into(),
                description: "Writes release notes.".into(),
                path: "/tmp/agentz-test/skills/release-notes/SKILL.md".into(),
                skipped: Vec::new(),
            },
            Skill {
                name: "review".into(),
                description: "Reviews a change.".into(),
                path: "/tmp/agentz-test/skills/review/SKILL.md".into(),
                skipped: vec![SkippedSkill {
                    agent_id: mock,
                    account: Some(AccountId(1)),
                    own: "/tmp/agentz-test/accounts/mock/1/.mock/skills/review".into(),
                }],
            },
        ];
        client.update(cx, |client, cx| {
            client.set_skills_for_test(skills.clone(), cx)
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("skill-create-empty").is_none());
        let first = cx
            .debug_bounds("skill-release-notes")
            .expect("a skill is listed");
        let second = cx
            .debug_bounds("skill-review")
            .expect("the new skill is listed");
        assert!(first.top() < second.top());
        let reasons = page.read_with(cx, |page, cx| {
            skills
                .iter()
                .map(|skill| page.skip_reasons(skill, cx))
                .collect::<Vec<_>>()
        });
        assert_eq!(
            reasons,
            [
                Vec::new(),
                vec![
                    "Mock on Work has its own skill named review, so it keeps that one."
                        .to_string()
                ],
            ]
        );

        // Deleting asks first, naming the folder.
        click("skill-delete-1", cx);
        assert!(!sent(&Request::DeleteSkill("review".into())));
        let confirm = confirms.borrow_mut().pop().expect("deleting asks first");
        assert_eq!(confirm.title.as_ref(), "Delete the skill \"review\"?");
        assert!(confirm.message.contains("/tmp/agentz-test/skills/review "));
        cx.update(|window, cx| (confirm.on_confirm)(window, cx));
        cx.run_until_parked();
        assert!(sent(&Request::DeleteSkill("review".into())));
    }
}
