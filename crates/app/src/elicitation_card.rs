//! A request for input from an agent (ACP's `elicitation/create`): a form to fill in, or a URL
//! to open. The fields, validation and the URL's safety checks are ported from Zed's
//! `agent_ui::conversation_view::elicitation`; the card is agentZ's: who's asking, the fields
//! with their labels above them, and Decline beside Submit or Open
//! (`design/agent-input/decisions.md`). An agent's own questions show one at a time.

use std::collections::BTreeMap;

use agent_client_protocol::schema::v1 as acp;
use agentz_protocol::thread::{Elicitation, form_questions, recommended_choice};
use collections::{HashMap, HashSet};
use gpui::{
    AnyElement, App, Context, Entity, Focusable as _, KeyBinding, SharedString, Subscription,
    Window, div,
};
use text_input::TextInput;
use ui::{Tooltip, prelude::*};

use crate::agent_login::login_elicitation;
use crate::controls::{
    ActionButton, ActionStyle, CONTROL_TEXT_SIZE, copy_to_clipboard, field_label, key_hint,
    link_host, on_fill_color, spinner, text_field,
};
use crate::thread_entity::AgentThread;

const KEY_CONTEXT: &str = "ElicitationForm";
const MIN_URL_DISPLAY_SEGMENT_CHARS: usize = 16;
const MAX_URL_DISPLAY_SEGMENT_CHARS: usize = 64;

pub fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("enter", menu::Confirm, Some(KEY_CONTEXT))]);
}

/// Keeps one card per elicitation the thread is waiting on, in its order, keeping what the
/// user has typed in the ones still open. The page a login asks to open shows in the login
/// panel instead.
pub(crate) fn sync_elicitation_cards(
    cards: &mut Vec<Entity<ElicitationCard>>,
    thread: &Entity<AgentThread>,
    cx: &mut App,
) {
    let login_request = login_elicitation(thread.read(cx)).map(|elicitation| elicitation.id);
    let elicitations: Vec<Elicitation> = thread
        .read(cx)
        .elicitations()
        .iter()
        .filter(|elicitation| Some(elicitation.id) != login_request)
        .cloned()
        .collect();
    let requester_name = thread.read(cx).agent_name().clone();
    let mut previous: HashMap<u64, Entity<ElicitationCard>> = cards
        .drain(..)
        .map(|card| (card.read(cx).elicitation.id, card))
        .collect();
    for elicitation in elicitations {
        let card = match previous.remove(&elicitation.id) {
            Some(card) => {
                card.update(cx, |card, cx| {
                    if card.elicitation != elicitation {
                        card.elicitation = elicitation;
                        cx.notify();
                    }
                });
                card
            }
            None => {
                let thread = thread.clone();
                let requester_name = requester_name.clone();
                cx.new(|cx| ElicitationCard::new(thread, elicitation, requester_name, cx))
            }
        };
        cards.push(card);
    }
}

pub struct ElicitationCard {
    thread: Entity<AgentThread>,
    elicitation: Elicitation,
    requester_name: SharedString,
    form: Option<FormState>,
    /// The agent's own questions (Claude Agent's), each a field with choices and the field
    /// for an answer of the user's own: `(field, other)`.
    questions: Option<Vec<(String, String)>>,
    /// The question shown, of several.
    step: usize,
    /// The "Other" fields opened from their folded line.
    opened_others: HashSet<String>,
    /// The URL's whole address, folded behind its host.
    shows_address: bool,
    /// Text fields check what's in them as they lose focus, from the first render on.
    _blur_subscriptions: Option<Vec<Subscription>>,
}

impl ElicitationCard {
    fn new(
        thread: Entity<AgentThread>,
        elicitation: Elicitation,
        requester_name: SharedString,
        cx: &mut Context<Self>,
    ) -> Self {
        let (form, questions) = match &elicitation.request.mode {
            acp::ElicitationMode::Form(mode) => {
                let schema = &mode.requested_schema;
                let questions = form_questions(schema).map(|questions| {
                    questions
                        .into_iter()
                        .map(|question| (question.field.to_string(), question.other.to_string()))
                        .collect()
                });
                (Some(FormState::new(schema, cx)), questions)
            }
            _ => (None, None),
        };
        Self {
            thread,
            elicitation,
            requester_name,
            form,
            questions,
            step: 0,
            opened_others: HashSet::default(),
            shows_address: false,
            _blur_subscriptions: None,
        }
    }

    /// Whether the agent waits on it: not yet answered, or a page not yet opened.
    pub fn is_waiting(&self) -> bool {
        !self.elicitation.opened
    }

    /// Whether it asks the agent's own questions for this tool call, and they wait for an
    /// answer.
    pub fn asks_questions_for(&self, tool_call_id: &acp::ToolCallId) -> bool {
        self.is_waiting()
            && self.questions.is_some()
            && self.elicitation.tool_call_id() == Some(tool_call_id)
    }

    /// Whether it belongs to a request rather than the conversation: a login asking for a
    /// code, say. Zed shows those beside the composer instead of in the thread.
    pub fn is_for_request(&self) -> bool {
        matches!(
            self.elicitation.request.scope(),
            acp::ElicitationScope::Request(_)
        )
    }

    fn respond(&mut self, action: acp::ElicitationAction, cx: &mut Context<Self>) {
        let id = self.elicitation.id;
        self.thread.update(cx, |thread, cx| {
            thread.respond_to_elicitation(id, action, cx)
        });
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let (Some(form), acp::ElicitationMode::Form(mode)) =
            (&mut self.form, &self.elicitation.request.mode)
        else {
            return;
        };
        match form.validate(&mode.requested_schema, cx) {
            Ok(content) => self.respond(
                acp::ElicitationAction::Accept(
                    acp::ElicitationAcceptAction::new().content(content),
                ),
                cx,
            ),
            Err(errors) => {
                // Back to the first question with a problem, when it's not the one shown.
                if let Some(step) = self.questions.as_ref().and_then(|questions| {
                    questions.iter().position(|(field, other)| {
                        errors.contains_key(field) || errors.contains_key(other)
                    })
                }) {
                    self.step = step;
                }
                form.errors = errors;
                cx.notify();
            }
        }
    }

    /// Whether the question shown is the last, or the form isn't asked a question at a time.
    fn is_last_step(&self) -> bool {
        self.questions
            .as_ref()
            .is_none_or(|questions| self.step + 1 >= questions.len())
    }

    /// Enter and the foot's button: the next question, or Submit on the last.
    fn confirm(&mut self, cx: &mut Context<Self>) {
        if self.is_last_step() {
            self.submit(cx);
        } else {
            self.step += 1;
            cx.notify();
        }
    }

    /// Checks a text field as it loses focus, so a wrong value says why before Submit. An
    /// empty one waits for Submit to say it's required.
    fn check_field(&mut self, name: &str, cx: &mut Context<Self>) {
        let (Some(form), acp::ElicitationMode::Form(mode)) =
            (&mut self.form, &self.elicitation.request.mode)
        else {
            return;
        };
        let is_empty = matches!(
            form.fields.get(name),
            Some(Field::Text(input)) if input.read(cx).text().trim().is_empty()
        );
        if is_empty {
            return;
        }
        let error = form
            .validate(&mode.requested_schema, cx)
            .err()
            .and_then(|mut errors| errors.remove(name));
        match error {
            Some(error) => {
                form.errors.insert(name.to_string(), error);
            }
            None => {
                form.errors.remove(name);
            }
        }
        cx.notify();
    }

    fn watch_blurs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self._blur_subscriptions.is_some() {
            return;
        }
        let inputs: Vec<(String, Entity<TextInput>)> = self
            .form
            .iter()
            .flat_map(|form| &form.fields)
            .filter_map(|(name, field)| match field {
                Field::Text(input) => Some((name.clone(), input.clone())),
                _ => None,
            })
            .collect();
        let subscriptions = inputs
            .into_iter()
            .map(|(name, input)| {
                cx.on_blur(&input.focus_handle(cx), window, move |this, _, cx| {
                    this.check_field(&name, cx)
                })
            })
            .collect();
        self._blur_subscriptions = Some(subscriptions);
    }

    /// Zed's Open: opening the URL accepts it, and the card waits for the agent to say the
    /// user finished there.
    fn open(&mut self, cx: &mut Context<Self>) {
        let Some(url) = self.elicitation.url() else {
            return;
        };
        cx.open_url(url);
        if !self.elicitation.opened {
            self.respond(
                acp::ElicitationAction::Accept(acp::ElicitationAcceptAction::new()),
                cx,
            );
        }
    }

    fn dismiss(&mut self, cx: &mut Context<Self>) {
        let id = self.elicitation.id;
        self.thread
            .update(cx, |thread, cx| thread.dismiss_elicitation(id, cx));
    }

    fn render_form(
        &self,
        mode: &acp::ElicitationFormMode,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(form) = &self.form else {
            return div().into_any_element();
        };
        let schema = &mode.requested_schema;
        if let Some(questions) = &self.questions {
            return self.render_question(schema, form, questions, window, cx);
        }
        v_flex()
            .gap_4()
            .when(
                schema.title.is_some() || schema.description.is_some(),
                |this| {
                    this.child(
                        v_flex()
                            .gap_0p5()
                            .when_some(schema.title.clone(), |this, title| {
                                this.child(Label::new(title).size(LabelSize::Small))
                            })
                            .when_some(schema.description.clone(), |this, description| {
                                this.child(
                                    Label::new(description)
                                        .size(LabelSize::Small)
                                        .color(Color::Muted),
                                )
                            }),
                    )
                },
            )
            .children(schema.properties.iter().filter_map(|(name, property)| {
                let field = form.fields.get(name)?;
                let is_required = schema
                    .required
                    .as_deref()
                    .unwrap_or_default()
                    .contains(name);
                Some(self.render_field(
                    name,
                    property,
                    field,
                    is_required,
                    form.errors.get(name),
                    window,
                    cx,
                ))
            }))
            .into_any_element()
    }

    /// One of the agent's own questions: which it is of several, with Back, then the
    /// question, its header, its choices, and "Other…" folded until it's clicked.
    fn render_question(
        &self,
        schema: &acp::ElicitationSchema,
        form: &FormState,
        questions: &[(String, String)],
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = self.elicitation.id;
        let Some((name, other)) = questions.get(self.step) else {
            return div().into_any_element();
        };
        let (Some(property), Some(field)) = (schema.properties.get(name), form.fields.get(name))
        else {
            return div().into_any_element();
        };
        let count = questions.len();
        let error = form.errors.get(name);
        let stepper = (count > 1).then(|| {
            h_flex()
                .gap_2()
                .when(self.step > 0, |this| {
                    this.child(
                        h_flex()
                            .id(("elicitation-back", id))
                            .debug_selector(move || format!("elicitation-back-{id}"))
                            .gap_1()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.step = this.step.saturating_sub(1);
                                cx.notify();
                            }))
                            .child(
                                Icon::new(IconName::ChevronLeft)
                                    .size(IconSize::XSmall)
                                    .color(Color::Muted),
                            )
                            .child(
                                Label::new("Back")
                                    .size(LabelSize::Small)
                                    .color(Color::Muted),
                            ),
                    )
                })
                .child(div().flex_1())
                .child(
                    div()
                        .debug_selector(move || format!("elicitation-step-{id}"))
                        .child(
                            Label::new(format!("{} of {count}", self.step + 1))
                                .size(LabelSize::Small)
                                .color(Color::Muted),
                        ),
                )
        });
        // Of several, each question is its field's description; one alone is the message.
        let question = (count > 1)
            .then(|| property_description(property))
            .flatten()
            .map(|question| {
                Label::new(question).size(LabelSize::Custom(rems_from_px(CONTROL_TEXT_SIZE)))
            });
        v_flex()
            .gap_3()
            .children(stepper)
            .children(question)
            .child(
                v_flex()
                    .gap_1p5()
                    .child(field_label(property_title(name, property)))
                    .child(self.render_control(name, property, field, error, window, cx))
                    .children(error.map(|error| {
                        Label::new(error.clone())
                            .size(LabelSize::Small)
                            .color(Color::Error)
                    })),
            )
            .children(self.render_other(schema, form, other, window, cx))
            .into_any_element()
    }

    /// The field for an answer of the user's own: a folded "Other…" line until it's clicked
    /// or has something in it.
    fn render_other(
        &self,
        schema: &acp::ElicitationSchema,
        form: &FormState,
        name: &str,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let id = self.elicitation.id;
        let property = schema.properties.get(name)?;
        let Some(Field::Text(input)) = form.fields.get(name) else {
            return None;
        };
        let error = form.errors.get(name);
        let is_open = self.opened_others.contains(name)
            || !input.read(cx).text().is_empty()
            || error.is_some();
        let title = property_title(name, property);
        if !is_open {
            let name = name.to_string();
            let input = input.clone();
            return Some(
                h_flex()
                    .id(SharedString::from(format!("elicitation-other-{id}-{name}")))
                    .debug_selector({
                        let name = name.clone();
                        move || format!("elicitation-other-{id}-{name}")
                    })
                    .gap_1()
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.opened_others.insert(name.clone());
                        window.focus(&input.focus_handle(cx), cx);
                        cx.notify();
                    }))
                    .child(
                        Icon::new(IconName::Plus)
                            .size(IconSize::XSmall)
                            .color(Color::Muted),
                    )
                    .child(
                        Label::new(format!("{title}…"))
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    )
                    .into_any_element(),
            );
        }
        Some(
            v_flex()
                .gap_1p5()
                .child(field_label(title))
                .children(property_description(property).map(|description| {
                    Label::new(description)
                        .size(LabelSize::Small)
                        .color(Color::Muted)
                }))
                .child(text_field(input, error.is_some(), window, cx))
                .children(error.map(|error| {
                    Label::new(error.clone())
                        .size(LabelSize::Small)
                        .color(Color::Error)
                }))
                .into_any_element(),
        )
    }

    fn render_field(
        &self,
        name: &str,
        property: &acp::ElicitationPropertySchema,
        field: &Field,
        is_required: bool,
        error: Option<&SharedString>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = self.elicitation.id;
        let title = property_title(name, property);
        let description = property_description(property).map(|description| {
            Label::new(description)
                .size(LabelSize::Small)
                .color(Color::Muted)
        });
        let error_label = error.map(|error| {
            let selector = format!("elicitation-error-{id}-{name}");
            div().debug_selector(move || selector).child(
                Label::new(error.clone())
                    .size(LabelSize::Small)
                    .color(Color::Error),
            )
        });
        let control = {
            let selector = format!("elicitation-field-{id}-{name}");
            div()
                .debug_selector(move || selector)
                .child(self.render_control(name, property, field, error, window, cx))
        };
        // A checkbox has its label beside it.
        let label = (!matches!(field, Field::Boolean(_))).then(|| {
            h_flex()
                .child(field_label(title))
                .when(is_required, |this| this.child(required_mark(name)))
        });
        v_flex()
            .gap_1p5()
            .children(label)
            .children(description)
            .child(control)
            .children(error_label)
            .into_any_element()
    }

    /// A field's control: a checkbox with its label, a text field, or its choices. A choice of
    /// one of the agent's questions goes on to the next question.
    fn render_control(
        &self,
        name: &str,
        property: &acp::ElicitationPropertySchema,
        field: &Field,
        error: Option<&SharedString>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = self.elicitation.id;
        match field {
            Field::Boolean(value) => {
                let next_value = !*value;
                let field_name = name.to_string();
                let is_required = match &self.elicitation.request.mode {
                    acp::ElicitationMode::Form(mode) => mode
                        .requested_schema
                        .required
                        .as_deref()
                        .unwrap_or_default()
                        .iter()
                        .any(|required| required == name),
                    _ => false,
                };
                h_flex()
                    .id(SharedString::from(format!("elicitation-bool-{id}-{name}")))
                    .gap_2()
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(form) = &mut this.form {
                            form.set(&field_name, Field::Boolean(next_value));
                            cx.notify();
                        }
                    }))
                    .child(check_box(*value, error.is_some(), cx))
                    .child(
                        Label::new(property_title(name, property))
                            .size(LabelSize::Custom(rems_from_px(CONTROL_TEXT_SIZE))),
                    )
                    .when(is_required, |this| this.child(required_mark(name)))
                    .into_any_element()
            }
            Field::Text(input) => text_field(input, error.is_some(), window, cx).into_any_element(),
            Field::SingleSelect(selected) => {
                let options = match property {
                    acp::ElicitationPropertySchema::String(schema) => single_select_options(schema),
                    _ => Vec::new(),
                };
                choice_group(&options)
                    .children(options.iter().map(|option| {
                        let is_selected = selected.as_ref() == Some(&option.value);
                        let field_name = name.to_string();
                        let value = option.value.clone();
                        choice(
                            SharedString::from(format!(
                                "elicitation-select-{id}-{name}-{}",
                                option.value
                            )),
                            radio(is_selected, error.is_some(), cx),
                            option,
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(form) = &mut this.form {
                                form.set(&field_name, Field::SingleSelect(Some(value.clone())));
                            }
                            let is_question = this.questions.as_ref().is_some_and(|questions| {
                                questions.iter().any(|(field, _)| field == &field_name)
                            });
                            if is_question && !this.is_last_step() {
                                this.step += 1;
                            }
                            cx.notify();
                        }))
                    }))
                    .into_any_element()
            }
            Field::MultiSelect(selected) => {
                let options = match property {
                    acp::ElicitationPropertySchema::Array(schema) => multi_select_options(schema),
                    _ => Vec::new(),
                };
                choice_group(&options)
                    .children(options.iter().map(|option| {
                        let is_selected = selected.contains(&option.value);
                        let field_name = name.to_string();
                        let value = option.value.clone();
                        choice(
                            SharedString::from(format!(
                                "elicitation-multi-{id}-{name}-{}",
                                option.value
                            )),
                            check_box(is_selected, error.is_some(), cx),
                            option,
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(form) = &mut this.form {
                                form.toggle(&field_name, &value);
                                cx.notify();
                            }
                        }))
                    }))
                    .into_any_element()
            }
        }
    }

    /// Where the page is, so it can be checked before opening it: its host, large, a warning
    /// when an internationalized address could imitate another, and the whole address, wrapped,
    /// behind "Show address".
    fn render_url(&self, url: &str, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors();
        let id = self.elicitation.id;
        let shows_address = self.shows_address;
        v_flex()
            .gap_2()
            .when_some(url_host(url), |this, (host, decoded_host)| {
                this.child(
                    h_flex()
                        .debug_selector(move || format!("elicitation-host-{id}"))
                        .gap_2()
                        .child(
                            Icon::new(IconName::Lock)
                                .size(IconSize::Small)
                                .color(Color::Success),
                        )
                        .child(Label::new(host).size(LabelSize::Large)),
                )
                .when_some(decoded_host, |this, decoded_host| {
                    this.child(
                        h_flex()
                            .items_start()
                            .gap_1p5()
                            .child(
                                Icon::new(IconName::Warning)
                                    .size(IconSize::XSmall)
                                    .color(Color::Warning),
                            )
                            .child(
                                Label::new(format!(
                                    "This internationalized address displays as \
                                     {decoded_host}. Verify it carefully."
                                ))
                                .size(LabelSize::Small)
                                .color(Color::Warning),
                            ),
                    )
                })
            })
            .child(
                h_flex().child(
                    h_flex()
                        .id(("elicitation-show-address", id))
                        .debug_selector(move || format!("elicitation-show-address-{id}"))
                        .gap_1()
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.shows_address = !this.shows_address;
                            cx.notify();
                        }))
                        .child(
                            Label::new(if shows_address {
                                "Hide address"
                            } else {
                                "Show address"
                            })
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                        )
                        .child(
                            Icon::new(if shows_address {
                                IconName::ChevronUp
                            } else {
                                IconName::ChevronDown
                            })
                            .size(IconSize::XSmall)
                            .color(Color::Muted),
                        ),
                ),
            )
            .when(shows_address, |this| {
                this.child(
                    h_flex()
                        .debug_selector(move || format!("elicitation-address-{id}"))
                        .min_w_0()
                        .flex_wrap()
                        .px(px(10.))
                        .py(px(6.))
                        .rounded(px(6.))
                        .border_1()
                        .border_color(colors.border_variant)
                        .bg(colors.editor_background)
                        .font_buffer(cx)
                        .text_size(rems_from_px(12_f32))
                        .text_color(colors.text_muted)
                        .children(display_url_segments(url)),
                )
            })
            .into_any_element()
    }

    /// Submit or Open, with Decline beside it. Once the page is open, the card waits for the
    /// agent to say the user finished there.
    fn render_footer(&self, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors();
        let id = self.elicitation.id;
        let url = self.elicitation.url().map(str::to_string);
        let footer = h_flex()
            .px(px(14.))
            .py(px(10.))
            .gap_2()
            .border_t_1()
            .border_color(colors.border_variant);
        let decline = ActionButton::new(("elicitation-decline", id), "Decline")
            .style(ActionStyle::Ghost)
            .on_click(
                cx.listener(|this, _, _, cx| this.respond(acp::ElicitationAction::Decline, cx)),
            );
        let Some(url) = url else {
            let is_last_step = self.is_last_step();
            // Submit stays dim until every required field has something in it.
            let lacks_required = match (&self.form, &self.elicitation.request.mode) {
                (Some(form), acp::ElicitationMode::Form(mode)) => {
                    form.lacks_required(&mode.requested_schema, cx)
                }
                _ => false,
            };
            return footer
                .child(h_flex().flex_1().gap_1().when(is_last_step, |this| {
                    this.child(key_hint("⏎", cx)).child(
                        Label::new("to submit")
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    )
                }))
                .child(decline)
                .child(
                    div()
                        .debug_selector(move || format!("elicitation-submit-{id}"))
                        .child(if is_last_step {
                            ActionButton::new(("elicitation-submit", id), "Submit")
                                .style(ActionStyle::Primary)
                                .disabled(lacks_required)
                                .on_click(cx.listener(|this, _, _, cx| this.submit(cx)))
                        } else {
                            ActionButton::new(("elicitation-submit", id), "Next")
                                .style(ActionStyle::Primary)
                                .on_click(cx.listener(|this, _, _, cx| this.confirm(cx)))
                        }),
                )
                .into_any_element();
        };
        if self.elicitation.opened {
            return footer
                .child(
                    h_flex()
                        .flex_1()
                        .gap_1p5()
                        .child(spinner(Color::Muted))
                        .child(
                            Label::new("Waiting for you to finish in your browser")
                                .size(LabelSize::Small)
                                .color(Color::Muted),
                        ),
                )
                .child(
                    ActionButton::new(("elicitation-open", id), "Open Again")
                        .end_icon(Icon::new(IconName::ArrowUpRight).size(IconSize::XSmall))
                        .on_click(cx.listener(|this, _, _, cx| this.open(cx))),
                )
                .into_any_element();
        }
        let open_label = link_host(&url).map_or("Open".to_string(), |host| format!("Open {host}"));
        footer
            .child(decline)
            .child(div().flex_1())
            .child(
                div()
                    .debug_selector(move || format!("elicitation-copy-link-{id}"))
                    .child(
                        ActionButton::new(("elicitation-copy-link", id), "Copy link")
                            .start_icon(Icon::new(IconName::Copy).size(IconSize::XSmall))
                            .on_click(move |_, _, cx| copy_to_clipboard(&url, cx)),
                    ),
            )
            .child(
                ActionButton::new(("elicitation-open", id), open_label)
                    .style(ActionStyle::Primary)
                    .end_icon(Icon::new(IconName::ArrowUpRight).size(IconSize::XSmall))
                    .on_click(cx.listener(|this, _, _, cx| this.open(cx))),
            )
            .into_any_element()
    }
}

impl Render for ElicitationCard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.watch_blurs(window, cx);
        let border = cx.theme().colors().border;
        let background = cx.theme().colors().panel_background;
        let id = self.elicitation.id;
        let is_url = self.elicitation.url().is_some();
        let is_opened = self.elicitation.opened;
        // A page's card says in its title who sends you there and why, in the agent's words.
        let title = if is_url {
            one_line(&self.elicitation.request.message)
        } else {
            format!("{} is asking", self.requester_name)
        };
        // Of several questions, each step shows its own.
        let asks_one_at_a_time = self
            .questions
            .as_ref()
            .is_some_and(|questions| questions.len() > 1);
        let body =
            v_flex()
                .px(px(14.))
                .pb(px(14.))
                .gap_3()
                .when(!is_url && !asks_one_at_a_time, |this| {
                    this.child(
                        Label::new(self.elicitation.request.message.clone())
                            .size(LabelSize::Custom(rems_from_px(CONTROL_TEXT_SIZE))),
                    )
                });
        let body = match &self.elicitation.request.mode {
            acp::ElicitationMode::Form(mode) => body.child(self.render_form(mode, window, cx)),
            acp::ElicitationMode::Url(mode) => body.child(self.render_url(&mode.url, cx)),
            _ => body,
        };
        v_flex()
            .debug_selector(move || format!("elicitation-card-{id}"))
            .key_context(KEY_CONTEXT)
            .on_action(cx.listener(|this, _: &menu::Confirm, _, cx| this.confirm(cx)))
            .w_full()
            .rounded(px(8.))
            .border_1()
            .border_color(border)
            .bg(background)
            .overflow_hidden()
            .child(
                h_flex()
                    .px(px(14.))
                    .py(px(10.))
                    .gap_2()
                    .child(
                        Icon::new(if is_url {
                            IconName::ToolWeb
                        } else {
                            IconName::Chat
                        })
                        .size(IconSize::Small)
                        .color(Color::Accent),
                    )
                    .child(
                        div().flex_1().min_w_0().child(
                            Label::new(title)
                                .size(LabelSize::Custom(rems_from_px(CONTROL_TEXT_SIZE)))
                                .truncate(),
                        ),
                    )
                    // ACP's third answer, Cancel: the request goes away unanswered.
                    .child(
                        IconButton::new(("elicitation-cancel", id), IconName::Close)
                            .icon_size(IconSize::Small)
                            .icon_color(Color::Muted)
                            .tooltip(Tooltip::text("Cancel"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if is_opened {
                                    this.dismiss(cx)
                                } else {
                                    this.respond(acp::ElicitationAction::Cancel, cx)
                                }
                            })),
                    ),
            )
            .child(body)
            .child(self.render_footer(cx))
    }
}

/// Choices sit in a row when they're short and have no descriptions, else one to a line.
fn choice_group(options: &[ChoiceOption]) -> gpui::Div {
    let is_inline = options.len() <= 4
        && options
            .iter()
            .all(|option| option.description.is_none() && option.label.len() <= 24);
    if is_inline {
        h_flex().flex_wrap().gap_x_4().gap_y_1p5()
    } else {
        v_flex().gap_1p5()
    }
}

/// A choice's control beside its label and description. The one the agent suggests ("…
/// (Recommended)") has a tag instead.
fn choice(
    id: SharedString,
    control: AnyElement,
    option: &ChoiceOption,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    let (label, is_recommended) = recommended_choice(&option.label);
    let accent = cx.theme().colors().text_accent;
    let selector = id.to_string();
    h_flex()
        .id(id)
        .debug_selector(move || selector)
        .items_start()
        .gap_2()
        .cursor_pointer()
        .child(div().h(px(19.)).flex().items_center().child(control))
        .child(
            v_flex()
                .min_w_0()
                .gap_0p5()
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Label::new(label.to_string())
                                .size(LabelSize::Custom(rems_from_px(CONTROL_TEXT_SIZE))),
                        )
                        .when(is_recommended, |this| {
                            this.child(
                                div()
                                    .flex_none()
                                    .px(px(6.))
                                    .rounded(px(4.))
                                    .bg(accent.opacity(0.15))
                                    .text_size(rems_from_px(11_f32))
                                    .text_color(accent)
                                    .child("Recommended"),
                            )
                        }),
                )
                .children(option.description.clone().map(|description| {
                    Label::new(description)
                        .size(LabelSize::Small)
                        .color(Color::Muted)
                })),
        )
}

/// The red star after a required field's label.
fn required_mark(name: &str) -> impl IntoElement {
    let name = name.to_string();
    div()
        .debug_selector(move || format!("elicitation-required-{name}"))
        .child(Label::new(" *").size(LabelSize::Small).color(Color::Error))
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn radio(is_selected: bool, is_invalid: bool, cx: &App) -> AnyElement {
    let colors = cx.theme().colors();
    let border = if is_selected {
        colors.text_accent
    } else if is_invalid {
        cx.theme().status().error
    } else {
        colors.border
    };
    div()
        .size(px(14.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .border_1()
        .border_color(border)
        .bg(colors.editor_background)
        .when(is_selected, |this| {
            this.child(div().size(px(6.)).rounded_full().bg(colors.text_accent))
        })
        .into_any_element()
}

fn check_box(is_checked: bool, is_invalid: bool, cx: &App) -> AnyElement {
    let colors = cx.theme().colors();
    let (border, background) = if is_checked {
        (colors.text_accent, colors.text_accent)
    } else if is_invalid {
        (cx.theme().status().error, colors.editor_background)
    } else {
        (colors.border, colors.editor_background)
    };
    div()
        .size(px(14.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .border_1()
        .border_color(border)
        .bg(background)
        .when(is_checked, |this| {
            this.child(
                Icon::new(IconName::Check)
                    .size(IconSize::Custom(rems_from_px(10_f32)))
                    .color(Color::Custom(on_fill_color(cx))),
            )
        })
        .into_any_element()
}

enum Field {
    Text(Entity<TextInput>),
    Boolean(bool),
    SingleSelect(Option<String>),
    MultiSelect(HashSet<String>),
}

/// What the user has entered in a form, field by field.
struct FormState {
    fields: HashMap<String, Field>,
    errors: HashMap<String, SharedString>,
}

impl FormState {
    fn new(schema: &acp::ElicitationSchema, cx: &mut App) -> Self {
        let required = schema.required.as_deref().unwrap_or_default();
        let text_input = |default: Option<String>, cx: &mut App| {
            cx.new(|cx| {
                let mut input = TextInput::new("", cx);
                if let Some(default) = default {
                    input.set_text(default, cx);
                }
                input
            })
        };
        let mut fields = HashMap::default();
        for (name, property) in &schema.properties {
            let is_required = required.contains(name);
            let field = match property {
                acp::ElicitationPropertySchema::String(schema) => {
                    let options = single_select_options(schema);
                    if options.is_empty() {
                        Field::Text(text_input(schema.default.clone(), cx))
                    } else {
                        // A default that isn't one of the options is dropped; a required
                        // choice starts on the first.
                        let value = schema
                            .default
                            .clone()
                            .filter(|default| options.iter().any(|option| &option.value == default))
                            .or_else(|| {
                                is_required
                                    .then(|| options.first().map(|option| option.value.clone()))
                                    .flatten()
                            });
                        Field::SingleSelect(value)
                    }
                }
                acp::ElicitationPropertySchema::Number(schema) => Field::Text(text_input(
                    schema.default.map(|default| default.to_string()),
                    cx,
                )),
                acp::ElicitationPropertySchema::Integer(schema) => Field::Text(text_input(
                    schema.default.map(|default| default.to_string()),
                    cx,
                )),
                acp::ElicitationPropertySchema::Boolean(schema) => {
                    Field::Boolean(schema.default.unwrap_or(false))
                }
                acp::ElicitationPropertySchema::Array(schema) => Field::MultiSelect(
                    schema
                        .default
                        .clone()
                        .unwrap_or_default()
                        .into_iter()
                        .collect(),
                ),
                _ => continue,
            };
            fields.insert(name.clone(), field);
        }
        Self {
            fields,
            errors: HashMap::default(),
        }
    }

    fn set(&mut self, name: &str, field: Field) {
        self.fields.insert(name.to_string(), field);
        self.errors.remove(name);
    }

    /// Whether a required field is still empty.
    fn lacks_required(&self, schema: &acp::ElicitationSchema, cx: &App) -> bool {
        schema
            .required
            .as_deref()
            .unwrap_or_default()
            .iter()
            .any(|name| match self.fields.get(name) {
                Some(Field::Text(input)) => input.read(cx).text().trim().is_empty(),
                Some(Field::SingleSelect(value)) => value.is_none(),
                Some(Field::MultiSelect(values)) => values.is_empty(),
                Some(Field::Boolean(_)) | None => false,
            })
    }

    fn toggle(&mut self, name: &str, value: &str) {
        if let Some(Field::MultiSelect(selected)) = self.fields.get_mut(name) {
            if !selected.remove(value) {
                selected.insert(value.to_string());
            }
            self.errors.remove(name);
        }
    }

    fn validate(
        &self,
        schema: &acp::ElicitationSchema,
        cx: &App,
    ) -> Result<BTreeMap<String, acp::ElicitationContentValue>, HashMap<String, SharedString>> {
        let values = self
            .fields
            .iter()
            .map(|(name, field)| {
                let value = match field {
                    Field::Text(input) => Value::Text(input.read(cx).text().to_string()),
                    Field::Boolean(value) => Value::Boolean(*value),
                    Field::SingleSelect(value) => Value::SingleSelect(value.clone()),
                    Field::MultiSelect(values) => Value::MultiSelect(values.clone()),
                };
                (name.clone(), value)
            })
            .collect();
        validate(schema, &values)
    }
}

/// A field's value when the form is submitted.
#[derive(Debug)]
enum Value {
    Text(String),
    Boolean(bool),
    SingleSelect(Option<String>),
    MultiSelect(HashSet<String>),
}

/// Zed's checks, with every field's problem at once.
fn validate(
    schema: &acp::ElicitationSchema,
    values: &HashMap<String, Value>,
) -> Result<BTreeMap<String, acp::ElicitationContentValue>, HashMap<String, SharedString>> {
    let required = schema.required.as_deref().unwrap_or_default();
    let mut content = BTreeMap::new();
    let mut errors = HashMap::default();
    for (name, property) in &schema.properties {
        let Some(value) = values.get(name) else {
            continue;
        };
        let is_required = required.contains(name);
        let title = property_title(name, property);
        let missing = || -> Result<Option<acp::ElicitationContentValue>, SharedString> {
            if is_required {
                Err(format!("{title} is required").into())
            } else {
                Ok(None)
            }
        };
        let field_content = match (property, value) {
            (acp::ElicitationPropertySchema::String(schema), Value::Text(value)) => {
                if value.is_empty() {
                    missing()
                } else {
                    validate_string(&title, schema, value)
                        .map(|()| Some(acp::ElicitationContentValue::String(value.clone())))
                }
            }
            (acp::ElicitationPropertySchema::String(schema), Value::SingleSelect(value)) => {
                match value {
                    Some(value) => {
                        if single_select_options(schema)
                            .iter()
                            .any(|option| &option.value == value)
                        {
                            validate_string(&title, schema, value)
                                .map(|()| Some(acp::ElicitationContentValue::String(value.clone())))
                        } else {
                            Err(format!("{title} must be one of the provided options").into())
                        }
                    }
                    None => missing(),
                }
            }
            (acp::ElicitationPropertySchema::Number(schema), Value::Text(value)) => {
                let value = value.trim();
                if value.is_empty() {
                    missing()
                } else {
                    validate_number(&title, schema, value)
                        .map(|number| Some(acp::ElicitationContentValue::Number(number)))
                }
            }
            (acp::ElicitationPropertySchema::Integer(schema), Value::Text(value)) => {
                let value = value.trim();
                if value.is_empty() {
                    missing()
                } else {
                    validate_integer(&title, schema, value)
                        .map(|integer| Some(acp::ElicitationContentValue::Integer(integer)))
                }
            }
            (acp::ElicitationPropertySchema::Boolean(schema), Value::Boolean(value)) => {
                if is_required || *value || schema.default.is_some() {
                    Ok(Some(acp::ElicitationContentValue::Boolean(*value)))
                } else {
                    Ok(None)
                }
            }
            (acp::ElicitationPropertySchema::Array(schema), Value::MultiSelect(selected)) => {
                let mut chosen: Vec<String> = multi_select_options(schema)
                    .into_iter()
                    .filter(|option| selected.contains(&option.value))
                    .map(|option| option.value)
                    .collect();
                chosen.sort();
                if chosen.is_empty() && !is_required {
                    Ok(None)
                } else if schema
                    .min_items
                    .is_some_and(|min_items| (chosen.len() as u64) < min_items)
                {
                    Err(format!("{title} needs more selections").into())
                } else if schema
                    .max_items
                    .is_some_and(|max_items| (chosen.len() as u64) > max_items)
                {
                    Err(format!("{title} has too many selections").into())
                } else {
                    Ok(Some(acp::ElicitationContentValue::StringArray(chosen)))
                }
            }
            _ => Ok(None),
        };
        match field_content {
            Ok(Some(value)) => {
                content.insert(name.clone(), value);
            }
            Ok(None) => {}
            Err(error) => {
                errors.insert(name.clone(), error);
            }
        }
    }
    if errors.is_empty() {
        Ok(content)
    } else {
        Err(errors)
    }
}

fn validate_number(
    title: &str,
    schema: &acp::NumberPropertySchema,
    value: &str,
) -> Result<f64, SharedString> {
    let number = value
        .parse::<f64>()
        .map_err(|_| SharedString::from(format!("{title} must be a number")))?;
    if !number.is_finite() {
        return Err(format!("{title} must be a finite number").into());
    }
    if let Some(minimum) = schema.minimum
        && number < minimum
    {
        return Err(format!("{title} must be at least {minimum}").into());
    }
    if let Some(maximum) = schema.maximum
        && number > maximum
    {
        return Err(format!("{title} must be at most {maximum}").into());
    }
    Ok(number)
}

fn validate_integer(
    title: &str,
    schema: &acp::IntegerPropertySchema,
    value: &str,
) -> Result<i64, SharedString> {
    let integer = value
        .parse::<i64>()
        .map_err(|_| SharedString::from(format!("{title} must be an integer")))?;
    if let Some(minimum) = schema.minimum
        && integer < minimum
    {
        return Err(format!("{title} must be at least {minimum}").into());
    }
    if let Some(maximum) = schema.maximum
        && integer > maximum
    {
        return Err(format!("{title} must be at most {maximum}").into());
    }
    Ok(integer)
}

/// Length, pattern and format. Zed checks the last two with a JSON Schema validator; this
/// checks them with the crates already at hand, so an agent's own check stays the last word.
fn validate_string(
    title: &str,
    schema: &acp::StringPropertySchema,
    value: &str,
) -> Result<(), SharedString> {
    let length = value.chars().count();
    if schema
        .min_length
        .is_some_and(|min_length| length < min_length as usize)
    {
        return Err(format!("{title} is too short").into());
    }
    if schema
        .max_length
        .is_some_and(|max_length| length > max_length as usize)
    {
        return Err(format!("{title} is too long").into());
    }
    let matches_pattern = match &schema.pattern {
        Some(pattern) => match regex::Regex::new(pattern) {
            Ok(regex) => regex.is_match(value),
            Err(_) => return Err(format!("{title} has an invalid validation pattern").into()),
        },
        None => true,
    };
    let format = schema.format.and_then(|format| {
        let (label, matches): (&str, bool) = match format {
            acp::StringFormat::Email => (
                "an email address",
                value.split_once('@').is_some_and(|(user, domain)| {
                    !user.is_empty()
                        && domain.contains('.')
                        && !domain.starts_with('.')
                        && !domain.ends_with('.')
                        && !value.contains(char::is_whitespace)
                }),
            ),
            acp::StringFormat::Uri => ("a URI", url::Url::parse(value).is_ok()),
            acp::StringFormat::Date => (
                "a date",
                chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok(),
            ),
            acp::StringFormat::DateTime => (
                "a date and time",
                chrono::DateTime::parse_from_rfc3339(value).is_ok(),
            ),
            _ => return None,
        };
        Some((label, matches))
    });
    match (schema.pattern.is_some(), matches_pattern, format) {
        (_, true, None) | (_, true, Some((_, true))) => Ok(()),
        (true, _, Some(_)) => {
            Err(format!("{title} does not match the requested constraints").into())
        }
        (true, false, None) => Err(format!("{title} does not match the requested pattern").into()),
        (false, _, Some((label, _))) => Err(format!("{title} must be {label}").into()),
        (false, false, None) => Ok(()),
    }
}

#[derive(Clone)]
struct ChoiceOption {
    value: String,
    label: SharedString,
    description: Option<SharedString>,
}

fn single_select_options(schema: &acp::StringPropertySchema) -> Vec<ChoiceOption> {
    if let Some(options) = &schema.one_of {
        return options.iter().map(enum_option).collect();
    }
    schema
        .enum_values
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|value| ChoiceOption {
            value: value.clone(),
            label: value.clone().into(),
            description: None,
        })
        .collect()
}

fn multi_select_options(schema: &acp::MultiSelectPropertySchema) -> Vec<ChoiceOption> {
    match &schema.items {
        acp::MultiSelectItems::String(items) => items
            .values
            .iter()
            .map(|value| ChoiceOption {
                value: value.clone(),
                label: value.clone().into(),
                description: None,
            })
            .collect(),
        acp::MultiSelectItems::Titled(items) => items.options.iter().map(enum_option).collect(),
        _ => Vec::new(),
    }
}

fn enum_option(option: &acp::EnumOption) -> ChoiceOption {
    ChoiceOption {
        value: option.value.clone(),
        label: option.title.clone().into(),
        description: option.description.clone().map(SharedString::from),
    }
}

fn property_title(name: &str, property: &acp::ElicitationPropertySchema) -> SharedString {
    let title = match property {
        acp::ElicitationPropertySchema::String(schema) => schema.title.as_deref(),
        acp::ElicitationPropertySchema::Number(schema) => schema.title.as_deref(),
        acp::ElicitationPropertySchema::Integer(schema) => schema.title.as_deref(),
        acp::ElicitationPropertySchema::Boolean(schema) => schema.title.as_deref(),
        acp::ElicitationPropertySchema::Array(schema) => schema.title.as_deref(),
        _ => None,
    };
    SharedString::from(title.unwrap_or(name).to_string())
}

fn property_description(property: &acp::ElicitationPropertySchema) -> Option<SharedString> {
    match property {
        acp::ElicitationPropertySchema::String(schema) => schema.description.clone(),
        acp::ElicitationPropertySchema::Number(schema) => schema.description.clone(),
        acp::ElicitationPropertySchema::Integer(schema) => schema.description.clone(),
        acp::ElicitationPropertySchema::Boolean(schema) => schema.description.clone(),
        acp::ElicitationPropertySchema::Array(schema) => schema.description.clone(),
        _ => None,
    }
    .map(SharedString::from)
}

/// The URL's host, and how it reads decoded when it's an internationalized address, which can
/// imitate another (Zed warns about those).
fn url_host(url: &str) -> Option<(String, Option<String>)> {
    let url = url::Url::parse(url).ok()?;
    let host = url.host_str()?.to_string();
    let is_internationalized = host.split('.').any(|label| label.starts_with("xn--"));
    let decoded_host = is_internationalized
        .then(|| idna::domain_to_unicode(&host).0)
        .filter(|decoded| decoded != &host);
    Some((host, decoded_host))
}

/// Splits a URL into pieces that can wrap, at `/`, `?`, `&` or `#` once a piece is long
/// enough, never dropping a character, as Zed does.
fn display_url_segments(url: &str) -> Vec<SharedString> {
    let mut segments = Vec::new();
    let mut segment = String::new();
    let mut segment_chars = 0;
    let mut characters = url.chars().peekable();
    while let Some(character) = characters.next() {
        segment.push(character);
        segment_chars += 1;
        let at_boundary = segment_chars >= MIN_URL_DISPLAY_SEGMENT_CHARS
            && matches!(character, '/' | '?' | '&' | '#');
        let at_length = segment_chars >= MAX_URL_DISPLAY_SEGMENT_CHARS;
        if characters.peek().is_some() && (at_boundary || at_length) {
            segments.push(std::mem::take(&mut segment).into());
            segment_chars = 0;
        }
    }
    if !segment.is_empty() {
        segments.push(segment.into());
    }
    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema(value: serde_json::Value) -> acp::ElicitationSchema {
        serde_json::from_value(value).expect("a valid schema")
    }

    #[test]
    fn validation_reports_every_field() {
        let schema = schema(serde_json::json!({
            "type": "object",
            "properties": {
                "name": {"type": "string", "title": "Name", "minLength": 2},
                "email": {"type": "string", "format": "email"},
                "count": {"type": "integer", "minimum": 1, "maximum": 3},
                "ratio": {"type": "number"},
                "code": {"type": "string", "pattern": "^[A-Z]{3}$"},
            },
            "required": ["name", "count"],
        }));
        let values = HashMap::from_iter([
            ("name".to_string(), Value::Text("a".into())),
            ("email".to_string(), Value::Text("nobody".into())),
            ("count".to_string(), Value::Text("5".into())),
            ("ratio".to_string(), Value::Text("lots".into())),
            ("code".to_string(), Value::Text("abc".into())),
        ]);
        let errors = validate(&schema, &values).unwrap_err();
        assert_eq!(errors["name"].as_ref(), "Name is too short");
        assert_eq!(errors["email"].as_ref(), "email must be an email address");
        assert_eq!(errors["count"].as_ref(), "count must be at most 3");
        assert_eq!(errors["ratio"].as_ref(), "ratio must be a number");
        assert_eq!(
            errors["code"].as_ref(),
            "code does not match the requested pattern"
        );
    }

    #[test]
    fn validation_collects_what_was_entered() {
        let schema = schema(serde_json::json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "tone": {"type": "string", "enum": ["calm", "loud"]},
                "times": {"type": "integer"},
                "loud": {"type": "boolean"},
                "quiet": {"type": "boolean"},
                "tags": {"type": "array", "items": {"type": "string", "enum": ["a", "b"]}},
                "note": {"type": "string"},
            },
            "required": ["name"],
        }));
        let values = HashMap::from_iter([
            ("name".to_string(), Value::Text("Ada".into())),
            ("tone".to_string(), Value::SingleSelect(Some("loud".into()))),
            ("times".to_string(), Value::Text(" 2 ".into())),
            ("loud".to_string(), Value::Boolean(true)),
            ("quiet".to_string(), Value::Boolean(false)),
            (
                "tags".to_string(),
                Value::MultiSelect(HashSet::from_iter(["b".to_string(), "a".to_string()])),
            ),
            ("note".to_string(), Value::Text(String::new())),
        ]);
        let content = validate(&schema, &values).expect("the values are valid");
        assert_eq!(
            serde_json::to_value(content).expect("serializable"),
            serde_json::json!({
                "name": "Ada",
                "tone": "loud",
                "times": 2,
                "loud": true,
                "tags": ["a", "b"],
            })
        );
    }

    #[test]
    fn required_fields_must_be_filled() {
        let schema = schema(serde_json::json!({
            "type": "object",
            "properties": {"name": {"type": "string", "title": "Name"}},
            "required": ["name"],
        }));
        let values = HashMap::from_iter([("name".to_string(), Value::Text(String::new()))]);
        let errors = validate(&schema, &values).unwrap_err();
        assert_eq!(errors["name"].as_ref(), "Name is required");
    }

    #[test]
    fn urls_wrap_without_losing_characters() {
        let url = "https://example.com/device/login?code=MOCK-1234&next=/home#done";
        let segments = display_url_segments(url);
        assert!(segments.len() > 1);
        assert_eq!(
            segments
                .iter()
                .map(|segment| segment.as_ref())
                .collect::<String>(),
            url
        );
    }

    #[test]
    fn internationalized_hosts_are_decoded() {
        assert_eq!(
            url_host("https://example.com/a"),
            Some(("example.com".to_string(), None))
        );
        let (host, decoded) = url_host("https://xn--80ak6aa92e.com/").expect("a host");
        assert_eq!(host, "xn--80ak6aa92e.com");
        assert_eq!(decoded.as_deref(), Some("аррӏе.com"));
    }
}
