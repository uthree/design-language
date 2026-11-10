//! Value controls: switches, checkboxes, segmented controls, sliders and knobs.
//!
//! Every one of them works as it is dropped in: the user can click, drag, Tab to and step it,
//! and it moves. How the application hears about it is up to the application:
//!
//! - **Store the value.** Pass the value the application keeps and a handler that stores the
//!   new one (`on_change`, or `on_select` on a [`SegmentedControl`]). The control then shows
//!   only what it is given, so the application can clamp, undo or ignore a change.
//! - **Leave it to the control.** With no handler, the control keeps the value it was moved to
//!   for itself, starting from the one it was built with, and takes the application's value
//!   again whenever that changes. Good for a mock-up, a demo, or a setting nobody reads yet.
//!
//! Either way the control needs an id that stays the same between frames. A [`Slider`] or a
//! [`Knob`] built without [`Slider::id`] only shows its value, like a meter; `disabled(true)`
//! greys any of them out and leaves them unmoved.

use std::f32::consts::PI;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, Div, ElementId, Hsla, InteractiveElement, Interactivity,
    IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement,
    PathBuilder, Pixels, Point, RenderOnce, SharedString, Stateful, StatefulInteractiveElement,
    StyleRefinement, Styled, Window, canvas, div, fill, point, prelude::FluentBuilder, px,
    relative, size,
};

use crate::button::ControlSize;
use crate::focus::{
    CONTROL_CONTEXT, StepBackward, StepForward, StepToEnd, StepToStart, TRACK_CONTEXT, focusable,
    selector,
};
use crate::icon::{Icon, IconName, IconSource};
use crate::motion::{lerp, transition};
use crate::text::{Label, TextColor, TextSize};
use crate::theme::{ActiveTheme, PrimaryStyle, Theme, mix};

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// The fill of an "on" control: the accent, or the foreground in solid-primary directions.
pub(crate) fn on_fill(theme: &Theme) -> Hsla {
    match theme.primary_style {
        PrimaryStyle::Accent => theme.colors.accent,
        PrimaryStyle::Solid => theme.colors.solid,
    }
}

/// The state a [`Switch`] or [`Checkbox`] draws and the handler its click runs: the
/// application's, or one that flips the state the control keeps for itself.
fn toggle(
    id: &ElementId,
    on: bool,
    on_click: Option<ClickHandler>,
    disabled: bool,
    window: &mut Window,
    cx: &mut App,
) -> (bool, Option<ClickHandler>) {
    if on_click.is_some() || disabled {
        return (on, on_click);
    }
    let (on, set) = held(id, on, window, cx);
    (
        on,
        Some(Box::new(move |_, window, cx| set(&!on, window, cx))),
    )
}

/// An on/off switch, drawn as a track and a thumb. Without a handler it flips by itself; see
/// the [module notes](self).
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

    /// Called on click, Enter or Space with the new value, the opposite of the one shown:
    /// store it. Use this or [`Switch::on_click`], not both.
    pub fn on_change(self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        let next = !self.on;
        self.on_click(move |_, window, cx| handler(&next, window, cx))
    }
}

impl RenderOnce for Switch {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (on, on_click) = toggle(
            &self.id,
            self.on,
            self.on_click.take(),
            self.disabled,
            window,
            cx,
        );
        self.on = on;
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
        if let Some(handler) = on_click {
            row = row.on_click(handler);
        }
        row
    }
}

/// A checkbox, with an optional label. Without a handler it flips by itself.
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

    /// Called on click, Enter or Space with the new value, the opposite of the one shown:
    /// store it. Use this or [`Checkbox::on_click`], not both.
    pub fn on_change(self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        let next = !self.checked;
        self.on_click(move |_, window, cx| handler(&next, window, cx))
    }
}

impl RenderOnce for Checkbox {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (checked, on_click) = toggle(
            &self.id,
            self.checked,
            self.on_click.take(),
            self.disabled,
            window,
            cx,
        );
        self.checked = checked;
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
        if let Some(handler) = on_click {
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

type SelectHandler = Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>;

/// Mutually exclusive choices drawn as one control, the way macOS draws them. Without
/// [`SegmentedControl::on_select`] it keeps the choice the user makes for itself.
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
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for SegmentedControl {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (selected_index, on_select) = match self.on_select {
            Some(handler) => (self.selected, handler),
            None => held(&self.id, self.selected, window, cx),
        };
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
                    let selected = index == selected_index;
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
                    let handler = on_select.clone();
                    item = item.on_click(move |_, window, cx| handler(&index, window, cx));
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

/// A change to a value control's value, from 0 to 1.
pub(crate) type ValueHandler = Rc<dyn Fn(&f32, &mut Window, &mut App) + 'static>;
type Formatter = Rc<dyn Fn(f32) -> SharedString + 'static>;

/// Where a control's new value goes.
pub(crate) type Setter<T> = Rc<dyn Fn(&T, &mut Window, &mut App) + 'static>;

/// The value a control keeps for itself while the application gives it no handler. It starts at
/// the value the control was built with and follows that value whenever the application changes
/// it; in between it keeps whatever the user set.
struct Held<T> {
    given: T,
    value: T,
}

/// The value to draw and where a change goes, for a control the application does not listen to:
/// state kept under `id` between frames, so the control still moves when it is used.
pub(crate) fn held<T: Clone + PartialEq + 'static>(
    id: &ElementId,
    given: T,
    window: &mut Window,
    cx: &mut App,
) -> (T, Setter<T>) {
    let state = window.use_keyed_state(
        ElementId::NamedChild(Box::new(id.clone()), "held".into()),
        cx,
        {
            let given = given.clone();
            move |_, _| Held {
                given: given.clone(),
                value: given,
            }
        },
    );
    let value = state.update(cx, |held, _| {
        if held.given != given {
            held.given = given.clone();
            held.value = given;
        }
        held.value.clone()
    });
    let set = Rc::new(move |value: &T, _: &mut Window, cx: &mut App| {
        state.update(cx, |held, cx| {
            held.value = value.clone();
            cx.notify();
        })
    });
    (value, set)
}

/// How the pointer moves a value control.
#[derive(Clone, Copy)]
enum Gesture {
    /// The value is where the pointer is along the control, `inset` in from either end: a click
    /// jumps there and a drag follows.
    Along { inset: Pixels },
    /// A drag up or to the right turns the value up from where it was, [`TURN_TRAVEL`] pixels for
    /// the whole range; a click alone changes nothing.
    Turn,
}

/// Pixels of drag that turn a knob through its whole range.
const TURN_TRAVEL: f32 = 200.;

/// Where a value control was last drawn and the drag in progress on it.
#[derive(Default)]
struct Drag {
    bounds: Bounds<Pixels>,
    dragging: bool,
    from: Point<Pixels>,
    from_value: f32,
}

/// The live parts of a [`Slider`] or [`Knob`].
struct Live {
    id: ElementId,
    value: f32,
    step: f32,
    reset: Option<f32>,
    set: ValueHandler,
}

/// Make `element`, already a tab stop, move a value from 0 to 1: the pointer per `gesture`, a
/// double click back to `reset`, and the arrow keys, Home and End once Tab has reached it.
fn value_control(
    element: Stateful<Div>,
    live: Live,
    gesture: Gesture,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let Live {
        id,
        value,
        step,
        reset,
        set,
    } = live;
    let state = window.use_keyed_state(
        ElementId::NamedChild(Box::new(id), "drag".into()),
        cx,
        |_, _| Drag::default(),
    );
    let dragging = state.read(cx).dragging;
    let at = move |drag: &Drag, position: Point<Pixels>| -> f32 {
        match gesture {
            Gesture::Along { inset } => {
                let span = (drag.bounds.size.width - inset * 2.).max(px(1.));
                f32::from(position.x - drag.bounds.left() - inset) / f32::from(span)
            }
            Gesture::Turn => {
                let moved =
                    f32::from(position.x - drag.from.x) - f32::from(position.y - drag.from.y);
                drag.from_value + moved / TURN_TRAVEL
            }
        }
        .clamp(0., 1.)
    };
    let step_to = |to: f32| {
        let set = set.clone();
        move |window: &mut Window, cx: &mut App| set(&to.clamp(0., 1.), window, cx)
    };
    let (back, forward, start, end) = (
        step_to(value - step),
        step_to(value + step),
        step_to(0.),
        step_to(1.),
    );
    element
        .key_context(format!("{CONTROL_CONTEXT} {TRACK_CONTEXT}").as_str())
        .on_action(move |_: &StepBackward, window, cx| back(window, cx))
        .on_action(move |_: &StepForward, window, cx| forward(window, cx))
        .on_action(move |_: &StepToStart, window, cx| start(window, cx))
        .on_action(move |_: &StepToEnd, window, cx| end(window, cx))
        .on_mouse_down(MouseButton::Left, {
            let (state, set) = (state.clone(), set.clone());
            move |event: &MouseDownEvent, window, cx| {
                if event.click_count == 2 {
                    if let Some(reset) = reset {
                        set(&reset, window, cx);
                    }
                    return;
                }
                let next = state.update(cx, |drag, cx| {
                    drag.dragging = true;
                    drag.from = event.position;
                    drag.from_value = value;
                    cx.notify();
                    at(drag, event.position)
                });
                if matches!(gesture, Gesture::Along { .. }) {
                    set(&next, window, cx);
                }
            }
        })
        .child(
            canvas(
                {
                    let state = state.clone();
                    move |bounds, _, cx| state.update(cx, |drag, _| drag.bounds = bounds)
                },
                move |_, _, window, _| {
                    if !dragging {
                        return;
                    }
                    // Follow the pointer anywhere in the window until the button comes up.
                    window.on_mouse_event({
                        let state = state.clone();
                        move |event: &MouseMoveEvent, phase, window, cx| {
                            if phase.bubble() && event.pressed_button == Some(MouseButton::Left) {
                                let next = at(state.read(cx), event.position);
                                set(&next, window, cx);
                            }
                        }
                    });
                    window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                        if phase.bubble() {
                            state.update(cx, |drag, cx| {
                                drag.dragging = false;
                                cx.notify();
                            });
                        }
                    });
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
}

/// The builder state [`Slider`] and [`Knob`] share.
#[derive(Default)]
struct ValueParts {
    id: Option<ElementId>,
    label: Option<SharedString>,
    readout: Option<SharedString>,
    readout_with: Option<Formatter>,
    bipolar: bool,
    step: Option<f32>,
    reset: Option<f32>,
    disabled: bool,
    on_change: Option<ValueHandler>,
}

impl ValueParts {
    /// The value to draw, the readout to show under or beside it, and the live parts, if the
    /// control has an id and is not disabled.
    fn resolve(
        &mut self,
        value: f32,
        window: &mut Window,
        cx: &mut App,
    ) -> (f32, Option<SharedString>, Option<Live>) {
        let live = match (&self.id, self.disabled) {
            (Some(id), false) => {
                let (value, set) = match self.on_change.take() {
                    Some(handler) => (value, handler),
                    None => held(id, value, window, cx),
                };
                Some(Live {
                    id: id.clone(),
                    value,
                    step: self.step.unwrap_or(0.01),
                    reset: self.reset.or(self.bipolar.then_some(0.5)),
                    set,
                })
            }
            _ => None,
        };
        let value = live.as_ref().map_or(value, |live| live.value);
        let readout = match &self.readout_with {
            Some(format) => Some(format(value)),
            None => self.readout.take(),
        };
        (value, readout, live)
    }
}

/// Builders a [`Slider`] and a [`Knob`] share.
macro_rules! value_builders {
    ($name:ident) => {
        impl $name {
            /// Make it a control the user moves, keeping `id` between frames: a drag, the arrow
            /// keys, Home and End, and a double click back to [`Self::default_value`]. Without
            /// [`Self::on_change`] it keeps the value it is moved to for itself, starting from the
            /// one it was built with; it takes that one again whenever the application passes a
            /// different one. Without an id it only shows its value.
            pub fn id(mut self, id: impl Into<ElementId>) -> Self {
                self.parts.id = Some(id.into());
                self
            }

            /// Called with the new value, from 0 to 1, as the user moves it: store it and pass
            /// it back on the next render. The control then shows only the value it is given.
            /// Needs [`Self::id`].
            pub fn on_change(
                mut self,
                handler: impl Fn(&f32, &mut Window, &mut App) + 'static,
            ) -> Self {
                self.parts.on_change = Some(Rc::new(handler));
                self
            }

            /// Formatted value, for example "-6.0 dB". Prefer [`Self::readout_with`] for a live
            /// control, so the readout follows the value as it moves.
            pub fn readout(mut self, readout: impl Into<SharedString>) -> Self {
                self.parts.readout = Some(readout.into());
                self
            }

            /// Format the value shown, from 0 to 1, into the readout.
            pub fn readout_with(mut self, format: impl Fn(f32) -> SharedString + 'static) -> Self {
                self.parts.readout_with = Some(Rc::new(format));
                self
            }

            /// How far one arrow key moves the value; 0.01 by default.
            pub fn step(mut self, step: f32) -> Self {
                self.parts.step = Some(step);
                self
            }

            /// The value a double click returns to; the centre for a bipolar control, and none
            /// otherwise.
            pub fn default_value(mut self, value: f32) -> Self {
                self.parts.reset = Some(value.clamp(0., 1.));
                self
            }

            /// Greyed out, out of the Tab order and unmoved by the pointer.
            pub fn disabled(mut self, disabled: bool) -> Self {
                self.parts.disabled = disabled;
                self
            }
        }
    };
}

/// A horizontal slider over a value from 0 to 1.
///
/// Give it an [`id`](Slider::id) and the user can move it: a click or drag along the track, the
/// arrow keys, Home and End once Tab reaches it, and a double click back to its default. Listen
/// with [`on_change`](Slider::on_change) and store the value, or leave it to keep its own:
///
/// ```ignore
/// Slider::new(self.volume)
///     .id("volume")
///     .label("Volume")
///     .readout_with(|value| format!("{:.1} dB", (value - 1.) * 24.).into())
///     .on_change(cx.listener(|this, value: &f32, _, cx| {
///         this.volume = *value;
///         cx.notify();
///     }))
/// ```
#[derive(IntoElement)]
pub struct Slider {
    value: f32,
    parts: ValueParts,
}

impl Slider {
    /// A slider at `value`, clamped to 0..=1.
    pub fn new(value: f32) -> Self {
        Self {
            value: value.clamp(0., 1.),
            parts: ValueParts::default(),
        }
    }

    /// Caption to the left.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.parts.label = Some(label.into());
        self
    }

    /// Fill from the centre, for pan and other signed values. A double click returns it there.
    pub fn bipolar(mut self) -> Self {
        self.parts.bipolar = true;
        self
    }
}

value_builders!(Slider);

impl RenderOnce for Slider {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (value, readout, live) = self.parts.resolve(self.value, window, cx);
        let theme = cx.theme().clone();
        let track_height = px(4.);
        let thumb = px(14.);
        let (start, end) = if self.parts.bipolar {
            (value.min(0.5), value.max(0.5))
        } else {
            (0., value)
        };
        let track = div()
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
                    .bg(on_fill(&theme)),
            )
            // The thumb's left edge sits at value × (width − thumb), so it never overhangs.
            .child(
                div()
                    .absolute()
                    .left(relative(value))
                    .ml(-thumb * value)
                    .top_0()
                    .size(thumb)
                    .rounded(thumb / 2.)
                    .bg(gpui::white())
                    .border_1()
                    .border_color(gpui::black().opacity(0.12))
                    .shadow(theme.elevation.medium.clone()),
            );
        let track = match live {
            Some(live) => {
                let id = live.id.clone();
                let element = focusable(track.id(id.clone()), &id, false, thumb / 2., window, cx)
                    .cursor_pointer();
                value_control(
                    element,
                    live,
                    Gesture::Along { inset: thumb / 2. },
                    window,
                    cx,
                )
                .into_any_element()
            }
            None => track.into_any_element(),
        };
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .w_full()
            .when(self.parts.disabled, |row| row.opacity(0.45))
            .children(self.parts.label.take().map(|label| {
                div().w(px(64.)).flex_none().child(
                    Label::new(label)
                        .size(TextSize::Small)
                        .color(TextColor::Muted),
                )
            }))
            .child(track)
            .children(readout.map(|readout| {
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
///
/// Give it an [`id`](Knob::id) and the user can turn it: a drag up or to the right turns it up,
/// the arrow keys, Home and End step it once Tab reaches it, and a double click returns it to
/// its default. Listen with [`on_change`](Knob::on_change), or leave it to keep its own value:
///
/// ```ignore
/// Knob::new(self.drive)
///     .id("drive")
///     .label("Drive")
///     .readout_with(|value| format!("{:.0} %", value * 100.).into())
///     .on_change(cx.listener(|this, value: &f32, _, cx| {
///         this.drive = *value;
///         cx.notify();
///     }))
/// ```
#[derive(IntoElement)]
pub struct Knob {
    value: f32,
    diameter: Pixels,
    color: Option<Hsla>,
    parts: ValueParts,
}

impl Knob {
    /// A knob at `value`, clamped to 0..=1.
    pub fn new(value: f32) -> Self {
        Self {
            value: value.clamp(0., 1.),
            diameter: px(40.),
            color: None,
            parts: ValueParts::default(),
        }
    }

    /// Outer diameter of the arc.
    pub fn diameter(mut self, diameter: Pixels) -> Self {
        self.diameter = diameter;
        self
    }

    /// Name under the knob.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.parts.label = Some(label.into());
        self
    }

    /// Draw the arc from twelve o'clock, for pan and detune. A double click returns it there.
    pub fn bipolar(mut self) -> Self {
        self.parts.bipolar = true;
        self
    }

    /// Arc colour; defaults to the accent. Use a track colour to tie a knob to its channel.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

value_builders!(Knob);

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
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (value, readout, live) = self.parts.resolve(self.value, window, cx);
        let theme = cx.theme().clone();
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
        let bipolar = self.parts.bipolar;

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
        let dial = match live {
            Some(live) => {
                let id = live.id.clone();
                let element = focusable(
                    div().id(id.clone()).size(diameter).child(dial),
                    &id,
                    false,
                    diameter / 2.,
                    window,
                    cx,
                )
                .rounded(diameter / 2.)
                .cursor_ns_resize();
                value_control(element, live, Gesture::Turn, window, cx).into_any_element()
            }
            None => dial.into_any_element(),
        };

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(2.))
            .when(self.parts.disabled, |knob| knob.opacity(0.45))
            .child(dial)
            .children(self.parts.label.take().map(|label| {
                Label::new(label)
                    .size(TextSize::XSmall)
                    .color(TextColor::Muted)
            }))
            .children(readout.map(|readout| Label::new(readout).size(TextSize::XSmall).mono()))
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
/// The text area has no padding of its own: the placeholder starts at its left edge, where the
/// field's padding (and the icon, if any) ends. Give the application's input no left padding
/// either, so its first character lands where the placeholder's does. An input that keeps a
/// padding of its own says how much with [`TextField::input_inset`], and the placeholder moves
/// in by the same amount.
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
    input_inset: Pixels,
    icon: Option<IconSource>,
    focused: bool,
    empty: Option<bool>,
    size: ControlSize,
    area: bool,
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
            input_inset: px(0.),
            icon: None,
            focused: false,
            empty: None,
            size: ControlSize::default(),
            area: false,
            children: Vec::new(),
            trailing: None,
            on_click: None,
        }
    }

    /// A well for several lines: at least the control's height and two lines tall, growing with
    /// its input, the placeholder and icon at the top. For a [`crate::TextInput::multiline`].
    pub fn area(mut self) -> Self {
        self.area = true;
        self
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

    /// The left padding the application's input keeps inside the text area, so the placeholder
    /// starts where the input's first character does. 0 by default, which is what an input
    /// should have: let the field's own padding do the work.
    pub fn input_inset(mut self, inset: Pixels) -> Self {
        self.input_inset = inset;
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
            .when(self.area, |el| {
                el.h_auto()
                    .min_h(height + theme.typography.size_md * 1.4)
                    .items_start()
                    .py(px(5.))
            })
            // A text field shows its ring whenever it has focus, from the pointer or the keyboard:
            // it is where typing goes.
            .focus(move |style| style.border_color(accent).shadow(ring))
            .cursor_text()
            .text_size(theme.typography.size_md)
            .children(
                self.icon
                    .map(|icon| Icon::new(icon).size(px(14.)).color(theme.colors.text_faint)),
            )
            // The text area: the value or the application's input, with the placeholder laid
            // under it where the caret starts, not beside it.
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .items_center()
                    .when(self.area, |el| el.h_auto().items_start())
                    .children(
                        empty
                            .then_some(self.placeholder)
                            .flatten()
                            .map(|placeholder| {
                                div()
                                    .absolute()
                                    .top_0()
                                    .bottom_0()
                                    .left_0()
                                    .right_0()
                                    .pl(self.input_inset)
                                    .flex()
                                    .items_center()
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .min_w_0()
                                            .truncate()
                                            .text_color(theme.colors.text_faint)
                                            .child(placeholder),
                                    )
                            }),
                    )
                    .children(self.value.map(|value| {
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(theme.colors.text)
                            .child(value)
                    }))
                    .children(self.children),
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
