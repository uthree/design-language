# Visualization

`atelier::viz` draws sound: plots for analysers, physical-model views and previews. Each plot is
a canvas that fills its parent, so wrap it in a sized `div`, usually on a `sunken` well:

```rust
div().h(px(200.)).rounded(theme.shape.radius_sm).bg(theme.colors.sunken).overflow_hidden()
    .child(Spectrum::new(levels).peaks(peaks))
```

| Plot | Data | Use |
| --- | --- | --- |
| `Waveform::new(samples)` | `-1..1` samples; `.filled()`, `.color()`, `.no_grid()` | Oscilloscopes, audio regions. Layer two with `no_grid` for L/R. |
| `Spectrum::new(levels)` | `0..1` per band, low to high; `.peaks()`, `.style(Bars \| Curve)`, `.octaves(n)` | Analysers (bars), EQ and body-response curves (curve). |
| `Heatmap::new(cols, rows, values)` | `0..1` row-major; `.signed()` for `-1..1`, `.circular()` | Spectrograms, membrane displacement, mode shapes. |
| `Vectorscope::new(points)` | `(left, right)` pairs | Goniometer / stereo image. |
| `StringLane::new(points)` | displacement along the string; `.trails()`, `.contact()`, `.inactive()` | Physical-model string motion, one lane per voice. |
| `PeakBars::new(peaks).progress(p)` | `0..1` peaks | A clip's waveform as a scrubber; used by `AudioPlayer`. |
| `NoteRoll::new(notes)` | `RollNote::new(pitch, start, length).part(n)`; `.progress(p)`, `.colors(..)` | A MIDI clip or generated phrase at a glance: an attachment preview, a candidate card. Fits pitch and time to the notes; parts take `theme.categorical`. |
| `heat_color(theme, v)` | — | A colour on the heat ramp, for legends and custom painting. |

## Colours

- `colors.series[0..4]` for curves and voices; keep a voice's colour the same in every plot and
  legend it appears in. Layer series 0 over series 1 for left/right.
- `colors.heat` for intensity. Its first stop is the well's own colour, so silence vanishes.
  Show a legend strip from `colors.heat` when the scale is not obvious.
- `colors.grid` for major lines (0 dB, octave centres) and `colors.grid_faint` for the rest.
- Document colours (a track's colour) may tint a plot that belongs to that track.

## Animating

Rebuild the plot with new data each frame. For live data, re-render from the view when a frame
arrives. For mock-ups and previews, a looping animation can feed it:

```rust
Spectrum::new(spectrum_at(0.)).with_animation("spectrum", Animation::new(LOOP).repeat(),
    |spectrum, t| spectrum.levels(spectrum_at(t)))
```

## Rules

- Label axes in the units people use (Hz with `k`, dB, LUFS, note names), in `Label::mono` at
  `XSmall`, faint. Do not label every grid line.
- Values that are read, not watched — loudness, peak, correlation — also get a number.
- Slow physical motion down rather than speed up the display, and say so in the section
  description ("Slowed down; amplitude exaggerated").
- A plot is content, not chrome: put it on `sunken`, inside a card, never on translucent chrome.
- Freeze, not pause, is the verb for stopping a live display without stopping the audio.
