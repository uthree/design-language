//! OpenCode, spoken to in the Agent Client Protocol (`opencode acp`), JSON-RPC 2.0 over stdio.
//!
//! The handshake is `initialize` then `session/new`, plus `session/set_model` when a model is
//! chosen. Each user message is a `session/prompt` request whose response ends the turn; the
//! reply streams meanwhile as `session/update` notifications. Permission prompts arrive as
//! `session/request_permission` requests offering options, of which the adapter only ever picks a
//! one-time allow or reject, never an "always".

use std::collections::HashMap;

use crate::CommandSpec;
use crate::{AgentEvent, PermissionKind, PermissionRequest, ToolCall};
use serde_json::{Value, json};

use crate::rpc::{self, Calls};
use crate::{
    Adapter, AgentConfig, ModelInfo, Output, PARAGRAPH, SessionMode, content_text, preamble,
    with_instructions,
};

/// The tool that starts a sub-agent, which the `task` permission governs.
pub(crate) const SUBAGENT_TOOLS: [&str; 1] = ["task"];

enum Purpose {
    Initialize,
    NewSession,
    LoadSession,
    SetModel,
    Prompt,
}

pub(crate) struct OpenCode {
    config: AgentConfig,
    calls: Calls<Purpose>,
    session: Option<String>,
    queued: Option<String>,
    answer: String,
    asked: HashMap<String, (Value, Value)>,
    /// Tool calls already reported as started, so updates do not start them twice.
    started: Vec<String>,
    /// A tool call came after this turn's text, so the next text starts a new message.
    parted: bool,
    first: bool,
    /// A `session/load` is replaying the old conversation, which is not reported again.
    loading: bool,
}

impl OpenCode {
    pub(crate) fn new(config: AgentConfig) -> Self {
        Self {
            config,
            calls: Calls::new(true),
            session: None,
            queued: None,
            answer: String::new(),
            asked: HashMap::new(),
            started: Vec::new(),
            parted: false,
            first: true,
            loading: false,
        }
    }

    /// The configured MCP servers, as ACP's `session/new` and `session/load` take them.
    fn mcp_servers(&self) -> Value {
        self.config
            .mcp_servers
            .iter()
            .map(|server| {
                let headers: Vec<Value> = server
                    .bearer_token
                    .iter()
                    .map(|token| json!({"name": "Authorization", "value": format!("Bearer {token}")}))
                    .collect();
                json!({"type": "http", "name": server.name, "url": server.url, "headers": headers})
            })
            .collect()
    }

    /// The session exists: choose the model, or start.
    fn opened(&mut self, session: String) -> Output {
        self.session = Some(session.clone());
        match self.config.model.clone() {
            Some(model) => Output::reply(self.calls.request(
                "session/set_model",
                json!({"sessionId": session, "modelId": model}),
                Purpose::SetModel,
            )),
            None => self.ready(),
        }
    }

    fn prompt(&mut self, session: &str, text: &str) -> Value {
        self.answer.clear();
        self.parted = false;
        let text = if std::mem::take(&mut self.first) {
            with_instructions(&preamble(&self.config), text)
        } else {
            text.to_owned()
        };
        self.calls.request(
            "session/prompt",
            json!({"sessionId": session, "prompt": [{"type": "text", "text": text}]}),
            Purpose::Prompt,
        )
    }

    /// The session is ready: report it and send whatever prompt was waiting.
    fn ready(&mut self) -> Output {
        let mut output = Output::event(AgentEvent::Started);
        if let Some(id) = self.session.clone().filter(|id| !id.is_empty()) {
            output.events.push(AgentEvent::SessionStarted { id });
        }
        let prompt = self.queued.take().filter(|prompt| !prompt.is_empty());
        if let (Some(session), Some(prompt)) = (self.session.clone(), prompt) {
            output.replies.push(self.prompt(&session, &prompt));
        }
        output
    }

    fn response(&mut self, purpose: Purpose, message: &Value) -> Output {
        if let Some(error) = rpc::error(message) {
            if let Purpose::LoadSession = purpose {
                self.loading = false;
                self.queued = None;
                return Output::event(AgentEvent::ResumeFailed {
                    message: format!("OpenCode: {error}"),
                });
            }
            return Output::event(AgentEvent::TurnFailed {
                message: format!("OpenCode: {error}"),
            });
        }
        let result = &message["result"];
        match purpose {
            Purpose::Initialize => {
                let servers = self.mcp_servers();
                let cwd = &self.config.cwd;
                if let SessionMode::Resume(session) = self.config.session.clone() {
                    if result["agentCapabilities"]["loadSession"] != true {
                        self.queued = None;
                        return Output::event(AgentEvent::ResumeFailed {
                            message: "OpenCode cannot load an earlier session".into(),
                        });
                    }
                    self.loading = true;
                    self.session = Some(session.clone());
                    return Output::reply(self.calls.request(
                        "session/load",
                        json!({"sessionId": session, "cwd": cwd, "mcpServers": servers}),
                        Purpose::LoadSession,
                    ));
                }
                Output::reply(self.calls.request(
                    "session/new",
                    json!({"cwd": cwd, "mcpServers": servers}),
                    Purpose::NewSession,
                ))
            }
            Purpose::NewSession => {
                let session = result["sessionId"].as_str().unwrap_or_default().to_owned();
                let models: Vec<ModelInfo> = result["models"]["availableModels"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|model| {
                        Some(ModelInfo {
                            id: model["modelId"]
                                .as_str()
                                .filter(|id| !id.is_empty())?
                                .to_owned(),
                            name: model["name"].as_str().map(str::to_owned),
                            efforts: Vec::new(),
                        })
                    })
                    .collect();
                let mut output = self.opened(session);
                if !models.is_empty() {
                    output.events.insert(0, AgentEvent::Models { models });
                }
                output
            }
            Purpose::LoadSession => {
                self.loading = false;
                let session = self.session.clone().unwrap_or_default();
                self.opened(session)
            }
            Purpose::SetModel => self.ready(),
            Purpose::Prompt => {
                self.asked.clear();
                self.started.clear();
                let answer = std::mem::take(&mut self.answer);
                Output::event(match result["stopReason"].as_str() {
                    Some("end_turn") | None => AgentEvent::TurnCompleted { answer },
                    Some("cancelled") => AgentEvent::TurnFailed {
                        message: "OpenCode turn cancelled".into(),
                    },
                    Some(reason) => AgentEvent::TurnFailed {
                        message: format!("OpenCode stopped: {reason}"),
                    },
                })
            }
        }
    }

    fn permission(&mut self, message: &Value) -> Output {
        if message["method"] != "session/request_permission" {
            return Output::reply(self.calls.refuse(&message["id"]));
        }
        let params = &message["params"];
        let tool = &params["toolCall"];
        let kind = match tool["kind"].as_str() {
            Some("execute") => PermissionKind::Command,
            Some("edit" | "delete" | "move") => PermissionKind::FileChange,
            _ => PermissionKind::Other,
        };
        let id = message["id"].to_string();
        let summary = tool["title"]
            .as_str()
            .unwrap_or("OpenCode action")
            .to_owned();
        self.asked.insert(
            id.clone(),
            (message["id"].clone(), params["options"].clone()),
        );
        Output::event(AgentEvent::PermissionRequested {
            request: PermissionRequest {
                id,
                kind,
                summary,
                detail: params.to_string(),
            },
        })
    }

    fn update(&mut self, update: &Value) -> Output {
        let kind = update["sessionUpdate"].as_str().unwrap_or_default();
        let event = match kind {
            "agent_message_chunk" => {
                let mut text = update["content"]["text"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                // ACP streams a message after a tool call as more chunks; it reads as a new
                // paragraph, as in the other adapters.
                if std::mem::take(&mut self.parted) && !text.is_empty() {
                    text.insert_str(0, PARAGRAPH);
                }
                self.answer.push_str(&text);
                AgentEvent::Text { text }
            }
            "agent_thought_chunk" => AgentEvent::Reasoning {
                text: update["content"]["text"]
                    .as_str()
                    .unwrap_or_default()
                    .into(),
            },
            "tool_call" | "tool_call_update" => {
                if !self.answer.is_empty() {
                    self.parted = true;
                }
                let id = update["toolCallId"].as_str().unwrap_or_default().to_owned();
                let done = matches!(update["status"].as_str(), Some("completed" | "failed"));
                if done {
                    AgentEvent::ToolFinished {
                        ok: update["status"] == "completed",
                        output: update["content"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|part| content_text(&part["content"]["text"]))
                            .collect::<Vec<_>>()
                            .join("\n"),
                        id,
                    }
                } else if kind == "tool_call" && !self.started.contains(&id) {
                    self.started.push(id.clone());
                    AgentEvent::ToolStarted {
                        call: ToolCall {
                            id,
                            name: update["title"].as_str().unwrap_or_default().into(),
                            input: update["rawInput"].to_string(),
                        },
                    }
                } else {
                    return Output::default();
                }
            }
            _ => return Output::default(),
        };
        Output::event(event)
    }
}

impl Adapter for OpenCode {
    fn command(&self) -> CommandSpec {
        let command = CommandSpec::new(self.config.program.as_deref().unwrap_or("opencode"))
            .cwd(&self.config.cwd)
            .arg("acp")
            .env("OPENCODE_DISABLE_AUTOUPDATE", "true")
            .env("OPENCODE_AUTO_SHARE", "false");
        let mut permission = serde_json::Map::new();
        if !self.config.native_tools {
            permission.insert("edit".into(), "deny".into());
            permission.insert("bash".into(), "deny".into());
        } else if self.config.read_only {
            // Edits are refused outright, and commands must ask so the application can refuse them.
            permission.insert("edit".into(), "deny".into());
            permission.insert("bash".into(), "ask".into());
        }
        if !self.config.subagents {
            permission.insert("task".into(), "deny".into());
        }
        if permission.is_empty() {
            command
        } else {
            command.env("OPENCODE_PERMISSION", Value::Object(permission).to_string())
        }
    }

    fn start(&mut self, prompt: &str) -> Vec<Value> {
        self.queued = Some(prompt.to_owned());
        vec![self.calls.request(
            "initialize",
            json!({"protocolVersion": 1,
                "clientCapabilities": {"fs": {"readTextFile": false, "writeTextFile": false}, "terminal": false},
                "clientInfo": {"name": self.config.client.name, "version": self.config.client.version}}),
            Purpose::Initialize,
        )]
    }

    fn send(&mut self, text: &str) -> Vec<Value> {
        match self.session.clone() {
            Some(session) => vec![self.prompt(&session, text)],
            None => {
                self.queued = Some(text.to_owned());
                Vec::new()
            }
        }
    }

    fn receive(&mut self, message: Value) -> Output {
        if rpc::is_request(&message) {
            return self.permission(&message);
        }
        if let Some(purpose) = self.calls.resolve(&message) {
            return self.response(purpose, &message);
        }
        if message["method"] == "session/update"
            && !self.loading
            && message["params"]["sessionId"].as_str() == self.session.as_deref()
        {
            return self.update(&message["params"]["update"]);
        }
        Output::default()
    }

    fn answer(&mut self, request: &str, allow: bool) -> Vec<Value> {
        let Some((id, options)) = self.asked.remove(request) else {
            return Vec::new();
        };
        let kind = if allow { "allow_once" } else { "reject_once" };
        let option = options
            .as_array()
            .into_iter()
            .flatten()
            .find(|option| option["kind"] == kind && option["optionId"].is_string());
        let outcome = option.map_or_else(
            || json!({"outcome": "cancelled"}),
            |option| json!({"outcome": "selected", "optionId": option["optionId"]}),
        );
        vec![self.calls.respond(&id, json!({"outcome": outcome}))]
    }

    fn interrupt(&mut self) -> Vec<Value> {
        match &self.session {
            Some(session) => {
                vec![
                    self.calls
                        .notify("session/cancel", json!({"sessionId": session})),
                ]
            }
            None => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProviderKind;
    use crate::config;

    #[test]
    fn permissions_refuse_edits_and_sub_agents_as_configured() {
        let permission = |read_only, subagents| {
            let config = AgentConfig {
                read_only,
                subagents,
                ..config(ProviderKind::OpenCode)
            };
            let command = OpenCode::new(config).command();
            command
                .env
                .get("OPENCODE_PERMISSION")
                .map(|text| serde_json::from_str::<Value>(text).unwrap())
        };
        assert_eq!(permission(false, true), None);
        assert_eq!(permission(false, false), Some(json!({"task": "deny"})));
        assert_eq!(
            permission(true, false),
            Some(json!({"edit": "deny", "bash": "ask", "task": "deny"}))
        );
    }

    #[test]
    fn handshake_selects_the_model_before_the_first_prompt() {
        let mut opencode = OpenCode::new(config(ProviderKind::OpenCode));
        let init = &opencode.start("Fix it")[0];
        assert_eq!(
            (init["jsonrpc"].as_str(), init["method"].as_str()),
            (Some("2.0"), Some("initialize"))
        );

        let new = &opencode
            .receive(json!({"jsonrpc":"2.0","id":init["id"],"result":{"protocolVersion":1}}))
            .replies[0];
        assert_eq!(new["params"]["cwd"], "/work/n1");
        let set = &opencode
            .receive(json!({"jsonrpc":"2.0","id":new["id"],"result":{"sessionId":"s"}}))
            .replies[0];
        assert_eq!(set["params"]["modelId"], "model-x");
        let output = opencode.receive(json!({"jsonrpc":"2.0","id":set["id"],"result":{}}));
        assert_eq!(
            output.events,
            [
                AgentEvent::Started,
                AgentEvent::SessionStarted { id: "s".into() }
            ]
        );
        let prompt = &output.replies[0];
        assert_eq!(prompt["method"], "session/prompt");

        let update = |update: Value| json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s","update":update}});
        let events: Vec<_> = [
            update(json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Ok"}})),
            update(json!({"sessionUpdate":"tool_call","toolCallId":"c","title":"bash","status":"pending","rawInput":{"command":"ls"}})),
            update(json!({"sessionUpdate":"tool_call_update","toolCallId":"c","status":"in_progress"})),
            update(json!({"sessionUpdate":"tool_call_update","toolCallId":"c","status":"completed","content":[{"type":"content","content":{"type":"text","text":"a"}}]})),
            update(json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Done"}})),
            update(json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"."}})),
            json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"other","update":{"sessionUpdate":"agent_message_chunk","content":{"text":"x"}}}}),
            json!({"jsonrpc":"2.0","id":prompt["id"],"result":{"stopReason":"end_turn"}}),
        ]
        .into_iter()
        .flat_map(|line| opencode.receive(line).events)
        .collect();
        assert_eq!(events.len(), 6);
        assert_eq!(
            events[2],
            AgentEvent::ToolFinished {
                id: "c".into(),
                ok: true,
                output: "a".into()
            }
        );
        // The message after the tool call is a new paragraph, once.
        assert_eq!(
            events[3],
            AgentEvent::Text {
                text: "\n\nDone".into()
            }
        );
        assert_eq!(events[4], AgentEvent::Text { text: ".".into() });
        assert_eq!(
            events[5],
            AgentEvent::TurnCompleted {
                answer: "Ok\n\nDone.".into()
            }
        );
    }

    #[test]
    fn permission_answers_pick_a_one_time_option() {
        let mut opencode = OpenCode::new(config(ProviderKind::OpenCode));
        let output = opencode.receive(json!({"jsonrpc":"2.0","id":7,"method":"session/request_permission","params":{
            "toolCall":{"kind":"execute","title":"bash"},
            "options":[{"kind":"allow_always","optionId":"always"},{"kind":"allow_once","optionId":"once"},{"kind":"reject_once","optionId":"no"}]}}));
        let [AgentEvent::PermissionRequested { request }] = &output.events[..] else {
            panic!()
        };
        assert_eq!(request.kind, PermissionKind::Command);
        let reply = &opencode.answer(&request.id, true)[0];
        assert_eq!(
            reply["result"]["outcome"],
            json!({"outcome":"selected","optionId":"once"})
        );
        assert_eq!(reply["id"], 7);
    }

    #[test]
    fn mcp_servers_ride_in_the_session_request_and_a_load_replays_silently() {
        let mut opencode = OpenCode::new(AgentConfig {
            mcp_servers: vec![crate::McpServer {
                bearer_token: Some("secret".into()),
                ..crate::McpServer::new("live", "http://127.0.0.1:9/mcp")
            }],
            session: SessionMode::Resume("old".into()),
            model: None,
            ..config(ProviderKind::OpenCode)
        });
        assert!(
            !opencode
                .command()
                .env
                .values()
                .any(|value| value.contains("secret"))
        );
        let init = opencode.start("Again").remove(0);
        let load = opencode
            .receive(json!({"jsonrpc":"2.0","id":init["id"],
                "result":{"agentCapabilities":{"loadSession":true}}}))
            .replies
            .remove(0);
        assert_eq!(load["method"], "session/load");
        assert_eq!(load["params"]["sessionId"], "old");
        assert_eq!(
            load["params"]["mcpServers"][0]["headers"][0]["value"],
            "Bearer secret"
        );
        let replayed = opencode.receive(json!({"jsonrpc":"2.0","method":"session/update",
            "params":{"sessionId":"old","update":{"sessionUpdate":"agent_message_chunk",
            "content":{"type":"text","text":"earlier"}}}}));
        assert!(replayed.events.is_empty());
        let ready = opencode.receive(json!({"jsonrpc":"2.0","id":load["id"],"result":{}}));
        assert_eq!(
            ready.events,
            [
                AgentEvent::Started,
                AgentEvent::SessionStarted { id: "old".into() }
            ]
        );
        assert_eq!(ready.replies[0]["method"], "session/prompt");
    }

    #[test]
    fn a_cli_that_cannot_load_sessions_reports_the_resume_lost() {
        let mut opencode = OpenCode::new(AgentConfig {
            session: SessionMode::Resume("old".into()),
            ..config(ProviderKind::OpenCode)
        });
        let init = opencode.start("Again").remove(0);
        let output = opencode.receive(json!({"jsonrpc":"2.0","id":init["id"],"result":{}}));
        assert!(output.replies.is_empty());
        assert!(matches!(
            &output.events[..],
            [AgentEvent::ResumeFailed { .. }]
        ));
    }

    #[test]
    fn without_native_tools_shell_and_edits_are_refused() {
        let command = OpenCode::new(AgentConfig {
            native_tools: false,
            ..config(ProviderKind::OpenCode)
        })
        .command();
        let permission: Value = serde_json::from_str(&command.env["OPENCODE_PERMISSION"]).unwrap();
        assert_eq!(permission["bash"], "deny");
        assert_eq!(permission["edit"], "deny");
    }
}
