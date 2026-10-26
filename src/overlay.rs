//! Surfaces that float above the window: tooltips, menus and popovers.
//!
//! These draw the surface only. Positioning is the caller's, with gpui's `anchored` and
//! `deferred`, because where a menu opens depends on what opened it.

use gpui::{
    AnyElement, AnyView, App, AppContext, Context, ElementId, InteractiveElement, IntoElement,
    ParentElement, Render, RenderOnce, SharedString, StyleRefinement, Styled, Window, div, px,
};

use crate::icon::{Icon, IconName};
use crate::text::{Label, TextColor, TextSize};
use crate::theme::ActiveTheme;

/// A tooltip view, built by [`Tooltip::build`] from a `.tooltip(...)` callback.
pub struct Tooltip {
    text: SharedString,
    shortcut: Option<SharedString>,
}

impl Tooltip {
    /// Tooltip text.
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            shortcut: None,
        }
    }

    /// A keyboard shortcut shown after the text, already formatted for the platform.
    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Wrap as the view gpui's `.tooltip` expects.
    pub fn build(self, _window: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|_| self).into()
    }
}

impl Render for Tooltip {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .font(theme.typography.ui.clone())
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(8.))
            .py(px(4.))
            .rounded(theme.shape.radius_sm)
            .bg(theme.colors.overlay)
            .border_1()
            .border_color(theme.colors.border)
            .shadow(theme.elevation.medium.clone())
            .text_size(theme.typography.size_sm)
            .text_color(theme.colors.text)
            .child(self.text.clone())
            .children(self.shortcut.clone().map(|shortcut| {
                Label::new(shortcut)
                    .size(TextSize::XSmall)
                    .color(TextColor::Faint)
            }))
    }
}

/// The floating surface of menus, popovers and palettes.
#[derive(IntoElement)]
pub struct Popover {
    base: gpui::Div,
}

impl Popover {
    /// An empty popover surface.
    pub fn new() -> Self {
        Self { base: div() }
    }
}

impl Default for Popover {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for Popover {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl ParentElement for Popover {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.base.extend(elements)
    }
}

impl RenderOnce for Popover {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        self.base
            .flex()
            .flex_col()
            .p(px(4.))
            .rounded(theme.shape.radius_md)
            .bg(theme.colors.overlay)
            .border_1()
            .border_color(theme.colors.border)
            .shadow(theme.elevation.medium.clone())
    }
}

/// One row of a menu.
#[derive(IntoElement)]
pub struct MenuItem {
    id: ElementId,
    label: SharedString,
    icon: Option<IconName>,
    shortcut: Option<SharedString>,
    checked: bool,
    danger: bool,
    disabled: bool,
}

impl MenuItem {
    /// A menu row.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            shortcut: None,
            checked: false,
            danger: false,
            disabled: false,
        }
    }

    /// Leading icon.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Shortcut hint, already formatted for the platform.
    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Show a check mark.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Draw as a destructive action.
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    /// Greyed out.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for MenuItem {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let text = if self.disabled {
            colors.text_disabled
        } else if self.danger {
            colors.danger
        } else {
            colors.text
        };
        let row = div()
            .id(self.id)
            .flex()
            .items_center()
            .gap(px(8.))
            .h(theme.density.row - px(2.))
            .px(px(8.))
            .rounded(theme.shape.radius_sm)
            .text_size(theme.typography.size_md)
            .text_color(text)
            .child(div().w(px(14.)).flex_none().children(self.checked.then(|| {
                Icon::new(IconName::Check)
                    .size(px(13.))
                    .color(colors.accent_text)
            })))
            .children(
                self.icon
                    .map(|icon| Icon::new(icon).size(px(15.)).color(text.opacity(0.8))),
            )
            .child(div().flex_1().child(self.label))
            .children(self.shortcut.map(|shortcut| {
                Label::new(shortcut)
                    .size(TextSize::Small)
                    .color(TextColor::Faint)
            }));
        if self.disabled {
            return row;
        }
        let hover = if self.danger {
            colors.danger_wash
        } else {
            colors.hover
        };
        row.hover(move |style| style.bg(hover))
    }
}

/// A thin rule between groups of menu items.
pub fn menu_separator(cx: &App) -> impl IntoElement + use<> {
    div()
        .my(px(4.))
        .mx(px(6.))
        .h(px(1.))
        .bg(cx.theme().colors.separator)
}
