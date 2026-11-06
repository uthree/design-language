//! Sliders and knobs move when used — by pointer, keys and double click — whether the
//! application stores the value or leaves it to the control.

use std::cell::RefCell;
use std::rc::Rc;

use atelier::{Knob, Slider, Theme, WindowFrame};
use gpui::{
    Context, IntoElement, Modifiers, MouseButton, MouseDownEvent, ParentElement, Render, Styled,
    TestAppContext, VisualTestContext, Window, div, point, px,
};

type Log = Rc<RefCell<Vec<f32>>>;

struct Panel {
    /// Values the stored slider and knob reported.
    changes: Log,
    /// Values the self-held knob drew its readout from.
    readouts: Log,
    level: f32,
    pan: f32,
}

impl Render for Panel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (changes, readouts) = (self.changes.clone(), self.readouts.clone());
        WindowFrame::new().child(
            div()
                .w(px(400.))
                .flex()
                .flex_col()
                .gap(px(20.))
                .child(Slider::new(self.level).id("level").step(0.1).on_change({
                    let changes = changes.clone();
                    cx.listener(move |this, value: &f32, _, cx| {
                        changes.borrow_mut().push(*value);
                        this.level = *value;
                        cx.notify();
                    })
                }))
                .child(
                    Knob::new(self.pan)
                        .id("pan")
                        .bipolar()
                        .on_change(cx.listener(move |this, value: &f32, _, cx| {
                            changes.borrow_mut().push(*value);
                            this.pan = *value;
                            cx.notify();
                        })),
                )
                .child(Knob::new(0.2).id("held").readout_with(move |value| {
                    readouts.borrow_mut().push(value);
                    format!("{value:.2}").into()
                })),
        )
    }
}

fn open(cx: &mut TestAppContext) -> (Log, Log, &mut VisualTestContext) {
    cx.update(|cx| {
        atelier::init(cx, Theme::default());
        atelier::motion::reduce_motion(cx);
    });
    let (changes, readouts) = (Log::default(), Log::default());
    let (_, window) = cx.add_window_view({
        let (changes, readouts) = (changes.clone(), readouts.clone());
        |_, _| Panel {
            changes,
            readouts,
            level: 0.,
            pan: 0.8,
        }
    });
    window.run_until_parked();
    (changes, readouts, window)
}

fn last(log: &Log) -> f32 {
    *log.borrow().last().expect("no value reported")
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.02
}

#[gpui::test]
fn a_click_along_the_slider_sets_it_there(cx: &mut TestAppContext) {
    let (changes, _, cx) = open(cx);
    let track = cx.debug_bounds("level").unwrap();
    // The thumb's centre runs from 7 px in from one end to 7 px in from the other.
    let x = track.left() + px(7.) + (track.size.width - px(14.)) * 0.75;
    cx.simulate_click(point(x, track.center().y), Modifiers::default());
    assert!(near(last(&changes), 0.75), "{:?}", changes.borrow());
}

#[gpui::test]
fn a_drag_along_the_slider_follows_the_pointer(cx: &mut TestAppContext) {
    let (changes, _, cx) = open(cx);
    let track = cx.debug_bounds("level").unwrap();
    let y = track.center().y;
    cx.simulate_mouse_down(
        point(track.left(), y),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.run_until_parked();
    let middle = point(track.center().x, y);
    cx.simulate_mouse_move(middle, Some(MouseButton::Left), Modifiers::default());
    assert!(near(last(&changes), 0.5), "{:?}", changes.borrow());
    // Past the end it stays at the end.
    let beyond = point(track.right() + px(100.), y);
    cx.simulate_mouse_move(beyond, Some(MouseButton::Left), Modifiers::default());
    cx.simulate_mouse_up(beyond, MouseButton::Left, Modifiers::default());
    assert_eq!(last(&changes), 1.);
}

#[gpui::test]
fn arrow_keys_step_a_focused_slider(cx: &mut TestAppContext) {
    let (changes, _, cx) = open(cx);
    cx.simulate_keystrokes("tab");
    cx.simulate_keystrokes("right");
    cx.simulate_keystrokes("right");
    assert!(near(last(&changes), 0.2), "{:?}", changes.borrow());
    cx.simulate_keystrokes("end");
    assert_eq!(last(&changes), 1.);
    cx.simulate_keystrokes("home");
    assert_eq!(last(&changes), 0.);
}

#[gpui::test]
fn dragging_a_knob_up_turns_it_up(cx: &mut TestAppContext) {
    let (changes, _, cx) = open(cx);
    let dial = cx.debug_bounds("pan").unwrap().center();
    cx.simulate_mouse_down(dial, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
    // A click alone does not move a knob.
    assert!(changes.borrow().is_empty());
    let down = point(dial.x, dial.y + px(100.));
    cx.simulate_mouse_move(down, Some(MouseButton::Left), Modifiers::default());
    cx.simulate_mouse_up(down, MouseButton::Left, Modifiers::default());
    // 100 px is half the range, from 0.8.
    assert!(near(last(&changes), 0.3), "{:?}", changes.borrow());
}

#[gpui::test]
fn a_double_click_returns_a_bipolar_knob_to_the_centre(cx: &mut TestAppContext) {
    let (changes, _, cx) = open(cx);
    let dial = cx.debug_bounds("pan").unwrap().center();
    cx.simulate_mouse_move(dial, None, Modifiers::default());
    cx.simulate_event(MouseDownEvent {
        position: dial,
        button: MouseButton::Left,
        modifiers: Modifiers::default(),
        click_count: 2,
        first_mouse: false,
    });
    assert_eq!(last(&changes), 0.5);
}

#[gpui::test]
fn a_knob_without_a_handler_keeps_its_own_value(cx: &mut TestAppContext) {
    let (_, readouts, cx) = open(cx);
    assert!(near(last(&readouts), 0.2));
    let dial = cx.debug_bounds("held").unwrap().center();
    cx.simulate_mouse_down(dial, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
    let up = point(dial.x, dial.y - px(60.));
    cx.simulate_mouse_move(up, Some(MouseButton::Left), Modifiers::default());
    cx.simulate_mouse_up(up, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
    // 60 px of 200 adds 0.3, and the readout is drawn from the new value.
    assert!(near(last(&readouts), 0.5), "{:?}", readouts.borrow());
}
