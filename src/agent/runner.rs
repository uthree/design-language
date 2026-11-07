//! A coding-agent CLI shown in a [`ChatView`]: `atelier-agents` runs the agent, this draws it.
//!
//! [`AgentFeed`] turns each [`AgentEvent`] into the chat's entries: the person's messages, the
//! streamed answer, reasoning, tool calls and their results, approvals and errors. It holds no
//! process, so a test or an application with its own runtime can feed it events directly.
//! [`AgentConnection`] is the whole loop for the common case: it starts the agent, feeds its
//! events to the chat on the UI thread, sends the person's answers to approvals back, and holds
//! messages sent mid-turn until the turn ends.
//!
//! Needs the `agents` feature.
//!
//! ```ignore
//! let chat = cx.new(|_| ChatView::new("chat"));
//! let config = AgentConfig::new(ProviderKind::ClaudeCode, project_dir);
//! let agent = AgentConnection::start(config, "Why does the test fail?", &chat, cx)?;
//! // later, from the composer:
//! agent.send("Fix it", cx);
//! ```

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use atelier_agents::{
    AgentConfig, AgentEvent, AgentSession, ModelInfo, SpawnError, ToolCall, adapter,
};
use futures::StreamExt;
use futures::channel::mpsc;
use gpui::{App, Context, Entity, SharedString, Subscription, Task};

use super::chat::{ChatEntry, ChatEvent, ChatView, Decision, EntryId};
use super::transcript::ToolState;
use crate::strings::ActiveStrings;

/// The index of each phase word in [`crate::Strings::agent_phases`].
mod phase {
    pub const WAITING: usize = 0;
    pub const THINKING: usize = 2;
    pub const WRITING: usize = 3;
    pub const TOOL: usize = 4;
    pub const APPROVAL: usize = 5;
}

/// Turns an agent's events into a [`ChatView`]'s entries.
#[derive(Default)]
pub struct AgentFeed {
    /// The answer being streamed, until a tool call or the turn's end closes it.
    answer: Option<EntryId>,
    /// The reasoning being streamed, until text or a tool call follows it.
    reasoning: Option<EntryId>,
    /// Tool-call entries by the provider's call id, with the input they were shown with.
    tools: HashMap<String, (EntryId, String)>,
    /// Approval entries and the request each answers.
    approvals: HashMap<EntryId, String>,
    session: Option<String>,
    models: Vec<ModelInfo>,
    busy: bool,
}

impl AgentFeed {
    /// A feed for an empty conversation.
    pub fn new() -> Self {
        Self::default()
    }

    /// The session id the CLI reported, to resume the conversation with later.
    pub fn session(&self) -> Option<&str> {
        self.session.as_deref()
    }

    /// The models the CLI listed in its handshake.
    pub fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    /// Whether a turn is running: between a prompt and its end.
    pub fn busy(&self) -> bool {
        self.busy
    }

    /// Show `event` in `chat`.
    pub fn apply(&mut self, event: &AgentEvent, chat: &mut ChatView, cx: &mut Context<ChatView>) {
        match event {
            AgentEvent::Started | AgentEvent::Queued { .. } => {}
            AgentEvent::SessionStarted { id } => self.session = Some(id.clone()),
            AgentEvent::Models { models } => self.models = models.clone(),
            AgentEvent::Prompted { text } => {
                self.close(chat, cx);
                chat.push(ChatEntry::user(text.clone()), cx);
                self.busy = true;
                self.phase(phase::WAITING, chat, cx);
            }
            AgentEvent::Text { text } => {
                self.reasoning = None;
                let answer = match self.answer {
                    Some(answer) => answer,
                    None => {
                        // A paragraph break that opens a new message has nothing to show.
                        if text.trim().is_empty() {
                            return;
                        }
                        let answer = chat.push(ChatEntry::streaming(), cx);
                        self.answer = Some(answer);
                        answer
                    }
                };
                chat.append(answer, text, cx);
                self.phase(phase::WRITING, chat, cx);
            }
            AgentEvent::Reasoning { text } => {
                let reasoning = match self.reasoning {
                    Some(reasoning) => reasoning,
                    None => {
                        let summary = cx.strings().agent_phases[phase::THINKING].clone();
                        let reasoning = chat.push(ChatEntry::reasoning(summary, ""), cx);
                        self.reasoning = Some(reasoning);
                        reasoning
                    }
                };
                chat.append(reasoning, text, cx);
                self.phase(phase::THINKING, chat, cx);
            }
            AgentEvent::ToolStarted { call } => {
                self.close(chat, cx);
                let entry = ChatEntry::tool(call.name.clone(), summary(call), ToolState::Running)
                    .detail(call.input.clone());
                let id = chat.push(entry, cx);
                self.tools.insert(call.id.clone(), (id, call.input.clone()));
                self.phase(phase::TOOL, chat, cx);
            }
            AgentEvent::ToolFinished { id, ok, output } => {
                if let Some((entry, input)) = self.tools.remove(id) {
                    let state = if *ok {
                        ToolState::Succeeded
                    } else {
                        ToolState::Failed
                    };
                    chat.update_entry(
                        entry,
                        |entry| {
                            if let ChatEntry::Tool {
                                state: current,
                                detail,
                                ..
                            } = entry
                            {
                                *current = state;
                                *detail = Some(if output.is_empty() {
                                    input
                                } else {
                                    format!("{input}\n\n{output}")
                                });
                            }
                        },
                        cx,
                    );
                }
            }
            AgentEvent::PermissionRequested { request } => {
                self.close(chat, cx);
                let id = chat.push(ChatEntry::approval(request.summary.clone()), cx);
                self.approvals.insert(id, request.id.clone());
                self.phase(phase::APPROVAL, chat, cx);
            }
            AgentEvent::TurnCompleted { .. } => self.end(None, chat, cx),
            AgentEvent::TurnFailed { message } => self.end(Some(message.clone()), chat, cx),
            AgentEvent::ResumeFailed { .. } => {}
            AgentEvent::Exited { reason } => {
                self.end(None, chat, cx);
                if let Some(reason) = reason {
                    chat.push(ChatEntry::error(reason.clone()), cx);
                }
            }
        }
    }

    /// The permission request an approval entry stands for, and whether the person allowed it.
    /// Returns `None` for any other entry, or one already answered.
    pub fn decision(&mut self, entry: EntryId, decision: Decision) -> Option<(String, bool)> {
        let request = self.approvals.remove(&entry)?;
        Some((request, decision != Decision::Deny))
    }

    /// Ends the streamed answer and reasoning, so what follows starts new entries.
    fn close(&mut self, chat: &mut ChatView, cx: &mut Context<ChatView>) {
        self.reasoning = None;
        if let Some(answer) = self.answer.take() {
            chat.finish(answer, cx);
        }
    }

    fn end(&mut self, error: Option<String>, chat: &mut ChatView, cx: &mut Context<ChatView>) {
        self.close(chat, cx);
        // Calls the CLI never finished did not succeed.
        for (_, (entry, _)) in self.tools.drain() {
            chat.set_tool_state(entry, ToolState::Failed, cx);
        }
        self.busy = false;
        chat.set_thinking(None, cx);
        if let Some(error) = error {
            chat.push(ChatEntry::error(error), cx);
        }
    }

    fn phase(&self, phase: usize, chat: &mut ChatView, cx: &mut Context<ChatView>) {
        if self.busy {
            let label: SharedString = cx.strings().agent_phases[phase].clone();
            chat.set_thinking(Some(label), cx);
        }
    }
}

/// What a tool call did, in a line: the command it ran or the file it touched, else its name.
fn summary(call: &ToolCall) -> SharedString {
    let input: serde_json::Value = serde_json::from_str(&call.input).unwrap_or_default();
    let target = match &input {
        serde_json::Value::String(text) => Some(text.as_str()),
        value => ["command", "file_path", "path", "pattern", "url"]
            .iter()
            .find_map(|key| value[key].as_str()),
    };
    match target {
        Some(target) => format!("{}: {}", call.name, target.lines().next().unwrap_or("")).into(),
        None => call.name.clone().into(),
    }
}

/// What the connection and its tasks share.
struct Shared {
    feed: AgentFeed,
    session: AgentSession,
    /// Messages sent during a turn, sent in order as turns end.
    pending: VecDeque<String>,
}

/// A running agent shown in a [`ChatView`]. Dropping it stops the agent.
pub struct AgentConnection {
    shared: Rc<RefCell<Shared>>,
    chat: Entity<ChatView>,
    _events: Task<()>,
    _decisions: Subscription,
}

impl AgentConnection {
    /// Starts the agent `config` describes, sends `prompt` (none when empty), and shows
    /// everything it does in `chat`.
    pub fn start(
        config: AgentConfig,
        prompt: &str,
        chat: &Entity<ChatView>,
        cx: &mut App,
    ) -> Result<Self, SpawnError> {
        let (sender, mut events) = mpsc::unbounded();
        let session = AgentSession::spawn(adapter(config), prompt, move |event| {
            sender.unbounded_send(event).ok();
        })?;
        let shared = Rc::new(RefCell::new(Shared {
            feed: AgentFeed::new(),
            session,
            pending: VecDeque::new(),
        }));
        if !prompt.is_empty() {
            let prompted = AgentEvent::Prompted {
                text: prompt.to_owned(),
            };
            chat.update(cx, |chat, cx| {
                shared.borrow_mut().feed.apply(&prompted, chat, cx)
            });
        }
        let task = cx.spawn({
            let shared = shared.clone();
            let chat = chat.clone();
            async move |cx| {
                while let Some(event) = events.next().await {
                    let ended = matches!(
                        event,
                        AgentEvent::TurnCompleted { .. } | AgentEvent::TurnFailed { .. }
                    );
                    let shown = chat.update(cx, |chat, cx| {
                        shared.borrow_mut().feed.apply(&event, chat, cx);
                        if ended {
                            Self::next(&shared, chat, cx);
                        }
                    });
                    if shown.is_err() {
                        return;
                    }
                }
            }
        });
        let decisions = cx.subscribe(chat, {
            let shared = shared.clone();
            move |_, event: &ChatEvent, _| {
                if let ChatEvent::Decided { entry, decision } = event {
                    let mut shared = shared.borrow_mut();
                    if let Some((request, allow)) = shared.feed.decision(*entry, *decision) {
                        shared.session.answer(request, allow);
                    }
                }
            }
        });
        Ok(Self {
            shared,
            chat: chat.clone(),
            _events: task,
            _decisions: decisions,
        })
    }

    /// Sends the person's next message. During a turn it waits, and goes when the turn ends.
    pub fn send(&self, text: impl Into<String>, cx: &mut App) {
        let text = text.into();
        let busy = self.shared.borrow().feed.busy();
        if busy {
            self.shared.borrow_mut().pending.push_back(text);
            return;
        }
        let shared = self.shared.clone();
        self.chat.update(cx, |chat, cx| {
            let mut shared = shared.borrow_mut();
            let prompted = AgentEvent::Prompted { text: text.clone() };
            shared.feed.apply(&prompted, chat, cx);
            shared.session.send(text);
        });
    }

    /// Messages waiting for the running turn to end, oldest first.
    pub fn pending(&self) -> Vec<String> {
        self.shared.borrow().pending.iter().cloned().collect()
    }

    /// Drops the messages waiting for the running turn to end.
    pub fn clear_pending(&self) {
        self.shared.borrow_mut().pending.clear();
    }

    /// Asks the agent to stop the running turn; it stays up for the next message.
    pub fn interrupt(&self) {
        self.shared.borrow().session.interrupt();
    }

    /// Whether a turn is running.
    pub fn busy(&self) -> bool {
        self.shared.borrow().feed.busy()
    }

    /// The session id the CLI reported, to resume the conversation with later.
    pub fn session_id(&self) -> Option<String> {
        self.shared.borrow().feed.session().map(str::to_owned)
    }

    /// The models the CLI listed in its handshake.
    pub fn models(&self) -> Vec<ModelInfo> {
        self.shared.borrow().feed.models().to_vec()
    }

    /// Sends the oldest waiting message, now that a turn has ended.
    fn next(shared: &Rc<RefCell<Shared>>, chat: &mut ChatView, cx: &mut Context<ChatView>) {
        let mut shared = shared.borrow_mut();
        if let Some(text) = shared.pending.pop_front() {
            let prompted = AgentEvent::Prompted { text: text.clone() };
            shared.feed.apply(&prompted, chat, cx);
            shared.session.send(text);
        }
    }
}
