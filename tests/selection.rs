//! A selection runs across pieces of text: the blocks of an answer, the messages of a
//! conversation, the lines of a log or a code block, and copies them with their separators.

use atelier::agent::{ChatEntry, ChatView, CodeBlock};
use atelier::{LogLevel, LogLine, LogView, Markdown, SelectableText, SelectionScope, Theme};
use gpui::{
    AppContext, Bounds, ClipboardItem, Context, Entity, IntoElement, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Render, Styled,
    TestAppContext, VisualTestContext, Window, div, point, px,
};

/// What a test window shows.
type Body = Box<dyn Fn(&mut Window, &mut Context<Host>) -> gpui::AnyElement>;

struct Host {
    body: Body,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = (self.body)(window, cx);
        div().w(px(480.)).h(px(600.)).p(px(20.)).child(body)
    }
}

fn open(
    cx: &mut TestAppContext,
    body: impl Fn(&mut Window, &mut Context<Host>) -> gpui::AnyElement + 'static,
) -> (Entity<Host>, &mut VisualTestContext) {
    cx.update(|cx| {
        atelier::init(cx, Theme::default());
        atelier::motion::reduce_motion(cx);
    });
    let (host, window) = cx.add_window_view(|_, _| Host {
        body: Box::new(body),
    });
    window.run_until_parked();
    (host, window)
}

fn bounds(selector: &str, cx: &mut VisualTestContext) -> Bounds<Pixels> {
    let selector: &'static str = Box::leak(selector.to_owned().into_boxed_str());
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("no {selector}"))
}

/// A point just inside the start of a piece of text.
fn start_of(bounds: Bounds<Pixels>) -> Point<Pixels> {
    bounds.origin + point(px(1.), px(4.))
}

/// A point just inside the end of a piece of text's last line.
fn end_of(bounds: Bounds<Pixels>) -> Point<Pixels> {
    bounds.bottom_right() + point(px(20.), px(-4.))
}

fn drag(from: Point<Pixels>, to: Point<Pixels>, cx: &mut VisualTestContext) {
    cx.simulate_mouse_move(from, None, Modifiers::default());
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: from,
        modifiers: Modifiers::default(),
        click_count: 1,
        first_mouse: false,
    });
    cx.run_until_parked();
    cx.simulate_event(MouseMoveEvent {
        position: to,
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::default(),
    });
    cx.run_until_parked();
    cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: to,
        modifiers: Modifiers::default(),
        click_count: 1,
    });
    cx.run_until_parked();
}

fn copied(keys: &str, cx: &mut VisualTestContext) -> Option<String> {
    cx.write_to_clipboard(ClipboardItem::new_string(String::new()));
    cx.simulate_keystrokes(keys);
    cx.read_from_clipboard()
        .and_then(|item| item.text())
        .filter(|text| !text.is_empty())
}

#[gpui::test]
fn a_drag_runs_across_the_blocks_of_an_answer(cx: &mut TestAppContext) {
    let (_, cx) = open(cx, |_, _| {
        Markdown::new(
            "answer",
            "First paragraph.\n\n- one\n- two\n\n```\nlet x = 1;\n```",
        )
        .into_any_element()
    });
    let first = bounds("answer-block-1", cx);
    let code = bounds("answer-block-4", cx);
    drag(start_of(first), end_of(code), cx);
    assert_eq!(
        copied("ctrl-c", cx).as_deref(),
        Some("First paragraph.\n\none\ntwo\n\nlet x = 1;")
    );
}

#[gpui::test]
fn select_all_takes_the_whole_scope_and_a_press_away_clears_it(cx: &mut TestAppContext) {
    let (_, cx) = open(cx, |_, _| {
        SelectionScope::new("scope")
            .flex()
            .flex_col()
            .gap(px(40.))
            .child(SelectableText::new("a", "Alpha"))
            .child(SelectableText::new("b", "Beta").separator(" / "))
            .into_any_element()
    });
    let alpha = bounds("a", cx);
    drag(
        start_of(alpha),
        start_of(alpha) + point(px(10.), px(0.)),
        cx,
    );
    assert_eq!(copied("ctrl-a ctrl-c", cx).as_deref(), Some("Alpha / Beta"));
    // Between the two pieces is the scope but no text: a press there lets the selection go.
    let gap = alpha.bottom_left() + point(px(4.), px(20.));
    cx.simulate_click(gap, Modifiers::default());
    assert_eq!(copied("ctrl-c", cx), None);
}

#[gpui::test]
fn a_drag_through_a_gap_stops_at_the_next_piece(cx: &mut TestAppContext) {
    let (_, cx) = open(cx, |_, _| {
        SelectionScope::new("scope")
            .flex()
            .flex_col()
            .gap(px(40.))
            .child(SelectableText::new("a", "Alpha"))
            .child(SelectableText::new("b", "Beta"))
            .into_any_element()
    });
    let alpha = bounds("a", cx);
    let gap = alpha.bottom_left() + point(px(4.), px(20.));
    drag(start_of(alpha), gap, cx);
    // Up to the start of the next piece: all of the first, none of the second.
    assert_eq!(copied("ctrl-c", cx).as_deref(), Some("Alpha"));
}

#[gpui::test]
fn a_conversation_selects_from_one_message_into_the_next(cx: &mut TestAppContext) {
    cx.update(|cx| atelier::init(cx, Theme::default()));
    let chat = cx.new(|_| ChatView::new("chat"));
    chat.update(cx, |chat, cx| {
        chat.push(ChatEntry::user("Why?"), cx);
        chat.push(ChatEntry::assistant("Because **it** is."), cx);
        chat.push(ChatEntry::error("The connection dropped."), cx);
    });
    let (_, cx) = open(cx, move |_, _| {
        div()
            .size_full()
            .flex()
            .child(chat.clone())
            .into_any_element()
    });
    let question = bounds("chat-user-0-text", cx);
    let error = bounds("chat-notice-2", cx);
    drag(start_of(question), end_of(error), cx);
    assert_eq!(
        copied("ctrl-c", cx).as_deref(),
        Some("Why?\n\nBecause it is.\n\nThe connection dropped.")
    );
}

#[gpui::test]
fn a_log_copies_as_a_terminal_would(cx: &mut TestAppContext) {
    let (_, cx) = open(cx, |_, _| {
        LogView::new("log")
            .line(
                LogLine::new("1", "lost the device")
                    .time("12s")
                    .level(LogLevel::Warn)
                    .source("app::audio"),
            )
            .line(LogLine::new("2", "back").time("13s").level(LogLevel::Info))
            .into_any_element()
    });
    let first = bounds("log-1-time", cx);
    let last = bounds("log-2-message", cx);
    drag(start_of(first), end_of(last), cx);
    assert_eq!(
        copied("ctrl-c", cx).as_deref(),
        Some("12s WARN app::audio: lost the device\n13s INFO back")
    );
}

#[gpui::test]
fn code_copies_its_lines_without_their_numbers(cx: &mut TestAppContext) {
    let (_, cx) = open(cx, |_, _| {
        CodeBlock::new("code", "fn main() {\n    run();\n}")
            .line_numbers(1)
            .into_any_element()
    });
    let first = bounds("code-line-0", cx);
    let last = bounds("code-line-2", cx);
    drag(start_of(first), end_of(last), cx);
    assert_eq!(
        copied("ctrl-c", cx).as_deref(),
        Some("fn main() {\n    run();\n}")
    );
}

#[gpui::test]
fn a_drag_held_past_the_bottom_scrolls_the_conversation_on(cx: &mut TestAppContext) {
    cx.update(|cx| atelier::init(cx, Theme::default()));
    let chat = cx.new(|_| ChatView::new("chat"));
    chat.update(cx, |chat, cx| {
        for index in 0..12 {
            chat.push(ChatEntry::user(format!("Question {index}")), cx);
            chat.push(
                ChatEntry::assistant(format!(
                    "Answer {index}, long enough to wrap onto a second line of the panel."
                )),
                cx,
            );
        }
    });
    let (_, cx) = open(cx, move |_, _| {
        div()
            .w(px(420.))
            .h(px(300.))
            .flex()
            .child(chat.clone())
            .into_any_element()
    });
    // Back to the top, then drag from the first question to below the panel and hold there.
    cx.simulate_event(gpui::ScrollWheelEvent {
        position: point(px(200.), px(150.)),
        delta: gpui::ScrollDelta::Pixels(point(px(0.), px(100_000.))),
        ..Default::default()
    });
    cx.run_until_parked();
    let question = bounds("chat-user-0-text", cx);
    let below = point(px(200.), px(400.));
    cx.simulate_mouse_move(start_of(question), None, Modifiers::default());
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: start_of(question),
        modifiers: Modifiers::default(),
        click_count: 1,
        first_mouse: false,
    });
    cx.run_until_parked();
    cx.simulate_event(MouseMoveEvent {
        position: below,
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::default(),
    });
    for _ in 0..200 {
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
    }
    cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: below,
        modifiers: Modifiers::default(),
        click_count: 1,
    });
    let text = copied("ctrl-c", cx).expect("a selection");
    assert!(text.starts_with("Question 0"), "{text}");
    assert!(text.contains("Answer 11"), "scrolled on to the end: {text}");
}
