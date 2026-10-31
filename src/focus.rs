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
//! and focuses an empty child of it once. Not the root itself: an element that tracks focus
//! marks every press inside it handled, and a handled press on the title bar moves no window.
//!
//! A component given an id also uses it as its `debug_selector`, so a test can find it with
//! `VisualTestContext::debug_bounds` (enable gpui's `test-support` feature in the test build).
//!
//! Every tab stop carries the key context [`CONTROL_CONTEXT`] (`AtelierControl`). While one
//! holds focus, Atelier takes Enter and Space before any key binding sees them, so a
//! single-key shortcut such as Space for play never fires there: the key presses the control
//! (gpui presses it on the key's release). A track — [`crate::AudioPlayer`]'s scrubber and
//! volume — also carries [`TRACK_CONTEXT`] and takes the arrows, Home and End the same way.
//! An application can also keep a binding off controls outright with
//! `Some("!AtelierControl")`. Arrow keys show the ring only while a control already holds
//! focus, so arrows that move a playhead or a note leave no ring behind.
//!
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
        FocusPrevious,
        /// An arrow key on a focused track, such as [`crate::AudioPlayer`]'s scrubber: one step
        /// back.
        StepBackward,
        /// One step forward.
        StepForward,
        /// Home on a focused track: to the start.
        StepToStart,
        /// End on a focused track: to the end.
        StepToEnd
    ]
);

/// The key context of every Atelier tab stop. While the focused element carries it, Enter and
/// Space go to the control and no binding sees them. Scope a binding away from controls
/// altogether with `KeyBinding::new("x", Mute, Some("!AtelierControl"))`.
pub const CONTROL_CONTEXT: &str = "AtelierControl";

/// The key context of a focused track that arrow keys move, inside [`CONTROL_CONTEXT`].
pub const TRACK_CONTEXT: &str = "AtelierTrack";

/// Whether the keyboard moved focus last, which is when the ring shows.
#[derive(Default)]
struct FocusVisible(bool);

impl Global for FocusVisible {}

/// Bind Tab, Shift-Tab and the control keys, and watch for the keyboard taking over from the
/// pointer. Binds only once however often it is called.
pub(crate) fn init(cx: &mut App) {
    if cx.has_global::<FocusVisible>() {
        return;
    }
    cx.set_global(FocusVisible::default());
    cx.bind_keys([
        KeyBinding::new("tab", FocusNext, None),
        KeyBinding::new("shift-tab", FocusPrevious, None),
    ]);
    cx.bind_keys(crate::selectable::bindings());
    // Interceptors run before any binding is matched, so the keys a focused control owns never
    // reach an application's shortcut of the same key, however it is bound.
    cx.intercept_keystrokes(|event, window, cx| {
        let keystroke = &event.keystroke;
        let key = keystroke.key.as_str();
        let arrow = matches!(key, "up" | "down" | "left" | "right");
        // The context of the focused element itself, not of one it sits inside.
        let focused = event.context_stack.last();
        let on = |context: &str| focused.is_some_and(|focused| focused.contains(context));
        // An arrow shows the ring only when it acts on a control: in a timeline it moves the
        // playhead, and no ring should appear there.
        if key == "tab" || arrow && on_control(window) {
            set_focus_visible(true, cx);
        }
        if keystroke.modifiers.modified() {
            return;
        }
        if matches!(key, "enter" | "space") && on(CONTROL_CONTEXT) {
            // Left for the control, which gpui presses on the key's release.
            cx.stop_propagation();
        } else if on(TRACK_CONTEXT) {
            let step: Option<Box<dyn gpui::Action>> = match key {
                "left" | "down" => Some(Box::new(StepBackward)),
                "right" | "up" => Some(Box::new(StepForward)),
                "home" => Some(Box::new(StepToStart)),
                "end" => Some(Box::new(StepToEnd)),
                _ => None,
            };
            if let Some(step) = step {
                window.dispatch_action(step, cx);
                cx.stop_propagation();
            }
        }
    })
    .detach();
}

/// Whether an Atelier control holds focus in `window`.
fn on_control(window: &Window) -> bool {
    window
        .context_stack()
        .iter()
        .any(|context| context.contains(CONTROL_CONTEXT))
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
    focusable_with(element, id, disabled, radius, None, window, cx)
}

/// [`focusable`], tracking `handle` when the application passed one, so it can move focus there
/// itself, for example back to the button that opened a popover once Escape closes it. The
/// handle becomes a tab stop and keeps the tab index it was given.
pub(crate) fn focusable_with(
    element: Stateful<Div>,
    id: &ElementId,
    disabled: bool,
    radius: Pixels,
    handle: Option<&FocusHandle>,
    window: &mut Window,
    cx: &App,
) -> Stateful<Div> {
    let element = selector(element, id);
    if disabled {
        return element;
    }
    let handle = match handle {
        Some(handle) => handle.clone().tab_stop(true),
        None => own_handle(id, window, cx),
    };
    let ring = handle.is_focused(window) && focus_visible(cx);
    let element = element
        .track_focus(&handle)
        .key_context(CONTROL_CONTEXT)
        .relative();
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

/// The focus handle a component with `id` keeps for itself between frames.
pub(crate) fn own_handle(id: &ElementId, window: &mut Window, cx: &App) -> FocusHandle {
    window.with_global_id(
        ElementId::NamedChild(Box::new(id.clone()), "focus".into()),
        |global_id, window| {
            window.with_element_state(global_id, |handle: Option<FocusHandle>, _| {
                let handle =
                    handle.unwrap_or_else(|| cx.focus_handle().tab_index(0).tab_stop(true));
                (handle.clone(), handle)
            })
        },
    )
}

/// Give `element` `id` as its test selector.
pub(crate) fn selector<E: InteractiveElement>(element: E, id: &ElementId) -> E {
    element.debug_selector(|| id.to_string())
}
