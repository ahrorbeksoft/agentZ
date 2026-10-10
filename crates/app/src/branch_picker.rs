//! The branches a new worktree or pasture can start from (t3code's branch picker): a search,
//! the local branches and then origin's, each with its mark, and a Fetch for origin's. New
//! Thread's "From main" and the Workspaces view's New Worktree open it.

use std::rc::Rc;

use agentz_protocol::workspace::{GitBranch, ProjectGit};
use anyhow::Result;
use gpui::{
    AnyElement, App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    KeyBinding, ScrollHandle, Subscription, Task, Window,
};
use text_input::{TextInput, TextInputEvent};
use ui::{
    ButtonLike, CommonAnimationExt as _, Divider, HighlightedLabel, ListItem, ListItemSpacing,
    Tooltip, WithScrollbar as _, prelude::*,
};

use crate::project_switcher::fuzzy_match;

const KEY_CONTEXT: &str = "BranchPicker";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", menu::SelectPrevious, Some(KEY_CONTEXT)),
        KeyBinding::new("down", menu::SelectNext, Some(KEY_CONTEXT)),
        KeyBinding::new("enter", menu::Confirm, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(KEY_CONTEXT)),
    ]);
}

/// `git fetch origin`, resolving to the repository's branches after it.
pub type FetchOrigin = Rc<dyn Fn(&mut App) -> Task<Result<ProjectGit>>>;

#[derive(Clone)]
struct BranchEntry {
    branch: GitBranch,
    mark: Option<&'static str>,
    positions: Vec<usize>,
}

pub struct BranchPicker {
    git: ProjectGit,
    /// The branch the new one starts from now.
    chosen: String,
    fetch: Option<FetchOrigin>,
    on_choose: Rc<dyn Fn(String, &mut Window, &mut App)>,
    search: Entity<TextInput>,
    entries: Vec<BranchEntry>,
    selected_index: usize,
    scroll_handle: ScrollHandle,
    fetching: Option<Task<()>>,
    fetch_error: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<DismissEvent> for BranchPicker {}

impl Focusable for BranchPicker {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.focus_handle(cx)
    }
}

impl BranchPicker {
    /// `fetch` is offered when the repository has an origin.
    pub fn new(
        git: ProjectGit,
        chosen: String,
        fetch: Option<FetchOrigin>,
        on_choose: impl Fn(String, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| TextInput::new("Search branches…", cx));
        let subscriptions = vec![cx.subscribe(&search, |this, _, _: &TextInputEvent, cx| {
            this.selected_index = 0;
            this.update_entries(cx)
        })];
        window.focus(&search.focus_handle(cx), cx);
        let mut this = Self {
            fetch: fetch.filter(|_| git.has_origin),
            git,
            chosen,
            on_choose: Rc::new(on_choose),
            search,
            entries: Vec::new(),
            selected_index: 0,
            scroll_handle: ScrollHandle::new(),
            fetching: None,
            fetch_error: None,
            _subscriptions: subscriptions,
        };
        this.update_entries(cx);
        this.selected_index = this
            .entries
            .iter()
            .position(|entry| entry.branch.name == this.chosen)
            .unwrap_or_default();
        this
    }

    fn update_entries(&mut self, cx: &mut Context<Self>) {
        let query = self.search.read(cx).text().trim().to_lowercase();
        self.entries = self
            .git
            .branches
            .iter()
            .filter_map(|branch| {
                let positions = fuzzy_match(&query, &branch.name)?;
                Some(BranchEntry {
                    mark: self.git.mark(branch),
                    branch: branch.clone(),
                    positions,
                })
            })
            .collect();
        self.selected_index = self
            .selected_index
            .min(self.entries.len().saturating_sub(1));
        cx.notify();
    }

    fn fetch(&mut self, cx: &mut Context<Self>) {
        let Some(fetch) = self.fetch.clone() else {
            return;
        };
        if self.fetching.is_some() {
            return;
        }
        let fetched = fetch(cx);
        self.fetch_error = None;
        self.fetching = Some(cx.spawn(async move |this, cx| {
            let fetched = fetched.await;
            this.update(cx, |this, cx| {
                this.fetching = None;
                match fetched {
                    Ok(git) => {
                        this.git = git;
                        this.update_entries(cx);
                    }
                    Err(error) => {
                        this.fetch_error = Some(format!("{error:#}").into());
                        cx.notify();
                    }
                }
            })
            .ok();
        }));
        cx.notify();
    }

    /// Where origin's branches start, after a divider.
    fn starts_remotes(&self, index: usize) -> bool {
        index > 0
            && self
                .entries
                .get(index)
                .is_some_and(|entry| entry.branch.is_remote)
            && self
                .entries
                .get(index - 1)
                .is_some_and(|entry| !entry.branch.is_remote)
    }

    fn scroll_to_selection(&self) {
        let dividers = (0..=self.selected_index)
            .filter(|index| self.starts_remotes(*index))
            .count();
        self.scroll_handle
            .scroll_to_item(self.selected_index + dividers);
    }

    fn select_next(&mut self, _: &menu::SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        if !self.entries.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.entries.len();
            self.scroll_to_selection();
            cx.notify();
        }
    }

    fn select_previous(
        &mut self,
        _: &menu::SelectPrevious,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.entries.is_empty() {
            self.selected_index = self
                .selected_index
                .checked_sub(1)
                .unwrap_or(self.entries.len() - 1);
            self.scroll_to_selection();
            cx.notify();
        }
    }

    fn confirm(&mut self, _: &menu::Confirm, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(entry) = self.entries.get(self.selected_index) {
            let name = entry.branch.name.clone();
            self.choose(name, window, cx);
        }
    }

    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn choose(&mut self, branch: String, window: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
        (self.on_choose)(branch, window, cx);
    }

    fn render_entry(&self, index: usize, entry: BranchEntry, cx: &mut Context<Self>) -> AnyElement {
        let is_chosen = entry.branch.name == self.chosen;
        let name = entry.branch.name.clone();
        let selector = format!("branch-{name}");
        let item = ListItem::new(("branch-picker-entry", index))
            .inset(true)
            .spacing(ListItemSpacing::Sparse)
            .toggle_state(index == self.selected_index)
            .start_slot(
                Icon::new(IconName::GitBranch)
                    .size(IconSize::Small)
                    .color(Color::Muted),
            )
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .child(HighlightedLabel::new(entry.branch.name, entry.positions).truncate()),
            )
            .end_slot(
                h_flex()
                    .gap_1()
                    .children(entry.mark.map(|mark| {
                        Label::new(mark)
                            .size(LabelSize::XSmall)
                            .color(Color::Placeholder)
                    }))
                    .child(div().w(IconSize::Small.rems()).when(is_chosen, |this| {
                        this.child(
                            Icon::new(IconName::Check)
                                .size(IconSize::Small)
                                .color(Color::Accent),
                        )
                    })),
            )
            .on_click(
                cx.listener(move |this, _, window, cx| this.choose(name.clone(), window, cx)),
            );
        div()
            .debug_selector(move || selector)
            .child(item)
            .into_any_element()
    }

    fn render_head(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border_variant = cx.theme().colors().border_variant;
        let is_fetching = self.fetching.is_some();
        h_flex()
            .px_3()
            .py_1()
            .gap_2()
            .border_b_1()
            .border_color(border_variant)
            .child(
                div().flex_1().child(
                    Label::new("Start from")
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                ),
            )
            .when(self.fetch.is_some(), |head| {
                let icon = Icon::new(IconName::ArrowCircle)
                    .size(IconSize::XSmall)
                    .color(Color::Muted);
                // Zed's Fetch: its icon turns while it runs.
                head.child(
                    div().debug_selector(|| "branch-picker-fetch".into()).child(
                        ButtonLike::new("branch-picker-fetch")
                            .size(ButtonSize::Compact)
                            .disabled(is_fetching)
                            .child(
                                h_flex()
                                    .gap_1()
                                    .child(if is_fetching {
                                        icon.with_rotate_animation(2).into_any_element()
                                    } else {
                                        icon.into_any_element()
                                    })
                                    .child(Label::new("Fetch").size(LabelSize::Small)),
                            )
                            .tooltip(Tooltip::text("Fetch origin's branches"))
                            .on_click(cx.listener(|this, _, _, cx| this.fetch(cx))),
                    ),
                )
            })
    }
}

impl Render for BranchPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border_variant = cx.theme().colors().border_variant;
        let mut rows = Vec::with_capacity(self.entries.len() + 1);
        for (index, entry) in self.entries.clone().into_iter().enumerate() {
            if self.starts_remotes(index) {
                rows.push(
                    div()
                        .debug_selector(|| "branch-picker-remotes".into())
                        .py_1()
                        .child(Divider::horizontal())
                        .into_any_element(),
                );
            }
            rows.push(self.render_entry(index, entry, cx));
        }

        v_flex()
            .key_context(KEY_CONTEXT)
            .debug_selector(|| "branch-picker".into())
            .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(DismissEvent)))
            .w(rems(20.))
            .elevation_3(cx)
            .overflow_hidden()
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::cancel))
            .child(self.render_head(cx))
            .child(
                h_flex()
                    .px_3()
                    .py_1p5()
                    .gap_2()
                    .border_b_1()
                    .border_color(border_variant)
                    .child(
                        Icon::new(IconName::MagnifyingGlass)
                            .size(IconSize::Small)
                            .color(Color::Muted),
                    )
                    .child(self.search.clone()),
            )
            .child(
                div()
                    .id("branch-picker-scroll")
                    .child(
                        v_flex()
                            .id("branch-picker-entries")
                            .max_h(rems(20.))
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll_handle)
                            .p_1()
                            .children(rows)
                            .when(self.entries.is_empty(), |list| {
                                list.child(
                                    div().px_2().py_1p5().child(
                                        Label::new("No matching branches")
                                            .size(LabelSize::Small)
                                            .color(Color::Muted),
                                    ),
                                )
                            }),
                    )
                    .vertical_scrollbar_for(&self.scroll_handle, window, cx),
            )
            .children(self.fetch_error.clone().map(|error| {
                div()
                    .debug_selector(|| "branch-picker-fetch-error".into())
                    .px_3()
                    .py_1p5()
                    .border_t_1()
                    .border_color(border_variant)
                    .child(Label::new(error).size(LabelSize::Small).color(Color::Error))
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{TestAppContext, VisualTestContext};
    use std::cell::{Cell, RefCell};

    fn branch(name: &str, is_remote: bool, is_checked_out: bool) -> GitBranch {
        GitBranch {
            name: name.into(),
            is_remote,
            is_checked_out,
        }
    }

    fn git() -> ProjectGit {
        ProjectGit {
            is_repository: true,
            branch: Some("login-fix".into()),
            default_branch: Some("main".into()),
            has_origin: true,
            branches: vec![
                branch("login-fix", false, true),
                branch("main", false, false),
                branch("agentz/docs", false, true),
                branch("spike", false, false),
                branch("origin/main", true, false),
            ],
            ..Default::default()
        }
    }

    fn rows(
        picker: &Entity<BranchPicker>,
        cx: &mut VisualTestContext,
    ) -> Vec<(String, Option<&'static str>)> {
        picker.read_with(cx, |picker, _| {
            picker
                .entries
                .iter()
                .map(|entry| (entry.branch.name.clone(), entry.mark))
                .collect()
        })
    }

    fn click(selector: &'static str, cx: &mut VisualTestContext) {
        let bounds = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} is shown"));
        cx.simulate_click(bounds.center(), gpui::Modifiers::none());
        cx.run_until_parked();
    }

    /// Local branches come first, each with t3code's mark, then origin's after a divider;
    /// the search narrows them, and Fetch brings origin's new ones, or says why it couldn't.
    #[gpui::test]
    fn origins_branches_follow_the_local_ones_and_fetch_adds_theirs(cx: &mut TestAppContext) {
        cx.update(crate::init_for_test);
        let chosen: Rc<RefCell<Vec<String>>> = Rc::default();
        let fetches = Rc::new(Cell::new(0));
        let fetch: FetchOrigin = {
            let fetches = fetches.clone();
            Rc::new(move |_: &mut App| {
                fetches.set(fetches.get() + 1);
                if fetches.get() == 1 {
                    return Task::ready(Err(anyhow::anyhow!("Could not resolve host")));
                }
                let mut fetched = git();
                fetched
                    .branches
                    .push(branch("origin/teammate", true, false));
                Task::ready(Ok(fetched))
            })
        };
        let (picker, cx) = cx.add_window_view(|window, cx| {
            let chosen = chosen.clone();
            BranchPicker::new(
                git(),
                "main".into(),
                Some(fetch),
                move |branch, _, _| chosen.borrow_mut().push(branch),
                window,
                cx,
            )
        });
        cx.run_until_parked();
        let named = |rows: &[(&str, Option<&'static str>)]| {
            rows.iter()
                .map(|(name, mark)| (name.to_string(), *mark))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            rows(&picker, cx),
            named(&[
                ("login-fix", Some("current")),
                ("main", Some("default")),
                ("agentz/docs", Some("worktree")),
                ("spike", None),
                ("origin/main", Some("remote")),
            ])
        );
        // The base chosen now is selected.
        assert_eq!(picker.read_with(cx, |picker, _| picker.selected_index), 1);
        let spike = cx.debug_bounds("branch-spike").expect("spike is listed");
        let divider = cx
            .debug_bounds("branch-picker-remotes")
            .expect("a divider before origin's");
        let remote = cx
            .debug_bounds("branch-origin/main")
            .expect("origin/main is listed");
        assert!(spike.bottom() <= divider.top() && divider.bottom() <= remote.top());

        cx.simulate_input("main");
        assert_eq!(
            rows(&picker, cx),
            named(&[("main", Some("default")), ("origin/main", Some("remote"))])
        );
        picker.update_in(cx, |picker, window, cx| {
            picker
                .search
                .update(cx, |search, cx| search.set_text("", cx));
            picker.update_entries(cx);
            window.focus(&picker.search.focus_handle(cx), cx);
        });

        click("branch-picker-fetch", cx);
        assert!(cx.debug_bounds("branch-picker-fetch-error").is_some());
        assert!(cx.debug_bounds("branch-origin/teammate").is_none());
        click("branch-picker-fetch", cx);
        assert_eq!(fetches.get(), 2);
        assert!(cx.debug_bounds("branch-picker-fetch-error").is_none());
        click("branch-origin/teammate", cx);
        assert_eq!(*chosen.borrow(), vec!["origin/teammate".to_string()]);
    }

    /// Without an origin there's nothing to fetch, so Fetch isn't offered.
    #[gpui::test]
    fn fetch_is_offered_only_with_an_origin(cx: &mut TestAppContext) {
        cx.update(crate::init_for_test);
        let fetch: FetchOrigin = Rc::new(|_: &mut App| Task::ready(Ok(git())));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let git = ProjectGit {
                has_origin: false,
                ..git()
            };
            BranchPicker::new(git, "main".into(), Some(fetch), |_, _, _| {}, window, cx)
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("branch-picker-fetch").is_none());
        assert!(cx.debug_bounds("branch-main").is_some());
    }
}
