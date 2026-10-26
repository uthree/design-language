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
`ProgressBar::new(0.4)`, `LevelMeter::new(level).peak(p)` (`.horizontal()`, `.thickness(px)`),
`Spinner::new(id)`, `StatusDot::new(color)`.

## Layout

These implement `Styled` and `ParentElement`, so chain sizes and children:

- `Toolbar::new()` — title bar / main toolbar row (chrome, bottom hairline).
- `Sidebar::new(Edge::Left | Edge::Right).w(px(232.))` — persistent side region.
- `Card::new()` — grouped region, hairline, no shadow.
- `SectionHeader::new(title).description(..).action(button)`; `SectionHeader::caption(title)`.
- `ListItem::new(id, label).icon(..).icon_color(..).description(..).trailing(..)
  .selected(bool).indent(depth)` — sidebar, list and tree rows with fixed icon/trailing lanes.
- `Disclosure::new(id, label, open).on_toggle(..)`.
- `EmptyState::new(icon, title).description(..).action(button)`.

## Overlays

`Popover::new().w(..).child(MenuItem::new(id, "Duplicate").shortcut("Ctrl+D"))`,
`menu_separator(cx)`, `MenuItem::checked/danger/disabled/icon`. Position them with gpui's
`anchored()` + `deferred()`. Tooltips: `Button::tooltip(..)`, or on any stateful div
`.tooltip(|window, cx| Tooltip::new("Text").shortcut("⌘B").build(window, cx))`.
