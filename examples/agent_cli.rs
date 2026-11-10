//! A real coding-agent CLI in an Atelier conversation: `atelier-agents` runs Claude Code, Codex,
//! OpenCode or Antigravity CLI, and `AgentConnection` shows what it does in a `ChatView`.
//! Type in the composer and press Enter; approvals arrive as cards; Stop interrupts the turn.
//! The ring beside the model shows what is left of the plan, once the CLI reports it.
//!
//! With `--mock`, the agent is `atelier-agents-mock` playing the CLI from the demo scenario, so
//! nothing needs an account. Build the mock first:
//!
//! ```text
//! cargo build -p atelier-agents-mock
//! cargo run --example agent_cli --features agents -- --mock [--provider claude|codex|opencode|agy]
//!     [--prompt "Add a greeting"] [--direction glass|studio|mono] [--appearance dark|light]
//! cargo run --example agent_cli --features agents -- --provider codex --cwd ~/src/project
//! ```

mod common;

use std::path::PathBuf;

use atelier::agent::{AgentConnection, ChatEntry, ChatView, Composer, SuggestionChip};
use atelier::agents::{AgentConfig, ProviderKind};
use atelier::{ActiveTheme, Appearance, Direction, Label, TitleBar, WindowFrame};
use common::Themed;
use gpui::{
    Context, Entity, FocusHandle, IntoElement, KeyDownEvent, ParentElement, Render, Styled, Window,
    div, prelude::*, px,
};
use unicode_segmentation::UnicodeSegmentation;

/// Prompts the demo scenario answers in different ways: an edit that asks first, an echo, a
/// failure.
const PROMPTS: [&str; 3] = [
    "Add a greeting to src/lib.rs",
    "What does this crate do?",
    "Try something that will fail",
];

struct AgentCli {
    choice: (Direction, Appearance),
    chat: Entity<ChatView>,
    agent: Option<AgentConnection>,
    provider: ProviderKind,
    draft: String,
    input: FocusHandle,
    autofocus: bool,
    /// The working directory, kept alive for the mock's scenario.
    _scratch: Option<std::path::PathBuf>,
}

impl Themed for AgentCli {
    fn choice(&mut self) -> &mut (Direction, Appearance) {
        &mut self.choice
    }
}

impl AgentCli {
    fn send(&mut self, text: String, cx: &mut Context<Self>) {
        if text.trim().is_empty() {
            return;
        }
        if let Some(agent) = &self.agent {
            agent.send(text, cx);
        }
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        if keystroke.modifiers.control || keystroke.modifiers.platform || keystroke.modifiers.alt {
            return;
        }
        match keystroke.key.as_str() {
            "enter" => {
                let text = std::mem::take(&mut self.draft);
                self.send(text, cx);
            }
            "backspace" => {
                if let Some((at, _)) = self.draft.grapheme_indices(true).next_back() {
                    self.draft.truncate(at);
                }
            }
            "tab" | "escape" => return,
            _ => match keystroke.key_char.as_deref() {
                Some(typed) => self.draft.push_str(typed),
                None => return,
            },
        }
        cx.notify();
    }
}

impl Render for AgentCli {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.autofocus) {
            window.focus(&self.input);
        }
        let theme = cx.theme().clone();
        let switch = common::theme_switch(self, cx);
        let busy = self.agent.as_ref().is_some_and(AgentConnection::busy);
        let waiting = self
            .agent
            .as_ref()
            .map(AgentConnection::pending)
            .unwrap_or_default();
        let session = self.agent.as_ref().and_then(AgentConnection::session_id);
        let usage = self
            .agent
            .as_ref()
            .and_then(|agent| agent.usage("usage", cx));
        let focused = self.input.is_focused(window);
        let input = div()
            .id("draft")
            .track_focus(&self.input)
            .on_key_down(cx.listener(Self::key_down))
            .min_h(theme.typography.size_md * 3.)
            .text_color(theme.colors.text)
            .cursor_text()
            .child(self.draft.clone())
            .when(focused, |input| {
                input.child(
                    div()
                        .w(px(1.5))
                        .h(theme.typography.size_md * 1.3)
                        .bg(theme.colors.text),
                )
            });
        WindowFrame::new()
            .font(theme.typography.ui.clone())
            .text_color(theme.colors.text)
            .size_full()
            .flex()
            .flex_col()
            .child(
                TitleBar::new()
                    .app_icon(common::app_icon("atelier"))
                    .title(format!("{} in Atelier", self.provider.label()))
                    .trailing(switch),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .bg(theme.colors.surface)
                    .child(self.chat.clone())
                    .child(
                        div()
                            .flex()
                            .justify_center()
                            .px(theme.spacing.xl)
                            .pb(theme.spacing.lg)
                            .child(
                                div()
                                    .w_full()
                                    .max_w(px(780.))
                                    .flex()
                                    .flex_col()
                                    .gap(theme.spacing.sm)
                                    .child(div().flex().flex_wrap().gap(theme.spacing.xs).children(
                                        PROMPTS.into_iter().enumerate().map(|(index, prompt)| {
                                            SuggestionChip::new(("prompt", index), prompt).on_click(
                                                cx.listener(move |this, _, _, cx| {
                                                    this.send(prompt.into(), cx)
                                                }),
                                            )
                                        }),
                                    ))
                                    .children(waiting.iter().map(|text| {
                                        Label::caption(format!("Waiting to send: {text}"))
                                    }))
                                    .child(
                                        Composer::new("composer")
                                            .placeholder("Ask the agent…")
                                            .input(input)
                                            .empty(self.draft.is_empty())
                                            .focused(focused)
                                            .busy(busy)
                                            .queue(busy)
                                            .model(self.provider.label())
                                            .when_some(usage, Composer::usage)
                                            .on_send(cx.listener(|this, _, _, cx| {
                                                let text = std::mem::take(&mut this.draft);
                                                this.send(text, cx);
                                            }))
                                            .on_stop(cx.listener(|this, _, _, cx| {
                                                if let Some(agent) = &this.agent {
                                                    agent.interrupt();
                                                }
                                                cx.notify();
                                            })),
                                    )
                                    .children(
                                        session.map(|id| Label::caption(format!("Session {id}"))),
                                    ),
                            ),
                    ),
            )
    }
}

/// The provider named by `--provider`, Claude Code by default.
fn provider() -> ProviderKind {
    match common::argument("--provider").as_deref() {
        Some("codex") => ProviderKind::Codex,
        Some("opencode") => ProviderKind::OpenCode,
        Some("agy" | "antigravity") => ProviderKind::Antigravity,
        _ => ProviderKind::ClaudeCode,
    }
}

/// `atelier-agents-mock` beside this example's own build, as `cargo build -p
/// atelier-agents-mock` leaves it.
fn mock_program() -> Option<PathBuf> {
    let examples = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let name = format!("atelier-agents-mock{}", std::env::consts::EXE_SUFFIX);
    Some(examples.parent()?.join(name)).filter(|path| path.is_file())
}

fn main() {
    let mock = std::env::args().any(|arg| arg == "--mock");
    let provider = provider();
    let mut config;
    let mut scratch = None;
    if mock {
        let Some(program) = mock_program() else {
            eprintln!("Build the mock first: cargo build -p atelier-agents-mock");
            std::process::exit(2);
        };
        let directory =
            std::env::temp_dir().join(format!("atelier-agent-cli-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("a working directory for the mock");
        std::fs::write(
            directory.join(".atelier-agents-mock.json"),
            include_str!("../crates/atelier-agents-mock/scenarios/demo.json"),
        )
        .expect("the demo scenario");
        config = AgentConfig::new(provider, &directory);
        config.program = Some(program.to_string_lossy().into_owned());
        scratch = Some(directory);
    } else {
        let cwd = common::argument("--cwd")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().expect("a working directory"));
        config = AgentConfig::new(provider, cwd);
    }
    let prompt = common::argument("--prompt").unwrap_or_default();
    common::run("Agent CLI", 1100., 820., move |choice, cx| {
        let chat = cx.new(|_| ChatView::new("chat").reading_width(px(780.)));
        let agent = match AgentConnection::start(config, &prompt, &chat, cx) {
            Ok(agent) => Some(agent),
            Err(error) => {
                chat.update(cx, |chat, cx| {
                    chat.push(ChatEntry::error(error.to_string()), cx);
                });
                None
            }
        };
        // Redraw as the agent's events change the conversation.
        cx.observe(&chat, |_, _, cx| cx.notify()).detach();
        AgentCli {
            choice,
            chat,
            agent,
            provider,
            draft: String::new(),
            input: cx.focus_handle(),
            autofocus: true,
            _scratch: scratch,
        }
    });
}
