# Design rules

Atelier builds on the GPUI Kit Design Guides (gpui-kit.com/docs/design-guides) and keeps their
judgment; this page records the rules as they apply to Atelier, plus what creator tools and
agents add. Where the two disagree, this page wins inside an Atelier app.

## Thesis

1. **The work is the hero.** A creator tool exists for the canvas — the timeline, the score, the
   image. Chrome is quiet and subordinate; the canvas uses `colors.background`, the deepest
   surface, and gets the surplus space.
2. **The AI is a collaborator with a visible hand.** Whatever the agent changed is marked where
   it lives (an AI-draft clip has an agent-coloured dashed border and a sparkle) and can be kept
   or undone there, not only in the chat.
3. **Clarity before personality; composition before invention; tokens before values.**
4. **State must be visible**, with distinct rest, hover, pressed, selected, disabled, loading
   and error treatments.
5. **Geometry explains containment.** Radii nest concentrically; elevation explains stacking.

## Surfaces and hierarchy

Back to front: `background` (canvas) → `chrome` (title bar, sidebars) → `surface` (panels) →
`elevated` (cards, bubbles, raised controls) → `overlay` (menus, tooltips, dialogs). `sunken` is
for wells: fields, tracks, meters, code.

- Use size, weight, spacing and separators before colour or containers. Do not nest cards.
- Shadows only for things that float (`elevation.medium` for menus, `high` for dialogs).
  Studio is the one direction whose raised buttons carry `elevation.low` as a bezel.
- A hairline belongs to the boundary's owner; two neighbours never both draw it.

## Spacing grammar

Pick the step by relationship, from `theme.spacing`:

| Step | px | Relationship |
| --- | --- | --- |
| `xxs` | 2 | optical correction |
| `xs` | 4 | parts of one control (icon + label in a row, title + description) |
| `sm` | 8 | closely related controls (button icon/label, dialog actions) |
| `md` | 12 | one content group |
| `lg` | 16 | separate groups in one section; panel padding |
| `xl` | 24 | separate sections |
| `xxl` | 32 | major region boundaries, empty states |

Inside before outside: a component pads itself, the parent owns the gap between components.
Equal gaps imply equal relationships. Zoom with `Theme::zoomed(factor)` so type, spacing,
controls and radii scale together.

## Radius

Use the tiers in `theme.shape`. Nested surfaces are concentric:
`Theme::concentric(outer, inset)` = `max(0, outer − inset)`. The composer's send button and a
segmented control's selected segment follow this. Capsules (`theme.control_radius`) only where
the direction sets `capsule_controls` (Glass) or for intentional pills.

## Components and states

- Variants by meaning: `Primary` (default commit), `Secondary` (ordinary), `Ghost` (quiet
  toolbar), `Danger` (destructive commitment), `Agent` (hands work to the AI).
- Latching buttons (mute, solo, arm, loop) use `.selected(true)` with a `.tint(...)` from the
  signal colours: `colors.info` mute, `colors.warning` solo/cycle, `colors.record` arm.
- A button that opens a dialog or needs more input ends in `…` ("Export…"). Immediate commands
  do not.
- Secondary commands go in a visible menu or a context menu, not in hover-only icon strips. The
  one exception is a transcript's message actions (Copy, Retry), which show on hover and while
  the keyboard moves, so the conversation reads as text.
- Loading indicators only while something really waits.
- **Everything the pointer can press, the keyboard can reach.** Use Atelier's components (they
  are tab stops, press on Enter/Space and leave the order when disabled) rather than a `div` with
  `on_click`; a custom control takes `.tab_index(0)` and its id as `debug_selector`, with
  `// atelier: allow` on that line so the adoption check knows it is one. The focus outline is
  Atelier's and shows only after keyboard navigation.
- **Single-key shortcuts yield to a focused control.** Space, Enter and letter keys bound
  across the window (Space for play) must not fire while a control holds focus: there the key
  belongs to the control. Atelier does this for Enter and Space by itself — every tab stop has
  the key context `AtelierControl`, and Atelier takes the two keys there before any binding.
  Bind letter keys, and any single key your own focusable views must not see, with the
  context `!AtelierControl`:
  `KeyBinding::new("space", TogglePlay, Some("!AtelierControl"))`. Never claim the keys area by
  area with a no-op action.
- **Arrows belong to what has focus.** On a track (the `AudioPlayer` scrubber, its volume) they
  step the value; on a canvas — a timeline, a piano roll — they move the playhead or the
  selection and show no focus ring. The ring appears for arrows only when a control already
  holds focus. A track steps by a twentieth of its range, or by five seconds of audio.
- **Text people may want to keep is selectable.** Messages, attached text, logs and error
  details use `SelectableText` (or `UserMessage` with an id); labels on controls do not.
  Selected text is drawn on `colors.selection`, in Atelier and in the application's own
  inputs.
- **Copy says it copied.** Every copy action is a `CopyButton`, so all of them answer the same
  way.
- When Atelier lacks something you need — an icon, a slot, a hook — use the open door
  (`IconSource`, `.control(..)`, `.window_controls(..)`, a container's `.id(..)`) and, if there
  is none, add it to Atelier rather than copying a component's sizes and colours into the app.

## Forms and preferences

- One preferences window: pages in a leading `Sidebar` (with search when there are more than a
  handful of settings), one page at a time, each opening with a `SectionHeader` and its
  description. The column is at most `density.form_width` wide; the rest of the page stays
  empty rather than stretching the fields.
- Labels go **above** anything typed into or chosen from (`FormField`, the default), so long and
  translated labels never squeeze the control; **beside** a switch or checkbox
  (`FormField::beside`), so the label reads as the question and the control as the answer.
- A `Switch` applies at once and is never followed by Save. Settings that take effect together
  (a backend and its address) are committed by the page's primary, Save, at the trailing end of
  the page's foot; it is disabled until something changed, and Revert sits beside it.
  "Reset to defaults" is a ghost at the leading end, away from Save.
- Group related fields under a `FormGroup` title; one group needs no title.
- Each field's description says what changes, not what the control is. An error sits under its
  field with a mark and says how to fix it.
- A `StatusBar` along the window's bottom edge reports what the last action did ("Saved",
  "Downloaded 2 models"); it never asks a question and never holds the only copy of an error the
  person must act on.

## Creator-tool specifics

- The window has one bar: `TitleBar` is both chrome and main toolbar. Keep it slim and on one
  line: only the document title, centred, with no subtitle beside or under it; frequent
  commands at the edges, and empty space around the title so the bar can be dragged.
- Canvas first: the timeline/editor gets `flex_1` and `min_w_0`/`min_h_0`; inspectors and the
  agent panel are fixed-width, dismissible regions on the trailing edge.
- Lay out for every window size between the minimum and a large display:
  - Give the window a `window_min_size` (the examples use 900 × 600) and check the layout there
    and on a 2560-wide screen.
  - When the width runs out, hide whole regions in a fixed order instead of squeezing them: the
    navigation sidebar first, then slim the agent panel (380 → 320) and the inspectors, then
    drop the least-used title-bar controls and inspector sections. Decide from
    `window.viewport_size()` in `render`.
  - When the height runs out, scroll the region (`.id(..).overflow_y_scroll()` with
    `flex_1().min_h_0()`); never let content run under a footer or off the window. Rows inside
    a scrolling column are `flex_none`.
  - When there is height to spare, give it to the plots and lanes (`flex_grow` on the cards,
    `flex_1().min_h(..)` on the plot) rather than leaving an empty band at the bottom.
  - Any `flex_1` element holding text in a row also gets `min_w_0`, or its text will not wrap
    or truncate and pushes past its neighbours. CJK text counts as one long word.
  - Rows of cards, chips or fields that can outgrow their container use `flex_wrap`.
- Two panes that share a region (an editor and its inspector, a model view and its controls)
  sit side by side from `density.split_width` and stack below it, controls under the view.
  Fold the navigation sidebar away below `density.sidebar_width` of window. Measure the region
  for the first and the window for the second; never pick your own breakpoint.
- **One thing, one colour, everywhere.** When colour stands for a category — a cluster of
  sounds, a tag, a group — take it from `theme.categorical(index)` and give each thing one
  index for good, so the map, the list and the tree all point at it with the same colour. Past
  a handful of categories, and always under Mono, add a label or a mark: colour alone does not
  tell ten things apart.
- Values are monospaced (`Label::mono`) and right-aligned where they are compared: timecode,
  dB, BPM, knob readouts.
- Document colours (track, clip, palette) are data and may be literal; draw clip bodies with the
  colour at low alpha and the header stronger, and text with `theme.text_on(color)`.
- Transport: play is `selected` while playing; record uses `colors.record`; the playhead uses
  `colors.playhead`.
- Knobs for continuous timbre parameters in dense strips; sliders for volume/pan rows where the
  label and readout share the line.

## Copy

Sentence case. Name the verb and the object: `Create "Pad swell" on Strings?` with `Create` and
`Deny`, never "Are you sure?" with OK. Nouns for destinations, verbs for commands, adjectives for
states. Errors say what happened and how to recover. No terminal period on labels.

## Review checklist

1. Is the primary task clear, and does the canvas get the space it deserves?
2. Does each action keep its promise — label, state, scope, result? Does each concept use the
   control [choosing.md](choosing.md) gives it?
3. Is emphasis scarce: one primary, few coloured badges, agent colour only for the agent?
4. Are peers on shared alignment spines, with equal gaps equal?
5. Do radii nest concentrically and do only floating layers cast shadows?
6. Does it read under Studio, Glass and Mono, light and dark?
7. Are empty, loading, error, awaiting-approval and disabled states designed?
8. Is every icon-only control named (tooltip) and every state shown by more than colour?
9. Can every control be reached with Tab and pressed with Enter or Space, in reading order, with
   disabled ones skipped — and does no single-key shortcut fire while one holds focus?
10. Does `tools/check-adoption.sh` pass: no wrapped components, no copied sizes, no workaround
    an Atelier release has since replaced?
