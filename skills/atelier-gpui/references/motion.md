# Motion, progress and loading

## How animation works in gpui

gpui has no implicit transitions: changing a style does not tween it. It has time-based
animations (`element.with_animation(id, Animation, animator)`) that run the animator from 0 to 1
once per element id, or forever with `.repeat()`. Atelier builds what components need on top of
that, in `atelier::motion`:

| Helper | Use |
| --- | --- |
| `transition(element, id, &value, duration, window, cx, animator)` | Animate each time `value` changes (never on first draw). The animator moves the element from the previous state to the current one. |
| `tween(element, id, f32_value, duration, window, cx, animator)` | Animate a number: the animator gets the interpolated value. Bars and rings glide with it. |
| `appear(element, id, cx, animator)` | Play once when the element is first drawn: a message arriving, a popover opening. Replays if the element leaves and comes back. |
| `reveal(div, id, cx)` | `appear` with the standard fade and settle, for content a disclosure just opened. |
| `looping(element, id, duration, rest, cx, animator)` | Repeat forever: spinners, shimmer, indeterminate bars. Drawn still at `rest` when motion is off. |
| `chevron(id, open, size, color, window, cx)` | The disclosure chevron that turns a quarter turn. |
| `change(id, &value, window, cx)` | The underlying record: change count and previous value. |
| `Morph::new(id, from_bounds)` with `Measure` | The hand-off between screens: a shape that grows from the rectangle the person acted on to the new screen (below). |

Ids must be stable across frames and unique per element: derive them from the component id,
e.g. `(self.id.clone(), "thumb")`. The animator receives the element by value and returns it,
so it can call any builder method — including a component's own (`|bar, v| bar.value(v)`).

## Tokens

`theme.motion`: `enabled`, `fast` (140 ms: switch thumbs, check marks, chevrons, selection),
`normal` (220 ms: revealed content, arriving messages, popovers, value glides), `slow` (1.4 s:
shimmer passes, indeterminate sweeps, the running edge). Easing is `motion::ease_out` (cubic)
for anything arriving or settling.

Set `theme.motion.enabled = false` for a reduced-motion preference. State still changes
instantly; looping indicators stand still in a readable pose.

## What animates in Atelier

- Switch: thumb slides, track cross-fades. Checkbox: mark grows in.
- Segmented control: the selected fill cross-fades between segments.
- Disclosure, reasoning, tool calls: chevron turns; revealed content fades and settles.
- Tool calls, tasks, plan steps: the state mark fades in when the state changes.
- `UserMessage` / `AssistantMessage` with `.id(..)`: fade in on arrival, without moving. `ApprovalCard`: rises into place.
- `Popover` with `.id(..)`: fades in on open.
- Agent panel while running: a light passes along the gradient edge.
- Streaming caret, thinking dots, spinners, skeleton shimmer, indeterminate bars: loop.

## Handing one screen to the next

When an action on one screen opens another in the same window — a choice card on the welcome
screen opening the compose screen — let the card's shape carry the eye across, and move nothing
else:

1. Put `measure.probe()` inside the card (`relative()`) so its window bounds are known; keep the
   `Measure` in the view.
2. On the action, store `measure.bounds()` and switch screens in the same update.
3. In the window's root (`relative()`), draw `Morph::new(("handoff", n), from)` first — it grows
   from the card to the root's bounds over `motion.normal`, from the card's radius to square,
   in the card's fill (`.fill(colors.elevated)`) — and then the arriving screen wrapped in
   `appear(.., |screen, delta| screen.opacity(delta))` so it fades in over the shape.

**Only the shape moves.** The arriving screen is laid out at its final size from its first frame
and only fades; never scale it, slide it or re-lay it out while the shape grows, and never move
the old screen's content along with the shape. Number each hand-off (`("handoff", n)`), since a
`Morph` plays once per id. With motion off the shape is at its end at once. `auris_screens`
does this from "AIで曲を作る…" to the compose window.

## Rules

- Motion explains a change of state or place. Never animate for decoration, and never make an
  animation the only way to see a state. The one exception is the welcome-screen backdrops,
  `MetalBackdrop` and `AmbientBackdrop`, which flow where no work is on screen: an ambient loop
  makes gpui redraw the whole window every frame, and every frame is shaded and read back.
  Their response to the pointer and the window is part of that exception and stops with it
  when motion is off.
- Small state changes use `fast`; content appearing uses `normal`. Nothing user-triggered should
  take longer than `normal`.
- Do not animate large layout changes (panel widths, list reflow); fade or reveal instead.
- **In a list that grows from the bottom** — a conversation, a log — a new row only fades in.
  Never move it with margin, padding or height: the list is pinned to its end, so a row that
  changes size as it arrives jolts everything above it and shifts the person's selection.
  Atelier's transcript rows do this already.
- One looping element per region at most. A screen full of spinners says nothing.

## Progress and loading — choose by what is known

| Known | Show |
| --- | --- |
| Fraction done | `ProgressBar::new(v).id(..)` (glides between values), `ProgressRing::new(v).show_value()` in tight spots |
| Only that work is happening | `ProgressBar::indeterminate(id)` or `Spinner::new(id)` (see below); for the model before its first token, `ThinkingIndicator` |
| The shape of coming content | `Skeleton::new(id).w(..).h(..)`, `skeleton_paragraph(id, lines, cx)` — only while it is really loading |
| A long job with a name | `TaskProgress::new(id, title).detail(..).progress(v).remaining(..).on_cancel(..)`; `.by_agent()` for the agent's work; `.state(TaskState::Succeeded).action(..)` afterwards |
| Several steps | `StepList::new(id).step(label, StepState::..)`, e.g. the agent's plan in the panel |

**Spinner or indeterminate bar.** Both say "working, length unknown"; pick by where the waiting
sits and what will replace it:

- `Spinner` — inline and small, next to the thing that is waiting: a button's label while it
  works, a row or a chip whose result is pending, a status line (`StatusBar::busy`), the first
  line of a card being generated. It takes no room of its own, so it fits wherever text fits.
- `ProgressBar::indeterminate` — a region's own progress slot, where a fraction will appear once
  it is known: a task card, an export, a download that has not reported its size yet. Switch the
  same bar to `ProgressBar::new(v)` as soon as there is a fraction, so it does not jump.
- A generating take (Auris's 作曲中 card) is a long named job: `TaskProgress` with an
  indeterminate bar while the stage is unknown, a fraction when there is one, and a `StepList`
  of stages; the card title may carry a `Spinner` badge. Not a lone spinner in the middle of an
  empty card.

Never both for one piece of work, and never a spinner in a region that has a progress slot.

Show nothing for work that finishes before it could be read (under about 300 ms), and do not
show a skeleton when switching between content that is already loaded.
