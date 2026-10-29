//! Rows of the conversation.

use std::time::Duration;

use gpui::{
    AnyElement, App, ClickEvent, ElementId, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, px,
};

use crate::button::{Button, ButtonVariant, ControlSize};
use crate::display::{Avatar, Spinner};
use crate::focus::focusable;
use crate::icon::{Icon, IconName, IconSource};
use crate::motion::{appear, chevron, looping, reveal, transition};
use crate::selectable::SelectableText;
use crate::strings::ActiveStrings;
use crate::text::{Label, TextColor, TextSize};
use crate::theme::{ActiveTheme, Theme};

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// The hover group of a message, whose footer actions show while the pointer is over it.
const MESSAGE_GROUP: &str = "atelier-message";

/// A message's footer actions: drawn only while the pointer is over the message, while the
/// person moves through the window with the keyboard, or always when `pinned`. The row keeps
/// its height while hidden, so showing it never moves the transcript.
fn hover_actions(actions: Vec<AnyElement>, pinned: bool, cx: &App) -> gpui::Div {
    let theme = cx.theme();
    div()
        .flex()
        .items_center()
        .gap(theme.spacing.xxs)
        .when(!pinned && !crate::focus::focus_visible(cx), |row| {
            row.opacity(0.)
                .group_hover(MESSAGE_GROUP, |style| style.opacity(1.))
        })
        .children(actions)
}

/// Fade a new transcript row in, if it has an id to remember that it already arrived.
///
/// Only its opacity moves, never its margin or height: a transcript grows from the bottom and
/// stays scrolled there, so a row that changed height while it arrived would jolt everything
/// above it and shift any selection the person is making.
fn arrive(row: gpui::Div, id: Option<ElementId>, cx: &App) -> AnyElement {
    let row = row.group(MESSAGE_GROUP);
    match id {
        Some(id) => appear(row, id, cx, |row, delta| row.opacity(delta)),
        None => row.into_any_element(),
    }
}

/// What the person said: a bubble on the trailing edge, with any attached context above it
/// and any actions below.
///
/// Given an id, its text can be selected and copied ([`SelectableText`]), exactly as typed.
#[derive(IntoElement)]
pub struct UserMessage {
    id: Option<ElementId>,
    text: SharedString,
    body: Option<AnyElement>,
    attachments: Vec<AnyElement>,
    actions: Vec<AnyElement>,
    pinned: bool,
}

impl UserMessage {
    /// A message.
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            id: None,
            text: text.into(),
            body: None,
            attachments: Vec::new(),
            actions: Vec::new(),
            pinned: false,
        }
    }

    /// A stable id for the message. With one, the message fades in when it first appears and
    /// its text can be selected; without, it is simply drawn.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Replace the text inside the bubble, as [`AssistantMessage::body`] does: an
    /// application's own Markdown or rich-text view. The bubble keeps its fill and shape.
    pub fn body(mut self, body: impl IntoElement) -> Self {
        self.body = Some(body.into_any_element());
        self
    }

    /// Context sent with the message, usually [`super::ContextChip`]s.
    pub fn attachment(mut self, attachment: impl IntoElement) -> Self {
        self.attachments.push(attachment.into_any_element());
        self
    }

    /// A small action under the bubble, such as an icon-only [`crate::CopyButton`] or Edit.
    /// Like [`AssistantMessage::action`], it shows while the pointer is over the message or the
    /// person is moving with the keyboard.
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }

    /// Keep the actions drawn even without the pointer over the message.
    pub fn pin_actions(mut self) -> Self {
        self.pinned = true;
        self
    }
}

impl RenderOnce for UserMessage {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let body = match (self.body, &self.id) {
            (Some(body), _) => body,
            (None, Some(id)) => SelectableText::new((id.clone(), "text"), self.text)
                .on_fill(colors.on_user_bubble)
                .into_any_element(),
            (None, None) => self.text.into_any_element(),
        };
        let message = div()
            .flex()
            .flex_col()
            .items_end()
            .gap(theme.spacing.xs)
            .pl(theme.spacing.xxl)
            .when(!self.attachments.is_empty(), |column| {
                column.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .justify_end()
                        .gap(theme.spacing.xs)
                        .children(self.attachments),
                )
            })
            .child(
                div()
                    .max_w_full()
                    .px(theme.spacing.md)
                    .py(theme.spacing.sm)
                    .rounded(theme.shape.radius_lg)
                    .bg(colors.user_bubble)
                    .text_color(colors.on_user_bubble)
                    .text_size(theme.typography.size_md)
                    .child(body),
            )
            .when(!self.actions.is_empty(), |column| {
                column.child(hover_actions(self.actions, self.pinned, cx))
            });
        arrive(message, self.id, cx)
    }
}

/// What the agent answered: full-width prose under a small identity line.
///
/// The body understands the little Markdown a chat answer needs — paragraphs, `-` bullets,
/// `#` headings and fenced code. An application with a real Markdown view passes it with
/// [`AssistantMessage::body`] instead.
#[derive(IntoElement)]
pub struct AssistantMessage {
    id: Option<ElementId>,
    text: SharedString,
    body: Option<AnyElement>,
    name: Option<SharedString>,
    meta: Option<SharedString>,
    logo: Option<IconSource>,
    anonymous: bool,
    streaming: bool,
    actions: Vec<AnyElement>,
    pinned: bool,
}

impl AssistantMessage {
    /// An answer.
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            id: None,
            text: text.into(),
            body: None,
            name: None,
            meta: None,
            logo: None,
            anonymous: false,
            streaming: false,
            actions: Vec::new(),
            pinned: false,
        }
    }

    /// A stable id for the message. With one, it fades in when it first appears and the text
    /// of the built-in rendering can be selected and copied, block by block.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Replace the built-in text rendering.
    pub fn body(mut self, body: impl IntoElement) -> Self {
        self.body = Some(body.into_any_element());
        self
    }

    /// The agent's name in the identity line; [`crate::Strings::assistant`] by default.
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Muted metadata after the name: the model, the time taken.
    pub fn meta(mut self, meta: impl Into<SharedString>) -> Self {
        self.meta = Some(meta.into());
        self
    }

    /// Draw the identity line's avatar as the provider's mark — a [`crate::ProviderLogo`] —
    /// instead of the agent's gradient, so the person sees whose model answered.
    pub fn logo(mut self, logo: impl Into<IconSource>) -> Self {
        self.logo = Some(logo.into());
        self
    }

    /// Leave out the identity line: no avatar, no name. Use it wherever only two voices speak —
    /// the person in their bubbles and one agent in open prose — since the form already says
    /// who is talking. Metadata, if any, moves to the footer.
    pub fn anonymous(mut self) -> Self {
        self.anonymous = true;
        self
    }

    /// Still arriving: draws a caret after the text and hides the actions.
    pub fn streaming(mut self, streaming: bool) -> Self {
        self.streaming = streaming;
        self
    }

    /// A small footer action such as an icon-only [`crate::CopyButton`] or Retry. Actions show
    /// while the pointer is over the message and whenever the person moves with the keyboard,
    /// so they stay out of the reading but Tab still finds them. Metadata stays visible.
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }

    /// Keep the actions drawn even without the pointer over the message, for example on the
    /// answer the person is most likely to act on next.
    pub fn pin_actions(mut self) -> Self {
        self.pinned = true;
        self
    }
}

fn prose(text: &str, id: Option<&ElementId>, theme: &Theme) -> Vec<AnyElement> {
    let colors = &theme.colors;
    let mut blocks = Vec::new();
    // With an id, each block is its own selectable text, keyed by its place in the message.
    let mut count = 0;
    let mut text_block = |text: String, mono: bool| -> AnyElement {
        count += 1;
        match id {
            Some(id) => {
                let id =
                    ElementId::NamedChild(Box::new(id.clone()), format!("block-{count}").into());
                SelectableText::new(id, text)
                    .when(mono, |text| text.mono().size(TextSize::Small))
                    .into_any_element()
            }
            None => text.into_any_element(),
        }
    };
    let mut lines = text.lines().peekable();
    let mut paragraph = String::new();
    let flush = |paragraph: &mut String,
                 blocks: &mut Vec<AnyElement>,
                 text_block: &mut dyn FnMut(String, bool) -> AnyElement| {
        if !paragraph.is_empty() {
            blocks.push(
                div()
                    .child(text_block(std::mem::take(paragraph), false))
                    .into_any_element(),
            );
        }
    };
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            flush(&mut paragraph, &mut blocks, &mut text_block);
            let mut code = Vec::new();
            for line in lines.by_ref() {
                if line.trim_start().starts_with("```") {
                    break;
                }
                code.push(line);
            }
            blocks.push(
                div()
                    .px(theme.spacing.md)
                    .py(theme.spacing.sm)
                    .rounded(theme.shape.radius_md)
                    .bg(colors.sunken)
                    .border_1()
                    .border_color(colors.border)
                    .font(theme.typography.mono.clone())
                    .text_size(theme.typography.size_sm)
                    .child(text_block(code.join("\n"), true))
                    .into_any_element(),
            );
        } else if let Some(item) = trimmed.strip_prefix("- ") {
            flush(&mut paragraph, &mut blocks, &mut text_block);
            blocks.push(
                div()
                    .flex()
                    .gap(theme.spacing.sm)
                    .child(div().text_color(colors.text_faint).child("•"))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(text_block(item.to_owned(), false)),
                    )
                    .into_any_element(),
            );
        } else if let Some(heading) = trimmed.strip_prefix("# ") {
            flush(&mut paragraph, &mut blocks, &mut text_block);
            blocks.push(
                Label::new(heading.to_owned())
                    .heading_weight()
                    .into_any_element(),
            );
        } else if trimmed.is_empty() {
            flush(&mut paragraph, &mut blocks, &mut text_block);
        } else {
            if !paragraph.is_empty() {
                paragraph.push(' ');
            }
            paragraph.push_str(trimmed);
        }
    }
    flush(&mut paragraph, &mut blocks, &mut text_block);
    blocks
}

impl RenderOnce for AssistantMessage {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let body = match self.body {
            Some(body) => vec![body],
            None => prose(&self.text, self.id.as_ref(), theme),
        };
        let caret = self.streaming.then(|| {
            looping(
                div()
                    .w(px(7.))
                    .h(theme.typography.size_md)
                    .rounded(px(1.))
                    .bg(colors.agent),
                "caret",
                Duration::from_millis(1000),
                1.,
                cx,
                |caret, delta| caret.opacity(gpui::pulsating_between(0.2, 1.)(delta)),
            )
        });
        let (identity, footer_meta) = if self.anonymous {
            (None, self.meta)
        } else {
            let identity = div()
                .flex()
                .items_center()
                .gap(theme.spacing.sm)
                .child(agent_avatar(self.logo).size(px(20.)))
                .child(
                    Label::new(self.name.unwrap_or_else(|| cx.strings().assistant.clone()))
                        .size(TextSize::Small)
                        .weight(theme.typography.label_weight),
                )
                .children(self.meta.map(|meta| {
                    Label::new(meta)
                        .size(TextSize::XSmall)
                        .color(TextColor::Faint)
                }));
            (Some(identity), None)
        };
        let footer_meta = footer_meta.filter(|_| !self.streaming).map(|meta| {
            Label::new(meta)
                .size(TextSize::XSmall)
                .color(TextColor::Faint)
        });
        let has_actions = !self.actions.is_empty();
        let message = div()
            .flex()
            .flex_col()
            .gap(theme.spacing.sm)
            .children(identity)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.spacing.sm)
                    .text_size(theme.typography.size_md)
                    .text_color(colors.text)
                    .children(body)
                    .children(caret),
            )
            .when(
                !self.streaming && (!self.actions.is_empty() || footer_meta.is_some()),
                |column| {
                    column.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(theme.spacing.xxs)
                            .when(has_actions, |footer| {
                                footer.child(hover_actions(self.actions, self.pinned, cx))
                            })
                            .children(footer_meta.map(|meta| {
                                div()
                                    .when(has_actions, |slot| slot.ml(theme.spacing.xs))
                                    .child(meta)
                            })),
                    )
                },
            );
        arrive(message, self.id, cx)
    }
}

/// The model's reasoning, collapsed to one line until opened.
#[derive(IntoElement)]
pub struct ReasoningBlock {
    id: ElementId,
    text: SharedString,
    summary: SharedString,
    open: bool,
    on_toggle: Option<ClickHandler>,
}

impl ReasoningBlock {
    /// Reasoning text with a one-line summary such as "Thought for 8 s".
    pub fn new(
        id: impl Into<ElementId>,
        summary: impl Into<SharedString>,
        text: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            summary: summary.into(),
            open: false,
            on_toggle: None,
        }
    }

    /// Expanded.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Called on click.
    pub fn on_toggle(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for ReasoningBlock {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let muted = cx.theme().colors.text_muted;
        let chevron = chevron(
            (self.id.clone(), "chevron"),
            self.open,
            px(11.),
            muted,
            window,
            cx,
        );
        let reveal_id = (self.id.clone(), "text");
        let theme = cx.theme();
        let colors = &theme.colors;
        let hover = colors.hover;
        let mut header = focusable(
            div().id(self.id.clone()),
            &self.id,
            self.on_toggle.is_none(),
            theme.shape.radius_sm,
            window,
            cx,
        )
        .flex()
        .items_center()
        .gap(theme.spacing.xs)
        .h(theme.density.control_sm)
        .px(theme.spacing.xs)
        .rounded(theme.shape.radius_sm)
        .text_size(theme.typography.size_sm)
        .text_color(colors.text_muted)
        .hover(move |style| style.bg(hover))
        .child(Icon::new(IconName::Brain).size(px(13.)))
        .child(self.summary)
        .child(chevron);
        if let Some(handler) = self.on_toggle {
            header = header.on_click(handler);
        }
        div()
            .flex()
            .flex_col()
            .gap(theme.spacing.xs)
            .child(div().flex().child(header))
            .when(self.open, |column| {
                column.child(reveal(
                    div()
                        .ml(theme.spacing.sm)
                        .pl(theme.spacing.md)
                        .border_l_1()
                        .border_color(colors.border_strong)
                        .text_size(theme.typography.size_sm)
                        .text_color(colors.text_muted)
                        .child(self.text),
                    reveal_id,
                    cx,
                ))
            })
    }
}

/// Where a tool call is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolState {
    /// Waiting for its answer.
    Running,
    /// Answered.
    Succeeded,
    /// Refused or errored.
    Failed,
    /// Waiting for the person's approval.
    AwaitingApproval,
}

/// One tool call: a one-line summary that opens onto its arguments and result.
#[derive(IntoElement)]
pub struct ToolCall {
    id: ElementId,
    name: SharedString,
    summary: SharedString,
    state: ToolState,
    detail: Option<SharedString>,
    open: bool,
    icon: IconName,
    on_toggle: Option<ClickHandler>,
}

impl ToolCall {
    /// A tool call with its wire name and a human summary, for example
    /// `edit_notes` / "Edited 12 notes in Piano".
    pub fn new(
        id: impl Into<ElementId>,
        name: impl Into<SharedString>,
        summary: impl Into<SharedString>,
        state: ToolState,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            summary: summary.into(),
            state,
            detail: None,
            open: false,
            icon: IconName::Wrench,
            on_toggle: None,
        }
    }

    /// Arguments and result, shown when open.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Expanded.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// The tool's icon.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = icon;
        self
    }

    /// Called on click.
    pub fn on_toggle(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for ToolCall {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = self.id.clone();
        let theme = cx.theme().clone();
        let colors = &theme.colors;
        let state_mark = match self.state {
            ToolState::Running => Spinner::new((self.id.clone(), "spinner"))
                .size(px(12.))
                .into_any_element(),
            ToolState::Succeeded => Icon::new(IconName::Check)
                .size(px(12.))
                .color(colors.success)
                .into_any_element(),
            ToolState::Failed => Icon::new(IconName::Close)
                .size(px(12.))
                .color(colors.danger)
                .into_any_element(),
            ToolState::AwaitingApproval => Icon::new(IconName::Alert)
                .size(px(12.))
                .color(colors.warning)
                .into_any_element(),
        };
        let hover = colors.hover;
        let mut row = focusable(
            div().id(self.id.clone()),
            &self.id,
            self.on_toggle.is_none(),
            theme.shape.radius_sm,
            window,
            cx,
        )
        .flex()
        .items_center()
        .gap(theme.spacing.sm)
        .h(theme.density.row)
        .px(theme.spacing.sm)
        .rounded(theme.shape.radius_sm)
        .text_size(theme.typography.size_sm)
        .hover(move |style| style.bg(hover))
        .child(Icon::new(self.icon).size(px(13.)).color(colors.text_faint))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(colors.text)
                .child(self.summary),
        )
        .child(
            Label::new(self.name)
                .size(TextSize::XSmall)
                .mono()
                .color(TextColor::Faint),
        )
        // A finished call's mark settles in as the spinner gives way to it.
        .child(transition(
            div().flex().w(px(14.)).justify_center().child(state_mark),
            (id.clone(), "state"),
            &self.state,
            theme.motion.normal,
            window,
            cx,
            |mark, delta| mark.opacity(delta),
        ));
        let reveal_id = (id, "detail");
        if let Some(handler) = self.on_toggle {
            row = row.on_click(handler);
        }
        div().flex().flex_col().child(row).when_some(
            self.detail.filter(|_| self.open),
            |column, detail| {
                column.child(reveal(
                    div()
                        .mx(theme.spacing.sm)
                        .mb(theme.spacing.xs)
                        .px(theme.spacing.md)
                        .py(theme.spacing.sm)
                        .rounded(theme.shape.radius_sm)
                        .bg(colors.sunken)
                        .font(theme.typography.mono.clone())
                        .text_size(theme.typography.size_xs)
                        .text_color(colors.text_muted)
                        .child(detail),
                    reveal_id,
                    cx,
                ))
            },
        )
    }
}

/// Consecutive tool calls, framed as one unit so a long turn reads as its answer.
#[derive(IntoElement)]
pub struct ToolGroup {
    title: SharedString,
    calls: Vec<AnyElement>,
}

impl ToolGroup {
    /// A group titled, for example, "Used 3 tools".
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            calls: Vec::new(),
        }
    }

    /// Add a [`ToolCall`].
    pub fn call(mut self, call: impl IntoElement) -> Self {
        self.calls.push(call.into_any_element());
        self
    }
}

impl RenderOnce for ToolGroup {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let inset = theme.spacing.xs;
        div()
            .flex()
            .flex_col()
            .p(inset)
            .rounded(theme.shape.radius_md)
            .border_1()
            .border_color(theme.colors.border)
            .child(
                div()
                    .px(theme.spacing.sm)
                    .pt(theme.spacing.xxs)
                    .pb(theme.spacing.xs)
                    .child(
                        Label::new(self.title)
                            .size(TextSize::XSmall)
                            .color(TextColor::Faint),
                    ),
            )
            .children(self.calls)
    }
}

type DecisionHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// A paused turn asking the person to allow an action.
///
/// The title names the verb and the object (`Replace "Verse 2" melody?`); the body says only what
/// the title cannot — scope, and whether it can be undone. Deny is the safe default.
#[derive(IntoElement)]
pub struct ApprovalCard {
    id: ElementId,
    title: SharedString,
    body: Option<SharedString>,
    allow_label: Option<SharedString>,
    destructive: bool,
    on_allow: Option<DecisionHandler>,
    on_deny: Option<DecisionHandler>,
    on_always: Option<DecisionHandler>,
}

impl ApprovalCard {
    /// An approval request.
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            body: None,
            allow_label: None,
            destructive: false,
            on_allow: None,
            on_deny: None,
            on_always: None,
        }
    }

    /// Scope and consequence.
    pub fn body(mut self, body: impl Into<SharedString>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// The verb of the allowing button, for example "Replace".
    pub fn allow_label(mut self, label: impl Into<SharedString>) -> Self {
        self.allow_label = Some(label.into());
        self
    }

    /// The action destroys work: draw the allowing button as danger.
    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    /// Called on allow.
    pub fn on_allow(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_allow = Some(Box::new(handler));
        self
    }

    /// Called on deny.
    pub fn on_deny(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_deny = Some(Box::new(handler));
        self
    }

    /// Offer "Always allow" and call this on it.
    pub fn on_always(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_always = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for ApprovalCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let mut allow = Button::new(
            (self.id.clone(), "allow"),
            self.allow_label
                .unwrap_or_else(|| cx.strings().allow.clone()),
        )
        .size(ControlSize::Small)
        .variant(if self.destructive {
            ButtonVariant::Danger
        } else {
            ButtonVariant::Primary
        });
        if let Some(handler) = self.on_allow {
            allow = allow.on_click(handler);
        }
        let mut deny = Button::new((self.id.clone(), "deny"), cx.strings().deny.clone())
            .size(ControlSize::Small);
        if let Some(handler) = self.on_deny {
            deny = deny.on_click(handler);
        }
        let always = self.on_always.map(|handler| {
            Button::new(
                (self.id.clone(), "always"),
                cx.strings().always_allow.clone(),
            )
            .size(ControlSize::Small)
            .ghost()
            .on_click(handler)
        });
        let card = div()
            .flex()
            .flex_col()
            .gap(theme.spacing.sm)
            .p(theme.spacing.md)
            .rounded(theme.shape.radius_md)
            .bg(colors.warning_wash)
            .border_1()
            .border_color(colors.warning.opacity(0.4))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme.spacing.sm)
                    .child(
                        Icon::new(IconName::Alert)
                            .size(px(15.))
                            .color(colors.warning),
                    )
                    .child(Label::new(self.title).weight(theme.typography.label_weight)),
            )
            .children(self.body.map(|body| {
                Label::new(body)
                    .size(TextSize::Small)
                    .color(TextColor::Muted)
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme.spacing.sm)
                    .children(always)
                    .child(div().flex_1())
                    .child(deny)
                    .child(allow),
            );
        arrive(card, Some(self.id), cx)
    }
}

/// The kind of a runtime notice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeKind {
    /// Produced by the runtime, not the model: "Context compacted".
    Status,
    /// Something went wrong.
    Error,
}

/// A centred line from the runtime, not from either speaker.
#[derive(IntoElement)]
pub struct ChatNotice {
    text: SharedString,
    kind: NoticeKind,
    action: Option<AnyElement>,
}

impl ChatNotice {
    /// A status line.
    pub fn status(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            kind: NoticeKind::Status,
            action: None,
        }
    }

    /// An error line. Say what happened and how to recover.
    pub fn error(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            kind: NoticeKind::Error,
            action: None,
        }
    }

    /// A recovery action, for example Retry.
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }
}

impl RenderOnce for ChatNotice {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        match self.kind {
            NoticeKind::Status => div()
                .flex()
                .items_center()
                .gap(theme.spacing.sm)
                .child(div().flex_1().h(px(1.)).bg(colors.separator))
                .child(
                    Label::new(self.text)
                        .size(TextSize::XSmall)
                        .color(TextColor::Faint),
                )
                .child(div().flex_1().h(px(1.)).bg(colors.separator)),
            NoticeKind::Error => div()
                .flex()
                .items_center()
                .gap(theme.spacing.sm)
                .px(theme.spacing.md)
                .py(theme.spacing.sm)
                .rounded(theme.shape.radius_md)
                .bg(colors.danger_wash)
                .child(
                    Icon::new(IconName::Alert)
                        .size(px(14.))
                        .color(colors.danger),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(theme.typography.size_sm)
                        .text_color(colors.text)
                        .child(self.text),
                )
                .children(self.action.map(|action| div().flex_none().child(action))),
        }
    }
}

/// Three pulsing dots and a phase word, while the model has not yet produced text.
#[derive(IntoElement)]
pub struct ThinkingIndicator {
    label: SharedString,
    logo: Option<IconSource>,
}

impl ThinkingIndicator {
    /// An indicator reading, for example, "Thinking" or "Reading the score".
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            logo: None,
        }
    }

    /// The provider's mark in place of the agent's gradient, as on [`AssistantMessage::logo`].
    pub fn logo(mut self, logo: impl Into<IconSource>) -> Self {
        self.logo = Some(logo.into());
        self
    }
}

impl RenderOnce for ThinkingIndicator {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let color = theme.colors.agent;
        div()
            .flex()
            .items_center()
            .gap(theme.spacing.sm)
            .child(agent_avatar(self.logo).size(px(20.)))
            .child(div().flex().gap(px(3.)).children((0..3usize).map(|index| {
                looping(
                    div().size(px(5.)).rounded(px(2.5)).bg(color),
                    ("thinking-dot", index),
                    Duration::from_millis(1200),
                    0.25,
                    cx,
                    move |dot, delta| {
                        let phase = (delta - index as f32 * 0.18).rem_euclid(1.);
                        let pulse = (phase * std::f32::consts::TAU).sin() * 0.5 + 0.5;
                        dot.opacity(0.25 + 0.75 * pulse)
                    },
                )
            })))
            .child(
                Label::new(self.label)
                    .size(TextSize::Small)
                    .color(TextColor::Muted),
            )
    }
}

/// The agent's avatar: the provider's mark when the application gave one, else the gradient.
pub(super) fn agent_avatar(logo: Option<IconSource>) -> Avatar {
    logo.map_or_else(Avatar::agent, Avatar::logo)
}
