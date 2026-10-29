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
        .action(CopyButton::new("copy", markdownish.clone())))
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
then your own `.control(..)`s before Send: the reasoning effort as a small `SegmentedControl`,
fast mode as a small latching `Button::icon_only(id, IconName::Bolt).selected(fast)`, a voice
picker as a small ghost button with a chevron. Keep them small and unfilled so the row stays one
line and Send is the only filled control in it. Do not rebuild the frame to add a control. Send
and Stop already follow the composer's corner (`Theme::concentric(radius_lg, spacing.sm)`); a
button you place in another rounded surface's corner does the same with `Button::radius(..)`.

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
- Give transcript rows ids (`UserMessage::id`, `AssistantMessage::id`) so they fade in as
  they arrive; a `UserMessage` with an id also lets its text be selected and copied.

## Message actions

Put a message's actions — Copy, Retry, Edit, "Ask for more" — in its footer with `.action(..)`,
on `AssistantMessage` and `UserMessage` alike: small ghost icon buttons, Copy first, then the
commands that start another turn. They stay **visible**, not hover-only, so Tab finds them and
a touch screen shows them; a streaming answer hides its footer until it is done. Copy is always
a `CopyButton` (`CopyButton::new(id, text)`), which shows a check and "Copied" for a moment; on a
card offering a candidate (a title, a concept), give it a label: `CopyButton::new(id,
title).label("Copy title").copied_label("コピーしました")`.

A message's own text is selectable: `UserMessage` with an id draws it with `SelectableText`;
pass your Markdown view to `AssistantMessage::body` and make it selectable there.

## Previewing an attachment

A `ContextChip` that opens shows its attachment in a `Card` over the transcript or above the
composer: the file's name as the title with a ghost `Button::icon_only(id, IconName::Close)` at
the trailing end, then the content — text in `SelectableText::mono()`, an image fitted inside,
audio in an `AudioPlayer`. Cap it at about half the panel's height and scroll inside. While it
loads show a `Skeleton`; when it fails, a `ChatNotice::error` saying what to do.

## Completions in the composer

Slash commands, model names and mentions complete in a `Completions` list over the composer:

```rust
Composer::new("composer").input(input).completions(
    Completions::new("slash")
        .items(matches.iter().map(|m| Completion::new(m.name).description(m.help).matched(m.hits)))
        .selected(Some(self.completion))
        .on_select(cx.listener(|this, index, window, cx| this.accept(*index, window, cx))),
)
```

Focus stays in the input. From there Up and Down move `selected` (the list keeps it in view),
Tab accepts it, Enter accepts it only while the list is open and otherwise sends, and Escape
closes the list and leaves the text. Show it as soon as the trigger (`/`, `@`) is typed, filter
fuzzily as the person types, and draw the matched letters with `.matched(..)`. Say so when
nothing matches (`.empty("No matching command")`) rather than hiding the list mid-word.

## Emoji in messages

Type emoji as shortcodes, as in Slack and GitHub: `:thi` opens a list of matching emoji over
the composer, and accepting one puts 🤔 where `:thi` was. `atelier::emoji` does everything but
the editing:

```rust
let query = emoji::shortcode_at(&text, caret); // the `:thin` before the caret, if any
let matches = query.as_ref().map(|q| emoji::search(&q.query, 8)).unwrap_or_default();
Composer::new("composer").input(input).when(!matches.is_empty(), |composer| {
    composer.completions(
        emoji::completions("emoji", &matches)
            .selected(Some(self.emoji))
            .on_select(cx.listener(|this, index, window, cx| this.accept_emoji(*index, window, cx))),
    )
})
// accepting: let (text, caret) = query.accept(&text, matches[index].emoji);
```

The keys are those of any completion list. A shortcode needs two letters after the colon and
does not open after a Latin letter or digit, so `10:30` and `https://` stay quiet, while
`いいね:+1` opens one. When the person types a closing colon, or before sending, run
`emoji::replace_shortcodes` so a shortcode finished by hand becomes its emoji too.

Emoji draw in colour from the platform's emoji font, which `ui_font` lists. On Linux, gpui 0.2
picks fallback fonts through cosmic-text, which tries DejaVu Sans, FreeFont and Noto Sans
Symbols before Noto Color Emoji: emoji those fonts also cover (many smileys, ❤️, ✅, ⚡) draw in
one colour there. Requesting Noto Color Emoji by name does not help: gpui drops a family with
no letter `m` from its font database. `emoji::color_font_available(cx)` says whether Noto
Color Emoji is installed at all.

## Code

- `CodeBlock::new(id, code).title("cargo test").copyable().line_numbers(1)` for command output
  and excerpts.
- `DiffBlock::new(path).lines([DiffLine::Hunk(..), DiffLine::Removed(n, ..), DiffLine::Added(n,
  ..), DiffLine::Context(n, ..)])` for a file change; the header counts additions and removals.

## In the canvas

Draw AI-authored objects with the object's own colour plus a dashed `colors.agent` border and a
sparkle in their header, and offer Keep / Undo next to them (see `examples/showcase.rs`, the
"Assistant suggestions" strip and the dashed clips).

## Short text the agent wrote

A name, a title or a one-line description the agent wrote — a cluster named by "Name with
AI", a take's title — shows that it is the agent's wherever it appears, in lists and trees as
well as where it was made: `AgentLabel::new(name)`, the text in its normal colour followed by a
small sparkle in `colors.agent`. Give it `.tooltip(id, "Named by AI — rename to keep your
own")`. The mark lasts until the person edits the text; then it is theirs and drawn as a plain
`Label`. Keep the original next to the record, so Undo can bring it back. Do not tint the text
itself or wash its row in `agent_wash`: in a list, only the mark differs.
