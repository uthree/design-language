//! Codex, spoken to as `codex app-server` JSON-RPC over stdio.
//!
//! The handshake is `initialize`, the `initialized` notification, then `thread/start`; each user
//! message is a `turn/start` on that thread, and the turn ends with a `turn/completed`
//! notification. Command and file-change approvals arrive as requests from the server.

use std::collections::HashMap;

use crate::CommandSpec;
use crate::{AgentEvent, Effort, PermissionKind, PermissionRequest, ToolCall};
use serde_json::{Value, json};

use crate::rpc::{self, Calls};
use crate::{
    Adapter, AgentConfig, ModelInfo, Output, PARAGRAPH, SessionMode, content_text,
    with_instructions,
};

/// What a call that starts or drives another agent is reported as. Codex has no switch for them
/// in `thread/start`, so an agent that may not use them is noticed when it does.
pub(crate) const SUBAGENT_TOOLS: [&str; 1] = ["collabAgentToolCall"];

enum Purpose {
    Initialize,
    ListModels,
    StartThread,
    StartTurn,
    Other,
}

/// The settings that leave the user's own Codex setup out of an isolated run.
const ISOLATED: [&str; 11] = [
    "features.shell_snapshot=false",
    "features.hooks=false",
    "features.plugins=false",
    "features.apps=false",
    "features.multi_agent=false",
    "features.remote_plugin=false",
    "features.memories=false",
    "features.skip_host_skill_discovery=true",
    "features.skill_mcp_dependency_install=false",
    "approvals_reviewer=\"user\"",
    "project_doc_max_bytes=0",
];

/// The environment variable that carries the bearer token of the `index`th MCP server.
fn token_variable(index: usize) -> String {
    format!("ATELIER_MCP_TOKEN_{index}")
}

pub(crate) struct Codex {
    config: AgentConfig,
    calls: Calls<Purpose>,
    thread: Option<String>,
    turn: Option<String>,
    /// A prompt waiting for the thread to exist.
    queued: Option<String>,
    answer: String,
    /// Approval requests awaiting an answer, by the id the adapter gave them.
    asked: HashMap<String, Value>,
    first: bool,
}

impl Codex {
    pub(crate) fn new(config: AgentConfig) -> Self {
        Self {
            config,
            calls: Calls::new(false),
            thread: None,
            turn: None,
            queued: None,
            answer: String::new(),
            asked: HashMap::new(),
            first: true,
        }
    }

    fn turn_start(&mut self, thread: &str, text: &str) -> Value {
        self.answer.clear();
        let text = if std::mem::take(&mut self.first) {
            with_instructions(&self.config.instructions, text)
        } else {
            text.to_owned()
        };
        let effort = self.config.effort.map(|effort| match effort {
            Effort::Minimal => "minimal",
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::XHigh | Effort::Max => "xhigh",
        });
        self.calls.request(
            "turn/start",
            json!({"threadId": thread, "input": [{"type": "text", "text": text}], "effort": effort}),
            Purpose::StartTurn,
        )
    }

    fn response(&mut self, purpose: Purpose, message: &Value) -> Output {
        if let Some(error) = rpc::error(message) {
            return match purpose {
                // A CLI too old to list its models still runs.
                Purpose::ListModels => Output::default(),
                Purpose::StartThread if matches!(self.config.session, SessionMode::Resume(_)) => {
                    self.queued = None;
                    Output::event(AgentEvent::ResumeFailed {
                        message: format!("Codex: {error}"),
                    })
                }
                _ => Output::event(AgentEvent::TurnFailed {
                    message: format!("Codex: {error}"),
                }),
            };
        }
        let result = &message["result"];
        match purpose {
            Purpose::Initialize => {
                let config = &self.config;
                let mut params = json!({"model": config.model, "cwd": config.cwd,
                    "approvalPolicy": "on-request",
                    "sandbox": if config.read_only { "read-only" } else { "workspace-write" }});
                if let Some(prompt) = &config.system_prompt {
                    params["baseInstructions"] = json!(prompt);
                }
                let method = match &config.session {
                    SessionMode::Resume(thread) => {
                        params["threadId"] = json!(thread);
                        "thread/resume"
                    }
                    SessionMode::Ephemeral => {
                        params["ephemeral"] = json!(true);
                        "thread/start"
                    }
                    SessionMode::New => "thread/start",
                };
                let models = self.calls.request(
                    "model/list",
                    json!({"cursor": null, "limit": 100}),
                    Purpose::ListModels,
                );
                let thread = self.calls.request(method, params, Purpose::StartThread);
                Output {
                    events: Vec::new(),
                    replies: vec![self.calls.notify("initialized", json!({})), models, thread],
                }
            }
            Purpose::ListModels => {
                let models = result["data"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|model| {
                        Some(ModelInfo {
                            id: model["model"].as_str()?.to_owned(),
                            name: model["displayName"].as_str().map(str::to_owned),
                            efforts: model["supportedReasoningEfforts"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter_map(|effort| effort["reasoningEffort"].as_str())
                                .map(str::to_owned)
                                .collect(),
                        })
                    })
                    .collect();
                Output::event(AgentEvent::Models { models })
            }
            Purpose::StartThread => {
                let thread = result["thread"]["id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                self.thread = Some(thread.clone());
                let mut output = Output::event(AgentEvent::Started);
                if self.config.session != SessionMode::Ephemeral && !thread.is_empty() {
                    output
                        .events
                        .push(AgentEvent::SessionStarted { id: thread.clone() });
                }
                if let Some(prompt) = self.queued.take().filter(|prompt| !prompt.is_empty()) {
                    output.replies.push(self.turn_start(&thread, &prompt));
                }
                output
            }
            Purpose::StartTurn => {
                self.turn = result["turn"]["id"].as_str().map(str::to_owned);
                Output::default()
            }
            Purpose::Other => Output::default(),
        }
    }

    fn approval(&mut self, message: &Value) -> Output {
        let kind = match message["method"].as_str() {
            Some("item/commandExecution/requestApproval") => PermissionKind::Command,
            Some("item/fileChange/requestApproval") => PermissionKind::FileChange,
            _ => return Output::reply(self.calls.refuse(&message["id"])),
        };
        let params = &message["params"];
        let id = message["id"].to_string();
        let summary = match (kind, params["command"].as_str()) {
            (PermissionKind::Command, Some(command)) => format!("Run: {command}"),
            (PermissionKind::Command, None) => "Run a command".into(),
            _ => "Change files".into(),
        };
        self.asked.insert(id.clone(), message["id"].clone());
        Output::event(AgentEvent::PermissionRequested {
            request: PermissionRequest {
                id,
                kind,
                summary,
                detail: params.to_string(),
            },
        })
    }

    fn notification(&mut self, message: &Value) -> Output {
        let params = &message["params"];
        // Notifications from an earlier turn arrive late after an interrupt; ignore them.
        if let (Some(turn), Some(current)) = (params["turnId"].as_str(), self.turn.as_deref())
            && turn != current
        {
            return Output::default();
        }
        let method = message["method"].as_str().unwrap_or_default();
        let item = &params["item"];
        let event = match method {
            "item/agentMessage/delta" => {
                let text = params["delta"].as_str().unwrap_or_default().to_owned();
                self.answer.push_str(&text);
                AgentEvent::Text { text }
            }
            // Several messages in one turn, around tool calls, read as separate paragraphs.
            "item/started" if item["type"] == "agentMessage" && !self.answer.is_empty() => {
                self.answer.push_str(PARAGRAPH);
                AgentEvent::Text {
                    text: PARAGRAPH.into(),
                }
            }
            "item/reasoning/textDelta" | "item/reasoning/summaryTextDelta" => {
                AgentEvent::Reasoning {
                    text: params["delta"].as_str().unwrap_or_default().into(),
                }
            }
            "item/completed" if item["type"] == "agentMessage" && self.answer.is_empty() => {
                let text = item["text"].as_str().unwrap_or_default().to_owned();
                self.answer.push_str(&text);
                AgentEvent::Text { text }
            }
            "item/started" if is_tool(item) => AgentEvent::ToolStarted {
                call: ToolCall {
                    id: item["id"].as_str().unwrap_or_default().into(),
                    name: tool_name(item),
                    input: item
                        .get("command")
                        .or(item.get("arguments"))
                        .unwrap_or(item)
                        .to_string(),
                },
            },
            "item/completed" if is_tool(item) => AgentEvent::ToolFinished {
                id: item["id"].as_str().unwrap_or_default().into(),
                ok: item["status"] == "completed"
                    && item["exitCode"].as_i64().is_none_or(|code| code == 0)
                    && item["error"].is_null(),
                output: item["aggregatedOutput"]
                    .as_str()
                    .map_or_else(|| content_text(&item["result"]["content"]), str::to_owned),
            },
            "turn/completed" => {
                self.turn = None;
                self.asked.clear();
                let turn = &params["turn"];
                match turn["status"].as_str() {
                    Some("completed") => AgentEvent::TurnCompleted {
                        answer: std::mem::take(&mut self.answer),
                    },
                    Some("interrupted") => AgentEvent::TurnFailed {
                        message: "Codex turn interrupted".into(),
                    },
                    _ => AgentEvent::TurnFailed {
                        message: format!(
                            "Codex turn failed: {}",
                            turn["error"]["message"].as_str().unwrap_or("unknown error")
                        ),
                    },
                }
            }
            _ => return Output::default(),
        };
        Output::event(event)
    }
}

fn is_tool(item: &Value) -> bool {
    matches!(
        item["type"].as_str(),
        Some("commandExecution" | "fileChange" | "mcpToolCall" | "collabAgentToolCall")
    )
}

fn tool_name(item: &Value) -> String {
    match item["type"].as_str() {
        Some("mcpToolCall") => item["tool"].as_str().unwrap_or("mcpToolCall").into(),
        other => other.unwrap_or_default().into(),
    }
}

impl Adapter for Codex {
    fn command(&self) -> CommandSpec {
        let config = &self.config;
        let mut command = CommandSpec::new(config.program.as_deref().unwrap_or("codex"))
            .cwd(&config.cwd)
            .args(["app-server", "--listen", "stdio://"])
            .env_remove("CODEX_THREAD_ID")
            .env_remove("CODEX_SANDBOX");
        let mut settings: Vec<String> = Vec::new();
        if config.isolated {
            settings.extend(ISOLATED.map(str::to_owned));
        }
        if !config.native_tools {
            for feature in ["shell_tool", "unified_exec", "apply_patch_freeform"] {
                settings.push(format!("features.{feature}=false"));
            }
        }
        if let Some(fast) = config.fast {
            settings.push("features.fast_mode=true".into());
            let tier = if fast { "fast" } else { "default" };
            settings.push(format!("service_tier=\"{tier}\""));
        }
        for (index, server) in config.mcp_servers.iter().enumerate() {
            let key = format!("mcp_servers.{}", server.name);
            settings.push(format!("{key}.url={}", json!(server.url)));
            settings.push(format!("{key}.enabled=true"));
            if server.auto_approve {
                settings.push(format!("{key}.default_tools_approval_mode=\"approve\""));
            }
            if let Some(token) = &server.bearer_token {
                // The token travels in the environment, never in the arguments.
                let variable = token_variable(index);
                settings.push(format!("{key}.bearer_token_env_var=\"{variable}\""));
                command = command.env(variable, token);
            }
        }
        for setting in settings {
            command = command.args(["-c", &setting]);
        }
        command
    }

    fn start(&mut self, prompt: &str) -> Vec<Value> {
        self.queued = Some(prompt.to_owned());
        vec![self.calls.request(
            "initialize",
            json!({"clientInfo": {"name": self.config.client.name, "title": self.config.client.title, "version": self.config.client.version}}),
            Purpose::Initialize,
        )]
    }

    fn send(&mut self, text: &str) -> Vec<Value> {
        match self.thread.clone() {
            Some(thread) => vec![self.turn_start(&thread, text)],
            None => {
                self.queued = Some(text.to_owned());
                Vec::new()
            }
        }
    }

    fn receive(&mut self, message: Value) -> Output {
        if rpc::is_request(&message) {
            return self.approval(&message);
        }
        if let Some(purpose) = self.calls.resolve(&message) {
            return self.response(purpose, &message);
        }
        self.notification(&message)
    }

    fn answer(&mut self, request: &str, allow: bool) -> Vec<Value> {
        let Some(id) = self.asked.remove(request) else {
            return Vec::new();
        };
        vec![self.calls.respond(
            &id,
            json!({"decision": if allow { "accept" } else { "decline" }}),
        )]
    }

    fn interrupt(&mut self) -> Vec<Value> {
        match (self.thread.clone(), self.turn.clone()) {
            (Some(thread), Some(turn)) => vec![self.calls.request(
                "turn/interrupt",
                json!({"threadId": thread, "turnId": turn}),
                Purpose::Other,
            )],
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProviderKind;
    use crate::config;

    #[test]
    fn handshake_then_prompt_then_turn() {
        let mut codex = Codex::new(config(ProviderKind::Codex));
        let init = &codex.start("Fix it")[0];
        assert_eq!(init["method"], "initialize");
        assert!(init.get("jsonrpc").is_none());

        let output = codex.receive(json!({"id": init["id"], "result": {}}));
        assert_eq!(output.replies[0]["method"], "initialized");
        assert_eq!(output.replies[1]["method"], "model/list");
        let thread = &output.replies[2];
        assert_eq!(thread["method"], "thread/start");
        assert_eq!(thread["params"]["sandbox"], "workspace-write");
        assert_eq!(thread["params"]["model"], "model-x");

        let output = codex.receive(json!({"id": thread["id"], "result": {"thread": {"id": "th"}}}));
        assert_eq!(
            output.events,
            [
                AgentEvent::Started,
                AgentEvent::SessionStarted { id: "th".into() }
            ]
        );
        let turn = &output.replies[0];
        assert_eq!(turn["params"]["threadId"], "th");
        assert_eq!(turn["params"]["effort"], "high");
        let text = turn["params"]["input"][0]["text"].as_str().unwrap();
        assert!(text.starts_with("Only touch src/.") && text.ends_with("Fix it"));

        codex.receive(json!({"id": turn["id"], "result": {"turn": {"id": "t1"}}}));
        let events: Vec<_> = [
            json!({"method":"item/started","params":{"turnId":"t1","item":{"id":"m1","type":"agentMessage"}}}),
            json!({"method":"item/agentMessage/delta","params":{"turnId":"t1","delta":"Hi"}}),
            json!({"method":"item/agentMessage/delta","params":{"turnId":"old","delta":"stale"}}),
            json!({"method":"item/started","params":{"turnId":"t1","item":{"id":"c1","type":"commandExecution","command":"ls"}}}),
            json!({"method":"item/completed","params":{"turnId":"t1","item":{"id":"c1","type":"commandExecution","status":"completed","exitCode":1,"aggregatedOutput":"no"}}}),
            json!({"method":"item/started","params":{"turnId":"t1","item":{"id":"m2","type":"agentMessage"}}}),
            json!({"method":"item/agentMessage/delta","params":{"turnId":"t1","delta":"Bye"}}),
            json!({"method":"turn/completed","params":{"turn":{"id":"t1","status":"completed"}}}),
        ]
        .into_iter()
        .flat_map(|line| codex.receive(line).events)
        .collect();
        assert_eq!(events[0], AgentEvent::Text { text: "Hi".into() });
        assert!(matches!(&events[1], AgentEvent::ToolStarted { call } if call.input == r#""ls""#));
        assert!(matches!(
            &events[2],
            AgentEvent::ToolFinished { ok: false, .. }
        ));
        // The second message is a new paragraph, in the stream and in the answer.
        assert_eq!(
            events[3],
            AgentEvent::Text {
                text: "\n\n".into()
            }
        );
        assert_eq!(events[4], AgentEvent::Text { text: "Bye".into() });
        assert_eq!(
            events[5],
            AgentEvent::TurnCompleted {
                answer: "Hi\n\nBye".into()
            }
        );

        // The next message goes straight to the existing thread, without instructions again.
        let next = &codex.send("Again")[0];
        assert_eq!(next["params"]["input"][0]["text"], "Again");
    }

    #[test]
    fn approvals_are_answered_by_the_server_request_id() {
        let mut codex = Codex::new(config(ProviderKind::Codex));
        let output = codex.receive(
            json!({"id": 900, "method": "item/commandExecution/requestApproval",
            "params": {"command": "cargo test"}}),
        );
        let [AgentEvent::PermissionRequested { request }] = &output.events[..] else {
            panic!()
        };
        assert_eq!(request.summary, "Run: cargo test");
        let reply = &codex.answer(&request.id, false)[0];
        assert_eq!(
            reply,
            &json!({"id": 900, "result": {"decision": "decline"}})
        );

        let other =
            codex.receive(json!({"id": 901, "method": "account/chatgptAuthTokens/refresh"}));
        assert_eq!(other.replies[0]["error"]["code"], -32601);
    }

    #[test]
    fn mcp_tokens_travel_in_the_environment() {
        let command = Codex::new(AgentConfig {
            mcp_servers: vec![crate::McpServer {
                bearer_token: Some("secret".into()),
                auto_approve: true,
                ..crate::McpServer::new("live", "http://127.0.0.1:9/mcp")
            }],
            fast: Some(false),
            native_tools: false,
            isolated: true,
            ..config(ProviderKind::Codex)
        })
        .command();
        assert!(!command.args.iter().any(|arg| arg.contains("secret")));
        assert_eq!(
            command.env.get("ATELIER_MCP_TOKEN_0").map(String::as_str),
            Some("secret")
        );
        for setting in [
            r#"mcp_servers.live.url="http://127.0.0.1:9/mcp""#,
            r#"mcp_servers.live.bearer_token_env_var="ATELIER_MCP_TOKEN_0""#,
            r#"mcp_servers.live.default_tools_approval_mode="approve""#,
            r#"service_tier="default""#,
            "features.shell_tool=false",
            "features.hooks=false",
        ] {
            assert!(
                command.args.windows(2).any(|pair| pair == ["-c", setting]),
                "{setting}: {:?}",
                command.args
            );
        }
        let plain = Codex::new(config(ProviderKind::Codex)).command();
        assert!(!plain.args.iter().any(|arg| arg == "-c"));
    }

    #[test]
    fn a_thread_is_resumed_or_reported_lost() {
        let mut codex = Codex::new(AgentConfig {
            session: SessionMode::Resume("th".into()),
            system_prompt: Some("Be brief.".into()),
            ..config(ProviderKind::Codex)
        });
        let init = codex.start("Again").remove(0);
        let output = codex.receive(json!({"id": init["id"], "result": {}}));
        let resume = &output.replies[2];
        assert_eq!(resume["method"], "thread/resume");
        assert_eq!(resume["params"]["threadId"], "th");
        assert_eq!(resume["params"]["baseInstructions"], "Be brief.");
        let lost = codex.receive(json!({"id": resume["id"], "error": {"message": "no rollout"}}));
        assert!(lost.replies.is_empty());
        assert!(matches!(
            &lost.events[..],
            [AgentEvent::ResumeFailed { .. }]
        ));
    }
}
