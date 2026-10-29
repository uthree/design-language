//! The agent's side panel and the chrome that opens it.

use gpui::{
    AnyElement, App, ClickEvent, ElementId, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div, linear_color_stop,
    linear_gradient, prelude::FluentBuilder, px, relative,
};

use super::transcript::agent_avatar;
use super::{AgentStatus, AgentStatusLabel};
use crate::button::Button;
use crate::focus::focusable;
use crate::icon::{Icon, IconName, IconSource};
use crate::motion::{lerp, looping};
use crate::strings::ActiveStrings;
use crate::text::{Label, TextColor, TextSize};
use crate::theme::ActiveTheme;

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// The side panel: a header with the conversation's title and status, a scrolling transcript,
/// and the composer pinned to the bottom.
///
/// While the agent works, a thin gradient runs along the panel's top edge — the one ambient
/// signal that something is happening off-screen in the transcript.
#[derive(IntoElement)]
pub struct AgentPanel {
    id: ElementId,
    title: SharedString,
    status: AgentStatus,
    status_detail: Option<SharedString>,
    logo: Option<IconSource>,
    actions: Vec<AnyElement>,
    transcript: Vec<AnyElement>,
    composer: Option<AnyElement>,
    empty: Option<AnyElement>,
    on_close: Option<ClickHandler>,
}

impl AgentPanel {
    /// A panel.
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            status: AgentStatus::Idle,
            status_detail: None,
            logo: None,
            actions: Vec::new(),
            transcript: Vec::new(),
            composer: None,
            empty: None,
            on_close: None,
        }
    }

    /// The agent's state.
    pub fn status(mut self, status: AgentStatus) -> Self {
        self.status = status;
        self
    }

    /// Words after the state.
    pub fn status_detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.status_detail = Some(detail.into());
        self
    }

    /// The header's avatar as the provider's mark — a [`crate::ProviderLogo`] — instead of the
    /// agent's gradient.
    pub fn logo(mut self, logo: impl Into<IconSource>) -> Self {
        self.logo = Some(logo.into());
        self
    }

    /// A header action: new conversation, history.
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }

    /// One transcript row.
    pub fn entry(mut self, entry: impl IntoElement) -> Self {
        self.transcript.push(entry.into_any_element());
        self
    }

    /// What to show when the transcript is empty: a greeting and suggestions.
    pub fn empty_state(mut self, empty: impl IntoElement) -> Self {
        self.empty = Some(empty.into_any_element());
        self
    }

    /// The composer, usually [`super::Composer`].
    pub fn composer(mut self, composer: impl IntoElement) -> Self {
        self.composer = Some(composer.into_any_element());
        self
    }

    /// Show a close button and call this on it.
    pub fn on_close(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_close = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for AgentPanel {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let running = self.status == AgentStatus::Running;
        let [from, middle, to] = colors.agent_gradient;
        let close = self.on_close.map(|handler| {
            Button::icon_only((self.id.clone(), "close"), IconName::SidebarRight)
                .tooltip(cx.strings().hide_assistant.clone())
                .on_click(handler)
        });
        let mut status = AgentStatusLabel::new(self.status);
        if let Some(detail) = self.status_detail {
            status = status.detail(detail);
        }
        let empty = self.transcript.is_empty();

        div()
            .id(self.id.clone())
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors.surface)
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(theme.spacing.sm)
                    .h(theme.density.toolbar)
                    .px(theme.spacing.md)
                    .border_b_1()
                    .border_color(colors.border)
                    .child(agent_avatar(self.logo).size(px(22.)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(
                                Label::new(self.title)
                                    .weight(theme.typography.label_weight)
                                    .truncate(),
                            )
                            .child(status),
                    )
                    .children(self.actions)
                    .children(close),
            )
            .when(running, |panel| {
                panel.child(
                    div()
                        .absolute()
                        .top(theme.density.toolbar - px(1.))
                        .left_0()
                        .right_0()
                        .h(px(2.))
                        .flex()
                        .child(div().flex_1().h_full().bg(linear_gradient(
                            90.,
                            linear_color_stop(from, 0.),
                            linear_color_stop(middle, 1.),
                        )))
                        .child(div().flex_1().h_full().bg(linear_gradient(
                            90.,
                            linear_color_stop(middle, 0.),
                            linear_color_stop(to, 1.),
                        )))
                        .overflow_hidden()
                        // A light passes along the edge, so "working" reads even from the
                        // corner of the eye.
                        .child(looping(
                            div().absolute().top_0().bottom_0().w(relative(0.25)).bg(
                                linear_gradient(
                                    90.,
                                    linear_color_stop(gpui::white().opacity(0.), 0.),
                                    linear_color_stop(gpui::white().opacity(0.85), 1.),
                                ),
                            ),
                            (self.id.clone(), "sheen"),
                            theme.motion.slow,
                            -1.,
                            cx,
                            |band, delta| band.left(relative(lerp(-0.25, 1., delta))),
                        )),
                )
            })
            .child(
                div()
                    .id((self.id.clone(), "transcript"))
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(theme.density.panel_padding)
                    .py(theme.spacing.lg)
                    .gap(theme.spacing.lg)
                    .children(self.transcript)
                    .when(empty, |body| body.children(self.empty)),
            )
            .children(self.composer.map(|composer| {
                div()
                    .flex_none()
                    .px(theme.spacing.sm)
                    .pb(theme.spacing.sm)
                    .child(composer)
            }))
    }
}

/// The title-bar switch that shows or hides the agent panel, and reports its state while hidden.
#[derive(IntoElement)]
pub struct AgentToggle {
    id: ElementId,
    open: bool,
    status: AgentStatus,
    on_click: Option<ClickHandler>,
}

impl AgentToggle {
    /// A toggle.
    pub fn new(id: impl Into<ElementId>, open: bool, status: AgentStatus) -> Self {
        Self {
            id: id.into(),
            open,
            status,
            on_click: None,
        }
    }

    /// Called on click.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for AgentToggle {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let height = theme.density.control_md;
        let color = self.status.color(theme);
        let hover = colors.hover;
        let mark = self.status.mark();
        let mut toggle = focusable(
            div().id(self.id.clone()),
            &self.id,
            self.on_click.is_none(),
            theme.control_radius(height),
            window,
            cx,
        )
        .relative()
        .flex()
        .items_center()
        .gap(theme.spacing.xs)
        .h(height)
        .px(theme.spacing.sm)
        .rounded(theme.control_radius(height))
        .text_size(theme.typography.size_sm)
        .font_weight(theme.typography.label_weight)
        .when(self.open, |toggle| {
            toggle.bg(colors.agent_wash).text_color(colors.agent)
        })
        .when(!self.open, |toggle| {
            toggle
                .text_color(colors.text_muted)
                .hover(move |style| style.bg(hover))
        })
        .child(
            Icon::new(IconName::Sparkles)
                .size(px(15.))
                .color(colors.agent),
        )
        .child(cx.strings().assistant.clone())
        // The state badge only matters while the panel cannot show it itself.
        .when(!self.open && !mark.is_empty(), |toggle| {
            toggle.child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(14.))
                    .rounded(px(7.))
                    .bg(color)
                    .text_size(px(9.))
                    .text_color(theme.text_on(color))
                    .child(mark),
            )
        });
        if let Some(handler) = self.on_click {
            toggle = toggle.on_click(handler);
        }
        toggle
    }
}

/// A row in the conversation history.
#[derive(IntoElement)]
pub struct ConversationRow {
    id: ElementId,
    title: SharedString,
    time: SharedString,
    selected: bool,
    status: AgentStatus,
    on_click: Option<ClickHandler>,
}

impl ConversationRow {
    /// A row.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        time: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            time: time.into(),
            selected: false,
            status: AgentStatus::Idle,
            on_click: None,
        }
    }

    /// Selected.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The conversation's state, if it is still running or waiting.
    pub fn status(mut self, status: AgentStatus) -> Self {
        self.status = status;
        self
    }

    /// Called on click.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for ConversationRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let hover = colors.hover;
        let live = matches!(
            self.status,
            AgentStatus::Running | AgentStatus::AwaitingApproval
        );
        let mut row = focusable(
            div().id(self.id.clone()),
            &self.id,
            self.on_click.is_none(),
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
        .when(self.selected, |row| row.bg(colors.selected))
        .when(!self.selected, |row| {
            row.hover(move |style| style.bg(hover))
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(theme.typography.size_md)
                .text_color(colors.text)
                .child(self.title),
        )
        .when(live, |row| row.child(AgentStatusLabel::new(self.status)))
        .when(!live, |row| {
            row.child(
                Label::new(self.time)
                    .size(TextSize::XSmall)
                    .color(TextColor::Faint),
            )
        });
        if let Some(handler) = self.on_click {
            row = row.on_click(handler);
        }
        row
    }
}
