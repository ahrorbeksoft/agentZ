//! Save Layout… from a tab's menu, as iTerm2 saves arrangements: the tab's splits under a
//! name, each pane running its command again or opening a plain shell. Layouts are kept in
//! the app's settings, so one opens as a tab in any workspace on any machine.

use agentz_protocol::layout::{Node, PaneId};
use agentz_protocol::spaces::LayoutNode;
use collections::HashSet;
use gpui::{
    App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable, KeyBinding,
    Subscription,
};
use text_input::{TextInput, TextInputEvent};
use ui::{Checkbox, ToggleState, prelude::*};

use crate::app_settings::{AppSettingsStore, SavedLayout};
use crate::controls::AgentIcon;

const KEY_CONTEXT: &str = "SaveLayoutModal";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", menu::Confirm, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(KEY_CONTEXT)),
    ]);
}

/// A pane of the tab being saved, as its header shows it.
#[derive(Clone)]
pub struct LayoutPane {
    pub id: PaneId,
    /// Colored already, as a thread's agent icon can be on its account's color.
    pub icon: AgentIcon,
    pub title: SharedString,
    /// What it runs, to run again when the layout opens. Without one it opens a shell.
    pub command: Option<String>,
}

pub struct SaveLayoutModal {
    root: Node,
    /// In the tree's order, top-left first.
    panes: Vec<LayoutPane>,
    /// The panes whose command runs again.
    kept: HashSet<PaneId>,
    name_input: Entity<TextInput>,
    app_settings: Entity<AppSettingsStore>,
    _subscription: Subscription,
}

impl EventEmitter<DismissEvent> for SaveLayoutModal {}

impl Focusable for SaveLayoutModal {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.name_input.focus_handle(cx)
    }
}

impl SaveLayoutModal {
    pub fn new(
        root: Node,
        panes: Vec<LayoutPane>,
        name: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name_input = cx.new(|cx| TextInput::new("Name", cx));
        if let Some(name) = name {
            name_input.update(cx, |input, cx| input.set_text(name, cx));
        }
        let subscription = cx.subscribe(&name_input, |_, _, _: &TextInputEvent, cx| cx.notify());
        window.focus(&name_input.focus_handle(cx), cx);
        let kept = panes
            .iter()
            .filter(|pane| pane.command.is_some())
            .map(|pane| pane.id)
            .collect();
        Self {
            root,
            panes,
            kept,
            name_input,
            app_settings: AppSettingsStore::global(cx),
            _subscription: subscription,
        }
    }

    fn name(&self, cx: &App) -> String {
        self.name_input.read(cx).text().trim().to_string()
    }

    fn confirm(&mut self, _: &menu::Confirm, _: &mut Window, cx: &mut Context<Self>) {
        let name = self.name(cx);
        if name.is_empty() {
            return;
        }
        let layout = LayoutNode::of(&self.root, &|id| {
            self.panes
                .iter()
                .find(|pane| pane.id == id && self.kept.contains(&id))
                .and_then(|pane| pane.command.clone())
        });
        self.app_settings.update(cx, |store, cx| {
            store.update(
                |settings| settings.save_layout(SavedLayout { name, layout }),
                cx,
            )
        });
        cx.emit(DismissEvent);
    }

    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn set_kept(&mut self, id: PaneId, is_kept: bool, cx: &mut Context<Self>) {
        if is_kept {
            self.kept.insert(id);
        } else {
            self.kept.remove(&id);
        }
        cx.notify();
    }

    fn render_pane(&self, index: usize, pane: &LayoutPane, cx: &mut Context<Self>) -> Div {
        let id = pane.id;
        let is_kept = self.kept.contains(&id);
        h_flex()
            .gap_2()
            .child(
                Checkbox::new(
                    ("save-layout-pane", index),
                    if is_kept {
                        ToggleState::Selected
                    } else {
                        ToggleState::Unselected
                    },
                )
                .disabled(pane.command.is_none())
                .on_click(cx.listener(move |this, state: &ToggleState, _, cx| {
                    this.set_kept(id, *state == ToggleState::Selected, cx)
                })),
            )
            .child(pane.icon.clone().size(IconSize::Small))
            .child(
                div().flex_1().min_w_0().child(
                    Label::new(pane.title.clone())
                        .size(LabelSize::Small)
                        .truncate(),
                ),
            )
            .child(
                Label::new(if is_kept { "runs again" } else { "plain shell" })
                    .size(LabelSize::XSmall)
                    .color(Color::Placeholder),
            )
    }
}

impl Render for SaveLayoutModal {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let border = colors.border;
        let border_variant = colors.border_variant;
        let editor_background = colors.editor_background;
        let has_name = !self.name(cx).is_empty();
        let panes: Vec<Div> = self
            .panes
            .iter()
            .enumerate()
            .map(|(index, pane)| self.render_pane(index, pane, cx))
            .collect();
        v_flex()
            .key_context(KEY_CONTEXT)
            .w(rems(22.))
            .elevation_3(cx)
            .overflow_hidden()
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::cancel))
            .child(
                v_flex()
                    .px_4()
                    .pt_3()
                    .pb_2()
                    .gap_3()
                    .child(Label::new("Save Layout"))
                    .child(
                        div()
                            .h(px(28.))
                            .px_2()
                            .flex()
                            .items_center()
                            .overflow_hidden()
                            .rounded_md()
                            .border_1()
                            .border_color(border)
                            .bg(editor_background)
                            .child(self.name_input.clone()),
                    )
                    .child(v_flex().gap_1().children(panes)),
            )
            .child(
                h_flex()
                    .mt_2()
                    .px_4()
                    .py_2()
                    .gap_2()
                    .justify_end()
                    .border_t_1()
                    .border_color(border_variant)
                    .child(
                        Button::new("save-layout-cancel", "Cancel")
                            .style(ButtonStyle::Subtle)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                    )
                    .child(
                        Button::new("save-layout-save", "Save")
                            .style(ButtonStyle::Filled)
                            .disabled(!has_name)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.confirm(&menu::Confirm, window, cx)
                            })),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use agentz_protocol::layout::Direction;
    use gpui::TestAppContext;

    use super::*;

    fn saved_layouts(cx: &mut gpui::VisualTestContext) -> Vec<SavedLayout> {
        cx.update(|_, cx| {
            AppSettingsStore::global(cx)
                .read(cx)
                .settings()
                .saved_layouts
                .clone()
        })
    }

    #[gpui::test]
    fn a_named_layout_runs_the_commands_it_keeps(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init_for_test(cx);
            init(cx);
        });
        let root = Node::Split {
            direction: Direction::Horizontal,
            ratio: 0.6,
            first: Box::new(Node::Pane(PaneId(1))),
            second: Box::new(Node::Split {
                direction: Direction::Vertical,
                ratio: 0.5,
                first: Box::new(Node::Pane(PaneId(2))),
                second: Box::new(Node::Pane(PaneId(3))),
            }),
        };
        let pane = |id: u64, command: Option<&str>| LayoutPane {
            id: PaneId(id),
            icon: Icon::new(IconName::Terminal).color(Color::Muted).into(),
            title: command.unwrap_or("Shell").to_string().into(),
            command: command.map(str::to_string),
        };
        let panes = vec![
            pane(1, Some("claude")),
            pane(2, None),
            pane(3, Some("npm run dev")),
        ];
        let layout = |third: Option<&str>| LayoutNode::Split {
            direction: Direction::Horizontal,
            ratio: 0.6,
            first: Box::new(LayoutNode::Pane {
                command: Some("claude".to_string()),
            }),
            second: Box::new(LayoutNode::Split {
                direction: Direction::Vertical,
                ratio: 0.5,
                first: Box::new(LayoutNode::Pane { command: None }),
                second: Box::new(LayoutNode::Pane {
                    command: third.map(str::to_string),
                }),
            }),
        };

        let (modal, cx) = cx.add_window_view({
            let root = root.clone();
            let panes = panes.clone();
            move |window, cx| SaveLayoutModal::new(root, panes, None, window, cx)
        });
        let dismissed = Rc::new(Cell::new(0));
        let count_dismissals = |modal: &Entity<SaveLayoutModal>,
                                cx: &mut gpui::VisualTestContext| {
            let dismissed = dismissed.clone();
            cx.update(|_, cx| {
                cx.subscribe(modal, move |_, _: &DismissEvent, _| {
                    dismissed.set(dismissed.get() + 1)
                })
                .detach()
            });
        };
        count_dismissals(&modal, cx);
        // Without a name there's nothing to save it as.
        cx.simulate_keystrokes("enter");
        assert_eq!(dismissed.get(), 0);
        assert!(saved_layouts(cx).is_empty());
        cx.simulate_input("dev");
        cx.simulate_keystrokes("enter");
        assert_eq!(dismissed.get(), 1);
        assert_eq!(
            saved_layouts(cx),
            [SavedLayout {
                name: "dev".to_string(),
                layout: layout(Some("npm run dev")),
            }]
        );

        // Saved again under its name, without npm, it replaces the first.
        let modal = cx.new_window_entity(move |window, cx| {
            SaveLayoutModal::new(root, panes, Some("dev".to_string()), window, cx)
        });
        count_dismissals(&modal, cx);
        modal.update_in(cx, |modal, window, cx| {
            modal.set_kept(PaneId(3), false, cx);
            modal.confirm(&menu::Confirm, window, cx);
        });
        cx.run_until_parked();
        assert_eq!(dismissed.get(), 2);
        assert_eq!(
            saved_layouts(cx),
            [SavedLayout {
                name: "dev".to_string(),
                layout: layout(None),
            }]
        );
    }
}
