//! Asking a CLI which models it offers, without starting a conversation.

use std::fmt;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::{
    AgentConfig, AgentEvent, AgentSession, ModelInfo, ProviderKind, SessionMode, SpawnError,
    adapter,
};

/// Why [`list_models`] has no list.
#[derive(Debug)]
pub enum ListModelsError {
    /// The CLI could not be started.
    Spawn(SpawnError),
    /// The CLI has no way to list its models through this crate (Antigravity CLI).
    Unsupported,
    /// The CLI exited, or reported a failure, before it listed any.
    Failed(String),
    /// It did not answer in time.
    TimedOut,
}

impl fmt::Display for ListModelsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(error) => error.fmt(f),
            Self::Unsupported => f.write_str("this CLI cannot list its models"),
            Self::Failed(message) => write!(f, "the CLI did not list its models: {message}"),
            Self::TimedOut => f.write_str("the CLI did not list its models in time"),
        }
    }
}

impl std::error::Error for ListModelsError {}

/// Starts the CLI `config` describes, completes its handshake without sending a prompt, and
/// returns the models it offers. The session is ephemeral and stopped afterwards. This blocks
/// for up to `timeout`: keep it off a UI thread.
pub fn list_models(
    config: AgentConfig,
    timeout: Duration,
) -> Result<Vec<ModelInfo>, ListModelsError> {
    if config.provider == ProviderKind::Antigravity {
        return Err(ListModelsError::Unsupported);
    }
    let config = AgentConfig {
        session: SessionMode::Ephemeral,
        ..config
    };
    let (sender, events) = mpsc::channel();
    let session = AgentSession::spawn(adapter(config), "", move |event| {
        sender.send(event).ok();
    })
    .map_err(ListModelsError::Spawn)?;
    let deadline = Instant::now() + timeout;
    let result = loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match events.recv_timeout(left) {
            Ok(AgentEvent::Models { models }) => break Ok(models),
            Ok(AgentEvent::TurnFailed { message }) => break Err(ListModelsError::Failed(message)),
            Ok(AgentEvent::Exited { reason }) => {
                break Err(ListModelsError::Failed(
                    reason.unwrap_or_else(|| "it exited".into()),
                ));
            }
            Ok(_) => {}
            Err(_) => break Err(ListModelsError::TimedOut),
        }
    };
    session.stop();
    result
}
