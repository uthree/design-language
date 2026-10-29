# Agent surfaces

`use atelier::agent::*;` The pieces take plain values; map your own conversation model onto
them. They know nothing about providers or transport.

## Principles

- **One colour for the AI** (`colors.agent`). Agent avatar, running work, AI drafts, the
  `Agent` button variant, `Tone::Agent` badges. Nothing else.
- **Two voices, two forms.** The person: a bubble on the trailing edge (`UserMessage`). The
  agent: full-width prose (`AssistantMessage`). **When only these two speak, leave out the
  agent's identity line** with `AssistantMessage::anonymous()`: the bubble already says which
  voice is the person, so a name and avatar on every answer only repeat it (Auris's compose
  window does this; its people said the name was noise). Keep the identity line — `.name(..)`,
  with the avatar — only where more than one agent or person speaks. An anonymous message moves
  its `.meta(..)` into the footer.
- **The answer leads; the work folds.** Reasoning and tool calls are collapsed one-liners
  (`ReasoningBlock`, `ToolCall` in a `ToolGroup`) that open on click.
- **Ask before acting on something the person owns.** `ApprovalCard` names the verb and object;
  Deny is always present; destructive approvals use `.destructive()`.
- **Mark the agent's work where it lives** — in the canvas, not only in chat — and let it be kept
  or undone there.
- **Status by shape and colour**: `AgentStatus` has `label()`, `mark()` and `color(theme)`.

## The panel

```rust
AgentPanel::new("agent", conversation_title)
    .status(AgentStatus::Running)            // gradient edge appears while running
    .status_detail("3 tools · 14 s")
    .action(Button::icon_only("history", IconName::History).tooltip("Conversations"))
    .action(Button::icon_only("new", IconName::Plus).tooltip("New conversation"))
    .on_close(cx.listener(|this, _, _, cx| { this.agent_open = false; cx.notify(); }))
    .entry(UserMessage::new(text).attachment(ContextChip::new("c1", IconName::Music, "Piano · bars 17–24")))
    .entry(ReasoningBlock::new("r1", "Thought for 6 s", reasoning).open(open).on_toggle(..))
    .entry(ToolGroup::new("Used 3 tools")
        .call(ToolCall::new("t1", "read_notes", "Read 96 notes", ToolState::Succeeded).icon(IconName::Eye))
        .call(ToolCall::new("t2", "edit_notes", "Revoiced bars 17–24", ToolState::Running).detail(json)))
    .entry(AssistantMessage::new(markdownish).meta("Claude Opus 5.5 · 14 s").streaming(false)
        .action(Button::icon_only("copy", IconName::Copy).size(ControlSize::Small)))
    .entry(ApprovalCard::new("a1", "Create \u{201c}Pad swell\u{201d} on Strings?")
        .body("Adds one clip in bars 17–32. Nothing existing is replaced.")
        .allow_label("Create").on_allow(..).on_deny(..).on_always(..))
    .entry(ChatNotice::status("Context compacted"))
    .empty_state(/* greeting + SuggestionChips */)
    .composer(Composer::new("composer").input(your_input_view).model("Claude Opus 5.5")
        .context(chip).busy(running).on_send(..).on_stop(..).on_model(..))
```

Put the panel in a fixed-width (≈360–400 px), dismissible region on the trailing edge, with
`border_l_1()` in `colors.border`. The transcript scrolls inside the panel; the composer stays
pinned. Use `ThinkingIndicator::new("Reading the score")` before the first token, and
`AssistantMessage::streaming(true)` while text arrives (it hides footer actions and shows a
caret). Errors: `ChatNotice::error("Couldn't reach the model. Check your connection and try
again.").action(Button::new("retry", "Retry"))`.

`AssistantMessage` renders paragraphs, `- ` bullets, `# ` headings and fenced code. Pass a real
Markdown view with `.body(element)` when you have one (Auris uses gpui-component's).

The composer's text area is a slot: pass your editable input entity with `.input(...)` and say
whether it holds text with `.empty(bool)` — the composer then shows the placeholder under your
input and disables Send while it is empty. Atelier does not ship a text editor; `.value(..)` is
for mock-ups.

The row under the text holds attach, the model picker (`.model(label)`, `.on_model(..)`) and
then your own `.control(..)`s before Send: the reasoning effort and fast mode as one
`EffortButton` (below), a voice picker as a small ghost button with a chevron. Keep them small and unfilled so the row stays one
line and Send is the only filled control in it. Do not rebuild the frame to add a control. Send
and Stop already follow the composer's corner (`Theme::concentric(radius_lg, spacing.sm)`); a
button you place in another rounded surface's corner does the same with `Button::radius(..)`.

### Effort and fast mode

Reasoning effort and fast mode are one control, because they are one trade: speed against
thought. `EffortButton::new(id, level).fast(fast).open(open)` sits in the composer row, showing
the level (and the bolt, latched, while fast mode is on), and opens an
`EffortPicker::new(id, levels, selected)` above it with `anchored` + `deferred`:

```rust
EffortPicker::new("effort", ["Low", "Medium", "High", "Max"], effort)
    .default_level(1)            // reset returns here; disabled while already there
    .fast(fast)                  // the bolt, top left, latches in the accent
    .model("GPT-6.1 Sol")        // under the level; .on_model opens the model menu
    .on_select(cx.listener(|this, level: &usize, _, cx| { this.effort = *level; cx.notify() }))
    .on_fast(cx.listener(|this, _, _, cx| { this.fast = !this.fast; cx.notify() }))
    .on_reset(cx.listener(|this, _, _, cx| { this.effort = 1; this.fast = false; cx.notify() }))
```

The level reads in the accent at the top. The slider is a wide stepped track: the filled part
sparkles (still when `theme.motion.enabled` is off), a dot marks each step above the thumb, and
a click, a drag or the arrow keys, Home and End move it a step; the thumb glides there. Close the
picker on a press outside it (`on_mouse_down_out`), leaving presses on the button to toggle it.
`examples/auris_screens.rs` (`--screen compose --effort`) wires it up.

## Entry points

- `AgentToggle::new(id, open, status)` in the title bar; while the panel is hidden it shows the
  status mark (… ? ✓ !) so running and waiting turns are never invisible.
- `AskBar::new(id, "Ask about this song…").shortcut("⌘K")` where the work is — sidebar foot,
  inspector top — opening the panel or an inline prompt with the selection as context.
- `SuggestionChip` for prompt suggestions in the empty state.
- `ContextChip` for what will be sent: a track, a clip range, a file. `.on_remove(..)` makes it
  removable in the composer.
- `ConversationRow` for history lists; live conversations show their status instead of time.
- `StepList` for the agent's plan inside the transcript, and `TaskProgress::by_agent()` for long
  agent jobs (batch edits, renders) that outlive one message.
- Give transcript rows ids (`UserMessage::id`, `AssistantMessage::id`) so they rise into place
  as they arrive.

## Code

- `CodeBlock::new(id, code).title("cargo test").copyable().line_numbers(1)` for command output
  and excerpts.
- `DiffBlock::new(path).lines([DiffLine::Hunk(..), DiffLine::Removed(n, ..), DiffLine::Added(n,
  ..), DiffLine::Context(n, ..)])` for a file change; the header counts additions and removals.

## In the canvas

Draw AI-authored objects with the object's own colour plus a dashed `colors.agent` border and a
sparkle in their header, and offer Keep / Undo next to them (see `examples/showcase.rs`, the
"Assistant suggestions" strip and the dashed clips).
