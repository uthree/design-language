# atelier-agents

Coding-agent CLIs behind one interface: Claude Code, Codex, OpenCode and Antigravity CLI.
Atelier draws an agent's conversation; this crate runs the agent. It has no UI, no gpui and no
async runtime, so a desktop app, a daemon, a command-line tool and a test can all use it.

```toml
[dependencies]
atelier-agents = { git = "https://github.com/uthree/design-language", tag = "vX.Y.Z" }

[dev-dependencies]
atelier-agents-mock = { git = "https://github.com/uthree/design-language", tag = "vX.Y.Z" }
```

## How it is built

- **`Adapter`**, one per CLI, is a state machine with no I/O. It says which command starts the
  CLI (`command()`), what to write first (`start`), what to write for the next message
  (`send`), how to answer a permission request (`answer`) and how to stop a turn
  (`interrupt`), and turns each JSON line the CLI writes into `AgentEvent`s and replies
  (`receive`). Test it by feeding it recorded JSON.
- **`AgentEvent`** is the one vocabulary: `Started`, `Text`, `Reasoning`, `ToolStarted`,
  `ToolFinished`, `PermissionRequested`, `TurnCompleted`, `TurnFailed`, `Exited`, plus
  `Prompted` and `Queued`, which the application reports so its transcript holds both sides.
- **`AgentSession`** runs an adapter on this machine with two threads and hands events to a
  callback. `AgentSession::spawn_in` takes the process to run instead, for an agent on another
  machine: wrap `CommandSpec::shell_line()` in `ssh`, `wsl.exe` or `docker exec`. An application
  on tokio can skip the session and drive the adapter from its own child process.
- **`AgentConfig`** holds the provider, model and effort, the working directory, standing
  instructions or a replacement system prompt, and read-only mode. It also says whether the
  agent may start sub-agents and use the CLI's own shell and file tools. Further settings
  cover MCP servers, a new, ephemeral or resumed session, the fast service tier, isolation from
  the user's own CLI setup, and how the application names itself.

### What each CLI supports

| Setting | Claude Code | Codex | OpenCode | Antigravity |
| --- | --- | --- | --- | --- |
| `mcp_servers` | `--mcp-config` file | `-c mcp_servers.*`, token in env | `session/new` | ignored |
| `session` | `--resume`, `--no-session-persistence` | `thread/resume`, `ephemeral` | `session/load` | none kept |
| `system_prompt` | `--system-prompt-file` | `baseInstructions` | before the first prompt | before the first prompt |
| `native_tools: false` | `--tools ""` | shell features off | `bash`/`edit` denied | not enforced |
| `fast` | `fastMode` setting | `service_tier` | ignored | ignored |
| `isolated` | setting sources, hooks, plugins, MCP | hooks, plugins, skills, memories, project docs | ignored | ignored |
| `AgentEvent::Models` | `initialize` | `model/list` | `session/new` | none |
| `AgentEvent::RateLimits` | `rate_limit_event`, after each response | `account/rateLimits/read` at the handshake, then `account/rateLimits/updated` | none | none |

Set `scratch` to a private directory so that Claude Code's MCP tokens and system prompt are
written to owner-only files there by `AgentSession::spawn`, instead of passed as arguments.

### Plan limits

A CLI signed in with a subscription says how much of the plan is spent, and
`AgentEvent::RateLimits` carries that as `RateLimit`s: the five-hour window, the week, and a
model's own weekly share (Claude Code's Opus or Sonnet, Codex's other buckets). A report may
name only the limits that moved; each one replaces the earlier report with the same `id`. A CLI
signed in with an API key reports none, and neither does OpenCode or Antigravity. Atelier's
`agents` feature shows them in the composer's `UsageMeter`.

### Resuming a conversation

Keep the id from `AgentEvent::SessionStarted` with the conversation, and start the next process
with `SessionMode::Resume(id)`. When the CLI no longer has that session, the agent reports
`AgentEvent::ResumeFailed` (Claude Code exits before `SessionStarted` instead) and sends no
prompt. Stop it and start a `SessionMode::New` agent with the earlier conversation handed over
as text.

```rust,no_run
use atelier_agents::{AgentConfig, AgentEvent, AgentSession, ProviderKind, adapter};

let config = AgentConfig::new(ProviderKind::Codex, "/path/to/project");
let session = AgentSession::spawn(adapter(config), "Explain this crate", |event| match event {
    AgentEvent::Text { text } => print!("{text}"),
    AgentEvent::PermissionRequested { request } => println!("\n? {}", request.summary),
    _ => {}
})?;
# Ok::<(), atelier_agents::SpawnError>(())
```

## Testing without an account

`atelier-agents-mock` is a fake CLI that speaks all four wire formats, scripted by a JSON
scenario of rules matched against each prompt:

```json
{
  "rules": [
    {"when": {"prompt_contains": "build"},
     "reply": [{"tool": {"name": "Bash", "input": {"command": "cargo build"},
                         "output": "Finished", "ask": true}},
               {"say": "Built."}]},
    {"when": {"prompt_contains": "flaky"}, "times": 1, "reply": [{"fail": "rate limited"}]}
  ],
  "otherwise": [{"say": "You said: {prompt}"}]
}
```

In a test, point `AgentConfig::program` at the mock (`env!("CARGO_BIN_EXE_atelier-agents-mock")`
from within this workspace, or an installed copy) and write the scenario to
`.atelier-agents-mock.json` in the agent's working directory, as
[the mock's own tests](../atelier-agents-mock/tests/adapters.rs) do. To run a whole app on fake
providers, `atelier-agents-mock install DIR` puts `claude`, `codex`, `agy` and `opencode` in
`DIR`; put it first on `PATH` and name the scenario in `ATELIER_AGENTS_MOCK_SCENARIO`.
`atelier-agents-mock serve` is an OpenAI-compatible server driven by the same scenario.

A scenario's `"limits": {"five_hour": 40, "weekly": 70, "per_turn": 5}` makes the Claude Code
and Codex stand-ins report a plan, spending `per_turn` percent of each limit every turn.

Every stand-in offers the models in `atelier_agents_mock::cli::MODELS` and keeps its sessions;
resuming an id that starts with `missing` fails the way a real CLI does when it no longer has
the session, so a test can reach the application's fallback.

The adapters were ported from Maestro's `maestro-agents`, whose wire formats follow what Auris
Studio's backends speak to the same CLIs.
