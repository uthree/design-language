//! Short text the agent wrote: a name, a title, a description.

use gpui::{
    AnyView, App, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};

use crate::icon::{Icon, IconName};
use crate::overlay::Tooltip;
use crate::text::{Label, TextColor, TextSize};
use crate::theme::ActiveTheme;

/// A short piece of text the agent wrote and nobody has changed yet — a cluster's name, a
/// take's title, a summary line — marked as the agent's where it is shown.
///
/// The text keeps its ordinary colour, so a list of names stays one list; a small sparkle in
/// [`crate::Colors::agent`] follows it. Once the person edits the text it is theirs: draw it
/// as a plain [`Label`] from then on. Give it an id to add a tooltip that says so, such as
/// "Named by AI — rename to keep your own".
#[derive(IntoElement)]
pub struct AgentLabel {
    text: SharedString,
    size: TextSize,
    color: TextColor,
    tooltip: Option<(ElementId, SharedString)>,
}

impl AgentLabel {
    /// Text the agent wrote.
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            size: TextSize::Medium,
            color: TextColor::Default,
            tooltip: None,
        }
    }

    /// Step on the type scale.
    pub fn size(mut self, size: TextSize) -> Self {
        self.size = size;
        self
    }

    /// Semantic colour of the text itself; the mark stays the agent's.
    pub fn color(mut self, color: TextColor) -> Self {
        self.color = color;
        self
    }

    /// Say on hover who wrote it.
    pub fn tooltip(mut self, id: impl Into<ElementId>, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some((id.into(), tooltip.into()));
        self
    }
}

impl RenderOnce for AgentLabel {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let size = self.size.pixels(theme);
        let mark = Icon::new(IconName::Sparkles)
            .size((size * 0.8).max(px(10.)))
            .color(theme.colors.agent);
        let row = div()
            .flex()
            .items_center()
            .gap(theme.spacing.xxs)
            .min_w_0()
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .child(Label::new(self.text).size(self.size).color(self.color)),
            )
            .child(div().flex_none().child(mark));
        match self.tooltip {
            Some((id, tooltip)) => row
                .id(id)
                .tooltip(move |window, cx| -> AnyView {
                    Tooltip::new(tooltip.clone()).build(window, cx)
                })
                .into_any_element(),
            None => row.into_any_element(),
        }
    }
}
