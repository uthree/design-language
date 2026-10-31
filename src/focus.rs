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
//! Every tab stop carries the key context [`CONTROL_CONTEXT`] (`AtelierControl`). Enter and
//! Space press the focused control on the key's release, which gpui does only when the press
//! reached the control as well. A binding that matches first takes the press, so a single-key
//! shortcut such as Space for play has to keep off controls with `Some("!AtelierControl")`.
//! Atelier cannot take the key first on its own: an interceptor that stops the press stops it
//! for the control too. A track — [`crate::AudioPlayer`]'s scrubber and volume — also carries
//! [`TRACK_CONTEXT`] and takes the arrows, Home and End before any binding sees them, since a
//! step is an action rather than a press. Arrow keys show the ring only while a control
//! already holds focus, so arrows that move a playhead or a note leave no ring behind.
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

/// The key context of every Atelier tab stop. Enter and Space press the focused control unless
/// a binding takes them first, so scope a single-key binding away from controls with
/// `KeyBinding::new("space", Play, Some("!AtelierControl"))`.
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
        // Enter and Space are left alone: stopping the press here would also keep it from the
        // control, and gpui presses a control on the release only after seeing the press.
        if on(TRACK_CONTEXT) {
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
    root.on_action(|_: &FocusNext, window: &mut Window, cx: &mut App| window.focus_next(cx))
        .on_action(|_: &FocusPrevious, window: &mut Window, cx: &mut App| window.focus_prev(cx))
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
            inset: false,
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
        ElementId::NamedChild(std::sync::Arc::new(id.clone()), "focus".into()),
        |global_id, window| {
            window.with_element_state(global_id, |handle: Option<FocusHandle>, _| {
                let handle =
                    handle.unwrap_or_else(|| cx.focus_handle().tab_index(0).tab_stop(true));
                (handle.clone(), handle)
            })
        },
    );
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

/// Give `element` `id` as its test selector.
pub(crate) fn selector<E: InteractiveElement>(element: E, id: &ElementId) -> E {
    element.debug_selector(|| id.to_string())
}
