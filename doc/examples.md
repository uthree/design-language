# Examples

Each example is a mock: it shows fixed or synthesised data and is wired to nothing. All of them
take `--direction glass|studio|mono`, `--appearance dark|light` and `--size WIDTHxHEIGHT`, and
carry the same switch in their title bar. Every example lays out from 900 × 600 (its minimum
window size) up to a large display: narrower windows drop the sidebar first, and spare height
goes to the plots. Screenshots are Glass dark, captured on Windows with `PrintWindow`, which does
not include the desktop blur behind translucent chrome.

## showcase — a DAW arrangement view

```bash
cargo run --example showcase
```

Track list, timeline with AI-drafted clips, inspector with knobs and meters, and the assistant
panel. The Components page lists every component, with live controls so transitions can be
watched, and a "Progress and loading" section.

## agent_desktop — a coding-agent app

```bash
cargo run --example agent_desktop
```

Sessions with live status on the left, the conversation in the middle (reasoning, grouped tool
calls, command output, a diff, the task plan, an approval), and the changes it made on the right.

![Agent desktop](images/agent-desktop.png)

## instrument_lab — a physical-model instrument

```bash
cargo run --example instrument_lab
```

Auris Studio's physical-instrument view: four voices' string motion with excitation points and
fading traces, the partials the bow position leaves in, the body's resonance curve and a drum
head's displacement, with exciter, string and body parameters and an agent fitting the model to
a reference. The motion is a sum of string modes computed per frame.

![Instrument lab](images/instrument-lab.png)

## visualizer — a master-bus analyser

```bash
cargo run --example visualizer
```

Spectrum with peak hold, R128 loudness readouts and meters, spectrogram, goniometer with phase
correlation, and a two-channel oscilloscope, driven by a synthesised programme.

![Visualizer](images/visualizer.png)

## node_editor — a node-based workflow editor

```bash
cargo run --example node_editor
```

A ComfyUI-style graph for generating a pad: model loader, prompts, sampler, decoder and outputs,
with the node library on the left and the selected node's inputs, parameters and the run queue on
the right. Nodes drag; wires are pulled from an output onto an input of the same type (inputs of
another type step back while a wire is held), and pulling a connected input picks its wire up
again. Empty space or the scroll wheel pans. Run steps through the graph in dependency order,
shows each node's state and the sampler's progress, and reuses unchanged nodes on the next run.
Port and wire colours come from `colors.ports`, with the type also named on the port, in a tooltip
and in the legend. The loudness node was added by the assistant: it is drawn dashed in the agent
colour with Keep and Undo on the node, and the assistant panel holds the conversation.

## plot_lab — a numerical workbench

```bash
cargo run --example plot_lab
cargo run --example plot_lab -- --figure 3d
```

Plots for maths and physics in the manner of MATLAB, built on `atelier::plot`, with a
workspace of variables and the command history on the left. Everything runs on one simulation
clock, recomputed each frame; the title bar pauses it, restarts it and shows `t`. Figure 1 is
2-D: a damped oscillator at three damping ratios traced in real time, a histogram filling with
molecular speeds until it matches the Maxwell–Boltzmann density, three Van der Pol trajectories
winding onto their limit cycle, a square wave's Fourier coefficients (stems) and its partial sum
travelling (the term count is switchable), and a turning dipole's potential with equipotentials,
a colour bar and cuts through it. Figure 2 is 3-D: a square drum head with five modes ringing,
the Lorenz attractor with a comet tail running round it and a swarm of 2,400 glowing particles
that leaves the comet's head as a tight ball every twelve seconds and spreads over the whole
attractor, and three point clusters drifting. With a GPU the 3-D plots are shaded with wgpu
(per-pixel lighting, depth, four samples a pixel); `ATELIER_GPU=off` shows the gpui-path
fallback.
Drag any of them to turn all three; the inspector has the standard views, the shading
(faceted, smooth, mesh) and a turntable.
These two screenshots were captured on Linux under Xvfb.

![Plot lab, 2-D](images/plot-lab-2d.png)

![Plot lab, 3-D](images/plot-lab-3d.png)

## ambient — a welcome screen over liquid metal or an ambient scene

```bash
cargo run --release --example ambient -- --backdrop metal|aurora|dunes|bloom|caustics
```

A welcome card over the backdrop picked in the title bar: liquid metal or one of four ambient
scenes. A bar below the card picks the backdrop's palette (`--palette
theme|dusk|ocean|ember|meadow|graphite`): the theme's own colours or a preset, through
`.palette(BackdropPalette::…)` on either backdrop.

### Liquid metal

`MetalBackdrop`: pools of liquid chrome that merge, stretch and pinch off
as their drops drift. The liquid is a metaball field; its height gives each pixel a normal, and
the reflected view looks up a studio — the theme's `metal` ramp from sky to ground with its
coloured fringes at the horizon, a softbox, a strip light behind, and dark behind the metal, so
edges darken as polished metal does. gpui takes no custom shaders, so the backdrop shades each
frame with a WGSL shader on its own small wgpu device, off screen, and hands gpui the read-back
frame as an image: four samples a pixel, thin dispersion fringes, ripples running over the
surface and a second softbox. Without a hardware GPU it falls back to the CPU (about 7 ms per
new frame for a 1300 × 750 window in a release build) with a single sample and no extras. The
liquid answers the pointer, which pushes drops aside and leaves ripples where it crosses the
metal, and the window, which leaves the liquid sloshing when it is moved or resized. Metal is
kept to the welcome screen: on controls it made sizes uneven and said too much. Captured on Linux under Weston.

![Liquid metal welcome, Glass dark](images/liquid-metal-welcome-glass-dark.png)

![Liquid metal welcome, Mono light](images/liquid-metal-welcome-mono-light.png)

### Ambient scenes

`AmbientBackdrop` behind the same card: four slow, abstract scenes in the manner of a desktop
wallpaper. **Aurora** hangs curtains of light in leaning rays
over a night sky with stars, mirrored in still water below. **Dunes** lays sheets of silk one in
front of another, each lit from above with a sheen on its slopes and a soft shadow on the one
behind. **Bloom** melts great clouds of colour into one another, stirred by slow eddies under a
frosted sheen. **Caustics** plays light through moving water over a deep floor, the colours
bending apart into thin fringes, with shafts of light from the surface. Every scene takes its
colours from the theme — background, agent gradient and accent — as light over the dark in a dark
appearance and as washes of colour in a light one.

Each is a WGSL shader on the crate's wgpu device, shaded off screen and read back like the metal.
Each answers the window: moving or resizing it leaves the scene behind for a moment, nearer
layers further, and it swings back. Bloom and Caustics also answer the pointer — stirring the
colour round itself, or gathering the light like a lens — and leave ripples as it travels; Aurora
and Dunes leave it alone, so the sky and the silk stay still.
Without a hardware GPU the backdrop is one still frame shaded on the CPU, with no animation and
no response. Captured on Linux under Weston.

![Aurora, Glass dark](images/ambient-aurora-glass-dark.png)

![Dunes, Glass dark](images/ambient-dunes-glass-dark.png)

![Bloom, Glass light](images/ambient-bloom-glass-light.png)

![Caustics, Mono dark](images/ambient-caustics-mono-dark.png)

## auris_screens — two Auris Studio windows redrawn

```bash
cargo run --example auris_screens -- --screen chooser
cargo run --example auris_screens -- --screen compose
```

**Project chooser.** What changed from the current window: the two ways to start are choice
tiles with a description each, instead of an icon and a caption under one of them; the AI tile
carries the agent colour. Setup guidance moved to the sidebar, since it is help rather than a
way to start. The primary button names its result ("AIで作り始める…" when the AI tile is
chosen) and is the only primary in the window.

![Project chooser](images/auris-chooser.png)

**AIで曲を作る.** The request and reply read as a conversation. Each take is a card with its
tempo and key, a waveform player rather than a bare slider, "この曲で続ける" and a quiet
"作り直す". The take still being made is tinted in the agent colour and shows its steps and
time left instead of an empty player. Stopping generation moved into the composer (Send becomes
Stop), so the footer keeps only navigation, the preference switch and the editor hand-off,
which explains why it is disabled.

![AI compose](images/auris-compose.png)
