# Plotting

`atelier::plot` draws numbers on labelled axes: the scientific side of a tool — a fitted
curve, a parameter sweep, a simulation's output — in the spirit of MATLAB's figures. Where
`atelier::viz` draws sound in fixed units (dB, Hz) without tick labels, these plots take data in
its own units and draw the axes for it. Each plot is a canvas that fills its parent, so wrap it
in a sized `div` on a `sunken` well, as with `viz`:

```rust
div().h(px(260.)).rounded(theme.shape.radius_sm).bg(theme.colors.sunken).overflow_hidden()
    .child(LinePlot::new()
        .series(Series::line(points).name("ζ = 0.04"))
        .series(Series::markers(measured).name("measured"))
        .x_label("t [s]").y_label("x [m]").legend(true))
```

| Plot | MATLAB | Data | Options |
| --- | --- | --- | --- |
| `LinePlot` + `Series::line` | `plot` | `(x, y)` points per series | `.style(Line \| Markers \| LineMarkers \| Stems \| Steps \| Area)`, `.name()`, `.color()`, `.faint()`; plot: `.x_range()`, `.y_range()`, `.x_label()`, `.y_label()`, `.legend()`, `.grid()` |
| `Series::markers` | `scatter` | `(x, y)` points | as above |
| `Series::stems` | `stem` | `(n, value)` | the y range always includes 0 |
| `Histogram::new(samples)` | `histogram` | raw samples | `.bins()`, `.range()`, `.y_range()`, `.density()`, `.overlay(curve, name)` for the expected distribution, `.name()` |
| `FieldPlot::new(cols, rows, values)` | `imagesc` + `contour` | row-major, **first row at y minimum** | `.x_range()`, `.y_range()`, `.value_range()`, `.signed()`, `.contours(n)` |
| `Colorbar::new(min, max)` | `colorbar` | — | `.signed()`, `.label()`; give it a 64 px wide column beside the plot it explains |
| `Plot3d::new().surface(cols, rows, z, x, y)` | `surf`, `mesh` | heights, row-major, first row at y minimum | `.shading(Faceted \| Smooth \| Mesh)` |
| `Plot3d::new().curve(points, color)` | `plot3` | `[x, y, z]` | no colour: the heat ramp runs along the curve, so direction shows |
| `Plot3d::new().scatter(points, color)` | `scatter3` | `[x, y, z]` | one call per class, in series colours |
| `Plot3d::new().particles(points, color)` | — | `[x, y, z]` | a glowing swarm or flow, thousands of points; the colour's opacity sets the brightness |
| `nice_ticks`, `format_tick` | — | — | the tick rule, for custom painting |

`Plot3d` takes `.view(azimuth, elevation)` in degrees, in MATLAB's convention (default
−37.5°, 30°; top is `(0, 90)`), `.labels(x, y, z)` and `.x_range()`, `.y_range()`,
`.z_range()`; a fixed z range also fixes a surface's colour scale. It is orthographic, draws
its back walls with the grid, and labels the two front floor edges and the leftmost upright.

Where there is a GPU, `Plot3d` shades its surface, curves and points off screen with wgpu at
the display's resolution, four samples a pixel and a depth buffer, and gpui draws the result as
one image between the walls and the labels: the surface is lit per pixel, its edges stay one
width at any angle, curves and points hide behind it, markers are small lit beads, and
particles glow (adding up on dark themes). Without one, or with `ATELIER_GPU=off`, it draws
the same plot with gpui paths: flat facets, painter's order, particles as faint dots. Nothing
in the API changes between the two, so do not branch on it.

## Axes

- Ticks fall on 1, 2, 2.5 and 5 times a power of ten; labels use the mono face at `XSmall`,
  faint, with a true minus sign. Do not set ticks by hand.
- Title axes with the quantity and its unit in brackets: `t [s]`, `V [kq/m]`. The y title sits
  above the axis, horizontal; text is never rotated.
- x spans the data exactly by default (`axis tight`); y and z round out to whole ticks. Fix a
  range when several plots must compare, or when the data has a natural one (a probability, a
  clamped field).
- Zero lines are drawn stronger than the grid. Turn the grid off for fields, which have their
  own colour.

## Colours

- Series take `colors.series[0..4]` in order. A theoretical or reference curve behind measured
  data is `.faint()`; a curve over a histogram is the second series colour.
- Unsigned fields and surfaces use the heat ramp from a step in, so the minimum is still a
  colour on the well. Signed fields (`.signed()`) run second series colour → well → first, so
  zero is the background and the sign is the hue. Always pair a field or surface with a
  `Colorbar` of the same ramp.
- Lighting on surfaces only darkens; the hue is the value.

## Animating

Plots take plain data, so a moving plot is the same plot rebuilt each frame from a clock.

- Keep one simulation clock in the view: advance it by the frame time in `render` while
  playing (cap the step, so a hidden window does not jump), and call
  `window.request_animation_frame()`. Every plot reads the same `t`, so they stay in step and
  pause together. Give the clock a Pause/Play button, a restart, and show `t` in the mono face.
- **Fix the ranges of anything that moves** (`x_range`, `y_range`, `z_range`, the histogram's
  `y_range`, a field's `value_range` and its colour bar). Auto-scaled axes that rescale every
  frame make motion unreadable.
- Show where the motion is: a marker at the head of a curve being traced, a comet tail (the
  last stretch of a 3-D curve with no colour, so the heat ramp brightens toward the head) over
  the whole path drawn in `grid_faint`.
- Run at the physics' own speed where people can follow it (an oscillator in real time) and
  say so; slow down anything faster ("slowed down").
- A loop that ends should hold its final picture for a moment before it starts again.

## 3-D interaction

- Turn a 3-D plot by dragging it: wrap it in a well with `.cursor_grab()` and update azimuth
  and elevation in `on_mouse_move` while dragging (see `examples/plot_lab.rs`). Offer the
  standard views (3-D, Top, Front, Side) as a segmented control, and show the angles as
  readouts.
- Plots that show the same space share one view, so turning one turns all.
- A turntable is an option the person switches on, never the default; it turns with the
  clock, so pausing stops it too.

## Rules

- A plot is content: on `sunken`, inside a card, with a title saying what is plotted and a
  description saying the conditions (parameters, sample size, units).
- A legend only when there is more than one named series; name series in the reader's terms
  (`ζ = 0.15, measured`), not variable names.
- Values that are read, not looked at — a mean, a fit statistic — also get a number, as badges
  under the plot.
