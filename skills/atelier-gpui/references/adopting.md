# Adopting Atelier

## Add the dependency

```toml
[dependencies]
gpui = "=0.2.2"
atelier = { git = "https://github.com/uthree/design-language" }   # or { path = "../design-language" }
```

Atelier pins gpui `=0.2.2`, the release Auris Studio uses, so both resolve to one gpui. When the
app moves to a newer gpui, move Atelier in the same change.

On Linux, gpui 0.2.2 pulls in `xattr` 0.2, which does not build against `libc` 0.2.190 or later
(`libc::ENOATTR` is gone). Keep `libc` at 0.2.189 in the app's lockfile —
`cargo update -p libc --precise 0.2.189` — until gpui drops it. Building also needs the
system's xkbcommon, Wayland, Vulkan and fontconfig headers (`libxkbcommon-x11-dev`,
`libwayland-dev`, `libvulkan-dev`, `libfontconfig-dev` on Debian and Ubuntu).

```rust
use atelier::{ActiveTheme, Appearance, Assets, Direction, Theme};

Application::new().with_assets(Assets::with_fallback(AppAssets)).run(|cx| {
    let theme = Theme::new(Direction::Glass, Appearance::Dark);
    let background = theme.window_background();
    atelier::init(cx, theme);
    let options = WindowOptions { window_background: background, ..Default::default() };
    cx.open_window(atelier::custom_frame(options, cx.theme()), |window, cx| {
        atelier::set_window_background(window, cx.theme());         // keeps the macOS blur alive
        ...                                                          // draw TitleBar in the root view
    });
});

// root view: WindowFrame paints window_fill() and, on Linux, the window's frame
WindowFrame::new().font(theme.typography.ui.clone()).text_color(theme.colors.text).flex_col()
```

Persist the choice with `Direction::id()` / `Direction::from_id()` and `Appearance`.

Call `atelier::init` once at start-up (calling it again only installs the theme) and switch
themes later with `Theme::install`.

`atelier::init` also binds Tab and Shift-Tab for keyboard navigation, and `WindowFrame` handles
them; an app whose window root is not a `WindowFrame` wraps its root in
`atelier::tab_navigation(..)` and focuses it once (see [components.md](components.md),
"Keyboard"). If the app binds Tab itself in a context (a code editor inserting a tab), its
binding wins there.

Application shortcuts on a single key need nothing more: every Atelier tab stop carries the key
context `AtelierControl`, and Atelier takes Enter and Space there before any binding, so Space
for play does not fire while a button holds focus. Bind letter keys and anything your own focusable views must not see with
`Some("!AtelierControl")` (see [rules.md](rules.md), "Components and states").

## Keep up with releases

Applications pin a git revision. When you move it, read [CHANGELOG.md](../../../CHANGELOG.md)
between the old and new revision — each release ends with **Workarounds you can delete** — and
delete them. Then run the adoption check over your sources:

```sh
path/to/atelier-gpui/tools/check-adoption.sh src            # errors fail; --strict fails on warnings too
```

It finds Atelier components wrapped in `div().debug_selector(..)`, hand-made tab stops, copies of
components (`nav_item`, `assistant_message`, `selectable_text`, a `*_copy_button`), keys claimed
area by area, re-implemented `Plot3d` projections, hand-made category colours and colour
literals, each with what replaces it. Add it to the application's CI next to `cargo clippy` so
code written later — by people or agents copying nearby code — cannot bring a workaround back.
Mark a line you keep on purpose with `// atelier: allow`.

## Testing

Components use their id as gpui's `debug_selector`, so with gpui's `test-support` feature on in
the test build (`[dev-dependencies] gpui = { version = "=0.2.2", features = ["test-support"] }`)
a `VisualTestContext` finds them directly:

Loop a screen test over every theme with `for theme in Theme::all() { ... }`.

```rust
let bounds = cx.debug_bounds("save").unwrap();
cx.simulate_click(bounds.center(), Modifiers::default());
// Keyboard: gpui presses a focused control on the key's release.
cx.simulate_keystrokes("tab");
cx.simulate_event(KeyUpEvent { keystroke: Keystroke::parse("enter").unwrap() });
```

Do not wrap Atelier components in `div().debug_selector(..)` to find them.

## Migrating an existing gpui UI (Auris Studio)

Auris already has its own `Theme`, `Scheme`, `Metrics` and `ui::widgets`. Migrate in layers, not
in one rewrite:

1. **Map tokens.** Auris `surface_raised` → `elevated`, `surface_sunken` → `sunken`,
   `surface_hover` → `hover`, `border_subtle` → `separator`, `accent_soft` → `accent_wash`,
   `text_on_accent` → `on_accent`, `playing`/`playhead`/`record`/`meter_*` → `playhead`,
   `record`, `meter`. Keep Auris's document palettes (track, velocity, chord colours) in Auris:
   they are data, not interface tokens.
2. **Swap leaf widgets** (`button`, `icon_button`, `disclosure`, `readout`, `level_meter`) for
   Atelier components behind the existing function signatures, one module at a time.
3. **Rebuild the agent panel** on `atelier::agent`: Auris `ChatEntry::You` → `UserMessage`,
   `Agent` → `AssistantMessage` (with the gpui-component Markdown view via `.body`),
   `Reasoning` → `ReasoningBlock`, consecutive `Tool` → `ToolGroup` of `ToolCall`,
   `Status`/`RestoredContext`/`Note` → `ChatNotice::status`, `Error` → `ChatNotice::error`.
   `AgentPanelStatus` maps one-to-one onto `AgentStatus`; the chrome switch becomes
   `AgentToggle`. Keep Auris's own `TextField` as the composer's `.input(...)`.
4. **Then layout**: toolbar, sidebars, inspector using `Toolbar`, `Sidebar`, `SectionHeader`,
   `ListItem`; the settings window on `FormField`, `FormGroup` and `StatusBar`
   (`examples/preferences.rs`).

Where Auris copied an Atelier component's sizes to get something it lacked, replace the copy
with the component and the hook it now has:

| Auris workaround | Atelier now |
| --- | --- |
| `nav_item` (a `ListItem` copy with `tab_index` and a focus ring) | `ListItem` — a tab stop with the ring; `.description_lines(2)` for long Japanese descriptions |
| own `button` for keyboard reach and `debug_selector` | `Button` — tab stop, Enter/Space, id as selector |
| fields copying `TextField`'s colours around an editable `div` | `TextField::new().id(..).track_focus(..).on_mouse_down(..).empty(..).child(input)` |
| `agent_components::assistant_message` without the name line | `AssistantMessage::anonymous()` |
| `auris-redo.svg`, `auris-bolt.svg`, `Code` for the keyboard page | `IconName::Redo`, `Bolt`, `Keyboard`; own SVGs and logos via `IconSource::svg/image` |
| `div().debug_selector(..)` wrappers | the component's id |
| `titlebar::composed` | `TitleBar::window_controls(width, buttons).window_drag(false)` |
| hand-made panel-to-screen transition | `Morph` + `Measure` ([motion.md](motion.md)) |
| pointer blocking on panels over the metal, stilling it when inactive | `.occlude()` (still needed); stilling is built in |
| a row of composer controls copied beside the composer | `Composer::control(..)`, `.empty(..)` |
| an opaque title bar on Windows so DWM's caption buttons do not show through | built into `TitleBar` under the acrylic backdrop |
| `ClaimVisualizationActivation` bound to Enter/Space per area | nothing: controls claim the keys (`AtelierControl`) |
| `agent_components::selectable_text` and a redrawn user bubble | `UserMessage::new(text).id(..)`; `SelectableText` elsewhere |
| copy buttons swapping their own label (`workshop_copy_button`) | `CopyButton::new(id, text).label(..).copied_label(..)` |
| `audio_player.rs` for seek, volume, mute, loading | `AudioPlayer::on_seek/on_volume/on_mute/loading` |
| `TimbreCamera`, `scatter_batches`, own `pick` | `scatter` per colour; `Plot3d::id(..).on_pick(..).on_orbit(..)`, `ViewPreset` |
| `group_color` (golden-ratio hues) | `theme.categorical(index)` |
| `has_global::<Theme>()` to choose `init` or `install` | `init` once, `Theme::install` to switch |
| the `/model` completion list in gpui-component spacing | `Completions` / `Composer::completions` |
| a hand-made attachment preview panel | `Card` + `SelectableText` / `AudioPlayer`, closed with a ghost `Button::icon_only(.., IconName::Close)` |

Check each step under all three directions in both appearances, and keep Auris's existing visual
tests passing.
