//! The shortcut sheet, herdr's keybind help: Cmd-/ lists the shortcuts of what's focused (a
//! terminal, its find bar, a thread's message editor, a sidebar's search), then the Workspaces
//! view's and the app's, filtered by command or key as you type. Each key is the one that
//! would run from where the sheet was opened, so a key another binding takes there is left out.

use std::rc::Rc;

use gpui::{
    Action, AnyElement, App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    KeyBinding, KeyContext, ScrollHandle, Subscription, Window,
};
use text_input::{TextInput, TextInputEvent};
use ui::{WithScrollbar as _, prelude::*};

use crate::agent_view::{self, AcceptSlashCommand};
use crate::spaces_view::{
    self, ActivatePaneDown, ActivatePaneLeft, ActivatePaneRight, ActivatePaneUp, ActivateTab,
    ClosePane, NewTab, NewWorkspace, NextTab, PreviousTab, SplitDown, SplitRight, ToggleZoom,
};
use crate::terminal_thread_view::{self, CloseTerminal};
use crate::terminal_view::{
    self, Clear, DecreaseFontSize, DismissFind, Find, IncreaseFontSize, ResetFontSize,
    ScrollLineDown, ScrollLineUp, ScrollPageDown, ScrollPageUp, ScrollToBottom, ScrollToTop,
    SelectNextMatch, SelectPreviousMatch,
};
use crate::{
    GoTo, NewThread, OpenFolder, OpenSettings, Quit, ShowShortcuts, ToggleCommandPalette,
    ToggleDiff, ToggleProjectSwitcher, ToggleSidebar, ToggleTerminalDrawer, shell, sidebar,
};

const KEY_CONTEXT: &str = "ShortcutSheet";

pub fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", menu::Cancel, Some(KEY_CONTEXT))]);
}

/// A row: what it does, and its keys.
struct Shortcut {
    label: &'static str,
    /// The binding, or a range's first and last, as Tab 1–9's.
    bindings: Vec<KeyBinding>,
    /// The keys as typed (`cmd-d`) and as spelled out (`Command-D`), lowercased, to filter by.
    key_text: String,
}

struct ShortcutGroup {
    title: &'static str,
    shortcuts: Vec<Shortcut>,
}

pub struct ShortcutSheet {
    search: Entity<TextInput>,
    groups: Vec<ShortcutGroup>,
    scroll_handle: ScrollHandle,
    _subscription: Subscription,
}

impl EventEmitter<DismissEvent> for ShortcutSheet {}

impl Focusable for ShortcutSheet {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.focus_handle(cx)
    }
}

impl ShortcutSheet {
    /// The sheet for what had focus, whose key contexts are `context_stack`.
    pub fn new(
        context_stack: Vec<KeyContext>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| TextInput::new("Filter by command or shortcut…", cx));
        let subscription = cx.subscribe(&search, |this, _, _: &TextInputEvent, cx| {
            this.scroll_handle.set_offset(gpui::point(px(0.), px(0.)));
            cx.notify();
        });
        window.focus(&search.focus_handle(cx), cx);
        Self {
            search,
            groups: shortcut_groups(&context_stack, cx),
            scroll_handle: ScrollHandle::new(),
            _subscription: subscription,
        }
    }

    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    /// herdr's filter: rows whose command or key holds the query, ignoring case, in the groups
    /// that keep any.
    fn visible_groups(&self, cx: &App) -> Vec<(&'static str, Vec<&Shortcut>)> {
        let query = self.search.read(cx).text().trim().to_lowercase();
        self.groups
            .iter()
            .filter_map(|group| {
                let shortcuts = group
                    .shortcuts
                    .iter()
                    .filter(|shortcut| {
                        query.is_empty()
                            || shortcut.label.to_lowercase().contains(&query)
                            || shortcut.key_text.contains(&query)
                    })
                    .collect::<Vec<_>>();
                (!shortcuts.is_empty()).then_some((group.title, shortcuts))
            })
            .collect()
    }
}

/// The groups that apply where `context_stack` is: what's focused first.
fn shortcut_groups(context_stack: &[KeyContext], cx: &App) -> Vec<ShortcutGroup> {
    let has = |context: &str| context_stack.iter().any(|entry| entry.contains(context));
    let entry = |label: &'static str, action: &dyn Action| (label, vec![action.boxed_clone()]);
    let mut groups = Vec::new();
    if has(terminal_view::KEY_CONTEXT) {
        let mut terminal = Vec::new();
        if has(terminal_thread_view::KEY_CONTEXT) {
            terminal.push(entry("Close Terminal", &CloseTerminal));
        }
        terminal.extend([
            entry("Find", &Find),
            entry("Copy", &terminal_view::Copy),
            entry("Paste", &terminal_view::Paste),
            entry("Select All", &terminal_view::SelectAll),
            entry("Clear", &Clear),
            entry("Scroll Page Up", &ScrollPageUp),
            entry("Scroll Page Down", &ScrollPageDown),
            entry("Scroll Line Up", &ScrollLineUp),
            entry("Scroll Line Down", &ScrollLineDown),
            entry("Scroll to Top", &ScrollToTop),
            entry("Scroll to Bottom", &ScrollToBottom),
            entry("Increase Font Size", &IncreaseFontSize),
            entry("Decrease Font Size", &DecreaseFontSize),
            entry("Reset Font Size", &ResetFontSize),
        ]);
        groups.push(("Terminal", terminal));
    }
    if has(terminal_view::FIND_KEY_CONTEXT) {
        groups.push((
            "Find in Terminal",
            vec![
                entry("Select Next Match", &SelectNextMatch),
                entry("Select Previous Match", &SelectPreviousMatch),
                entry("Close Search Bar", &DismissFind),
            ],
        ));
    }
    if has(agent_view::KEY_CONTEXT) {
        groups.push((
            "Thread",
            vec![
                entry("Send Message", &menu::Confirm),
                entry("Stop Generation", &menu::Cancel),
                entry("Complete Slash Command", &AcceptSlashCommand),
            ],
        ));
    }
    if has(sidebar::SEARCH_KEY_CONTEXT) {
        groups.push((
            "Sidebar Search",
            vec![
                entry("Next Result", &menu::SelectNext),
                entry("Previous Result", &menu::SelectPrevious),
                entry("Open Result", &menu::Confirm),
                entry("Clear Search", &menu::Cancel),
            ],
        ));
    }
    if has(spaces_view::SEARCH_KEY_CONTEXT) {
        groups.push(("Sidebar Search", vec![entry("Clear Search", &menu::Cancel)]));
    }
    let in_workspaces = has(spaces_view::KEY_CONTEXT);
    if in_workspaces {
        groups.push((
            "Workspaces",
            vec![
                entry("New Workspace…", &NewWorkspace),
                entry("New Tab", &NewTab),
                entry("Next Tab", &NextTab),
                entry("Previous Tab", &PreviousTab),
                (
                    "Tab 1–9",
                    vec![Box::new(ActivateTab(1)), Box::new(ActivateTab(9))],
                ),
                entry("Split Right", &SplitRight),
                entry("Split Down", &SplitDown),
                entry("Close Pane", &ClosePane),
                entry("Zoom In or Out", &ToggleZoom),
                entry("Focus Pane Left", &ActivatePaneLeft),
                entry("Focus Pane Right", &ActivatePaneRight),
                entry("Focus Pane Up", &ActivatePaneUp),
                entry("Focus Pane Down", &ActivatePaneDown),
            ],
        ));
    }
    if has(shell::KEY_CONTEXT) {
        let mut general = vec![
            entry("New Thread…", &NewThread),
            entry("Open Folder…", &OpenFolder),
            entry("Toggle Sidebar", &ToggleSidebar),
        ];
        // The project switcher and the open thread's panels, which the Workspaces view
        // doesn't show.
        if !in_workspaces {
            general.push(entry("Switch Project…", &ToggleProjectSwitcher));
            general.push(entry("Toggle Changes", &ToggleDiff));
            general.push(entry("Toggle Terminal", &ToggleTerminalDrawer));
        }
        general.extend([
            entry("Command Palette", &ToggleCommandPalette),
            entry("Go To…", &GoTo),
            entry("Show Shortcuts", &ShowShortcuts),
            entry("Settings…", &OpenSettings),
            entry("Quit agentZ", &Quit),
        ]);
        groups.push(("General", general));
    }

    groups
        .into_iter()
        .map(|(title, entries)| ShortcutGroup {
            title,
            shortcuts: entries
                .into_iter()
                .filter_map(|(label, actions)| {
                    let bindings = actions
                        .iter()
                        .map(|action| binding_in(action.as_ref(), context_stack, cx))
                        .collect::<Option<Vec<_>>>()?;
                    let key_text = bindings
                        .iter()
                        .flat_map(|binding| {
                            let typed = binding
                                .keystrokes()
                                .iter()
                                .map(|keystroke| keystroke.unparse())
                                .collect::<Vec<_>>()
                                .join(" ");
                            [
                                typed,
                                ui::text_for_keybinding_keystrokes(binding.keystrokes(), cx),
                            ]
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                        .to_lowercase();
                    Some(Shortcut {
                        label,
                        bindings,
                        key_text,
                    })
                })
                .collect(),
        })
        .filter(|group| !group.shortcuts.is_empty())
        .collect()
}

/// The binding that runs `action` from `context_stack`, as GPUI shows one for the focused
/// element: the latest whose keys no other binding there takes first.
fn binding_in(action: &dyn Action, context_stack: &[KeyContext], cx: &App) -> Option<KeyBinding> {
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    keymap
        .bindings_for_action(action)
        .rev()
        .find(|binding| {
            keymap
                .bindings_for_input(binding.keystrokes(), context_stack)
                .0
                .first()
                .is_some_and(|winner| winner.action().partial_eq(binding.action()))
        })
        .cloned()
}

fn render_keys(bindings: &[KeyBinding]) -> AnyElement {
    let key = |binding: &KeyBinding| {
        ui::KeyBinding::from_keystrokes(Rc::from(binding.keystrokes()), false).into_any_element()
    };
    match bindings {
        [first, .., last] => h_flex()
            .gap_1()
            .child(key(first))
            .child(Label::new("–").size(LabelSize::Small).color(Color::Muted))
            .child(key(last))
            .into_any_element(),
        [binding] => key(binding),
        [] => gpui::Empty.into_any_element(),
    }
}

impl Render for ShortcutSheet {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border_variant = cx.theme().colors().border_variant;
        let groups = self.visible_groups(cx);
        let empty_state = groups.is_empty().then(|| {
            div()
                .p_3()
                .child(Label::new("No matching shortcuts").color(Color::Muted))
        });
        let groups =
            groups
                .into_iter()
                .map(|(title, shortcuts)| {
                    v_flex()
                        .debug_selector(move || format!("shortcut-group-{title}"))
                        .pb_2()
                        .child(
                            div().px_2().pt_1().pb_0p5().child(
                                Label::new(title).size(LabelSize::Small).color(Color::Muted),
                            ),
                        )
                        .children(shortcuts.into_iter().map(|shortcut| {
                            h_flex()
                                .debug_selector(|| format!("shortcut-{}", shortcut.label))
                                .px_2()
                                .py_0p5()
                                .gap_4()
                                .justify_between()
                                .child(Label::new(shortcut.label))
                                .child(render_keys(&shortcut.bindings))
                        }))
                        .into_any_element()
                })
                .collect::<Vec<_>>();

        v_flex()
            .key_context(KEY_CONTEXT)
            .w(rems(30.))
            .max_h(rems(34.))
            .elevation_3(cx)
            .overflow_hidden()
            .on_action(cx.listener(Self::cancel))
            .child(
                h_flex()
                    .px_3()
                    .py_2p5()
                    .gap_3()
                    .border_b_1()
                    .border_color(border_variant)
                    .child(
                        div()
                            .flex_none()
                            .child(Label::new("Shortcuts").color(Color::Muted)),
                    )
                    .child(div().flex_1().min_w_0().child(self.search.clone())),
            )
            .child(
                // The scrollbar sits on this non-scrolling wrapper so it stays put, as in Zed.
                div()
                    .id("shortcut-sheet-scroll")
                    .child(
                        v_flex()
                            .id("shortcut-sheet-groups")
                            .max_h(rems(28.))
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll_handle)
                            .p_1()
                            .children(groups)
                            .children(empty_state),
                    )
                    .vertical_scrollbar_for(&self.scroll_handle, window, cx),
            )
    }
}

#[cfg(test)]
mod tests {
    use gpui::{TestAppContext, VisualTestContext};

    use super::*;

    fn titles(sheet: &Entity<ShortcutSheet>, cx: &mut VisualTestContext) -> Vec<&'static str> {
        sheet.read_with(cx, |sheet, cx| {
            sheet
                .visible_groups(cx)
                .into_iter()
                .map(|(title, _)| title)
                .collect()
        })
    }

    fn labels(sheet: &Entity<ShortcutSheet>, cx: &mut VisualTestContext) -> Vec<&'static str> {
        sheet.read_with(cx, |sheet, cx| {
            sheet
                .visible_groups(cx)
                .into_iter()
                .flat_map(|(_, shortcuts)| shortcuts.into_iter().map(|shortcut| shortcut.label))
                .collect()
        })
    }

    fn stack(contexts: &[&str]) -> Vec<KeyContext> {
        contexts
            .iter()
            .map(|context| KeyContext::parse(context).expect("a valid context"))
            .collect()
    }

    #[gpui::test]
    fn the_sheet_lists_what_is_focused_first_and_filters_as_you_type(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init_for_test(cx);
            init(cx);
            agent_view::init(cx);
        });
        let in_terminal = stack(&["Shell", "Workspaces", "Terminal"]);
        let (sheet, cx) =
            cx.add_window_view(|window, cx| ShortcutSheet::new(in_terminal, window, cx));
        cx.run_until_parked();
        assert_eq!(titles(&sheet, cx), ["Terminal", "Workspaces", "General"]);
        assert!(cx.debug_bounds("shortcut-Find").is_some());
        // Cmd-D splits here; the Agents view's Cmd-D for changes doesn't apply.
        let all = labels(&sheet, cx);
        assert!(all.contains(&"Split Right"));
        assert!(!all.contains(&"Toggle Changes"));
        assert!(!all.contains(&"Toggle Terminal"));

        // By command, ignoring case.
        cx.simulate_input("SPLIT");
        assert_eq!(titles(&sheet, cx), ["Workspaces"]);
        assert_eq!(labels(&sheet, cx), ["Split Right", "Split Down"]);

        // By key, as typed or spelled out.
        sheet.update(cx, |sheet, cx| {
            sheet.search.update(cx, |search, cx| {
                search.set_text(crate::platform_keys("cmd-f", "ctrl-shift-f"), cx)
            })
        });
        assert_eq!(labels(&sheet, cx), ["Find"]);
        // On Linux, Ctrl-Shift-E would match Ctrl-Shift-Enter too.
        let (spelled_out, label) = if cfg!(target_os = "macos") {
            ("command-shift-d", "Split Down")
        } else {
            ("ctrl-shift-o", "Split Right")
        };
        sheet.update(cx, |sheet, cx| {
            sheet
                .search
                .update(cx, |search, cx| search.set_text(spelled_out, cx))
        });
        assert_eq!(labels(&sheet, cx), [label]);
        sheet.update(cx, |sheet, cx| {
            sheet
                .search
                .update(cx, |search, cx| search.set_text("nothing like this", cx))
        });
        assert!(titles(&sheet, cx).is_empty());
        cx.run_until_parked();
        assert!(cx.debug_bounds("shortcut-group-General").is_none());
    }

    #[gpui::test]
    fn a_thread_in_the_agents_view_gets_its_own_shortcuts(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init_for_test(cx);
            init(cx);
            agent_view::init(cx);
        });
        let in_thread = stack(&["Shell", "AgentComposer", "TextInput"]);
        let (sheet, cx) =
            cx.add_window_view(|window, cx| ShortcutSheet::new(in_thread, window, cx));
        assert_eq!(titles(&sheet, cx), ["Thread", "General"]);
        let all = labels(&sheet, cx);
        assert!(all.contains(&"Send Message"));
        assert!(all.contains(&"Toggle Changes"));
        assert!(!all.contains(&"Split Right"));
    }

    /// Cmd-W closes a terminal thread; a thread's drawer terminal has no such key.
    #[gpui::test]
    fn a_terminal_thread_closes_with_cmd_w(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init_for_test(cx);
            init(cx);
        });
        let in_terminal_thread = stack(&["Shell", "TerminalThread", "Terminal"]);
        let (sheet, cx) =
            cx.add_window_view(|window, cx| ShortcutSheet::new(in_terminal_thread, window, cx));
        cx.simulate_input(crate::platform_keys("cmd-w", "ctrl-shift-w"));
        assert_eq!(labels(&sheet, cx), ["Close Terminal"]);

        let in_drawer = stack(&["Shell", "Terminal"]);
        let drawer_labels: Vec<&str> = cx.update(|_, cx| {
            shortcut_groups(&in_drawer, cx)
                .iter()
                .flat_map(|group| group.shortcuts.iter().map(|shortcut| shortcut.label))
                .collect()
        });
        assert!(drawer_labels.contains(&"Find"));
        assert!(!drawer_labels.contains(&"Close Terminal"));
    }
}
