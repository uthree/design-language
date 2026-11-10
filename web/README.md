# Atelier for the web

The Atelier design language for pages in a browser or a [Tauri](https://tauri.app) webview: the
same six themes, the same components and the same ambient backdrops as the gpui kit, drawn with
CSS and WebGPU. It aims for the same style, not the same pixels.

![The showcase in Glass Dark over the Aurora backdrop](../doc/images/web-showcase-glass-dark.png)

No framework and no build step: one stylesheet and two small ES modules. Use them from plain
HTML, React, Svelte or anything else that renders markup.

```
web/
  showcase.html           every component, the agent surfaces and the backdrops
  atelier/
    atelier.css           the components
    atelier.js            themes, icons, and the behaviour of segmented controls, tabs, sliders
    ambient.js            the ambient backdrops on WebGPU
    generated/            written from the Rust crate; do not edit
      tokens.css          the six themes as CSS custom properties
      ambient.js          the backdrops' WGSL shaders, parameter layout and palettes
      icons.js            the icons as inline SVG
```

Try it with any static server from this directory, for example
`python3 -m http.server --directory web` and then <http://localhost:8000/showcase.html>.
`?theme=studio-light`, `?view=agent`, `?scene=Galaxy` and `?still=0.2` pick what it shows.

## Themes

Pick a theme with `data-theme` on `<html>` (or on any element, for a region in another theme):

```html
<html data-theme="studio-dark">
```

The ids are `studio-light`, `studio-dark`, `glass-light`, `glass-dark` (the default), `mono-light`
and `mono-dark`. From script, `setTheme({ direction: "mono" })` keeps the appearance and changes
the direction, and `followSystemAppearance()` follows the system's light or dark setting.

Every value is a token: `var(--at-accent)`, `var(--at-radius-md)`, `var(--at-control-md)`,
`var(--at-space-lg)`, `var(--at-shadow-medium)`. The names follow the Rust fields in kebab case
(`colors.text_muted` is `--at-text-muted`), so the design guide in
[`skills/atelier-gpui`](../skills/atelier-gpui/SKILL.md) applies unchanged. Write your own
components with tokens, never with colours.

### Where the tokens come from

`generated/tokens.css` is written by `atelier::web::stylesheet()` from `Theme::new`, so the rules
that make a palette coherent (contrast floors, accent fallbacks, how each button variant is
painted) live in one place. After a change to `src/theme.rs`, a button's paint, a shader or an
icon, write the files again:

```sh
ATELIER_BLESS_WEB=1 cargo test --lib web
```

`cargo test` fails while they are out of date. A Tauri app can also call
`atelier::web::theme_css(&theme)` itself to hand its webview a theme the six files don't cover,
such as `Theme::new(..).zoomed(1.25)`.

## Components

Class names follow the Rust components, and modifiers follow their builders:

| Rust | Web |
| --- | --- |
| `Button::new(..).primary()` / `.ghost()` / `.variant(Danger)` / `Agent` | `.at-button.at-button--primary` / `--ghost` / `--danger` / `--agent` |
| `.size(Small)` / `Large`, `Button::icon_only`, `.selected(true)` | `--sm` / `--lg`, `--icon`, `aria-pressed="true"` |
| `SegmentedControl`, `Tabs` | `.at-segmented`, `.at-tabs` with `<button>`s |
| `TextField`, `FormField` | `.at-field` around an `<input>`, `.at-form-field` |
| `TextInput` (in a `TextField`), `TextField::area` | the browser's own `<input>` / `<textarea>` in `.at-field`, `.at-field--area` |
| `Table`, `Column`, `TableRow`, `RowMark`, `TableCell` | `table(element, { columns, rows, empty })`, returning `update`, `select`, `cell`; `atelier:row-select` with `{ index }`, `atelier:cell-activate` with `{ row, column, element }`; `.at-table`, `.at-table__row[aria-selected]`, `[data-active]`, `[data-faint]`, `[data-tinted]`, `--_mark` |
| `Follow`, `BackToPlayhead`, `PlayheadSide` | `follow()` from `atelier.js`, returning `update(offset, span, at, playing)`, `engage`, `release`, `released`, `page(span, at)`, `side(offset, span, at)`; `.at-back-to-playhead[data-side="before" \| "after"]` on an `.at-button--sm` |
| `Stat` (`unit`, `size`, `color`, `inline`, `end`, `empty`) | `.at-stat` holding `.at-stat__caption` and `.at-stat__figure` (`.at-stat__value`, `.at-stat__unit`); an empty value shows the dash; `.at-stat--display`, `--md`, `--sm`, `--inline`, `--end`; `data-tone="success" \| "warning" \| "danger"` |
| `InlineEdit`, `SpotBook` | `inlineEdit(label, { value, multiline, allowEmpty, minWidth, maxWidth, onCommit, onClose })`, returning `field`, `input`, `close`; `onCommit` may return a message to refuse; `atelier:inline-commit` with `{ value }`, `atelier:inline-close`; `.at-inline-edit` (the label's own rectangle stands in for the spot) |
| `PreferencesWindow`, `SettingsPage`, `SettingsGroup`, `Setting`, `FormGroup` | `preferences(element, { pages, selected, actions, labels })`, returning `select`, `search`, `page`; `atelier:page`; `.at-preferences`, `.at-form-group` |
| `KeybindingList`, `KeyBinding`, `keystroke_label` | `keybindings(element, { groups, labels })`, returning `update`, `recording`; `keystrokeLabel(event)`; `atelier:keybinding`, `keybinding-remove`, `keybinding-reset`; `.at-keybindings`, `.at-keybinding` |
| `Switch`, `Checkbox`, `Slider` | `.at-switch`, `.at-checkbox` around an `<input type=checkbox>`, `input.at-slider` |
| `Slider::bipolar`, `.default_value(v)` | `input.at-slider.at-slider--bipolar`, `data-default="v"` (a double click goes back) |
| `Knob::new(v).id(..).bipolar().label(..).readout_with(..)` | `.at-knob` with `data-value`, `data-bipolar`, `data-default`, `data-step`, `.at-knob__label` and an `<output class="at-knob__readout">`; `--_size` and `--_value-color` restyle it |
| `Card`, `Popover`, `SectionHeader` | `.at-card`, `.at-popover`, `.at-section-header` |
| `ListItem`, `OptionCard`, `MenuItem` | `.at-list-item`, `.at-option-card`, `.at-menu-item` in `.at-menu` |
| `Badge`, `StatusDot`, `Kbd`, `Avatar` | `.at-badge--success` etc., `.at-status-dot`, `.at-kbd`, `.at-avatar` |
| `ProgressBar`, `Spinner`, `Skeleton` | `.at-progress`, `.at-spinner`, `.at-skeleton` |
| `LevelMeter` (`peak`, `clipped`, `horizontal`, `LevelMeter::position(db)`) | `.at-meter` with `--level` and `--peak` (0 to 1), a `.at-meter__peak` child, `data-clipped`, `.at-meter--horizontal`; `setMeter(element, { level, peak, clipped })` and `meterPosition(db)` from `atelier.js`. The older `<span style="height: …">` fill still works. |
| `TitleBar`, `Toolbar`, `Sidebar`, `StatusBar` | `.at-title-bar`, `.at-toolbar`, `.at-sidebar`, `.at-status-bar`, each with `.at-chrome` |
| `Composer`, `ContextChip`, user and assistant messages | `.at-composer`, `.at-context-chip`, `.at-user-message`, `.at-assistant-message` |
| `Dialog`, `ApprovalDialog` (`Decision`) | `openDialog({ title, body, icon, tone, content, actions })` and `approvalDialog({ title, body, command, allowLabel, destructive, offerAlways, reviewing })` from `atelier.js`, resolving with the answer; `dialog.at-dialog` with `::backdrop` in `--at-scrim` |
| `PermissionPicker`, `PermissionMode` | `permissionPicker(element, { mode, modes, labels })`, firing `atelier:permission`; `PERMISSION_MODES` |
| `AgentActivity`, `AgentPhase` | `activity(element, { phase, detail, progress, elapsed, rate, tokens, context, labels })`; `.at-activity`; `AGENT_PHASES` |
| `PlanView`, `StepList` | `planView(element, { title, steps, open })`, firing `atelier:toggle`; `.at-plan`, `ol.at-steps > li.at-step--done/running/pending/failed/skipped` |
| `ChangedFiles`, `FileChange`, `ChangeKind` | `changedFiles(element, { title, files, actions })`, firing `atelier:open-file` with `{ index, path }`; `.at-changes`, `.at-change[data-kind]` |
| `graph::NodeCanvas`, `GraphNode`, `NodeField`, `Wire`, `NodeState` | `nodeCanvas(element, { portTypes, nodes, wires, legend, labels })` (a node's `comments` are drawn as notes), returning `add`, `remove`, `update`, `setState`, `connect`, `disconnect`, `select`, `undoAgentNode`, `resetView`, `nodes`, `wires`, `downstream`, `executionOrder`; `atelier:node-select`, `node-move`, `node-connect`, `node-disconnect`, `node-remove`, `node-keep`, `node-undo`, `node-comment`; `.at-node-canvas`, `.at-node`, `.at-node__comment` |
| `MenuItem::description`, `.danger()` | `.at-menu-item--described` with `.at-menu-item__text` and `.at-menu-item__description`, `.at-menu-item--danger` |
| `ChatView` (`push`, `append`, `finish`, `set_tool_state`, `answer`, `jump_to_latest`, `open_attachments`, `ChatEvent::Decided`, `ChatEvent::Chose`, `ChatEvent::OpenAttachment`), `ChatAttachment` | `chat(element, { panel, logo, strings, openAttachments })` from `atelier.js`, returning `push`, `append`, `finish`, `setToolState`, `answer`, `jumpToLatest`; `atelier:decided` events with `{ id, decision }`, `atelier:chose` with `{ id, picked }` for a `choices` entry, `atelier:open-attachment` with `{ id, index, label }`; a user entry's `attachments` are labels or `{ label, icon }` |
| `Markdown` | `markdown(element, text, { labels })`, `.at-markdown` (answers in `chat()` use it) |
| `agent::ChoiceCard`, `Choice` | `choiceCard(element, { question, options, multiple, picked, answered, labels })`, returning `answer`, `picked`; `atelier:choose` with `{ picked }`; `.at-choice-card` |
| `agent::MessageQueue`, `QueuedMessage` | `messageQueue(element, { messages, paused, editable, labels })`, returning `update`; `atelier:queue-send-now`, `queue-edit`, `queue-remove` with `{ index }`, `queue-pause` with `{ paused }`; `.at-queue` |
| `agent::ConversationList`, `Conversation`, `ConversationRow` | `conversationList(element, { conversations, selected, archived, labels })`, returning `update`, `search`; `atelier:conversation-open`, `conversation-more` with `{ index }`, `conversation-new`, `conversation-archived`; `.at-conversations`, `.at-conversation` |
| `Tooltip` | `data-tooltip="…"` on any element |
| `Button::shortcut`, `Button::shortcut_for`, `Tooltip::shortcut`, `binding_label` | `data-shortcut="secondary-d"` (gpui's notation, or already written) beside `data-tooltip`, turned by `hydrate()` into `.at-tooltip` with `.at-tooltip__keys` and `aria-keyshortcuts`; `shortcutTooltip(element, words, keys)` and `shortcutLabel(binding)` from `atelier.js` |

Icons are `<span data-icon="sparkles"></span>`; `hydrate()` from `atelier.js` draws them, and
wires segmented controls and tabs (click and arrow keys, an `atelier:select` event), slider fills
and knobs. Call it again on markup a framework renders later. The provider marks are icons too:
`logo-anthropic`, `logo-openai`, `logo-google`, `logo-local`.

Every control works as it is dropped in, as it does natively: switches, checkboxes and sliders
are the browser's own inputs, and a knob turns with a drag up or to the right, the arrow keys,
Home and End, and goes back to `data-default` on a double click. Listen for `input` (while it
moves) and `change` (once it settles) and read `.value`, from 0 to 1 on a knob; set `.value`
from script to move it. The readout is yours to write, in your own units:

```html
<div class="at-knob" data-value="0.8" data-default="0.8">
  <span class="at-knob__label">Drive</span><output class="at-knob__readout">80 %</output>
</div>
<script type="module">
  import { hydrate } from "./atelier/atelier.js";
  hydrate();
  const knob = document.querySelector(".at-knob");
  knob.addEventListener("input", () => {
    knob.querySelector("output").textContent = `${Math.round(knob.value * 100)} %`;
  });
</script>
```

## Translucency

`.at-chrome` fills a surface with `--at-chrome` and, under Glass, blurs what lies behind it with
`backdrop-filter`. In a browser that is the page: put the ambient backdrop, or any picture,
behind the window, as the showcase does. Content areas (`.at-content`) stay opaque, as they do
natively.

### Tauri

To blur the desktop itself, as the native Glass window does, make the window transparent, give
it the system material, and tell the page:

```jsonc
// tauri.conf.json (Tauri 2)
{
  "app": {
    "macOSPrivateApi": true,
    "windows": [{
      "transparent": true,
      "decorations": false,
      "windowEffects": { "effects": ["underWindowBackground", "acrylic"] }
    }]
  }
}
```

```js
import { theme, useVibrancy } from "./atelier/atelier.js";
if (window.__TAURI__ && theme().direction === "glass") useVibrancy();
```

`useVibrancy` clears the page's background so the window's material shows through the chrome.
macOS uses the first effect and Windows 11 the second; most Linux desktops can't blur behind a
window, so leave vibrancy off there, as `Theme::window_background` does natively. With
`decorations: false`, give `.at-title-bar` the `data-tauri-drag-region` attribute and draw the
caption buttons yourself.

## Ambient backdrops

```js
import { AmbientBackdrop } from "./atelier/ambient.js";
const backdrop = await AmbientBackdrop.create(canvas, { scene: "Aurora" });
backdrop?.play();
```

The backdrops run the native scenes' own WGSL shaders on WebGPU, in the page's theme, and answer
the pointer with the same eased focus and ripples. `create` resolves to `null` without WebGPU;
`.at-ambient` then keeps a still gradient in the theme's colours. Call `still(phase)` instead of
`play()` for a reduced-motion preference.

Seven scenes come across: Aurora, Dunes, Bloom, Caustics, Solar, Galaxy and Jellyfish. Shoal,
Formation and Garden need their CPU simulations (the fish, the figures, the plants) and are left
out for now, as are Aurora's constellations and the window's swing; compiling those simulations
to WebAssembly would bring them over. The liquid-metal backdrop is not ported yet.

## Not yet

The prototype covers the tokens, the core controls, the window frame, the agent transcript and
composer, dialogs and approvals, the activity line, plan and changed files, the node canvas,
preferences, Markdown, choice cards, the message queue, the conversation list, and the backdrops. Still to come: menus that open (the menu is
styled, not positioned; the permission picker's menu is the exception), completions, plots (and with them `LinePlot::on_drag`) and the instrument controls such as
`PianoKeyboard`, `NoteRoll` and `ReadoutStrip`/`Readout`, and the effort picker's `note` and
`EffortButton::disabled`. `IconName::Menu` is in the generated icons.

`AgentConnection` and `AgentFeed` (the `agents` feature) stay native: they run the CLI as a
process and have no look of their own. A Tauri app runs `atelier-agents` in its Rust side and
sends the events to the page, which draws them with the transcript classes above.
