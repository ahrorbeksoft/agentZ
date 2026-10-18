//! Open in Editor (`design/open-in-editor/decisions.md`; t3code's OpenInPicker and
//! `launchEditor`): the editors a thread's folder opens in from this Mac, and the button and
//! menu that list them.
//!
//! A folder on this Mac opens through its server ([`Request::OpenInEditor`]), which knows which
//! editors are installed. A folder on another machine opens in an editor on this Mac over SSH:
//! the OS is handed the editor's own link ([`Editor::remote_url`]) and the editor connects by
//! itself.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use agentz_protocol::editors::{self, Editor};
use agentz_protocol::{Request, Response};
use anyhow::{Result, anyhow, bail};
use gpui::{AnyElement, App, ClipboardItem, Task, Window};
use ui::{ButtonLike, ContextMenu, ContextMenuEntry, PopoverMenu, Tooltip, prelude::*};
use util::ResultExt as _;

use crate::OpenInEditor;
use crate::app_settings::AppSettingsStore;
use crate::machines::{MachineId, Machines};
use crate::server_client::Transport;

/// Opens the folder in the editor that was picked.
pub type OpenWith = Rc<dyn Fn(&'static Editor, &mut Window, &mut App)>;

/// The icon an editor's rows and buttons show: its own where agentZ has one, else the
/// outward arrow.
pub fn editor_icon(editor: &Editor) -> IconName {
    match editor.id {
        "cursor" => IconName::EditorCursor,
        "vscode" | "vscode-insiders" | "vscodium" => IconName::EditorVsCode,
        "zed" => IconName::AiZed,
        _ if editor.launch == editors::LaunchStyle::LineColumn => IconName::EditorJetBrains,
        _ => IconName::ArrowUpRight,
    }
}

/// The installed editors that can open a folder, last used first and the rest as
/// [`editors::EDITORS`] lists them. A folder on another machine opens only in those with a
/// link for it.
pub fn order(
    installed: &[String],
    last_used: Option<&str>,
    over_ssh: bool,
) -> Vec<&'static Editor> {
    let mut editors: Vec<&'static Editor> = installed
        .iter()
        .filter_map(|id| editors::find(id))
        .filter(|editor| !over_ssh || editor.opens_over_ssh())
        .collect();
    // Stable, so only the last used moves.
    editors.sort_by_key(|editor| Some(editor.id) != last_used);
    editors
}

/// The SSH machine a folder is on.
#[derive(Clone)]
struct Remote {
    /// What this Mac dials for it.
    target: String,
    label: SharedString,
}

/// What a folder on a machine can be opened in, from this Mac.
#[derive(Clone)]
pub struct Choices {
    editors: Vec<&'static Editor>,
    remote: Option<Remote>,
    /// Whether a folder has been opened over SSH from here before.
    has_opened_over_ssh: bool,
}

impl Choices {
    pub fn of(machine: MachineId, cx: &App) -> Self {
        let settings = AppSettingsStore::global(cx).read(cx).settings();
        let remote = Machines::global(cx)
            .read(cx)
            .client(machine, cx)
            .and_then(|client| {
                let client = client.read(cx);
                match client.transport() {
                    Transport::Ssh(target) => Some(Remote {
                        target: target.clone(),
                        label: client.label().clone(),
                    }),
                    Transport::Local => None,
                }
            });
        let editors = order(
            Machines::local(cx).read(cx).editors(),
            settings.last_editor.as_deref(),
            remote.is_some(),
        );
        Self {
            editors,
            remote,
            has_opened_over_ssh: settings.opened_over_ssh,
        }
    }

    /// The editor the button opens: the last used, else the first found.
    pub fn preferred(&self) -> Option<&'static Editor> {
        self.editors.first().copied()
    }

    /// The menu's first line for a folder on another machine, until one has been opened over
    /// SSH from here (`design/open-in-editor`, topic 3C).
    pub fn ssh_note(&self) -> Option<String> {
        let remote = self.remote.as_ref()?;
        (!self.has_opened_over_ssh && !self.editors.is_empty())
            .then(|| format!("Opens over SSH. Needs your key on {}", remote.label))
    }

    /// The menu's rows (`design/open-in-editor`, topic 2B): the preferred editor with its key,
    /// the others, then Show in Finder (a folder on this Mac) and Copy Path below a line. For
    /// another machine's folder, a first line says it opens over SSH until one has.
    pub fn add_to_menu(
        &self,
        mut menu: ContextMenu,
        folder: &Path,
        open: &OpenWith,
    ) -> ContextMenu {
        if let Some(note) = self.ssh_note() {
            menu = menu.label(note).separator();
        }
        match self.editors.split_first() {
            None => {
                menu = menu.item(ContextMenuEntry::new("No editor found").disabled(true));
            }
            Some((&preferred, others)) => {
                menu = menu.item(editor_entry(preferred, preferred.label.into(), true, open));
                if !others.is_empty() {
                    menu = menu.separator();
                    for &editor in others {
                        menu = menu.item(editor_entry(editor, editor.label.into(), false, open));
                    }
                }
            }
        }
        menu = menu.separator();
        if self.remote.is_none() {
            let folder = folder.to_path_buf();
            menu = menu.item(
                ContextMenuEntry::new(if cfg!(target_os = "macos") {
                    "Show in Finder"
                } else {
                    "Show in Files"
                })
                .icon(IconName::FolderOpen)
                .icon_color(Color::Muted)
                .handler(move |_, cx| cx.reveal_path(&folder)),
            );
        }
        let path = folder.display().to_string();
        menu.item(
            ContextMenuEntry::new("Copy Path")
                .icon(IconName::Copy)
                .icon_color(Color::Muted)
                .handler(move |_, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(path.clone()))
                }),
        )
    }

    /// What a narrow header's ⋯ gets: "Open in <preferred>" with its key, and "Open in…" with
    /// the whole menu.
    pub fn add_to_compact_menu(
        &self,
        mut menu: ContextMenu,
        folder: PathBuf,
        open: OpenWith,
    ) -> ContextMenu {
        if let Some(preferred) = self.preferred() {
            menu = menu.item(editor_entry(
                preferred,
                format!("Open in {}", preferred.label).into(),
                true,
                &open,
            ));
        }
        let choices = self.clone();
        menu.submenu_with_icon("Open in…", IconName::ArrowUpRight, move |menu, _, _| {
            choices.add_to_menu(menu, &folder, &open)
        })
    }
}

fn editor_entry(
    editor: &'static Editor,
    label: SharedString,
    has_key: bool,
    open: &OpenWith,
) -> ContextMenuEntry {
    let open = open.clone();
    ContextMenuEntry::new(label)
        .icon(editor_icon(editor))
        .icon_color(Color::Muted)
        .when(has_key, |entry| entry.action(Box::new(OpenInEditor)))
        .handler(move |window, cx| open(editor, window, cx))
}

/// The header's split button (`design/open-in-editor`, topic 1A): the preferred editor's icon
/// and "Open", which opens the folder there with one click, and a chevron for the menu. With
/// no editor found, a plain "Open" with an outward arrow that opens the menu.
pub fn render_split_button(
    machine: MachineId,
    folder: PathBuf,
    open: OpenWith,
    cx: &App,
) -> AnyElement {
    let choices = Choices::of(machine, cx);
    let colors = cx.theme().colors();
    let border = colors.border_variant;
    let menu = {
        let open = open.clone();
        // Asked again as the menu opens, so an editor installed since is there next time.
        move |window: &mut Window, cx: &mut App| {
            Machines::local(cx).update(cx, |client, cx| client.refresh_editors(cx));
            let choices = Choices::of(machine, cx);
            let folder = folder.clone();
            let open = open.clone();
            Some(ContextMenu::build(window, cx, move |menu, _, _| {
                choices.add_to_menu(menu, &folder, &open)
            }))
        }
    };
    let Some(editor) = choices.preferred() else {
        return div()
            .debug_selector(|| "thread-header-open".into())
            .child(
                PopoverMenu::new("open-in-editor-menu")
                    .menu(menu)
                    .trigger_with_tooltip(
                        ButtonLike::new("open-in-editor")
                            .style(ButtonStyle::Outlined)
                            .size(ButtonSize::Compact)
                            .child(
                                h_flex()
                                    .gap_1()
                                    .px_1()
                                    .child(Label::new("Open").size(LabelSize::Small))
                                    .child(
                                        Icon::new(IconName::ArrowUpRight)
                                            .size(IconSize::XSmall)
                                            .color(Color::Muted),
                                    ),
                            ),
                        Tooltip::text("No editor found"),
                    )
                    .anchor(gpui::Anchor::TopRight),
            )
            .into_any_element();
    };
    let tooltip: SharedString = format!("Open in {}", editor.label).into();
    h_flex()
        .debug_selector(|| "thread-header-open".into())
        .flex_none()
        .rounded_md()
        .border_1()
        .border_color(border)
        .bg(colors.element_background)
        .overflow_hidden()
        .child(
            div()
                .debug_selector(|| "thread-header-open-editor".into())
                .child(
                    ButtonLike::new("open-in-editor")
                        .style(ButtonStyle::Subtle)
                        .size(ButtonSize::Compact)
                        .child(
                            h_flex()
                                .gap_1()
                                .child(Icon::new(editor_icon(editor)).size(IconSize::XSmall))
                                .child(Label::new("Open").size(LabelSize::Small)),
                        )
                        .tooltip(move |_, cx| {
                            Tooltip::for_action(tooltip.clone(), &OpenInEditor, cx)
                        })
                        .on_click(move |_, window, cx| open(editor, window, cx)),
                ),
        )
        .child(div().w_px().h(px(12.)).flex_none().bg(border))
        .child(
            div()
                .debug_selector(|| "thread-header-open-menu".into())
                .child(
                    PopoverMenu::new("open-in-editor-menu")
                        .menu(menu)
                        .trigger_with_tooltip(
                            ButtonLike::new("open-in-editor-chevron")
                                .style(ButtonStyle::Subtle)
                                .size(ButtonSize::Compact)
                                .child(
                                    Icon::new(IconName::ChevronDown)
                                        .size(IconSize::XSmall)
                                        .color(Color::Muted),
                                ),
                            Tooltip::text("Open In…"),
                        )
                        .anchor(gpui::Anchor::TopRight),
                ),
        )
        .into_any_element()
}

/// Opens `path` on `machine` in `editor`, at `line` where the editor takes one, and makes it
/// the editor the button opens next. On this Mac the server starts the editor; for another
/// machine's path the OS is given the editor's link.
pub fn open(
    machine: MachineId,
    path: &Path,
    editor: &'static Editor,
    line: Option<u32>,
    cx: &mut App,
) -> Task<Result<()>> {
    let choices = Choices::of(machine, cx);
    AppSettingsStore::global(cx).update(cx, |store, cx| {
        store.update(
            |settings| {
                settings.last_editor = Some(editor.id.to_string());
                if choices.remote.is_some() {
                    settings.opened_over_ssh = true;
                }
            },
            cx,
        )
    });
    if let Some(remote) = &choices.remote {
        let Some(url) = editor.remote_url(&remote.target, &path.to_string_lossy(), line) else {
            return Task::ready(Err(anyhow!(
                "{} can't open a folder on another machine",
                editor.label
            )));
        };
        cx.open_url(&url);
        return Task::ready(Ok(()));
    }
    let response = Machines::local(cx).read(cx).request(Request::OpenInEditor {
        editor: editor.id.to_string(),
        path: path.to_path_buf(),
        line,
    });
    cx.spawn(async move |_| match response.await? {
        Response::Ok => Ok(()),
        response => bail!("expected Ok, got {response:?}"),
    })
}

/// Opens as [`open`] does, and logs what goes wrong, for a click with nowhere to show it.
pub fn open_and_log(
    machine: MachineId,
    path: &Path,
    editor: &'static Editor,
    line: Option<u32>,
    cx: &mut App,
) {
    let task = open(machine, path, editor, line, cx);
    cx.spawn(async move |_| {
        task.await.log_err();
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(editors: Vec<&'static Editor>) -> Vec<&'static str> {
        editors.into_iter().map(|editor| editor.id).collect()
    }

    fn installed(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| id.to_string()).collect()
    }

    #[test]
    fn the_last_used_editor_comes_first_and_the_rest_keep_their_order() {
        let found = installed(&["cursor", "vscode", "zed", "webstorm"]);
        assert_eq!(
            ids(order(&found, None, false)),
            ["cursor", "vscode", "zed", "webstorm"]
        );
        assert_eq!(
            ids(order(&found, Some("zed"), false)),
            ["zed", "cursor", "vscode", "webstorm"]
        );
        // One that isn't installed here doesn't take the first place.
        assert_eq!(
            ids(order(&found, Some("rider"), false)),
            ["cursor", "vscode", "zed", "webstorm"]
        );
        assert!(order(&[], Some("zed"), false).is_empty());
    }

    #[test]
    fn a_folder_on_another_machine_opens_only_in_editors_with_a_link_for_it() {
        let found = installed(&["cursor", "vscode", "zed", "webstorm"]);
        assert_eq!(
            ids(order(&found, Some("webstorm"), true)),
            ["cursor", "vscode", "zed"]
        );
    }
}
