# Choosing a control

Start from what the person is handling, not from the widget you have in mind. Find the
concept in the left column, use the component in the middle, and check the right column before
settling: it names the neighbour that is easy to pick by mistake. The `control_legend` example
draws every row live (`cargo run --example control_legend`); its pages follow the sections
below. API details are in [components.md](components.md), the reasons in [rules.md](rules.md).

Ask in this order:

1. **Is it a command or a value?** A command does something once (Button, MenuItem). A value is
   a state that stays (everything else). Never make a button that toggles a value unless it is a
   latch (below), and never make a switch that runs a command.
2. **Does it apply at once, or with the page's Save?** At once: `Switch`, latches, segmented
   choices on a canvas. With Save: `Checkbox`, fields, choices in a form.
3. **How many options, and must they all be seen?** 2–5 short ones, always visible:
   `SegmentedControl`. More, long names, or logos: a menu button. A few that need explaining and
   are the point of the screen: `OptionCard`s.
4. **Is it continuous?** A label and readout on one line: `Slider`. A dense strip of timbre
   parameters: `Knob`. Reported, not set: a meter or progress.

## Turning something on or off

| Concept | Use | Not |
| --- | --- | --- |
| A setting that takes effect at once ("Autosave", "Snap to grid") | `Switch`, in a form `FormField::beside()` | `Checkbox` — it reads as "pending until Save" |
| An independent yes/no committed with the rest of the page ("Use GPU") | `Checkbox`, `FormField::beside()` | `Switch` followed by a Save button |
| Several independent options from one set ("Export: stems, MIDI, lyrics") | a column of `Checkbox`es under one `FormField`/`FormGroup` | a `SegmentedControl` (it picks exactly one) |
| A state of one thing in a dense strip — mute, solo, arm, loop, play | `Button::selected(on).tint(..)` (latch; `colors.info` mute, `warning` solo/loop, `record` arm) | `Switch` — too wide, and it reads as a preference |
| A toggle inside a menu ("Snap to grid" in the clip menu) | `MenuItem::checked(on)` | a `Switch` laid in a popover |
| Showing or hiding a block of detail | `Disclosure` | a `Switch` (it changes nothing, it only reveals) |

## Choosing one of several

| Concept | Use | Not |
| --- | --- | --- |
| One of 2–5 short options, all visible (language, engine, tool) | `SegmentedControl` (`Segment::label` / `icon`) | `Tabs` — tabs change the view, not a value |
| One of many (6+), long names, or options led by a logo (model, grid, sample rate) | `Button::icon_after(IconName::ChevronDown)` that opens a `Popover` of `MenuItem::checked(..)`, grouped under `Label::caption` headings | a long `SegmentedControl`; a list of radio-style checkboxes |
| One of hundreds, found by typing (emoji, commands, sounds) | `TextField` + `Completions` under or over it | a menu that scrolls forever |
| A choice that *is* the screen, each option needing a sentence (new project template, hand to the agent or do it yourself) | a row of `OptionCard`s; `.agent()` on the one that hands work to the agent | a `SegmentedControl` with tooltips |
| Which view of the same object to show (Score / Piano roll) | `Tabs` | `SegmentedControl` in a form |
| Which page, place or document to be in | `Sidebar` of `ListItem::selected(..)` (`.indent(depth)` for a tree) | `Tabs` with a dozen entries |

## Setting an amount

| Concept | Use | Not |
| --- | --- | --- |
| A continuous value with its label and readout on one line (volume, tempo, strength) | `Slider::new(v).label(..).readout(..)`; `.bipolar()` around a centre (pan) | a `Knob` in a roomy row |
| Many continuous parameters packed together (drive, tone, decay) | `Knob`, `.bipolar()` for ± values, the readout in `Label::mono` | a stack of sliders in an inspector strip |
| An exact number the person types (BPM 128.00, a port) | `TextField` with the value in mono, the unit as `.trailing(..)` | a slider whose readout cannot be typed into |
| Position in a piece of audio, and its volume | `AudioPlayer` with `.on_seek(..)` / `.on_volume(..)` | a `Slider` beside a play button |
| How much effort the agent spends | `agent::EffortButton` / `EffortPicker` (see [agent.md](agent.md)) | a generic slider |

The application owns dragging and keyboard stepping for `Slider` and `Knob` (it owns the value
and its undo); wrap them in an element that handles the gesture.

## Reporting an amount (read-only)

| Concept | Use | Not |
| --- | --- | --- |
| Work done of a known total | `ProgressBar` / `ProgressRing::show_value()`; a named long task `TaskProgress` | a `Slider` (it invites dragging) |
| Waiting of unknown length | `ProgressBar::indeterminate`, `Spinner`; content that will arrive `Skeleton` | a bar that jumps to 90 % and stops |
| A live signal level | `LevelMeter` | a `ProgressBar` |
| A quota or budget (tokens, minutes) | `agent::UsageMeter` | a bare percentage label |
| Steps of a plan | `StepList` | a numbered list of labels |

## Entering text

| Concept | Use | Not |
| --- | --- | --- |
| A short single line (a name, an address) | `TextField` (the frame; the application owns the editing) under a `FormField` label | a placeholder used as the label |
| Searching or filtering | `TextField::icon(IconName::Search)`, at the top of the list it filters | a "Search" button |
| A file or folder | `TextField` showing the path + `.trailing(Button::new(.., "Choose…"))` | a bare button that hides the current path |
| A message or prompt to the agent | `agent::Composer` | a `TextField` with a Send button beside it |
| Text to read and copy (a message, a log, an error) | `SelectableText` + `CopyButton` | a `TextField` made read-only |

## Doing something

| Concept | Use | Not |
| --- | --- | --- |
| The default commit of a decision area (Save, Export…, Create) | `Button::primary()` — one per area, the Enter action | a second primary "because it is important" |
| An ordinary command | `Button` (Secondary) | `primary` |
| A frequent command in a toolbar | `Button::ghost()`, or `Button::icon_only(..).tooltip(..)` | an icon with no tooltip |
| A command that needs more input first | label ends in `…` ("Export…", "Choose…") | `…` on an immediate command |
| A command that destroys or discards | `ButtonVariant::Danger` / `MenuItem::danger()`, named with verb and object ("Delete track") | "OK" under "Are you sure?" |
| Handing work to the agent | `ButtonVariant::Agent` with `IconName::Sparkles` | agent colour on anything else |
| Secondary commands on an object | a visible menu button or a context menu: `Popover` of `MenuItem` | hover-only icon strips (the transcript's message actions are the one exception) |
| Copying something | `CopyButton` | a `Button` that copies silently |
| An agent's proposed change | `agent::ApprovalCard`, and Keep / Undo on the changed thing itself | a toast with "Undo" only |

## Showing a state

| Concept | Use | Not |
| --- | --- | --- |
| The status of one thing (rendered, clipping, draft) | `Badge::tone(..).dot()` with a word | colour alone (`StatusDot` without a word) |
| What the last action did ("Saved", "Downloaded 2 models") | `StatusBar` along the window's bottom | a modal; a status bar that asks a question |
| Something the agent made or wrote | `Badge::tone(Tone::Agent)`, `AgentLabel`, `ListItem::description_agent` | agent colour as decoration |
| A category shared across views (cluster, tag) | `theme.categorical(index)`, one index per thing for good, plus a label | a fresh hue per view |
| Nothing here yet | `EmptyState` with one action | an empty panel |
| A key that does it | `Kbd` | a shortcut written in plain text |

## Not in Atelier yet

These concepts have no component. Use the stand-in and do not hand-draw one from copied sizes
and colours; when the stand-in is not enough, add the component to Atelier.

| Concept | Stand-in for now |
| --- | --- |
| A confirmation dialog | an `ApprovalCard` in place (verb + object, `.destructive()` when it deletes); a destructive command in a menu with a clear name |
| A drop-down select | the menu button above (`Button` + `ChevronDown` + `Popover` of `MenuItem::checked`) |
| A radio group | a `SegmentedControl` (few options) or `OptionCard`s (options that need words) |
| A multi-line text area | the application's own editor inside a `TextField` frame |
| A number field with steppers | a `TextField` holding the number, the unit trailing |
| Reordering a list by dragging | `ListItem`s with the application's own drag handling and a menu with "Move up / Move down" |
| A colour picker, a date or time picker | the application's own, using tokens; ask for it to be added |
