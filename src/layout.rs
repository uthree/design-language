//! Structure: panels, toolbars, sidebars, rows, tabs and section headers.
//!
//! The container builders here implement `Styled` and `ParentElement` by delegating to an inner
//! `div`, so they compose like any gpui element while owning their surface treatment.

use gpui::{
    AnyElement, App, ClickEvent, Div, ElementId, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, SharedString, StatefulInteractiveElement, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder, px,
};

use crate::icon::{Icon, IconName};
use crate::text::{Label, TextColor, TextSize};
use crate::theme::{ActiveTheme, Theme};

macro_rules! container {
    ($name:ident) => {
        impl Styled for $name {
            fn style(&mut self) -> &mut StyleRefinement {
                self.base.style()
            }
        }

        impl ParentElement for $name {
            fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
                self.base.extend(elements)
            }
        }
    };
}

/// Which edge of a region borders the rest of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// Leading side: a navigation sidebar.
    Left,
    /// Trailing side: an inspector or the agent panel.
    Right,
    /// Top: a status or transport bar under content.
    Top,
    /// Bottom: a toolbar above content.
    Bottom,
}

fn border_edge(element: Div, edge: Edge, theme: &Theme) -> Div {
    let element = element.border_color(theme.colors.border);
    match edge {
        Edge::Left => element.border_l_1(),
        Edge::Right => element.border_r_1(),
        Edge::Top => element.border_t_1(),
        Edge::Bottom => element.border_b_1(),
    }
}

/// The application's toolbar or title bar: chrome-coloured, one row, one centre line.
#[derive(IntoElement)]
pub struct Toolbar {
    base: Div,
}

impl Toolbar {
    /// An empty toolbar.
    pub fn new() -> Self {
        Self { base: div() }
    }
}

impl Default for Toolbar {
    fn default() -> Self {
        Self::new()
    }
}

container!(Toolbar);

impl RenderOnce for Toolbar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        border_edge(
            self.base
                .flex()
                .flex_none()
                .items_center()
                .gap(theme.spacing.sm)
                .h(theme.density.toolbar)
                .px(theme.spacing.md)
                .bg(theme.colors.chrome),
            Edge::Bottom,
            theme,
        )
    }
}

/// A persistent side region: navigation on the leading edge, inspectors on the trailing one.
#[derive(IntoElement)]
pub struct Sidebar {
    base: Div,
    edge: Edge,
}

impl Sidebar {
    /// A sidebar whose border faces `edge`'s opposite: a [`Edge::Left`] sidebar borders on its
    /// right.
    pub fn new(edge: Edge) -> Self {
        Self { base: div(), edge }
    }
}

container!(Sidebar);

impl RenderOnce for Sidebar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let border = match self.edge {
            Edge::Left => Edge::Right,
            Edge::Right => Edge::Left,
            Edge::Top => Edge::Bottom,
            Edge::Bottom => Edge::Top,
        };
        border_edge(
            self.base
                .flex()
                .flex_col()
                .flex_none()
                .bg(theme.colors.chrome),
            border,
            theme,
        )
    }
}

/// A grouped region on a panel: background and hairline, no shadow. Do not nest cards.
#[derive(IntoElement)]
pub struct Card {
    base: Div,
}

impl Card {
    /// An empty card.
    pub fn new() -> Self {
        Self { base: div() }
    }
}

impl Default for Card {
    fn default() -> Self {
        Self::new()
    }
}

container!(Card);

impl RenderOnce for Card {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        self.base
            .flex()
            .flex_col()
            .rounded(theme.shape.radius_md)
            .bg(theme.colors.elevated)
            .border_1()
            .border_color(theme.colors.border)
    }
}

/// A section title with optional trailing actions.
#[derive(IntoElement)]
pub struct SectionHeader {
    title: SharedString,
    description: Option<SharedString>,
    actions: Vec<AnyElement>,
    caption: bool,
}

impl SectionHeader {
    /// A heading-sized section title.
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            description: None,
            actions: Vec::new(),
            caption: false,
        }
    }

    /// A small caption-style title, for sidebars and inspectors.
    pub fn caption(title: impl Into<SharedString>) -> Self {
        Self {
            caption: true,
            ..Self::new(title)
        }
    }

    /// Muted line under the title.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// A trailing action.
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }
}

impl RenderOnce for SectionHeader {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let title = if self.caption {
            Label::caption(self.title)
        } else {
            Label::heading(self.title)
        };
        div()
            .flex()
            .items_center()
            .gap(theme.spacing.sm)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(theme.spacing.xxs)
                    .child(title)
                    .children(self.description.map(|description| {
                        Label::new(description)
                            .size(TextSize::Small)
                            .color(TextColor::Muted)
                    })),
            )
            .children(self.actions)
    }
}

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// A row in a sidebar, list or tree. Icon, label and trailing slot keep fixed lanes, so rows
/// line up whether or not each has an icon.
#[derive(IntoElement)]
pub struct ListItem {
    id: ElementId,
    label: SharedString,
    icon: Option<IconName>,
    icon_color: Option<gpui::Hsla>,
    description: Option<SharedString>,
    trailing: Option<AnyElement>,
    selected: bool,
    indent: usize,
    icon_lane: bool,
    on_click: Option<ClickHandler>,
}

impl ListItem {
    /// A row.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            icon_color: None,
            description: None,
            trailing: None,
            selected: false,
            indent: 0,
            icon_lane: true,
            on_click: None,
        }
    }

    /// Leading icon.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Colour of the leading icon, for example a track colour.
    pub fn icon_color(mut self, color: gpui::Hsla) -> Self {
        self.icon_color = Some(color);
        self
    }

    /// Muted second line.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Trailing content: a count, a badge, a shortcut.
    pub fn trailing(mut self, trailing: impl IntoElement) -> Self {
        self.trailing = Some(trailing.into_any_element());
        self
    }

    /// Selected: shown by the row's own fill, never by an edge bar.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Tree depth.
    pub fn indent(mut self, indent: usize) -> Self {
        self.indent = indent;
        self
    }

    /// Drop the reserved icon lane, for lists where no row has an icon.
    pub fn no_icon_lane(mut self) -> Self {
        self.icon_lane = false;
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

impl RenderOnce for ListItem {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let text = if self.selected {
            colors.text
        } else {
            colors.text_muted
        };
        let icon_color = self.icon_color.unwrap_or(if self.selected {
            colors.accent_text
        } else {
            colors.text_faint
        });
        let hover = colors.hover;
        let height = if self.description.is_some() {
            theme.density.row + theme.typography.size_sm
        } else {
            theme.density.row
        };
        let mut row = div()
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .gap(theme.spacing.sm)
            .h(height)
            .pl(theme.spacing.sm + theme.spacing.lg * self.indent as f32)
            .pr(theme.spacing.sm)
            .rounded(theme.shape.radius_sm)
            .text_size(theme.typography.size_md)
            .text_color(text)
            .when(self.selected, |row| {
                row.bg(colors.selected)
                    .font_weight(theme.typography.label_weight)
            })
            .when(!self.selected, |row| {
                row.hover(move |style| style.bg(hover))
            })
            .when(self.icon_lane, |row| {
                row.child(
                    div()
                        .flex()
                        .flex_none()
                        .justify_center()
                        .w(px(16.))
                        .children(
                            self.icon
                                .map(|icon| Icon::new(icon).size(px(15.)).color(icon_color)),
                        ),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(div().truncate().child(self.label))
                    .children(self.description.map(|description| {
                        Label::new(description)
                            .size(TextSize::Small)
                            .color(TextColor::Faint)
                            .truncate()
                    })),
            )
            .children(self.trailing);
        if let Some(handler) = self.on_click {
            row = row.on_click(handler);
        }
        row
    }
}

type SelectHandler = std::rc::Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>;

/// A row of tabs for switching between views of one object.
#[derive(IntoElement)]
pub struct Tabs {
    id: ElementId,
    labels: Vec<SharedString>,
    selected: usize,
    on_select: Option<SelectHandler>,
}

impl Tabs {
    /// Tabs with `selected` active.
    pub fn new(
        id: impl Into<ElementId>,
        labels: impl IntoIterator<Item = impl Into<SharedString>>,
        selected: usize,
    ) -> Self {
        Self {
            id: id.into(),
            labels: labels.into_iter().map(Into::into).collect(),
            selected,
            on_select: None,
        }
    }

    /// Called with the index of the clicked tab.
    pub fn on_select(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(std::rc::Rc::new(handler));
        self
    }
}

impl RenderOnce for Tabs {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let id = self.id.clone();
        let height = theme.density.control_md;
        div()
            .id(self.id)
            .flex()
            .items_center()
            .gap(theme.spacing.xxs)
            .children(self.labels.into_iter().enumerate().map(|(index, label)| {
                let selected = index == self.selected;
                let hover = colors.hover;
                let mut tab = div()
                    .id(ElementId::NamedChild(
                        Box::new(id.clone()),
                        index.to_string().into(),
                    ))
                    .flex()
                    .items_center()
                    .h(height)
                    .px(theme.spacing.md)
                    .rounded(theme.control_radius(height))
                    .text_size(theme.typography.size_md)
                    .font_weight(theme.typography.label_weight)
                    .text_color(if selected {
                        colors.text
                    } else {
                        colors.text_muted
                    })
                    .when(selected, |tab| tab.bg(colors.selected))
                    .when(!selected, |tab| tab.hover(move |style| style.bg(hover)))
                    .child(label);
                if let Some(handler) = self.on_select.clone() {
                    tab = tab.on_click(move |_, window, cx| handler(&index, window, cx));
                }
                tab
            }))
    }
}

/// A row that shows or hides the content under it.
#[derive(IntoElement)]
pub struct Disclosure {
    id: ElementId,
    label: SharedString,
    open: bool,
    trailing: Option<AnyElement>,
    on_toggle: Option<ClickHandler>,
}

impl Disclosure {
    /// A disclosure row.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>, open: bool) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            open,
            trailing: None,
            on_toggle: None,
        }
    }

    /// Trailing content.
    pub fn trailing(mut self, trailing: impl IntoElement) -> Self {
        self.trailing = Some(trailing.into_any_element());
        self
    }

    /// Called on click; flip `open` there.
    pub fn on_toggle(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Disclosure {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let muted = cx.theme().colors.text_muted;
        let chevron = crate::motion::chevron(
            (self.id.clone(), "chevron"),
            self.open,
            px(12.),
            muted,
            window,
            cx,
        );
        let theme = cx.theme();
        let hover = theme.colors.hover;
        let mut row = div()
            .id(self.id)
            .flex()
            .items_center()
            .gap(theme.spacing.xs)
            .h(theme.density.control_sm)
            .px(theme.spacing.xs)
            .rounded(theme.shape.radius_sm)
            .text_size(theme.typography.size_sm)
            .text_color(theme.colors.text_muted)
            .hover(move |style| style.bg(hover))
            .child(chevron)
            .child(div().flex_1().min_w_0().truncate().child(self.label))
            .children(self.trailing);
        if let Some(handler) = self.on_toggle {
            row = row.on_click(handler);
        }
        row
    }
}

/// What a region shows when it has nothing yet, and what to do about it.
#[derive(IntoElement)]
pub struct EmptyState {
    icon: IconName,
    title: SharedString,
    description: Option<SharedString>,
    action: Option<AnyElement>,
}

impl EmptyState {
    /// An empty state.
    pub fn new(icon: IconName, title: impl Into<SharedString>) -> Self {
        Self {
            icon,
            title: title.into(),
            description: None,
            action: None,
        }
    }

    /// One sentence on why it is empty or what fills it.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// The next action.
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }
}

impl RenderOnce for EmptyState {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(theme.spacing.sm)
            .p(theme.spacing.xxl)
            .child(
                Icon::new(self.icon)
                    .size(px(28.))
                    .color(theme.colors.text_faint),
            )
            .child(Label::new(self.title).heading_weight())
            .children(self.description.map(|description| {
                Label::new(description)
                    .size(TextSize::Small)
                    .color(TextColor::Muted)
            }))
            .children(
                self.action
                    .map(|action| div().mt(theme.spacing.sm).child(action)),
            )
    }
}

/// A large choice tile: an icon, a title and a line of description. Use a row of them when the
/// choice is the whole point of the screen — how to start a project, which template to use.
#[derive(IntoElement)]
pub struct OptionCard {
    id: ElementId,
    icon: IconName,
    title: SharedString,
    description: Option<SharedString>,
    selected: bool,
    agent: bool,
    on_click: Option<ClickHandler>,
}

impl OptionCard {
    /// A tile.
    pub fn new(id: impl Into<ElementId>, icon: IconName, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            icon,
            title: title.into(),
            description: None,
            selected: false,
            agent: false,
            on_click: None,
        }
    }

    /// What choosing it does, in one line.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// The current choice.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The choice hands the work to the agent: its icon takes the agent colours.
    pub fn agent(mut self) -> Self {
        self.agent = true;
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

impl RenderOnce for OptionCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let (icon_color, icon_wash) = if self.agent {
            (colors.agent, colors.agent_wash)
        } else {
            (colors.accent_text, colors.accent_wash)
        };
        let tile = px(56.);
        let hover = colors.hover;
        let mut card = div()
            .id(self.id)
            .flex()
            .flex_col()
            .gap(theme.spacing.md)
            .p(theme.spacing.lg)
            .rounded(theme.shape.radius_lg)
            .bg(colors.elevated)
            .border_1()
            .border_color(if self.selected {
                colors.accent
            } else {
                colors.border
            })
            // The selected tile carries a ring as well as the border colour, so it does not rely on
            // a hue difference alone.
            .when(self.selected, |card| {
                card.shadow(vec![gpui::BoxShadow {
                    color: colors.focus_ring.opacity(0.35),
                    offset: gpui::point(px(0.), px(0.)),
                    blur_radius: px(0.),
                    spread_radius: px(3.),
                }])
            })
            .when(!self.selected, |card| {
                card.hover(move |style| style.bg(hover))
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(tile)
                    .rounded(
                        Theme::concentric(theme.shape.radius_lg, theme.spacing.lg)
                            .max(theme.shape.radius_md),
                    )
                    .bg(icon_wash)
                    .child(Icon::new(self.icon).size(px(28.)).color(icon_color)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.spacing.xs)
                    .child(
                        Label::new(self.title)
                            .size(TextSize::Large)
                            .heading_weight(),
                    )
                    .children(self.description.map(|description| {
                        Label::new(description)
                            .size(TextSize::Small)
                            .color(TextColor::Muted)
                    })),
            );
        if let Some(handler) = self.on_click {
            card = card.on_click(handler);
        }
        card
    }
}
