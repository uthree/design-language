//! Claude Code, spoken to in `stream-json` over stdio.
//!
//! One process serves the whole conversation: each user message is a line on stdin, and the turn
//! ends with a `result` line. Tool permissions arrive as `control_request`s (because of
//! `--permission-prompt-tool stdio`) and are answered with `control_response`s.
//!
//! The adapter opens with an `initialize` control request, whose response lists the models the
//! CLI offers, and sends the first prompt once it is answered. A `--resume` of a session the CLI
//! no longer has fails there, before any prompt is sent.

use std::collections::HashMap;

use crate::CommandSpec;
use crate::{AgentEvent, Effort, PermissionKind, PermissionRequest, RateLimit, ToolCall};
use serde_json::{Value, json};

use crate::{
    Adapter, AgentConfig, ModelInfo, Output, PARAGRAPH, PreparedFile, SessionMode, content_text,
};

/// The id of the opening `initialize` control request.
const INITIALIZE: &str = "atelier-initialize";

/// The tools that start a sub-agent: `Task`, called `Agent` in later versions.
pub(crate) const SUBAGENT_TOOLS: [&str; 2] = ["Task", "Agent"];

pub(crate) struct Claude {
    config: AgentConfig,
    answer: String,
    /// Whether this turn's text already arrived as deltas, so the full message is not repeated.
    streamed: bool,
    /// Inputs of permission requests awaiting an answer; Claude Code wants them echoed back.
    asked: HashMap<String, Value>,
    /// The first prompt, held until `initialize` is answered.
    queued: Option<String>,
    /// Whether [`AgentEvent::SessionStarted`] was reported.
    reported: bool,
}

impl Claude {
    pub(crate) fn new(config: AgentConfig) -> Self {
        Self {
            config,
            answer: String::new(),
            streamed: false,
            asked: HashMap::new(),
            queued: None,
            reported: false,
        }
    }

    /// The MCP configuration Claude Code reads with `--mcp-config`, or `None` when it needs
    /// none.
    fn mcp_config(&self) -> Option<String> {
        let config = &self.config;
        if config.mcp_servers.is_empty() && !config.isolated {
            return None;
        }
        let servers: serde_json::Map<String, Value> = config
            .mcp_servers
            .iter()
            .map(|server| {
                let mut entry = json!({"type": "http", "url": server.url});
                if let Some(token) = &server.bearer_token {
                    entry["headers"] = json!({"Authorization": format!("Bearer {token}")});
                }
                (server.name.clone(), entry)
            })
            .collect();
        Some(json!({"mcpServers": servers}).to_string())
    }

    /// The `--settings` Claude Code is started with, or `None` when it needs none.
    fn settings(&self) -> Option<String> {
        let config = &self.config;
        let mut settings = serde_json::Map::new();
        if config.isolated {
            settings.insert("disableAllHooks".into(), json!(true));
            settings.insert("autoMemoryEnabled".into(), json!(false));
            settings.insert("enabledPlugins".into(), json!({}));
        }
        if let Some(fast) = config.fast {
            settings.insert("fastMode".into(), json!(fast));
        }
        let allow: Vec<String> = config
            .mcp_servers
            .iter()
            .filter(|server| server.auto_approve)
            .map(|server| format!("mcp__{}__*", server.name))
            .collect();
        if !allow.is_empty() {
            settings.insert("permissions".into(), json!({"allow": allow}));
        }
        (!settings.is_empty()).then(|| Value::Object(settings).to_string())
    }

    fn initialized(&mut self, response: &Value) -> Output {
        let mut output = Output::default();
        if response["subtype"] != "success"
            && let SessionMode::Resume(_) = self.config.session
        {
            self.queued = None;
            let message = response["error"]
                .as_str()
                .unwrap_or("Claude Code could not resume the session")
                .to_owned();
            return Output::event(AgentEvent::ResumeFailed { message });
        }
        let models: Vec<ModelInfo> = response["response"]["models"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|model| {
                Some(ModelInfo {
                    id: model["value"].as_str()?.to_owned(),
                    name: model["displayName"].as_str().map(str::to_owned),
                    efforts: Vec::new(),
                })
            })
            .collect();
        if !models.is_empty() {
            output.events.push(AgentEvent::Models { models });
        }
        if let Some(prompt) = self.queued.take().filter(|prompt| !prompt.is_empty()) {
            output.replies.extend(self.send(&prompt));
        }
        output
    }

    fn user(text: &str) -> Value {
        json!({"type": "user", "message": {"role": "user", "content": text}})
    }

    fn control(&mut self, message: &Value) -> Output {
        let request = &message["request"];
        let id = message["request_id"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        if request["subtype"] != "can_use_tool" {
            return Output::reply(json!({"type": "control_response", "response": {
                "subtype": "error", "request_id": id, "error": "not supported by this application"}}));
        }
        let tool = request["tool_name"].as_str().unwrap_or_default();
        let kind = match tool {
            "Bash" => PermissionKind::Command,
            "Write" | "Edit" | "MultiEdit" | "NotebookEdit" => PermissionKind::FileChange,
            _ => PermissionKind::Other,
        };
        let input = &request["input"];
        let summary = match input["command"]
            .as_str()
            .or_else(|| input["file_path"].as_str())
        {
            Some(target) => format!("{tool}: {target}"),
            None => tool.to_owned(),
        };
        self.asked.insert(id.clone(), input.clone());
        Output::event(AgentEvent::PermissionRequested {
            request: PermissionRequest {
                id,
                kind,
                summary,
                detail: request.to_string(),
            },
        })
    }
}

impl Adapter for Claude {
    fn command(&self) -> CommandSpec {
        let config = &self.config;
        let mut command = CommandSpec::new(config.program.as_deref().unwrap_or("claude"))
            .cwd(&config.cwd)
            .args([
                "--print",
                "--input-format",
                "stream-json",
                "--output-format",
                "stream-json",
                "--verbose",
                "--include-partial-messages",
                "--permission-mode",
                "default",
                "--permission-prompt-tool",
                "stdio",
            ])
            // An agent must not inherit the runtime of an agent that started the application.
            .env_remove("CLAUDECODE")
            .env_remove("CLAUDE_CODE_ENTRYPOINT");
        let mut refused: Vec<&str> = Vec::new();
        if config.read_only {
            refused.extend(["Write", "Edit", "MultiEdit", "NotebookEdit"]);
        }
        if !config.subagents {
            refused.extend(SUBAGENT_TOOLS);
        }
        if !refused.is_empty() {
            command = command.args(["--disallowedTools", &refused.join(",")]);
        }
        if !config.instructions.is_empty() {
            command = command.args(["--append-system-prompt", &config.instructions]);
        }
        if let Some(prompt) = &config.system_prompt {
            command = match &config.scratch {
                Some(scratch) => command
                    .arg("--system-prompt-file")
                    .arg(scratch.join("system-prompt.md").to_string_lossy()),
                None => command.args(["--system-prompt", prompt]),
            };
        }
        if config.isolated {
            command = command.args([
                "--setting-sources=",
                "--strict-mcp-config",
                "--disable-slash-commands",
                "--no-chrome",
            ]);
        }
        if let Some(mcp) = self.mcp_config() {
            command = match &config.scratch {
                Some(scratch) => command
                    .arg("--mcp-config")
                    .arg(scratch.join("mcp.json").to_string_lossy()),
                None => command.args(["--mcp-config", &mcp]),
            };
        }
        if let Some(settings) = self.settings() {
            command = command.args(["--settings", &settings]);
        }
        if !config.native_tools {
            command = command.args(["--tools", ""]);
        }
        command = match &config.session {
            SessionMode::New => command,
            SessionMode::Ephemeral => command.arg("--no-session-persistence"),
            SessionMode::Resume(id) => command.args(["--resume", id]),
        };
        if let Some(model) = &config.model {
            command = command.arg(format!("--model={model}"));
        }
        if let Some(effort) = config.effort {
            let effort = match effort {
                Effort::Minimal | Effort::Low => "low",
                Effort::Medium => "medium",
                Effort::High => "high",
                Effort::XHigh => "xhigh",
                Effort::Max => "max",
            };
            command = command.args(["--effort", effort]);
        }
        command
    }

    fn files(&self) -> Vec<PreparedFile> {
        let Some(scratch) = &self.config.scratch else {
            return Vec::new();
        };
        let mut files = Vec::new();
        if let Some(mcp) = self.mcp_config() {
            files.push(PreparedFile {
                path: scratch.join("mcp.json"),
                contents: mcp,
            });
        }
        if let Some(prompt) = &self.config.system_prompt {
            files.push(PreparedFile {
                path: scratch.join("system-prompt.md"),
                contents: prompt.clone(),
            });
        }
        files
    }

    fn start(&mut self, prompt: &str) -> Vec<Value> {
        self.queued = Some(prompt.to_owned());
        vec![json!({"type": "control_request", "request_id": INITIALIZE,
            "request": {"subtype": "initialize", "hooks": {}, "agents": {}, "skills": []}})]
    }

    fn send(&mut self, text: &str) -> Vec<Value> {
        self.answer.clear();
        self.streamed = false;
        vec![Self::user(text)]
    }

    fn receive(&mut self, message: Value) -> Output {
        if message["type"] == "control_request" {
            return self.control(&message);
        }
        if message["type"] == "control_response" && message["response"]["request_id"] == INITIALIZE
        {
            return self.initialized(&message["response"]);
        }
        // Messages from Claude Code's own subagents belong to their parent tool call.
        if !message["parent_tool_use_id"].is_null() {
            return Output::default();
        }
        let mut output = Output::default();
        match message["type"].as_str().unwrap_or_default() {
            "system" if message["subtype"] == "init" => {
                output.events.push(AgentEvent::Started);
                let id = message["session_id"].as_str().filter(|id| !id.is_empty());
                if let Some(id) = id
                    && self.config.session != SessionMode::Ephemeral
                    && !std::mem::replace(&mut self.reported, true)
                {
                    output
                        .events
                        .push(AgentEvent::SessionStarted { id: id.to_owned() });
                }
            }
            "stream_event" => {
                let delta = &message["event"]["delta"];
                match delta["type"].as_str().unwrap_or_default() {
                    "text_delta" => {
                        // A new message after a tool call starts a new paragraph.
                        if !self.streamed && !self.answer.is_empty() {
                            output.events.push(AgentEvent::Text {
                                text: PARAGRAPH.into(),
                            });
                        }
                        self.streamed = true;
                        let text = delta["text"].as_str().unwrap_or_default().to_owned();
                        output.events.push(AgentEvent::Text { text });
                    }
                    "thinking_delta" => {
                        let text = delta["thinking"].as_str().unwrap_or_default().to_owned();
                        output.events.push(AgentEvent::Reasoning { text });
                    }
                    _ => {}
                }
            }
            "assistant" => {
                for part in message["message"]["content"]
                    .as_array()
                    .into_iter()
                    .flatten()
                {
                    match part["type"].as_str().unwrap_or_default() {
                        "text" => {
                            let mut text = part["text"].as_str().unwrap_or_default().to_owned();
                            if !self.answer.is_empty() {
                                text.insert_str(0, PARAGRAPH);
                            }
                            self.answer.push_str(&text);
                            if !self.streamed {
                                output.events.push(AgentEvent::Text { text });
                            }
                        }
                        "tool_use" => output.events.push(AgentEvent::ToolStarted {
                            call: ToolCall {
                                id: part["id"].as_str().unwrap_or_default().to_owned(),
                                name: part["name"].as_str().unwrap_or_default().to_owned(),
                                input: part["input"].to_string(),
                            },
                        }),
                        _ => {}
                    }
                }
                self.streamed = false;
            }
            "user" => {
                for part in message["message"]["content"]
                    .as_array()
                    .into_iter()
                    .flatten()
                {
                    if part["type"] == "tool_result" {
                        output.events.push(AgentEvent::ToolFinished {
                            id: part["tool_use_id"].as_str().unwrap_or_default().to_owned(),
                            ok: part["is_error"] != true,
                            output: content_text(&part["content"]),
                        });
                    }
                }
            }
            "rate_limit_event" => {
                let limits = rate_limits(&message["rate_limit_info"]);
                if !limits.is_empty() {
                    output.events.push(AgentEvent::RateLimits { limits });
                }
            }
            "result" => {
                self.asked.clear();
                if message["is_error"] == true || message["subtype"] != "success" {
                    let message = message["result"]
                        .as_str()
                        .or_else(|| message["subtype"].as_str())
                        .unwrap_or("Claude Code turn failed")
                        .to_owned();
                    output.events.push(AgentEvent::TurnFailed { message });
                } else {
                    let mut answer = std::mem::take(&mut self.answer);
                    if answer.is_empty() {
                        answer = message["result"].as_str().unwrap_or_default().to_owned();
                    }
                    output.events.push(AgentEvent::TurnCompleted { answer });
                }
            }
            _ => {}
        }
        output
    }

    fn answer(&mut self, request: &str, allow: bool) -> Vec<Value> {
        let Some(input) = self.asked.remove(request) else {
            return Vec::new();
        };
        let response = if allow {
            json!({"behavior": "allow", "updatedInput": input})
        } else {
            json!({"behavior": "deny", "message": "The user refused this action."})
        };
        vec![json!({"type": "control_response", "response": {
            "subtype": "success", "request_id": request, "response": response}})]
    }

    fn interrupt(&mut self) -> Vec<Value> {
        vec![
            json!({"type": "control_request", "request_id": "atelier-interrupt",
            "request": {"subtype": "interrupt"}}),
        ]
    }
}

/// The plan limits in a `rate_limit_event`'s `rate_limit_info`: every window it tracks, and the
/// limiting one it names at the top when that is a model's own week. Overage is spending past
/// the plan, not a limit of it, so it is left out.
fn rate_limits(info: &Value) -> Vec<RateLimit> {
    fn limit(id: &str, used: &Value, resets: &Value, status: &Value) -> Option<RateLimit> {
        let (window_minutes, scope) = match id {
            "five_hour" => (300, None),
            "seven_day" => (10_080, None),
            "seven_day_opus" => (10_080, Some("Opus")),
            "seven_day_sonnet" => (10_080, Some("Sonnet")),
            _ => return None,
        };
        let reached = status == "rejected";
        let used_percent = match used.as_f64() {
            Some(used) => (used * 100.).round().max(0.) as u32,
            None if reached => 100,
            None => return None,
        };
        Some(RateLimit {
            id: id.into(),
            window_minutes: Some(window_minutes),
            scope: scope.map(str::to_owned),
            used_percent,
            resets_at: resets.as_u64(),
            reached,
        })
    }
    let mut limits: Vec<RateLimit> = info["unifiedWindows"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(id, window)| {
            limit(
                id,
                &window["utilization"],
                &window["resetsAt"],
                &window["status"],
            )
        })
        .collect();
    if let Some(id) = info["rateLimitType"].as_str()
        && !limits.iter().any(|known| known.id == id)
        && let Some(top) = limit(id, &info["utilization"], &info["resetsAt"], &info["status"])
    {
        limits.push(top);
    }
    limits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProviderKind;
    use crate::config;

    #[test]
    fn command_streams_json_and_routes_permissions_through_stdio() {
        let command = Claude::new(config(ProviderKind::ClaudeCode)).command();
        assert_eq!(command.program, "claude");
        let args = &command.args;
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--permission-prompt-tool", "stdio"])
        );
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--append-system-prompt", "Only touch src/."])
        );
        assert!(args.windows(2).any(|pair| pair == ["--effort", "high"]));
        assert!(args.iter().any(|arg| arg == "--model=model-x"));
        assert_eq!(
            command.cwd.as_deref(),
            Some(std::path::Path::new("/work/n1"))
        );
        assert!(!args.iter().any(|arg| arg == "--disallowedTools"));
    }

    #[test]
    fn refused_tools_are_listed_once_together() {
        let refused = |read_only, subagents| {
            let config = AgentConfig {
                read_only,
                subagents,
                ..config(ProviderKind::ClaudeCode)
            };
            let args = Claude::new(config).command().args;
            let at = args.iter().position(|arg| arg == "--disallowedTools")?;
            assert_eq!(
                args.iter()
                    .filter(|arg| *arg == "--disallowedTools")
                    .count(),
                1
            );
            args.get(at + 1).cloned()
        };
        assert_eq!(refused(false, false).as_deref(), Some("Task,Agent"));
        assert_eq!(
            refused(true, false).as_deref(),
            Some("Write,Edit,MultiEdit,NotebookEdit,Task,Agent")
        );
        assert_eq!(
            refused(true, true).as_deref(),
            Some("Write,Edit,MultiEdit,NotebookEdit")
        );
        assert_eq!(refused(false, true), None);
    }

    #[test]
    fn a_turn_streams_text_once_and_reports_tools() {
        let mut claude = Claude::new(config(ProviderKind::ClaudeCode));
        let opening = claude.start("Fix it");
        assert_eq!(opening[0]["request"]["subtype"], "initialize");
        let ready = claude.receive(json!({"type":"control_response","response":{
            "subtype":"success","request_id":INITIALIZE,
            "response":{"models":[{"value":"opus","displayName":"Opus"}]}}}));
        assert_eq!(ready.replies[0]["message"]["content"], "Fix it");
        assert_eq!(
            ready.events,
            [AgentEvent::Models {
                models: vec![ModelInfo {
                    id: "opus".into(),
                    name: Some("Opus".into()),
                    efforts: Vec::new()
                }]
            }]
        );
        let lines = [
            json!({"type":"system","subtype":"init"}),
            json!({"type":"stream_event","event":{"delta":{"type":"thinking_delta","thinking":"Hmm"}}}),
            json!({"type":"stream_event","event":{"delta":{"type":"text_delta","text":"Done."}}}),
            json!({"type":"assistant","message":{"content":[{"type":"text","text":"Done."},
                {"type":"tool_use","id":"t1","name":"Bash","input":{"command":"ls"}}]}}),
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":[{"type":"text","text":"a.txt"}]}]}}),
            json!({"type":"assistant","parent_tool_use_id":"t1","message":{"content":[{"type":"text","text":"sub"}]}}),
            json!({"type":"stream_event","event":{"delta":{"type":"text_delta","text":"All set."}}}),
            json!({"type":"assistant","message":{"content":[{"type":"text","text":"All set."}]}}),
            json!({"type":"result","subtype":"success","is_error":false,"result":"All set."}),
        ];
        let events: Vec<_> = lines
            .into_iter()
            .flat_map(|line| claude.receive(line).events)
            .collect();
        assert_eq!(
            events,
            [
                AgentEvent::Started,
                AgentEvent::Reasoning { text: "Hmm".into() },
                AgentEvent::Text {
                    text: "Done.".into()
                },
                AgentEvent::ToolStarted {
                    call: ToolCall {
                        id: "t1".into(),
                        name: "Bash".into(),
                        input: r#"{"command":"ls"}"#.into()
                    }
                },
                AgentEvent::ToolFinished {
                    id: "t1".into(),
                    ok: true,
                    output: "a.txt".into()
                },
                AgentEvent::Text {
                    text: "\n\n".into()
                },
                AgentEvent::Text {
                    text: "All set.".into()
                },
                AgentEvent::TurnCompleted {
                    answer: "Done.\n\nAll set.".into()
                },
            ]
        );
    }

    #[test]
    fn permissions_are_asked_and_answered_with_the_original_input() {
        let mut claude = Claude::new(config(ProviderKind::ClaudeCode));
        let output = claude.receive(json!({"type":"control_request","request_id":"r1",
            "request":{"subtype":"can_use_tool","tool_name":"Bash","input":{"command":"rm -rf build"}}}));
        let [AgentEvent::PermissionRequested { request }] = &output.events[..] else {
            panic!()
        };
        assert_eq!(
            (request.kind, request.summary.as_str()),
            (PermissionKind::Command, "Bash: rm -rf build")
        );

        let reply = &claude.answer("r1", true)[0];
        assert_eq!(reply["response"]["request_id"], "r1");
        assert_eq!(
            reply["response"]["response"]["updatedInput"]["command"],
            "rm -rf build"
        );
        assert!(
            claude.answer("r1", true).is_empty(),
            "an answer is sent once"
        );

        let other = claude.receive(json!({"type":"control_request","request_id":"r2","request":{"subtype":"hook_callback"}}));
        assert_eq!(other.replies[0]["response"]["subtype"], "error");
    }

    #[test]
    fn a_failed_result_fails_the_turn() {
        let mut claude = Claude::new(config(ProviderKind::ClaudeCode));
        let output =
            claude.receive(json!({"type":"result","subtype":"error_max_turns","is_error":true}));
        assert_eq!(
            output.events,
            [AgentEvent::TurnFailed {
                message: "error_max_turns".into()
            }]
        );
    }

    fn server(auto_approve: bool) -> crate::McpServer {
        crate::McpServer {
            bearer_token: Some("secret".into()),
            auto_approve,
            ..crate::McpServer::new("live", "http://127.0.0.1:9/mcp")
        }
    }

    #[test]
    fn mcp_servers_and_the_system_prompt_go_into_private_files() {
        let claude = Claude::new(AgentConfig {
            mcp_servers: vec![server(true)],
            system_prompt: Some("You are a test.".into()),
            scratch: Some("/private/run".into()),
            ..config(ProviderKind::ClaudeCode)
        });
        let args = claude.command().args;
        assert!(!args.iter().any(|arg| arg.contains("secret")), "{args:?}");
        assert!(args.windows(2).any(|pair| pair[0] == "--mcp-config"
            && std::path::Path::new(&pair[1]) == std::path::Path::new("/private/run/mcp.json")));
        assert!(args.iter().any(|arg| arg == "--system-prompt-file"));
        let settings = args
            .windows(2)
            .find(|pair| pair[0] == "--settings")
            .unwrap();
        let settings: Value = serde_json::from_str(&settings[1]).unwrap();
        assert_eq!(settings["permissions"]["allow"], json!(["mcp__live__*"]));
        let files = claude.files();
        assert_eq!(files.len(), 2);
        let mcp: Value = serde_json::from_str(&files[0].contents).unwrap();
        assert_eq!(
            mcp["mcpServers"]["live"]["headers"]["Authorization"],
            "Bearer secret"
        );
        assert_eq!(files[1].contents, "You are a test.");
    }

    #[test]
    fn without_scratch_the_configuration_rides_in_the_arguments() {
        let claude = Claude::new(AgentConfig {
            mcp_servers: vec![server(false)],
            system_prompt: Some("You are a test.".into()),
            ..config(ProviderKind::ClaudeCode)
        });
        let args = claude.command().args;
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--system-prompt", "You are a test."])
        );
        let mcp = args
            .windows(2)
            .find(|pair| pair[0] == "--mcp-config")
            .unwrap();
        assert!(mcp[1].contains("http://127.0.0.1:9/mcp"));
        assert!(!args.iter().any(|arg| arg == "--settings"));
        assert!(claude.files().is_empty());
    }

    #[test]
    fn isolation_tools_speed_and_sessions_become_flags() {
        let args = |config: AgentConfig| Claude::new(config).command().args;
        let isolated = args(AgentConfig {
            isolated: true,
            native_tools: false,
            fast: Some(true),
            session: SessionMode::Ephemeral,
            ..config(ProviderKind::ClaudeCode)
        });
        for flag in [
            "--setting-sources=",
            "--strict-mcp-config",
            "--disable-slash-commands",
            "--no-session-persistence",
        ] {
            assert!(isolated.iter().any(|arg| arg == flag), "{flag}");
        }
        assert!(isolated.windows(2).any(|pair| pair == ["--tools", ""]));
        let settings = isolated
            .windows(2)
            .find(|pair| pair[0] == "--settings")
            .unwrap();
        let settings: Value = serde_json::from_str(&settings[1]).unwrap();
        assert_eq!(settings["fastMode"], true);
        assert_eq!(settings["disableAllHooks"], true);
        // An isolated run reads no MCP servers but the ones it is given: none.
        let mcp = isolated
            .windows(2)
            .find(|pair| pair[0] == "--mcp-config")
            .unwrap();
        assert_eq!(mcp[1], r#"{"mcpServers":{}}"#);

        let resumed = args(AgentConfig {
            session: SessionMode::Resume("abc".into()),
            ..config(ProviderKind::ClaudeCode)
        });
        assert!(resumed.windows(2).any(|pair| pair == ["--resume", "abc"]));
        let plain = args(config(ProviderKind::ClaudeCode));
        for flag in ["--settings", "--mcp-config", "--tools", "--resume"] {
            assert!(!plain.iter().any(|arg| arg == flag), "{flag}");
        }
    }

    #[test]
    fn a_session_is_reported_once_and_a_failed_resume_sends_no_prompt() {
        let mut claude = Claude::new(config(ProviderKind::ClaudeCode));
        claude.start("hi");
        claude.receive(json!({"type":"control_response","response":{"subtype":"success","request_id":INITIALIZE}}));
        let init = json!({"type":"system","subtype":"init","session_id":"s1"});
        assert_eq!(
            claude.receive(init.clone()).events,
            [
                AgentEvent::Started,
                AgentEvent::SessionStarted { id: "s1".into() }
            ]
        );
        assert_eq!(claude.receive(init).events, [AgentEvent::Started]);

        let mut claude = Claude::new(AgentConfig {
            session: SessionMode::Resume("gone".into()),
            ..config(ProviderKind::ClaudeCode)
        });
        claude.start("hi");
        let output = claude.receive(json!({"type":"control_response","response":{
            "subtype":"error","request_id":INITIALIZE,"error":"No conversation found"}}));
        assert!(output.replies.is_empty());
        assert_eq!(
            output.events,
            [AgentEvent::ResumeFailed {
                message: "No conversation found".into()
            }]
        );
    }

    #[test]
    fn rate_limit_events_report_every_window_of_the_plan() {
        let mut claude = Claude::new(config(ProviderKind::ClaudeCode));
        let output = claude.receive(json!({"type":"rate_limit_event","rate_limit_info":{
            "status":"allowed_warning","rateLimitType":"seven_day_opus","utilization":0.91,
            "resetsAt":1_800_000_000u64,
            "unifiedWindows":{
                "five_hour":{"utilization":0.424,"resetsAt":1_790_000_000u64,"status":"allowed"},
                "seven_day":{"utilization":1.0,"resetsAt":1_800_000_000u64,"status":"rejected"},
                "seven_day_overage_included":{"utilization":0.1,"resetsAt":1_800_000_000u64}}}}));
        let limits = |id: &str, minutes, scope: Option<&str>, used, resets, reached| RateLimit {
            id: id.into(),
            window_minutes: Some(minutes),
            scope: scope.map(Into::into),
            used_percent: used,
            resets_at: Some(resets),
            reached,
        };
        assert_eq!(
            output.events,
            [AgentEvent::RateLimits {
                limits: vec![
                    limits("five_hour", 300, None, 42, 1_790_000_000, false),
                    limits("seven_day", 10_080, None, 100, 1_800_000_000, true),
                    limits(
                        "seven_day_opus",
                        10_080,
                        Some("Opus"),
                        91,
                        1_800_000_000,
                        false
                    ),
                ]
            }]
        );
        // An API key reports a status and nothing to measure.
        let none = claude
            .receive(json!({"type":"rate_limit_event","rate_limit_info":{"status":"allowed"}}));
        assert!(none.events.is_empty());
    }
}
