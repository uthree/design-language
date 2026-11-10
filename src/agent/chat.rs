//! The conversation as one view: the entries, their state, and the scrolling that follows them.

use std::collections::HashSet;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, ElementId, EventEmitter, InteractiveElement, IntoElement,
    ParentElement, Pixels, Render, ScrollHandle, SharedString, StatefulInteractiveElement, Styled,
    Window, div, prelude::FluentBuilder, px,
};

use super::choice::{Choice, ChoiceCard};
use super::composer::ContextChip;
use super::transcript::{
    ApprovalCard, AssistantMessage, ChatNotice, NoticeKind, ReasoningBlock, ThinkingIndicator,
    ToolCall, ToolGroup, ToolState, UserMessage,
};
use crate::button::{Button, ControlSize, CopyButton};
use crate::icon::{Icon, IconName, IconSource};
use crate::selectable::SelectionScope;
use crate::strings::ActiveStrings;
use crate::text::{Label, TextColor, TextSize};
use crate::theme::ActiveTheme;

/// A stable name for one entry of a [`ChatView`], handed out by [`ChatView::push`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntryId(u64);

/// The person's answer to an approval.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Allow this once.
    Allow,
    /// Allow this and everything like it from now on.
    AlwaysAllow,
    /// Refuse it.
    Deny,
}

/// A row an application draws itself, for what the built-in entries do not cover: a diff, a
/// plan, a card of choices.
pub type CustomEntry = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// One entry of the conversation, as plain values. Build one with the constructors and
/// [`ChatView::push`] it; change it later through its [`EntryId`].
#[derive(Clone)]
pub enum ChatEntry {
    /// What the person said, with the files or context sent with it.
    User {
        /// Message text, exactly as typed.
        text: SharedString,
        /// Labels of the attached context, drawn as chips above the bubble.
        attachments: Vec<SharedString>,
    },
    /// What the agent answered.
    Assistant {
        /// The answer so far.
        text: String,
        /// Muted metadata: the model, the time taken.
        meta: Option<SharedString>,
        /// Still arriving.
        streaming: bool,
    },
    /// The model's reasoning, collapsed to its summary until opened.
    Reasoning {
        /// One line such as "Thought for 8 s".
        summary: SharedString,
        /// The reasoning text.
        text: String,
    },
    /// One tool call. Consecutive calls are drawn as one group.
    Tool {
        /// The tool's wire name.
        name: SharedString,
        /// What it did, in words.
        summary: SharedString,
        /// Where it is.
        state: ToolState,
        /// Arguments and result, shown when the row is opened.
        detail: Option<String>,
        /// The tool's icon.
        icon: IconName,
    },
    /// A paused turn asking the person to allow an action. Answering it emits
    /// [`ChatEvent::Decided`] and folds the card into one line.
    Approval {
        /// The verb and the object: `Run "cargo test"?`.
        title: SharedString,
        /// Scope, and whether it can be undone.
        body: Option<SharedString>,
        /// The allowing button's verb.
        allow_label: Option<SharedString>,
        /// The action destroys work.
        destructive: bool,
        /// Offer "Always allow".
        offer_always: bool,
        /// The answer, once given.
        answer: Option<Decision>,
    },
    /// A question the agent asked with fixed answers, drawn as a [`ChoiceCard`]. Picking emits
    /// [`ChatEvent::Chose`]; the card then shows the pick and takes no more. Once the person
    /// sends a message after it instead, it stops taking picks too.
    Choices {
        /// The question.
        question: SharedString,
        /// The answers.
        options: Vec<Choice>,
        /// Several answers may be ticked, then sent together.
        multiple: bool,
        /// The ticked answers.
        picked: Vec<usize>,
        /// The pick was sent.
        answered: bool,
    },
    /// A line from the runtime rather than either speaker.
    Notice {
        /// Status or error.
        kind: NoticeKind,
        /// What happened.
        text: SharedString,
    },
    /// A row the application draws.
    Custom(CustomEntry),
}

impl ChatEntry {
    /// What the person said.
    pub fn user(text: impl Into<SharedString>) -> Self {
        Self::User {
            text: text.into(),
            attachments: Vec::new(),
        }
    }

    /// A finished answer.
    pub fn assistant(text: impl Into<String>) -> Self {
        Self::Assistant {
            text: text.into(),
            meta: None,
            streaming: false,
        }
    }

    /// An answer that is about to arrive; [`ChatView::append`] adds to it and
    /// [`ChatView::finish`] ends it.
    pub fn streaming() -> Self {
        Self::Assistant {
            text: String::new(),
            meta: None,
            streaming: true,
        }
    }

    /// The model's reasoning.
    pub fn reasoning(summary: impl Into<SharedString>, text: impl Into<String>) -> Self {
        Self::Reasoning {
            summary: summary.into(),
            text: text.into(),
        }
    }

    /// A tool call, for example `bash` / "Ran the tests".
    pub fn tool(
        name: impl Into<SharedString>,
        summary: impl Into<SharedString>,
        state: ToolState,
    ) -> Self {
        Self::Tool {
            name: name.into(),
            summary: summary.into(),
            state,
            detail: None,
            icon: IconName::Wrench,
        }
    }

    /// An approval request.
    pub fn approval(title: impl Into<SharedString>) -> Self {
        Self::Approval {
            title: title.into(),
            body: None,
            allow_label: None,
            destructive: false,
            offer_always: false,
            answer: None,
        }
    }

    /// A status line from the runtime: "Context compacted".
    pub fn status(text: impl Into<SharedString>) -> Self {
        Self::Notice {
            kind: NoticeKind::Status,
            text: text.into(),
        }
    }

    /// An error line. Say what happened and how to recover.
    pub fn error(text: impl Into<SharedString>) -> Self {
        Self::Notice {
            kind: NoticeKind::Error,
            text: text.into(),
        }
    }

    /// A row drawn by the application.
    pub fn custom(render: impl Fn(&mut Window, &mut App) -> AnyElement + 'static) -> Self {
        Self::Custom(Rc::new(render))
    }

    /// Attach context to a user entry; other entries are left as they are.
    pub fn attachment(mut self, label: impl Into<SharedString>) -> Self {
        if let Self::User { attachments, .. } = &mut self {
            attachments.push(label.into());
        }
        self
    }

    /// Metadata on an answer; other entries are left as they are.
    pub fn meta(mut self, value: impl Into<SharedString>) -> Self {
        if let Self::Assistant { meta, .. } = &mut self {
            *meta = Some(value.into());
        }
        self
    }

    /// A tool call's arguments and result; other entries are left as they are.
    pub fn detail(mut self, value: impl Into<String>) -> Self {
        if let Self::Tool { detail, .. } = &mut self {
            *detail = Some(value.into());
        }
        self
    }

    /// A tool call's icon; other entries are left as they are.
    pub fn icon(mut self, value: IconName) -> Self {
        if let Self::Tool { icon, .. } = &mut self {
            *icon = value;
        }
        self
    }

    /// An approval's scope and consequence; other entries are left as they are.
    pub fn body(mut self, value: impl Into<SharedString>) -> Self {
        if let Self::Approval { body, .. } = &mut self {
            *body = Some(value.into());
        }
        self
    }

    /// An approval's allowing verb, for example "Run"; other entries are left as they are.
    pub fn allow_label(mut self, value: impl Into<SharedString>) -> Self {
        if let Self::Approval { allow_label, .. } = &mut self {
            *allow_label = Some(value.into());
        }
        self
    }

    /// A question with fixed answers. Make it take several with [`ChatEntry::multiple`].
    pub fn choices<C: Into<Choice>>(
        question: impl Into<SharedString>,
        options: impl IntoIterator<Item = C>,
    ) -> Self {
        Self::Choices {
            question: question.into(),
            options: options.into_iter().map(Into::into).collect(),
            multiple: false,
            picked: Vec::new(),
            answered: false,
        }
    }

    /// Let a question with fixed answers take several; other entries are left as they are.
    pub fn multiple(mut self) -> Self {
        if let Self::Choices { multiple, .. } = &mut self {
            *multiple = true;
        }
        self
    }

    /// Draw an approval's allowing button as danger; other entries are left as they are.
    pub fn destructive(mut self) -> Self {
        if let Self::Approval { destructive, .. } = &mut self {
            *destructive = true;
        }
        self
    }

    /// Offer "Always allow" on an approval; other entries are left as they are.
    pub fn offer_always(mut self) -> Self {
        if let Self::Approval { offer_always, .. } = &mut self {
            *offer_always = true;
        }
        self
    }

    /// The entry's text, as a transcript copy would hold it. Custom rows have none.
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::User { text, .. } | Self::Notice { text, .. } => Some(text),
            Self::Assistant { text, .. } | Self::Reasoning { text, .. } => Some(text),
            Self::Tool { summary, .. } => Some(summary),
            Self::Approval { title, .. } => Some(title),
            Self::Choices { question, .. } => Some(question),
            Self::Custom(_) => None,
        }
    }
}

/// What a [`ChatView`] tells the application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatEvent {
    /// The person answered an approval.
    Decided {
        /// The approval entry.
        entry: EntryId,
        /// Their answer.
        decision: Decision,
    },
    /// The person answered a question with fixed answers.
    Chose {
        /// The question's entry.
        entry: EntryId,
        /// The indices of the picked answers, in order.
        picked: Vec<usize>,
    },
}

/// How close to the end, in pixels, still counts as reading the latest entry.
const TAIL_SLACK: f32 = 64.;

/// The conversation as a view the application holds: it keeps the entries, which rows are
/// open, and where the person is reading, so an application only says what happened.
///
/// It scrolls with the conversation while the person is at its end and stays put once they
/// scroll up to read, counting what arrived meanwhile on a "jump to latest" button. Answers
/// stream in with [`ChatView::append`]; tool calls change state with
/// [`ChatView::set_tool_state`]; approvals report back as [`ChatEvent::Decided`].
///
/// It is the body of [`super::AgentPanel::chat`] in a side panel, and with
/// [`ChatView::reading_width`] the middle of a window.
///
/// ```ignore
/// let chat = cx.new(|_| ChatView::new("chat"));
/// cx.subscribe(&chat, |app, _, event: &ChatEvent, cx| { /* answer the agent */ }).detach();
/// chat.update(cx, |chat, cx| {
///     chat.push(ChatEntry::user("Why does the test fail?"), cx);
///     let answer = chat.push(ChatEntry::streaming(), cx);
///     chat.append(answer, "Found it.", cx);
///     chat.finish(answer, cx);
/// });
/// ```
pub struct ChatView {
    id: ElementId,
    entries: Vec<(EntryId, ChatEntry)>,
    next_id: u64,
    open: HashSet<EntryId>,
    scroll: ScrollHandle,
    follow_tail: bool,
    unread: usize,
    thinking: Option<SharedString>,
    logo: Option<IconSource>,
    anonymous: bool,
    copy_buttons: bool,
    reading_width: Option<Pixels>,
    capacity: Option<usize>,
    empty: Option<CustomEntry>,
}

impl EventEmitter<ChatEvent> for ChatView {}

impl ChatView {
    /// An empty conversation. `id` keeps its rows' element ids apart from other views'.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            entries: Vec::new(),
            next_id: 0,
            open: HashSet::new(),
            scroll: ScrollHandle::new(),
            follow_tail: true,
            unread: 0,
            thinking: None,
            logo: None,
            anonymous: false,
            copy_buttons: true,
            reading_width: None,
            capacity: None,
            empty: None,
        }
    }

    /// Centre the conversation in a column at most `width` wide, for a conversation that fills
    /// the middle of a window rather than a side panel.
    pub fn reading_width(mut self, width: Pixels) -> Self {
        self.reading_width = Some(width);
        self
    }

    /// Keep at most `count` entries, dropping the oldest, for a conversation that only ever
    /// grows and is saved elsewhere.
    pub fn capacity(mut self, count: usize) -> Self {
        self.capacity = Some(count.max(1));
        self
    }

    /// Leave out the answers' identity line; see [`AssistantMessage::anonymous`].
    pub fn anonymous(mut self) -> Self {
        self.anonymous = true;
        self
    }

    /// Leave out the copy buttons under messages.
    pub fn without_copy_buttons(mut self) -> Self {
        self.copy_buttons = false;
        self
    }

    /// What to show while the conversation is empty.
    pub fn empty_state(
        mut self,
        render: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        self.empty = Some(Rc::new(render));
        self
    }

    /// Draw the answers' avatar as the provider's mark of the model now answering.
    pub fn set_logo(&mut self, logo: Option<IconSource>, cx: &mut Context<Self>) {
        self.logo = logo;
        cx.notify();
    }

    /// Show the thinking indicator at the end with this phase word, or hide it with `None`.
    pub fn set_thinking(&mut self, label: Option<SharedString>, cx: &mut Context<Self>) {
        let follow = self.reading_tail();
        self.thinking = label;
        self.changed(follow, false, cx);
    }

    /// Add an entry at the end and name it.
    pub fn push(&mut self, entry: ChatEntry, cx: &mut Context<Self>) -> EntryId {
        let follow = self.reading_tail();
        let id = EntryId(self.next_id);
        self.next_id += 1;
        self.entries.push((id, entry));
        if let Some(capacity) = self.capacity {
            let excess = self.entries.len().saturating_sub(capacity);
            for (gone, _) in self.entries.drain(..excess) {
                self.open.remove(&gone);
            }
        }
        self.changed(follow, true, cx);
        id
    }

    /// Replace every entry, as when another conversation is opened, and show its end.
    pub fn replace(
        &mut self,
        entries: impl IntoIterator<Item = ChatEntry>,
        cx: &mut Context<Self>,
    ) {
        self.entries.clear();
        self.open.clear();
        self.thinking = None;
        // The old conversation's scroll position means nothing in the new one.
        self.scroll = ScrollHandle::new();
        for entry in entries {
            let id = EntryId(self.next_id);
            self.next_id += 1;
            self.entries.push((id, entry));
        }
        self.jump_to_latest(cx);
    }

    /// Remove every entry.
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.replace([], cx);
    }

    /// Remove one entry.
    pub fn remove(&mut self, id: EntryId, cx: &mut Context<Self>) {
        self.entries.retain(|(entry, _)| *entry != id);
        self.open.remove(&id);
        cx.notify();
    }

    /// The entries, oldest first.
    pub fn entries(&self) -> impl Iterator<Item = (EntryId, &ChatEntry)> {
        self.entries.iter().map(|(id, entry)| (*id, entry))
    }

    /// One entry.
    pub fn entry(&self, id: EntryId) -> Option<&ChatEntry> {
        self.entries
            .iter()
            .find_map(|(entry, value)| (*entry == id).then_some(value))
    }

    /// Change one entry in place. Returns whether it was there.
    pub fn update_entry(
        &mut self,
        id: EntryId,
        change: impl FnOnce(&mut ChatEntry),
        cx: &mut Context<Self>,
    ) -> bool {
        let follow = self.reading_tail();
        let Some((_, entry)) = self.entries.iter_mut().find(|(entry, _)| *entry == id) else {
            return false;
        };
        change(entry);
        self.changed(follow, false, cx);
        true
    }

    /// Add streamed text to an answer or reasoning, or to a tool call's detail.
    pub fn append(&mut self, id: EntryId, delta: &str, cx: &mut Context<Self>) {
        self.update_entry(
            id,
            |entry| match entry {
                ChatEntry::Assistant { text, .. } | ChatEntry::Reasoning { text, .. } => {
                    text.push_str(delta)
                }
                ChatEntry::Tool { detail, .. } => detail.get_or_insert_default().push_str(delta),
                _ => {}
            },
            cx,
        );
    }

    /// End a streaming answer.
    pub fn finish(&mut self, id: EntryId, cx: &mut Context<Self>) {
        self.update_entry(
            id,
            |entry| {
                if let ChatEntry::Assistant { streaming, .. } = entry {
                    *streaming = false;
                }
            },
            cx,
        );
    }

    /// Move a tool call on, for example from running to succeeded.
    pub fn set_tool_state(&mut self, id: EntryId, state: ToolState, cx: &mut Context<Self>) {
        self.update_entry(
            id,
            |entry| {
                if let ChatEntry::Tool { state: current, .. } = entry {
                    *current = state;
                }
            },
            cx,
        );
    }

    /// Record the answer to an approval without the person clicking, for example when the
    /// runtime decided it by rule.
    pub fn answer(&mut self, id: EntryId, decision: Decision, cx: &mut Context<Self>) {
        self.update_entry(
            id,
            |entry| {
                if let ChatEntry::Approval { answer, .. } = entry {
                    *answer = Some(decision);
                }
            },
            cx,
        );
    }

    /// Whether a question with fixed answers still takes a pick: not answered, and not passed
    /// over by a message the person sent after it.
    pub fn choices_open(&self, id: EntryId) -> bool {
        let Some(at) = self.entries.iter().position(|(entry, _)| *entry == id) else {
            return false;
        };
        matches!(
            self.entries[at].1,
            ChatEntry::Choices {
                answered: false,
                ..
            }
        ) && !self.entries[at + 1..]
            .iter()
            .any(|(_, entry)| matches!(entry, ChatEntry::User { .. }))
    }

    /// Record the answer to a question with fixed answers, as if the person had picked it.
    /// Emits nothing.
    pub fn choose(&mut self, id: EntryId, chosen: Vec<usize>, cx: &mut Context<Self>) {
        if let Some((
            _,
            ChatEntry::Choices {
                picked, answered, ..
            },
        )) = self.entries.iter_mut().find(|(entry, _)| *entry == id)
        {
            *picked = chosen;
            *answered = true;
            cx.notify();
        }
    }

    /// Whether an approval is still waiting for the person.
    pub fn awaiting_approval(&self) -> bool {
        self.entries
            .iter()
            .any(|(_, entry)| matches!(entry, ChatEntry::Approval { answer: None, .. }))
    }

    /// Open or close a tool call or reasoning row.
    pub fn set_open(&mut self, id: EntryId, open: bool, cx: &mut Context<Self>) {
        if open {
            self.open.insert(id);
        } else {
            self.open.remove(&id);
        }
        cx.notify();
    }

    /// Whether a tool call or reasoning row is open.
    pub fn is_open(&self, id: EntryId) -> bool {
        self.open.contains(&id)
    }

    /// How many entries arrived while the person was reading further up.
    pub fn unread(&self) -> usize {
        self.unread
    }

    /// Scroll to the end and follow the conversation again.
    pub fn jump_to_latest(&mut self, cx: &mut Context<Self>) {
        self.follow_tail = true;
        self.unread = 0;
        self.scroll.scroll_to_bottom();
        cx.notify();
    }

    /// The whole conversation as text, for copying or a bug report.
    pub fn transcript_text(&self) -> String {
        let mut out = String::new();
        for (_, entry) in &self.entries {
            let line = match entry {
                ChatEntry::User { text, .. } => format!("> {text}"),
                ChatEntry::Tool { name, summary, .. } => format!("[{name}] {summary}"),
                other => match other.text() {
                    Some(text) => text.to_owned(),
                    None => continue,
                },
            };
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&line);
        }
        out
    }

    /// Whether the person is at the end, or near enough to keep following it.
    fn reading_tail(&self) -> bool {
        let remaining =
            f32::from(self.scroll.max_offset().height) + f32::from(self.scroll.offset().y);
        self.follow_tail && remaining <= TAIL_SLACK || remaining <= 1.
    }

    /// After a change: follow the end if the person was there, else count a new entry.
    fn changed(&mut self, follow: bool, new_entry: bool, cx: &mut Context<Self>) {
        self.follow_tail = follow;
        if follow {
            self.unread = 0;
            self.scroll.scroll_to_bottom();
        } else if new_entry {
            self.unread += 1;
        }
        cx.notify();
    }

    fn child_id(&self, entry: EntryId, part: &'static str) -> ElementId {
        ElementId::NamedChild(
            Box::new(self.id.clone()),
            format!("{part}-{}", entry.0).into(),
        )
    }

    fn toggle(
        id: EntryId,
        cx: &mut Context<Self>,
    ) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static + use<> {
        let view = cx.entity().downgrade();
        move |_, _, cx| {
            view.update(cx, |this, cx| {
                let open = !this.open.contains(&id);
                this.set_open(id, open, cx);
            })
            .ok();
        }
    }

    fn decide(
        id: EntryId,
        decision: Decision,
        cx: &mut Context<Self>,
    ) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static + use<> {
        let view = cx.entity().downgrade();
        move |_, _, cx| {
            view.update(cx, |this, cx| {
                this.answer(id, decision, cx);
                cx.emit(ChatEvent::Decided {
                    entry: id,
                    decision,
                });
            })
            .ok();
        }
    }

    fn tool_row(&self, id: EntryId, entry: &ChatEntry, cx: &mut Context<Self>) -> AnyElement {
        let ChatEntry::Tool {
            name,
            summary,
            state,
            detail,
            icon,
        } = entry
        else {
            return div().into_any_element();
        };
        let mut call = ToolCall::new(
            self.child_id(id, "tool"),
            name.clone(),
            summary.clone(),
            *state,
        )
        .icon(*icon)
        .open(self.open.contains(&id));
        if let Some(detail) = detail {
            call = call.detail(detail.clone()).on_toggle(Self::toggle(id, cx));
        }
        call.into_any_element()
    }

    fn row(
        &self,
        id: EntryId,
        entry: &ChatEntry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match entry {
            ChatEntry::User { text, attachments } => {
                let mut message = UserMessage::new(text.clone()).id(self.child_id(id, "user"));
                for (index, label) in attachments.iter().enumerate() {
                    message = message.attachment(ContextChip::new(
                        ElementId::NamedChild(
                            Box::new(self.child_id(id, "attachment")),
                            index.to_string().into(),
                        ),
                        IconName::File,
                        label.clone(),
                    ));
                }
                if self.copy_buttons {
                    message =
                        message.action(CopyButton::new(self.child_id(id, "copy"), text.clone()));
                }
                message.into_any_element()
            }
            ChatEntry::Assistant {
                text,
                meta,
                streaming,
            } => {
                let mut message = AssistantMessage::new(text.clone())
                    .id(self.child_id(id, "answer"))
                    .streaming(*streaming);
                if let Some(meta) = meta {
                    message = message.meta(meta.clone());
                }
                if let Some(logo) = self.logo.clone() {
                    message = message.logo(logo);
                }
                if self.anonymous {
                    message = message.anonymous();
                }
                if self.copy_buttons && !*streaming {
                    message =
                        message.action(CopyButton::new(self.child_id(id, "copy"), text.clone()));
                }
                message.into_any_element()
            }
            ChatEntry::Reasoning { summary, text } => ReasoningBlock::new(
                self.child_id(id, "reasoning"),
                summary.clone(),
                text.clone(),
            )
            .open(self.open.contains(&id))
            .on_toggle(Self::toggle(id, cx))
            .into_any_element(),
            ChatEntry::Tool { .. } => self.tool_row(id, entry, cx),
            ChatEntry::Approval {
                title,
                body,
                allow_label,
                destructive,
                offer_always,
                answer,
            } => match answer {
                Some(decision) => answered(title.clone(), *decision, cx).into_any_element(),
                None => {
                    let decide =
                        |decision: Decision, cx: &mut Context<Self>| Self::decide(id, decision, cx);
                    let mut card = ApprovalCard::new(self.child_id(id, "approval"), title.clone())
                        .on_allow(decide(Decision::Allow, cx))
                        .on_deny(decide(Decision::Deny, cx));
                    if let Some(body) = body {
                        card = card.body(body.clone());
                    }
                    if let Some(label) = allow_label {
                        card = card.allow_label(label.clone());
                    }
                    if *destructive {
                        card = card.destructive();
                    }
                    if *offer_always {
                        card = card.on_always(decide(Decision::AlwaysAllow, cx));
                    }
                    card.into_any_element()
                }
            },
            ChatEntry::Choices {
                question,
                options,
                multiple,
                picked,
                ..
            } => {
                let open = self.choices_open(id);
                let card = ChoiceCard::new(self.child_id(id, "choices"), question.clone())
                    .options(options.iter().cloned())
                    .multiple(*multiple);
                if !open {
                    return card.answered(picked.clone()).into_any_element();
                }
                let view = cx.entity().downgrade();
                let toggle = view.clone();
                card.picked(picked.clone())
                    .on_toggle(move |index, _, cx| {
                        toggle
                            .update(cx, |this, cx| {
                                if let Some((_, ChatEntry::Choices { picked, .. })) =
                                    this.entries.iter_mut().find(|(entry, _)| *entry == id)
                                {
                                    match picked.iter().position(|ticked| ticked == index) {
                                        Some(at) => {
                                            picked.remove(at);
                                        }
                                        None => picked.push(*index),
                                    }
                                    cx.notify();
                                }
                            })
                            .ok();
                    })
                    .on_choose(move |chosen, _, cx| {
                        view.update(cx, |this, cx| {
                            this.choose(id, chosen.clone(), cx);
                            cx.emit(ChatEvent::Chose {
                                entry: id,
                                picked: chosen.clone(),
                            });
                        })
                        .ok();
                    })
                    .into_any_element()
            }
            ChatEntry::Notice { kind, text } => match kind {
                NoticeKind::Status => ChatNotice::status(text.clone()),
                NoticeKind::Error => ChatNotice::error(text.clone()),
            }
            .id(self.child_id(id, "notice"))
            .into_any_element(),
            ChatEntry::Custom(render) => render(window, cx),
        }
    }

    fn rows(&self, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut rows = Vec::new();
        let mut index = 0;
        while index < self.entries.len() {
            let (id, entry) = &self.entries[index];
            if matches!(entry, ChatEntry::Tool { .. }) {
                let run = self.entries[index..]
                    .iter()
                    .take_while(|(_, entry)| matches!(entry, ChatEntry::Tool { .. }))
                    .count();
                if run > 1 {
                    let title = cx.strings().used_tools.replace("{}", &run.to_string());
                    let mut group = ToolGroup::new(title);
                    for (id, entry) in &self.entries[index..index + run] {
                        group = group.call(self.tool_row(*id, entry, cx));
                    }
                    rows.push(group.into_any_element());
                    index += run;
                    continue;
                }
            }
            rows.push(self.row(*id, entry, window, cx));
            index += 1;
        }
        rows
    }
}

/// An approval once answered: one quiet line that keeps the question and the answer.
fn answered(title: SharedString, decision: Decision, cx: &App) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let colors = &theme.colors;
    let answers = &cx.strings().approval_answers;
    let (icon, color, word) = match decision {
        Decision::Allow => (IconName::Check, colors.success, answers[0].clone()),
        Decision::AlwaysAllow => (IconName::Check, colors.success, answers[1].clone()),
        Decision::Deny => (IconName::Close, colors.text_muted, answers[2].clone()),
    };
    div()
        .flex()
        .items_center()
        .gap(theme.spacing.sm)
        .px(theme.spacing.sm)
        .min_w_0()
        .child(Icon::new(icon).size(px(12.)).color(color))
        .child(
            Label::new(word)
                .size(TextSize::Small)
                .color(TextColor::Muted)
                .weight(theme.typography.label_weight),
        )
        .child(
            div().min_w_0().truncate().child(
                Label::new(title)
                    .size(TextSize::Small)
                    .color(TextColor::Faint),
            ),
        )
}

impl Render for ChatView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self.rows(window, cx);
        let empty = self.entries.is_empty() && self.thinking.is_none();
        let empty_state = self
            .empty
            .clone()
            .filter(|_| empty)
            .map(|render| render(window, cx));
        let thinking = self.thinking.clone().map(|label| {
            let mut indicator = ThinkingIndicator::new(label);
            if let Some(logo) = self.logo.clone() {
                indicator = indicator.logo(logo);
            }
            indicator
        });
        let away = !self.reading_tail() && !empty;
        let theme = cx.theme();
        let strings = cx.strings();
        let jump_label = if self.unread > 0 {
            strings
                .new_messages
                .replace("{}", &self.unread.to_string())
                .into()
        } else {
            strings.jump_to_latest.clone()
        };
        // One selection over the whole transcript: a drag runs from a question through the
        // answer, its code and tool output into the next message, and scrolls on past the edge.
        let column = SelectionScope::new((self.id.clone(), "selection"))
            .scroll(&self.scroll)
            .flex()
            .flex_col()
            .w_full()
            .gap(theme.spacing.lg)
            .when_some(self.reading_width, |column, width| column.max_w(width))
            .children(rows)
            .children(thinking)
            .children(empty_state);
        let wide = self.reading_width.is_some();
        div()
            .id(self.id.clone())
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .size_full()
            .min_h_0()
            .child(
                div()
                    .id((self.id.clone(), "scroll"))
                    .flex()
                    .flex_col()
                    .items_center()
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    // The jump button shows and hides with where the person is reading.
                    .on_scroll_wheel(cx.listener(|this, _, _, cx| {
                        this.follow_tail = this.reading_tail();
                        if this.follow_tail {
                            this.unread = 0;
                        }
                        cx.notify();
                    }))
                    .px(if wide {
                        theme.spacing.xl
                    } else {
                        theme.density.panel_padding
                    })
                    .py(if wide {
                        theme.spacing.xl
                    } else {
                        theme.spacing.lg
                    })
                    .child(column),
            )
            .when(away, |view| {
                view.child(
                    div()
                        .absolute()
                        .bottom(theme.spacing.md)
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        // It floats over the transcript, so it casts the shadow a popover does.
                        .child(
                            div()
                                .rounded(theme.shape.radius_md)
                                .shadow(theme.elevation.medium.clone())
                                .child(
                                    Button::new((self.id.clone(), "jump"), jump_label)
                                        .size(ControlSize::Small)
                                        .icon(IconName::ChevronDown)
                                        .on_click(
                                            cx.listener(|this, _, _, cx| this.jump_to_latest(cx)),
                                        ),
                                ),
                        ),
                )
            })
    }
}
