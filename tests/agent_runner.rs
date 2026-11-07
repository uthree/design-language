//! An agent's events become the conversation's entries, and the person's answers go back.

use atelier::Theme;
use std::time::{Duration, UNIX_EPOCH};

use atelier::Strings;
use atelier::agent::{AgentFeed, ChatEntry, ChatView, Decision, UsageLimit, usage_limit};
use atelier::agents::{AgentEvent, PermissionKind, PermissionRequest, RateLimit, ToolCall};
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

fn limit(id: &str, minutes: u32, scope: Option<&str>, used: u32, reached: bool) -> RateLimit {
    RateLimit {
        id: id.into(),
        window_minutes: Some(minutes),
        scope: scope.map(Into::into),
        used_percent: used,
        resets_at: Some(1_000_000 + 2 * 3600 + 600),
        reached,
    }
}

#[gpui::test]
fn plan_limits_are_kept_as_last_reported_and_read_as_the_meter_shows_them(cx: &mut TestAppContext) {
    let chat = chat(cx);
    let mut agent = AgentFeed::new();
    feed(
        &mut agent,
        &chat,
        &[
            AgentEvent::RateLimits {
                limits: vec![
                    limit("five_hour", 300, None, 40, false),
                    limit("seven_day", 10_080, None, 70, false),
                ],
            },
            AgentEvent::RateLimits {
                limits: vec![limit("five_hour", 300, None, 45, false)],
            },
        ],
        cx,
    );
    let used: Vec<_> = agent
        .rate_limits()
        .iter()
        .map(|limit| (limit.id.as_str(), limit.used_percent))
        .collect();
    assert_eq!(used, [("five_hour", 45), ("seven_day", 70)]);
    // Nothing in the conversation changes.
    assert!(kinds(&chat, cx).is_empty());

    let now = UNIX_EPOCH + Duration::from_secs(1_000_000);
    let english = Strings::english();
    assert_eq!(
        usage_limit(&agent.rate_limits()[0], &english, now),
        UsageLimit::new("5-hour window", 0.45).resets("in 2 h 10 min")
    );
    let opus = limit("seven_day_opus", 10_080, Some("Opus"), 97, true);
    assert_eq!(
        usage_limit(&opus, &Strings::japanese(), now),
        UsageLimit::new("週間枠 · Opus", 1.).resets("2時間10分後")
    );
    // A reset already past says nothing about when.
    let later = now + Duration::from_secs(86_400);
    assert_eq!(
        usage_limit(&agent.rate_limits()[1], &english, later),
        UsageLimit::new("Weekly", 0.7)
    );
}
