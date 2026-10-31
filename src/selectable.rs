//! Read-only text the person can select and copy: a message, an attached file, a log.

use std::ops::Range;

use gpui::{
    App, ClipboardItem, ElementId, FocusHandle, HighlightStyle, Hsla, InteractiveElement,
    IntoElement, KeyBinding, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement, Pixels, Point, RenderOnce, SharedString, Styled, StyledText, TextLayout, Window,
    actions, canvas, div, prelude::FluentBuilder,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::text::{TextColor, TextSize};
use crate::theme::ActiveTheme;

actions!(
    atelier,
    [
        /// Copy the selected text of a focused [`SelectableText`].
        CopySelection,
        /// Select all of a focused [`SelectableText`].
        SelectAll
    ]
);

/// The key context of a focused [`SelectableText`], where ⌘C / Ctrl+C copies and ⌘A / Ctrl+A
/// selects all.
pub const SELECTABLE_CONTEXT: &str = "AtelierSelectableText";

pub(crate) fn bindings() -> Vec<KeyBinding> {
    let context = Some(SELECTABLE_CONTEXT);
    vec![
        KeyBinding::new("cmd-c", CopySelection, context),
        KeyBinding::new("ctrl-c", CopySelection, context),
        KeyBinding::new("cmd-a", SelectAll, context),
        KeyBinding::new("ctrl-a", SelectAll, context),
    ]
}

/// What one [`SelectableText`] remembers between frames.
struct Selection {
    focus: FocusHandle,
    text: SharedString,
    /// Where the selection started and where it ends now, as byte offsets.
    anchor: usize,
    head: usize,
    /// Whether the pointer is dragging the selection out.
    dragging: bool,
    /// The text's layout in the last frame, to turn the pointer into an offset.
    layout: Option<TextLayout>,
}

impl Selection {
    fn range(&self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }

    fn index_at(&self, position: Point<Pixels>) -> Option<usize> {
        let layout = self.layout.as_ref()?;
        let index = layout
            .index_for_position(position)
            .unwrap_or_else(|index| index);
        Some(floor_boundary(&self.text, index))
    }

    /// The word, or run of spaces or punctuation, around `index`.
    fn word_at(&self, index: usize) -> Range<usize> {
        self.text
            .split_word_bound_indices()
            .map(|(start, word)| start..start + word.len())
            .find(|word| word.contains(&index))
            .unwrap_or(index..index)
    }

    /// The line around `index`.
    fn line_at(&self, index: usize) -> Range<usize> {
        let start = self.text[..index].rfind('\n').map_or(0, |at| at + 1);
        let end = self.text[index..]
            .find('\n')
            .map_or(self.text.len(), |at| index + at);
        start..end
    }
}

fn floor_boundary(text: &str, mut index: usize) -> usize {
    index = index.min(text.len());
    while !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

/// Text the person can select with the pointer and copy, character for character.
///
/// Drag to select, Shift-click to extend, double-click for a word and triple-click for a line;
/// ⌘C or Ctrl+C copies, ⌘A or Ctrl+A selects all. Line breaks, indentation and symbols such as
/// `<` or `**` are drawn and copied exactly as given, with nothing parsed. It is no tab stop:
/// reading is not an action. The selection shows in [`crate::Colors::selection`] while the
/// text holds focus, and goes when focus moves on.
///
/// The id keeps the selection from frame to frame, so give each piece of text its own.
#[derive(IntoElement)]
pub struct SelectableText {
    id: ElementId,
    text: SharedString,
    size: TextSize,
    color: TextColor,
    mono: bool,
    selection: Option<Hsla>,
}

impl SelectableText {
    /// Body text in the default colour.
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            size: TextSize::Medium,
            color: TextColor::Default,
            mono: false,
            selection: None,
        }
    }

    /// Step on the type scale.
    pub fn size(mut self, size: TextSize) -> Self {
        self.size = size;
        self
    }

    /// Semantic colour.
    pub fn color(mut self, color: TextColor) -> Self {
        self.color = color;
        self
    }

    /// Set in the monospaced font, for code, logs and attached text files.
    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }

    /// Draw the selection for text on a coloured fill such as the person's message bubble,
    /// where the accent would not show: a wash of the text's own colour.
    pub(crate) fn on_fill(mut self, text: Hsla) -> Self {
        self.color = TextColor::Custom(text);
        self.selection = Some(text.opacity(0.3));
        self
    }
}

impl RenderOnce for SelectableText {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let color = self.color.resolve(theme);
        let highlight = self.selection.unwrap_or(theme.colors.selection);
        let size = self.size.pixels(theme);
        let mono = self.mono.then(|| theme.typography.mono.clone());

        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| Selection {
            focus: cx.focus_handle(),
            text: SharedString::default(),
            anchor: 0,
            head: 0,
            dragging: false,
            layout: None,
        });
        let text = self.text;
        let styled = state.update(cx, |selection, _| {
            if selection.text != text {
                selection.anchor = floor_boundary(&text, selection.anchor);
                selection.head = floor_boundary(&text, selection.head);
                selection.text = text.clone();
            }
            let range = selection.range();
            let shown = selection.focus.is_focused(window) && !range.is_empty();
            let styled = StyledText::new(text.clone()).with_highlights(shown.then(|| {
                (
                    range,
                    HighlightStyle {
                        background_color: Some(highlight),
                        ..Default::default()
                    },
                )
            }));
            selection.layout = Some(styled.layout().clone());
            styled
        });
        let (focus, dragging) = {
            let selection = state.read(cx);
            (selection.focus.clone(), selection.dragging)
        };

        let mut element = crate::focus::selector(div().id(self.id.clone()), &self.id)
            .track_focus(&focus)
            .key_context(SELECTABLE_CONTEXT)
            .relative()
            .cursor_text()
            .text_size(size)
            .text_color(color);
        if let Some(mono) = mono {
            element = element.font(mono);
        }
        element
            .on_mouse_down(MouseButton::Left, {
                let state = state.clone();
                move |event: &MouseDownEvent, window, cx| {
                    let focus = state.read(cx).focus.clone();
                    window.focus(&focus, cx);
                    state.update(cx, |selection, cx| {
                        let Some(index) = selection.index_at(event.position) else {
                            return;
                        };
                        let range = match event.click_count {
                            2 => selection.word_at(index),
                            3.. => selection.line_at(index),
                            _ if event.modifiers.shift => selection.anchor..index,
                            _ => index..index,
                        };
                        (selection.anchor, selection.head) = (range.start, range.end);
                        selection.dragging = event.click_count < 2;
                        cx.notify();
                    });
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &CopySelection, _, cx| {
                    let selection = state.read(cx);
                    let range = selection.range();
                    if range.is_empty() {
                        cx.propagate();
                        return;
                    }
                    let copied = selection.text[range].to_owned();
                    cx.write_to_clipboard(ClipboardItem::new_string(copied));
                }
            })
            .on_action({
                let state = state.clone();
                move |_: &SelectAll, _, cx| {
                    state.update(cx, |selection, cx| {
                        (selection.anchor, selection.head) = (0, selection.text.len());
                        cx.notify();
                    });
                }
            })
            .child(styled)
            // While a drag is under way, follow the pointer anywhere in the window, so the
            // selection keeps growing past the text's edge.
            .when_some(dragging.then_some(state), |element, state| {
                element.child(
                    canvas(
                        |_, _, _| {},
                        move |_, _, window, _| {
                            window.on_mouse_event({
                                let state = state.clone();
                                move |event: &MouseMoveEvent, phase, _, cx| {
                                    if !phase.bubble() || event.pressed_button.is_none() {
                                        return;
                                    }
                                    state.update(cx, |selection, cx| {
                                        if let Some(index) = selection.index_at(event.position)
                                            && index != selection.head
                                        {
                                            selection.head = index;
                                            cx.notify();
                                        }
                                    });
                                }
                            });
                            window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                                if phase.bubble() {
                                    state.update(cx, |selection, cx| {
                                        selection.dragging = false;
                                        cx.notify();
                                    });
                                }
                            });
                        },
                    )
                    .absolute(),
                )
            })
    }
}
