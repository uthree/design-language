//! Keyboard reach: every control can be found with Tab and pressed with Enter or Space, and a
//! test can find a component by its id.

use std::cell::RefCell;
use std::rc::Rc;

use atelier::{
    Button, Checkbox, ListItem, Segment, SegmentedControl, Switch, TextField, Theme, TitleBar,
    WindowFrame,
};
use gpui::{
    Context, InteractiveElement, IntoElement, KeyUpEvent, Keystroke, Modifiers, MouseButton,
    MouseDownEvent, ParentElement, Render, Styled, TestAppContext, VisualTestContext, Window, div,
    point, px,
};

type Log = Rc<RefCell<Vec<String>>>;

struct Screen {
    log: Log,
    /// Whether the last press reached the top of the window marked handled.
    handled: Rc<RefCell<Option<bool>>>,
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
        let handled = self.handled.clone();
        let frame = WindowFrame::new()
            .child(TitleBar::new().title("Keys"))
            .child(
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
            );
        // Bubbling ends here, so this sees whether anything inside marked the press handled.
        div()
            .size_full()
            .on_any_mouse_down(move |_, window, _| {
                *handled.borrow_mut() = Some(window.default_prevented())
            })
            .child(frame)
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
    let (log, _, window) = open_watching(cx);
    (log, window)
}

fn open_watching(
    cx: &mut TestAppContext,
) -> (Log, Rc<RefCell<Option<bool>>>, &mut gpui::VisualTestContext) {
    cx.update(|cx| atelier::init(cx, Theme::default()));
    let log = Log::default();
    let handled = Rc::<RefCell<Option<bool>>>::default();
    let (_, window) = cx.add_window_view({
        let (log, handled) = (log.clone(), handled.clone());
        |_, _| Screen { log, handled }
    });
    window.run_until_parked();
    (log, handled, window)
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

#[gpui::test]
fn a_press_on_the_title_bar_is_left_for_the_window_move(cx: &mut TestAppContext) {
    let (log, handled, cx) = open_watching(cx);
    press(cx, "tab");
    // A press the window marks handled is one the platform starts no window move from.
    let title_bar = point(px(200.), px(10.));
    cx.simulate_mouse_move(title_bar, None, Modifiers::default());
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: title_bar,
        modifiers: Modifiers::default(),
        click_count: 1,
        first_mouse: false,
    });
    assert_eq!(*handled.borrow(), Some(false));
    // Focus went back to the window's root, so Tab starts from the first control again.
    press(cx, "tab enter");
    assert_eq!(*log.borrow(), ["first"]);
}
