//! Dialogs take focus and answer Escape; approvals report the person's decision; the
//! permission picker opens its menu and reports the mode picked.

use std::cell::RefCell;
use std::rc::Rc;

use atelier::agent::{ApprovalDialog, Decision, PermissionMode, PermissionPicker};
use atelier::{Button, Dialog, Theme, WindowFrame};
use gpui::{
    Context, IntoElement, KeyUpEvent, Keystroke, Modifiers, ParentElement, Render, Styled,
    TestAppContext, VisualTestContext, Window, div, px,
};

#[derive(Clone, Copy, PartialEq)]
enum Show {
    Dialog,
    Approval { reviewing: bool },
    Picker,
}

#[derive(Default)]
struct Log {
    dismissed: usize,
    decisions: Vec<Decision>,
    modes: Vec<PermissionMode>,
}

struct Host {
    show: Show,
    log: Rc<RefCell<Log>>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let log = self.log.clone();
        let content = match self.show {
            Show::Dialog => Dialog::new("dialog", "Delete the clip?")
                .body("Undo brings it back.")
                .action(Button::new("cancel", "Cancel"))
                .on_dismiss(move |_, _, _| log.borrow_mut().dismissed += 1)
                .into_any_element(),
            Show::Approval { reviewing } => ApprovalDialog::new("approval", "Run the tests?")
                .command("cargo test")
                .allow_label("Run")
                .offer_always()
                .reviewing(reviewing)
                .on_decide(move |decision, _, _| log.borrow_mut().decisions.push(*decision))
                .into_any_element(),
            Show::Picker => div()
                .flex()
                .pt(px(400.))
                .child(
                    PermissionPicker::new("permission", PermissionMode::Ask)
                        .on_select(move |mode, _, _| log.borrow_mut().modes.push(*mode)),
                )
                .into_any_element(),
        };
        WindowFrame::new().size_full().child(content)
    }
}

fn open(show: Show, cx: &mut TestAppContext) -> (Rc<RefCell<Log>>, &mut VisualTestContext) {
    cx.update(|cx| {
        atelier::init(cx, Theme::default());
        atelier::motion::reduce_motion(cx);
    });
    let log = Rc::new(RefCell::new(Log::default()));
    let (_, window) = cx.add_window_view({
        let log = log.clone();
        move |_, _| Host { show, log }
    });
    window.run_until_parked();
    (log, window)
}

fn click(selector: &'static str, cx: &mut VisualTestContext) {
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("no {selector}"));
    cx.simulate_click(bounds.center(), Modifiers::default());
    cx.run_until_parked();
}

#[gpui::test]
fn escape_dismisses_a_dialog(cx: &mut TestAppContext) {
    let (log, cx) = open(Show::Dialog, cx);
    cx.simulate_keystrokes("escape");
    assert_eq!(log.borrow().dismissed, 1);
}

#[gpui::test]
fn a_press_on_the_card_does_not_dismiss_it(cx: &mut TestAppContext) {
    let (log, cx) = open(Show::Dialog, cx);
    let card = cx.debug_bounds("dialog").expect("no scrim");
    // The scrim fills the window; its centre is on the card.
    cx.simulate_click(card.center(), Modifiers::default());
    cx.run_until_parked();
    assert_eq!(log.borrow().dismissed, 0);
    // Its corner is not.
    cx.simulate_click(
        card.origin + gpui::point(px(4.), px(4.)),
        Modifiers::default(),
    );
    cx.run_until_parked();
    assert_eq!(log.borrow().dismissed, 1);
}

#[gpui::test]
fn an_approval_reports_each_answer(cx: &mut TestAppContext) {
    let (log, cx) = open(Show::Approval { reviewing: false }, cx);
    click("approval-allow", cx);
    click("approval-always", cx);
    click("approval-deny", cx);
    cx.simulate_keystrokes("escape");
    assert_eq!(
        log.borrow().decisions,
        [
            Decision::Allow,
            Decision::AlwaysAllow,
            Decision::Deny,
            Decision::Deny
        ]
    );
}

#[gpui::test]
fn deny_has_focus_so_enter_refuses(cx: &mut TestAppContext) {
    let (log, cx) = open(Show::Approval { reviewing: false }, cx);
    cx.simulate_keystrokes("enter");
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    cx.run_until_parked();
    assert_eq!(log.borrow().decisions, [Decision::Deny]);
}

#[gpui::test]
fn while_reviewing_only_deny_answers(cx: &mut TestAppContext) {
    let (log, cx) = open(Show::Approval { reviewing: true }, cx);
    click("approval-allow", cx);
    click("approval-always", cx);
    assert!(log.borrow().decisions.is_empty());
    click("approval-deny", cx);
    assert_eq!(log.borrow().decisions, [Decision::Deny]);
}

#[gpui::test]
fn the_picker_opens_its_menu_and_reports_the_mode(cx: &mut TestAppContext) {
    let (log, cx) = open(Show::Picker, cx);
    assert!(cx.debug_bounds("permission-mode-4").is_none());
    click("permission", cx);
    let bypass = cx
        .debug_bounds("permission-mode-4")
        .expect("the menu did not open");
    click("permission-mode-4", cx);
    assert_eq!(log.borrow().modes, [PermissionMode::Bypass]);
    // Picking closes the menu: the same place no longer picks anything.
    cx.simulate_click(bypass.center(), Modifiers::default());
    cx.run_until_parked();
    assert_eq!(log.borrow().modes, [PermissionMode::Bypass]);
}
