//! Editable text: a line or a short passage the person types, with a caret, a selection and an
//! input method.
//!
//! [`TextInput`] is the editing itself and draws only the text; put it in a [`crate::TextField`]
//! for the frame. Typing goes through the platform's input handler rather than key events, which
//! is what lets Japanese, Chinese and Korean input methods compose there at all. Offsets inside
//! are byte offsets into the text; the platform speaks UTF-16, and every value crossing that
//! boundary is converted.
//!
//! While an input holds focus, Atelier takes the editing keys (arrows, Home and End, Backspace
//! and Delete, Return, Escape, Space, and ⌘/Ctrl with A, C, X, V and Z) before any key binding
//! sees them, so an application's Space for play or Backspace for delete never fires while the
//! person is typing. A single-letter shortcut of the application's own still would: scope it
//! away with `KeyBinding::new("m", Mute, Some("!AtelierTextInput"))` ([`TEXT_INPUT_CONTEXT`]).

use std::cell::RefCell;
use std::ops::Range;

use gpui::{
    App, Bounds, ClipboardItem, Context, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, EventEmitter, FocusHandle, Focusable, GlobalElementId, Hsla,
    InteractiveElement, IntoElement, Keystroke, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Render, ShapedLine, SharedString,
    Style, Styled, TextRun, UTF16Selection, UnderlineStyle, WeakEntity, Window, actions, div, fill,
    point, px, relative, size,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::theme::ActiveTheme;

actions!(
    atelier,
    [
        /// Return in a focused [`TextInput`]. It bubbles from the input to the elements around
        /// it, which is how [`crate::InlineEdit`] commits; the input also emits
        /// [`TextInputEvent::Submit`].
        SubmitText,
        /// Escape in a focused [`TextInput`], bubbling the same way; the input also emits
        /// [`TextInputEvent::Cancel`].
        CancelText
    ]
);

/// The key context of a focused [`TextInput`]. Scope a single-key shortcut away from typing with
/// `Some("!AtelierTextInput")`.
pub const TEXT_INPUT_CONTEXT: &str = "AtelierTextInput";

/// What a [`TextInput`] tells the view that subscribes to it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextInputEvent {
    /// The text changed, by typing, pasting, an input method or undo.
    Changed,
    /// Return was pressed (Shift-Return in a multi-line input breaks the line instead).
    Submit,
    /// Escape was pressed.
    Cancel,
}

/// The text, its selection and the input method's pre-edit, with every edit a person can make
/// on them and nothing about drawing. [`TextInput`] wraps one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextBuffer {
    text: String,
    /// Byte range of the selection; an empty range is the caret.
    selection: Range<usize>,
    /// Whether the moving end of the selection is its start.
    reversed: bool,
    /// Byte range the input method is still composing.
    marked: Option<Range<usize>>,
    multiline: bool,
    undo: Vec<(String, Range<usize>)>,
    redo: Vec<(String, Range<usize>)>,
    /// Whether the last edit was typing, so a run of typed characters is one undo step.
    typing: bool,
}

impl TextBuffer {
    /// `text` with the caret at its end.
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let end = text.len();
        Self {
            text,
            selection: end..end,
            ..Self::default()
        }
    }

    /// Whether Shift-Return and pasting keep line breaks. A single line turns pasted breaks
    /// into spaces.
    pub fn set_multiline(&mut self, multiline: bool) {
        self.multiline = multiline;
    }

    /// Whether the buffer holds several lines.
    pub fn is_multiline(&self) -> bool {
        self.multiline
    }

    /// The text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The selection as byte offsets.
    pub fn selection(&self) -> Range<usize> {
        self.selection.clone()
    }

    /// The input method's pre-edit, if it is composing.
    pub fn marked(&self) -> Option<Range<usize>> {
        self.marked.clone()
    }

    /// Where the caret is: the moving end of the selection.
    pub fn caret(&self) -> usize {
        if self.reversed {
            self.selection.start
        } else {
            self.selection.end
        }
    }

    /// The selected text.
    pub fn selected_text(&self) -> &str {
        &self.text[self.selection.clone()]
    }

    /// Replace everything, with the caret at the end. Not an undo step.
    pub fn set_text(&mut self, text: impl Into<String>) {
        *self = Self {
            multiline: self.multiline,
            ..Self::new(text)
        };
    }

    /// Select everything, as a rename does so the first key replaces the old name.
    pub fn select_all(&mut self) {
        self.selection = 0..self.text.len();
        self.reversed = false;
    }

    fn snapshot(&mut self, typing: bool) {
        if !(typing && self.typing) {
            self.undo.push((self.text.clone(), self.selection.clone()));
            self.redo.clear();
        }
        self.typing = typing;
    }

    fn clean(&self, text: &str) -> String {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        if self.multiline {
            text
        } else {
            text.replace('\n', " ")
        }
    }

    /// Replace `range` (the pre-edit, or the selection, when `None`) with `text`.
    pub fn replace(&mut self, range: Option<Range<usize>>, text: &str) {
        let text = self.clean(text);
        let range = range
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection.clone());
        let range = clamp(&self.text, range);
        self.snapshot(!text.is_empty() && !text.contains(char::is_whitespace));
        self.text.replace_range(range.clone(), &text);
        let caret = range.start + text.len();
        self.selection = caret..caret;
        self.reversed = false;
        self.marked = None;
    }

    /// Replace `range` with `text` the input method is still composing, selecting
    /// `selected` within it (relative to its start).
    pub fn replace_and_mark(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
    ) {
        let range = range
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection.clone());
        let range = clamp(&self.text, range);
        if self.marked.is_none() {
            self.snapshot(false);
        }
        self.text.replace_range(range.clone(), text);
        self.marked = (!text.is_empty()).then(|| range.start..range.start + text.len());
        self.selection = match selected {
            Some(selected) => {
                let start = floor(&self.text, range.start + selected.start);
                let end = floor(&self.text, range.start + selected.end);
                start..end
            }
            None => {
                let caret = range.start + text.len();
                caret..caret
            }
        };
        self.reversed = false;
    }

    /// End the input method's composition, keeping what it wrote.
    pub fn unmark(&mut self) {
        self.marked = None;
    }

    /// Type `text` over the selection.
    pub fn insert(&mut self, text: &str) {
        self.replace(None, text);
    }

    /// Break the line at the caret, in a multi-line buffer.
    pub fn newline(&mut self) {
        if self.multiline {
            self.snapshot(false);
            self.typing = false;
            let range = self.selection.clone();
            self.text.replace_range(range.clone(), "\n");
            let caret = range.start + 1;
            self.selection = caret..caret;
            self.reversed = false;
        }
    }

    /// Delete the selection, or the character (`word`: the word) before the caret.
    pub fn backspace(&mut self, word: bool) {
        if self.selection.is_empty() {
            let to = if word {
                self.word_start(self.caret())
            } else {
                previous_boundary(&self.text, self.caret())
            };
            self.select_to(to);
        }
        self.delete_selection();
    }

    /// Delete the selection, or the character (`word`: the word) after the caret.
    pub fn delete_forward(&mut self, word: bool) {
        if self.selection.is_empty() {
            let to = if word {
                self.word_end(self.caret())
            } else {
                next_boundary(&self.text, self.caret())
            };
            self.select_to(to);
        }
        self.delete_selection();
    }

    fn delete_selection(&mut self) {
        if !self.selection.is_empty() {
            self.snapshot(false);
            self.typing = false;
            let range = self.selection.clone();
            self.text.replace_range(range.clone(), "");
            self.selection = range.start..range.start;
            self.reversed = false;
        }
    }

    /// Put the caret at `offset`, or with `extend` move the selection's moving end there.
    pub fn move_to(&mut self, offset: usize, extend: bool) {
        let offset = floor(&self.text, offset);
        if extend {
            self.select_to(offset);
        } else {
            self.selection = offset..offset;
            self.reversed = false;
        }
        self.typing = false;
    }

    fn select_to(&mut self, offset: usize) {
        if self.reversed {
            self.selection.start = offset;
        } else {
            self.selection.end = offset;
        }
        if self.selection.end < self.selection.start {
            self.reversed = !self.reversed;
            self.selection = self.selection.end..self.selection.start;
        }
    }

    /// Select from `from` to `to`, the caret at `to`.
    pub fn select_range(&mut self, from: usize, to: usize) {
        let (from, to) = (floor(&self.text, from), floor(&self.text, to));
        self.reversed = to < from;
        self.selection = from.min(to)..from.max(to);
    }

    /// One character left; with a selection and no `extend`, to its start.
    pub fn left(&mut self, word: bool, extend: bool) {
        if !extend && !self.selection.is_empty() {
            return self.move_to(self.selection.start, false);
        }
        let to = if word {
            self.word_start(self.caret())
        } else {
            previous_boundary(&self.text, self.caret())
        };
        self.move_to(to, extend);
    }

    /// One character right; with a selection and no `extend`, to its end.
    pub fn right(&mut self, word: bool, extend: bool) {
        if !extend && !self.selection.is_empty() {
            return self.move_to(self.selection.end, false);
        }
        let to = if word {
            self.word_end(self.caret())
        } else {
            next_boundary(&self.text, self.caret())
        };
        self.move_to(to, extend);
    }

    /// The start of the line the caret is on.
    pub fn line_start(&self, offset: usize) -> usize {
        self.text[..offset].rfind('\n').map_or(0, |at| at + 1)
    }

    /// The end of the line the caret is on.
    pub fn line_end(&self, offset: usize) -> usize {
        self.text[offset..]
            .find('\n')
            .map_or(self.text.len(), |at| offset + at)
    }

    /// Up a line, keeping the column in characters; on the first line, to the start.
    pub fn up(&mut self, extend: bool) {
        let caret = self.caret();
        let start = self.line_start(caret);
        if start == 0 {
            return self.move_to(0, extend);
        }
        let column = self.text[start..caret].chars().count();
        let above = self.line_start(start - 1);
        self.move_to(column_offset(&self.text, above, start - 1, column), extend);
    }

    /// Down a line, keeping the column; on the last line, to the end.
    pub fn down(&mut self, extend: bool) {
        let caret = self.caret();
        let end = self.line_end(caret);
        if end == self.text.len() {
            return self.move_to(end, extend);
        }
        let column = self.text[self.line_start(caret)..caret].chars().count();
        let below = end + 1;
        let below_end = self.line_end(below);
        self.move_to(column_offset(&self.text, below, below_end, column), extend);
    }

    fn word_start(&self, offset: usize) -> usize {
        self.text
            .split_word_bound_indices()
            .rev()
            .find(|(at, word)| *at < offset && !word.trim().is_empty())
            .map(|(at, _)| at)
            .unwrap_or(0)
    }

    fn word_end(&self, offset: usize) -> usize {
        self.text
            .split_word_bound_indices()
            .map(|(at, word)| (at + word.len(), word))
            .find(|(end, word)| *end > offset && !word.trim().is_empty())
            .map_or(self.text.len(), |(end, _)| end)
    }

    /// The word, or run of spaces or punctuation, around `offset`.
    pub fn word_at(&self, offset: usize) -> Range<usize> {
        self.text
            .split_word_bound_indices()
            .map(|(at, word)| at..at + word.len())
            .find(|word| word.contains(&offset))
            .unwrap_or(offset..offset)
    }

    /// Take back the last edit. Returns whether there was one.
    pub fn undo(&mut self) -> bool {
        let Some((text, selection)) = self.undo.pop() else {
            return false;
        };
        self.redo.push((
            std::mem::replace(&mut self.text, text),
            self.selection.clone(),
        ));
        self.selection = clamp(&self.text, selection);
        self.reversed = false;
        self.marked = None;
        self.typing = false;
        true
    }

    /// Put back the last edit undone. Returns whether there was one.
    pub fn redo(&mut self) -> bool {
        let Some((text, selection)) = self.redo.pop() else {
            return false;
        };
        self.undo.push((
            std::mem::replace(&mut self.text, text),
            self.selection.clone(),
        ));
        self.selection = clamp(&self.text, selection);
        self.reversed = false;
        self.marked = None;
        self.typing = false;
        true
    }

    /// `offset` in UTF-16 code units, as the platform counts.
    pub fn to_utf16(&self, offset: usize) -> usize {
        self.text[..offset.min(self.text.len())]
            .chars()
            .map(char::len_utf16)
            .sum()
    }

    /// A UTF-16 offset from the platform, as a byte offset.
    pub fn from_utf16(&self, offset: usize) -> usize {
        let mut units = 0;
        for (at, ch) in self.text.char_indices() {
            if units >= offset {
                return at;
            }
            units += ch.len_utf16();
        }
        self.text.len()
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.to_utf16(range.start)..self.to_utf16(range.end)
    }

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.from_utf16(range.start)..self.from_utf16(range.end)
    }

    /// The line breaks' positions split the text into lines: each line's byte range.
    pub fn lines(&self) -> Vec<Range<usize>> {
        let mut lines = Vec::new();
        let mut start = 0;
        for (at, _) in self.text.match_indices('\n') {
            lines.push(start..at);
            start = at + 1;
        }
        lines.push(start..self.text.len());
        lines
    }
}

fn floor(text: &str, mut offset: usize) -> usize {
    offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

fn clamp(text: &str, range: Range<usize>) -> Range<usize> {
    let start = floor(text, range.start);
    start..floor(text, range.end).max(start)
}

fn previous_boundary(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true)
        .rev()
        .find_map(|(at, _)| (at < offset).then_some(at))
        .unwrap_or(0)
}

fn next_boundary(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true)
        .find_map(|(at, _)| (at > offset).then_some(at))
        .unwrap_or(text.len())
}

/// The offset `column` characters into the line `start..end`, or its end.
fn column_offset(text: &str, start: usize, end: usize, column: usize) -> usize {
    text[start..end]
        .char_indices()
        .nth(column)
        .map_or(end, |(at, _)| start + at)
}

thread_local! {
    /// Every live input, so the keystroke interceptor can find the focused one.
    static INPUTS: RefCell<Vec<WeakEntity<TextInput>>> = const { RefCell::new(Vec::new()) };
}

/// An editable line, or a few lines, of text: the caret, the selection, the input method,
/// clipboard and undo. It draws only the text, so put it inside a [`crate::TextField`]:
///
/// ```ignore
/// let name = cx.new(|cx| TextInput::new(cx).with_text("Untitled", cx));
/// cx.subscribe(&name, |this, input, event, cx| match event {
///     TextInputEvent::Submit => this.rename(input.read(cx).text().to_owned(), cx),
///     _ => {}
/// })
/// .detach();
/// // In render:
/// TextField::new()
///     .placeholder("Name")
///     .empty(self.name.read(cx).is_empty())
///     .focused(self.name.read(cx).is_focused(window))
///     .child(self.name.clone())
/// ```
///
/// Return emits [`TextInputEvent::Submit`] and Escape [`TextInputEvent::Cancel`], and both also
/// bubble as the [`SubmitText`] and [`CancelText`] actions. A [`TextInput::multiline`] input
/// grows a line for each break, and Shift-Return breaks the line. Double-click selects a word,
/// triple-click everything.
pub struct TextInput {
    focus: FocusHandle,
    buffer: TextBuffer,
    /// The lines as drawn last frame, each with its byte range.
    layout: Vec<(Range<usize>, ShapedLine)>,
    bounds: Option<Bounds<Pixels>>,
    line_height: Pixels,
    /// How far a long single line has scrolled to keep the caret in view.
    scroll: Pixels,
    selecting: bool,
    /// Where a drag began, for a word or line selection to grow from.
    anchor: Option<usize>,
}

impl EventEmitter<TextInputEvent> for TextInput {}

impl TextInput {
    /// An empty single-line input.
    pub fn new(cx: &mut Context<Self>) -> Self {
        let weak = cx.entity().downgrade();
        INPUTS.with(|inputs| {
            let mut inputs = inputs.borrow_mut();
            inputs.retain(|input| input.upgrade().is_some());
            inputs.push(weak);
        });
        Self {
            focus: cx.focus_handle(),
            buffer: TextBuffer::default(),
            layout: Vec::new(),
            bounds: None,
            line_height: px(18.),
            scroll: px(0.),
            selecting: false,
            anchor: None,
        }
    }

    /// Start with `text`, the caret at its end.
    pub fn with_text(mut self, text: impl Into<String>, _cx: &mut Context<Self>) -> Self {
        let multiline = self.buffer.is_multiline();
        self.buffer = TextBuffer::new(text);
        self.buffer.set_multiline(multiline);
        self
    }

    /// Hold several lines: each break is a line of its own and Shift-Return breaks the line.
    pub fn multiline(mut self) -> Self {
        self.buffer.set_multiline(true);
        self
    }

    /// The text.
    pub fn text(&self) -> &str {
        self.buffer.text()
    }

    /// Whether there is no text.
    pub fn is_empty(&self) -> bool {
        self.buffer.text().is_empty()
    }

    /// The editing state, for reading the selection or the pre-edit.
    pub fn buffer(&self) -> &TextBuffer {
        &self.buffer
    }

    /// Replace the text, the caret at its end. Not an undo step; emits no event.
    pub fn set_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.buffer.set_text(text);
        self.scroll = px(0.);
        cx.notify();
    }

    /// Select all the text.
    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        self.buffer.select_all();
        cx.notify();
    }

    /// Whether the input holds keyboard focus.
    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus.is_focused(window)
    }

    /// Give the input keyboard focus.
    pub fn focus(&self, window: &mut Window) {
        self.focus.focus(window);
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(TextInputEvent::Changed);
        cx.notify();
    }

    /// The byte offset under `position`, in window coordinates.
    fn offset_at(&self, position: Point<Pixels>) -> usize {
        let Some(bounds) = self.bounds else {
            return self.buffer.text().len();
        };
        if self.layout.is_empty() {
            return 0;
        }
        let row = ((position.y - bounds.top()) / self.line_height).floor();
        let row = (row.max(0.) as usize).min(self.layout.len() - 1);
        let (range, line) = &self.layout[row];
        let x = position.x - bounds.left() + self.scroll;
        range.start + line.closest_index_for_x(x).min(range.len())
    }

    /// The caret's rectangle relative to the text's origin, and the line it is on.
    fn position_of(&self, offset: usize) -> Option<Point<Pixels>> {
        self.layout
            .iter()
            .enumerate()
            .find(|(_, (range, _))| range.start <= offset && offset <= range.end)
            .map(|(row, (range, line))| {
                point(
                    line.x_for_index(offset - range.start),
                    self.line_height * row as f32,
                )
            })
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus.focus(window);
        let offset = self.offset_at(event.position);
        match event.click_count {
            2 => {
                let word = self.buffer.word_at(offset);
                self.buffer.select_range(word.start, word.end);
                self.anchor = Some(word.start);
            }
            count if count >= 3 => {
                self.buffer.select_all();
                self.anchor = None;
            }
            _ => {
                self.buffer.move_to(offset, event.modifiers.shift);
                self.anchor = None;
            }
        }
        self.selecting = true;
        cx.stop_propagation();
        cx.notify();
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.selecting && event.pressed_button == Some(MouseButton::Left) {
            let offset = self.offset_at(event.position);
            match self.anchor {
                Some(anchor) => self.buffer.select_range(anchor, offset),
                None => self.buffer.move_to(offset, true),
            }
            cx.notify();
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
    }

    /// Answer a key the interceptor took for this input.
    fn key(&mut self, keystroke: &Keystroke, window: &mut Window, cx: &mut Context<Self>) -> Key {
        let modifiers = &keystroke.modifiers;
        let secondary = modifiers.secondary();
        // ⌥ moves by words on a Mac, Ctrl elsewhere; ⌘ to the line's ends on a Mac.
        let word = if cfg!(target_os = "macos") {
            modifiers.alt
        } else {
            modifiers.control
        };
        let line = cfg!(target_os = "macos") && modifiers.platform;
        let shift = modifiers.shift;
        let before = self.buffer.text().to_owned();
        match keystroke.key.as_str() {
            "enter" if shift && self.buffer.is_multiline() => self.buffer.newline(),
            "enter" => return Key::Submit,
            "escape" => return Key::Cancel,
            "space" if !secondary => self.buffer.insert(" "),
            "backspace" if line => {
                let caret = self.buffer.caret();
                self.buffer.move_to(self.buffer.line_start(caret), true);
                self.buffer.backspace(false);
            }
            "backspace" => self.buffer.backspace(word),
            "delete" => self.buffer.delete_forward(word),
            "left" if line => {
                let to = self.buffer.line_start(self.buffer.caret());
                self.buffer.move_to(to, shift);
            }
            "right" if line => {
                let to = self.buffer.line_end(self.buffer.caret());
                self.buffer.move_to(to, shift);
            }
            "left" => self.buffer.left(word, shift),
            "right" => self.buffer.right(word, shift),
            "up" if line => self.buffer.move_to(0, shift),
            "down" if line => self.buffer.move_to(self.buffer.text().len(), shift),
            "up" => self.buffer.up(shift),
            "down" => self.buffer.down(shift),
            "home" if secondary => self.buffer.move_to(0, shift),
            "end" if secondary => self.buffer.move_to(self.buffer.text().len(), shift),
            "home" => {
                let to = self.buffer.line_start(self.buffer.caret());
                self.buffer.move_to(to, shift);
            }
            "end" => {
                let to = self.buffer.line_end(self.buffer.caret());
                self.buffer.move_to(to, shift);
            }
            "a" if secondary => self.buffer.select_all(),
            "c" if secondary => {
                if !self.buffer.selection().is_empty() {
                    cx.write_to_clipboard(ClipboardItem::new_string(
                        self.buffer.selected_text().to_owned(),
                    ));
                }
            }
            "x" if secondary => {
                if !self.buffer.selection().is_empty() {
                    cx.write_to_clipboard(ClipboardItem::new_string(
                        self.buffer.selected_text().to_owned(),
                    ));
                    self.buffer.backspace(false);
                }
            }
            "v" if secondary => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    self.buffer.insert(&text);
                }
            }
            "z" if secondary && shift => {
                self.buffer.redo();
            }
            "z" if secondary => {
                self.buffer.undo();
            }
            "y" if secondary && !cfg!(target_os = "macos") => {
                self.buffer.redo();
            }
            _ => return Key::Ignored,
        }
        let _ = window;
        if self.buffer.text() != before {
            self.changed(cx);
        } else {
            cx.notify();
        }
        Key::Handled
    }
}

/// What the interceptor does with a key after the focused input answered it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Key {
    Ignored,
    Handled,
    Submit,
    Cancel,
}

/// The focused input in `window`, if any.
fn focused_input(window: &Window, cx: &App) -> Option<Entity<TextInput>> {
    INPUTS.with(|inputs| {
        inputs
            .borrow()
            .iter()
            .filter_map(WeakEntity::upgrade)
            .find(|input| input.read(cx).focus.is_focused(window))
    })
}

/// Take the editing keys of a focused input before any binding; called from the focus
/// interceptor. Returns whether the key was taken.
pub(crate) fn intercept(keystroke: &Keystroke, window: &mut Window, cx: &mut App) -> bool {
    let Some(input) = focused_input(window, cx) else {
        return false;
    };
    // While an input method composes, every key is its own.
    if input.read(cx).buffer.marked().is_some() {
        return false;
    }
    let editing = matches!(
        keystroke.key.as_str(),
        "enter"
            | "escape"
            | "space"
            | "backspace"
            | "delete"
            | "left"
            | "right"
            | "up"
            | "down"
            | "home"
            | "end"
    ) || keystroke.modifiers.secondary()
        && !keystroke.modifiers.alt
        && matches!(keystroke.key.as_str(), "a" | "c" | "x" | "v" | "z" | "y");
    if !editing || keystroke.modifiers.function {
        return false;
    }
    let answer = input.update(cx, |input, cx| input.key(keystroke, window, cx));
    match answer {
        Key::Ignored => return false,
        Key::Handled => {}
        Key::Submit => {
            input.update(cx, |_, cx| cx.emit(TextInputEvent::Submit));
            window.dispatch_action(Box::new(SubmitText), cx);
        }
        Key::Cancel => {
            input.update(cx, |_, cx| cx.emit(TextInputEvent::Cancel));
            window.dispatch_action(Box::new(CancelText), cx);
        }
    }
    true
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.buffer.range_from_utf16(&range);
        actual.replace(self.buffer.range_to_utf16(&range));
        Some(self.buffer.text()[range].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.buffer.range_to_utf16(&self.buffer.selection()),
            reversed: self.buffer.reversed,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.buffer
            .marked()
            .map(|range| self.buffer.range_to_utf16(&range))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.buffer.unmark();
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range.map(|range| self.buffer.range_from_utf16(&range));
        self.buffer.replace(range, text);
        self.changed(cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range.map(|range| self.buffer.range_from_utf16(&range));
        // The selection inside the pre-edit is counted in UTF-16 from the pre-edit's start.
        let selected = selected.map(|selected| {
            let units = |count: usize| {
                text.chars()
                    .scan(0, |seen, ch| {
                        let at = *seen;
                        *seen += ch.len_utf16();
                        Some((at, ch.len_utf8()))
                    })
                    .take_while(|(at, _)| *at < count)
                    .map(|(_, len)| len)
                    .sum::<usize>()
            };
            units(selected.start)..units(selected.end)
        });
        self.buffer.replace_and_mark(range, text, selected);
        self.changed(cx);
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.buffer.range_from_utf16(&range);
        let start = self.position_of(range.start)?;
        let end = self.position_of(range.end).unwrap_or(start);
        let origin = bounds.origin - point(self.scroll, px(0.));
        Some(Bounds::from_corners(
            origin + start,
            origin + point(end.x.max(start.x), start.y + self.line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let offset = self.offset_at(point);
        Some(self.buffer.to_utf16(offset))
    }
}

impl Render for TextInput {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .id(ElementId::NamedInteger(
                "atelier-text-input".into(),
                cx.entity_id().as_u64(),
            ))
            .key_context(TEXT_INPUT_CONTEXT)
            .track_focus(&self.focus)
            .flex_1()
            .min_w_0()
            .cursor_text()
            .text_color(theme.colors.text)
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .child(TextElement {
                input: cx.entity(),
                selection: theme.colors.selection,
                caret: theme.colors.accent,
            })
    }
}

/// The text of a [`TextInput`], with its selection, pre-edit and caret.
struct TextElement {
    input: Entity<TextInput>,
    selection: Hsla,
    caret: Hsla,
}

struct Prepaint {
    lines: Vec<(Range<usize>, ShapedLine)>,
    line_height: Pixels,
    scroll: Pixels,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl gpui::Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let rows = self.input.read(cx).buffer.lines().len().max(1);
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = (window.line_height() * rows as f32).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Prepaint {
        let input = self.input.read(cx);
        let buffer = &input.buffer;
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let marked = buffer.marked();
        let lines: Vec<_> = buffer
            .lines()
            .into_iter()
            .map(|range| {
                let text: SharedString = buffer.text()[range.clone()].to_owned().into();
                let run = |len: usize, underline: bool| TextRun {
                    len,
                    font: style.font(),
                    color: style.color,
                    background_color: None,
                    underline: underline.then_some(UnderlineStyle {
                        color: Some(style.color),
                        thickness: px(1.),
                        wavy: false,
                    }),
                    strikethrough: None,
                };
                let runs: Vec<TextRun> = match &marked {
                    Some(marked) if marked.start < range.end && range.start < marked.end => {
                        let from = marked.start.max(range.start) - range.start;
                        let to = marked.end.min(range.end) - range.start;
                        [
                            run(from, false),
                            run(to - from, true),
                            run(range.len() - to, false),
                        ]
                        .into_iter()
                        .filter(|run| run.len > 0)
                        .collect()
                    }
                    _ => vec![run(range.len(), false)],
                };
                let line = window
                    .text_system()
                    .shape_line(text, font_size, &runs, None);
                (range, line)
            })
            .collect();
        // Keep the caret in view along a line longer than the field.
        let caret = buffer.caret();
        let mut scroll = input.scroll;
        if let Some((range, line)) = lines
            .iter()
            .find(|(range, _)| range.start <= caret && caret <= range.end)
        {
            let x = line.x_for_index(caret - range.start);
            let width = bounds.size.width - px(2.);
            if x - scroll > width {
                scroll = x - width;
            }
            if x < scroll {
                scroll = x;
            }
            let widest = lines
                .iter()
                .map(|(_, line)| line.width)
                .fold(px(0.), Pixels::max);
            scroll = scroll.min((widest - width).max(px(0.))).max(px(0.));
        }
        Prepaint {
            lines,
            line_height,
            scroll,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        prepaint: &mut Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.input.read(cx).focus.clone();
        let focused = focus.is_focused(window);
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        let (selection, caret) = {
            let buffer = &self.input.read(cx).buffer;
            (buffer.selection(), buffer.caret())
        };
        let line_height = prepaint.line_height;
        let origin = bounds.origin - point(prepaint.scroll, px(0.));
        let selection_color = self.selection;
        let caret_color = self.caret;
        let lines = std::mem::take(&mut prepaint.lines);
        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
            for (row, (range, line)) in lines.iter().enumerate() {
                let top = origin.y + line_height * row as f32;
                if focused && !selection.is_empty() {
                    let from = selection.start.max(range.start);
                    let to = selection.end.min(range.end);
                    // A selection running past a line's end shows a sliver for the break.
                    let past = selection.end > range.end && selection.start <= range.end;
                    if from <= to && (from < to || past) {
                        let left = line.x_for_index(from - range.start);
                        let right =
                            line.x_for_index(to - range.start) + if past { px(4.) } else { px(0.) };
                        window.paint_quad(fill(
                            Bounds::from_corners(
                                point(origin.x + left, top),
                                point(origin.x + right, top + line_height),
                            ),
                            selection_color,
                        ));
                    }
                }
                line.paint(point(origin.x, top), line_height, window, cx)
                    .ok();
                if focused && selection.is_empty() && range.start <= caret && caret <= range.end {
                    let x = line.x_for_index(caret - range.start);
                    window.paint_quad(fill(
                        Bounds::new(
                            point(origin.x + x, top + px(1.)),
                            size(px(1.5), line_height - px(2.)),
                        ),
                        caret_color,
                    ));
                }
            }
        });
        let scroll = prepaint.scroll;
        self.input.update(cx, |input, _| {
            input.layout = lines;
            input.bounds = Some(bounds);
            input.line_height = line_height;
            input.scroll = scroll;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_replaces_a_selection_and_undoes_as_one_step() {
        let mut buffer = TextBuffer::new("Lead");
        buffer.select_all();
        for ch in ["B", "a", "s", "s"] {
            buffer.insert(ch);
        }
        assert_eq!(buffer.text(), "Bass");
        assert!(buffer.undo());
        assert_eq!(buffer.text(), "Lead");
        assert!(buffer.redo());
        assert_eq!(buffer.text(), "Bass");
    }

    #[test]
    fn a_single_line_turns_pasted_breaks_into_spaces() {
        let mut buffer = TextBuffer::new("");
        buffer.insert("one\r\ntwo");
        assert_eq!(buffer.text(), "one two");
        let mut area = TextBuffer::new("");
        area.set_multiline(true);
        area.insert("one\r\ntwo");
        assert_eq!(area.text(), "one\ntwo");
        assert_eq!(area.lines(), vec![0..3, 4..7]);
    }

    #[test]
    fn backspace_takes_a_whole_grapheme_and_words_by_word() {
        let mut buffer = TextBuffer::new("café 👍🏽");
        buffer.backspace(false);
        assert_eq!(buffer.text(), "café ");
        buffer.backspace(true);
        assert_eq!(buffer.text(), "");
    }

    #[test]
    fn utf16_offsets_round_trip_through_japanese() {
        let buffer = TextBuffer::new("歌詞a");
        assert_eq!(buffer.to_utf16(6), 2);
        assert_eq!(buffer.from_utf16(2), 6);
        assert_eq!(buffer.from_utf16(3), 7);
    }

    #[test]
    fn composition_marks_then_commits() {
        let mut buffer = TextBuffer::new("");
        buffer.replace_and_mark(None, "か", None);
        assert_eq!(buffer.marked(), Some(0..3));
        buffer.replace_and_mark(None, "かし", None);
        buffer.replace(None, "歌詞");
        assert_eq!(buffer.text(), "歌詞");
        assert_eq!(buffer.marked(), None);
        assert_eq!(buffer.caret(), 6);
    }

    #[test]
    fn up_and_down_keep_the_column() {
        let mut buffer = TextBuffer::new("first\nno\nthird");
        buffer.set_multiline(true);
        buffer.move_to(4, false);
        buffer.down(false);
        assert_eq!(buffer.caret(), 8, "the short line's end");
        buffer.down(false);
        assert_eq!(buffer.caret(), 11);
        buffer.up(true);
        assert_eq!(buffer.selection(), 8..11);
    }

    #[test]
    fn shift_extends_and_a_plain_arrow_collapses_to_the_side() {
        let mut buffer = TextBuffer::new("chord");
        buffer.move_to(1, false);
        buffer.right(false, true);
        buffer.right(false, true);
        assert_eq!(buffer.selection(), 1..3);
        buffer.left(false, false);
        assert_eq!(buffer.caret(), 1);
        buffer.left(false, true);
        assert_eq!(buffer.selection(), 0..1);
    }
}
