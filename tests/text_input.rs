//! A `TextInput` beside a list of suggestions, in the Tab order, and set from the application.

use std::cell::RefCell;
use std::rc::Rc;

use atelier::{TextField, TextInput, TextInputEvent, Theme, WindowFrame};
use gpui::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, TestAppContext,
    VisualTestContext, Window, div, px,
};

struct Form {
    first: Entity<TextInput>,
    second: Entity<TextInput>,
}

impl Render for Form {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        WindowFrame::new().child(
            div()
                .p(px(40.))
                .flex()
                .flex_col()
                .gap(px(12.))
                .w(px(240.))
                .child(TextField::new().child(self.first.clone()))
                .child(TextField::new().child(self.second.clone())),
        )
    }
}

fn open(
    cx: &mut TestAppContext,
) -> (
    Rc<RefCell<Vec<TextInputEvent>>>,
    Entity<Form>,
    &mut VisualTestContext,
) {
    cx.update(|cx| {
        atelier::init(cx, Theme::default());
        atelier::motion::reduce_motion(cx);
    });
    let events = Rc::new(RefCell::new(Vec::new()));
    let (form, window) = cx.add_window_view({
        let events = events.clone();
        |_, cx| {
            let first = cx.new(|cx| TextInput::new(cx).with_text("Cm", cx));
            cx.subscribe(&first, move |_, _, event: &TextInputEvent, _| {
                if *event != TextInputEvent::Changed {
                    events.borrow_mut().push(event.clone());
                }
            })
            .detach();
            Form {
                first,
                second: cx.new(TextInput::new),
            }
        }
    });
    window.run_until_parked();
    form.update_in(window, |form, window, cx| {
        form.first.read(cx).focus(window);
    });
    window.run_until_parked();
    (events, form, window)
}

fn caret(form: &Entity<Form>, window: &mut VisualTestContext) -> usize {
    form.read_with(window, |form, cx| form.first.read(cx).buffer().caret())
}

#[gpui::test]
fn completing_hands_up_down_tab_and_escape_to_the_list(cx: &mut TestAppContext) {
    let (events, form, window) = open(cx);
    // Not completing: Up goes to the start, as in any single line.
    window.simulate_keystrokes("up");
    assert_eq!(caret(&form, window), 0);
    window.simulate_keystrokes("down");
    assert_eq!(caret(&form, window), 2);
    assert!(events.borrow().is_empty());

    form.update_in(window, |form, _, cx| {
        form.first
            .update(cx, |input, cx| input.set_completing(true, cx))
    });
    window.simulate_keystrokes("up down down tab escape");
    assert_eq!(
        *events.borrow(),
        vec![
            TextInputEvent::PreviousCompletion,
            TextInputEvent::NextCompletion,
            TextInputEvent::NextCompletion,
            TextInputEvent::AcceptCompletion,
            TextInputEvent::DismissCompletions,
        ]
    );
    assert_eq!(caret(&form, window), 2, "the caret stayed put");
    form.update_in(window, |form, window, cx| {
        assert!(
            form.first.read(cx).is_focused(window),
            "Tab did not move focus while completing"
        )
    });
}

#[gpui::test]
fn tab_moves_to_the_next_input(cx: &mut TestAppContext) {
    let (_, form, window) = open(cx);
    window.simulate_keystrokes("tab");
    form.update_in(window, |form, window, cx| {
        assert!(form.second.read(cx).is_focused(window));
        assert!(!form.first.read(cx).is_focused(window));
    });
    window.simulate_input("G7");
    form.read_with(window, |form, cx| {
        assert_eq!(form.second.read(cx).text(), "G7")
    });
}

#[gpui::test]
fn the_application_selects_and_replaces_a_range(cx: &mut TestAppContext) {
    let (_, form, window) = open(cx);
    form.update_in(window, |form, _, cx| {
        form.first.update(cx, |input, cx| {
            input.set_text("Cm7 G", cx);
            // Accepting `G7` for the word being typed.
            input.replace_range(4..5, "G7", cx);
            assert_eq!(input.text(), "Cm7 G7");
            assert_eq!(input.buffer().caret(), 6);
            input.select_range(4..6, cx);
            assert_eq!(input.buffer().selection(), 4..6);
        })
    });
    window.simulate_input("F");
    form.read_with(window, |form, cx| {
        assert_eq!(form.first.read(cx).text(), "Cm7 F")
    });
    window.simulate_keystrokes("secondary-z");
    form.read_with(window, |form, cx| {
        assert_eq!(form.first.read(cx).text(), "Cm7 G7")
    });
}
