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
                    // An editor dials a target from `~/.ssh/config`, which a cloud machine
                    // reached with its provider's options doesn't have.
                    Transport::Ssh(target)
                    | Transport::Cloud {
                        target: Some(target),
                        ..
                    } => Some(Remote {
                        target: target.clone(),
                        label: client.label().clone(),
                    }),
                    Transport::Local | Transport::Cloud { target: None, .. } => None,
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

    /// Whether the folder is on another machine, which only an editor can open from here.
    pub fn is_remote(&self) -> bool {
        self.remote.is_some()
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

/// A file a link in a message names, read as t3code's `parseMarkdownFileLink` reads one: a
/// `file:` URL, or a path with no scheme, absolute or from the thread's folder, with an
/// optional `:line[:column]` or `#L<line>`. Agents link files this way (Codex's
/// `[AUDIT.md](/Users/…/AUDIT.md)`), and the OS can't open a bare path as a URL.
#[derive(Debug, PartialEq)]
pub struct FileLink {
    pub path: PathBuf,
    pub line: Option<u32>,
}

impl FileLink {
    pub fn parse(url: &str, folder: Option<&Path>) -> Option<Self> {
        let url = url.trim();
        let url = url
            .strip_prefix('<')
            .and_then(|url| url.strip_suffix('>'))
            .unwrap_or(url);
        if url.is_empty() || url.starts_with('#') || url.starts_with("//") {
            return None;
        }
        let (path, fragment) = if url
            .get(..5)
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("file:"))
        {
            let url = url::Url::parse(url).ok()?;
            let path = url.to_file_path().ok()?.to_string_lossy().into_owned();
            (path, url.fragment().map(str::to_string))
        } else {
            let (path, fragment) = match url.split_once('#') {
                Some((path, fragment)) => (path, Some(fragment.to_string())),
                None => (url, None),
            };
            let path = path.split_once('?').map_or(path, |(path, _)| path);
            let path = percent_encoding::percent_decode_str(path)
                .decode_utf8()
                .map_or_else(|_| path.to_string(), |path| path.into_owned());
            if has_scheme(&path) {
                return None;
            }
            (path, fragment)
        };
        let (path, line) = split_line(&path);
        let line = line.or_else(|| fragment.as_deref().and_then(fragment_line));
        if path.is_empty() {
            return None;
        }
        let path = Path::new(path);
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            folder?.join(path)
        };
        Some(Self { path, line })
    }
}

/// Whether a link names a scheme, as `https:` or `mailto:` do. A file's `:12` isn't one.
fn has_scheme(link: &str) -> bool {
    let Some((scheme, rest)) = link.split_once(':') else {
        return false;
    };
    let is_scheme = scheme.starts_with(|character: char| character.is_ascii_alphabetic())
        && scheme.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
        });
    is_scheme
        && (rest.starts_with("//")
            || !rest
                .split(':')
                .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit())))
}

/// A path's `:line` or `:line:column` suffix, split off.
fn split_line(path: &str) -> (&str, Option<u32>) {
    let is_number = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    let Some((rest, last)) = path.rsplit_once(':').filter(|(_, last)| is_number(last)) else {
        return (path, None);
    };
    let (rest, line) = match rest.rsplit_once(':').filter(|(_, line)| is_number(line)) {
        Some((rest, line)) => (rest, line),
        None => (rest, last),
    };
    (rest, line.parse().ok().filter(|line| *line > 0))
}

/// The line of a GitHub-style `#L12`, `#L12C3` or `#L12-L20`.
fn fragment_line(fragment: &str) -> Option<u32> {
    let digits = fragment.strip_prefix(['L', 'l'])?;
    let end = digits
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(digits.len());
    digits[..end].parse().ok().filter(|line| *line > 0)
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
    fn a_link_names_a_file_by_its_path_from_the_folder_and_its_line() {
        let folder = Path::new("/work/demo");
        let parse = |url: &str| FileLink::parse(url, Some(folder));
        let file = |path: &str, line: Option<u32>| {
            Some(FileLink {
                path: path.into(),
                line,
            })
        };
        assert_eq!(
            parse("/Users/me/agentZ/AUDIT.md"),
            file("/Users/me/agentZ/AUDIT.md", None)
        );
        assert_eq!(
            parse("/work/demo/src/projects.rs:978"),
            file("/work/demo/src/projects.rs", Some(978))
        );
        assert_eq!(
            parse("src/main.rs:12:5"),
            file("/work/demo/src/main.rs", Some(12))
        );
        assert_eq!(
            parse("README.md#L4-L9"),
            file("/work/demo/README.md", Some(4))
        );
        assert_eq!(parse("./docs"), file("/work/demo/./docs", None));
        assert_eq!(parse("<docs/a b.md>"), file("/work/demo/docs/a b.md", None));
        assert_eq!(
            parse("docs/a%20b.md?plain=1#L3"),
            file("/work/demo/docs/a b.md", Some(3))
        );
        assert_eq!(
            parse("file:///work/demo/a%20b.rs#L7"),
            file("/work/demo/a b.rs", Some(7))
        );
        assert_eq!(parse("Makefile:0"), file("/work/demo/Makefile", None));

        for url in [
            "https://example.com/a.rs",
            "mailto:me@example.com",
            "agentz://handoff",
            "#usage",
            "//example.com/a.rs",
            "",
        ] {
            assert_eq!(parse(url), None, "{url}");
        }
        // A relative path needs the thread's folder.
        assert_eq!(FileLink::parse("src/main.rs", None), None);
        assert_eq!(FileLink::parse("/tmp/a.rs", None), file("/tmp/a.rs", None));
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
