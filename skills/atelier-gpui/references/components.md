# Components

All are builders implementing `RenderOnce`; pass them to `.child(...)`. Interactive ones take an
id and optional `on_click` / `on_select` / `on_toggle` callbacks; they draw a value but do not
own it — the view keeps the state and calls `cx.notify()`. With `cx.listener` a select handler
receives `&usize`.

Assets: start the app with `Application::new().with_assets(atelier::Assets::default())`, or
`Assets::with_fallback(your_source)` to serve your own files too. Icons are `IconName::*`
drawn with `Icon::new(name).size(px).color(c)`.

## Text

`Label::new(text)` with `.size(TextSize::XSmall|Small|Medium|Large|XLarge|Display)`,
`.color(TextColor::Default|Muted|Faint|Accent|Agent|Success|Warning|Danger|Disabled|Custom)`,
`.weight(..)`, `.heading_weight()`, `.mono()`, `.truncate()`.
`Label::heading(t)` for section titles, `Label::caption(t)` for small uppercase region labels.

## Buttons

```rust
Button::new("export", "Export…").primary()
Button::new("bounce", "Bounce in place")                 // Secondary
Button::new("dup", "Duplicate").ghost().icon(IconName::Copy)
Button::new("ask", "Ask assistant").variant(ButtonVariant::Agent).icon(IconName::Sparkles)
Button::new("del", "Delete track").variant(ButtonVariant::Danger)
Button::new("grid", "Grid: 1/16").icon_after(IconName::ChevronDown).size(ControlSize::Small)
Button::icon_only("settings", IconName::Settings).tooltip("Project settings…")
Button::new(("solo", i), "S").size(ControlSize::Small).selected(solo).tint(colors.warning)
```

Also `.disabled(bool)`, `.full_width()`, `.on_click(|event, window, cx| ...)`.

## Controls

- `Switch::new(id, on).label("Snap to grid")` — a setting that applies immediately.
- `Checkbox::new(id, checked).label("Count-in")` — an independent choice.
- `SegmentedControl::new(id, [Segment::label("Light"), Segment::icon(IconName::Play)], selected)`
  `.size(ControlSize::Small).on_select(cx.listener(|this, index: &usize, window, cx| ...))`.
- `Tabs::new(id, ["Score", "Piano roll"], selected).on_select(...)` — views of one object.
- `Slider::new(0..=1 value).label("Volume").readout("-3.2 dB")`, `.bipolar()` for pan.
- `Knob::new(value).label("Drive").readout("80 %")`, `.bipolar()`, `.color(track_colour)`,
  `.diameter(px)`.
- `TextField::new().icon(IconName::Search).placeholder("Search sounds…")`; put the app's
  editable input inside with `.child(input)`, or `.value(..)` for mock-ups; `.focused(true)`
  draws the ring.

Dragging and keyboard stepping for sliders and knobs belong to the application (it owns the value
and its undo); wrap the component in an element that handles the gesture.

## Display

`Badge::new("Rendered").tone(Tone::Success).dot()`, `Kbd::new("⌘K")`, `Avatar::person("UT")`,
`Avatar::agent()`, `Divider::horizontal()` / `Divider::vertical().full()`,
`LevelMeter::new(level).peak(p)` (`.horizontal()`, `.thickness(px)`), `StatusDot::new(color)`.

## Progress and loading

`ProgressBar::new(v)` (`.id(..)` to glide, `.tone(..)`, `.height(..)`),
`ProgressBar::indeterminate(id)`, `ProgressRing::new(v).size(..).show_value()`,
`Spinner::new(id)`, `Skeleton::new(id).w(..).h(..)`, `skeleton_paragraph(id, lines, cx)`,
`TaskProgress::new(id, title)`, `StepList::new(id).step(label, state)`. When to use which, and
how components animate: [motion.md](motion.md).

## Layout

These implement `Styled` and `ParentElement`, so chain sizes and children:

- `TitleBar::new().leading(..).title(..).subtitle(..).child(..).trailing(..)` — the window's own
  title bar, one slim row (`theme.density.title_bar`). Open the window with
  `atelier::custom_frame(options, cx.theme())` so the system bar is hidden. The title is centred
  on the window; the subtitle follows it on the same line in faint text, never on a second line.
  Leading controls and children sit at the leading edge, trailing ones at the far end. Its empty
  space drags the window; children are shielded from the drag. On Windows it draws
  minimise/maximise/close and lets the OS hit-test them (snap layouts keep working); on macOS it
  leaves room for the traffic lights; on Linux it draws the three buttons too, and right-click
  opens the window menu. When one side's controls need more than half the bar the title slides
  away from them; in a narrow window it then truncates, subtitle first, and the leading children
  are cut off before they reach the title, so drop children you can do without below a width
  rather than relying on the cut. Use small controls (`ControlSize::Small`) in it.
- `WindowFrame::new()` — the window root. Paints `theme.window_fill()`; on Linux, where the
  window draws its own decorations, it adds the resize edges, the shadow and the rounded top
  corners around the content. Elsewhere, and on an X11 desktop without a compositor (which keeps
  the system frame), it is a plain full-size `div`.
- `Toolbar::new()` — a toolbar row inside the window (chrome, bottom hairline).
- `Sidebar::new(Edge::Left | Edge::Right).w(px(232.))` — persistent side region.
- `Card::new()` — grouped region, hairline, no shadow.
- `SectionHeader::new(title).description(..).action(button)`; `SectionHeader::caption(title)`.
- `ListItem::new(id, label).icon(..).icon_color(..).description(..).trailing(..)
  .selected(bool).indent(depth)` — sidebar, list and tree rows with fixed icon/trailing lanes.
- `Disclosure::new(id, label, open).on_toggle(..)`.
- `EmptyState::new(icon, title).description(..).action(button)`.

## Media and choices

- `AudioPlayer::new(id, position, "1:32", "2:00").peaks(peaks).playing(bool).volume(v)` — a
  preview player; with peaks the scrubber is a waveform. `.disabled(true)` while a take is made.
- `OptionCard::new(id, icon, title).description(..).selected(bool)`; `.agent()` when the choice
  hands work to the agent. Use a row of them when choosing is the point of the screen.

## Ornament

- `MetalBackdrop::new(id)` — liquid metal (`atelier::metal`) for the background of a welcome
  screen only: pools of chrome that merge, stretch and pinch off, shaded as a real reflective
  surface (darkening at the edges, a softbox glint, a rim light, the theme's coloured fringes at
  the horizon). Fill the screen with it and put the words and actions on a `Card` above it,
  never on the metal. It answers the pointer (drops pushed aside, ripples where it crosses the
  metal) and the window (the liquid sloshes when the window is moved or resized), and its drops
  can be picked up: press on one to carry it, carried drops fuse with whatever they touch, fused
  drops shed drops now and then (often when the pointer brushes them, at once when carried too
  fast), and a double click parts them all. There is nothing to wire up, but anything laid over the metal
  that takes clicks (the card, a bar of controls) must `.occlude()`, or presses reach the metal. `.phase(t)` draws one still frame and answers nothing. It is shaded off
  screen on its own wgpu device (one still frame on the CPU when there is no GPU) and read back at 30 frames a
  second, 60 while it answers, so keep it to the welcome screen. Do not use metal on controls or
  anywhere work happens.
- `AmbientBackdrop::new(id, Ambience::…)` — an ambient scene (`atelier::ambient`) for the same
  place, when chrome is too much: `Aurora` (curtains of light over a night sky and still water),
  `Dunes` (sheets of silk in layers), `Bloom` (clouds of colour melting together) or `Caustics`
  (light through water). Colours come from the theme unless a palette is set (below). The same rules as the metal: fill the
  welcome screen, put the words on a `Card`, nothing to wire up; it answers the window (and the
  pointer too in `Bloom` and `Caustics`, see `answers_pointer()`), `.phase(t)` draws one still
  frame, `.period(d)` changes the cycle. Without a GPU it is a single still frame. Offer
  `Ambience::ALL` (with `label()`) if the person picks the scene.
- `.palette(BackdropPalette::…)` on either backdrop — its colours: `Theme` (default: the agent
  gradient and accent), the presets `Dusk`, `Ocean`, `Ember`, `Meadow`, `Graphite`, or
  `Custom([Hsla; 4])`, warm end first. The appearance still decides how they are laid on; the
  metal stays near-neutral chrome, with the palette in its tint and its horizon fringes. Offer
  `BackdropPalette::ALL` (with `label()`) if the person picks it.
- Both backdrops shade on the crate's own wgpu device, shared with `Plot3d`. Without a hardware
  GPU, or with `ATELIER_GPU=off`, they fall back to the CPU and show one still frame that answers
  nothing (no drops to pick up either); nothing in the API changes, so do not branch on it.

## Node graphs

There is no node-graph component: compose one from the pieces above on a `canvas`, as
`examples/node_editor.rs` does, and read that example before drawing a graph.

- Each port and its wires take the colour for their data type from `colors.ports`, and the type
  is also named — on the port, in a tooltip and in a legend. Never the agent colour.
- Wires join an output to an input of the same type; while a wire is held, inputs of another
  type step back. Pulling a connected input picks its wire up again.
- A node's run state is shown with a mark or a word as well as colour (`StatusDot`, `Badge`,
  `Spinner`, `Skeleton`), and long runs use `TaskProgress`.
- A node the agent added is drawn with a dashed `colors.agent` border and a sparkle, with Keep
  and Undo on the node itself.

## Overlays

`Popover::new().w(..).child(MenuItem::new(id, "Duplicate").shortcut("Ctrl+D"))`,
`menu_separator(cx)`, `MenuItem::checked/danger/disabled/icon`. Position them with gpui's
`anchored()` + `deferred()`. Tooltips: `Button::tooltip(..)`, or on any stateful div
`.tooltip(|window, cx| Tooltip::new("Text").shortcut("⌘B").build(window, cx))`.
