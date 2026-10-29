# Changelog

Each release lists what it adds and, under **Workarounds you can delete**, the code an
application wrote to cover a gap that the release closes. When you move the `atelier`
dependency to a newer revision, read the sections in between, delete what they list, and run
[`skills/atelier-gpui/tools/check-adoption.sh`](skills/atelier-gpui/tools/check-adoption.sh)
over your sources to find what is left. Add the script to the application's CI so new code
does not bring the old ways back.

Atelier has no tagged releases yet; applications pin a git revision. Each section names the
merge it shipped in.

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
