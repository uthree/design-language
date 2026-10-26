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

use std::time::Duration;

use std::f32::consts::FRAC_PI_2;

use gpui::{
    Animation, AnimationExt, AnyElement, App, Div, ElementId, Hsla, IntoElement, Pixels,
    SharedString, Styled, Transformation, Window, px, radians, svg,
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
    let id = ElementId::NamedChild(Box::new(id.into()), "change".into());
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
            ElementId::NamedChild(Box::new(id), key),
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
            ElementId::NamedChild(Box::new(id), key),
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
            ElementId::NamedChild(Box::new(id.into()), "appear".into()),
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
