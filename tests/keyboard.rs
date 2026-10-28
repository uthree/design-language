//! Keyboard reach: every control can be found with Tab and pressed with Enter or Space, and a
//! test can find a component by its id.

use std::cell::RefCell;
use std::rc::Rc;

use atelier::{
    Button, Checkbox, ListItem, Segment, SegmentedControl, Switch, TextField, Theme, WindowFrame,
};
use gpui::{
    Context, IntoElement, KeyUpEvent, Keystroke, ParentElement, Render, Styled, TestAppContext,
    VisualTestContext, Window, div,
};

type Log = Rc<RefCell<Vec<String>>>;

struct Screen {
    log: Log,
}

fn note(
    log: &Log,
    what: &str,
) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static {
    let (log, what) = (log.clone(), what.to_owned());
    move |_, _, _| log.borrow_mut().push(what.clone())
}

impl Render for Screen {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let log = &self.log;
        let segments = {
            let log = log.clone();
            move |index: &usize, _: &mut Window, _: &mut gpui::App| {
                log.borrow_mut().push(format!("segment {index}"))
            }
        };
        WindowFrame::new().child(
            div()
                .flex()
                .flex_col()
                .child(Button::new("first", "First").on_click(note(log, "first")))
                .child(
                    Button::new("skipped", "Disabled")
                        .disabled(true)
                        .on_click(note(log, "skipped")),
                )
                .child(
                    Button::new("second", "Second")
                        .primary()
                        .on_click(note(log, "second")),
                )
                .child(ListItem::new("page", "A page").on_click(note(log, "page")))
                // A row that does nothing is no stop.
                .child(ListItem::new("static", "Static row"))
                .child(Switch::new("switch", false).on_click(note(log, "switch")))
                .child(Checkbox::new("check", false).on_click(note(log, "check")))
                .child(
                    SegmentedControl::new(
                        "segments",
                        [Segment::label("One"), Segment::label("Two")],
                        0,
                    )
                    .on_select(segments),
                )
                .child(TextField::new().id("field").w_full()),
        )
    }
}

/// Press and release each key in turn: gpui presses a focused control on the key's release.
fn press(cx: &mut VisualTestContext, keys: &str) {
    for key in keys.split(' ') {
        cx.simulate_keystrokes(key);
        cx.simulate_event(KeyUpEvent {
            keystroke: Keystroke::parse(key).unwrap(),
        });
    }
}

fn open(cx: &mut TestAppContext) -> (Log, &mut gpui::VisualTestContext) {
    cx.update(|cx| atelier::init(cx, Theme::default()));
    let log = Log::default();
    let (_, window) = cx.add_window_view({
        let log = log.clone();
        |_, _| Screen { log }
    });
    window.run_until_parked();
    (log, window)
}

#[gpui::test]
fn tab_reaches_every_control_in_order(cx: &mut TestAppContext) {
    let (log, cx) = open(cx);
    // Tab to each stop and press it: Enter for buttons and rows, Space for the toggles.
    press(
        cx,
        "tab enter tab enter tab enter tab space tab space tab enter tab enter",
    );
    assert_eq!(
        *log.borrow(),
        [
            "first",
            "second",
            "page",
            "switch",
            "check",
            "segment 0",
            "segment 1"
        ],
        "the disabled button and the static row are skipped",
    );
}

#[gpui::test]
fn shift_tab_goes_back(cx: &mut TestAppContext) {
    let (log, cx) = open(cx);
    press(cx, "tab tab shift-tab enter");
    assert_eq!(*log.borrow(), ["first"]);
}

#[gpui::test]
fn components_are_found_by_id(cx: &mut TestAppContext) {
    let (_, cx) = open(cx);
    for id in [
        "first", "second", "page", "switch", "check", "segments", "field",
    ] {
        assert!(cx.debug_bounds(id).is_some(), "no bounds for {id}");
    }
}

#[gpui::test]
fn ring_shows_after_tab_and_hides_after_a_click(cx: &mut TestAppContext) {
    let (_, cx) = open(cx);
    assert!(!cx.update(|_, cx| atelier::focus_visible(cx)));
    press(cx, "tab");
    assert!(cx.update(|_, cx| atelier::focus_visible(cx)));
    let first = cx.debug_bounds("first").unwrap();
    cx.simulate_click(first.center(), gpui::Modifiers::default());
    assert!(!cx.update(|_, cx| atelier::focus_visible(cx)));
}
