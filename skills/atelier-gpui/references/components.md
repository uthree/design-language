# Components

All are builders implementing `RenderOnce`; pass them to `.child(...)`. Interactive ones take an
id and optional `on_click` / `on_select` / `on_toggle` callbacks; they draw a value but do not
own it — the view keeps the state and calls `cx.notify()`. With `cx.listener` a select handler
receives `&usize`.

Assets: start the app with `Application::new().with_assets(atelier::Assets::default())`, or
`Assets::with_fallback(your_source)` to serve your own files too. Icons are `IconName::*`
drawn with `Icon::new(name).size(px).color(c)`.

**Your own icons.** Every icon argument (`Button::icon`, `ListItem::icon`, `OptionCard::new`,
`EmptyState::new`, `ContextChip::new`, `Segment::icon`, `MenuItem::icon`, …) takes
`impl Into<IconSource>`: an `IconName`, `IconSource::svg("icons/app-punch.svg")` for an app's own
single-colour glyph (drawn as a mask in the given colour, so draw it on the set's 24×24 grid
with a 2px stroke), or `IconSource::image("logos/provider.png")` for a full-colour logo. Serve
the files through `Assets::with_fallback`. Check `IconName::ALL` before drawing one: the set
includes `Undo`/`Redo`, `Bolt` (fast mode), `Keyboard`, `Volume`/`VolumeOff`, `Knob`,
`Headphones`.

## Keyboard

Every component that takes a click is a tab stop: Tab and Shift-Tab move through them in the
order they are drawn, Enter and Space press the focused one (its `on_click`, `on_select` or
`on_toggle` runs), and `.disabled(true)` takes it out of the order. A row or chip without a
handler is not a stop. Nothing to wire up beyond:

- `atelier::init` binds Tab and Shift-Tab (`FocusNext`, `FocusPrevious`).
- `WindowFrame` handles them and holds the window's root focus, so Tab works from the moment
  the window opens. A window whose root is your own element wraps it in
  `atelier::tab_navigation(root)`, puts the focus `handle` on an empty child
  (`div().absolute().size_0().track_focus(&handle)`) and focuses it once. Never `track_focus`
  the root itself: an element that tracks focus marks every press inside it handled, and the
  platform then starts no window move from the title bar.
- The focus outline (2px of `colors.focus_ring`, a pixel outside the control's corner) shows
  only after the keyboard has moved focus, never after a click (`focus_visible(cx)`). Do not
  draw your own; a custom control built from a `div` takes `.tab_index(0)` and follows the same
  rule.
- Every component given an id uses it as its `debug_selector`, so a test finds it with
  `VisualTestContext::debug_bounds("export")` (enable gpui's `test-support` in the test build).
  Do not wrap components in `div().debug_selector(..)`. `tests/keyboard.rs` shows how to drive
  Tab, Enter and Space in a test (gpui presses a control on the key's release, so simulate the
  `KeyUpEvent` too).

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

Also `.disabled(bool)`, `.full_width()`, `.on_click(|event, window, cx| ...)`, and
`.radius(px)` for a button sitting in a surface's corner, which must follow that corner:
`Theme::concentric(surface_radius, inset)`, as the composer's Send does.

## Controls

- `Switch::new(id, on).label("Snap to grid")` — a setting that applies immediately.
- `Checkbox::new(id, checked).label("Count-in")` — an independent choice. Both take
  `.disabled(bool)`.
- `SegmentedControl::new(id, [Segment::label("Light"), Segment::icon(IconName::Play)], selected)`
  `.size(ControlSize::Small).on_select(cx.listener(|this, index: &usize, window, cx| ...))`.
- `Tabs::new(id, ["Score", "Piano roll"], selected).on_select(...)` — views of one object.
- `Slider::new(0..=1 value).label("Volume").readout("-3.2 dB")`, `.bipolar()` for pan.
- `Knob::new(value).label("Drive").readout("80 %")`, `.bipolar()`, `.color(track_colour)`,
  `.diameter(px)`.
- `TextField::new().icon(IconName::Search).placeholder("Search sounds…")` — the frame of a
  text input. Atelier draws the well, the height, the radius and the ring; the application owns
  the editing (caret, selection, IME). This is the supported way to use it:

  ```rust
  TextField::new()
      .id("project-name")                        // stateful; also the test selector
      .track_focus(&self.name_focus)             // ring while focused, by itself
      .on_mouse_down(MouseButton::Left, cx.listener(Self::place_caret))
      .key_context("TextInput")
      .placeholder("Untitled")
      .empty(self.name.is_empty())               // placeholder under your input
      .trailing(Button::new("browse", "Choose…").size(ControlSize::Small))
      .child(self.name_input.clone())            // your editable element
  ```

  It is an interactive element like a `div` (`on_mouse_down`, `on_key_down`, `on_action`,
  `track_focus`, `key_context`), and `.id(..)` makes it stateful (`.on_click`). `.value(..)` is
  for mock-ups; `.focused(true)` draws the ring by hand.

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

These implement `Styled`, `ParentElement` and gpui's interactive-element traits, so chain
sizes, children and listeners. Give a container (`Sidebar`, `Toolbar`, `Card`) an `.id(..)`
first and it becomes stateful like `div().id(..)`: it can then scroll
(`Sidebar::new(Edge::Left).id("pages").overflow_y_scroll()`), track a `ScrollHandle`, take
clicks and tooltips. A sidebar whose pages can outgrow the window must scroll.

- `TitleBar::new().leading(..).title(..).child(..).trailing(..)` — the window's own
  title bar, one slim row (`theme.density.title_bar`). Open the window with
  `atelier::custom_frame(options, cx.theme())` so the system bar is hidden. The title is centred
  on the window and is the only text in the bar: there is no subtitle (a save state, a path or a
  count goes in the content or the `StatusBar`).
  Leading controls and children sit at the leading edge, trailing ones at the far end. Its empty
  space drags the window; children are shielded from the drag. On Windows it draws
  minimise/maximise/close and lets the OS hit-test them (snap layouts keep working); on macOS it
  leaves room for the traffic lights; on Linux it draws the three buttons too, and right-click
  opens the window menu. When one side's controls need more than half the bar the title slides
  away from them; in a narrow window it then truncates, and the leading children
  are cut off before they reach the title, so drop children you can do without below a width
  rather than relying on the cut. Use small controls (`ControlSize::Small`) in it.
  An application that hit-tests its caption natively (Windows snapping and double-click
  maximise through its own `WM_NCHITTEST`) keeps that and still uses `TitleBar` for the layout:
  `.window_controls(width, its_buttons)` puts its own minimise/maximise/close in place of the
  title bar's (the title stays centred against `width`), and `.window_drag(false)` leaves the
  drag region to it. On Windows 11 22H2 and later under Glass, the acrylic backdrop makes DWM
  draw its own caption buttons under the window; the title bar paints its chrome opaque there so
  they never show through its own (doubled glyphs when maximised). Do not paint it yourself.
  Do not re-implement the bar's centring: the sides carry no padding, because
  a flex basis smaller than a side's padding is raised to it and the title drifts.
- `WindowFrame::new()` — the window root. Paints `theme.window_fill()`; on Linux, where the
  window draws its own decorations, it adds the resize edges, the shadow and the rounded top
  corners around the content. Elsewhere, and on an X11 desktop without a compositor (which keeps
  the system frame), it is a plain full-size `div`. It also starts keyboard navigation (above).
- `Toolbar::new()` — a toolbar row inside the window (chrome, bottom hairline).
- `Sidebar::new(Edge::Left | Edge::Right).w(px(232.))` — persistent side region.
- `Card::new()` — grouped region, hairline, no shadow.
- `SectionHeader::new(title).description(..).action(button)`; `SectionHeader::caption(title)`.
- `ListItem::new(id, label).icon(..).icon_color(..).description(..).trailing(..)
  .selected(bool).indent(depth).disabled(bool)` — sidebar, list and tree rows with fixed
  icon/trailing lanes. The description is cut to one line; `.description_lines(2)` lets it wrap
  (use it for Japanese sidebar descriptions, which run long without spaces).
- `StatusBar::new().message("保存しました").tone(Tone::Success).busy(bool).trailing(..)` — the
  thin line along a window's bottom edge (`density.status_bar`) that reports what the last
  action did. It reports, never asks; a non-neutral tone carries its mark.

## Forms and preferences

`FormField::new(label).description(..).error(..).child(control)` — one labelled setting; the
label sits above the control (`LabelPlacement::Above`, the default) or, for a switch or
checkbox, `.beside()` it with the control at the trailing edge. `FormGroup::new(title)
.description(..)` holds fields on one surface; `FormGroup::untitled()` for a single group.
A preferences window, as `examples/preferences.rs` builds it:

- a `Sidebar(Edge::Left)` of `ListItem` pages (search field first, a setup or help link last);
- per page a `SectionHeader::new(title).description(..)`, then `FormGroup`s in a column of at
  most `theme.density.form_width`, on `colors.surface`;
- the page's actions at its foot, the primary at the trailing end, a reset as a ghost at the
  leading end;
- a `StatusBar` along the window's bottom edge.

Rules (also in [rules.md](rules.md)): labels above fields you type into, beside toggles. A
`Switch` applies at once and needs no Save; anything else on the page is committed together by
the page's primary (Save), which stays disabled until something changed. Describe what a
setting changes, not the control. Errors say what is wrong and how to fix it, next to the field.
- `Disclosure::new(id, label, open).on_toggle(..)`.
- `EmptyState::new(icon, title).description(..).action(button)`.

## Media and choices

- `AudioPlayer::new(id, position, "1:32", "2:00").peaks(peaks).playing(bool).volume(v)` — a
  preview player; with peaks the scrubber is a waveform. `.disabled(true)` while a take is made.
- `OptionCard::new(id, icon, title).description(..).selected(bool)`; `.agent()` when the choice
  hands work to the agent. Use a row of them when choosing is the point of the screen. Moving
  from a card to the screen it starts uses the hand-off in [motion.md](motion.md).

## Ornament

- `MetalBackdrop::new(id)` — liquid metal (`atelier::metal`) for the background of a welcome
  screen only: pools of chrome that merge, stretch and pinch off, shaded as a real reflective
  surface (darkening at the edges, a softbox glint, a rim light, the theme's coloured fringes at
  the horizon). Fill the screen with it and put the words and actions on a `Card` above it,
  never on the metal. It answers the pointer (drops pushed aside, ripples where it crosses the
  metal) and the window (the liquid sloshes when the window is moved or resized), and its drops
  can be picked up: press on one to carry it, carried drops fuse with whatever they touch, fused
  drops shed drops now and then (often when the pointer brushes them, at once when carried too
  fast), and a double click parts them all. There is nothing to wire up, but **anything laid
  over the metal that takes clicks (the card, a bar of controls) must `.occlude()`**, or presses
  and drags on it reach the metal underneath. While its window is inactive it holds still where
  it was and flows on from there when the window comes back (the ambient scenes do the same);
  there is nothing to add for that either. `.phase(t)` draws one still frame and answers nothing. It is shaded off
  screen on its own wgpu device (one still frame on the CPU when there is no GPU) and read back at 30 frames a
  second, 60 while it answers, so keep it to the welcome screen. Do not use metal on controls or
  anywhere work happens.
- `AmbientBackdrop::new(id, Ambience::…)` — an ambient scene (`atelier::ambient`) for the same
  place, when chrome is too much: `Aurora` (curtains of light over a night sky and still water),
  `Dunes` (sheets of silk in layers), `Bloom` (clouds of colour melting together), `Caustics`
  (light through water), `Solar` (a low sun in graded light over ridges in haze), `Galaxy` (a
  spiral galaxy turning in deep space), `Jellyfish` (jellyfish drifting up through deep water),
  `Shoal` (schools of fish wheeling through sunlit shallows), `Formation` (points of light
  gathering into one turning figure after another) or `Garden` (plants that grow, bloom and fade
  over the cycle). Colours come from the theme unless a palette is set (below). The same rules as the metal: fill the
  welcome screen, put the words on a `Card`, nothing to wire up; it answers the window, and the
  pointer in every scene but `Dunes` (see `answers_pointer()`): Bloom stirs, Caustics and Solar
  are lenses, Galaxy bends light, Jellyfish and Shoal shy away, Formation's points scatter,
  Garden's plants lean away, and Aurora's brighter stars wake
  into constellations that the person can join by dragging from star to star (a click on the
  open sky sends a shooting star, a double click wipes the lines). As with the metal, **anything
  laid over it that takes clicks must `.occlude()`**. `.phase(t)` draws one still
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

`Popover::new().w(..).child(MenuItem::new(id, "Duplicate").shortcut("Ctrl+D").on_click(..))`,
`menu_separator(cx)`, `MenuItem::checked/danger/disabled/icon`. Position them with gpui's
`anchored()` + `deferred()`. Tooltips: `Button::tooltip(..)`, or on any stateful div
`.tooltip(|window, cx| Tooltip::new("Text").shortcut("⌘B").build(window, cx))`.
