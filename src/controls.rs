//! Value controls: switches, checkboxes, segmented controls, sliders and knobs.
//!
//! These are presentational: they draw a value and report clicks. Dragging, keyboard stepping
//! and value mapping stay with the application, which already owns the value and its undo.

use std::f32::consts::PI;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, Div, ElementId, Hsla, InteractiveElement, Interactivity,
    IntoElement, ParentElement, PathBuilder, Pixels, RenderOnce, SharedString,
    StatefulInteractiveElement, StyleRefinement, Styled, Window, canvas, div, fill, point,
    prelude::FluentBuilder, px, relative, size,
};

use crate::button::ControlSize;
use crate::focus::{focusable, selector};
use crate::icon::{Icon, IconName, IconSource};
use crate::motion::{lerp, transition};
use crate::text::{Label, TextColor, TextSize};
use crate::theme::{ActiveTheme, PrimaryStyle, Theme, mix};

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// The fill of an "on" control: the accent, or the foreground in solid-primary directions.
fn on_fill(theme: &Theme) -> Hsla {
    match theme.primary_style {
        PrimaryStyle::Accent => theme.colors.accent,
        PrimaryStyle::Solid => theme.colors.solid,
    }
}

/// An on/off switch, drawn as a track and a thumb.
#[derive(IntoElement)]
pub struct Switch {
    id: ElementId,
    on: bool,
    label: Option<SharedString>,
    disabled: bool,
    on_click: Option<ClickHandler>,
}

impl Switch {
    /// A switch showing `on`.
    pub fn new(id: impl Into<ElementId>, on: bool) -> Self {
        Self {
            id: id.into(),
            on,
            label: None,
            disabled: false,
            on_click: None,
        }
    }

    /// Text after the switch.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Greyed out and out of the Tab order.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Called on click, Enter or Space; flip the value there.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Switch {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let height = (theme.density.control_sm - px(2.)).max(px(18.));
        let width = height * 1.75;
        let inset = px(2.);
        let thumb = height - inset * 2.;
        let off_track = theme.colors.border_strong;
        let on_track = on_fill(&theme);
        let thumb_color = if self.on && theme.primary_style == PrimaryStyle::Solid {
            theme.colors.on_solid
        } else {
            gpui::white()
        };
        let off_left = f32::from(inset);
        let on_left = f32::from(width - thumb - inset);
        let (from, to) = if self.on {
            (off_left, on_left)
        } else {
            (on_left, off_left)
        };
        let (from_track, to_track) = if self.on {
            (off_track, on_track)
        } else {
            (on_track, off_track)
        };

        // The thumb slides and the track cross-fades when the value changes.
        let thumb_element = transition(
            div()
                .absolute()
                .top(inset)
                .left(px(to))
                .size(thumb)
                .rounded(thumb / 2.)
                .bg(thumb_color)
                .shadow(theme.elevation.low.clone()),
            (self.id.clone(), "thumb"),
            &self.on,
            theme.motion.fast,
            window,
            cx,
            move |thumb, delta| thumb.left(px(lerp(from, to, delta))),
        );
        let track = transition(
            div()
                .relative()
                .flex_none()
                .w(width)
                .h(height)
                .rounded(height / 2.)
                .bg(to_track)
                .child(thumb_element),
            (self.id.clone(), "track"),
            &self.on,
            theme.motion.fast,
            window,
            cx,
            move |track, delta| track.bg(mix(from_track, to_track, delta)),
        );
        let mut row = focusable(
            div().id(self.id.clone()),
            &self.id,
            self.disabled,
            height / 2.,
            window,
            cx,
        )
        .flex()
        .items_center()
        .gap(theme.spacing.sm)
        .rounded(height / 2.)
        .child(track)
        .children(
            self.label
                .map(|label| Label::new(label).size(TextSize::Medium)),
        );
        if self.disabled {
            return row.opacity(0.45);
        }
        if let Some(handler) = self.on_click {
            row = row.on_click(handler);
        }
        row
    }
}

/// A checkbox, with an optional label.
#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    checked: bool,
    label: Option<SharedString>,
    disabled: bool,
    on_click: Option<ClickHandler>,
}

impl Checkbox {
    /// A checkbox showing `checked`.
    pub fn new(id: impl Into<ElementId>, checked: bool) -> Self {
        Self {
            id: id.into(),
            checked,
            label: None,
            disabled: false,
            on_click: None,
        }
    }

    /// Text after the box.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Greyed out and out of the Tab order.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Called on click, Enter or Space.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Checkbox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let fill_color = on_fill(&theme);
        let mark = theme.text_on(fill_color);
        // The mark pops in when checked; unchecking is immediate, so nothing lingers.
        let check = self.checked.then(|| {
            transition(
                Icon::new(IconName::Check).size(px(12.)).color(mark),
                (self.id.clone(), "mark"),
                &self.checked,
                theme.motion.fast,
                window,
                cx,
                |icon, delta| icon.size(px(lerp(6., 12., delta))),
            )
        });
        let mut row = focusable(
            div().id(self.id.clone()),
            &self.id,
            self.disabled,
            theme.shape.radius_xs,
            window,
            cx,
        )
        .flex()
        .items_center()
        .gap(theme.spacing.sm)
        .rounded(theme.shape.radius_xs)
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(16.))
                .rounded(theme.shape.radius_xs)
                .when_else(
                    self.checked,
                    |el| el.bg(fill_color),
                    |el| {
                        el.bg(theme.colors.sunken)
                            .border_1()
                            .border_color(theme.colors.border_strong)
                    },
                )
                .children(check),
        )
        .children(self.label.map(Label::new));
        if self.disabled {
            return row.opacity(0.45);
        }
        if let Some(handler) = self.on_click {
            row = row.on_click(handler);
        }
        row
    }
}

/// One segment of a [`SegmentedControl`].
pub struct Segment {
    label: Option<SharedString>,
    icon: Option<IconSource>,
}

impl Segment {
    /// A text segment.
    pub fn label(label: impl Into<SharedString>) -> Self {
        Self {
            label: Some(label.into()),
            icon: None,
        }
    }

    /// An icon segment.
    pub fn icon(icon: impl Into<IconSource>) -> Self {
        Self {
            label: None,
            icon: Some(icon.into()),
        }
    }

    /// Add an icon before the label.
    pub fn with_icon(mut self, icon: impl Into<IconSource>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

type SelectHandler = std::rc::Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>;

/// Mutually exclusive choices drawn as one control, the way macOS draws them.
#[derive(IntoElement)]
pub struct SegmentedControl {
    id: ElementId,
    segments: Vec<Segment>,
    selected: usize,
    size: ControlSize,
    on_select: Option<SelectHandler>,
}

impl SegmentedControl {
    /// A control with `selected` chosen.
    pub fn new(
        id: impl Into<ElementId>,
        segments: impl IntoIterator<Item = Segment>,
        selected: usize,
    ) -> Self {
        Self {
            id: id.into(),
            segments: segments.into_iter().collect(),
            selected,
            size: ControlSize::default(),
            on_select: None,
        }
    }

    /// Control height.
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// Called with the index of the segment clicked or pressed with Enter or Space. Each segment
    /// is a tab stop.
    pub fn on_select(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(std::rc::Rc::new(handler));
        self
    }
}

impl RenderOnce for SegmentedControl {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let theme = &theme;
        let height = self.size.height(theme);
        let inset = px(2.);
        let inner = height - inset * 2.;
        let id = self.id.clone();
        selector(div().id(self.id), &id)
            .flex()
            .flex_none()
            .items_center()
            .h(height)
            .p(inset)
            .gap(px(1.))
            .rounded(theme.control_radius(height))
            .bg(theme.colors.sunken)
            .border_1()
            .border_color(theme.colors.border)
            .children({
                let mut items = Vec::new();
                for (index, segment) in self.segments.into_iter().enumerate() {
                    let selected = index == self.selected;
                    let selected_fill = mix(
                        theme.colors.elevated,
                        theme.colors.text,
                        if theme.appearance.is_dark() { 0.08 } else { 0. },
                    );
                    let text = if selected {
                        theme.colors.text
                    } else {
                        theme.colors.text_muted
                    };
                    let item_id =
                        ElementId::NamedChild(Box::new(id.clone()), index.to_string().into());
                    let mut item = focusable(
                        div().id(item_id.clone()),
                        &item_id,
                        false,
                        Theme::concentric(theme.control_radius(height), inset),
                        window,
                        cx,
                    )
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_none()
                    .gap(px(5.))
                    .h(inner)
                    .min_w(inner)
                    .whitespace_nowrap()
                    .px(theme.density.control_padding * 0.8)
                    .rounded(Theme::concentric(theme.control_radius(height), inset))
                    .text_size(theme.typography.size_sm)
                    .font_weight(theme.typography.label_weight)
                    .text_color(text)
                    .children(
                        segment
                            .icon
                            .map(|icon| Icon::new(icon).size(px(14.)).color(text)),
                    )
                    .children(segment.label);
                    if selected {
                        item = item.bg(selected_fill).shadow(theme.elevation.low.clone());
                    } else {
                        let hover = theme.colors.hover;
                        item = item.hover(move |style| style.bg(hover));
                    }
                    if let Some(handler) = self.on_select.clone() {
                        item = item.on_click(move |_, window, cx| handler(&index, window, cx));
                    }
                    // The chosen segment's fill fades in as the old one's fades out.
                    items.push(transition(
                        item,
                        ElementId::NamedChild(Box::new(id.clone()), format!("fill-{index}").into()),
                        &selected,
                        theme.motion.fast,
                        window,
                        cx,
                        move |item, delta| {
                            let strength = if selected { delta } else { 1. - delta };
                            item.bg(selected_fill.opacity(strength))
                        },
                    ));
                }
                items
            })
    }
}

/// A horizontal slider showing a normalised value from 0 to 1.
#[derive(IntoElement)]
pub struct Slider {
    value: f32,
    label: Option<SharedString>,
    readout: Option<SharedString>,
    bipolar: bool,
}

impl Slider {
    /// A slider at `value`, clamped to 0..=1.
    pub fn new(value: f32) -> Self {
        Self {
            value: value.clamp(0., 1.),
            label: None,
            readout: None,
            bipolar: false,
        }
    }

    /// Caption to the left.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Formatted value to the right, for example "-6.0 dB".
    pub fn readout(mut self, readout: impl Into<SharedString>) -> Self {
        self.readout = Some(readout.into());
        self
    }

    /// Fill from the centre, for pan and other signed values.
    pub fn bipolar(mut self) -> Self {
        self.bipolar = true;
        self
    }
}

impl RenderOnce for Slider {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let track_height = px(4.);
        let thumb = px(14.);
        let (start, end) = if self.bipolar {
            (self.value.min(0.5), self.value.max(0.5))
        } else {
            (0., self.value)
        };
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .w_full()
            .children(self.label.map(|label| {
                div().w(px(64.)).flex_none().child(
                    Label::new(label)
                        .size(TextSize::Small)
                        .color(TextColor::Muted),
                )
            }))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .h(thumb)
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .top((thumb - track_height) / 2.)
                            .h(track_height)
                            .rounded(track_height / 2.)
                            .bg(theme.colors.sunken)
                            .border_1()
                            .border_color(theme.colors.border),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(relative(start))
                            .w(relative(end - start))
                            .top((thumb - track_height) / 2.)
                            .h(track_height)
                            .rounded(track_height / 2.)
                            .bg(on_fill(theme)),
                    )
                    // The thumb's left edge sits at value × (width − thumb), so it never overhangs.
                    .child(
                        div()
                            .absolute()
                            .left(relative(self.value))
                            .ml(-thumb * self.value)
                            .top_0()
                            .size(thumb)
                            .rounded(thumb / 2.)
                            .bg(gpui::white())
                            .border_1()
                            .border_color(gpui::black().opacity(0.12))
                            .shadow(theme.elevation.medium.clone()),
                    ),
            )
            .children(self.readout.map(|readout| {
                div().w(px(56.)).flex_none().flex().justify_end().child(
                    Label::new(readout)
                        .size(TextSize::Small)
                        .mono()
                        .color(TextColor::Muted),
                )
            }))
    }
}

/// A rotary knob, the creator-tool control: a 270° arc, a pointer and a value underneath.
#[derive(IntoElement)]
pub struct Knob {
    value: f32,
    diameter: Pixels,
    label: Option<SharedString>,
    readout: Option<SharedString>,
    bipolar: bool,
    color: Option<Hsla>,
}

impl Knob {
    /// A knob at `value`, clamped to 0..=1.
    pub fn new(value: f32) -> Self {
        Self {
            value: value.clamp(0., 1.),
            diameter: px(40.),
            label: None,
            readout: None,
            bipolar: false,
            color: None,
        }
    }

    /// Outer diameter of the arc.
    pub fn diameter(mut self, diameter: Pixels) -> Self {
        self.diameter = diameter;
        self
    }

    /// Name under the knob.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Formatted value under the name.
    pub fn readout(mut self, readout: impl Into<SharedString>) -> Self {
        self.readout = Some(readout.into());
        self
    }

    /// Draw the arc from twelve o'clock, for pan and detune.
    pub fn bipolar(mut self) -> Self {
        self.bipolar = true;
        self
    }

    /// Arc colour; defaults to the accent. Use a track colour to tie a knob to its channel.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

/// The knob's sweep: from 7:30 clockwise to 4:30, in radians from +x, y pointing down.
const SWEEP_START: f32 = 0.75 * PI;
const SWEEP: f32 = 1.5 * PI;

pub(crate) fn arc(
    center: gpui::Point<Pixels>,
    radius: Pixels,
    from: f32,
    to: f32,
    width: Pixels,
) -> Option<gpui::Path<Pixels>> {
    if (to - from).abs() < 0.001 {
        return None;
    }
    let steps = (((to - from).abs() / (PI / 48.)).ceil() as usize).max(2);
    let mut builder = PathBuilder::stroke(width);
    for step in 0..=steps {
        let angle = from + (to - from) * step as f32 / steps as f32;
        let at = point(
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
        );
        if step == 0 {
            builder.move_to(at);
        } else {
            builder.line_to(at);
        }
    }
    builder.build().ok()
}

impl RenderOnce for Knob {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let diameter = self.diameter;
        let track = theme.colors.sunken;
        let ring = theme.colors.border_strong;
        let value_color = self.color.unwrap_or(theme.colors.accent);
        let dark = theme.appearance.is_dark();
        let body = mix(
            theme.colors.elevated,
            gpui::white(),
            if dark { 0.06 } else { 0. },
        );
        let body_edge = theme.colors.border_strong;
        let pointer = theme.colors.text;
        let value = self.value;
        let bipolar = self.bipolar;

        let dial = canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, _| {
                let center = bounds.center();
                let radius = diameter / 2. - px(3.);
                let width = px(3.);
                let end = SWEEP_START + SWEEP;
                if let Some(path) = arc(center, radius, SWEEP_START, end, width) {
                    window.paint_path(path, track);
                }
                if let Some(path) = arc(center, radius + px(2.), SWEEP_START, end, px(0.75)) {
                    window.paint_path(path, ring.opacity(0.5));
                }
                let origin = if bipolar {
                    SWEEP_START + SWEEP / 2.
                } else {
                    SWEEP_START
                };
                let at = SWEEP_START + SWEEP * value;
                let (from, to) = if at < origin {
                    (at, origin)
                } else {
                    (origin, at)
                };
                if let Some(path) = arc(center, radius, from, to, width) {
                    window.paint_path(path, value_color);
                }
                let body_radius = radius - px(6.);
                let body_bounds = Bounds::new(
                    point(center.x - body_radius, center.y - body_radius),
                    size(body_radius * 2., body_radius * 2.),
                );
                window.paint_quad(
                    fill(body_bounds, body)
                        .corner_radii(body_radius)
                        .border_widths(px(1.))
                        .border_color(body_edge),
                );
                let mut needle = PathBuilder::stroke(px(2.));
                needle.move_to(point(
                    center.x + body_radius * 0.25 * at.cos(),
                    center.y + body_radius * 0.25 * at.sin(),
                ));
                needle.line_to(point(
                    center.x + body_radius * 0.8 * at.cos(),
                    center.y + body_radius * 0.8 * at.sin(),
                ));
                if let Ok(path) = needle.build() {
                    window.paint_path(path, pointer);
                }
            },
        )
        .size(diameter);

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(2.))
            .child(dial)
            .children(self.label.map(|label| {
                Label::new(label)
                    .size(TextSize::XSmall)
                    .color(TextColor::Muted)
            }))
            .children(
                self.readout
                    .map(|readout| Label::new(readout).size(TextSize::XSmall).mono()),
            )
    }
}

/// A framed text field: the sunken well, the control height and radius, and the focus ring.
///
/// Atelier draws the frame; the application owns the editing. Put its editable input inside with
/// `.child(...)` — the element that holds the caret, the selection and the IME — and route the
/// field's own events to it: the field is an interactive element, so it takes `.id(...)`,
/// `.track_focus(...)`, `.on_mouse_down(...)`, `.on_key_down(...)`, `.on_action(...)` and
/// `.key_context(...)` like a `div`. While its focus handle is focused it draws the ring by
/// itself; [`TextField::focused`] draws it by hand. Tell it [`TextField::empty`] so the
/// placeholder shows under the application's input. A static [`TextField::value`] is for
/// mock-ups.
///
/// ```ignore
/// TextField::new()
///     .id("project-name")
///     .track_focus(&self.name.focus_handle(cx))
///     .on_mouse_down(MouseButton::Left, cx.listener(Self::place_caret))
///     .placeholder("Untitled")
///     .empty(self.name.read(cx).is_empty())
///     .child(self.name.clone())
/// ```
#[derive(IntoElement)]
pub struct TextField {
    base: Div,
    id: Option<ElementId>,
    value: Option<SharedString>,
    placeholder: Option<SharedString>,
    icon: Option<IconSource>,
    focused: bool,
    empty: Option<bool>,
    size: ControlSize,
    children: Vec<AnyElement>,
    trailing: Option<AnyElement>,
    on_click: Option<ClickHandler>,
}

impl TextField {
    /// An empty field.
    pub fn new() -> Self {
        Self {
            base: div(),
            id: None,
            value: None,
            placeholder: None,
            icon: None,
            focused: false,
            empty: None,
            size: ControlSize::default(),
            children: Vec::new(),
            trailing: None,
            on_click: None,
        }
    }

    /// An id, which keeps the field's interaction state across frames and becomes its test
    /// selector. Needed for [`TextField::on_click`].
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Static content.
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Text shown while empty.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Whether the application's input is empty, which decides whether the placeholder shows.
    /// Without it the field counts as empty while it has no value and no children.
    pub fn empty(mut self, empty: bool) -> Self {
        self.empty = Some(empty);
        self
    }

    /// Leading icon, typically [`IconName::Search`].
    pub fn icon(mut self, icon: impl Into<IconSource>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Trailing content inside the frame: a clear button, a unit, a shortcut.
    pub fn trailing(mut self, trailing: impl IntoElement) -> Self {
        self.trailing = Some(trailing.into_any_element());
        self
    }

    /// Draw the focus ring by hand. A field tracking a focus handle draws it while that handle
    /// is focused without being told.
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Control height.
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// Called on click. Needs [`TextField::id`].
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl Default for TextField {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for TextField {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for TextField {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl ParentElement for TextField {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements)
    }
}

impl RenderOnce for TextField {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let height = self.size.height(theme);
        let empty = self
            .empty
            .unwrap_or(self.value.is_none() && self.children.is_empty());
        let ring = theme.focus_ring();
        let accent = theme.colors.accent;
        let field = self
            .base
            .flex()
            .items_center()
            .gap(px(6.))
            .h(height)
            .w_full()
            .px(theme.density.control_padding * 0.8)
            .rounded(theme.control_radius(height))
            .bg(theme.colors.sunken)
            .border_1()
            .border_color(if self.focused {
                theme.colors.accent
            } else {
                theme.colors.border
            })
            .when(self.focused, |el| el.shadow(ring.clone()))
            // A text field shows its ring whenever it has focus, from the pointer or the keyboard:
            // it is where typing goes.
            .focus(move |style| style.border_color(accent).shadow(ring))
            .cursor_text()
            .text_size(theme.typography.size_md)
            .children(
                self.icon
                    .map(|icon| Icon::new(icon).size(px(14.)).color(theme.colors.text_faint)),
            )
            .children(self.value.map(|value| {
                div()
                    .flex_1()
                    .truncate()
                    .text_color(theme.colors.text)
                    .child(value)
            }))
            .children(self.children)
            .children(
                empty
                    .then_some(self.placeholder)
                    .flatten()
                    .map(|placeholder| {
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(theme.colors.text_faint)
                            .child(placeholder)
                    }),
            )
            .children(self.trailing);
        match self.id {
            Some(id) => {
                let mut field = selector(field, &id).id(id);
                if let Some(handler) = self.on_click {
                    field = field.on_click(handler);
                }
                field.into_any_element()
            }
            None => field.into_any_element(),
        }
    }
}
