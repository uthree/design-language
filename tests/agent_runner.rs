//! An agent's events become the conversation's entries, and the person's answers go back.

use atelier::Theme;
use atelier::agent::{AgentFeed, ChatEntry, ChatView, Decision};
use atelier::agents::{AgentEvent, PermissionKind, PermissionRequest, ToolCall};
use gpui::{AppContext, Entity, TestAppContext};

fn chat(cx: &mut TestAppContext) -> Entity<ChatView> {
    cx.update(|cx| atelier::init(cx, Theme::default()));
    cx.new(|_| ChatView::new("chat"))
}

fn feed(
    feed: &mut AgentFeed,
    chat: &Entity<ChatView>,
    events: &[AgentEvent],
    cx: &mut TestAppContext,
) {
    chat.update(cx, |chat, cx| {
        for event in events {
            feed.apply(event, chat, cx);
        }
    });
}

fn kinds(chat: &Entity<ChatView>, cx: &mut TestAppContext) -> Vec<String> {
    chat.read_with(cx, |chat, _| {
        chat.entries()
            .map(|(_, entry)| match entry {
                ChatEntry::User { text, .. } => format!("user {text}"),
                ChatEntry::Assistant {
                    text, streaming, ..
                } => {
                    format!("assistant {text}{}", if *streaming { "…" } else { "" })
                }
                ChatEntry::Reasoning { text, .. } => format!("reasoning {text}"),
                ChatEntry::Tool { summary, state, .. } => format!("tool {summary} {state:?}"),
                ChatEntry::Approval { title, answer, .. } => format!("approval {title} {answer:?}"),
                ChatEntry::Notice { text, .. } => format!("notice {text}"),
                _ => "other".into(),
            })
            .collect()
    })
}

#[gpui::test]
fn a_turn_reads_as_its_entries(cx: &mut TestAppContext) {
    let chat = chat(cx);
    let mut agent = AgentFeed::new();
    feed(
        &mut agent,
        &chat,
        &[
            AgentEvent::Started,
            AgentEvent::SessionStarted { id: "s1".into() },
            AgentEvent::Prompted {
                text: "Build it".into(),
            },
            AgentEvent::Reasoning {
                text: "Build ".into(),
            },
            AgentEvent::Reasoning {
                text: "first.".into(),
            },
            AgentEvent::Text {
                text: "Look".into(),
            },
            AgentEvent::Text {
                text: "ing.".into(),
            },
            AgentEvent::ToolStarted {
                call: ToolCall {
                    id: "t1".into(),
                    name: "Bash".into(),
                    input: r#"{"command":"cargo build"}"#.into(),
                },
            },
            AgentEvent::ToolFinished {
                id: "t1".into(),
                ok: true,
                output: "Finished".into(),
            },
            AgentEvent::Text {
                text: "\n\n".into(),
            },
            AgentEvent::Text {
                text: "Done.".into(),
            },
        ],
        cx,
    );
    assert!(agent.busy());
    feed(
        &mut agent,
        &chat,
        &[AgentEvent::TurnCompleted {
            answer: String::new(),
        }],
        cx,
    );
    assert_eq!(
        kinds(&chat, cx),
        [
            "user Build it",
            "reasoning Build first.",
            "assistant Looking.",
            "tool Bash: cargo build Succeeded",
            "assistant Done.",
        ]
    );
    assert!(!agent.busy());
    assert_eq!(agent.session(), Some("s1"));
    let detail = chat.read_with(cx, |chat, _| {
        chat.entries()
            .find_map(|(_, entry)| match entry {
                ChatEntry::Tool { detail, .. } => detail.clone(),
                _ => None,
            })
            .unwrap()
    });
    assert!(detail.ends_with("Finished"), "{detail}");
}

#[gpui::test]
fn an_approval_answers_its_request_once(cx: &mut TestAppContext) {
    let chat = chat(cx);
    let mut agent = AgentFeed::new();
    feed(
        &mut agent,
        &chat,
        &[
            AgentEvent::Prompted {
                text: "Clean".into(),
            },
            AgentEvent::PermissionRequested {
                request: PermissionRequest {
                    id: "r7".into(),
                    kind: PermissionKind::Command,
                    summary: "Bash: rm -rf build".into(),
                    detail: String::new(),
                },
            },
        ],
        cx,
    );
    let entry = chat.read_with(cx, |chat, _| chat.entries().last().unwrap().0);
    assert_eq!(
        agent.decision(entry, Decision::Deny),
        Some(("r7".into(), false))
    );
    assert_eq!(agent.decision(entry, Decision::Allow), None);
}

#[gpui::test]
fn a_failed_turn_fails_its_open_calls_and_says_why(cx: &mut TestAppContext) {
    let chat = chat(cx);
    let mut agent = AgentFeed::new();
    feed(
        &mut agent,
        &chat,
        &[
            AgentEvent::Prompted { text: "Go".into() },
            AgentEvent::ToolStarted {
                call: ToolCall {
                    id: "t1".into(),
                    name: "Edit".into(),
                    input: r#"{"file_path":"a.rs"}"#.into(),
                },
            },
            AgentEvent::TurnFailed {
                message: "rate limited".into(),
            },
        ],
        cx,
    );
    assert_eq!(
        kinds(&chat, cx),
        ["user Go", "tool Edit: a.rs Failed", "notice rate limited"]
    );
    assert!(!agent.busy());
}

/// A stand-in `claude` that lists a model, answers one turn and stays for the next.
#[cfg(unix)]
fn fake_claude(directory: &std::path::Path) -> String {
    use std::os::unix::fs::PermissionsExt;
    let script = directory.join("claude");
    std::fs::write(
        &script,
        r#"#!/bin/sh
read init
echo '{"type":"control_response","response":{"subtype":"success","request_id":"atelier-initialize","response":{"models":[{"value":"opus"}]}}}'
while read line; do
  echo '{"type":"system","subtype":"init","session_id":"s9"}'
  echo '{"type":"assistant","message":{"content":[{"type":"text","text":"hello"}]}}'
  echo '{"type":"result","subtype":"success","is_error":false,"result":"hello"}'
done
"#,
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    script.to_string_lossy().into_owned()
}

#[cfg(unix)]
#[gpui::test]
fn a_connection_runs_the_cli_and_holds_messages_sent_mid_turn(cx: &mut TestAppContext) {
    use atelier::agent::AgentConnection;
    use atelier::agents::{AgentConfig, ProviderKind};

    let directory = std::env::temp_dir().join(format!("atelier-runner-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let chat = chat(cx);
    let config = AgentConfig {
        program: Some(fake_claude(&directory)),
        ..AgentConfig::new(ProviderKind::ClaudeCode, &directory)
    };
    let agent = cx.update(|cx| AgentConnection::start(config, "hi", &chat, cx).unwrap());
    cx.update(|cx| agent.send("again", cx));
    assert_eq!(agent.pending(), ["again"]);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while kinds(&chat, cx).len() < 4 {
        assert!(
            std::time::Instant::now() < deadline,
            "{:?}",
            kinds(&chat, cx)
        );
        cx.executor().allow_parking();
        std::thread::sleep(std::time::Duration::from_millis(20));
        cx.run_until_parked();
    }
    assert_eq!(
        kinds(&chat, cx),
        [
            "user hi",
            "assistant hello",
            "user again",
            "assistant hello"
        ]
    );
    assert!(agent.pending().is_empty());
    assert_eq!(agent.session_id().as_deref(), Some("s9"));
    assert_eq!(agent.models()[0].id, "opus");
    drop(agent);
    std::fs::remove_dir_all(&directory).ok();
}
