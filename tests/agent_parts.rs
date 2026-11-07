//! The choice card, the message queue, the conversation list and Markdown links report what
//! the person did.

use std::cell::RefCell;
use std::rc::Rc;

use atelier::agent::{
    AgentStatus, ChoiceCard, Conversation, ConversationList, MessageQueue, QueuedMessage,
};
use atelier::{Markdown, Theme, WindowFrame};
use gpui::{
    Context, IntoElement, Modifiers, ParentElement, Render, Styled, TestAppContext,
    VisualTestContext, Window, div, px,
};

type Log = Rc<RefCell<Vec<String>>>;

struct Host {
    log: Log,
    query: &'static str,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let log = |name: &'static str, log: &Log| {
            let log = log.clone();
            move |value: String| log.borrow_mut().push(format!("{name} {value}"))
        };
        let single = log("single", &self.log);
        let multiple = log("multiple", &self.log);
        let send_now = log("send-now", &self.log);
        let remove = log("remove", &self.log);
        let pause = log("pause", &self.log);
        let opened = log("open", &self.log);
        let more = log("more", &self.log);
        let link = log("link", &self.log);
        WindowFrame::new().size_full().child(
            div()
                .flex()
                .w(px(420.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .w(px(420.))
                        .child(
                            ChoiceCard::new("single", "Fix the recorder too?")
                                .options(["Fix both", "Only playback"])
                                .on_choose(move |picked, _, _| single(format!("{picked:?}"))),
                        )
                        .child(
                            ChoiceCard::new("multiple", "Which platforms?")
                                .options(["Windows", "macOS", "Linux"])
                                .multiple(true)
                                .on_choose(move |picked, _, _| multiple(format!("{picked:?}"))),
                        )
                        .child(
                            MessageQueue::new("queue")
                                .message("Check the recorder")
                                .message(
                                    QueuedMessage::new("Update the changelog")
                                        .attachment("CHANGELOG.md"),
                                )
                                .on_send_now(move |index, _, _| send_now(index.to_string()))
                                .on_remove(move |index, _, _| remove(index.to_string()))
                                .on_pause(move |paused, _, _| pause(paused.to_string())),
                        )
                        .child(
                            Markdown::new("doc", "[the docs](https://example.com/docs)")
                                .on_link(move |url, _, _| link(url.to_string())),
                        ),
                )
                .child(
                    div().w(px(280.)).h(px(400.)).child(
                        ConversationList::new("list")
                            .search(self.query, div())
                            .conversation(
                                Conversation::new("Fix flaky playback test")
                                    .status(AgentStatus::Running),
                            )
                            .conversation(
                                Conversation::new("Profile physical piano")
                                    .when("09:05")
                                    .preview("Most time goes to the soundboard"),
                            )
                            .conversation(Conversation::new("").preview("How do I add a word?"))
                            .on_open(move |index, _, _| opened(index.to_string()))
                            .on_more(move |index, _, _, _| more(index.to_string())),
                    ),
                ),
        )
    }
}

fn open<'a>(query: &'static str, cx: &'a mut TestAppContext) -> (Log, &'a mut VisualTestContext) {
    cx.update(|cx| {
        atelier::init(cx, Theme::default());
        atelier::motion::reduce_motion(cx);
    });
    let log = Log::default();
    let (_, window) = cx.add_window_view({
        let log = log.clone();
        move |_, _| Host { log, query }
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
fn one_answer_answers_on_a_press(cx: &mut TestAppContext) {
    let (log, cx) = open("", cx);
    click("single-option-1", cx);
    assert_eq!(*log.borrow(), ["single [1]"]);
}

#[gpui::test]
fn several_answers_wait_for_send(cx: &mut TestAppContext) {
    let (log, cx) = open("", cx);
    click("multiple-option-2", cx);
    click("multiple-option-0", cx);
    assert!(log.borrow().is_empty());
    click("multiple-send", cx);
    assert_eq!(*log.borrow(), ["multiple [0, 2]"]);
}

#[gpui::test]
fn queued_messages_report_their_index(cx: &mut TestAppContext) {
    let (log, cx) = open("", cx);
    click("queue-send-now-1", cx);
    click("queue-remove-0", cx);
    click("queue-pause", cx);
    assert_eq!(*log.borrow(), ["send-now 1", "remove 0", "pause true"]);
}

#[gpui::test]
fn a_search_keeps_matching_conversations(cx: &mut TestAppContext) {
    let (log, cx) = open("piano", cx);
    assert!(cx.debug_bounds("list-row-0").is_none());
    assert!(cx.debug_bounds("list-row-2").is_none());
    click("list-row-1", cx);
    // "More" is a button of its own: it does not open the conversation as well.
    click("list-row-1-more", cx);
    assert_eq!(*log.borrow(), ["open 1", "more 1"]);
}

#[gpui::test]
fn nothing_matching_says_so(cx: &mut TestAppContext) {
    let (_, cx) = open("violin", cx);
    assert!(cx.debug_bounds("list-row-0").is_none());
    assert!(cx.debug_bounds("list-row-1").is_none());
}

#[gpui::test]
fn a_click_on_a_link_follows_it(cx: &mut TestAppContext) {
    let (log, cx) = open("", cx);
    // On the words, not past their end.
    let text = cx.debug_bounds("doc-block-1").expect("the paragraph");
    let on_link = text.origin + gpui::point(px(12.), text.size.height / 2.);
    cx.simulate_click(on_link, Modifiers::default());
    cx.run_until_parked();
    assert_eq!(*log.borrow(), ["link https://example.com/docs"]);
}
