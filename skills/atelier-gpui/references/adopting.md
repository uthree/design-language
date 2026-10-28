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

`atelier::init` also binds Tab and Shift-Tab for keyboard navigation, and `WindowFrame` handles
them; an app whose window root is not a `WindowFrame` wraps its root in
`atelier::tab_navigation(..)` and focuses it once (see [components.md](components.md),
"Keyboard"). If the app binds Tab itself in a context (a code editor inserting a tab), its
binding wins there.

## Testing

Components use their id as gpui's `debug_selector`, so with gpui's `test-support` feature on in
the test build (`[dev-dependencies] gpui = { version = "=0.2.2", features = ["test-support"] }`)
a `VisualTestContext` finds them directly:

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

Check each step under all three directions in both appearances, and keep Auris's existing visual
tests passing.
