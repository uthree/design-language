//! Each of the four adapters, driven through a real `AgentSession` against `atelier-agents-mock`
//! playing that CLI: the same scenario must read the same way whichever CLI plays it.

use std::sync::mpsc;
use std::time::Duration;

use atelier_agents::{AgentConfig, AgentSession, adapter};
use atelier_agents::{
    AgentEvent, Effort, ListModelsError, PermissionKind, ProviderKind, SessionMode, list_models,
};
use atelier_agents_mock::cli::MODELS;
use atelier_agents_mock::{SCENARIO_FILE, Scenario};

const ALL: [ProviderKind; 4] = ProviderKind::ALL;

/// A worker on `provider` with the scenario in its working directory.
struct Worker {
    config: AgentConfig,
    _directory: tempfile::TempDir,
}

fn worker(provider: ProviderKind, scenario: &str) -> Worker {
    let scenario = Scenario::from_json(scenario).expect("a valid scenario");
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join(SCENARIO_FILE), scenario.to_json()).unwrap();
    let config = AgentConfig {
        effort: Some(Effort::Medium),
        program: Some(env!("CARGO_BIN_EXE_atelier-agents-mock").into()),
        ..AgentConfig::new(provider, directory.path())
    };
    Worker {
        config,
        _directory: directory,
    }
}

/// What happened in the first turn of `worker` given `prompt`. Permission requests are answered
/// from `answers` in order; `act` may act on the session at each event.
fn turn(
    worker: &Worker,
    prompt: &str,
    answers: &[bool],
    act: impl Fn(&AgentSession, &AgentEvent),
) -> Vec<AgentEvent> {
    let (sender, events) = mpsc::channel();
    let session = AgentSession::spawn(adapter(worker.config.clone()), prompt, move |event| {
        sender.send(event).ok();
    })
    .unwrap();
    let mut answers = answers.iter();
    let mut seen = Vec::new();
    loop {
        let event = events
            .recv_timeout(Duration::from_secs(10))
            .expect("an event in time");
        if let AgentEvent::PermissionRequested { request } = &event {
            session.answer(
                request.id.clone(),
                *answers.next().expect("an answer to give"),
            );
        }
        act(&session, &event);
        let end = matches!(
            event,
            AgentEvent::TurnCompleted { .. }
                | AgentEvent::TurnFailed { .. }
                | AgentEvent::Exited { .. }
        );
        seen.push(event);
        if end {
            return seen;
        }
    }
}

fn text(events: &[AgentEvent]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn count(events: &[AgentEvent], wanted: impl Fn(&AgentEvent) -> bool) -> usize {
    events.iter().filter(|event| wanted(event)).count()
}

const FULL_TURN: &str = r#"{
    "stream": {"chunk": 3},
    "rules": [{"when": {"prompt_contains": "build"}, "reply": [
        {"noise": "Welcome to the CLI"},
        {"think": "Build first."},
        {"say": "Looking."},
        {"tool": {"name": "Bash", "input": {"command": "cargo build"}, "output": "Finished", "ask": true}},
        {"tool": {"name": "Edit", "input": {"file_path": "src/lib.rs"}, "ask": true}},
        {"say": "Done."}
    ]}]
}"#;

#[test]
fn a_turn_streams_reasons_calls_tools_and_asks_permission_on_every_cli() {
    for provider in ALL {
        let worker = worker(provider, FULL_TURN);
        let events = turn(&worker, "Please build it", &[true, false], |_, _| {});
        let label = provider.label();
        // Only the models the CLI listed in its handshake may come before it starts.
        let first = events
            .iter()
            .find(|event| !matches!(event, AgentEvent::Models { .. }));
        assert_eq!(first, Some(&AgentEvent::Started), "{label}: {events:#?}");
        let Some(AgentEvent::TurnCompleted { answer }) = events.last() else {
            panic!("{label} did not complete: {events:#?}")
        };
        // Two messages of one turn read as two paragraphs, in the answer and as streamed.
        assert_eq!(answer, "Looking.\n\nDone.", "{label}");
        assert_eq!(
            &text(&events),
            answer,
            "{label}: the streamed text is the answer"
        );

        let asked: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                AgentEvent::PermissionRequested { request } => Some(request.kind),
                _ => None,
            })
            .collect();
        assert_eq!(asked.len(), 2, "{label}");
        if provider != ProviderKind::Antigravity {
            assert_eq!(
                asked,
                [PermissionKind::Command, PermissionKind::FileChange],
                "{label}"
            );
        }
        let started = count(&events, |event| {
            matches!(event, AgentEvent::ToolStarted { .. })
        });
        assert_eq!(started, 2, "{label}");
        let finished: Vec<bool> = events
            .iter()
            .filter_map(|event| match event {
                AgentEvent::ToolFinished { ok, .. } => Some(*ok),
                _ => None,
            })
            .collect();
        assert_eq!(finished, [true, false], "{label}: the refused call failed");
        let reasoned = count(&events, |event| {
            matches!(event, AgentEvent::Reasoning { .. })
        });
        assert_eq!(
            reasoned > 0,
            provider != ProviderKind::Antigravity,
            "{label}"
        );
    }
}

#[test]
fn a_failing_step_fails_the_turn_on_every_cli() {
    let scenario = r#"{"rules": [{"reply": [{"say": "Trying."}, {"fail": "rate limited"}]}]}"#;
    for provider in ALL {
        let events = turn(&worker(provider, scenario), "go", &[], |_, _| {});
        let Some(AgentEvent::TurnFailed { message }) = events.last() else {
            panic!("{}: {events:#?}", provider.label())
        };
        assert!(
            message.contains("rate limited"),
            "{}: {message}",
            provider.label()
        );
    }
}

#[test]
fn an_exiting_cli_ends_the_session_with_its_status() {
    let scenario = r#"{"rules": [{"reply": [{"say": "Bye"}, {"exit": 3}]}]}"#;
    for provider in ALL {
        let events = turn(&worker(provider, scenario), "go", &[], |_, _| {});
        let Some(AgentEvent::Exited {
            reason: Some(reason),
        }) = events.last()
        else {
            panic!("{}: {events:#?}", provider.label())
        };
        assert!(reason.contains('3'), "{}: {reason}", provider.label());
    }
}

#[test]
fn an_interrupt_ends_a_hanging_turn() {
    let scenario = r#"{"rules": [{"reply": [{"say": "Working"}, "hang"]}]}"#;
    // `agy` has no interrupt in its protocol; Maestro kills it instead.
    for provider in [
        ProviderKind::ClaudeCode,
        ProviderKind::Codex,
        ProviderKind::OpenCode,
    ] {
        let events = turn(&worker(provider, scenario), "go", &[], |session, event| {
            if matches!(event, AgentEvent::Text { .. }) {
                session.interrupt();
            }
        });
        assert!(
            matches!(events.last(), Some(AgentEvent::TurnFailed { .. })),
            "{}: {events:#?}",
            provider.label()
        );
    }
}

#[test]
fn rules_see_the_turn_whether_the_cli_is_read_only_and_the_model() {
    let scenario = r#"{"rules": [
        {"when": {"read_only": true, "model": "m-1"}, "reply": [{"say": "planner on m-1"}]},
        {"when": {"turn": 1}, "reply": [{"say": "worker"}]}
    ]}"#;
    for provider in [
        ProviderKind::ClaudeCode,
        ProviderKind::Codex,
        ProviderKind::OpenCode,
    ] {
        let mut planner = worker(provider, scenario);
        planner.config.read_only = true;
        planner.config.model = Some("m-1".into());
        let events = turn(&planner, "plan", &[], |_, _| {});
        let label = provider.label();
        assert_eq!(
            events.last(),
            Some(&AgentEvent::TurnCompleted {
                answer: "planner on m-1".into()
            }),
            "{label}"
        );
        let events = turn(&worker(provider, scenario), "work", &[], |_, _| {});
        assert_eq!(
            events.last(),
            Some(&AgentEvent::TurnCompleted {
                answer: "worker".into()
            }),
            "{label}"
        );
    }
}

#[test]
fn later_turns_follow_the_conversation() {
    let scenario = r#"{"otherwise": [{"say": "turn {turn}: {prompt}"}]}"#;
    for provider in ALL {
        let worker = worker(provider, scenario);
        let (sender, events) = mpsc::channel();
        let session = AgentSession::spawn(adapter(worker.config.clone()), "one", move |event| {
            sender.send(event).ok();
        })
        .unwrap();
        let mut answers = Vec::new();
        while answers.len() < 2 {
            match events
                .recv_timeout(Duration::from_secs(10))
                .expect("an event in time")
            {
                AgentEvent::TurnCompleted { answer } => {
                    answers.push(answer);
                    session.send("two");
                }
                AgentEvent::TurnFailed { message } => panic!("{message}"),
                _ => {}
            }
        }
        assert_eq!(
            answers,
            ["turn 1: one", "turn 2: two"],
            "{}",
            provider.label()
        );
    }
}

/// Every event of a session until `until` matches or it exits, with no permissions asked.
fn events_until(
    config: &atelier_agents::AgentConfig,
    prompt: &str,
    until: impl Fn(&AgentEvent) -> bool,
) -> Vec<AgentEvent> {
    let (sender, events) = mpsc::channel();
    let _session = AgentSession::spawn(adapter(config.clone()), prompt, move |event| {
        sender.send(event).ok();
    })
    .unwrap();
    let mut seen = Vec::new();
    loop {
        let event = events
            .recv_timeout(Duration::from_secs(10))
            .expect("an event in time");
        let end = until(&event) || matches!(event, AgentEvent::Exited { .. });
        seen.push(event);
        if end {
            return seen;
        }
    }
}

fn session_id(events: &[AgentEvent]) -> Option<String> {
    events.iter().find_map(|event| match event {
        AgentEvent::SessionStarted { id } => Some(id.clone()),
        _ => None,
    })
}

const ECHO: &str = r#"{"otherwise": [{"say": "turn {turn}: {prompt}"}]}"#;

fn turn_ended(event: &AgentEvent) -> bool {
    matches!(
        event,
        AgentEvent::TurnCompleted { .. } | AgentEvent::TurnFailed { .. }
    )
}

#[test]
fn a_kept_session_is_reported_and_carried_on() {
    for provider in [
        ProviderKind::ClaudeCode,
        ProviderKind::Codex,
        ProviderKind::OpenCode,
    ] {
        let label = provider.label();
        let mut worker = worker(provider, ECHO);
        let first = events_until(&worker.config, "hello", turn_ended);
        let id = session_id(&first).unwrap_or_else(|| panic!("{label}: {first:#?}"));

        worker.config.session = SessionMode::Resume(id.clone());
        let resumed = events_until(&worker.config, "again", turn_ended);
        assert_eq!(
            session_id(&resumed).as_deref(),
            Some(id.as_str()),
            "{label}"
        );
        let Some(AgentEvent::TurnCompleted { answer }) = resumed.last() else {
            panic!("{label}: {resumed:#?}")
        };
        // A loaded session's replayed history is not reported as new text.
        assert_eq!(&text(&resumed), answer, "{label}");
        assert!(answer.ends_with("again"), "{label}: {answer}");
    }
}

#[test]
fn an_ephemeral_session_reports_no_id() {
    for provider in [ProviderKind::ClaudeCode, ProviderKind::Codex] {
        let mut worker = worker(provider, ECHO);
        worker.config.session = SessionMode::Ephemeral;
        let events = events_until(&worker.config, "hello", turn_ended);
        assert!(events.last().is_some_and(turn_ended), "{events:#?}");
        assert_eq!(session_id(&events), None, "{}", provider.label());
    }
}

#[test]
fn a_lost_session_fails_before_any_prompt_is_sent() {
    for provider in [
        ProviderKind::ClaudeCode,
        ProviderKind::Codex,
        ProviderKind::OpenCode,
    ] {
        let label = provider.label();
        let mut worker = worker(provider, ECHO);
        worker.config.session = SessionMode::Resume("missing-1".into());
        let events = events_until(&worker.config, "hello", |event| {
            matches!(event, AgentEvent::ResumeFailed { .. })
        });
        assert!(
            matches!(
                events.last(),
                Some(AgentEvent::ResumeFailed { .. } | AgentEvent::Exited { .. })
            ),
            "{label}: {events:#?}"
        );
        assert_eq!(session_id(&events), None, "{label}");
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, AgentEvent::Text { .. })),
            "{label}: no turn ran: {events:#?}"
        );
    }
}

#[test]
fn models_are_listed_without_a_conversation() {
    for provider in [
        ProviderKind::ClaudeCode,
        ProviderKind::Codex,
        ProviderKind::OpenCode,
    ] {
        let worker = worker(provider, ECHO);
        let models = list_models(worker.config.clone(), Duration::from_secs(10))
            .unwrap_or_else(|error| panic!("{}: {error}", provider.label()));
        let ids: Vec<_> = models.iter().map(|model| model.id.as_str()).collect();
        assert_eq!(ids, MODELS, "{}", provider.label());
    }
    let worker = worker(ProviderKind::Antigravity, ECHO);
    assert!(matches!(
        list_models(worker.config.clone(), Duration::from_secs(10)),
        Err(ListModelsError::Unsupported)
    ));
}

#[test]
fn prepared_files_are_written_privately_before_the_cli_starts() {
    let mut worker = worker(ProviderKind::ClaudeCode, ECHO);
    let scratch = worker._directory.path().join("private");
    worker.config.scratch = Some(scratch.clone());
    worker.config.system_prompt = Some("You are a test.".into());
    worker.config.mcp_servers = vec![atelier_agents::McpServer {
        bearer_token: Some("secret".into()),
        ..atelier_agents::McpServer::new("live", "http://127.0.0.1:9/mcp")
    }];
    let events = events_until(&worker.config, "hello", turn_ended);
    assert!(
        matches!(events.last(), Some(AgentEvent::TurnCompleted { .. })),
        "{events:#?}"
    );
    let mcp = std::fs::read_to_string(scratch.join("mcp.json")).unwrap();
    assert!(mcp.contains("Bearer secret"));
    let prompt = std::fs::read_to_string(scratch.join("system-prompt.md")).unwrap();
    assert_eq!(prompt, "You are a test.");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |path: &std::path::Path| std::fs::metadata(path).unwrap().permissions().mode();
        assert_eq!(mode(&scratch.join("mcp.json")) & 0o777, 0o600);
        assert_eq!(mode(&scratch) & 0o777, 0o700);
    }
}
