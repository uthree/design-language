//! The conversation browser: past conversations, searchable, with a way to start a new one.

use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, ElementId, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder,
};

use super::AgentStatus;
use super::panel::ConversationRow;
use crate::button::{Button, ButtonVariant, ControlSize};
use crate::controls::{Segment, SegmentedControl};
use crate::icon::IconName;
use crate::layout::EmptyState;
use crate::strings::ActiveStrings;
use crate::theme::ActiveTheme;

/// One conversation in a [`ConversationList`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conversation {
    /// Its title; empty for one not named yet.
    pub title: SharedString,
    /// The start of its last message.
    pub preview: SharedString,
    /// When it was last used, already worded: "2 min", "Yesterday".
    pub when: SharedString,
    /// Whether a turn is running or waiting in it.
    pub status: AgentStatus,
}

impl Conversation {
    /// A conversation.
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            preview: SharedString::default(),
            when: SharedString::default(),
            status: AgentStatus::Idle,
        }
    }

    /// The start of its last message.
    pub fn preview(mut self, preview: impl Into<SharedString>) -> Self {
        self.preview = preview.into();
        self
    }

    /// When it was last used, already worded.
    pub fn when(mut self, when: impl Into<SharedString>) -> Self {
        self.when = when.into();
        self
    }

    /// A turn is running or waiting for an approval in it.
    pub fn status(mut self, status: AgentStatus) -> Self {
        self.status = status;
        self
    }
}

type IndexHandler = Rc<dyn Fn(&usize, &mut Window, &mut App)>;
type FlagHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;
type NewHandler = Rc<dyn Fn(&mut Window, &mut App)>;
type MoreHandler = Rc<dyn Fn(&usize, &ClickEvent, &mut Window, &mut App)>;

/// The conversations an agent app keeps, as the panel's history page or a sidebar: a "New
/// conversation" button, a search field, Recent and Archived, and a [`ConversationRow`] per
/// conversation with its title, when it was used, a line of its last message and a "More
/// actions" button for the application's menu.
///
/// gpui has no text input, so the application passes the search field and its text with
/// [`ConversationList::search`]; the list then shows only the conversations whose title or
/// preview has every word, and says so when none has. Callbacks get the conversation's index
/// in what was passed, not in what is shown.
///
/// Selectors: `{id}-new`, `{id}-row-{i}`, `{id}-row-{i}-more`, `{id}-archived`.
#[derive(IntoElement)]
pub struct ConversationList {
    id: ElementId,
    conversations: Vec<Conversation>,
    selected: Option<usize>,
    search: Option<(SharedString, AnyElement)>,
    archived: Option<bool>,
    on_archived: Option<FlagHandler>,
    on_open: Option<IndexHandler>,
    on_new: Option<NewHandler>,
    on_more: Option<MoreHandler>,
}

impl ConversationList {
    /// An empty list.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            conversations: Vec::new(),
            selected: None,
            search: None,
            archived: None,
            on_archived: None,
            on_open: None,
            on_new: None,
            on_more: None,
        }
    }

    /// Add a conversation, most recent first.
    pub fn conversation(mut self, conversation: Conversation) -> Self {
        self.conversations.push(conversation);
        self
    }

    /// Add conversations, most recent first.
    pub fn conversations(mut self, conversations: impl IntoIterator<Item = Conversation>) -> Self {
        self.conversations.extend(conversations);
        self
    }

    /// The conversation open now.
    pub fn selected(mut self, index: Option<usize>) -> Self {
        self.selected = index;
        self
    }

    /// The search field, drawn above the list, and the text in it.
    pub fn search(mut self, query: impl Into<SharedString>, field: impl IntoElement) -> Self {
        self.search = Some((query.into(), field.into_any_element()));
        self
    }

    /// Offer Recent and Archived, showing `archived`; the handler gets the new choice.
    pub fn archived(
        mut self,
        archived: bool,
        handler: impl Fn(&bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.archived = Some(archived);
        self.on_archived = Some(Rc::new(handler));
        self
    }

    /// Called with a conversation's index when the person opens it.
    pub fn on_open(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_open = Some(Rc::new(handler));
        self
    }

    /// Show "New conversation", which calls this.
    pub fn on_new(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_new = Some(Rc::new(handler));
        self
    }

    /// Give each row a "More actions" button, which calls this with the conversation's index
    /// and the click, whose position is where to open a menu.
    pub fn on_more(
        mut self,
        handler: impl Fn(&usize, &ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_more = Some(Rc::new(handler));
        self
    }

    fn child(&self, part: impl Into<SharedString>) -> ElementId {
        ElementId::NamedChild(Box::new(self.id.clone()), part.into())
    }
}

impl RenderOnce for ConversationList {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let strings = cx.strings();
        let query = self
            .search
            .as_ref()
            .map(|(query, _)| query.trim().to_owned())
            .unwrap_or_default();
        let shown: Vec<usize> = (0..self.conversations.len())
            .filter(|&index| {
                let conversation = &self.conversations[index];
                crate::preferences::matches(&query, [&conversation.title, &conversation.preview])
            })
            .collect();
        let rows: Vec<AnyElement> = shown
            .iter()
            .map(|&index| {
                let conversation = &self.conversations[index];
                let title = if conversation.title.is_empty() {
                    strings.untitled.clone()
                } else {
                    conversation.title.clone()
                };
                let mut row = ConversationRow::new(
                    self.child(format!("row-{index}")),
                    title,
                    conversation.when.clone(),
                )
                .status(conversation.status)
                .selected(self.selected == Some(index));
                if !conversation.preview.is_empty() {
                    row = row.preview(conversation.preview.clone());
                }
                if let Some(on_open) = self.on_open.clone() {
                    row = row.on_click(move |_, window, cx| on_open(&index, window, cx));
                }
                if let Some(on_more) = self.on_more.clone() {
                    row = row.on_more(move |event, window, cx| on_more(&index, event, window, cx));
                }
                row.into_any_element()
            })
            .collect();
        let empty = rows.is_empty().then(|| {
            if query.is_empty() {
                EmptyState::new(IconName::History, strings.no_conversations.clone())
            } else {
                EmptyState::new(IconName::Search, strings.nothing_matches(&query))
            }
        });
        let new = self.on_new.clone().map(|on_new| {
            Button::new(self.child("new"), strings.new_conversation.clone())
                .icon(IconName::Plus)
                .size(ControlSize::Small)
                .variant(ButtonVariant::Secondary)
                .on_click(move |_, window, cx| on_new(window, cx))
        });
        let archived = self.archived.map(|archived| {
            let on_archived = self.on_archived.clone();
            SegmentedControl::new(
                self.child("archived"),
                [
                    Segment::label(strings.recent.clone()),
                    Segment::label(strings.archived.clone()),
                ],
                usize::from(archived),
            )
            .size(ControlSize::Small)
            .on_select(move |index, window, cx| {
                if let Some(on_archived) = &on_archived {
                    on_archived(&(*index == 1), window, cx);
                }
            })
        });
        let list_id = self.child("list");
        let field = self.search.map(|(_, field)| field);
        crate::focus::selector(div().id(self.id.clone()), &self.id)
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .min_h_0()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.spacing.sm)
                    .p(theme.spacing.sm)
                    .when_some(new, |header, new| header.child(div().flex().child(new)))
                    .children(field)
                    .children(archived),
            )
            .child(
                div()
                    .id(list_id)
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .gap(theme.spacing.xxs)
                    .px(theme.spacing.xs)
                    .pb(theme.spacing.sm)
                    .overflow_y_scroll()
                    .children(rows)
                    .children(empty),
            )
    }
}
