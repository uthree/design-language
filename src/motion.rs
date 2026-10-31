//! Motion: durations, easing, and the helpers components use to animate a change of state.
//!
//! gpui animates by time, not by property: an element wrapped in `with_animation` replays its
//! animator from 0 to 1 once per element id. There is no implicit transition when a value
//! changes. [`transition`] supplies one: it remembers, per element, how many times a value has
//! changed, and keys the animation by that count, so a new animation starts exactly when the
//! value changes and never on the first frame.
//!
//! Motion explains change; it is never decoration. Keep it short, and let an application turn it
//! off through [`Motion::enabled`] (for a reduced-motion preference).

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use std::f32::consts::FRAC_PI_2;

use gpui::{
    Animation, AnimationExt, AnyElement, App, Bounds, Div, ElementId, Hsla, IntoElement, Pixels,
    RenderOnce, SharedString, Styled, Transformation, Window, canvas, fill, point, px, radians,
    size, svg,
};

use crate::icon::IconName;

use crate::theme::ActiveTheme;

/// Durations and the global switch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    /// Whether components animate at all. Turn off for a reduced-motion preference: state still
    /// changes, it just does not move, and looping indicators stand still.
    pub enabled: bool,
    /// Small state changes: a switch thumb, a check mark, a chevron.
    pub fast: Duration,
    /// Content appearing: an expanded section, a new message, a popover.
    pub normal: Duration,
    /// Large or ambient movement: shimmer passes, indeterminate progress.
    pub slow: Duration,
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            enabled: true,
            fast: Duration::from_millis(140),
            normal: Duration::from_millis(220),
            slow: Duration::from_millis(1400),
        }
    }
}

/// Decelerating curve for things arriving or settling.
pub fn ease_out(delta: f32) -> f32 {
    1. - (1. - delta).powi(3)
}

/// Linear interpolation.
pub fn lerp(from: f32, to: f32, delta: f32) -> f32 {
    from + (to - from) * delta
}

/// The history of a value drawn by one element, kept across frames.
#[derive(Clone, Debug, PartialEq)]
pub struct Change<T> {
    /// How many times the value has changed. Zero until the first change, so nothing animates
    /// when an element first appears.
    pub count: usize,
    /// The value before the latest change: where a transition starts from.
    pub previous: T,
}

/// Record `value` for the element `id` and report how it has changed.
pub fn change<T: PartialEq + Clone + 'static>(
    id: impl Into<ElementId>,
    value: &T,
    window: &mut Window,
    cx: &mut App,
) -> Change<T> {
    let id = ElementId::NamedChild(std::sync::Arc::new(id.into()), "change".into());
    let state = window.use_keyed_state(id, cx, |_, _| {
        (
            value.clone(),
            Change {
                count: 0,
                previous: value.clone(),
            },
        )
    });
    let (current, change) = state.read(cx).clone();
    if current == *value {
        return change;
    }
    let change = Change {
        count: change.count + 1,
        previous: current,
    };
    // No notify: the change is already being drawn, and notifying would only redraw again.
    state.update(cx, |state, _| *state = (value.clone(), change.clone()));
    change
}

/// Animate `element` with `animator` each time `value` changes, over `duration`.
///
/// The animator receives the element and an eased delta from 0 to 1, and should move it from
/// where the previous value drew it to where the current one does.
pub fn transition<E, T>(
    element: E,
    id: impl Into<ElementId>,
    value: &T,
    duration: Duration,
    window: &mut Window,
    cx: &mut App,
    animator: impl Fn(E, f32) -> E + 'static,
) -> AnyElement
where
    E: IntoElement + 'static,
    T: PartialEq + Clone + 'static,
{
    let id = id.into();
    let count = change(id.clone(), value, window, cx).count;
    if count == 0 || !cx.theme().motion.enabled {
        return element.into_any_element();
    }
    let key: SharedString = format!("transition-{count}").into();
    element
        .with_animation(
            ElementId::NamedChild(std::sync::Arc::new(id), key),
            Animation::new(duration).with_easing(ease_out),
            animator,
        )
        .into_any_element()
}

/// Animate a number: when `value` changes, `animator` redraws `element` at values running from the
/// previous one to the new one. Build `element` at the new value; it is shown as is otherwise.
pub fn tween<E: IntoElement + 'static>(
    element: E,
    id: impl Into<ElementId>,
    value: f32,
    duration: Duration,
    window: &mut Window,
    cx: &mut App,
    animator: impl Fn(E, f32) -> E + 'static,
) -> AnyElement {
    let id = id.into();
    let change = change(id.clone(), &value, window, cx);
    if change.count == 0 || !cx.theme().motion.enabled {
        return element.into_any_element();
    }
    let from = change.previous;
    let key: SharedString = format!("tween-{}", change.count).into();
    element
        .with_animation(
            ElementId::NamedChild(std::sync::Arc::new(id), key),
            Animation::new(duration).with_easing(ease_out),
            move |element, delta| animator(element, lerp(from, value, delta)),
        )
        .into_any_element()
}

/// Play `animator` once, the first time the element `id` is drawn: a message arriving, a
/// popover opening.
pub fn appear<E: IntoElement + 'static>(
    element: E,
    id: impl Into<ElementId>,
    cx: &App,
    animator: impl Fn(E, f32) -> E + 'static,
) -> AnyElement {
    let motion = cx.theme().motion;
    if !motion.enabled {
        return element.into_any_element();
    }
    element
        .with_animation(
            ElementId::NamedChild(std::sync::Arc::new(id.into()), "appear".into()),
            Animation::new(motion.normal).with_easing(ease_out),
            animator,
        )
        .into_any_element()
}

/// Loop `animator` forever over `duration`, unless motion is off, when the element is drawn
/// at `rest` instead.
pub fn looping<E: IntoElement + 'static>(
    element: E,
    id: impl Into<ElementId>,
    duration: Duration,
    rest: f32,
    cx: &App,
    animator: impl Fn(E, f32) -> E + 'static,
) -> AnyElement {
    if !cx.theme().motion.enabled {
        return animator(element, rest).into_any_element();
    }
    element
        .with_animation(id, Animation::new(duration).repeat(), animator)
        .into_any_element()
}

/// A disclosure chevron that turns a quarter turn when `open` changes.
pub fn chevron(
    id: impl Into<ElementId>,
    open: bool,
    size: Pixels,
    color: Hsla,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let turn = |delta: f32| Transformation::rotate(radians(delta * FRAC_PI_2));
    let (from, to) = if open { (0., 1.) } else { (1., 0.) };
    let fast = cx.theme().motion.fast;
    transition(
        svg()
            .path(IconName::ChevronRight.path())
            .flex_none()
            .size(size)
            .text_color(color)
            .with_transformation(turn(to)),
        id,
        &open,
        fast,
        window,
        cx,
        move |chevron, delta| chevron.with_transformation(turn(lerp(from, to, delta))),
    )
}

/// Fade `element` in and settle it from a few pixels above, the first time it is drawn: content
/// a disclosure has just revealed.
pub fn reveal(element: Div, id: impl Into<ElementId>, cx: &App) -> AnyElement {
    appear(element, id, cx, |element, delta| {
        element.opacity(delta).mt(px(lerp(-4., 0., delta)))
    })
}

/// Where an element was last laid out, in window coordinates: the "from" of a [`Morph`].
///
/// Put [`Measure::probe`] inside the element (which must be `relative()`), and read
/// [`Measure::bounds`] when the hand-off starts.
#[derive(Clone, Debug, Default)]
pub struct Measure(Rc<Cell<Option<Bounds<Pixels>>>>);

impl Measure {
    /// Nothing measured yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// The bounds the probe last saw.
    pub fn bounds(&self) -> Option<Bounds<Pixels>> {
        self.0.get()
    }

    /// An invisible child that records its parent's bounds each frame.
    pub fn probe(&self) -> impl IntoElement + use<> {
        let cell = self.0.clone();
        canvas(move |bounds, _, _| cell.set(Some(bounds)), |_, _, _, _| {})
            .absolute()
            .top_0()
            .left_0()
            .size_full()
    }
}

/// A shared-element hand-off between two screens: a surface that grows from the rectangle the
/// person acted on to the rectangle of the screen that replaces it.
///
/// Only the shape moves. The arriving screen is laid out at its final size from its first
/// frame and fades in over the shape ([`appear`] with opacity); nothing inside either screen is
/// re-laid out while the shape grows. It runs once, over [`Motion::normal`], the first time an
/// id is drawn, and with motion off the shape is simply at its end.
///
/// ```ignore
/// // On "Compose with AI": remember where the card was and switch screens.
/// self.handoff = self.card.bounds();
/// // In the window's root, the shape first and the arriving screen over it:
/// .children(self.handoff.map(|from| Morph::new("compose-handoff", from)))
/// .child(appear(compose_screen, "compose-screen", cx, |screen, delta| screen.opacity(delta)))
/// // Drop `self.handoff` once the screen has settled, or keep it: at rest the shape is just the
/// // screen's own background.
/// ```
#[derive(IntoElement)]
pub struct Morph {
    id: ElementId,
    from: Bounds<Pixels>,
    to: Option<Bounds<Pixels>>,
    from_radius: Option<Pixels>,
    to_radius: Pixels,
    fill: Option<Hsla>,
    fill_to: Option<Hsla>,
    border: Option<(Hsla, Option<Hsla>)>,
    duration: Option<Duration>,
}

impl Morph {
    /// A shape that grows from `from` to the bounds of wherever it is placed: it fills its
    /// parent absolutely, so place it in the window's root (which must be `relative()`), before
    /// the arriving screen.
    pub fn new(id: impl Into<ElementId>, from: Bounds<Pixels>) -> Self {
        Self {
            id: id.into(),
            from,
            to: None,
            from_radius: None,
            to_radius: px(0.),
            fill: None,
            fill_to: None,
            border: None,
            duration: None,
        }
    }

    /// Grow to `to`, in window coordinates, instead of to the element's own bounds.
    pub fn to(mut self, to: Bounds<Pixels>) -> Self {
        self.to = Some(to);
        self
    }

    /// The corner radii at either end; from the large radius to square by default.
    pub fn radius(mut self, from: Pixels, to: Pixels) -> Self {
        self.from_radius = Some(from);
        self.to_radius = to;
        self
    }

    /// The surface's colour; the theme's surface by default, the colour of the panel it grows
    /// from.
    pub fn fill(mut self, fill: Hsla) -> Self {
        self.fill = Some(fill);
        self
    }

    /// The colour it ends in, when the arriving screen is not the panel's colour: an opaque
    /// panel that becomes a translucent veil over a backdrop. It blends from [`Morph::fill`]
    /// on the same clock and easing as the shape, alpha included.
    pub fn fill_to(mut self, fill: Hsla) -> Self {
        self.fill_to = Some(fill);
        self
    }

    /// A one-pixel border, as the panel it grows from has, fading to `to` (pass
    /// `transparent_black()` to let it go) or kept to the end with `None`.
    pub fn border(mut self, from: Hsla, to: Option<Hsla>) -> Self {
        self.border = Some((from, to));
        self
    }

    /// How long it takes; [`Motion::normal`] by default.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }
}

impl RenderOnce for Morph {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let motion = theme.motion;
        let duration = self.duration.unwrap_or(motion.normal);
        let fill_color = self.fill.unwrap_or(theme.colors.surface);
        let from_radius = self.from_radius.unwrap_or(theme.shape.radius_lg);
        let to_radius = self.to_radius;
        let started = window
            .use_keyed_state(
                ElementId::NamedChild(std::sync::Arc::new(self.id), "morph".into()),
                cx,
                |_, _| Instant::now(),
            )
            .read(cx)
            .to_owned();
        let delta = if motion.enabled && !duration.is_zero() {
            (started.elapsed().as_secs_f32() / duration.as_secs_f32()).min(1.)
        } else {
            1.
        };
        if delta < 1. {
            window.request_animation_frame();
        }
        let delta = ease_out(delta);
        let (from, to) = (self.from, self.to);
        let fill_color = match self.fill_to {
            Some(end) => mix_alpha(fill_color, end, delta),
            None => fill_color,
        };
        let border = self.border.map(|(start, end)| match end {
            Some(end) => mix_alpha(start, end, delta),
            None => start,
        });
        canvas(
            |_, _, _| {},
            move |own, _, window, _| {
                let to = to.unwrap_or(own);
                let at = |a: Pixels, b: Pixels| px(lerp(f32::from(a), f32::from(b), delta));
                let bounds = Bounds::new(
                    point(
                        at(from.origin.x, to.origin.x),
                        at(from.origin.y, to.origin.y),
                    ),
                    size(
                        at(from.size.width, to.size.width),
                        at(from.size.height, to.size.height),
                    ),
                );
                let mut quad = fill(bounds, fill_color).corner_radii(at(from_radius, to_radius));
                if let Some(border) = border {
                    quad = quad.border_widths(px(1.)).border_color(border);
                }
                window.paint_quad(quad);
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }
}

/// `a` to `b` by `amount`, the colour in RGB and the opacity alongside, so a solid panel can
/// thin to a veil of another colour.
fn mix_alpha(a: Hsla, b: Hsla, amount: f32) -> Hsla {
    let alpha = lerp(a.a, b.a, amount);
    let color = crate::theme::mix(Hsla { a: 1., ..a }, Hsla { a: 1., ..b }, amount);
    Hsla { a: alpha, ..color }
}

/// Turn animation off, for tests that press and drag over things that would otherwise still be
/// moving into place. The same switch a reduced-motion setting turns.
pub fn reduce_motion(cx: &mut App) {
    if let Some(theme) = cx.try_global::<crate::theme::Theme>() {
        let mut theme = theme.clone();
        theme.motion.enabled = false;
        cx.set_global(theme);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ease_out_spans_zero_to_one() {
        assert_eq!(ease_out(0.), 0.);
        assert_eq!(ease_out(1.), 1.);
        assert!(ease_out(0.5) > 0.5, "decelerates");
    }
}
