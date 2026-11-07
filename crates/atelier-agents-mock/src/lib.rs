//! Scripted stand-ins for the coding-agent CLIs `atelier-agents` drives, for tests that need no
//! account.
//!
//! The adapters in `atelier-agents` drive each provider's CLI over stdio and read that CLI's
//! JSON, so the main stand-in is a fake CLI. [`cli::run`] plays any of the four (Claude Code,
//! Codex, Antigravity CLI, OpenCode) in its own wire format, and the `atelier-agents-mock`
//! executable runs it as a process, so a test exercises the real adapter, `AgentSession`'s pipes,
//! and the application above them.
//!
//! What it answers comes from a [`Scenario`]: rules matched against each prompt, whose replies
//! stream text and reasoning, call tools, ask permission, pause, fail, crash, or print noise.
//! The same scenario drives [`server::Server`], an OpenAI-compatible HTTP server, for pointing a
//! real CLI (or an application's own HTTP client) at.
//!
//! The executable:
//!
//! ```sh
//! # links claude, codex, agy and opencode in DIR to itself
//! atelier-agents-mock install DIR
//! # an application, on fake providers
//! PATH=DIR:$PATH ATELIER_AGENTS_MOCK_SCENARIO=scenario.json cargo run
//! # the OpenAI-compatible server
//! atelier-agents-mock serve [--listen 127.0.0.1:8080]
//! ```
//!
//! Run under any other name, it is the CLI its name or arguments say it is (see
//! [`cli::Cli::detect`]). The scenario comes from [`scenario::SCENARIO_VAR`], or else from
//! [`scenario::SCENARIO_FILE`] in the working directory.
//!
//! [`AgentSession`]: https://docs.rs/maestro-agents

#![warn(missing_docs)]

pub mod cli;
pub mod scenario;
pub mod server;

pub use cli::Cli;
pub use scenario::{Rule, SCENARIO_FILE, SCENARIO_VAR, Scenario, Step, Stream, Tool, When};
pub use server::Server;
