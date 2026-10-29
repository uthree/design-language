//! Agent surfaces: the side panel, the transcript and the composer.
//!
//! The pieces are presentational and take plain values, so an application maps its own
//! conversation model onto them; nothing here knows how a model is called. The rules they
//! follow:
//!
//! * **The AI has one colour.** [`crate::Colors::agent`] marks what the agent did or is doing —
//!   its avatar, running work, its edits. Nothing else uses it, so it keeps meaning something.
//! * **A model is shown with its provider's mark.** Where the model is known, the picker, the
//!   current model and the assistant's avatar carry its [`crate::ProviderLogo`], in the text
//!   colour, so the person can tell whose model is answering before reading the name.
//! * **Every state has a shape as well as a colour.** [`AgentStatus::mark`] pairs with each one.
//! * **The person's words and the agent's words differ in form, not only colour.** The person
//!   speaks in a bubble on the trailing edge; the agent writes full-width prose.
//! * **Tool work is collapsed by default** and grouped, so a long turn reads as its answer.
//! * **Approvals name the object and the verb**, with the safe choice as the default.

mod code;
mod composer;
mod panel;
mod transcript;

pub use code::{CodeBlock, DiffBlock, DiffLine};
pub use composer::{AskBar, Composer, ContextChip, SuggestionChip};
pub use panel::{AgentPanel, AgentToggle, ConversationRow};
pub use transcript::{
    ApprovalCard, AssistantMessage, ChatNotice, NoticeKind, ReasoningBlock, ThinkingIndicator,
    ToolCall, ToolGroup, ToolState, UserMessage,
};

use gpui::{App, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div};

use crate::display::{Spinner, StatusDot};
use crate::theme::{ActiveTheme, Theme};

/// What the agent is doing, as the window chrome and the panel header report it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AgentStatus {
    /// Nothing is running and nothing needs attention.
    #[default]
    Idle,
    /// A turn is running.
    Running,
    /// The turn is paused for the person's decision.
    AwaitingApproval,
    /// The last turn finished.
    Completed,
    /// The last turn stopped or failed.
    Failed,
}

impl AgentStatus {
    /// Short state text.
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "Ready",
            Self::Running => "Working",
            Self::AwaitingApproval => "Needs approval",
            Self::Completed => "Done",
            Self::Failed => "Stopped",
        }
    }

    /// A mark that carries the state without colour.
    pub fn mark(self) -> &'static str {
        match self {
            Self::Idle => "",
            Self::Running => "…",
            Self::AwaitingApproval => "?",
            Self::Completed => "✓",
            Self::Failed => "!",
        }
    }

    /// The state's colour under `theme`.
    pub fn color(self, theme: &Theme) -> Hsla {
        let colors = &theme.colors;
        match self {
            Self::Idle => colors.text_faint,
            Self::Running => colors.agent,
            Self::AwaitingApproval => colors.warning,
            Self::Completed => colors.success,
            Self::Failed => colors.danger,
        }
    }
}

/// The status as a dot (or spinner) and its words.
#[derive(IntoElement)]
pub struct AgentStatusLabel {
    status: AgentStatus,
    detail: Option<SharedString>,
}

impl AgentStatusLabel {
    /// A status label.
    pub fn new(status: AgentStatus) -> Self {
        Self {
            status,
            detail: None,
        }
    }

    /// Extra words after the state, for example "3 tools · 12 s".
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

impl RenderOnce for AgentStatusLabel {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let color = self.status.color(theme);
        div()
            .flex()
            .items_center()
            .min_w_0()
            .gap(theme.spacing.xs)
            .text_size(theme.typography.size_xs)
            .text_color(theme.colors.text_muted)
            .whitespace_nowrap()
            .child(match self.status {
                AgentStatus::Running => Spinner::new("agent-status")
                    .size(theme.typography.size_sm)
                    .into_any_element(),
                status => StatusDot::new(status.color(theme))
                    .size(gpui::px(7.))
                    .into_any_element(),
            })
            .child(
                div()
                    .flex_none()
                    .text_color(color)
                    .child(self.status.label()),
            )
            // The detail gives way first when the label runs out of room.
            .children(
                self.detail
                    .map(|detail| div().min_w_0().truncate().child(format!("· {detail}"))),
            )
    }
}
