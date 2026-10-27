# Tokens and directions

```rust
use atelier::{ActiveTheme, Appearance, Direction, Theme};

atelier::init(cx, Theme::new(Direction::Glass, Appearance::Dark));  // at start-up
let theme = cx.theme();                                             // anywhere with &App
Theme::new(Direction::Glass, Appearance::Light).install(cx);        // switch live
atelier::set_window_background(window, theme);                      // when it opens and after
```

`set_window_background` replaces `Window::set_background_appearance`: it asks for the theme's
background and, on macOS, keeps the blur on a material that still draws a backdrop. Call it in
the `open_window` closure and again after every theme change.

Under Glass, `window_fill()` is clear and only chrome is tinted: do not add another background
under the title bar or sidebars, or the blur disappears. Keep content areas on opaque
`surface`/`background`.

Make the window root a `WindowFrame` (it paints `theme.window_fill()` and, on Linux, the window's
own frame) and give it `.font(theme.typography.ui.clone())`; labels inherit the interface font
and its CJK fallbacks from there. Where the system cannot blur — every Linux desktop but KDE
Plasma on Wayland — Glass asks for an opaque window and `window_fill()` is the opaque
`background`, so the chrome tints that instead of the bare desktop.

## Directions (the candidates)

| | Studio | Glass | Mono |
| --- | --- | --- | --- |
| After | Logic Pro / Final Cut Pro | Apple Liquid Glass (adopted, default) | zeron |
| Mood | dense pro tool | calm, consumer | editorial, restrained |
| Chrome | opaque graphite | translucent tint over a blurred, clear window (`Material::Translucent`) | opaque near-black / white |
| Controls | 24 px, radius 5, top-lit bezel | 30 px, capsules | 28 px, radius 7, hairlines |
| Primary | accent fill (system blue) | accent fill | solid foreground fill (`PrimaryStyle::Solid`) |
| Agent colour | warm→cool gradient (Apple Intelligence) | same | single violet |
| User bubble | accent | accent | quiet grey card |

Each direction has light and dark variants. Branch on `theme.direction` only inside a component
that genuinely draws differently; application code should read the resolved tokens instead.

## Colours (`theme.colors`)

Surfaces: `background`, `chrome`, `surface`, `elevated`, `overlay`, `sunken`.
Washes over any surface: `hover`, `pressed`, `selected`.
Lines: `border`, `border_strong`, `separator`, `highlight` (bezel), `focus_ring`.
Text: `text`, `text_muted` (≥4.5:1 everywhere), `text_faint`, `text_disabled`.
Accent: `accent`, `accent_hover`, `accent_text` (readable), `accent_wash`, `on_accent`,
`solid`, `on_solid`.
Signals: `success`, `warning`, `danger`, `info`, each with `*_wash`.
Agent: `agent`, `agent_wash`, `agent_gradient` (3 stops), `user_bubble`, `on_user_bubble`.
Creator: `record`, `playhead`, `meter` (safe, hot, clip).
Plots: `series` (4), `heat` (5-stop ramp), `grid`, `grid_faint`.
Node graphs: `ports` (5), one per data type of a port and its wires; never the agent colour.
Always label the type too (the port's name, a tooltip, a legend).
`shadow` is used by `theme.elevation`.

`theme.text_on(fill)` gives readable text on any fill, e.g. a track colour.
Helpers: `mix`, `lighten`, `contrast`, `ensure_contrast`.

## Sizes

- `theme.density`: `control_sm/md/lg`, `row`, `control_padding`, `panel_padding`, `toolbar`.
  `ControlSize::height(theme)` maps a component size to these.
- `theme.spacing`: `xxs 2 · xs 4 · sm 8 · md 12 · lg 16 · xl 24 · xxl 32` (see rules.md).
- `theme.shape`: `radius_xs/sm/md/lg/xl`, `capsule_controls`; use `theme.control_radius(h)`
  for controls and `Theme::concentric(outer, inset)` for nested surfaces.
- `theme.typography`: `ui`, `mono`, `size_xs/sm/md/lg/xl/display`, `heading_weight`,
  `label_weight`. Prefer `Label` with `TextSize`/`TextColor` over raw sizes.
- `theme.elevation`: `low`, `medium`, `high` shadow lists.
- `theme.zoomed(factor)` scales type, density, spacing and radii together.

## Adding a token

If a role is missing, add a field to `Colors` (or the relevant struct) in `src/theme.rs`, derive
it in `colors()` for every direction and appearance, and extend the contrast tests at the bottom
of that file. Never satisfy one screen with a literal.
