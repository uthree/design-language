//! The conversation view keeps its entries, follows the end while the person is there, and
//! reports approvals.

use std::cell::RefCell;
use std::rc::Rc;

use atelier::agent::{ChatAttachment, ChatEntry, ChatEvent, ChatView, Decision, ToolState};
use atelier::{Theme, WindowFrame};
use gpui::{
    AppContext, Context, Entity, IntoElement, Modifiers, ParentElement, Render, ScrollDelta,
    ScrollWheelEvent, Styled, TestAppContext, VisualTestContext, Window, div, point, px,
};

struct Host {
    chat: Entity<ChatView>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        WindowFrame::new().child(
            div()
                .w(px(420.))
                .h(px(300.))
                .flex()
                .child(self.chat.clone()),
        )
    }
}

type Events = Rc<RefCell<Vec<ChatEvent>>>;

fn open(cx: &mut TestAppContext) -> (Entity<ChatView>, Events, &mut VisualTestContext) {
    open_with(cx, |chat| chat)
}

fn open_with(
    cx: &mut TestAppContext,
    build: impl FnOnce(ChatView) -> ChatView,
) -> (Entity<ChatView>, Events, &mut VisualTestContext) {
    cx.update(|cx| {
        atelier::init(cx, Theme::default());
        atelier::motion::reduce_motion(cx);
    });
    let events = Events::default();
    let chat = cx.new(|_| build(ChatView::new("chat")));
    cx.update({
        let events = events.clone();
        let chat = chat.clone();
        move |cx| {
            cx.subscribe(&chat, move |_, event: &ChatEvent, _| {
                events.borrow_mut().push(event.clone())
            })
            .detach();
        }
    });
    let (_, window) = cx.add_window_view({
        let chat = chat.clone();
        |_, _| Host { chat }
    });
    window.run_until_parked();
    (chat, events, window)
}

fn fill(chat: &Entity<ChatView>, count: usize, cx: &mut VisualTestContext) {
    chat.update(cx, |chat, cx| {
        for index in 0..count {
            chat.push(ChatEntry::user(format!("Message {index}")), cx);
            chat.push(
                ChatEntry::assistant("An answer long enough to take a few lines of the panel, so that the conversation soon runs past its height."),
                cx,
            );
        }
    });
    cx.run_until_parked();
}

#[gpui::test]
fn a_streamed_answer_collects_its_text_and_ends(cx: &mut TestAppContext) {
    let (chat, _, cx) = open(cx);
    let answer = chat.update(cx, |chat, cx| {
        chat.push(ChatEntry::user("Why?"), cx);
        let answer = chat.push(ChatEntry::streaming(), cx);
        chat.append(answer, "Found ", cx);
        chat.append(answer, "it.", cx);
        chat.finish(answer, cx);
        answer
    });
    cx.run_until_parked();
    chat.read_with(cx, |chat, _| match chat.entry(answer) {
        Some(ChatEntry::Assistant {
            text, streaming, ..
        }) => {
            assert_eq!(text, "Found it.");
            assert!(!streaming);
        }
        _ => panic!("the answer is gone"),
    });
}

#[gpui::test]
fn a_tool_call_moves_on_and_takes_its_result(cx: &mut TestAppContext) {
    let (chat, _, cx) = open(cx);
    let call = chat.update(cx, |chat, cx| {
        let call = chat.push(
            ChatEntry::tool("bash", "Ran the tests", ToolState::Running),
            cx,
        );
        chat.push(
            ChatEntry::tool("read", "Read a file", ToolState::Succeeded),
            cx,
        );
        chat.append(call, "ok", cx);
        chat.set_tool_state(call, ToolState::Succeeded, cx);
        call
    });
    cx.run_until_parked();
    chat.read_with(cx, |chat, _| {
        assert!(matches!(
            chat.entry(call),
            Some(ChatEntry::Tool {
                state: ToolState::Succeeded,
                detail: Some(detail),
                ..
            }) if detail == "ok"
        ));
    });
}

#[gpui::test]
fn allowing_an_approval_reports_it_and_folds_the_card(cx: &mut TestAppContext) {
    let (chat, events, cx) = open(cx);
    let approval = chat.update(cx, |chat, cx| {
        chat.push(ChatEntry::approval("Run the tests?").allow_label("Run"), cx)
    });
    cx.run_until_parked();
    assert!(chat.read_with(cx, |chat, _| chat.awaiting_approval()));
    let allow = cx
        .debug_bounds("chat-approval-0-allow")
        .expect("the approval has an allow button");
    cx.simulate_click(allow.center(), Modifiers::default());
    cx.run_until_parked();
    assert_eq!(
        events.borrow().as_slice(),
        [ChatEvent::Decided {
            entry: approval,
            decision: Decision::Allow
        }]
    );
    assert!(!chat.read_with(cx, |chat, _| chat.awaiting_approval()));
}

#[gpui::test]
fn the_view_counts_what_arrives_while_the_person_reads_further_up(cx: &mut TestAppContext) {
    let (chat, _, cx) = open(cx);
    fill(&chat, 12, cx);
    assert_eq!(chat.read_with(cx, |chat, _| chat.unread()), 0);
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("chat-jump").is_none(),
        "at the end, with no jump button"
    );

    // Scroll back up, away from the end.
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(200.), px(150.)),
        delta: ScrollDelta::Pixels(point(px(0.), px(2000.))),
        ..Default::default()
    });
    cx.run_until_parked();
    chat.update(cx, |chat, cx| {
        chat.push(ChatEntry::assistant("New"), cx);
        chat.push(ChatEntry::assistant("Newer"), cx);
    });
    cx.run_until_parked();
    assert_eq!(chat.read_with(cx, |chat, _| chat.unread()), 2);
    assert!(
        cx.debug_bounds("chat-jump").is_some(),
        "no way back to the end"
    );

    chat.update(cx, |chat, cx| chat.jump_to_latest(cx));
    cx.run_until_parked();
    chat.update(cx, |chat, cx| {
        chat.push(ChatEntry::assistant("Newest"), cx);
    });
    cx.run_until_parked();
    assert_eq!(chat.read_with(cx, |chat, _| chat.unread()), 0);
}

#[gpui::test]
fn the_transcript_reads_as_text(cx: &mut TestAppContext) {
    let (chat, _, cx) = open(cx);
    chat.update(cx, |chat, cx| {
        chat.push(ChatEntry::user("Why?"), cx);
        chat.push(
            ChatEntry::tool("bash", "Ran the tests", ToolState::Succeeded),
            cx,
        );
        chat.push(ChatEntry::assistant("Because."), cx);
        chat.push(ChatEntry::custom(|_, _| div().into_any_element()), cx);
    });
    assert_eq!(
        chat.read_with(cx, |chat, _| chat.transcript_text()),
        "> Why?\n\n[bash] Ran the tests\n\nBecause."
    );
}

#[gpui::test]
fn a_question_takes_one_pick_and_reports_it(cx: &mut TestAppContext) {
    let (chat, events, cx) = open(cx);
    let id = chat.update(cx, |chat, cx| {
        chat.push(
            ChatEntry::choices("Fix the recorder too?", ["Fix both", "Only playback"]),
            cx,
        )
    });
    cx.run_until_parked();
    let option = cx
        .debug_bounds("chat-choices-0-option-1")
        .expect("the second answer");
    cx.simulate_click(option.center(), Modifiers::default());
    cx.run_until_parked();
    assert_eq!(
        *events.borrow(),
        [ChatEvent::Chose {
            entry: id,
            picked: vec![1]
        }]
    );
    // Answered: a second press changes nothing.
    assert!(!chat.read_with(cx, |chat, _| chat.choices_open(id)));
    cx.simulate_click(option.center(), Modifiers::default());
    cx.run_until_parked();
    assert_eq!(events.borrow().len(), 1);
}

#[gpui::test]
fn a_message_sent_past_a_question_closes_it(cx: &mut TestAppContext) {
    let (chat, _, cx) = open(cx);
    let id = chat.update(cx, |chat, cx| {
        let id = chat.push(
            ChatEntry::choices("Which platforms?", ["Windows", "macOS"]).multiple(),
            cx,
        );
        assert!(chat.choices_open(id));
        chat.push(ChatEntry::user("Never mind, just Linux"), cx);
        id
    });
    assert!(!chat.read_with(cx, |chat, _| chat.choices_open(id)));
}

#[gpui::test]
fn an_attachment_chip_asks_to_be_opened(cx: &mut TestAppContext) {
    let (chat, events, cx) = open_with(cx, ChatView::open_attachments);
    let message = chat.update(cx, |chat, cx| {
        chat.push(
            ChatEntry::user("Why does this clip clip?")
                .attachment("mix.wav")
                .attachment(ChatAttachment::new("Bars 17–24").icon(atelier::IconName::Music)),
            cx,
        )
    });
    cx.run_until_parked();
    let chip = cx
        .debug_bounds("chat-attachment-0-1-open")
        .expect("the second chip is pressable");
    cx.simulate_click(chip.center(), Modifiers::default());
    cx.run_until_parked();
    assert_eq!(
        events.borrow().as_slice(),
        [ChatEvent::OpenAttachment {
            entry: message,
            index: 1,
            attachment: ChatAttachment::new("Bars 17–24").icon(atelier::IconName::Music),
        }]
    );
}

#[gpui::test]
fn attachment_chips_stay_still_unless_asked(cx: &mut TestAppContext) {
    let (chat, _, cx) = open(cx);
    chat.update(cx, |chat, cx| {
        chat.push(ChatEntry::user("Look").attachment("mix.wav"), cx);
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("chat-attachment-0-0").is_some());
    assert!(cx.debug_bounds("chat-attachment-0-0-open").is_none());
}
