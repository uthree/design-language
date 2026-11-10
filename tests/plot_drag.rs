//! A line plot that is its own control: a drag moves values in the plot's units, Shift slows
//! it without a jump, a double-click resets, the wheel steps and the arrows nudge.

use std::cell::RefCell;
use std::rc::Rc;

use atelier::plot::{DragPhase, LinePlot, PlotDrag, Series};
use atelier::{Theme, WindowFrame};
use gpui::{
    Context, IntoElement, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement, Pixels, Point, Render, ScrollDelta, ScrollWheelEvent, Styled, TestAppContext,
    TouchPhase, VisualTestContext, Window, div, point, px,
};

#[derive(Default)]
struct Log {
    drags: Vec<PlotDrag>,
    resets: usize,
    steps: Vec<i32>,
}

struct Panel {
    log: Rc<RefCell<Log>>,
}

impl Render for Panel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let (drag, reset, wheel) = (self.log.clone(), self.log.clone(), self.log.clone());
        WindowFrame::new().child(
            div().w(px(400.)).h(px(240.)).child(
                LinePlot::new()
                    .id("envelope")
                    .series(Series::line(vec![(0., 0.), (5., 1.), (10., 0.)]))
                    .x_range(0., 10.)
                    .y_range(0., 1.)
                    .on_drag(move |event: &PlotDrag, _, _| drag.borrow_mut().drags.push(*event))
                    .on_reset(move |_, _, _| reset.borrow_mut().resets += 1)
                    .on_wheel(move |steps: &i32, _, _| wheel.borrow_mut().steps.push(*steps)),
            ),
        )
    }
}

fn open(cx: &mut TestAppContext) -> (Rc<RefCell<Log>>, &mut VisualTestContext) {
    cx.update(|cx| {
        atelier::init(cx, Theme::default());
        atelier::motion::reduce_motion(cx);
    });
    let log = Rc::new(RefCell::new(Log::default()));
    let (_, window) = cx.add_window_view({
        let log = log.clone();
        |_, _| Panel { log }
    });
    window.run_until_parked();
    (log, window)
}

fn press(window: &mut VisualTestContext, at: Point<Pixels>, clicks: usize) {
    window.simulate_event(MouseDownEvent {
        position: at,
        button: MouseButton::Left,
        modifiers: Modifiers::none(),
        click_count: clicks,
        first_mouse: false,
    });
    window.run_until_parked();
}

fn drag_to(window: &mut VisualTestContext, at: Point<Pixels>, modifiers: Modifiers) {
    window.simulate_event(MouseMoveEvent {
        position: at,
        pressed_button: Some(MouseButton::Left),
        modifiers,
    });
    window.run_until_parked();
}

#[gpui::test]
fn a_drag_moves_in_plot_units_and_shift_slows_it_without_a_jump(cx: &mut TestAppContext) {
    let (log, window) = open(cx);
    let plot = window.debug_bounds("envelope").expect("the plot is drawn");
    let start = plot.center();
    press(window, start, 1);
    drag_to(window, start + point(px(40.), px(-20.)), Modifiers::none());
    let first = *log.borrow().drags.last().unwrap();
    assert_eq!(log.borrow().drags[0].phase, DragPhase::Start);
    assert!(
        (first.from.0 - 5.).abs() < 1.5,
        "pressed near the middle: {first:?}"
    );
    assert_eq!(first.phase, DragPhase::Move);
    // Rightwards and upwards are more, in the plot's own units.
    assert!(first.delta.0 > 0.5 && first.delta.0 < 2., "{first:?}");
    assert!(first.delta.1 > 0.05 && first.delta.1 < 0.3, "{first:?}");

    // Shift pressed on the way: no jump, then a tenth the rate.
    drag_to(window, start + point(px(40.), px(-20.)), Modifiers::shift());
    let held = log.borrow().drags.last().unwrap().delta;
    assert!((held.0 - first.delta.0).abs() < 1e-4);
    drag_to(window, start + point(px(80.), px(-20.)), Modifiers::shift());
    let fine = log.borrow().drags.last().unwrap().delta;
    let step = fine.0 - held.0;
    assert!(
        (step - first.delta.0 / 10.).abs() < 1e-3,
        "{step} vs {first:?}"
    );

    window.simulate_event(MouseUpEvent {
        position: start + point(px(80.), px(-20.)),
        button: MouseButton::Left,
        modifiers: Modifiers::shift(),
        click_count: 1,
    });
    window.run_until_parked();
    let last = *log.borrow().drags.last().unwrap();
    assert_eq!(last.phase, DragPhase::End);
    assert!((last.delta.0 - fine.0).abs() < 1e-4);
}

#[gpui::test]
fn double_click_resets_the_wheel_steps_and_arrows_nudge(cx: &mut TestAppContext) {
    let (log, window) = open(cx);
    let plot = window.debug_bounds("envelope").unwrap();
    press(window, plot.center(), 2);
    assert_eq!(log.borrow().resets, 1);

    for _ in 0..3 {
        window.simulate_event(ScrollWheelEvent {
            position: plot.center(),
            delta: ScrollDelta::Pixels(point(px(0.), px(8.))),
            modifiers: Modifiers::none(),
            touch_phase: TouchPhase::Moved,
        });
    }
    window.run_until_parked();
    // Half a notch at a time: one whole step after two, the third half still to come.
    assert_eq!(log.borrow().steps, vec![1]);

    // A press focuses the plot; then each arrow is a whole Start, Move, End.
    press(window, plot.center(), 1);
    window.simulate_event(MouseUpEvent {
        position: plot.center(),
        button: MouseButton::Left,
        modifiers: Modifiers::none(),
        click_count: 1,
    });
    log.borrow_mut().drags.clear();
    window.simulate_keystrokes("right");
    let drags = log.borrow().drags.clone();
    assert_eq!(
        drags.iter().map(|drag| drag.phase).collect::<Vec<_>>(),
        vec![DragPhase::Start, DragPhase::Move, DragPhase::End]
    );
    assert!((drags[1].delta.0 - 0.1).abs() < 1e-5 && drags[1].delta.1 == 0.);
    window.simulate_keystrokes("shift-up");
    let up = log.borrow().drags[4];
    assert!(up.delta.0 == 0. && (up.delta.1 - 0.001).abs() < 1e-6);
}
