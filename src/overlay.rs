//! Surfaces that float above the window: tooltips, menus and popovers.
//!
//! These draw the surface only. Positioning is the caller's, with gpui's `anchored` and
//! `deferred`, because where a menu opens depends on what opened it.

use std::rc::Rc;

use gpui::{
    AnyElement, AnyView, App, AppContext, ClickEvent, Context, ElementId, HighlightStyle,
    InteractiveElement, IntoElement, ParentElement, Render, RenderOnce, ScrollHandle, SharedString,
    StatefulInteractiveElement, StyleRefinement, Styled, StyledText, Window, div,
    prelude::FluentBuilder, px,
};

use crate::focus::focusable;
use crate::icon::{Icon, IconName, IconSource};
use crate::text::{Label, TextColor, TextSize};
use crate::theme::{ActiveTheme, Theme};

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
    id: Option<ElementId>,
}

impl Popover {
    /// An empty popover surface.
    pub fn new() -> Self {
        Self {
            base: div(),
            id: None,
        }
    }

    /// A stable id, so the popover fades in when it opens.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
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
        let surface = self
            .base
            .flex()
            .flex_col()
            .p(px(4.))
            .rounded(theme.shape.radius_md)
            .bg(theme.colors.overlay)
            .border_1()
            .border_color(theme.colors.border)
            .shadow(theme.elevation.medium.clone());
        match self.id {
            Some(id) => crate::motion::appear(surface, id, cx, |surface, delta| {
                surface
                    .opacity(delta)
                    .mt(gpui::px(crate::motion::lerp(-4., 0., delta)))
            }),
            None => surface.into_any_element(),
        }
    }
}

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// One row of a menu. A row with [`MenuItem::on_click`] is a tab stop.
#[derive(IntoElement)]
pub struct MenuItem {
    id: ElementId,
    label: SharedString,
    icon: Option<IconSource>,
    shortcut: Option<SharedString>,
    checked: bool,
    danger: bool,
    disabled: bool,
    on_click: Option<ClickHandler>,
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
            on_click: None,
        }
    }

    /// Leading icon.
    pub fn icon(mut self, icon: impl Into<IconSource>) -> Self {
        self.icon = Some(icon.into());
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

    /// Greyed out, unclickable, and out of the Tab order.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Called on click, Enter or Space. Close the menu there.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for MenuItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let text = if self.disabled {
            colors.text_disabled
        } else if self.danger {
            colors.danger
        } else {
            colors.text
        };
        let row = focusable(
            div().id(self.id.clone()),
            &self.id,
            self.disabled || self.on_click.is_none(),
            theme.shape.radius_sm,
            window,
            cx,
        )
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
        .child(div().flex_1().min_w_0().truncate().child(self.label))
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
        let row = row.hover(move |style| style.bg(hover));
        match self.on_click {
            Some(handler) => row.on_click(handler),
            None => row,
        }
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

/// One suggestion in [`Completions`]: what it inserts, and what it does.
pub struct Completion {
    label: SharedString,
    description: Option<SharedString>,
    icon: Option<IconSource>,
    glyph: Option<SharedString>,
    matched: Vec<usize>,
}

impl Completion {
    /// A suggestion such as `/model` or `claude-opus`.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            description: None,
            icon: None,
            glyph: None,
            matched: Vec::new(),
        }
    }

    /// What it does or what it is, set muted after the label.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// A leading icon.
    pub fn icon(mut self, icon: impl Into<IconSource>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// A leading character in place of the icon, drawn a size up: an emoji, a symbol.
    pub fn glyph(mut self, glyph: impl Into<SharedString>) -> Self {
        self.glyph = Some(glyph.into());
        self
    }

    /// The byte offsets in the label that matched what was typed, drawn in the accent, as a
    /// fuzzy matcher reports them.
    pub fn matched(mut self, matched: Vec<usize>) -> Self {
        self.matched = matched;
        self
    }
}

type SelectHandler = Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>;

/// What [`Completions`] remembers: its scroll, and which row it last scrolled to.
#[derive(Default)]
struct CompletionsScroll {
    handle: ScrollHandle,
    shown: Option<usize>,
}

/// The suggestions under or over a text input as the person types: slash commands, model
/// names, mentions.
///
/// Focus stays in the input the whole time, so the rows are no tab stops: the application
/// moves [`Completions::selected`] with Up and Down from the input, accepts it with Tab (and
/// Enter, where Enter does not already send), and closes the list with Escape. The list
/// scrolls the selected row into view, so the keyboard never leaves it hidden. A click picks a
/// row through [`Completions::on_select`]. Each row sets its label on the leading side and its
/// description on the trailing side, so a column of commands reads down one edge.
///
/// Draw it on its own or hand it to [`crate::agent::Composer::completions`], which floats it
/// over the composer.
#[derive(IntoElement)]
pub struct Completions {
    id: ElementId,
    items: Vec<Completion>,
    selected: Option<usize>,
    rows: usize,
    empty: Option<SharedString>,
    on_select: Option<SelectHandler>,
}

impl Completions {
    /// An empty list.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            selected: None,
            rows: 6,
            empty: None,
            on_select: None,
        }
    }

    /// Add a suggestion.
    pub fn item(mut self, item: Completion) -> Self {
        self.items.push(item);
        self
    }

    /// Add suggestions.
    pub fn items(mut self, items: impl IntoIterator<Item = Completion>) -> Self {
        self.items.extend(items);
        self
    }

    /// The row the keyboard is on, which Tab would accept.
    pub fn selected(mut self, selected: Option<usize>) -> Self {
        self.selected = selected;
        self
    }

    /// How many rows show before the list scrolls; six by default.
    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows.max(1);
        self
    }

    /// What to say when nothing matches, instead of hiding the list.
    pub fn empty(mut self, empty: impl Into<SharedString>) -> Self {
        self.empty = Some(empty.into());
        self
    }

    /// Called with the index of a clicked row.
    pub fn on_select(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Completions {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let colors = &theme.colors;
        let row = theme.density.row;
        let inset = theme.spacing.xxs;
        let scroll = window.use_keyed_state(
            ElementId::NamedChild(Box::new(self.id.clone()), "scroll".into()),
            cx,
            |_, _| CompletionsScroll::default(),
        );
        let handle = scroll.update(cx, |scroll, _| {
            // Scroll only when the selection moves, so a wheel scroll is left alone.
            if scroll.shown != self.selected {
                scroll.shown = self.selected;
                if let Some(selected) = self.selected {
                    scroll.handle.scroll_to_item(selected);
                }
            }
            scroll.handle.clone()
        });
        let accent = HighlightStyle {
            color: Some(colors.accent_text),
            font_weight: Some(theme.typography.label_weight),
            ..Default::default()
        };
        let rows = self.items.into_iter().enumerate().map(|(index, item)| {
            let selected = self.selected == Some(index);
            let label = item.label.clone();
            let highlights = item
                .matched
                .iter()
                .filter_map(|&at| {
                    let length = label.get(at..)?.chars().next()?.len_utf8();
                    Some((at..at + length, accent))
                })
                .collect::<Vec<_>>();
            let mut entry = div()
                .id(ElementId::NamedChild(
                    Box::new(self.id.clone()),
                    format!("item-{index}").into(),
                ))
                .flex()
                .flex_none()
                .items_center()
                .gap(theme.spacing.sm)
                .h(row - px(2.))
                .px(theme.spacing.sm)
                .rounded(Theme::concentric(theme.shape.radius_md, inset))
                .text_size(theme.typography.size_md)
                .text_color(colors.text)
                .when(selected, |entry| entry.bg(colors.selected))
                .when(!selected, |entry| {
                    let hover = colors.hover;
                    entry.hover(move |style| style.bg(hover))
                })
                .children(
                    item.icon
                        .map(|icon| Icon::new(icon).size(px(14.)).color(colors.text_muted)),
                )
                .children(item.glyph.map(|glyph| {
                    div()
                        .flex_none()
                        .flex()
                        .justify_center()
                        .w(theme.typography.size_lg * 1.25)
                        .text_size(theme.typography.size_lg)
                        .child(glyph)
                }))
                .child(
                    div()
                        .flex_none()
                        .whitespace_nowrap()
                        .child(StyledText::new(label).with_highlights(highlights)),
                )
                .child(div().flex_1().min_w(theme.spacing.lg))
                .children(item.description.map(|description| {
                    div().min_w_0().truncate().child(
                        Label::new(description)
                            .size(TextSize::Small)
                            .color(TextColor::Muted),
                    )
                }));
            if let Some(handler) = self.on_select.clone() {
                entry = entry.on_click(move |_, window, cx| handler(&index, window, cx));
            }
            entry
        });
        let rows: Vec<_> = rows.collect();
        let nothing = rows.is_empty();
        div()
            .id(self.id.clone())
            .debug_selector({
                let id = self.id.clone();
                move || id.to_string()
            })
            .flex()
            .flex_col()
            .p(inset)
            .rounded(theme.shape.radius_md)
            .bg(colors.overlay)
            .border_1()
            .border_color(colors.border)
            .shadow(theme.elevation.medium.clone())
            .child(
                div()
                    .id(ElementId::NamedChild(Box::new(self.id), "rows".into()))
                    .flex()
                    .flex_col()
                    .max_h((row - px(2.)) * self.rows as f32)
                    .overflow_y_scroll()
                    .track_scroll(&handle)
                    .children(rows)
                    .when(nothing, |list| {
                        list.children(self.empty.map(|empty| {
                            div()
                                .flex()
                                .items_center()
                                .h(row - px(2.))
                                .px(theme.spacing.sm)
                                .child(
                                    Label::new(empty)
                                        .size(TextSize::Small)
                                        .color(TextColor::Faint),
                                )
                        }))
                    }),
            )
    }
}
