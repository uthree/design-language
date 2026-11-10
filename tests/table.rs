//! A table selects rows by pointer and keys, and asks for a cell to be edited.

use std::cell::RefCell;
use std::rc::Rc;

use atelier::{Column, RowMark, SpotBook, Table, TableCell, TableRow, Theme, WindowFrame};
use gpui::{
    Context, Entity, IntoElement, KeyUpEvent, Keystroke, Modifiers, MouseButton, MouseDownEvent,
    ParentElement, Render, Styled, TestAppContext, VisualTestContext, Window, div, px,
};

#[derive(Default)]
struct Log {
    activated: Vec<TableCell>,
}

struct Panel {
    log: Rc<RefCell<Log>>,
    selected: Option<usize>,
    spots: SpotBook,
}

impl Render for Panel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = (0..200).map(|index| {
            TableRow::new([
                format!("{}", index / 4 + 1),
                format!("C{}", index % 8),
                "96".into(),
            ])
            .mark(RowMark::Category(index % 3))
            .selected(self.selected == Some(index))
        });
        let log = self.log.clone();
        WindowFrame::new().child(
            div().w(px(360.)).h(px(300.)).child(
                Table::new(
                    "events",
                    [
                        Column::new("Bar").width(px(48.)).end().mono(),
                        Column::new("Event"),
                        Column::new("Vel").width(px(40.)).end(),
                    ],
                    rows,
                )
                .spots(&self.spots)
                .on_select(cx.listener(|this, row: &usize, _, cx| {
                    this.selected = Some(*row);
                    cx.notify();
                }))
                .on_activate(move |cell: &TableCell, _, _| log.borrow_mut().activated.push(*cell)),
            ),
        )
    }
}

fn open(cx: &mut TestAppContext) -> (Rc<RefCell<Log>>, Entity<Panel>, &mut VisualTestContext) {
    cx.update(|cx| {
        atelier::init(cx, Theme::default());
        atelier::motion::reduce_motion(cx);
    });
    let log = Rc::new(RefCell::new(Log::default()));
    let (panel, window) = cx.add_window_view({
        let log = log.clone();
        |_, _| Panel {
            log,
            selected: None,
            spots: SpotBook::new(),
        }
    });
    window.run_until_parked();
    (log, panel, window)
}

fn selected(panel: &Entity<Panel>, window: &mut VisualTestContext) -> Option<usize> {
    panel.read_with(window, |panel, _| panel.selected)
}

#[gpui::test]
fn a_click_selects_and_a_double_click_activates_the_cell(cx: &mut TestAppContext) {
    let (log, panel, window) = open(cx);
    let cell = window
        .debug_bounds("events-cell-2-1")
        .expect("the third row's event cell is drawn");
    window.simulate_click(cell.center(), Modifiers::none());
    assert_eq!(selected(&panel, window), Some(2));
    assert!(log.borrow().activated.is_empty());
    window.simulate_event(MouseDownEvent {
        position: cell.center(),
        button: MouseButton::Left,
        modifiers: Modifiers::none(),
        click_count: 2,
        first_mouse: false,
    });
    assert_eq!(
        log.borrow().activated,
        vec![TableCell { row: 2, column: 1 }]
    );
    // The cell recorded where it is, for an inline field to open over it.
    let spot = panel.read_with(window, |panel, _| panel.spots.clone());
    let key = Table::cell_key("events", TableCell { row: 2, column: 1 });
    let found = window.update(|window, _| spot.get(&key, window));
    assert_eq!(found, Some(cell));
}

#[gpui::test]
fn arrows_move_the_selection_and_enter_activates_the_row(cx: &mut TestAppContext) {
    let (log, panel, window) = open(cx);
    let cell = window.debug_bounds("events-cell-0-0").unwrap();
    window.simulate_click(cell.center(), Modifiers::none());
    window.simulate_keystrokes("down down");
    assert_eq!(selected(&panel, window), Some(2));
    window.simulate_keystrokes("up");
    assert_eq!(selected(&panel, window), Some(1));
    window.simulate_keystrokes("end");
    assert_eq!(selected(&panel, window), Some(199));
    // The last row was scrolled into view.
    assert!(window.debug_bounds("events-cell-199-0").is_some());
    window.simulate_keystrokes("enter");
    window.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    window.run_until_parked();
    assert_eq!(
        log.borrow().activated,
        vec![TableCell {
            row: 199,
            column: 0
        }]
    );
}
