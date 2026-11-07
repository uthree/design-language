//! Messages the person sent while the agent was still answering.

use std::rc::Rc;

use gpui::{
    App, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};

use crate::button::{Button, ButtonVariant, ControlSize};
use crate::icon::IconName;
use crate::strings::ActiveStrings;
use crate::text::{Label, TextColor, TextSize};
use crate::theme::ActiveTheme;

/// One message waiting in a [`MessageQueue`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueuedMessage {
    /// What the person wrote.
    pub text: SharedString,
    /// Labels of the files sent with it.
    pub attachments: Vec<SharedString>,
}

impl QueuedMessage {
    /// A message.
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            attachments: Vec::new(),
        }
    }

    /// A file sent with it.
    pub fn attachment(mut self, label: impl Into<SharedString>) -> Self {
        self.attachments.push(label.into());
        self
    }
}

type IndexHandler = Rc<dyn Fn(&usize, &mut Window, &mut App)>;
type PauseHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;

/// The messages waiting for the agent's turn to end, between the transcript and the
/// [`super::Composer`] (whose [`super::Composer::queue`] sends them here).
///
/// A header counts them and pauses or resumes the queue; each message is one line, the whole
/// text in its tooltip, with "Send now" (stop the reply and send this next), "Edit" (take it
/// back into the composer) and remove. The application keeps the messages and sends the first
/// when a turn ends, unless paused.
///
/// Selectors: `{id}-pause`, and per message `{id}-message-{i}`, `{id}-send-now-{i}`,
/// `{id}-edit-{i}` and `{id}-remove-{i}`.
#[derive(IntoElement)]
pub struct MessageQueue {
    id: ElementId,
    messages: Vec<QueuedMessage>,
    paused: bool,
    disabled: bool,
    editable: bool,
    on_pause: Option<PauseHandler>,
    on_send_now: Option<IndexHandler>,
    on_edit: Option<IndexHandler>,
    on_remove: Option<IndexHandler>,
}

impl MessageQueue {
    /// An empty queue.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            messages: Vec::new(),
            paused: false,
            disabled: false,
            editable: true,
            on_pause: None,
            on_send_now: None,
            on_edit: None,
            on_remove: None,
        }
    }

    /// Add a waiting message, first to be sent first.
    pub fn message(mut self, message: impl Into<QueuedMessage>) -> Self {
        self.messages.push(message.into());
        self
    }

    /// Add waiting messages.
    pub fn messages<M: Into<QueuedMessage>>(
        mut self,
        messages: impl IntoIterator<Item = M>,
    ) -> Self {
        self.messages.extend(messages.into_iter().map(Into::into));
        self
    }

    /// The queue is paused: nothing is sent when a turn ends until it resumes.
    pub fn paused(mut self, paused: bool) -> Self {
        self.paused = paused;
        self
    }

    /// Nothing can be sent, edited or removed now, for example while the conversation loads.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Whether "Edit" can take a message back now; turn it off while the composer holds a
    /// draft it would overwrite.
    pub fn editable(mut self, editable: bool) -> Self {
        self.editable = editable;
        self
    }

    /// Called with `true` to pause and `false` to resume.
    pub fn on_pause(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_pause = Some(Rc::new(handler));
        self
    }

    /// Called with a message's index to send it now, ahead of the rest.
    pub fn on_send_now(
        mut self,
        handler: impl Fn(&usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_send_now = Some(Rc::new(handler));
        self
    }

    /// Called with a message's index to take it back into the composer.
    pub fn on_edit(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_edit = Some(Rc::new(handler));
        self
    }

    /// Called with a message's index to drop it.
    pub fn on_remove(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_remove = Some(Rc::new(handler));
        self
    }

    fn child(&self, part: impl Into<SharedString>) -> ElementId {
        ElementId::NamedChild(Box::new(self.id.clone()), part.into())
    }
}

impl From<&'static str> for QueuedMessage {
    fn from(text: &'static str) -> Self {
        Self::new(text)
    }
}

impl From<String> for QueuedMessage {
    fn from(text: String) -> Self {
        Self::new(text)
    }
}

impl From<SharedString> for QueuedMessage {
    fn from(text: SharedString) -> Self {
        Self::new(text)
    }
}

impl RenderOnce for MessageQueue {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let strings = cx.strings();
        let enabled = !self.disabled;
        let paused = self.paused;
        let pause = {
            let on_pause = self.on_pause.clone();
            Button::new(
                self.child("pause"),
                if paused {
                    strings.resume.clone()
                } else {
                    strings.pause.clone()
                },
            )
            .icon(if paused {
                IconName::Play
            } else {
                IconName::Pause
            })
            .variant(ButtonVariant::Ghost)
            .size(ControlSize::Small)
            .disabled(!enabled)
            .on_click(move |_, window, cx| {
                if let Some(on_pause) = &on_pause {
                    on_pause(&!paused, window, cx);
                }
            })
        };
        let small = |id: ElementId,
                     label: SharedString,
                     enabled: bool,
                     handler: Option<IndexHandler>,
                     index: usize| {
            Button::new(id, label)
                .variant(ButtonVariant::Ghost)
                .size(ControlSize::Small)
                .disabled(!enabled)
                .on_click(move |_, window, cx| {
                    if let Some(handler) = &handler {
                        handler(&index, window, cx);
                    }
                })
        };
        let rows: Vec<_> = self
            .messages
            .iter()
            .enumerate()
            .map(|(index, message)| {
                // One line: runs of spaces and line breaks read as one space.
                let mut preview = message
                    .text
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                if !message.attachments.is_empty() {
                    preview = format!("{preview} · {}", message.attachments.join(", "));
                }
                let full = message.text.clone();
                let id = self.child(format!("message-{index}"));
                crate::focus::selector(div().id(id.clone()), &id)
                    .flex()
                    .items_center()
                    .gap(theme.spacing.xs)
                    .min_w_0()
                    .py(theme.spacing.xxs)
                    .border_t_1()
                    .border_color(colors.separator)
                    .tooltip(move |window, cx| {
                        crate::overlay::Tooltip::new(full.clone()).build(window, cx)
                    })
                    .child(
                        div().flex_1().min_w_0().truncate().child(
                            Label::new(preview)
                                .size(TextSize::Small)
                                .color(TextColor::Muted),
                        ),
                    )
                    .child(
                        small(
                            self.child(format!("send-now-{index}")),
                            strings.send_now.clone(),
                            enabled,
                            self.on_send_now.clone(),
                            index,
                        )
                        .tooltip(strings.send_now_hint.clone()),
                    )
                    .child(small(
                        self.child(format!("edit-{index}")),
                        strings.edit.clone(),
                        enabled && self.editable,
                        self.on_edit.clone(),
                        index,
                    ))
                    .child({
                        let on_remove = self.on_remove.clone();
                        Button::icon_only(self.child(format!("remove-{index}")), IconName::Close)
                            .variant(ButtonVariant::Ghost)
                            .size(ControlSize::Small)
                            .disabled(!enabled)
                            .tooltip(strings.remove.clone())
                            .on_click(move |_, window, cx| {
                                if let Some(on_remove) = &on_remove {
                                    on_remove(&index, window, cx);
                                }
                            })
                    })
            })
            .collect();
        crate::focus::selector(div().id(self.id.clone()), &self.id)
            .flex()
            .flex_col()
            .min_w_0()
            .px(theme.spacing.sm)
            .py(theme.spacing.xs)
            .gap(theme.spacing.xxs)
            .when(self.messages.is_empty(), |queue| queue.hidden())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme.spacing.xs)
                    .child(
                        div().flex_1().min_w_0().child(
                            Label::new(strings.queued(self.messages.len(), paused))
                                .size(TextSize::XSmall)
                                .color(if paused {
                                    TextColor::Warning
                                } else {
                                    TextColor::Faint
                                }),
                        ),
                    )
                    .child(pause),
            )
            .child(
                div()
                    .id(self.child("list"))
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .max_h(px(144.))
                    .overflow_y_scroll()
                    .children(rows),
            )
    }
}
