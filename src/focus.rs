//! Keyboard focus: the Tab order, the focus ring, and the selectors tests find components by.
//!
//! Every component that takes a click is also a tab stop: Tab and Shift-Tab move through them in
//! the order they are drawn, Enter and Space press the focused one (gpui fires its `on_click`),
//! and a disabled component leaves the order. The ring shows only once the keyboard has moved
//! focus, the way a browser's `:focus-visible` does, so a mouse click does not leave one behind.
//!
//! [`crate::init`] binds Tab and Shift-Tab to [`FocusNext`] and [`FocusPrevious`].
//! [`crate::WindowFrame`] handles both and gives the window somewhere to start from, so a window
//! built on it needs nothing more. A window with a root of its own adds [`tab_navigation`] to it
//! and focuses that root once.
//!
//! A component given an id also uses it as its `debug_selector`, so a test can find it with
//! `VisualTestContext::debug_bounds` (enable gpui's `test-support` feature in the test build).

use gpui::{
    App, BoxShadow, Div, ElementId, FocusHandle, Global, InteractiveElement, KeyBinding,
    ParentElement, Pixels, Stateful, Styled, Window, actions, div, point, px,
};

use crate::theme::{ActiveTheme, Theme};

actions!(
    atelier,
    [
        /// Move keyboard focus to the next tab stop.
        FocusNext,
        /// Move keyboard focus to the previous tab stop.
        FocusPrevious
    ]
);

/// Whether the keyboard moved focus last, which is when the ring shows.
#[derive(Default)]
struct FocusVisible(bool);

impl Global for FocusVisible {}

/// Bind Tab and Shift-Tab, and watch for the keyboard taking over from the pointer.
pub(crate) fn init(cx: &mut App) {
    cx.set_global(FocusVisible::default());
    cx.bind_keys([
        KeyBinding::new("tab", FocusNext, None),
        KeyBinding::new("shift-tab", FocusPrevious, None),
    ]);
    cx.intercept_keystrokes(|event, _, cx| {
        let key = event.keystroke.key.as_str();
        if matches!(key, "tab" | "up" | "down" | "left" | "right") {
            set_focus_visible(true, cx);
        }
    })
    .detach();
}

/// Whether focus rings show: true once the keyboard has moved focus, false again after a click.
pub fn focus_visible(cx: &App) -> bool {
    cx.try_global::<FocusVisible>()
        .is_some_and(|visible| visible.0)
}

/// Say whether the keyboard (true) or the pointer (false) moved focus last. [`WindowFrame`]
/// calls this on every press; a window with a root of its own calls it from a mouse-down
/// listener there.
///
/// [`WindowFrame`]: crate::WindowFrame
pub fn set_focus_visible(visible: bool, cx: &mut App) {
    if focus_visible(cx) != visible {
        cx.set_global(FocusVisible(visible));
    }
}

/// Handle [`FocusNext`] and [`FocusPrevious`] on `root`, the element at the top of a window.
pub fn tab_navigation<E: InteractiveElement>(root: E) -> E {
    root.on_action(|_: &FocusNext, window: &mut Window, _: &mut App| window.focus_next())
        .on_action(|_: &FocusPrevious, window: &mut Window, _: &mut App| window.focus_prev())
}

impl Theme {
    /// The focus halo of a filled control that holds focus while typing goes into it, such as
    /// [`crate::TextField`]: a hairline of the accent just outside its border, in a soft glow.
    /// Every other control draws a two-pixel outline in [`crate::Colors::focus_ring`] instead,
    /// a pixel clear of its edge, while the keyboard has focused it.
    pub fn focus_ring(&self) -> Vec<BoxShadow> {
        // A shadow needs some blur to be drawn at all; a pixel's worth keeps the edge crisp.
        let ring = |color, spread, blur| BoxShadow {
            color,
            offset: point(px(0.), px(0.)),
            blur_radius: blur,
            spread_radius: spread,
        };
        vec![
            ring(self.colors.focus_ring.opacity(0.4), px(3.), px(2.)),
            ring(self.colors.focus_ring, px(1.), px(1.)),
        ]
    }
}

/// Width of the keyboard focus outline.
pub(crate) const RING_WIDTH: f32 = 2.;
/// Gap between a control's edge and its focus outline.
pub(crate) const RING_GAP: f32 = 1.;

/// Make `element` a tab stop, unless `disabled`, and give it `id` as its test selector. While
/// the keyboard has focused it, it carries an outline `RING_GAP` outside its edge, following its
/// corner `radius`.
///
/// The outline is a child drawn over the element rather than a shadow: a shadow fills the
/// element's whole box, which shows through a control with no fill of its own.
pub(crate) fn focusable(
    element: Stateful<Div>,
    id: &ElementId,
    disabled: bool,
    radius: Pixels,
    window: &mut Window,
    cx: &App,
) -> Stateful<Div> {
    let element = selector(element, id);
    if disabled {
        return element;
    }
    let handle = window.with_global_id(
        ElementId::NamedChild(Box::new(id.clone()), "focus".into()),
        |global_id, window| {
            window.with_element_state(global_id, |handle: Option<FocusHandle>, _| {
                let handle =
                    handle.unwrap_or_else(|| cx.focus_handle().tab_index(0).tab_stop(true));
                (handle.clone(), handle)
            })
        },
    );
    let ring = handle.is_focused(window) && focus_visible(cx);
    let element = element.track_focus(&handle).relative();
    if !ring {
        return element;
    }
    let outset = px(RING_WIDTH + RING_GAP);
    element.child(
        div()
            .absolute()
            .top(-outset)
            .left(-outset)
            .right(-outset)
            .bottom(-outset)
            .rounded(radius + outset)
            .border(px(RING_WIDTH))
            .border_color(cx.theme().colors.focus_ring),
    )
}

/// Give `element` `id` as its test selector.
pub(crate) fn selector<E: InteractiveElement>(element: E, id: &ElementId) -> E {
    element.debug_selector(|| id.to_string())
}
