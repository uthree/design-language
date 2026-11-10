//! Read-only text the person can select and copy: a message, an attached file, a log — one
//! piece at a time, or many at once inside a [`SelectionScope`].

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, ClipboardItem, Context, Div, Element, ElementId, Entity, FocusHandle,
    GlobalElementId, HighlightStyle, Hsla, InspectorElementId, InteractiveElement, IntoElement,
    KeyBinding, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement,
    Pixels, Point, RenderOnce, ScrollHandle, SharedString, Stateful, StyleRefinement, Styled,
    StyledText, TextLayout, Window, actions, canvas, div, point, px,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::text::{TextColor, TextSize};
use crate::theme::ActiveTheme;

actions!(
    atelier,
    [
        /// Copy the selected text of a focused [`SelectableText`] or [`SelectionScope`].
        CopySelection,
        /// Select all of a focused [`SelectableText`] or [`SelectionScope`].
        SelectAll
    ]
);

/// The key context of a focused [`SelectableText`] or [`SelectionScope`], where ⌘C / Ctrl+C
/// copies and ⌘A / Ctrl+A selects all.
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

/// The separator a piece of text puts before itself when a selection that runs into it from
/// an earlier piece is copied: a blank line, as between paragraphs.
pub const BLOCK_SEPARATOR: &str = "\n\n";

/// The separator for consecutive lines of one block: code, a log, a list.
pub const LINE_SEPARATOR: &str = "\n";

thread_local! {
    /// The scopes being laid out, innermost last. A [`SelectableText`] joins the top one.
    static SCOPES: RefCell<Vec<Entity<Scope>>> = const { RefCell::new(Vec::new()) };
}

fn current_scope() -> Option<Entity<Scope>> {
    SCOPES.with(|scopes| scopes.borrow().last().cloned())
}

/// A point in the text of a scope: a piece and a byte offset into it.
#[derive(Clone, Debug, PartialEq)]
struct Caret {
    segment: ElementId,
    offset: usize,
}

/// One piece of selectable text, as it was laid out in the last frame.
struct Segment {
    id: ElementId,
    text: SharedString,
    layout: TextLayout,
    separator: SharedString,
    links: Rc<Vec<(Range<usize>, SharedString)>>,
    on_link: Option<LinkHandler>,
    /// Set once the text has its bounds, so the pointer can be measured against it.
    placed: Rc<Cell<bool>>,
}

impl Segment {
    fn bounds(&self) -> Option<Bounds<Pixels>> {
        self.placed.get().then(|| self.layout.bounds())
    }

    fn index_at(&self, position: Point<Pixels>) -> usize {
        let index = self
            .layout
            .index_for_position(position)
            .unwrap_or_else(|index| index);
        floor_boundary(&self.text, index)
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

/// What a selection remembers between frames: the pieces of text it spans, in reading order,
/// and where it starts and ends among them.
pub(crate) struct Scope {
    focus: FocusHandle,
    anchor: Option<Caret>,
    head: Option<Caret>,
    /// Whether the pointer is dragging the selection out.
    dragging: bool,
    /// The link under the pointer when the button went down, followed if it comes up there
    /// without a drag.
    pressed_link: Option<(SharedString, Option<LinkHandler>)>,
    /// The pieces of the last complete frame, in reading order.
    segments: Vec<Segment>,
    order: HashMap<ElementId, usize>,
    /// The pieces of the frame being laid out.
    building: Vec<Segment>,
    /// The scrolled area the text sits in, which a drag past its edge scrolls.
    scroll: Option<ScrollHandle>,
    /// Where the pointer was last seen during a drag.
    pointer: Option<Point<Pixels>>,
}

impl Scope {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            anchor: None,
            head: None,
            dragging: false,
            pressed_link: None,
            segments: Vec::new(),
            order: HashMap::new(),
            building: Vec::new(),
            scroll: None,
            pointer: None,
        }
    }

    fn finish_frame(&mut self) {
        self.segments = std::mem::take(&mut self.building);
        self.order.clear();
        for (index, segment) in self.segments.iter().enumerate() {
            self.order.entry(segment.id.clone()).or_insert(index);
        }
        // A piece that went away takes the selection with it.
        let gone = |caret: &Option<Caret>| {
            caret
                .as_ref()
                .is_some_and(|caret| !self.order.contains_key(&caret.segment))
        };
        if gone(&self.anchor) || gone(&self.head) {
            self.anchor = None;
            self.head = None;
            self.dragging = false;
        }
    }

    fn segment(&self, id: &ElementId) -> Option<&Segment> {
        self.order.get(id).map(|&index| &self.segments[index])
    }

    /// The selection's ends as (piece, offset), earliest first.
    fn ordered(&self) -> Option<((usize, usize), (usize, usize))> {
        let anchor = self.anchor.as_ref()?;
        let head = self.head.as_ref()?;
        let anchor = (*self.order.get(&anchor.segment)?, anchor.offset);
        let head = (*self.order.get(&head.segment)?, head.offset);
        Some(if anchor <= head {
            (anchor, head)
        } else {
            (head, anchor)
        })
    }

    /// The part of the piece `id` (with `text`) that is selected.
    fn selected_in(&self, id: &ElementId, text: &str) -> Option<Range<usize>> {
        let index = *self.order.get(id)?;
        let (start, end) = self.ordered()?;
        if index < start.0 || index > end.0 {
            return None;
        }
        let from = if index == start.0 {
            floor_boundary(text, start.1)
        } else {
            0
        };
        let to = if index == end.0 {
            floor_boundary(text, end.1)
        } else {
            text.len()
        };
        (from < to).then_some(from..to)
    }

    /// The selected text, each piece's part joined to the last by the piece's separator. A
    /// selection that only touches the end of its first piece or the start of its last leaves
    /// that piece, and its separator, out.
    fn selected_text(&self) -> String {
        let Some((start, end)) = self.ordered() else {
            return String::new();
        };
        let parts: Vec<(usize, Range<usize>)> = (start.0..=end.0)
            .map(|index| {
                let text = self.segments[index].text.as_ref();
                let from = if index == start.0 {
                    floor_boundary(text, start.1)
                } else {
                    0
                };
                let to = if index == end.0 {
                    floor_boundary(text, end.1)
                } else {
                    text.len()
                };
                (index, from..to.max(from))
            })
            .collect();
        let touched_only = |(index, range): &(usize, Range<usize>)| {
            range.is_empty() && start.0 != end.0 && (*index == start.0 || *index == end.0)
        };
        let mut copied = String::new();
        let mut first = true;
        for part in parts.iter().filter(|part| !touched_only(part)) {
            let segment = &self.segments[part.0];
            if !first {
                copied.push_str(&segment.separator);
            }
            first = false;
            copied.push_str(&segment.text[part.1.clone()]);
        }
        copied
    }

    fn select_all(&mut self) {
        let (Some(first), Some(last)) = (self.segments.first(), self.segments.last()) else {
            return;
        };
        self.anchor = Some(Caret {
            segment: first.id.clone(),
            offset: 0,
        });
        self.head = Some(Caret {
            segment: last.id.clone(),
            offset: last.text.len(),
        });
    }

    fn clear(&mut self) {
        self.anchor = None;
        self.head = None;
    }

    /// The point of the text nearest the pointer, for a drag that has left the piece it
    /// started in: the piece under the pointer, else one level with it, else the start of the
    /// next piece below, so the selection does not stop in the gap between blocks.
    fn caret_at(&self, position: Point<Pixels>) -> Option<Caret> {
        let placed: Vec<(usize, Bounds<Pixels>)> = self
            .segments
            .iter()
            .enumerate()
            .filter_map(|(index, segment)| segment.bounds().map(|bounds| (index, bounds)))
            .collect();
        let caret = |index: usize, offset: usize| Caret {
            segment: self.segments[index].id.clone(),
            offset,
        };
        let measure = |index: usize| caret(index, self.segments[index].index_at(position));
        if let Some(&(index, _)) = placed.iter().find(|(_, bounds)| bounds.contains(&position)) {
            return Some(measure(index));
        }
        let level = placed
            .iter()
            .filter(|(_, bounds)| bounds.top() <= position.y && position.y <= bounds.bottom())
            .min_by(|(_, a), (_, b)| {
                let distance = |bounds: &Bounds<Pixels>| {
                    f32::from(bounds.left() - position.x)
                        .max(f32::from(position.x - bounds.right()))
                        .max(0.)
                };
                distance(a).total_cmp(&distance(b))
            });
        if let Some(&(index, _)) = level {
            return Some(measure(index));
        }
        if let Some(&(index, _)) = placed.iter().find(|(_, bounds)| bounds.top() > position.y) {
            return Some(caret(index, 0));
        }
        let &(index, _) = placed.last()?;
        Some(caret(index, self.segments[index].text.len()))
    }

    fn press(&mut self, id: &ElementId, event: &MouseDownEvent) {
        let Some(segment) = self.segment(id) else {
            return;
        };
        let index = segment.index_at(event.position);
        let pressed_link = (event.click_count == 1 && !event.modifiers.shift)
            .then(|| {
                segment
                    .links
                    .iter()
                    .find(|(range, _)| range.start <= index && index < range.end)
                    .map(|(_, url)| (url.clone(), segment.on_link.clone()))
            })
            .flatten();
        let range = match event.click_count {
            2 => Some(segment.word_at(index)),
            3.. => Some(segment.line_at(index)),
            _ => None,
        };
        let at = |offset| Caret {
            segment: id.clone(),
            offset,
        };
        match range {
            Some(range) => {
                self.anchor = Some(at(range.start));
                self.head = Some(at(range.end));
            }
            None if event.modifiers.shift && self.anchor.is_some() => {
                self.head = Some(at(index));
            }
            None => {
                self.anchor = Some(at(index));
                self.head = Some(at(index));
            }
        }
        self.pressed_link = pressed_link;
        self.dragging = event.click_count < 2;
        self.pointer = Some(event.position);
    }

    /// While a drag is held past the top or bottom of the scrolled area, scroll towards the
    /// pointer, faster the further past it is, and carry the selection with it. Whether it
    /// moved.
    fn scroll_towards_pointer(&mut self) -> bool {
        let (Some(scroll), Some(pointer)) = (&self.scroll, self.pointer) else {
            return false;
        };
        let viewport = scroll.bounds();
        let past = if pointer.y < viewport.top() {
            f32::from(pointer.y - viewport.top())
        } else if pointer.y > viewport.bottom() {
            f32::from(pointer.y - viewport.bottom())
        } else {
            return false;
        };
        let step = (past * 0.4).clamp(-40., 40.);
        let offset = scroll.offset();
        let lowest = -f32::from(scroll.max_offset().height);
        let y = (f32::from(offset.y) - step).clamp(lowest, 0.);
        if (y - f32::from(offset.y)).abs() < 0.5 {
            return false;
        }
        scroll.set_offset(point(offset.x, px(y)));
        true
    }
}

/// Lays out a scope's content with the scope current, so the text inside joins it.
struct ScopeRoot {
    scope: Entity<Scope>,
    child: AnyElement,
}

impl IntoElement for ScopeRoot {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for ScopeRoot {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        self.scope.update(cx, |scope, _| scope.building.clear());
        SCOPES.with(|scopes| scopes.borrow_mut().push(self.scope.clone()));
        let layout = self.child.request_layout(window, cx);
        SCOPES.with(|scopes| scopes.borrow_mut().pop());
        self.scope.update(cx, |scope, _| scope.finish_frame());
        (layout, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
        if !self.scope.read(cx).dragging {
            return;
        }
        let scrolled = self.scope.update(cx, |scope, cx| {
            let scrolled = scope.scroll_towards_pointer();
            if let Some(caret) = scope.pointer.and_then(|pointer| scope.caret_at(pointer))
                && scope.head.as_ref() != Some(&caret)
            {
                scope.head = Some(caret);
                cx.notify();
            }
            scrolled
        });
        if scrolled {
            // Keep scrolling while the pointer is held still past the edge.
            self.scope.update(cx, |_, cx| cx.notify());
            window.request_animation_frame();
        }
        // While a drag is under way, follow the pointer anywhere in the window, so the
        // selection keeps growing past the edge of the text it started in.
        window.on_mouse_event({
            let scope = self.scope.clone();
            move |event: &MouseMoveEvent, phase, _, cx| {
                if !phase.bubble() || event.pressed_button.is_none() {
                    return;
                }
                scope.update(cx, |scope, cx| {
                    scope.pointer = Some(event.position);
                    if let Some(caret) = scope.caret_at(event.position)
                        && scope.head.as_ref() != Some(&caret)
                    {
                        scope.head = Some(caret);
                        cx.notify();
                    }
                });
            }
        });
        let scope = self.scope.clone();
        window.on_mouse_event(move |_: &MouseUpEvent, phase, window, cx| {
            if !phase.bubble() {
                return;
            }
            let follow = scope.update(cx, |scope, cx| {
                scope.dragging = false;
                cx.notify();
                let link = scope.pressed_link.take();
                link.filter(|_| scope.anchor == scope.head)
            });
            if let Some((url, handler)) = follow {
                match handler {
                    Some(handler) => handler(&url, window, cx),
                    None => cx.open_url(&url),
                }
            }
        });
    }
}

/// Make `container` the scope's: it holds the focus, answers copy and select all, and a press
/// on it away from the text clears the selection.
fn scope_root(scope: Entity<Scope>, container: Stateful<Div>, cx: &App) -> ScopeRoot {
    let focus = scope.read(cx).focus.clone();
    let child = container
        .track_focus(&focus)
        .key_context(SELECTABLE_CONTEXT)
        .on_mouse_down(MouseButton::Left, {
            let scope = scope.clone();
            move |event: &MouseDownEvent, _, cx| {
                scope.update(cx, |scope, cx| {
                    let on_text = scope.segments.iter().any(|segment| {
                        segment
                            .bounds()
                            .is_some_and(|bounds| bounds.contains(&event.position))
                    });
                    if !on_text && scope.anchor.is_some() {
                        scope.clear();
                        cx.notify();
                    }
                });
            }
        })
        .on_action({
            let scope = scope.clone();
            move |_: &CopySelection, window, cx| {
                let scope = scope.read(cx);
                let copied = scope.selected_text();
                if !scope.focus.is_focused(window) || copied.is_empty() {
                    cx.propagate();
                    return;
                }
                cx.write_to_clipboard(ClipboardItem::new_string(copied));
            }
        })
        .on_action({
            let scope = scope.clone();
            move |_: &SelectAll, window, cx| {
                scope.update(cx, |scope, cx| {
                    if !scope.focus.is_focused(window) {
                        cx.propagate();
                        return;
                    }
                    scope.select_all();
                    cx.notify();
                });
            }
        })
        .into_any_element();
    ScopeRoot { scope, child }
}

/// Text the person can select as one: a drag that starts in one [`SelectableText`] inside it
/// runs on through the others, in reading order, and copies them all.
///
/// Wrap a conversation, an answer's blocks, the lines of a log or a code block in one, and the
/// person can drag across paragraphs, list items, code and messages the way they would in a
/// browser. ⌘A or Ctrl+A selects everything in it. Between pieces, a copy puts each piece's
/// separator ([`SelectableText::separator`]): a blank line unless the piece says otherwise.
///
/// A scope inside another is simply part of the outer one, so components that keep their own
/// text selectable as a whole ([`crate::Markdown`], [`crate::agent::CodeBlock`]) join a larger
/// selection when they sit in one. The scope holds the keyboard focus while it has a selection,
/// so ⌘C reaches it; it is no tab stop. Only the text drawn in the frame is in it: rows a
/// virtualised list has not drawn are not.
///
/// It is laid out as a plain block (`div`), styled with the usual methods.
#[derive(IntoElement)]
pub struct SelectionScope {
    id: ElementId,
    container: Div,
    scroll: Option<ScrollHandle>,
}

impl SelectionScope {
    /// A scope. The id keeps the selection from frame to frame.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            container: div(),
            scroll: None,
        }
    }

    /// The scrolled area the text sits in. A drag held past its top or bottom edge scrolls it,
    /// so a selection can run on past what is on screen.
    pub fn scroll(mut self, handle: &ScrollHandle) -> Self {
        self.scroll = Some(handle.clone());
        self
    }
}

impl ParentElement for SelectionScope {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.container.extend(elements);
    }
}

impl Styled for SelectionScope {
    fn style(&mut self) -> &mut StyleRefinement {
        self.container.style()
    }
}

impl RenderOnce for SelectionScope {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if current_scope().is_some() {
            return self.container.into_any_element();
        }
        let scope = window.use_keyed_state(self.id.clone(), cx, |_, cx| Scope::new(cx));
        scope.update(cx, |scope, _| scope.scroll = self.scroll);
        scope_root(scope, self.container.id(self.id), cx).into_any_element()
    }
}

/// Flatten overlapping runs into the sorted, disjoint ones a [`StyledText`] takes, each the
/// later runs laid over the earlier.
pub(crate) fn merge(
    runs: Vec<(Range<usize>, HighlightStyle)>,
    len: usize,
) -> Vec<(Range<usize>, HighlightStyle)> {
    let mut cuts: Vec<usize> = runs
        .iter()
        .flat_map(|(range, _)| [range.start.min(len), range.end.min(len)])
        .collect();
    cuts.sort_unstable();
    cuts.dedup();
    cuts.windows(2)
        .filter_map(|pair| {
            let (start, end) = (pair[0], pair[1]);
            runs.iter()
                .filter(|(range, _)| range.start <= start && end <= range.end)
                .map(|(_, style)| *style)
                .reduce(|under, over| under.highlight(over))
                .map(|style| (start..end, style))
        })
        .collect()
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
///
/// Inside a [`SelectionScope`] it is one piece of a larger selection: a drag runs on into the
/// next piece, and ⌘A selects the whole scope. On its own it is a scope of one.
///
/// [`SelectableText::styles`] sets runs in bold, italic or another colour, and
/// [`SelectableText::link`] makes a run a link that a click (not a drag) follows; that is how
/// [`crate::Markdown`] draws its paragraphs.
#[derive(IntoElement)]
pub struct SelectableText {
    id: ElementId,
    text: SharedString,
    size: TextSize,
    color: TextColor,
    mono: bool,
    selection: Option<Hsla>,
    styles: Vec<(Range<usize>, HighlightStyle)>,
    links: Vec<(Range<usize>, SharedString)>,
    on_link: Option<LinkHandler>,
    separator: SharedString,
}

/// What follows a link: given its address.
pub type LinkHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

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
            styles: Vec::new(),
            links: Vec::new(),
            on_link: None,
            separator: BLOCK_SEPARATOR.into(),
        }
    }

    /// What a copy puts between the text before this one in a [`SelectionScope`] and this:
    /// [`BLOCK_SEPARATOR`] (a blank line) unless set, [`LINE_SEPARATOR`] for the next line of
    /// code or a log, an empty string for a run of the same line.
    pub fn separator(mut self, separator: impl Into<SharedString>) -> Self {
        self.separator = separator.into();
        self
    }

    /// Draw byte ranges of the text in other styles: bold, italic, a colour, a fill. Ranges may
    /// overlap; where they do, the later one wins for what both set.
    pub fn styles(
        mut self,
        styles: impl IntoIterator<Item = (Range<usize>, HighlightStyle)>,
    ) -> Self {
        self.styles.extend(styles);
        self
    }

    /// Make a byte range of the text a link to `url`. A click on it, without a drag, opens it
    /// with the system's handler, or calls [`SelectableText::on_link`]. Style it with
    /// [`SelectableText::styles`].
    pub fn link(mut self, range: Range<usize>, url: impl Into<SharedString>) -> Self {
        self.links.push((range, url.into()));
        self
    }

    /// Follow links yourself instead of opening them with the system's handler.
    pub fn on_link(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_link = Some(Rc::new(handler));
        self
    }

    pub(crate) fn on_link_shared(mut self, handler: Option<LinkHandler>) -> Self {
        self.on_link = handler.or(self.on_link);
        self
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
        let Some(scope) = current_scope() else {
            // On its own, it is a scope of one.
            let id = ElementId::NamedChild(Box::new(self.id.clone()), "selection".into());
            return SelectionScope::new(id).child(self).into_any_element();
        };
        let theme = cx.theme();
        let color = self.color.resolve(theme);
        let highlight = self.selection.unwrap_or(theme.colors.selection);
        let size = self.size.pixels(theme);
        let mono = self.mono.then(|| theme.typography.mono.clone());

        let text = self.text;
        let (focus, selected) = {
            let scope = scope.read(cx);
            let selected = scope
                .focus
                .is_focused(window)
                .then(|| scope.selected_in(&self.id, &text))
                .flatten();
            (scope.focus.clone(), selected)
        };
        let mut runs = self.styles;
        if let Some(range) = selected {
            runs.push((
                range,
                HighlightStyle {
                    background_color: Some(highlight),
                    ..Default::default()
                },
            ));
        }
        let styled = StyledText::new(text.clone()).with_highlights(merge(runs, text.len()));
        let placed = Rc::new(Cell::new(false));
        scope.update(cx, |scope, _| {
            scope.building.push(Segment {
                id: self.id.clone(),
                text,
                layout: styled.layout().clone(),
                separator: self.separator,
                links: Rc::new(self.links),
                on_link: self.on_link,
                placed: placed.clone(),
            })
        });

        let mut element = crate::focus::selector(div().id(self.id.clone()), &self.id)
            .relative()
            .cursor_text()
            .text_size(size)
            .text_color(color);
        if let Some(mono) = mono {
            element = element.font(mono);
        }
        let id = self.id;
        element
            .on_mouse_down(
                MouseButton::Left,
                move |event: &MouseDownEvent, window, cx| {
                    window.focus(&focus);
                    scope.update(cx, |scope, cx| {
                        scope.press(&id, event);
                        cx.notify();
                    });
                },
            )
            .child(styled)
            .child(canvas(move |_, _, _| placed.set(true), |_, _, _, _| {}).absolute())
            .into_any_element()
    }
}
