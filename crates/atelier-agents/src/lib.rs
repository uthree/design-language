//! Coding-agent CLIs behind one interface: Claude Code, Codex, OpenCode and Antigravity CLI.
//!
//! Atelier draws an agent's conversation; this crate runs the agent. It knows no UI and needs no
//! gpui, so a daemon, a command-line tool or a test can use it as well as a desktop app.
//!
//! Each provider module holds an [`Adapter`]: a state machine with no I/O that knows which
//! command starts the CLI, what to write to it, and how to read what it writes back into
//! [`AgentEvent`]s. Keeping I/O out means an adapter is tested by feeding it recorded JSON, and the
//! same adapter works whether the CLI's stdio is a local pipe, an `ssh` session or an async
//! runtime's child process. [`AgentSession`] supplies the I/O for the common case: it starts the
//! process, pumps lines through the adapter on its own threads and hands events to a callback.
//!
//! The wire formats are Claude Code's `stream-json`, Codex's `app-server` JSON-RPC, OpenCode's
//! Agent Client Protocol and Antigravity's `stream-json`. The `atelier-agents-mock` crate plays
//! all four from a scripted scenario, so an application's tests need no account.
//!
//! ```no_run
//! use atelier_agents::{AgentConfig, AgentEvent, AgentSession, ProviderKind, adapter};
//!
//! let config = AgentConfig::new(ProviderKind::ClaudeCode, "/path/to/project");
//! let session = AgentSession::spawn(adapter(config), "Explain this crate", |event| {
//!     if let AgentEvent::Text { text } = event {
//!         print!("{text}");
//!     }
//! })?;
//! # drop(session);
//! # Ok::<(), atelier_agents::SpawnError>(())
//! ```

#![warn(missing_docs)]

mod antigravity;
mod claude;
mod codex;
mod command;
mod event;
mod opencode;
mod provider;
mod rpc;
pub mod session;

use std::path::PathBuf;

use serde_json::Value;

pub use command::{CommandSpec, quote};
pub use event::{AgentEvent, PermissionKind, PermissionRequest, ToolCall};
pub use provider::{Effort, ProviderKind, ProviderStatus, SetupAction};
pub use session::{AgentSession, SpawnError};

/// How the application names itself to CLIs that ask (Codex's and OpenCode's handshakes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientInfo {
    /// A short machine name, such as `auris-studio`.
    pub name: String,
    /// The name a person reads, such as `Auris Studio`.
    pub title: String,
    /// The application's version.
    pub version: String,
}

impl Default for ClientInfo {
    fn default() -> Self {
        Self {
            name: "atelier-agents".into(),
            title: "Atelier".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        }
    }
}

/// How to start one agent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentConfig {
    /// The CLI.
    pub provider: ProviderKind,
    /// The provider's model name, or `None` for the CLI's default.
    pub model: Option<String>,
    /// Reasoning effort, or `None` for the CLI's default.
    pub effort: Option<Effort>,
    /// The directory it works in, on its host.
    pub cwd: PathBuf,
    /// Standing instructions put in front of the conversation: the agent's role and limits.
    pub instructions: String,
    /// Overrides the executable's name or path.
    pub program: Option<String>,
    /// Refuse every change to files, where the CLI can be told so up front. The application
    /// should also refuse any permission such an agent asks for.
    pub read_only: bool,
    /// Whether it may start sub-agents of its own. Where the CLI can be told so up front, its
    /// sub-agent tools are refused; otherwise [`starts_subagent`] lets the application notice
    /// their use.
    pub subagents: bool,
    /// How the application names itself to the CLI.
    pub client: ClientInfo,
}

impl AgentConfig {
    /// An agent on `provider` working in `cwd`, with the CLI's default model and effort, no
    /// instructions, and permission to edit files and start sub-agents.
    pub fn new(provider: ProviderKind, cwd: impl Into<PathBuf>) -> Self {
        Self {
            provider,
            model: None,
            effort: None,
            cwd: cwd.into(),
            instructions: String::new(),
            program: None,
            read_only: false,
            subagents: true,
            client: ClientInfo::default(),
        }
    }
}

/// What an adapter produced from one message.
#[derive(Debug, Default, PartialEq)]
pub struct Output {
    /// Events for the application.
    pub events: Vec<AgentEvent>,
    /// Messages to write back to the CLI.
    pub replies: Vec<Value>,
}

impl Output {
    fn event(event: AgentEvent) -> Self {
        Self {
            events: vec![event],
            replies: Vec::new(),
        }
    }

    fn reply(reply: Value) -> Self {
        Self {
            events: Vec::new(),
            replies: vec![reply],
        }
    }
}

/// One provider's protocol, without I/O.
///
/// The session calls [`Adapter::start`] once after spawning [`Adapter::command`], then
/// [`Adapter::receive`] for every line the CLI writes. [`Adapter::send`] is called only between
/// turns, after a [`AgentEvent::TurnCompleted`] or [`AgentEvent::TurnFailed`].
pub trait Adapter: Send {
    /// The process to start.
    fn command(&self) -> CommandSpec;
    /// The first messages, carrying the first prompt. An adapter whose CLI needs a handshake
    /// sends the handshake here and the prompt once the CLI is ready.
    fn start(&mut self, prompt: &str) -> Vec<Value>;
    /// The next user message.
    fn send(&mut self, text: &str) -> Vec<Value>;
    /// One message from the CLI.
    fn receive(&mut self, message: Value) -> Output;
    /// Answers a [`AgentEvent::PermissionRequested`].
    fn answer(&mut self, request: &str, allow: bool) -> Vec<Value>;
    /// Messages that ask the CLI to stop the current turn before it is killed.
    fn interrupt(&mut self) -> Vec<Value> {
        Vec::new()
    }
}

/// The adapter for an agent's provider.
pub fn adapter(config: AgentConfig) -> Box<dyn Adapter> {
    match config.provider {
        ProviderKind::ClaudeCode => Box::new(claude::Claude::new(config)),
        ProviderKind::Codex => Box::new(codex::Codex::new(config)),
        ProviderKind::Antigravity => Box::new(antigravity::Antigravity::new(config)),
        ProviderKind::OpenCode => Box::new(opencode::OpenCode::new(config)),
    }
}

/// Whether `tool`, a [`ToolCall::name`] the provider's adapter reported, hands work
/// to a sub-agent. An agent that may not start sub-agents can be stopped when it calls one: some
/// CLIs can be told up front (see [`AgentConfig::subagents`]) and some cannot.
pub fn starts_subagent(provider: ProviderKind, tool: &str) -> bool {
    let tools: &[&str] = match provider {
        ProviderKind::ClaudeCode => &claude::SUBAGENT_TOOLS,
        ProviderKind::Codex => &codex::SUBAGENT_TOOLS,
        ProviderKind::Antigravity => &antigravity::SUBAGENT_TOOLS,
        ProviderKind::OpenCode => &opencode::SUBAGENT_TOOLS,
    };
    tools.iter().any(|name| name.eq_ignore_ascii_case(tool))
}

/// What separates two messages of one turn, in the answer and in the streamed text.
const PARAGRAPH: &str = "\n\n";

/// Puts the standing instructions in front of the first prompt, for CLIs that have no
/// separate channel for them that leaves their own system prompt intact.
fn with_instructions(instructions: &str, prompt: &str) -> String {
    if instructions.is_empty() {
        prompt.to_owned()
    } else {
        format!("{instructions}\n\n---\n\n{prompt}")
    }
}

/// The text of a tool result that may be a string or a list of content parts.
fn content_text(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| part["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[cfg(test)]
fn config(provider: ProviderKind) -> AgentConfig {
    AgentConfig {
        provider,
        model: Some("model-x".into()),
        effort: Some(Effort::High),
        cwd: "/work/n1".into(),
        instructions: "Only touch src/.".into(),
        program: None,
        read_only: false,
        subagents: true,
        client: ClientInfo::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_provider_names_its_own_sub_agent_tools() {
        assert!(starts_subagent(ProviderKind::ClaudeCode, "Task"));
        assert!(starts_subagent(ProviderKind::ClaudeCode, "Agent"));
        assert!(!starts_subagent(ProviderKind::ClaudeCode, "Bash"));
        assert!(starts_subagent(
            ProviderKind::Antigravity,
            "invoke_subagent"
        ));
        assert!(!starts_subagent(ProviderKind::Antigravity, "run_command"));
        assert!(starts_subagent(ProviderKind::OpenCode, "task"));
        assert!(starts_subagent(ProviderKind::Codex, "collabAgentToolCall"));
        assert!(!starts_subagent(ProviderKind::Codex, "commandExecution"));
    }
}
