//! Typing into a `TextInput`, and editing a label in place with `InlineEdit`.

use std::cell::RefCell;
use std::rc::Rc;

use atelier::{InlineEdit, SpotBook, TextInput, Theme, WindowFrame};
use gpui::{
    AppContext, Context, Entity, IntoElement, KeyBinding, Modifiers, ParentElement, Render,
    SharedString, Styled, TestAppContext, VisualTestContext, Window, actions, div, point, px,
};

actions!(test, [Play]);

#[derive(Default)]
struct Log {
    commits: Vec<String>,
    closes: usize,
    plays: usize,
}

struct Panel {
    log: Rc<RefCell<Log>>,
    spots: SpotBook,
    name: SharedString,
    editing: bool,
    input: Entity<TextInput>,
}

impl Render for Panel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let over = self.spots.get("name", window);
        let log = self.log.clone();
        WindowFrame::new().child(
            div()
                .p(px(40.))
                .flex()
                .flex_col()
                .gap(px(20.))
                .child(
                    div()
                        .relative()
                        .w(px(80.))
                        .h(px(20.))
                        .child(self.name.clone())
                        .child(self.spots.spot("name")),
                )
                .child(div().h(px(200.)).w_full())
                .children(self.editing.then_some(over).flatten().map(|over| {
                    let close_log = log.clone();
                    InlineEdit::new("rename", &self.input, over)
                        .original(self.name.clone())
                        .on_commit(cx.listener(move |this, text: &SharedString, _, cx| {
                            log.borrow_mut().commits.push(text.to_string());
                            this.name = text.clone();
                            this.editing = false;
                            cx.notify();
                        }))
                        .on_close(cx.listener(move |this, _, _, cx| {
                            close_log.borrow_mut().closes += 1;
                            this.editing = false;
                            cx.notify();
                        }))
                })),
        )
    }
}

fn open(cx: &mut TestAppContext) -> (Rc<RefCell<Log>>, Entity<Panel>, &mut VisualTestContext) {
    cx.update(|cx| {
        atelier::init(cx, Theme::default());
        atelier::motion::reduce_motion(cx);
        cx.bind_keys([KeyBinding::new("space", Play, None)]);
    });
    let log = Rc::new(RefCell::new(Log::default()));
    cx.update({
        let log = log.clone();
        |cx| cx.on_action(move |_: &Play, _| log.borrow_mut().plays += 1)
    });
    let (panel, window) = cx.add_window_view({
        let log = log.clone();
        |_, cx| Panel {
            log,
            spots: SpotBook::new(),
            name: "Bass".into(),
            editing: false,
            input: cx.new(TextInput::new),
        }
    });
    window.run_until_parked();
    (log, panel, window)
}

fn start_editing(panel: &Entity<Panel>, window: &mut VisualTestContext) {
    panel.update_in(window, |panel, _, cx| {
        let name = panel.name.to_string();
        panel.input.update(cx, |input, cx| input.set_text(name, cx));
        panel.editing = true;
        cx.notify();
    });
    window.run_until_parked();
}

fn text(panel: &Entity<Panel>, window: &mut VisualTestContext) -> String {
    panel.read_with(window, |panel, cx| panel.input.read(cx).text().to_owned())
}

#[gpui::test]
fn the_field_opens_over_the_label_with_the_name_selected(cx: &mut TestAppContext) {
    let (log, panel, window) = open(cx);
    start_editing(&panel, window);
    // The first key replaces the old name.
    window.simulate_input("Sub");
    assert_eq!(text(&panel, window), "Sub");
    window.simulate_keystrokes("space");
    window.simulate_input("bass");
    assert_eq!(text(&panel, window), "Sub bass");
    assert_eq!(log.borrow().plays, 0, "Space typed a space, not Play");
    window.simulate_keystrokes("backspace backspace enter");
    assert_eq!(log.borrow().commits, vec!["Sub ba".to_owned()]);
    window.run_until_parked();
    panel.read_with(window, |panel, _| assert!(!panel.editing));
    // With nothing focused, Space plays again.
    window.simulate_keystrokes("space");
    assert_eq!(log.borrow().plays, 1);
}

#[gpui::test]
fn escape_and_an_untouched_click_away_close_without_an_edit(cx: &mut TestAppContext) {
    let (log, panel, window) = open(cx);
    start_editing(&panel, window);
    window.simulate_input("Lead");
    window.simulate_keystrokes("escape");
    assert!(log.borrow().commits.is_empty());
    assert_eq!(log.borrow().closes, 1);

    start_editing(&panel, window);
    window.simulate_click(point(px(300.), px(250.)), Modifiers::none());
    assert!(log.borrow().commits.is_empty());
    assert_eq!(log.borrow().closes, 2);

    start_editing(&panel, window);
    window.simulate_input("Pad");
    window.simulate_click(point(px(300.), px(250.)), Modifiers::none());
    assert_eq!(log.borrow().commits, vec!["Pad".to_owned()]);
}

#[gpui::test]
fn editing_keys_undo_and_select(cx: &mut TestAppContext) {
    let (_, panel, window) = open(cx);
    start_editing(&panel, window);
    window.simulate_keystrokes("right");
    window.simulate_input("s");
    assert_eq!(text(&panel, window), "Basss");
    window.simulate_keystrokes("left left shift-left shift-left");
    window.simulate_input("e");
    assert_eq!(text(&panel, window), "Bess");
    window.simulate_keystrokes("secondary-z");
    assert_eq!(text(&panel, window), "Basss");
    window.simulate_keystrokes("secondary-a backspace");
    assert_eq!(text(&panel, window), "");
    window.simulate_keystrokes("secondary-z");
    assert_eq!(text(&panel, window), "Basss");
}
