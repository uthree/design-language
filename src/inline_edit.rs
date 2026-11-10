//! Text typed where it is shown: a name, a lyric, a chord or a comment edited over its own label
//! rather than in a sheet over the window.
//!
//! A rename that dims the window and asks for the name again in a box in the middle makes the
//! person look away from the thing they are naming. [`InlineEdit`] draws a [`TextInput`] in a
//! field on top of the label it edits, at the label's place and about its size, with nothing
//! behind it dimmed.
//!
//! The label's rectangle comes from the view's own geometry (a lane, a row, a note) or, for a
//! label laid out by flex, from a [`SpotBook`]: the label carries [`SpotBook::spot`], which
//! records where it was painted, and [`SpotBook::get`] answers only while the label is still on
//! screen. When neither can say where the label is, open the application's sheet instead, so
//! nothing that was editable stops being so.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, ElementId, Entity, FocusHandle, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Pixels, RenderOnce, SharedString, Styled, Window, WindowId,
    anchored, canvas, deferred, div, point, prelude::FluentBuilder, px,
};

use crate::button::ControlSize;
use crate::controls::TextField;
use crate::input::{CancelText, SubmitText, TextInput};
use crate::text::{Label, TextSize};
use crate::theme::ActiveTheme;

#[derive(Default)]
struct Spots {
    /// The frame count of each window, moved on once per frame in which something was painted.
    frames: HashMap<WindowId, u64>,
    /// Windows whose count moves on at their next frame.
    scheduled: HashSet<WindowId>,
    /// Each key's last paint: its window, the frame and the rectangle.
    painted: HashMap<SharedString, (WindowId, u64, Bounds<Pixels>)>,
}

/// Where labels were last painted, so an [`InlineEdit`] can open over one laid out by flex.
///
/// Clone it freely; the clones share one book. Keep one per view (or per application).
#[derive(Clone, Default)]
pub struct SpotBook(Rc<RefCell<Spots>>);

impl SpotBook {
    /// An empty book.
    pub fn new() -> Self {
        Self::default()
    }

    /// An invisible child that records its parent's rectangle under `key` each time it is
    /// painted. The parent must be `relative()`.
    pub fn spot(&self, key: impl Into<SharedString>) -> gpui::Canvas<()> {
        let book = self.clone();
        let key = key.into();
        canvas(
            move |bounds, window, _| book.record(key, bounds, window),
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }

    fn tick(&self, window: &Window) {
        let id = window.window_handle().window_id();
        if self.0.borrow_mut().scheduled.insert(id) {
            let book = self.clone();
            window.on_next_frame(move |_, _| {
                let mut spots = book.0.borrow_mut();
                spots.scheduled.remove(&id);
                *spots.frames.entry(id).or_default() += 1;
            });
        }
    }

    fn record(&self, key: SharedString, bounds: Bounds<Pixels>, window: &Window) {
        self.tick(window);
        let id = window.window_handle().window_id();
        let mut spots = self.0.borrow_mut();
        let frame = spots.frames.get(&id).copied().unwrap_or_default();
        spots.painted.insert(key, (id, frame, bounds));
    }

    /// Where `key` was painted in `window`, if it was painted in the last frame: a label that
    /// scrolled away or whose panel closed answers `None`, since a field floating where it used
    /// to be would be worse than the sheet.
    pub fn get(&self, key: &str, window: &Window) -> Option<Bounds<Pixels>> {
        let id = window.window_handle().window_id();
        let spots = self.0.borrow();
        let frame = spots.frames.get(&id).copied().unwrap_or_default();
        spots
            .painted
            .get(key)
            .filter(|(painted_in, painted_at, _)| *painted_in == id && painted_at + 1 >= frame)
            .map(|(_, _, bounds)| *bounds)
    }
}

/// What leaving an inline field does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Leave {
    /// Write the text.
    Commit(SharedString),
    /// Close without writing: the text is as it was, or empty where empty is not an answer.
    Close,
}

pub(crate) fn leave(original: &str, text: &str, allow_empty: bool) -> Leave {
    if text == original || (!allow_empty && text.trim().is_empty()) {
        Leave::Close
    } else {
        Leave::Commit(text.to_owned().into())
    }
}

type CommitHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type CloseHandler = Rc<dyn Fn(&(), &mut Window, &mut App)>;
type ReturnHandler = Rc<dyn Fn(&Option<SharedString>, &mut Window, &mut App)>;

/// A field opened over a label to edit it in place.
///
/// Return commits and Escape closes. A press anywhere else commits too, as every list of names
/// does, except that a field left untouched, or emptied where empty is not an answer
/// ([`InlineEdit::allow_empty`]), closes instead: wandering off a box never writes an edit
/// nobody made. A [`TextInput::multiline`] input opens as an area, and Shift-Return breaks the
/// line.
///
/// The field is at least [`InlineEdit::min_width`] wide, so a chord one beat wide at a far zoom
/// still holds `bVII7` and a caret, and at most [`InlineEdit::max_width`]. It focuses its input
/// and selects all of it when it opens, so the first key replaces the old name.
///
/// ```ignore
/// // On double-click: remember what is being renamed and open the input on its name.
/// self.renaming = Some(track);
/// self.rename.update(cx, |input, cx| input.set_text(name, cx));
/// // In render, drawn anywhere in the tree (it floats over the window):
/// .children(self.renaming.and_then(|track| {
///     let over = self.spots.get(&format!("track-{track}"), window)?;
///     Some(
///         InlineEdit::new("rename", &self.rename, over)
///             .original(name)
///             .on_commit(cx.listener(|this, text, _, cx| this.rename_to(text.clone(), cx)))
///             .on_close(cx.listener(|this, _, _, _| this.renaming = None)),
///     )
/// }))
/// // The label itself, laid out by flex:
/// div().relative().child(name).child(self.spots.spot(format!("track-{track}")))
/// ```
#[derive(IntoElement)]
pub struct InlineEdit {
    id: ElementId,
    input: Entity<TextInput>,
    over: Bounds<Pixels>,
    original: SharedString,
    allow_empty: bool,
    min_width: Pixels,
    max_width: Pixels,
    error: Option<SharedString>,
    on_commit: Option<CommitHandler>,
    on_return: Option<ReturnHandler>,
    on_close: Option<CloseHandler>,
}

impl InlineEdit {
    /// A field editing `input` over `over`, the label's rectangle in window coordinates.
    pub fn new(id: impl Into<ElementId>, input: &Entity<TextInput>, over: Bounds<Pixels>) -> Self {
        Self {
            id: id.into(),
            input: input.clone(),
            over,
            original: SharedString::default(),
            allow_empty: false,
            min_width: px(150.),
            max_width: px(420.),
            error: None,
            on_commit: None,
            on_return: None,
            on_close: None,
        }
    }

    /// What the label said, so that leaving the field with it unchanged closes without an
    /// edit.
    pub fn original(mut self, original: impl Into<SharedString>) -> Self {
        self.original = original.into();
        self
    }

    /// Commit an emptied field rather than close it, where empty is an answer (a comment's
    /// second line, an optional lyric).
    pub fn allow_empty(mut self) -> Self {
        self.allow_empty = true;
        self
    }

    /// The narrowest the field is drawn, whatever it covers. 150 px by default.
    pub fn min_width(mut self, width: Pixels) -> Self {
        self.min_width = width;
        self
    }

    /// The widest the field is drawn. 420 px by default: a lyric line twenty bars long does not
    /// need a box twenty bars long.
    pub fn max_width(mut self, width: Pixels) -> Self {
        self.max_width = width;
        self
    }

    /// Why the last commit was refused, shown under the field, which stays open.
    pub fn error(mut self, error: impl Into<SharedString>) -> Self {
        self.error = Some(error.into());
        self
    }

    /// Called with the new text on Return or a press elsewhere. Close the field (stop drawing
    /// it) when the edit is taken, or keep it open with [`InlineEdit::error`].
    pub fn on_commit(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_commit = Some(Rc::new(handler));
        self
    }

    /// Called on Return instead of [`InlineEdit::on_commit`] and [`InlineEdit::on_close`],
    /// with the text to write, or `None` when there is nothing to write: for a list editor whose
    /// Return writes the cell and walks on to the same cell of the next row.
    pub fn on_return(
        mut self,
        handler: impl Fn(&Option<SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_return = Some(Rc::new(handler));
        self
    }

    /// Called on Escape, or when the field is left with nothing to write. Stop drawing it.
    pub fn on_close(mut self, handler: impl Fn(&(), &mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for InlineEdit {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (focused, multiline) = {
            let input = self.input.read(cx);
            (input.is_focused(window), input.buffer().is_multiline())
        };
        // Focus and select all once, when the field first opens, remembering where focus was so
        // that it goes back there when the field closes.
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| None::<Opened>);
        // A field moved on to another label (a list editor walking down a column) opens afresh
        // there: focused, everything selected.
        let target = (self.original.clone(), self.over.origin);
        match state.read(cx) {
            None => {
                let previous = window.focused(cx);
                state.update(cx, |state, _| {
                    *state = Some(Opened {
                        previous,
                        finished: false,
                        target,
                    })
                });
                self.input.update(cx, |input, cx| {
                    input.focus(window);
                    input.select_all(cx);
                });
            }
            Some(opened) if opened.target != target => {
                state.update(cx, |state, _| {
                    if let Some(state) = state {
                        state.finished = false;
                        state.target = target;
                    }
                });
                self.input.update(cx, |input, cx| {
                    input.focus(window);
                    input.select_all(cx);
                });
            }
            // Still drawn after a commit: the application kept it open, with an error.
            Some(opened) if opened.finished => state.update(cx, |state, _| {
                if let Some(state) = state {
                    state.finished = false;
                }
            }),
            Some(_) => {}
        }

        let control = ControlSize::Small.height(&theme);
        let height = self.over.size.height.max(control);
        let width = (self.over.size.width + px(8.))
            .max(self.min_width)
            .min(self.max_width);
        // Centred on a one-line label; an area opens downward from the label's top.
        let top = if multiline {
            self.over.origin.y
        } else {
            self.over.origin.y + (self.over.size.height - height) / 2.
        };
        let origin = point(self.over.origin.x - px(4.), top);

        let finish = {
            let input = self.input.clone();
            let original = self.original.clone();
            let allow_empty = self.allow_empty;
            let on_commit = self.on_commit.clone();
            let on_close = self.on_close.clone();
            let on_return = self.on_return.clone();
            let state = state.clone();
            Rc::new(move |returned: bool, window: &mut Window, cx: &mut App| {
                finished(&state, &input, window, cx);
                let text = input.read(cx).text().to_owned();
                let leaving = leave(&original, &text, allow_empty);
                if let (true, Some(handler)) = (returned, &on_return) {
                    let text = match leaving {
                        Leave::Commit(text) => Some(text),
                        Leave::Close => None,
                    };
                    return handler(&text, window, cx);
                }
                match leaving {
                    Leave::Commit(text) => {
                        if let Some(handler) = &on_commit {
                            handler(&text, window, cx);
                        }
                    }
                    Leave::Close => {
                        if let Some(handler) = &on_close {
                            handler(&(), window, cx);
                        }
                    }
                }
            })
        };
        let cancel = {
            let on_close = self.on_close.clone();
            let state = state.clone();
            let input = self.input.clone();
            move |window: &mut Window, cx: &mut App| {
                finished(&state, &input, window, cx);
                if let Some(handler) = &on_close {
                    handler(&(), window, cx);
                }
            }
        };
        let error = self.error.clone();
        let field = TextField::new()
            .size(ControlSize::Small)
            .focused(focused)
            .when(multiline, TextField::area)
            .child(self.input.clone());
        // An opaque layer under the well, whose own fill may be translucent, so the label it
        // covers does not show through.
        let field = div()
            .w_full()
            .when(!multiline, |el| el.h(height).flex().items_center())
            .rounded(theme.control_radius(control))
            .bg(theme.colors.elevated)
            .shadow(theme.elevation.medium.clone())
            .child(field);

        let body: AnyElement = div()
            .id(self.id.clone())
            .w(width)
            .flex()
            .flex_col()
            .gap(px(2.))
            .occlude()
            .on_action({
                let finish = finish.clone();
                move |_: &SubmitText, window, cx| finish(true, window, cx)
            })
            .on_action(move |_: &CancelText, window, cx| cancel(window, cx))
            .on_mouse_down_out({
                let finish = finish.clone();
                move |_, window, cx| finish(false, window, cx)
            })
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(field)
            .children(error.map(|error| {
                div()
                    .px(px(6.))
                    .py(px(2.))
                    .rounded(theme.shape.radius_sm)
                    .bg(theme.colors.elevated)
                    .shadow(theme.elevation.medium.clone())
                    .child(
                        Label::new(error)
                            .size(TextSize::XSmall)
                            .color(crate::text::TextColor::Custom(theme.colors.danger)),
                    )
            }))
            .into_any_element();
        deferred(
            anchored()
                .position(origin)
                .snap_to_window_with_margin(px(8.))
                .child(body),
        )
        .with_priority(1)
    }
}

/// What an open field remembers between frames.
struct Opened {
    /// What held focus before the field took it.
    previous: Option<FocusHandle>,
    /// Whether the field was committed or closed and has not been drawn since.
    finished: bool,
    /// What it is editing: the label's text and place.
    target: (SharedString, gpui::Point<Pixels>),
}

/// Mark the field finished and, once a frame has been drawn without it, hand focus back to
/// whatever held it before, unless the person has already moved it elsewhere.
fn finished(
    state: &Entity<Option<Opened>>,
    input: &Entity<TextInput>,
    window: &mut Window,
    cx: &mut App,
) {
    state.update(cx, |state, _| {
        if let Some(state) = state {
            state.finished = true;
        }
    });
    let (state, input) = (state.clone(), input.clone());
    window.on_next_frame(move |window, cx| {
        let Some(opened) = state.read(cx) else {
            return;
        };
        if !opened.finished || !input.read(cx).is_focused(window) {
            return;
        }
        match opened.previous.clone() {
            Some(previous) => previous.focus(window),
            None => window.blur(),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaving_untouched_or_emptied_closes_and_anything_else_commits() {
        assert_eq!(leave("Bass", "Bass", false), Leave::Close);
        assert_eq!(leave("Bass", "  ", false), Leave::Close);
        assert_eq!(leave("Bass", "", true), Leave::Commit("".into()));
        assert_eq!(leave("Bass", "Sub", false), Leave::Commit("Sub".into()));
    }
}
