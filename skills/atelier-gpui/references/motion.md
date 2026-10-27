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
- `UserMessage` / `AssistantMessage` with `.id(..)`, `ApprovalCard`: rise into place on arrival.
- `Popover` with `.id(..)`: fades in on open.
- Agent panel while running: a light passes along the gradient edge.
- Streaming caret, thinking dots, spinners, skeleton shimmer, indeterminate bars: loop.

## Rules

- Motion explains a change of state or place. Never animate for decoration, and never make an
  animation the only way to see a state. The one exception is `MetalBackdrop`, which flows
  behind a welcome screen, where no work is on screen: an ambient loop makes gpui redraw the
  whole window every frame, and every frame of the metal is shaded and read back. Its response
  to the pointer and the window is part of that exception and stops with it when motion is off.
- Small state changes use `fast`; content appearing uses `normal`. Nothing user-triggered should
  take longer than `normal`.
- Do not animate large layout changes (panel widths, list reflow); fade or reveal instead.
- One looping element per region at most. A screen full of spinners says nothing.

## Progress and loading — choose by what is known

| Known | Show |
| --- | --- |
| Fraction done | `ProgressBar::new(v).id(..)` (glides between values), `ProgressRing::new(v).show_value()` in tight spots |
| Only that work is happening | `ProgressBar::indeterminate(id)`, `Spinner::new(id)`; for the model before its first token, `ThinkingIndicator` |
| The shape of coming content | `Skeleton::new(id).w(..).h(..)`, `skeleton_paragraph(id, lines, cx)` — only while it is really loading |
| A long job with a name | `TaskProgress::new(id, title).detail(..).progress(v).remaining(..).on_cancel(..)`; `.by_agent()` for the agent's work; `.state(TaskState::Succeeded).action(..)` afterwards |
| Several steps | `StepList::new(id).step(label, StepState::..)`, e.g. the agent's plan in the panel |

Show nothing for work that finishes before it could be read (under about 300 ms), and do not
show a skeleton when switching between content that is already loaded.
