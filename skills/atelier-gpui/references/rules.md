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
- Secondary commands go in a visible menu or a context menu, not in hover-only icon strips.
- Loading indicators only while something really waits.

## Creator-tool specifics

- The window has one bar: `TitleBar` is both chrome and main toolbar. Put the document title and
  frequent commands in it; leave its middle empty so it can be dragged.
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
2. Does each action keep its promise — label, state, scope, result?
3. Is emphasis scarce: one primary, few coloured badges, agent colour only for the agent?
4. Are peers on shared alignment spines, with equal gaps equal?
5. Do radii nest concentrically and do only floating layers cast shadows?
6. Does it read under Studio, Glass and Mono, light and dark?
7. Are empty, loading, error, awaiting-approval and disabled states designed?
8. Is every icon-only control named (tooltip) and every state shown by more than colour?
