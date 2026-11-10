# Changelog

Each release lists what it adds and, under **Workarounds you can delete**, the code an
application wrote to cover a gap that the release closes. When you move the `atelier`
dependency to a newer revision, read the sections in between, delete what they list, and run
[`skills/atelier-gpui/tools/check-adoption.sh`](skills/atelier-gpui/tools/check-adoption.sh)
over your sources to find what is left. Add the script to the application's CI so new code
does not bring the old ways back.

Applications pin a release tag, `vX.Y.Z`, which matches the version in `Cargo.toml` (see
"Install" in the README). A `## vX.Y.Z` heading marks where a release was tagged: it contains
every section below it down to the previous one. Sections headed **Unreleased** above the
newest tag are on `main` but in no release yet; when a release is tagged, the version in
`Cargo.toml` goes up and its heading goes above them. A change that makes an application edit
its code bumps the minor version while Atelier is below 1.0.

## Unreleased: shortcuts in button tooltips

### Added

- `Button::shortcut_for(&action)`: the tooltip names the keys bound to the action, looked up
  in the keymap each time it opens, so a key the person rebinds shows as rebound.
  `Button::shortcut("⌘R")` takes the keys already written. A labelled button with a shortcut
  and no tooltip shows its label there.
- `binding_label(&KeyBinding)`: a binding as the platform writes it ("⇧⌘S", "Ctrl+Shift+S",
  chords with spaces).

### Changed

- `Tooltip::shortcut("")` shows no keys, so a command without one needs no branch.

### Web

- `data-shortcut` beside `data-tooltip` (gpui's notation such as `secondary-d`, or already
  written) gives the tooltip its keys, faint after the words, and sets `aria-keyshortcuts`;
  `shortcutTooltip(..)` and `shortcutLabel(..)` in `atelier.js`. The showcase's transport and
  settings buttons have them.

### Workarounds you can delete

- Auris Studio's `keyed_tip` and `tip` in `ui/tooltip.rs`, and the adapter around the native
  button that kept them.

## Unreleased: plots that are dragged

### Added

- `LinePlot::id(..)` with `.on_drag(..)`, `.on_reset(..)` and `.on_wheel(..)`: a plot that is
  the control for what it draws, such as a synth's envelope or filter. A drag reports
  `PlotDrag { phase, from, delta }` in the plot's own units, measured on the data area the plot
  really drew. Shift goes a tenth as far without a jump when it changes mid-drag. A
  double-click resets, and the wheel reports whole notches. The plot is then a tab stop whose
  arrows nudge it a hundredth of a range.

### Web

- Not ported: the web kit has no plots yet (listed under "Not yet" in `web/README.md`).

### Workarounds you can delete

- Auris Studio's `synth_plot_well`, `press_synth_plot`, `drag_synth_plot` and the `GUTTER`
  guess at the plot's data area in `ui/synth_edit.rs`; keep only the mapping from delta to
  parameters.

## Unreleased: meters that read dB and hold a clip

### Added

- `LevelMeter::clipped(bool)`: a clip light at the hot end, latched by the application until
  the person clears it.
- `LevelMeter::db(db)`, `.peak_db(db)` and `LevelMeter::position(db)`: a dBFS scale with room
  for the working range. Green runs to −6 dB, yellow to full scale, red only over it, and the
  floor is `METER_FLOOR_DB` (−60). −∞ and NaN read as an empty meter.

### Changed

- A vertical `LevelMeter` fills the height its parent is stretched to (a horizontal one the
  width), where it used to need a parent with a set height. It is never shorter than 16 px.
  A peak at zero is no longer drawn.

### Web

- `.at-meter` takes `--level` and `--peak`, a `.at-meter__peak` child, `data-clipped` and
  `.at-meter--horizontal`; `setMeter(..)` and `meterPosition(db)` in `atelier.js`. The span
  fill with `--_full` still works.

### Workarounds you can delete

- Auris Studio's `level_meter` and `db_to_meter_position` in `ui/widgets.rs`, the clip light
  drawn over a meter, and the absolute top/bottom pinning around the visualizer's meters.

## Unreleased: figures

### Added

- `Stat`: one figure read at a glance, a faint caption over a large mono number and its unit,
  as Auris Studio's loudness panel and typing-panel readouts draw by hand. `Stat::empty` shows
  a dash before anything is measured; `.size(..)`, `.color(..)` for a figure that is a verdict
  too, `.inline()` for the one-line form in a title bar, `.min_width(..)` and `.end()`.

### Web

- `.at-stat` with `__caption`, `__figure`, `__value` and `__unit`; an empty value shows the
  dash. Sizes `--display`, `--md`, `--sm`, `--inline`, `--end` and `data-tone`. The showcase
  has a row of loudness figures.

### Workarounds you can delete

- Auris Studio's `labelled_figure` in `ui/visualizer.rs` and `readout` in
  `ui/typing_panel.rs`.

## Unreleased: following the playhead

### Added

- `Follow`: a view that plays pages along so the playhead stays in view, and lets go when the
  person scrolls away. Call `update(offset, span, at, playing)` each frame, in any one unit;
  it returns the offset of the next page turn. A scroll that hides the playhead releases it, a
  zoom does not; it follows again once the playhead is back in view, playback starts again, or
  `engage()` is called. `side(..)` says which way the playhead went while released.
- `BackToPlayhead`: the button that takes a released view back, floating in the bottom corner
  on the playhead's side and pointing that way. `Strings::back_to_playhead` ("Back to
  playhead", "再生位置に戻る").

### Web

- `follow()` in `atelier.js`, with the same `update`, `engage`, `page` and `side`, and
  `.at-back-to-playhead[data-side]`. The showcase has a strip that plays and can be scrolled
  away from.

### Workarounds you can delete

- Auris Studio's `PlayheadFollow` in `ui/timeline.rs`, and its return-to-playhead chip in
  `ui/lanes.rs`.

## Unreleased: tables

### Added

- `Table`: rows of short values under column headings, such as Auris Studio's MIDI event
  list. `Column`s are fixed (`.width(..)`) or share the free width (`.grow(..)`), with `.end()`
  for numbers and `.mono()` for digits that line up. A `TableRow` can carry a stripe down its
  leading edge (`RowMark::Category(n)` from the theme's categorical colours, `Accent`,
  `Agent`), `.tinted()` text in the same colour, `.active(..)` for the row being heard and
  `.faint(..)` for one past the clip's end. Only the rows on screen are built. A click selects
  (`on_select`); a double-click on a cell, or Enter on the selected row, asks to edit it
  (`on_activate` with a `TableCell`); while the table has focus, Up, Down, Home and End move the
  selection and keep it in view, taken before any key binding (`LIST_CONTEXT`, `SelectNext` and
  the rest). `Table::spots(&book)` records every visible cell under `Table::cell_key`, so an
  `InlineEdit` opens over the cell.
- `InlineEdit::on_return(..)`: Return hands over the text (or `None` when nothing changed)
  instead of committing, for a list editor whose Return walks on to the same cell of the next
  row. A field that moves on to another label opens there afresh, focused, with everything
  selected.

### Web

- `table(element, { columns, rows, empty })` in `atelier.js`, returning `update`, `select` and
  `cell`; `atelier:row-select` and `atelier:cell-activate` (with the cell's element, for
  `inlineEdit`); `.at-table` with its heading, stripes and the row being heard. The showcase
  has an event list whose cells edit in place and whose Return walks down the column.

### Workarounds you can delete

- A hand-built list editor table: Auris Studio's `ui/event_list.rs` heading, rows,
  stripes, sounding tint and cell spots.

## Unreleased: text typed where it is shown

### Added

- `TextInput`: editable text, the editing a `TextField` frame was waiting for. A caret and a
  selection (drag, Shift, double-click for a word, triple-click for all), the platform's input
  method, so Japanese and other compositions work, clipboard, and undo that takes back a typed
  run as one step. Return emits `TextInputEvent::Submit` and bubbles the `SubmitText` action,
  Escape `Cancel` and `CancelText`; `.multiline()` keeps line breaks, Shift-Return makes one.
  While an input holds focus, Atelier takes its editing keys (Space, Backspace, Delete, the
  arrows, Home, End, Return, Escape and ⌘/Ctrl with A, C, X, V, Z) before any key binding, so
  Space for play or Backspace for delete never fires while the person types; scope a
  single-letter shortcut away with `Some("!AtelierTextInput")` (`TEXT_INPUT_CONTEXT`).
  `TextBuffer` is the same editing without drawing, for tests.
- `InlineEdit`: rename a track, a clip, a lyric or a chord over its own label instead of in a
  sheet, as Auris Studio does. The field opens at the label's place (150 to 420 px wide) with
  the text selected; Return commits, Escape closes, and a press elsewhere commits unless the
  text is unchanged or emptied, so wandering off never writes an edit nobody made.
  `.error(..)` keeps it open with the reason, and focus goes back where it was when it closes.
  `SpotBook::spot` records where a label laid out by flex was painted, and `SpotBook::get`
  answers only while it is still on screen, so the application can fall back to its sheet.
- `TextField::area()`: the well for several lines.

### Web

- `inlineEdit(label, options)` in `atelier.js` and `.at-inline-edit`: the same field over a
  label, with the same Return, Escape and press-elsewhere rules; it leaves Return alone while an
  input method composes. The showcase renames a track in place.

### Workarounds you can delete

- An editable text field of the application's own (an `EntityInputHandler` with caret, selection
  and IME plumbing): Auris Studio's `ui/text_field.rs` and `ui/text_area.rs` for one-line and
  short multi-line text.
- Placing a prompt over a label by hand: Auris Studio's `ui/inline_edit.rs` (`Spots`, `spot`,
  `InlinePlace::Spot`, `fitted`) and the click-away rules in `ui/prompt.rs`.
- Bindings scoped to keep Space or Backspace from firing while a field has focus.

## Unreleased: Markdown as tall as it is drawn

### Fixed

- `Markdown` (and so `AssistantMessage` and `ChatView`) stacks its blocks, list items and
  quotes as block containers rather than flex columns. As flex columns they could take their
  height from a measurement made before the width was known, when text wraps at every break
  opportunity: Japanese breaks between any two characters, so in Auris Studio's song workshop
  (a reading column centred across a row in a scrolling body) a five-line answer stood more
  than 2,000 px tall, and the cards after it fell past the end of the scroll range. The blocks
  keep their spacing, now as a top margin on each block after the first.
- A `UserMessage`'s attachment chips stay inside the message's column. The row holding them
  took the width of its chips, so in a side panel narrower than one chip's 240 px a file's
  chip reached past the panel's leading edge instead of cutting its name short. (The web kit's
  user message carries no chips, so it is unchanged.)

### Workarounds you can delete

- A Markdown renderer of the application's own, kept only because `Markdown` collapsed or
  grew in a centred column: Auris Studio's `agent_markdown.rs`.

## v0.4.0

A workspace: alongside `atelier`, the `atelier-agents` crate drives the coding-agent CLIs
(Claude Code, Codex, OpenCode, Antigravity) without a UI, `atelier-agents-mock` stands in for
them in tests, and `atelier-i18n` keeps an application's words in one file per language.
`ChatView` shows a CLI's session behind the `agents` feature, and the CLIs report how much of
the plan is left. Each crate's version is 0.4.0 and they are pinned with the same tag.

## Interface text in language files

### Added

- `atelier-i18n`, a crate of its own with no gpui, for an application's words. They live in
  `locales/<tag>.toml`, one file per language, and the `build` feature's
  `build::Catalog::generate`, called from a build script, writes a `Language` enum, a `Key`
  enum of the fixed strings and a `messages` function for each string that takes values.
  A key one language lacks, or a placeholder one translation names and another does not,
  fails the build with the file and the key. The runtime half picks the language:
  `preferred_locales` reads the locale variables and then the operating system (Windows and
  an application macOS starts from Finder set no variables), and `negotiate` matches a locale
  to the build's languages. See [crates/atelier-i18n](crates/atelier-i18n/README.md).
  `Strings`, the words the components draw, is unchanged.

## What is left of the plan

### Added

- `atelier_agents::AgentEvent::RateLimits` reports how much of the plan a CLI's sign-in has
  spent, as `RateLimit`s (five-hour window, week, a model's weekly share; whole percent used,
  reset time, whether it is reached). Claude Code reports it from `rate_limit_event`, Codex
  from `account/rateLimits/read` at the handshake and `account/rateLimits/updated` after.
  OpenCode, Antigravity and API-key sign-ins report none.
- `agent::AgentFeed::rate_limits` and `agent::AgentConnection::rate_limits` keep the limits as
  last reported. `AgentConnection::usage` builds the composer's `UsageMeter` from them, and
  `agent::usage_meter` and `agent::usage_limit` do the same for an application with its own
  loop.
- `Strings::usage_windows`, `usage_window_hours`, `usage_window_days` and `usage_in` name a
  limit's window and say when it resets ("Weekly", "in 2 h 10 min"; "週間枠", "2時間10分後"),
  with `Strings::usage_window` and `Strings::usage_in` putting the numbers in.
- The mock's scenarios take `"limits"` so that its Claude Code and Codex stand-ins report a
  plan that drains turn by turn. The `agent_cli` example shows it in the composer.

### Workarounds you can delete

- Reading the plan's usage some other way, such as calling a provider's usage endpoint or
  scraping the CLI's `/usage` output, to feed a `UsageMeter`.

## An agent CLI in the conversation view

### Added

- The `agents` feature, which brings in `atelier-agents` (re-exported as `atelier::agents`) and
  adds two parts:
  - `agent::AgentFeed` turns `AgentEvent`s into a `ChatView`'s entries: messages, streamed
    answers and reasoning, tool calls with their input and output, approval cards, errors and
    the thinking indicator's phase word. `AgentFeed::decision` maps an answered card back to
    its request.
  - `agent::AgentConnection` runs the whole loop. It starts the CLI, feeds its events to the
    chat on the UI thread and answers approvals from `ChatEvent::Decided`. Messages sent during
    a turn are held and go in order as turns end. It also exposes `interrupt`, `session_id`,
    `models` and `pending`.
- Example `agent_cli`: a real Claude Code, Codex, OpenCode or Antigravity CLI in the
  conversation, or with `--mock` the `atelier-agents-mock` stand-in, so it runs without an
  account.

### Workarounds you can delete

- The code that turns an agent's stream into `ChatEntry`s by hand: Maestro's transcript
  mapping, and Auris Studio's for the CLI backends once they move to `atelier-agents`.

## What Auris Studio needs from the agent CLIs

### Added

- `AgentConfig` grows the settings Auris Studio's backends use:
  - `mcp_servers`: HTTP MCP servers with a bearer token and `auto_approve`. They reach Claude
    Code, Codex and OpenCode, and the token never appears in the process's arguments.
  - `session`: `New`, `Ephemeral` or `Resume(id)`, with `AgentEvent::SessionStarted { id }` to
    keep. A session the CLI no longer has ends in `AgentEvent::ResumeFailed` (or, for Claude
    Code, an exit before `SessionStarted`) with no prompt sent.
  - `system_prompt` replaces the CLI's own (Claude Code, Codex).
  - `native_tools: false` leaves the agent no shell or file tools.
  - `fast` picks the fast or standard service tier (Claude Code, Codex).
  - `isolated` leaves the user's hooks, plugins, skills, slash commands, memories and project
    instruction files out of the run, and on Claude Code their MCP servers too.
  - `scratch` is a private directory for the files that keep tokens out of the process list.
- `Adapter::files()` names those files, and `AgentSession::spawn` writes them before starting
  the CLI: owner-only on Unix.
- `AgentEvent::Models` from each CLI's handshake (Claude Code's `initialize`, Codex's
  `model/list`, OpenCode's `session/new`), and `list_models(config, timeout)` to ask without
  starting a conversation. An empty first prompt now runs the handshake alone.
- `ProviderKind::locate()` finds a CLI on `PATH` or in `~/.local/bin`, only as a native `.exe`
  on Windows. `ProviderStatus::check()` adds the CLI's `--version`.
- `Effort::Minimal` and `Effort::XHigh`.
- The mock lists models (`atelier_agents_mock::cli::MODELS`), keeps and resumes sessions, and
  fails a resume of any id starting with `missing`, so applications can test their fallback.

### Workarounds you can delete

- Auris Studio: the process and protocol halves of `auris-agent`'s Claude Code, Codex and
  OpenCode backends: command lines, MCP configuration files, session start and resume, model
  listing and executable lookup. Its own policy stays where it is: the tool-failure budget,
  the memory hand-off, the live tools, the ChatGPT-account check for Codex, and turning off
  Codex's inherited MCP servers (which needs a `config/read` and a restart).

## Coding agents without a UI

### Added

- `atelier-agents`, a second crate in this repository: Claude Code (`stream-json`), Codex
  (`app-server`), OpenCode (Agent Client Protocol) and Antigravity CLI (`stream-json`) behind one
  `Adapter` trait and one `AgentEvent` stream (text, reasoning, tool start and finish,
  permission requests, turn completed or failed, exit). Adapters do no I/O, so they work over a
  local pipe, `ssh` or an async runtime alike; `AgentSession` runs one on two threads for the
  common case. `CommandSpec::shell_line` gives the quoted line for running an agent on another
  machine. It needs no gpui. Ported from Maestro's `maestro-agents`.
- `atelier-agents-mock`: a fake CLI that speaks all four wire formats from a JSON scenario
  (say, think, tool calls with or without asking, pauses, failures, crashes, noise), an
  `install DIR` that puts `claude`, `codex`, `agy` and `opencode` on a test's `PATH`, and an
  OpenAI-compatible server driven by the same scenario. Its tests run every adapter through a
  real process.

### Workarounds you can delete

- Maestro: `maestro-agents` and `maestro-mock` (`ProviderKind`, `Effort`, `AgentEvent`,
  `ToolCall`, `PermissionRequest` move with them; `WorkerConfig` is `AgentConfig` with a
  `client` name, and `HostCommand` stays in `maestro-host`, built from `Adapter::command()` with
  `AgentSession::spawn_in`).

## v0.3.0

The first tagged release: Atelier as a library, with the shared agent parts (the
conversation view, approvals, activity, plan and changes, the node canvas, the preferences
window, questions, the message queue, the conversation list and Markdown).

## Usable as a library

### Added

- Release tags. The first will be `v0.3.0`, and the crate's version is 0.3.0 to match;
  applications that pinned a revision (Auris Studio pins `b584b12`) can move to it once it is
  tagged. The README's Install section shows the dependency line, how to upgrade
  (`cargo update -p atelier`) and how to build against a local checkout with `[patch]`.
- `atelier::prelude`: `use atelier::prelude::*` brings in the theme and its tokens,
  `ActiveTheme` and `ActiveStrings`, `Assets` and the everyday components (buttons, labels,
  icons, form controls, layout, popovers, the window frame). Specialised parts stay in their
  modules.
- `atelier::gpui` re-exports the gpui Atelier is built against.
- Cargo features. `gpu` (default) is the wgpu shading of the liquid metal, ambient backdrops and
  3-D plots; with `default-features = false` wgpu is not built and they draw on the CPU, as they
  already did without a GPU. `runtime-shaders` turns on gpui's `runtime_shaders` for Macs
  without full Xcode.

## Questions, the queue, the conversation list and Markdown

### Added

- `atelier::Markdown`, Markdown drawn in Atelier's type: paragraphs, headings, bullet, numbered
  and task lists (nested), quotes, fenced code with its language and a copy button, GitHub
  tables, rules, bold, italic, struck text, inline code and links. Each block is selectable and
  copyable (`{id}-block-N`); a click on a link opens it or goes to `.on_link(..)`. Raw HTML stays
  text. `.plain()` draws it without selection. It reads CommonMark with `pulldown-cmark`, a new
  dependency (no default features).
- `AssistantMessage` and `ChatView` draw answers as `Markdown`, which used to understand only
  paragraphs, `-` bullets, `#` headings and fenced code.
- `SelectableText::styles(..)` for bold, italic or coloured runs, and `::link(range, url)` with
  `::on_link(..)` for links a click (not a drag) follows.
- `agent::ChoiceCard` and `Choice`: a question the agent asked with fixed answers, answered with
  one press, or with checkboxes and Send when `.multiple(true)`; `.answered(..)` shows the pick.
  In a `ChatView`, `ChatEntry::choices(question, options)` (`.multiple()`) emits the new
  `ChatEvent::Chose { entry, picked }`; a message sent after the question closes it.
  `ChatView::choices_open(id)` and `::choose(id, picked)`.
- `agent::MessageQueue` and `QueuedMessage`: the messages waiting for the turn to end, with a
  count, pause and resume, and per message "Send now", "Edit" and remove.
- `agent::ConversationList` and `Conversation`: "New conversation", a search field the
  application passes, Recent and Archived, and a row per conversation with its state, when it
  was used, a line of its last message and a "More actions" button. `ConversationRow::preview(..)`
  and `::on_more(..)` add the second line and the button to a row of your own.
- `Strings::pick_any`, `queued`, `queue_paused`, `resume`, `send_now`, `send_now_hint`, `edit`,
  `new_conversation`, `untitled`, `recent`, `archived`, `no_conversations`,
  `search_conversations` and `more_actions`, in English and Japanese, and `Strings::queued(n,
  paused)`.
- The `agent_desktop` example's sidebar is a `ConversationList` (`--search piano`); `--ask
  single|multiple` ends the turn on a question, `--queue` fills the queue, and its answer shows
  a table, a numbered list and a link.
- Web kit: `markdown(..)`, `choiceCard(..)`, `messageQueue(..)` and `conversationList(..)` in
  `atelier.js` with `.at-markdown`, `.at-choice-card`, `.at-queue` and `.at-conversations` in
  `atelier.css`. `chat()` draws answers as Markdown and takes a `choices` entry, firing
  `atelier:chose`. The showcase's agent view has the conversation list in its sidebar, the queue
  over the composer, and `?ask=single` or `?ask=multiple`.

### Workarounds you can delete

| Workaround | Use instead |
| --- | --- |
| A Markdown view from another crate (gpui-component's `TextView`) passed with `AssistantMessage::body` | Nothing: answers are `Markdown` now; `atelier::Markdown` elsewhere |
| A choice card built from `ListItem`s and `Checkbox`es, or pushed as `ChatEntry::custom` | `ChatEntry::choices` in a `ChatView`, or `agent::ChoiceCard` |
| A queued-messages strip with its own Send now, Edit and remove buttons | `agent::MessageQueue` |
| A conversation browser of hand-built rows with a search and Recent/Archived buttons | `agent::ConversationList` |

## The preferences window

### Added

- `PreferencesWindow`, the settings window Auris Studio's `settings_window.rs` builds by hand:
  pages in a sidebar under a search field, the open page as a header and groups of settings in
  a readable column, the page's actions at its foot (the last one is the primary, at the
  trailing end), an optional sidebar footer and status bar. It holds the open page itself
  unless you pass `.selected(..)` and `.on_select(..)`. gpui has no text input, so the
  application passes its search field and query with `.search(query, field)`; with a query the
  window lists every page's matching settings under a link to the page, with a count, or says
  nothing matches.
- `SettingsPage` (`.icon`, `.description`, `.keywords`, `.group`, `.child`), `SettingsGroup`
  (`new`, `untitled`, `.description`, `.setting`) and `Setting`, a `FormField` that search can
  find by its label, description or `.keywords([..])`. Every word of a query must match; a
  group title, or a page's title or keywords, brings in all of it.
- `KeybindingList` and `KeyBinding`: shortcuts in groups as key chips, with a scope, a reset
  for a changed binding and a note when two commands share a key. `.recording(Some((row,
  slot)))` arms a chip and the next key press arrives at `.on_capture(..)` (Escape gives
  `None`). `keystroke_label(..)` prints a keystroke as menus do: "⌘⇧S" on macOS,
  "Ctrl+Shift+S" elsewhere.
- `Strings::search_settings`, `nothing_matches`, `settings_found`, `press_a_key`,
  `add_shortcut`, `reset_shortcut`, `no_shortcut` and `also_runs`, in English and Japanese.
- The `preferences` example draws its window with `PreferencesWindow`, takes typed search
  (`--search 保存`) and has a working keys page.
- Web kit: `preferences(..)`, `keybindings(..)` and `keystrokeLabel(..)` in `atelier.js`, with
  `.at-preferences`, `.at-form-group` and `.at-keybinding` in `atelier.css`, and a preferences
  sample on the showcase's components page. An input inside `.at-field` no longer draws a
  second focus ring inside the field's own.

### Workarounds you can delete

| Workaround | Use instead |
| --- | --- |
| A hand-built settings window: sidebar of pages, search over a list of settings, page header, actions row | `PreferencesWindow` with `SettingsPage`, `SettingsGroup`, `Setting` |
| A key-binding table with its own recording state and keystroke formatting | `KeybindingList`, `KeyBinding`, `keystroke_label` |

## The node canvas

### Added

- `atelier::graph::NodeCanvas`, a node graph on a pannable canvas, taken out of the
  `node_editor` example. It is a view the application holds: it keeps the nodes, wires,
  selection and pan, and reports what the person did as `NodeCanvasEvent`s (`Selected`, `Moved`,
  `Connected`, `Disconnected`, `Removed`, `Kept`, `Undone`), already applied. The person drags
  nodes by their cards, draws a wire from an output to an input of the same type (others step
  back while it is held), picks a wire up by its input, pans with the background, the middle
  button or the wheel, deletes the selected node with Delete, and keeps or undoes a node the
  assistant added. `.port_types([..])` names the types for the tooltips and the legend; their
  colours are `colors.ports`.
- `graph::GraphNode` (`.at(..)`, `.width(..)`, `.icon(..)`, `.input(..)`, `.output(..)`,
  `.field(..)`, `.state(..)`, `.by_agent(..)`, `.body(..)`), `NodeField` (`number`, `choice`,
  `text`), `NodeState` (`Idle`, `Queued`, `Running`, `Done`, `Cached`, `Failed`), `Port`,
  `Wire` and `NodeId`. The canvas answers `execution_order()`, `downstream(id)`,
  `source(id, input)`, `unconnected_inputs()` and `can_connect(wire)`.
- `Strings::added_by_agent`, `keep`, `undo`, `reset_view` and `cached`, in English and Japanese.
- The `node_editor` example now draws its graph with `NodeCanvas` and keeps only its node kinds,
  parameters, run and inspector.
- Web kit: `nodeCanvas(..)` in `atelier.js` with `.at-node-canvas` and `.at-node` in
  `atelier.css`, firing `atelier:node-*` events. The web showcase has a node canvas on its
  components page. The node editor is no longer listed as not yet ported.

### Workarounds you can delete

| Workaround | Use instead |
| --- | --- |
| A graph copied from `examples/node_editor.rs`: its `Drag`, `Geometry`, socket and wire painting, `bezier`, `execution_order` | `graph::NodeCanvas` |

## What the agent is doing, its plan and its changes

### Added

- `agent::AgentActivity`, one line of what a running turn is doing: its `AgentPhase`
  (`Waiting`, `Reading`, `Thinking`, `Writing`, `Tool`, `Approval`, `Compacting`, `Reviewing`)
  and `.detail(..)`, then whichever figures the backend reports: `.progress(..)` through a
  phase, `.tokens(..)`, `.rate(..)` in tokens per second, `.elapsed(..)` and `.context(..)` as
  a ring that warns past 80 % and alarms past 95 %. The phase leads; the figures give way first.
- `agent::PlanView`, the agent's plan: a header with how many steps are done and a thin bar
  over a `StepList`. Pressing the header folds it to the step in progress (or the next one);
  it holds that itself unless you pass `.open(..)` and `.on_toggle(..)`.
- `agent::ChangedFiles` and `FileChange` (`ChangeKind::Added`, `Modified`, `Deleted`,
  `Renamed`): the files a turn changed, name before folder, lines added and removed per file
  and in total, `.action(..)` in the header for "Keep all", `.on_open(|index, ..|)` per row.
- `Strings::agent_phases`, `plan` and `changed_files`, in English and Japanese.
- The `agent_desktop` example shows the activity line above the composer (it follows the
  approval: needs approval, then running the tests; `--activity tool|thinking|compacting`
  starts elsewhere), and its changes pane is a `PlanView` over `ChangedFiles`.
- Web kit: `activity(..)`, `planView(..)` and `changedFiles(..)` in `atelier.js`, with
  `.at-activity`, `.at-plan`, `.at-steps`/`.at-step` and `.at-changes`/`.at-change` in
  `atelier.css`. A plan in static markup folds through `hydrate`. The web showcase's agent view
  has all three.

### Workarounds you can delete

| Workaround | Use instead |
| --- | --- |
| A status row of phase, tok/s, elapsed time and a context gauge laid out by hand | `AgentActivity` |
| A plan drawn as a bare `StepList` with its own header and count | `PlanView` |
| Changed files as `ListItem`s with `+n`/`−n` labels in the trailing slot | `ChangedFiles` |

## Dialogs, approvals and permission modes

### Added

- `Dialog`, a modal dialog: a card over a scrim that dims the whole window, drawn from anywhere
  while it is open. It takes focus when it appears (`.initial_focus(&handle)` for the safe
  choice), and Escape or a press on the scrim calls `.on_dismiss(..)`. `.icon(icon, tone)`,
  `.body(..)`, `.child(..)`, `.action(..)`, `.leading_action(..)` and `.width(..)` build it.
- `Colors::scrim`, what a modal lays over the window; `--at-scrim` on the web.
- `agent::ApprovalDialog`, an approval as a modal for a request that must stop everything:
  `.command(..)` shows the exact command, `.detail(..)` a diff, `.reviewing(true)` says an
  automatic reviewer is deciding and leaves only Deny pressable. Deny takes focus, so Enter
  refuses, and Escape or a press outside denies. `.on_decide(|decision: &Decision, ..|)` hears
  the answer, the same `Decision` a `ChatView` approval reports.
- `agent::PermissionMode` (`Plan`, `Ask`, `AcceptEdits`, `AutoReview`, `Bypass`, with `label`,
  `detail`, `icon` and `warns`) and `agent::PermissionPicker`, the mode as a small button for
  the composer row that opens a menu of the modes upwards, each with what it lets the agent
  do. `Bypass` keeps the button in the danger colour for as long as it is chosen. Without
  `.on_select(..)` it keeps the chosen mode itself.
- `MenuItem::description(..)`, a second line saying what a choice does.
- `Strings::reviewing`, `permissions`, `permission_modes` and `permission_details`, in English
  and Japanese.
- The `agent_desktop` example has a permission picker in its composer (the status bar follows
  it), and once the tests are allowed the mock agent asks to push in an `ApprovalDialog`
  (`--push-request` opens it at start, `--permission bypass` starts in bypass). The showcase's
  agent pieces open both dialogs (`--dialog delete|approval`) and show two pickers.
- Web kit: `openDialog(..)`, `approvalDialog(..)` and `permissionPicker(..)` in `atelier.js`,
  with `dialog.at-dialog`, `.at-permission` and described menu items in `atelier.css`. The web
  showcase has them on its components page and in the agent composer.

### Workarounds you can delete

| Workaround | Use instead |
| --- | --- |
| A full-window overlay with an occluding scrim and a centred card | `Dialog` |
| Allow-once / Deny / Always buttons laid out by hand around a pending tool call | `ApprovalDialog` or `ChatEntry::approval` |
| An application enum of permission modes with its own menu, and a warning drawn while bypass is chosen | `PermissionMode` and `PermissionPicker` |

## The conversation view

### Added

- `agent::ChatView`, the conversation as a view the application holds (`cx.new(|_|
  ChatView::new("chat"))`). It keeps the entries (`ChatEntry::user`, `assistant`, `streaming`,
  `reasoning`, `tool`, `approval`, `status`, `error`, `custom`), which tool calls and reasoning
  are open, and where the person is reading: it follows the end while they are there, stays
  put once they scroll up, and counts what arrives meanwhile on a "jump to latest" button.
  `push` returns an `EntryId`; `append` streams text into an answer, reasoning or a tool's
  detail; `finish`, `set_tool_state`, `answer`, `update_entry`, `replace`, `clear`, `remove`,
  `jump_to_latest`, `set_thinking` and `set_logo` change it; `transcript_text` reads it out.
  Consecutive tool calls are grouped and counted; an answered approval folds into one line and
  emits `ChatEvent::Decided { entry, decision }`. `reading_width(..)` centres it for the middle
  of a window, `capacity(..)` keeps the last entries.
- `AgentPanel::chat(view)` puts a `ChatView` where the panel's transcript goes.
- `Strings::jump_to_latest`, `new_messages`, `used_tools` and `approval_answers`, in English and
  Japanese.
- The `agent_desktop` example's conversation is a `ChatView`: approving or denying the tests
  folds the card and the mock agent answers, and sent messages follow the end.
- Web kit: `chat(element, options)` in `atelier.js`, the same view for the browser and Tauri,
  with `.at-chat`, `.at-chat__jump`, `.at-message__actions`, `.at-approval--answered` and
  `.at-notice` in `atelier.css`. The showcase's Agent view is driven by it.

### Workarounds you can delete

| Workaround | Use instead |
| --- | --- |
| A transcript `ScrollHandle` with "near the tail" checks, an unread count and a jump button | `ChatView`, which follows and counts by itself |
| An application enum of chat rows with its own open/closed sets for tool calls and reasoning | `ChatEntry` and `ChatView::set_open` |
| Approval buttons wired to a pending-request field and a folded "allowed" line drawn by hand | `ChatEntry::approval(..)` and `ChatEvent::Decided` |

## Controls that move when used

### Added

- `Slider` and `Knob` take an `.id(..)` and then move when used: a click or drag along the
  slider's track, a drag up or to the right on the knob (200 px for the whole range), the arrow
  keys, Home and End once Tab reaches them, and a double click back to `.default_value(..)` (the
  centre when bipolar). `.on_change(|value: &f32, window, cx| ..)` hears the new value,
  `.step(..)` sets an arrow key's worth (0.01), `.readout_with(|value| ..)` formats the readout
  from the value shown, and `.disabled(true)` greys them out. Without an id they only show
  their value, as before.
- Every control works as it is dropped in. `Switch`, `Checkbox`, `SegmentedControl`, `Tabs`,
  `Slider`, `Knob` and `AudioPlayer` given no handler keep the value they are moved to for
  themselves, starting from the one they were built with and taking the application's value
  again whenever it changes. Give them a handler and they show only what the application passes,
  as before.
- The examples' sliders, knobs and switches can all be used: the showcase's volume, pan, input
  monitoring and EQ (one set per track), the instrument lab's exciter, string and body knobs,
  the plot lab's oscillator and gas controls (its azimuth and elevation turn the plot), the
  control legend's samples and the node editor's number parameters, whose readouts follow.
- Web kit: `.at-knob`, the knob, drawn and turned by `hydrate()` (drag, arrow keys, Home, End,
  double click to `data-default`, `input`/`change` events and `.value`), and
  `input.at-slider--bipolar`, which fills from the centre. A slider with `data-default` goes back
  to it on a double click. The showcase's sliders and knobs move, with live readouts.

### Changed

- A `Switch`, `Checkbox`, `SegmentedControl` or `Tabs` drawn with no handler used to ignore
  clicks; it now flips or selects by itself. Pass `.disabled(true)` for one that must not move.

### Workarounds you can delete

| Workaround | Use instead |
| --- | --- |
| An element around a `Slider` or `Knob` that tracks the drag, the arrow keys or a double click and sets the value | `.id(..)` and `.on_change(..)` on the control itself |
| A readout string formatted from the value at each call site | `.readout_with(|value| ..)` |

## Screenshots retaken, knob icon redrawn

### Changed

- `IconName::Knob` is redrawn as a knob under a scale arc, with its pointer reaching the rim. The
  old feet and top tick read as an alarm clock.
- Web kit: `icons.js` carries the redrawn knob.
- The screenshots in `doc/` and `web/README.md` show the current kit, taken on Linux under
  Weston, and the node editor has one.

## Wrench icon redrawn

### Fixed

- `IconName::Wrench` was a damaged path that drew a crescent with a stray notch instead of a
  wrench. It is redrawn as an open-ended wrench in the set's 2 px outline style, centred in the
  24 px grid. The web kit's `wrench` icon is regenerated from the same file. Every other icon,
  provider mark and the Auris Studio app icon was rendered at 10–28 px (1x and 2x) through
  gpui's resvg path and in Chromium and came out intact, with no difference between the two.

## Fewer AI-template tells

### Changed

- `Label::caption` (and so `SectionHeader::caption`) is no longer set in capitals. It is the
  text as written, one step under body text (`TextSize::Small`), muted, medium weight. Pass
  captions in sentence case; one written in capitals for the old look now reads as shouting.
- `Strings::ask_anything`, the composer's placeholder, reads "Ask the assistant…" ("アシスタントに
  質問…" in Japanese) instead of the stock "Ask anything…". The field keeps its name.
- The design rules' Copy section now spells out sentence-case section labels, no exclamation
  marks, short placeholders, welcome screens that name the product instead of greeting, and no
  emoji in the chrome.
- Web kit: `.at-caption` follows `Label::caption` (sentence case, `--at-size-sm`, muted, 500),
  and `.at-title` drops its negative letter spacing, which the gpui kit never had. The
  showcase's welcome screen matches the native one: the product name and what it is for, Open…
  beside New project, with no gradient sparkle tile and no "Welcome to" headline.

## Focus handed in (Auris feedback round 6)

### Added

- `Button::track_focus(&FocusHandle)` and `EffortButton::track_focus(..)`: the button tracks
  the application's handle instead of one of its own, so `window.focus(&handle)` can move focus
  there, for example back to the effort button once Escape closes its picker. The handle
  becomes a tab stop.
- `EffortPicker::autofocus(true)`: the slider takes focus once each time the picker appears,
  so the arrows, Home and End change the level at once and the focus ring sits on the slider.
  `EffortPicker::slider_focus(&FocusHandle)` hands the slider a handle of the application's.
- `ReadoutStrip::id(..)`: its test selector, like every other component's id.
- `check-adoption.sh` reports a `debug_selector` wrapped around a `ReadoutStrip`.
- Web kit: nothing to port. Its buttons are DOM buttons that `element.focus()` reaches, and the
  effort picker and `ReadoutStrip` are not in the web kit yet.

### Workarounds you can delete

| Workaround | Use instead |
| --- | --- |
| A `menu_picker` of your own as the effort button, so focus can return to it on Escape | `EffortButton::new(..).track_focus(&handle)` and `window.focus(&handle)` |
| An outer frame around `EffortPicker` that holds focus and handles the arrows, Home and End | `EffortPicker::autofocus(true)` |
| `div().debug_selector("transport-primary").child(ReadoutStrip::new()..)` | `ReadoutStrip::new().id("transport-primary")` |

## Flat buttons under Glass

### Added

- `Shape::bezel_controls`: whether a secondary `Button` is lifted with a top-lit gradient and
  `elevation.low`. Only Studio sets it.
- `Colors::control`: the secondary `Button` fill. It is `elevated` except under Glass with
  `Material::Translucent`, where it is a see-through tone (white 10 % dark, black 5 % light) so
  the glass behind shows, as Liquid Glass's buttons do. Cards and menus keep `elevated`.

### Fixed

- Secondary buttons under Glass no longer carry a bezel and a shadow. `Button` decided by
  `colors.highlight.a > 0`, which Glass also sets; it now asks `shape.bezel_controls`, so
  `highlight` is free for other uses.
- The web kit follows: `--at-control` is the new token, and `.at-button` secondary is flat and
  see-through under Glass (`--at-raised-shadow: none`), lifted only under Studio.

### Workarounds you can delete

| Workaround | Use instead |
| --- | --- |
| `colors.highlight` made transparent so Glass buttons lie flat | Nothing: only Studio bezels its buttons now |

## From Auris Studio 0.15 and 0.16

What Auris Studio built for itself between 0.15 and 0.16 that other applications need too.

### Added

- `PianoKeyboard::new(id, low, high)`: a run of piano keys for auditioning a sound. A key sounds
  on the press, a drag plays each key it reaches, `.pressed(..)` lights the keys that sound, and
  the keyboard is one tab stop (arrows step a semitone, Home/End, Enter plays again).
  `note_name(60)` is "C4".
- `viz::NoteRoll` with `viz::RollNote`: a clip's notes as a small piano roll, for a MIDI
  attachment or a generated phrase. Parts take `theme.categorical` colours unless given their own.
- `ReadoutStrip` and `Readout`: the transport's controls and captioned values ("TEMPO" over
  "120.00 BPM") on one sunken surface, with hairlines between the readouts.
- `IconName::Menu`, three lines, for an application menu in the title bar.
- `EffortPicker::show_fast(false)` for reasoning effort without a fast mode,
  `EffortPicker::note(..)` for a line under the slider ("This model has no effort levels"), and
  `EffortButton::disabled(bool)`.
- `Strings::effort`, `fast_mode_on`, `fast_mode_off` and `reset`: the effort picker's tooltips
  were English only.

### Changed

- The effort picker's top level is a small treat: as the thumb glides onto it the fill runs from
  the accent into `colors.agent_gradient`, the sparkles twinkle faster, the thumb glows, the level
  reads in the agent's colour, and a few sparks fly off the thumb when it lands. Not when the
  picker opens already at the top, and no sparks with motion off.

### Deprecated

- `EffortPicker::model`, `model_icon` and `on_model`. The effort card no longer names the model:
  switching models is its own button in the composer row (`Composer::model`), beside the effort
  button, as Codex does. The card still draws the line while they are passed.

### Workarounds you can delete

| Workaround | Use instead |
| --- | --- |
| A note keyboard laid out by hand from `Button`s with absolute black keys | `PianoKeyboard::new(id, 60, 72).pressed(..).on_press(..)` |
| A piano-roll preview drawn with absolute `div`s per note | `viz::NoteRoll::new(notes)` |
| A sunken transport box with readouts bordered by hand | `ReadoutStrip::new().controls(..).readout(Readout::new(..))` |
| `assets/icons/menu.svg` of your own | `IconName::Menu` |
| A reasoning popover of your own, rebuilt on gpui-component's `Slider`, to drop the bolt or show "unavailable" | `EffortPicker::show_fast(false).note(..)`, `EffortButton::disabled(..)` |

## Atelier for the web

### Added

- `web/`: the design language for a browser or a Tauri webview, in the same style rather than the
  same pixels. `atelier.css` draws the core components (buttons, segmented controls, tabs, fields,
  switches, checkboxes, sliders, cards, lists, menus, badges, progress, the window frame, and the
  agent transcript and composer) from CSS custom properties; `atelier.js` picks themes, draws
  icons and wires the controls; `ambient.js` runs seven of the ambient backdrops' own WGSL shaders
  on WebGPU, with a CSS gradient where there is none. `web/showcase.html` shows them all. See
  [web/README.md](web/README.md), which also covers Tauri's window vibrancy.
- `atelier::web::{stylesheet, theme_css, theme_id}`: a theme's tokens as CSS custom properties.
  `web/atelier/generated/` is written from them, the shaders and the icons;
  `ATELIER_BLESS_WEB=1 cargo test --lib web` writes it again, and `cargo test` fails while it
  is out of date.

## 0.2.0 — Auris feedback, fourth round

Also in 0.2.0, on top of the third round below.

### Added

- `Composer::queue(bool)`: while `busy`, Send stays beside Stop for an agent that takes the next
  message during a turn. Stop comes first so Send keeps the corner; its tooltip is the new
  `Strings::queue` ("Queue message", 「予約して送る」).
- `ContextChip::max_width(..)` and `ContextChip::remove_disabled(bool)`. The remove button can be
  turned off (greyed, out of the Tab order) while the message waits.
- `ListItem::description_agent(bool)`: a description the agent wrote ends in its sparkle, as an
  `AgentLabel` would.
- `TextField::input_inset(..)`: the left padding an application's input keeps, so the
  placeholder starts where its first character does. The doc now says the text area has no
  padding of its own.
- `Composer` and `ContextChip` ids are test selectors, like the other components'
  (`composer-send`, `composer-stop`, `chip-remove`).

### Changed

- `ContextChip` shrinks: its label is cut with an ellipsis at 240 px (or `max_width`) or at the
  composer's width, and the whole label shows on hover. It no longer pushes past the input.

### Workarounds you can delete

| Workaround | Use instead |
| --- | --- |
| `composer_frame` kept for Stop and a queueing Send side by side | `Composer::busy(..).queue(true)` |
| Cutting a chip's label by hand, or leaving removal live while a message waits | `ContextChip` cuts itself; `.remove_disabled(true)` |
| A list row redrawn to mark an AI-written description | `ListItem::description_agent(true)` |
| A placeholder drawn by hand because the input keeps its own left padding | `TextField::placeholder(..).input_inset(px(..))`, or no padding on the input |

## 0.2.0 — Auris feedback, third round

Also in 0.2.0, on top of the second round below.

### Added

- `Strings`: every word a component draws by itself (tooltips, Send, "Copied", Allow, Deny,
  Cancel, task and agent states, the composer's placeholder) in one global, English by default.
  `Strings::japanese().install(cx)` once at start-up; a label given to a component still wins.
- Message text is the way to copy it: `AssistantMessage` with an id makes its built-in text
  selectable block by block, as `UserMessage` already did. Message actions show only while the
  pointer is over the message or the keyboard is moving (`pin_actions()` keeps them), so a
  transcript carries no row of Copy buttons.
- `TitleBar::drag_region(el)` for an application that marks its own drag area with
  `window_drag(false)`, and `TitleBar::title_id(..)`: the title has the selector `atelier-title`.
- `Composer::tone(..)` (a frame in the signal colour with its mark), `Composer::notice(..)` (a
  row inside the frame) and `Composer::attach_disabled(..)`. `ContextChip::on_click` opens the
  attachment; `ContextChip::remove_label(..)` names the remove tooltip.
- `agent::UsageMeter` and `Composer::usage(..)`: what is left of the plan, as a small ring and
  "28 % left" before Send. It follows the limit closest to running out, lists every
  `UsageLimit` with its reset time on hover, and turns to warning under 20 % and danger under
  5 %. Its words are in `Strings` (`usage`, `usage_left`, `usage_used`, `usage_resets`,
  `usage_reached`).
- `Morph::fill_to(..)` and `Morph::border(from, to)`.
- `FormField::unlabelled()` and `FormField::note(..)`; under a top label the first control fills
  the width.
- `Switch::on_change` and `Checkbox::on_change`, handed the new value.
- `atelier::motion::reduce_motion(cx)`, for tests.
- `IconName::Punch` (punch-in range) and `IconName::Notes` (a note of the score).
- The adoption check reports a `debug_selector` wrapper by the component it wraps, and finds
  `composer_frame`, own `punch`/`notes` SVGs and motion turned off by hand.

### Changed

- `TextField`'s placeholder lies under the application's input at the text's start instead of
  beside it.
- The composer's placeholder and tooltips, the agent panel, the player, approvals and task
  progress read their words from `Strings` instead of hard-coded English.
- `CopyButton::copied_label` and `tooltip` default to `Strings`; `ApprovalCard::allow_label`
  too.

### Workarounds you can delete

| Workaround | Use instead |
| --- | --- |
| `composer_frame` drawing a warning frame, a notice row or a greyed attach | `Composer::tone`, `.notice`, `.attach_disabled` |
| An own drag region laid beside `TitleBar::window_drag(false)` | `TitleBar::drag_region(el)` |
| A `div` around `Morph` fading it into a veil | `Morph::fill_to(..)`, `.border(..)` |
| `.empty(false)` plus an own placeholder in `TextField` | `.placeholder(..).empty(..)` |
| `div().flex_1()` around each control in a `FormField` | Nothing: the first control fills the width |
| `cx.global_mut::<Theme>().motion.enabled = false` in tests | `atelier::motion::reduce_motion(cx)` |
| Inverting the value in `Switch::on_click` | `Switch::on_change` |
| `punch.svg`, `notes.svg` | `IconName::Punch`, `IconName::Notes` |
| Translating each tooltip (`.tooltip("コピー").copied_label("コピーしました")`) | `Strings::japanese().install(cx)` |
| A labelled "Copy message" button under each message | The message's selectable text, and an icon-only `CopyButton` shown on hover |

## 0.2.0 — Auris feedback, second round

### Added

- **Focused controls own Enter and Space.** Every Atelier tab stop carries the key context
  `AtelierControl` (`atelier::CONTROL_CONTEXT`), and while one holds focus Atelier takes Enter
  and Space before any key binding is matched, so a shortcut such as Space for play no longer
  fires while a button, row or switch holds focus: the key presses the control. Scope a shortcut away from controls
  entirely with `KeyBinding::new("space", TogglePlay, Some("!AtelierControl"))`.
- **Arrow keys show the focus ring only on a control.** Arrows that move a playhead or a note
  leave no ring behind.
- `SelectableText`: read-only text the person can select and copy exactly as written (drag,
  Shift-click, double-click for a word, triple-click for a line, ⌘C/Ctrl+C, ⌘A/Ctrl+A).
- `colors.selection`, the colour behind selected text, for `SelectableText` and for the
  application's own inputs inside `TextField`.
- `UserMessage::body(..)` and `UserMessage::action(..)`, matching `AssistantMessage`. With an
  id, a `UserMessage`'s text is selectable.
- `CopyButton`: copies, shows a check and "Copied" for a moment, and moves its label to the
  tooltip. `CodeBlock::copyable()` now uses it and actually copies.
- `AudioPlayer::on_seek`, `seek_step`, `on_volume`, `muted`, `on_mute` and `loading`. The
  scrubber and the volume follow a click or a drag and, once focused, the arrow keys, Home and
  End (key context `AtelierTrack`).
- `Plot3d` draws every `scatter` in one view in a single depth order, so points of different
  colours overlap correctly. `Plot3d::id`, `on_pick`, `on_orbit`, `pick`, `project` and
  `ViewPreset` (oblique, front, top, side).
- `Theme::categorical(index)`: a colour for any number of categories, readable on every
  theme and clear of the agent's hue.
- `Theme::all()`: every direction in both appearances, for screen tests.
- `Density::split_width` and `Density::sidebar_width`: where two panes stack and where a
  sidebar folds away.
- `Completions` and `Composer::completions(..)`: the suggestion list under an input — slash
  commands, model names — with focus left in the input and the selected row kept in view.
- `agent::AgentLabel`: short text the agent wrote (a cluster's name, a title), marked with the
  agent's sparkle until the person edits it.
- `atelier::init` may be called more than once: it binds keys only the first time.
- `atelier::emoji`: emoji typed as shortcodes. `shortcode_at` finds the `:thin` before the
  caret, `search` ranks the emoji it could mean, `emoji::completions` draws them as a
  `Completions` list and `ShortcodeQuery::accept` puts 🤔 in its place; `replace_shortcodes`
  turns a finished or pasted `:tada:` into 🎉. `Completion::glyph(..)` gives a row a leading
  emoji or symbol in place of an icon.
- `ui_font` and `mono_font` fall back to Apple Color Emoji on macOS and Segoe UI Emoji on
  Windows, so emoji draw in colour in every Atelier text.

### Changed

- Transcript rows fade in without moving: `UserMessage` and `AssistantMessage` no longer rise
  8px, which changed their height and jolted a conversation pinned to the bottom.
- The crate version is 0.2.0. `Colors` gained `selection` and `Density` two widths; code that
  builds either struct field by field (none should; derive from `Theme::new`) must add them.

### Workarounds you can delete

| Workaround | Use instead |
| --- | --- |
| A no-op action (`ClaimVisualizationActivation`) bound to Enter/Space in a context put on each area with controls | Nothing: Atelier controls claim the keys. Optionally scope the shortcut with `!AtelierControl` |
| A hand-made selectable text (`selectable_text`) and a redrawn user bubble around it | `UserMessage::new(text).id(..)`, or `SelectableText` anywhere else |
| Borrowing gpui-component's `selection` colour | `colors.selection` |
| Copy buttons that swap their own label and tooltip (`workshop_copy_button`) | `CopyButton::new(id, text).label("Copy title").copied_label("Copied")` |
| A user message drawn by hand to drop the rise animation | `UserMessage`; it only fades now |
| An own audio player for seek, volume, mute and loading | `AudioPlayer` with `on_seek`, `on_volume`, `on_mute`, `loading` |
| Re-sorting every point and batching runs of one colour into `scatter` (`scatter_batches`) | Call `scatter` once per colour |
| A copy of `Plot3d`'s projection for clicks (`TimbreCamera`, `pick`) | `Plot3d::id(..).on_pick(..)`, or `Plot3d::pick` / `project` |
| Drag-to-rotate and view presets built around `Plot3d` | `Plot3d::on_orbit` and `ViewPreset` |
| A golden-ratio `group_color` | `theme.categorical(index)` |
| `if cx.has_global::<Theme>() { install } else { init }` | `atelier::init` once at start-up, `Theme::install` to switch |
| A loop over three directions × two appearances in screen tests | `for theme in Theme::all()` |
| A breakpoint such as `48rem` chosen by the app | `theme.density.split_width` |
| A completion list drawn with gpui-component spacing | `Completions`, or `Composer::completions` |

## 0.1.0 — up to `30f9209` (#18)

The first round of Auris feedback landed in #16 (`22d3a55`).

### Workarounds you can delete

| Workaround | Use instead |
| --- | --- |
| `nav_item` (a `ListItem` copy with `tab_index` and a focus ring) | `ListItem`, a tab stop with the ring; `.description_lines(2)` for long descriptions |
| An own `button` for keyboard reach and a test selector | `Button`: tab stop, Enter/Space, its id as the selector |
| `div().debug_selector(..)` around Atelier components | The component's own id |
| Fields copying `TextField`'s colours around an editable `div` | `TextField::new().id(..).track_focus(..).on_mouse_down(..).empty(..).child(input)` |
| `assistant_message` without the name line | `AssistantMessage::anonymous()` |
| `auris-redo.svg`, `auris-bolt.svg`, `Code` for the keyboard page | `IconName::Redo`, `Bolt`, `Keyboard`; own SVGs via `IconSource::svg/image` |
| `titlebar::composed` | `TitleBar::window_controls(width, buttons).window_drag(false)` |
| A hand-made panel-to-screen transition | `Morph` + `Measure` |
| Composer controls copied beside the composer | `Composer::control(..)`, `.empty(..)` |
| An opaque title bar on Windows so DWM's caption buttons do not show through | Built into `TitleBar` |
