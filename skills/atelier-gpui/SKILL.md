---
name: atelier-gpui
description: Design language and component kit for gpui (Rust) creator tools with built-in AI agents — themes/tokens, buttons, controls, knobs, meters, layout, the agent side panel, chat transcript, tool calls, approvals and composer, audio visualisations, 2-D and 3-D plots (wgpu-shaded), node-graph editors, and welcome-screen backdrops (liquid metal, ambient scenes, palettes). Use when building, redesigning or reviewing gpui UI that depends on the `atelier` crate (Auris Studio and other gpui creator apps), when adding an AI assistant panel or chat to a gpui app, or when asked which design candidate (Studio, Glass, Mono) or token to use, or when making an app icon or logo (two-letter element marks such as Auris Studio's "Au"). Do not use for non-gpui UI.
---

# Atelier for gpui

Atelier is a Rust crate (`atelier`, gpui `=0.2.2`) that supplies one design language in three
candidate **directions** and the components that draw it. **Glass is the adopted direction** and
the default; Studio and Mono remain as alternatives, so never assume one look — read the tokens.
Read this page fully, then the reference for the part you touch.

| Read | When |
| --- | --- |
| [references/rules.md](references/rules.md) | Always before writing or reviewing UI. The design rules and the review checklist. |
| [references/tokens.md](references/tokens.md) | Choosing a colour, size, radius, spacing step, font, or a direction. |
| [references/components.md](references/components.md) | Using buttons, controls, knobs, meters, layout containers, forms and preferences, keyboard focus, menus, node graphs, welcome-screen backdrops. |
| [references/agent.md](references/agent.md) | Anything that shows the AI: panel, transcript, tool calls, approvals, composer, entry points. |
| [references/motion.md](references/motion.md) | Animating anything, including handing one screen to the next; showing progress, loading or long-running tasks. |
| [references/visualization.md](references/visualization.md) | Plots for sound: spectra, waveforms, spectrograms, vectorscopes, physical-model views. |
| [references/plotting.md](references/plotting.md) | Plots for numbers on labelled axes: line, scatter, stem, histogram, fields with contours, 3-D surfaces, curves and point clouds. |
| [references/app-icons.md](references/app-icons.md) | Making or reviewing an app's icon or logo: the two-letter element mark. |
| [references/adopting.md](references/adopting.md) | Adding Atelier to an app, or migrating an existing gpui UI (e.g. Auris Studio) onto it. |

Before writing code, look at the crate itself (`src/` of the `atelier` dependency, or this
repository) and at the examples that compose it into whole screens: `showcase` (a DAW and the
component catalogue), `agent_desktop` (a coding-agent app), `instrument_lab` (physical-model
views), `visualizer` (analysers), `node_editor` (a ComfyUI-style node graph), `plot_lab`
(MATLAB-style 2-D and 3-D plots), `ambient` (a welcome screen over liquid metal or an
ambient scene: aurora, silk dunes, clouds of colour, caustics, a low sun, a galaxy, jellyfish,
a shoal of fish, a formation of light points or a growing garden) and
`auris_screens` (Auris's project chooser and AI compose
window, with the hand-off between them) and `preferences` (a settings window: pages, forms,
status bar, keyboard navigation).
Do not invent a builder method because its name seems plausible: check the source.

## Non-negotiables

- **Tokens, never literals.** Read `cx.theme()` (`use atelier::ActiveTheme`). No `rgb(...)`,
  `hsla(...)` or hex in application UI; the only exceptions are document data such as a track's
  own colour. Sizes come from `theme.density`, `theme.spacing`, `theme.shape`,
  `theme.typography`; a missing role is added to the theme, not hard-coded at the call site.
- **One primary per decision area.** `Button::primary()` is the default commit (the Enter
  action), not "the important-looking button".
- **The AI has one colour.** `colors.agent` / `agent_wash` / `ButtonVariant::Agent` /
  `Tone::Agent` mark only what the agent did or is doing. Never use them for decoration.
- **State is never colour alone.** Pair status colour with a mark, an icon or a word
  (`AgentStatus::mark`, badges with labels).
- **Selection is the item's own fill**, never a leading edge bar.
- **Desktop, not web.** Arrow cursor on controls; visible menus for secondary commands;
  every icon-only button has a `.tooltip(...)` when its meaning is not obvious.
- **Keyboard reach.** Every control is reachable with Tab and pressed with Enter/Space; build
  on Atelier's components (they do it) and never on a bare `div` with `on_click`.
- **Extend, don't copy.** If Atelier lacks an icon, slot or hook, use its open doors
  (`IconSource`, `.control(..)`, a container's `.id(..)`, `TitleBar::window_controls`) or add
  it here; never re-implement a component by copying its sizes and colours.
- **Every direction, both appearances.** Check a change under Studio, Glass and Mono in light
  and dark (`cargo run --example showcase -- --direction glass --appearance light`).

Finish by running the review checklist in [references/rules.md](references/rules.md).
