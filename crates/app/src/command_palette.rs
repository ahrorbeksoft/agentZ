//! Cmd-Shift-P: Zed's command palette. It lists the actions that apply where it was opened,
//! named as Zed names them ("workspaces: split right") with their keys, and runs the one
//! chosen there.

use gpui::{
    Action, AnyElement, App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    KeyBinding, ScrollHandle, Subscription, Window,
};
use text_input::{TextInput, TextInputEvent};
use ui::{HighlightedLabel, ListItem, ListItemSpacing, WithScrollbar as _, prelude::*};

use crate::project_switcher::fuzzy_match;

const KEY_CONTEXT: &str = "CommandPalette";

/// A list's and a text field's own keys (select next, backspace), which do nothing useful on
/// their own.
const HIDDEN_NAMESPACES: &[&str] = &["menu", "text_input"];

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", menu::SelectPrevious, Some(KEY_CONTEXT)),
        KeyBinding::new("down", menu::SelectNext, Some(KEY_CONTEXT)),
        KeyBinding::new("enter", menu::Confirm, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(KEY_CONTEXT)),
    ]);
}

struct Command {
    name: SharedString,
    action: Box<dyn Action>,
}

pub struct CommandPalette {
    commands: Vec<Command>,
    /// Where the palette was opened: its actions are listed, and the chosen one runs there.
    previous_focus: FocusHandle,
    search: Entity<TextInput>,
    /// The commands that match, by index, with where their names matched.
    matches: Vec<(usize, Vec<usize>)>,
    selected_index: usize,
    scroll_handle: ScrollHandle,
    _subscription: Subscription,
}

impl EventEmitter<DismissEvent> for CommandPalette {}

impl Focusable for CommandPalette {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.focus_handle(cx)
    }
}

impl CommandPalette {
    /// Opened while `previous_focus` still has focus, so its actions are the available ones.
    pub fn new(previous_focus: FocusHandle, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut commands: Vec<Command> = window
            .available_actions(cx)
            .into_iter()
            .filter(|action| {
                let namespace = action.name().split("::").next().unwrap_or_default();
                !HIDDEN_NAMESPACES.contains(&namespace)
            })
            .map(|action| Command {
                name: humanize_action_name(action.name()).into(),
                action,
            })
            .collect();
        commands.sort_by(|a, b| a.name.cmp(&b.name));
        let search = cx.new(|cx| TextInput::new("Execute a command…", cx));
        let subscription = cx.subscribe(&search, |this, _, _: &TextInputEvent, cx| {
            this.selected_index = 0;
            this.update_matches(cx);
        });
        window.focus(&search.focus_handle(cx), cx);
        let mut this = Self {
            commands,
            previous_focus,
            search,
            matches: Vec::new(),
            selected_index: 0,
            scroll_handle: ScrollHandle::new(),
            _subscription: subscription,
        };
        this.update_matches(cx);
        this
    }

    fn update_matches(&mut self, cx: &mut Context<Self>) {
        let query = normalize_action_query(&self.search.read(cx).text().to_lowercase());
        self.matches = self
            .commands
            .iter()
            .enumerate()
            .filter_map(|(index, command)| {
                match_name(&query, &command.name).map(|positions| (index, positions))
            })
            .collect();
        self.selected_index = self
            .selected_index
            .min(self.matches.len().saturating_sub(1));
        self.scroll_handle.scroll_to_item(0);
        cx.notify();
    }

    fn select_next(&mut self, _: &menu::SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        if !self.matches.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.matches.len();
            self.scroll_handle.scroll_to_item(self.selected_index);
            cx.notify();
        }
    }

    fn select_previous(
        &mut self,
        _: &menu::SelectPrevious,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.matches.is_empty() {
            self.selected_index = self
                .selected_index
                .checked_sub(1)
                .unwrap_or(self.matches.len() - 1);
            self.scroll_handle.scroll_to_item(self.selected_index);
            cx.notify();
        }
    }

    fn confirm(&mut self, _: &menu::Confirm, window: &mut Window, cx: &mut Context<Self>) {
        match self.matches.get(self.selected_index) {
            Some((command, _)) => self.run(*command, window, cx),
            None => cx.emit(DismissEvent),
        }
    }

    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    /// As Zed's: focus goes back first, so the action runs where the palette was opened.
    fn run(&mut self, command: usize, window: &mut Window, cx: &mut Context<Self>) {
        let action = self.commands[command].action.boxed_clone();
        window.focus(&self.previous_focus, cx);
        cx.emit(DismissEvent);
        window.dispatch_action(action, cx);
    }

    fn render_match(&self, index: usize, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (command_index, positions) = self.matches.get(index)?;
        let command = &self.commands[*command_index];
        let command_index = *command_index;
        Some(
            ListItem::new(("command", index))
                .inset(true)
                .spacing(ListItemSpacing::Sparse)
                .toggle_state(index == self.selected_index)
                .child(
                    h_flex()
                        .debug_selector(|| format!("command-{}", command.name))
                        .w_full()
                        .py_px()
                        .justify_between()
                        .gap_2()
                        .child(
                            HighlightedLabel::new(command.name.clone(), positions.clone())
                                .truncate(),
                        )
                        .child(div().flex_shrink_0().child(ui::KeyBinding::for_action_in(
                            command.action.as_ref(),
                            &self.previous_focus,
                            cx,
                        ))),
                )
                .on_click(
                    cx.listener(move |this, _, window, cx| this.run(command_index, window, cx)),
                )
                .into_any_element(),
        )
    }
}

impl Render for CommandPalette {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border_variant = cx.theme().colors().border_variant;
        let rows: Vec<AnyElement> = (0..self.matches.len())
            .filter_map(|index| self.render_match(index, cx))
            .collect();

        v_flex()
            .key_context(KEY_CONTEXT)
            .w(rems(38.))
            .elevation_3(cx)
            .overflow_hidden()
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::cancel))
            .child(
                h_flex()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(border_variant)
                    .child(self.search.clone()),
            )
            .child(
                // The scrollbar sits on this non-scrolling wrapper so it stays put, as in Zed.
                div()
                    .id("command-palette-scroll")
                    .child(
                        v_flex()
                            .id("command-palette-entries")
                            .max_h(rems(24.))
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll_handle)
                            .p_1()
                            .children(rows)
                            .when(self.matches.is_empty(), |list| {
                                list.child(
                                    div()
                                        .px_2()
                                        .py_1p5()
                                        .child(Label::new("No matches").color(Color::Muted)),
                                )
                            }),
                    )
                    .vertical_scrollbar_for(&self.scroll_handle, window, cx),
            )
    }
}

/// Where `query` (lowercase) matches `name`: as a whole where it appears, so "split right"
/// lights up there rather than letter by letter from the namespace on, or else in order.
fn match_name(query: &str, name: &str) -> Option<Vec<usize>> {
    // Action names are ASCII, so lowercasing keeps their byte positions.
    if let Some(start) = name.to_lowercase().find(query) {
        return Some(
            name.char_indices()
                .map(|(position, _)| position)
                .filter(|position| (start..start + query.len()).contains(position))
                .collect(),
        );
    }
    fuzzy_match(query, name)
}

/// Zed's: drops repeated spaces and the second colon of `::`, and reads underscores as spaces,
/// so a query matches the humanized name whether typed that way or as the keymap names it.
fn normalize_action_query(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut last_char = None;
    for char in input.trim().chars() {
        let normalized_char = if char == '_' { ' ' } else { char };
        match (last_char, normalized_char) {
            (Some(':'), ':') => continue,
            (Some(last_char), char) if last_char.is_whitespace() && char.is_whitespace() => {
                continue;
            }
            _ => last_char = Some(normalized_char),
        }
        result.push(normalized_char);
    }
    result
}

/// Zed's: `workspaces::SplitRight` reads "workspaces: split right", keeping acronyms whole.
fn humanize_action_name(name: &str) -> String {
    let chars = name.chars().collect::<Vec<_>>();
    let capacity = name.len() + chars.iter().filter(|char| char.is_uppercase()).count();
    let mut result = String::with_capacity(capacity);
    let mut index = 0;
    while index < chars.len() {
        let char = chars[index];
        if char == ':' {
            if result.ends_with(':') {
                result.push(' ');
            } else {
                result.push(':');
            }
            index += 1;
        } else if char == '_' {
            result.push(' ');
            index += 1;
        } else if char.is_uppercase() {
            let start = index;
            index += 1;
            while chars
                .get(index)
                .is_some_and(|next_char| next_char.is_uppercase())
            {
                index += 1;
            }
            let uppercase_run = &chars[start..index];
            if uppercase_run.len() > 1 {
                let split_before_last = chars
                    .get(index)
                    .is_some_and(|next_char| next_char.is_lowercase());
                let acronym_end = if split_before_last {
                    uppercase_run.len() - 1
                } else {
                    uppercase_run.len()
                };
                if acronym_end > 0 {
                    if !result.ends_with(' ') {
                        result.push(' ');
                    }
                    result.extend(&uppercase_run[..acronym_end]);
                }
                if split_before_last {
                    if !result.ends_with(' ') {
                        result.push(' ');
                    }
                    result.extend(uppercase_run[acronym_end].to_lowercase());
                }
            } else {
                if !result.ends_with(' ') {
                    result.push(' ');
                }
                result.extend(char.to_lowercase());
            }
        } else {
            result.push(char);
            index += 1;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{humanize_action_name, match_name, normalize_action_query};

    #[test]
    fn a_query_lights_up_where_it_appears_whole() {
        let name = "workspaces: split right";
        assert_eq!(match_name("split", name), Some(vec![12, 13, 14, 15, 16]));
        assert_eq!(match_name("wsr", name), Some(vec![0, 4, 18]));
        assert_eq!(match_name("diff", name), None);
    }

    #[test]
    fn action_names_read_as_zed_reads_them() {
        assert_eq!(
            humanize_action_name("workspaces::SplitRight"),
            "workspaces: split right"
        );
        assert_eq!(
            humanize_action_name("agentz::ToggleProjectSwitcher"),
            "agentz: toggle project switcher"
        );
        assert_eq!(humanize_action_name("go_to::Toggle"), "go to: toggle");
        assert_eq!(humanize_action_name("editor::OpenURL"), "editor: open URL");
        assert_eq!(
            humanize_action_name("editor::OpenURLParser"),
            "editor: open URL parser"
        );
    }

    #[test]
    fn queries_match_either_way_of_naming() {
        assert_eq!(normalize_action_query("split  right"), "split right");
        assert_eq!(
            normalize_action_query("workspaces::splitright"),
            "workspaces:splitright"
        );
        assert_eq!(normalize_action_query("new_thread"), "new thread");
    }
}
