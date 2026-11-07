//! What a running agent reports, in one vocabulary for every provider.
//!
//! Claude Code, Codex, Antigravity and OpenCode each stream their own JSON. The adapters
//! translate it into [`AgentEvent`]s at the edge, so the rest of an application sees one shape
//! and never learns which CLI produced it.

use serde::{Deserialize, Serialize};

/// A tool the agent started, as shown in a transcript.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    /// The provider's id for the call, used to pair it with its result.
    pub id: String,
    /// The tool's name as the provider reports it (`Bash`, `commandExecution`, `execute`, ...).
    pub name: String,
    /// The tool's input, as JSON text, for display.
    pub input: String,
}

/// The broad kind of an action awaiting approval, for policy and for the approval card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionKind {
    /// Running a shell command.
    Command,
    /// Creating, editing or deleting files.
    FileChange,
    /// Anything the adapter could not classify.
    Other,
}

/// An action the agent will take only once it is allowed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRequest {
    /// Opaque to everyone but the adapter that issued it; sent back with the answer.
    pub id: String,
    /// What sort of action it is.
    pub kind: PermissionKind,
    /// A one-line description for the user.
    pub summary: String,
    /// The provider's full request, as JSON text, for anyone who wants detail.
    pub detail: String,
}

/// A model the CLI offers, as it reported it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInfo {
    /// What to put in [`crate::AgentConfig::model`].
    pub id: String,
    /// The name a person reads, when the CLI gives one.
    pub name: Option<String>,
    /// The reasoning efforts it accepts, in the CLI's own words, when the CLI says.
    pub efforts: Vec<String>,
}

/// One limit of the plan the CLI is signed in with: a rolling window, a week, a model's own
/// weekly share. Only subscription sign-ins report these; an API key has none.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimit {
    /// Names this limit across reports, so a later one replaces it: `five_hour`, `seven_day`,
    /// `seven_day_opus`, `codex/primary`.
    pub id: String,
    /// How long the window is, in minutes, when the CLI says: 300 for five hours, 10080 for a
    /// week.
    pub window_minutes: Option<u32>,
    /// What the limit covers beyond the plan as a whole, such as one model ("Opus"), in the
    /// CLI's words.
    pub scope: Option<String>,
    /// How much of it is spent, in whole percent. It can pass 100 when the CLI lets usage run
    /// over.
    pub used_percent: u32,
    /// When it fills again, in seconds since the Unix epoch.
    pub resets_at: Option<u64>,
    /// Whether the CLI refuses work until it resets.
    pub reached: bool,
}

impl RateLimit {
    /// The share still left, in whole percent.
    pub fn remaining_percent(&self) -> u32 {
        100u32.saturating_sub(self.used_percent)
    }
}

/// One thing a running agent did.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum AgentEvent {
    /// The process is up and has accepted its first instructions.
    Started,
    /// The CLI keeps this conversation under `id`: pass it as
    /// [`crate::SessionMode::Resume`] to carry it on in a later process.
    SessionStarted {
        /// The provider's session or thread id.
        id: String,
    },
    /// The session asked for by [`crate::SessionMode::Resume`] could not be resumed. No prompt
    /// was sent; stop this agent and start a new one.
    ResumeFailed {
        /// What the CLI said.
        message: String,
    },
    /// The models the CLI offers, as it reported them during its handshake (Claude Code, Codex,
    /// OpenCode).
    Models {
        /// The models.
        models: Vec<ModelInfo>,
    },
    /// How much of the plan's limits is spent (Claude Code, Codex). A report may name only some
    /// limits; each replaces the earlier report with the same [`RateLimit::id`]. Claude Code
    /// reports after each model response, Codex at the handshake and as usage moves.
    RateLimits {
        /// The limits this report names.
        limits: Vec<RateLimit>,
    },
    /// A message was sent to the agent: its first prompt or a later one. The application
    /// reports this, not the CLI, so that its transcript holds both sides of the conversation.
    Prompted {
        /// The message.
        text: String,
    },
    /// A message was sent while the agent was in a turn. The application holds it and sends it when
    /// the turn ends, reporting [`AgentEvent::Prompted`] then.
    Queued {
        /// The message.
        text: String,
    },
    /// Part of the reply the user reads.
    Text {
        /// The new text.
        text: String,
    },
    /// Part of the model's visible reasoning.
    Reasoning {
        /// The new text.
        text: String,
    },
    /// A tool call began.
    ToolStarted {
        /// The call.
        call: ToolCall,
    },
    /// A tool call ended.
    ToolFinished {
        /// The call's id.
        id: String,
        /// Whether it succeeded.
        ok: bool,
        /// Its output, as text.
        output: String,
    },
    /// The agent is blocked until this is allowed or refused.
    PermissionRequested {
        /// The request.
        request: PermissionRequest,
    },
    /// A turn ended normally; the agent waits for the next message.
    TurnCompleted {
        /// The whole reply of the turn.
        answer: String,
    },
    /// A turn ended in an error; the agent may still accept another message.
    TurnFailed {
        /// What went wrong, in words.
        message: String,
    },
    /// The process is gone and accepts nothing more.
    Exited {
        /// Why, when known.
        reason: Option<String>,
    },
}
