//! The plan folds to its current step and reports it; changed files report the one pressed.

use std::cell::RefCell;
use std::rc::Rc;

use atelier::agent::{AgentActivity, AgentPhase, ChangeKind, ChangedFiles, FileChange, PlanView};
use atelier::{StepState, Theme, WindowFrame};
use gpui::{
    Context, IntoElement, Modifiers, ParentElement, Render, Styled, TestAppContext,
    VisualTestContext, Window, div,
};

#[derive(Default)]
struct Log {
    toggles: Vec<bool>,
    opened: Vec<usize>,
}

struct Host {
    log: Rc<RefCell<Log>>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let toggles = self.log.clone();
        let opened = self.log.clone();
        WindowFrame::new().size_full().child(
            div()
                .flex()
                .flex_col()
                .w(gpui::px(360.))
                .child(
                    AgentActivity::new("activity", AgentPhase::Writing)
                        .tokens(1_240)
                        .rate(62.)
                        .elapsed(std::time::Duration::from_secs(75))
                        .context(0.9),
                )
                .child(
                    PlanView::new("plan")
                        .step("Read", StepState::Done)
                        .step("Edit", StepState::Running)
                        .step("Test", StepState::Pending)
                        .on_toggle(move |open, _, _| toggles.borrow_mut().toggles.push(*open)),
                )
                .child(
                    ChangedFiles::new("files")
                        .file(FileChange::new("src/a.rs", 3, 1))
                        .file(FileChange::new("src/b.rs", 8, 0).kind(ChangeKind::Added))
                        .on_open(move |index, _, _| opened.borrow_mut().opened.push(*index)),
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
        move |_, _| Host { log }
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
fn the_plan_header_folds_and_unfolds(cx: &mut TestAppContext) {
    let (log, cx) = open(cx);
    click("plan-header", cx);
    click("plan-header", cx);
    assert_eq!(log.borrow().toggles, [false, true]);
}

#[gpui::test]
fn a_changed_file_reports_its_index(cx: &mut TestAppContext) {
    let (log, cx) = open(cx);
    click("files-1", cx);
    click("files-0", cx);
    assert_eq!(log.borrow().opened, [1, 0]);
}
