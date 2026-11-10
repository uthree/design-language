//! A mock creator tool for judging the design candidates side by side.
//!
//! Nothing here works: the timeline, the mixer and the assistant show fixed sample data. Switch
//! the candidate and the appearance from the title bar.
//!
//! ```text
//! cargo run --example showcase -- [--direction studio|glass|mono] [--appearance dark|light]
//!                                 [--page arrange|components] [--agent-closed]
//!                                 [--dialog delete|approval]
//! ```

mod common;

use std::time::Duration;

use atelier::agent::{
    AgentPanel, AgentStatus, AgentToggle, ApprovalCard, ApprovalDialog, AskBar, AssistantMessage,
    ChatNotice, Composer, ContextChip, Decision, PermissionMode, PermissionPicker, ReasoningBlock,
    SuggestionChip, ThinkingIndicator, ToolCall, ToolGroup, ToolState, UserMessage,
};
use atelier::{
    ActiveTheme, Appearance, Assets, Avatar, BackToPlayhead, Badge, Button, ButtonVariant, Card,
    Checkbox, Column, ControlSize, Dialog, Direction, Disclosure, Divider, Edge, EmptyState,
    Follow, Icon, IconName, InlineEdit, Kbd, Knob, Label, LevelMeter, ListItem, MenuItem, Popover,
    ProgressBar, ProgressRing, ProviderLogo, RowMark, SectionHeader, Segment, SegmentedControl,
    Sidebar, Skeleton, Slider, Spinner, SpotBook, Stat, StepList, StepState, Switch, Table,
    TableCell, TableRow, Tabs, TaskProgress, TaskState, TextColor, TextField, TextInput, TextSize,
    Theme, TitleBar, Tone, WindowFrame, menu_separator, skeleton_paragraph,
};
use gpui::{
    Animation, AnimationExt, AnyElement, App, Application, Bounds, Context, Entity, Hsla,
    IntoElement, ParentElement, Render, SharedString, Styled, TitlebarOptions, Window,
    WindowBounds, WindowOptions, div, prelude::*, px, relative, rgb, size,
};

use common::argument;
use common::readout::{gain_db, pan, percent, volume_db};

/// The sample dialog open over the window.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OpenDialog {
    Delete,
    Approval,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Arrange,
    Components,
}

/// How the window's width is shared out. The track sidebar goes first when the window narrows,
/// then the assistant panel slims down; the title bar and the inspector drop their least-used
/// parts.
#[derive(Clone, Copy)]
struct Fit {
    window: f32,
    sidebar: bool,
    agent_width: f32,
    /// Width left for the arrangement and the inspector.
    content: f32,
}

const SIDEBAR_WIDTH: f32 = 232.;
/// The arrangement keeps at least this much before the sidebar is hidden.
const MIN_CONTENT_WIDTH: f32 = 560.;

impl Fit {
    fn new(window: f32, agent_open: bool) -> Self {
        let agent_width = match (agent_open, window >= 1200.) {
            (false, _) => 0.,
            (true, true) => 380.,
            (true, false) => 320.,
        };
        let sidebar = window - agent_width - SIDEBAR_WIDTH >= MIN_CONTENT_WIDTH;
        let content = window - agent_width - if sidebar { SIDEBAR_WIDTH } else { 0. };
        Self {
            window,
            sidebar,
            agent_width,
            content,
        }
    }
}

struct Showcase {
    direction: Direction,
    appearance: Appearance,
    page: Page,
    agent_open: bool,
    reasoning_open: bool,
    tool_open: bool,
    selected_track: usize,
    mute: [bool; 5],
    solo: [bool; 5],
    switches: [bool; 2],
    checks: [bool; 2],
    tool: usize,
    tab: usize,
    disclosures: [bool; 3],
    /// The demo progress value the Advance button steps.
    progress: f32,
    /// How many steps of the demo plan are done.
    plan_step: usize,
    /// The sample dialog open over the window, if any.
    dialog: Option<OpenDialog>,
    /// The sample approval's automatic reviewer is still deciding.
    reviewing: bool,
    /// The project name field's input.
    project: Entity<TextInput>,
    /// The track name edited in place, and whether its field is open.
    track_name: SharedString,
    renaming: bool,
    rename: Entity<TextInput>,
    spots: SpotBook,
    /// The event list's rows (position, event, value), its selection and the cell being edited.
    events: Vec<[SharedString; 3]>,
    event_row: Option<usize>,
    event_edit: Option<TableCell>,
    cell_input: Entity<TextInput>,
    event_scroll: gpui::UniformListScrollHandle,
    /// The follow demo: whether its strip follows, where it shows, the playhead, and the clock.
    follow: Follow,
    follow_offset: f64,
    follow_at: f64,
    follow_run: Option<gpui::Task<()>>,
}

/// Sample tracks. Their colours are document data, not interface tokens.
struct Track {
    name: &'static str,
    icon: IconName,
    color: u32,
    clips: &'static [(f32, f32, &'static str, bool)],
    level: f32,
}

const TRACKS: [Track; 5] = [
    Track {
        name: "Lead Vocal",
        icon: IconName::Mic,
        color: 0xf06292,
        clips: &[
            (0.12, 0.30, "Verse 1", false),
            (0.46, 0.30, "Chorus", false),
        ],
        level: 0.78,
    },
    Track {
        name: "Grand Piano",
        icon: IconName::Music,
        color: 0x4fc3f7,
        clips: &[
            (0.0, 0.44, "Intro – Verse", false),
            (0.46, 0.40, "Chorus voicing", true),
        ],
        level: 0.64,
    },
    Track {
        name: "Strings",
        icon: IconName::Waveform,
        color: 0xffb74d,
        clips: &[(0.46, 0.50, "Pad swell", true)],
        level: 0.42,
    },
    Track {
        name: "Drums",
        icon: IconName::Metronome,
        color: 0x81c784,
        clips: &[
            (0.06, 0.38, "Groove A", false),
            (0.46, 0.50, "Groove B", false),
        ],
        level: 0.92,
    },
    Track {
        name: "Bass",
        icon: IconName::Waveform,
        color: 0xba68c8,
        clips: &[(0.06, 0.90, "Bassline", false)],
        level: 0.58,
    },
];

fn track_color(track: &Track) -> Hsla {
    rgb(track.color).into()
}

impl Showcase {
    fn apply_theme(&self, window: &mut Window, cx: &mut App) {
        let theme = Theme::new(self.direction, self.appearance);
        atelier::set_window_background(window, &theme);
        theme.install(cx);
    }

    fn title_bar(&self, fit: Fit, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let direction_index = Direction::ALL
            .iter()
            .position(|d| *d == self.direction)
            .unwrap_or(0);
        TitleBar::new()
            .leading(Button::icon_only("sidebar", IconName::SidebarLeft).tooltip("Hide sidebar"))
            .title("Midnight Drive")
            .child(div().w(theme.spacing.lg))
            .child(transport(fit, cx))
            .trailing(
                SegmentedControl::new(
                    "direction",
                    Direction::ALL.map(|direction| Segment::label(direction.label())),
                    direction_index,
                )
                .size(ControlSize::Small)
                .on_select(cx.listener(|this, index: &usize, window, cx| {
                    this.direction = Direction::ALL[*index];
                    this.apply_theme(window, cx);
                })),
            )
            .trailing(
                SegmentedControl::new(
                    "appearance",
                    [Segment::label("Light"), Segment::label("Dark")],
                    if self.appearance.is_dark() { 1 } else { 0 },
                )
                .size(ControlSize::Small)
                .on_select(cx.listener(|this, index: &usize, window, cx| {
                    this.appearance = Appearance::ALL[*index];
                    this.apply_theme(window, cx);
                })),
            )
            .trailing(Divider::vertical())
            .trailing(
                AgentToggle::new(
                    "agent-toggle",
                    self.agent_open,
                    AgentStatus::AwaitingApproval,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.agent_open = !this.agent_open;
                    cx.notify();
                })),
            )
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        Sidebar::new(Edge::Left)
            .w(px(SIDEBAR_WIDTH))
            .p(theme.spacing.sm)
            .gap(theme.spacing.xxs)
            .child(
                div()
                    .px(theme.spacing.sm)
                    .pt(theme.spacing.sm)
                    .pb(theme.spacing.xs)
                    .child(SectionHeader::caption("Tracks")),
            )
            .children(TRACKS.iter().enumerate().map(|(index, track)| {
                ListItem::new(("track", index), track.name)
                    .icon(track.icon)
                    .icon_color(track_color(track))
                    .selected(index == self.selected_track)
                    .trailing(
                        Label::new(format!("{}", index + 1))
                            .size(TextSize::XSmall)
                            .color(TextColor::Faint),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected_track = index;
                        cx.notify();
                    }))
            }))
            .child(
                div()
                    .px(theme.spacing.sm)
                    .pt(theme.spacing.lg)
                    .pb(theme.spacing.xs)
                    .child(SectionHeader::caption("Library")),
            )
            .child(ListItem::new("lib-instruments", "Instruments").icon(IconName::Music))
            .child(ListItem::new("lib-loops", "Loops").icon(IconName::Loop))
            .child(ListItem::new("lib-samples", "Samples").icon(IconName::Folder))
            .child(
                ListItem::new("lib-voices", "Voices")
                    .icon(IconName::Mic)
                    .trailing(Badge::new("New").tone(Tone::Accent)),
            )
            .child(div().flex_1())
            .child(AskBar::new("ask", "Ask about this song…").shortcut(
                if cfg!(target_os = "macos") {
                    "⌘K"
                } else {
                    "Ctrl+K"
                },
            ))
    }

    fn arrange(&self, fit: Fit, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let colors = &theme.colors;
        let header_width = px(196.);
        // Number every other bar line when the bars get too narrow for their numbers.
        let bar_width = (fit.content - 196.) / 8.;
        let label_every = if bar_width < 40. { 2 } else { 1 };
        let lane = theme.density.row * 2.2;
        let ruler = div()
            .flex()
            .h(theme.density.control_md)
            .border_b_1()
            .border_color(colors.border)
            .bg(colors.chrome)
            .child(
                div()
                    .w(header_width)
                    .flex_none()
                    .border_r_1()
                    .border_color(colors.border)
                    .flex()
                    .items_center()
                    .px(theme.spacing.md)
                    .child(Label::caption("5 tracks")),
            )
            .child(
                div()
                    .flex_1()
                    .relative()
                    .overflow_hidden()
                    .children((0..8).map(|bar| {
                        div()
                            .absolute()
                            .left(relative(bar as f32 / 8.))
                            .w(relative(1. / 8.))
                            .top_0()
                            .bottom_0()
                            .pl(theme.spacing.xs)
                            .border_l_1()
                            .border_color(colors.border)
                            .flex()
                            .items_center()
                            .overflow_hidden()
                            .when(bar % label_every == 0, |cell| {
                                cell.child(
                                    Label::new(format!("{}", bar * 4 + 1))
                                        .size(TextSize::XSmall)
                                        .mono()
                                        .color(TextColor::Faint),
                                )
                            })
                    })),
            );

        let rows = TRACKS.iter().enumerate().map(|(index, track)| {
            let color = track_color(track);
            let selected = index == self.selected_track;
            let mute = self.mute[index];
            let solo = self.solo[index];
            div()
                .flex()
                .flex_none()
                .h(lane)
                .border_b_1()
                .border_color(colors.separator)
                .child(
                    div()
                        .w(header_width)
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(theme.spacing.sm)
                        .px(theme.spacing.sm)
                        .border_r_1()
                        .border_color(colors.border)
                        .when(selected, |header| header.bg(colors.selected))
                        .when(!selected, |header| header.bg(colors.surface))
                        .child(
                            div()
                                .w(px(3.))
                                .h(lane - theme.spacing.lg)
                                .rounded(px(2.))
                                .bg(color),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w_0()
                                .gap(theme.spacing.xs)
                                .child(
                                    Label::new(track.name)
                                        .size(TextSize::Small)
                                        .weight(theme.typography.label_weight)
                                        .truncate(),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .gap(theme.spacing.xxs)
                                        .child(
                                            Button::new(("mute", index), "M")
                                                .size(ControlSize::Small)
                                                .selected(mute)
                                                .tint(colors.info)
                                                .tooltip("Mute")
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.mute[index] = !this.mute[index];
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            Button::new(("solo", index), "S")
                                                .size(ControlSize::Small)
                                                .selected(solo)
                                                .tint(colors.warning)
                                                .tooltip("Solo")
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.solo[index] = !this.solo[index];
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            Button::new(("arm", index), "R")
                                                .size(ControlSize::Small)
                                                .tooltip("Arm for recording"),
                                        ),
                                ),
                        )
                        .child(
                            div().h(lane - theme.spacing.lg).child(
                                LevelMeter::new(track.level)
                                    .peak(track.level + 0.05)
                                    .thickness(px(4.)),
                            ),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .relative()
                        .bg(colors.background)
                        .children((1..8).map(|bar| {
                            div()
                                .absolute()
                                .left(relative(bar as f32 / 8.))
                                .top_0()
                                .bottom_0()
                                .w(px(1.))
                                .bg(colors.separator)
                        }))
                        .children(track.clips.iter().map(|&(start, length, name, by_agent)| {
                            clip(theme, color, start, length, name, by_agent)
                        })),
                )
        });

        let playhead = div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(header_width)
            .right_0()
            .child(
                div()
                    .absolute()
                    .left(relative(0.36))
                    .top_0()
                    .bottom_0()
                    .w(px(1.5))
                    .bg(colors.playhead),
            );

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(ruler)
            // A short window scrolls the tracks; the inspector stays.
            .child(
                div()
                    .id("tracks")
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .bg(colors.background)
                    .children(rows)
                    .child(playhead),
            )
            .child(self.inspector(fit, cx))
    }

    fn inspector(&self, fit: Fit, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        // The suggestions need room for their text and buttons; below that they wait in the
        // assistant panel.
        let suggestions = fit.content >= 860.;
        // Whole knobs only: as many as fit beside the track controls and the meters.
        let knobs = if suggestions {
            5
        } else {
            ((fit.content - 340.) / 60.).clamp(1., 5.) as usize
        };
        let theme = cx.theme();
        let colors = &theme.colors;
        let track = &TRACKS[self.selected_track];
        let color = track_color(track);
        div()
            .flex()
            .flex_none()
            .gap(theme.spacing.xl)
            .h(px(176.))
            .p(theme.spacing.lg)
            .bg(colors.surface)
            .border_t_1()
            .border_color(colors.border)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .w(px(220.))
                    .gap(theme.spacing.md)
                    .child(
                        SectionHeader::new(track.name)
                            .description("Instrument · Grand Piano (Physical)"),
                    )
                    // No handlers: each control keeps its own value, one per track.
                    .child(
                        Slider::new(0.72)
                            .id(("volume", self.selected_track))
                            .label("Volume")
                            .default_value(0.75)
                            .readout_with(volume_db),
                    )
                    .child(
                        Slider::new(0.42)
                            .id(("pan", self.selected_track))
                            .label("Pan")
                            .bipolar()
                            .readout_with(pan),
                    )
                    .child(
                        Switch::new(("monitor", self.selected_track), true)
                            .label("Input monitoring"),
                    ),
            )
            .child(Divider::vertical().full())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .when(!suggestions, |eq| eq.flex_1())
                    .min_w_0()
                    .overflow_hidden()
                    .gap(theme.spacing.md)
                    .child(SectionHeader::caption("Channel EQ"))
                    .child(
                        div().flex().gap(theme.spacing.lg).children(
                            [
                                Knob::new(0.62)
                                    .id(("eq-low", self.selected_track))
                                    .bipolar()
                                    .label("Low")
                                    .readout_with(gain_db),
                                Knob::new(0.5)
                                    .id(("eq-mid", self.selected_track))
                                    .bipolar()
                                    .label("Mid")
                                    .readout_with(gain_db),
                                Knob::new(0.35)
                                    .id(("eq-high", self.selected_track))
                                    .bipolar()
                                    .label("High")
                                    .readout_with(gain_db),
                                Knob::new(0.8)
                                    .id(("drive", self.selected_track))
                                    .color(color)
                                    .label("Drive")
                                    .readout_with(percent),
                                Knob::new(0.25)
                                    .id(("mix", self.selected_track))
                                    .label("Mix")
                                    .readout_with(percent),
                            ]
                            .into_iter()
                            .take(knobs),
                        ),
                    ),
            )
            .when(suggestions, |inspector| {
                inspector.child(Divider::vertical().full())
            })
            .when(suggestions, |inspector| {
                inspector.child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .gap(theme.spacing.md)
                        .child(
                            SectionHeader::caption("Assistant suggestions").action(
                                Badge::new("2 drafts")
                                    .tone(Tone::Agent)
                                    .icon(IconName::Sparkles),
                            ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(theme.spacing.sm)
                                .child(suggestion_row(
                                    theme,
                                    "Chorus voicing",
                                    "Opened the left hand to tenths in bars 17–24",
                                ))
                                .child(suggestion_row(
                                    theme,
                                    "Pad swell",
                                    "Added a strings pad under the chorus",
                                )),
                        ),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_none()
                    .gap(theme.spacing.xs)
                    .h_full()
                    .child(LevelMeter::new(0.74).peak(0.8))
                    .child(LevelMeter::new(0.69).peak(0.77)),
            )
    }

    fn agent_panel(&self, fit: Fit, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let border = cx.theme().colors.border;
        let piano = track_color(&TRACKS[1]);
        let panel = AgentPanel::new("agent", "Reharmonise the chorus")
            .logo(ProviderLogo::Anthropic)
            .status(AgentStatus::AwaitingApproval)
            .status_detail("3 tools · 14 s")
            .action(Button::icon_only("history", IconName::History).tooltip("Conversations"))
            .action(Button::icon_only("new-chat", IconName::Plus).tooltip("New conversation"))
            .on_close(cx.listener(|this, _, _, cx| {
                this.agent_open = false;
                cx.notify();
            }))
            .entry(
                UserMessage::new("The chorus feels flat. Can you open up the piano voicing and add something to lift it?")
                    .id("message-1")
                    .attachment(ContextChip::new("ctx-piano", IconName::Music, "Grand Piano · bars 17–24").color(piano)),
            )
            .entry(
                ReasoningBlock::new("reasoning", "Thought for 6 s", "The chorus sits on close-position triads in the right hand and root-only bass, so the texture never widens. Spreading to tenths and adding a sustained pad an octave above should lift it without changing the melody.")
                    .open(self.reasoning_open)
                    .on_toggle(cx.listener(|this, _, _, cx| {
                        this.reasoning_open = !this.reasoning_open;
                        cx.notify();
                    })),
            )
            .entry(
                StepList::new("panel-plan")
                    .step("Read the chorus", StepState::Done)
                    .step("Revoice the piano", StepState::Done)
                    .step_with_detail(
                        "Draft a strings pad",
                        "Waiting for your approval",
                        StepState::Running,
                    )
                    .step("Balance the mix", StepState::Pending),
            )
            .entry(
                ToolGroup::new("Used 3 tools")
                    .call(ToolCall::new("tool-read", "read_notes", "Read 96 notes in Grand Piano", ToolState::Succeeded).icon(IconName::Eye))
                    .call(
                        ToolCall::new("tool-edit", "edit_notes", "Revoiced bars 17–24", ToolState::Succeeded)
                            .icon(IconName::Music)
                            .detail("{ \"track\": \"Grand Piano\", \"bars\": [17, 24], \"voicing\": \"open-tenths\" }\n→ 48 notes moved, 0 added")
                            .open(self.tool_open)
                            .on_toggle(cx.listener(|this, _, _, cx| {
                                this.tool_open = !this.tool_open;
                                cx.notify();
                            })),
                    )
                    .call(ToolCall::new("tool-create", "create_clip", "Drafting a strings pad", ToolState::AwaitingApproval).icon(IconName::Waveform)),
            )
            .entry(AssistantMessage::new(
                "I opened the chorus voicing and drafted a pad to sit above it:\n\n- Left hand now plays tenths, so the bass line breathes.\n- Right hand keeps the melody on top; nothing in the vocal range moved.\n- A strings pad holds the chord tones an octave up.\n\nThe revoiced clip is marked as an AI draft — undo restores the original.",
            )
            .logo(ProviderLogo::Anthropic)
            .meta("Claude Opus 5.5 · 14 s")
            .action(Button::icon_only("copy", IconName::Copy).size(ControlSize::Small))
            .action(Button::icon_only("retry", IconName::Refresh).size(ControlSize::Small).tooltip("Regenerate")))
            .entry(
                ApprovalCard::new("approval", "Create \u{201c}Pad swell\u{201d} on Strings?")
                    .body("Adds one clip in bars 17–32. Nothing existing is replaced.")
                    .allow_label("Create")
                    .on_always(|_, _, _| {}),
            )
            .entry(ChatNotice::status("Waiting for your decision"))
            .composer(
                Composer::new("composer")
                    .placeholder("Ask about the song, or describe a change…")
                    .context(ContextChip::new("ctx-sel", IconName::Music, "Selection: 2 clips").color(piano).on_remove(|_, _, _| {}))
                    .model("Claude Opus 5.5")
                    .model_logo(ProviderLogo::Anthropic)
                    .focused(true),
            )
;
        div()
            .w(px(fit.agent_width))
            .flex_none()
            .border_l_1()
            .border_color(border)
            .child(panel)
    }

    fn progress_samples(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let column = || div().flex().flex_col().gap(theme.spacing.md);

        // Indicators that run on their own; the looping ones are fed a value by an animation,
        // standing in for real work.
        let indicators = column()
            .w(px(300.))
            .child(Label::caption("Indicators"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme.spacing.lg)
                    .child(Spinner::new("spin-sm").size(px(12.)))
                    .child(
                        Spinner::new("spin-md")
                            .size(px(16.))
                            .color(theme.colors.accent),
                    )
                    .child(
                        Spinner::new("spin-lg")
                            .size(px(24.))
                            .color(theme.colors.text_muted),
                    )
                    .child(
                        ProgressRing::new(0.)
                            .size(px(36.))
                            .show_value()
                            .with_animation(
                                "ring-loop",
                                Animation::new(Duration::from_secs(5)).repeat(),
                                |ring, delta| ring.value(delta),
                            ),
                    )
                    .child(ProgressRing::new(0.72).size(px(20.)).tone(Tone::Agent)),
            )
            .child(ThinkingIndicator::new("Thinking").logo(ProviderLogo::Anthropic))
            .child(
                Label::new("Indeterminate")
                    .size(TextSize::Small)
                    .color(TextColor::Muted),
            )
            .child(ProgressBar::indeterminate("bar-indeterminate"))
            .child(
                Label::new("Determinate")
                    .size(TextSize::Small)
                    .color(TextColor::Muted),
            )
            .child(ProgressBar::new(0.).with_animation(
                "bar-loop",
                Animation::new(Duration::from_secs(6)).repeat(),
                |bar, delta| bar.value(delta),
            ));

        // A value that moves only when asked, to show the fill gliding between values.
        let stepped = column()
            .w(px(300.))
            .child(Label::caption("Glide between values"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme.spacing.md)
                    .child(
                        ProgressRing::new(self.progress)
                            .id("ring-stepped")
                            .size(px(40.))
                            .show_value(),
                    )
                    .child(
                        div().flex_1().child(
                            ProgressBar::new(self.progress)
                                .id("bar-stepped")
                                .height(px(6.)),
                        ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(theme.spacing.sm)
                    .child(
                        Button::new("advance", "Advance")
                            .size(ControlSize::Small)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.progress = if this.progress >= 0.99 {
                                    0.
                                } else {
                                    (this.progress + 0.2).min(1.)
                                };
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("plan-next", "Next plan step")
                            .size(ControlSize::Small)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.plan_step = (this.plan_step + 1) % 6;
                                cx.notify();
                            })),
                    ),
            )
            .child(Label::caption("Loading a list"))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.spacing.md)
                    .children((0..2usize).map(|row| {
                        div()
                            .flex()
                            .items_center()
                            .gap(theme.spacing.md)
                            .child(
                                Skeleton::new(("sk-avatar", row))
                                    .size(px(28.))
                                    .rounded(px(14.)),
                            )
                            .child(div().flex_1().child(skeleton_paragraph(
                                format!("sk-text-{row}"),
                                2,
                                cx,
                            )))
                    })),
            );

        let plan_state = |index: usize| match index.cmp(&self.plan_step) {
            std::cmp::Ordering::Less => StepState::Done,
            std::cmp::Ordering::Equal => StepState::Running,
            std::cmp::Ordering::Greater => StepState::Pending,
        };
        let plan = column()
            .w(px(260.))
            .child(Label::caption("Agent plan"))
            .child(
                StepList::new("plan")
                    .step("Read the chorus", plan_state(0))
                    .step_with_detail(
                        "Revoice the piano",
                        "Bars 17–24 · open tenths",
                        plan_state(1),
                    )
                    .step("Draft a strings pad", plan_state(2))
                    .step("Balance the mix", plan_state(3))
                    .step("Render a preview", plan_state(4)),
            );

        let tasks = column()
            .w(px(420.))
            .child(Label::caption("Long-running tasks"))
            .child(
                TaskProgress::new("task-render", "Export stems")
                    .detail("Bouncing stem 3 of 8 · Drums")
                    .progress(self.progress)
                    .remaining("About 20 s left")
                    .on_cancel(|_, _, _| {}),
            )
            .child(
                TaskProgress::new("task-agent", "Assistant is revoicing the chorus")
                    .icon(IconName::Sparkles)
                    .detail("Editing Grand Piano")
                    .by_agent()
                    .on_cancel(|_, _, _| {}),
            )
            .child(
                TaskProgress::new("task-queued", "Download voice model")
                    .icon(IconName::Mic)
                    .state(TaskState::Queued)
                    .detail("Waits for the export"),
            )
            .child(
                TaskProgress::new("task-done", "Render preview")
                    .state(TaskState::Succeeded)
                    .detail("midnight-drive-preview.wav · 2:41")
                    .action(
                        Button::new("reveal", "Show in folder")
                            .size(ControlSize::Small)
                            .ghost(),
                    ),
            )
            .child(
                TaskProgress::new("task-failed", "Analyse reference track")
                    .state(TaskState::Failed)
                    .detail("Couldn\u{2019}t decode the file. Convert it to WAV and try again.")
                    .action(Button::new("retry-task", "Retry").size(ControlSize::Small)),
            );

        div()
            .flex()
            .flex_wrap()
            .items_start()
            .gap(theme.spacing.xl)
            .child(indicators)
            .child(stepped)
            .child(plan)
            .child(tasks)
            .into_any_element()
    }

    fn components(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let progress = self.progress_samples(cx);
        let event_list = self.event_list(window, cx);
        let follow = self.follow_demo(cx);
        // A loudness panel's figures: one not measured yet, one over its ceiling.
        // The meters take the height of the figures beside them, which nothing sets.
        let meters = div()
            .flex()
            .gap(px(3.))
            .child(LevelMeter::db(-4.).peak_db(-1.5))
            .child(LevelMeter::db(-7.).peak_db(0.8).clipped(true));
        let figures = div()
            .flex()
            .flex_wrap()
            .items_end()
            .gap(px(28.))
            .child(
                div().flex().gap(px(12.)).child(meters).child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(12.))
                        .child(
                            Stat::new("Peak L", "−1.5")
                                .unit("dB")
                                .size(TextSize::Medium),
                        )
                        .child(
                            Stat::new("Peak R", "+0.8")
                                .unit("dB · clipped")
                                .size(TextSize::Medium)
                                .color(TextColor::Danger),
                        ),
                ),
            )
            .child(
                Stat::new("Integrated", "−14.2")
                    .unit("LUFS")
                    .min_width(px(110.)),
            )
            .child(
                Stat::new("Short-term", "−11.8")
                    .unit("LUFS")
                    .min_width(px(110.)),
            )
            .child(
                Stat::new("True peak", "+0.4")
                    .unit("dBTP")
                    .color(TextColor::Danger)
                    .min_width(px(100.)),
            )
            .child(Stat::empty("Range").unit("LU"))
            .child(
                Stat::new("Tempo", "120.00")
                    .unit("BPM")
                    .size(TextSize::Display),
            )
            .child(
                Stat::new("Position", "013.2.1")
                    .inline()
                    .size(TextSize::Small),
            );
        let theme = cx.theme();
        let section = |title: &'static str, body: AnyElement| {
            div()
                .flex()
                .flex_col()
                .gap(theme.spacing.md)
                .child(SectionHeader::new(title))
                .child(body)
        };
        let row = || {
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(theme.spacing.sm)
        };

        let buttons = row()
            .child(Button::new("b-primary", "Export…").primary())
            .child(Button::new("b-secondary", "Bounce in place"))
            .child(
                Button::new("b-ghost", "Duplicate")
                    .ghost()
                    .icon(IconName::Copy),
            )
            .child(
                Button::new("b-agent", "Ask assistant")
                    .variant(ButtonVariant::Agent)
                    .icon(IconName::Sparkles),
            )
            .child(Button::new("b-danger", "Delete track").variant(ButtonVariant::Danger))
            .child(Button::new("b-disabled", "Freeze").disabled(true))
            .child(
                Button::new("b-menu", "Grid: 1/16")
                    .icon_after(IconName::ChevronDown)
                    .size(ControlSize::Small),
            )
            .child(
                Button::new("b-large", "New project")
                    .primary()
                    .size(ControlSize::Large),
            )
            .child(Button::icon_only("b-icon", IconName::Settings).tooltip("Project settings…"));

        // Every control here is live, so its transition can be watched.
        let controls = row()
            .child(
                Switch::new("s-on", self.switches[0])
                    .label("Snap to grid")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.switches[0] = !this.switches[0];
                        cx.notify();
                    })),
            )
            .child(
                Switch::new("s-off", self.switches[1])
                    .label("Follow playhead")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.switches[1] = !this.switches[1];
                        cx.notify();
                    })),
            )
            .child(
                Checkbox::new("c-on", self.checks[0])
                    .label("Count-in")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.checks[0] = !this.checks[0];
                        cx.notify();
                    })),
            )
            .child(
                Checkbox::new("c-off", self.checks[1])
                    .label("Pre-roll")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.checks[1] = !this.checks[1];
                        cx.notify();
                    })),
            )
            .child(
                SegmentedControl::new(
                    "tools",
                    [
                        Segment::icon(IconName::Play),
                        Segment::label("Pencil"),
                        Segment::label("Scissors"),
                        Segment::label("Glue"),
                    ],
                    self.tool,
                )
                .on_select(cx.listener(|this, index: &usize, _, cx| {
                    this.tool = *index;
                    cx.notify();
                })),
            )
            .child(
                Tabs::new("tabs", ["Score", "Piano roll", "Audio"], self.tab).on_select(
                    cx.listener(|this, index: &usize, _, cx| {
                        this.tab = *index;
                        cx.notify();
                    }),
                ),
            );

        let fields = div()
            .flex()
            .flex_wrap()
            .gap(theme.spacing.md)
            .child(
                div().w(px(240.)).child(
                    TextField::new()
                        .icon(IconName::Search)
                        .placeholder("Search sounds…"),
                ),
            )
            .child(
                div().w(px(200.)).child(
                    TextField::new()
                        .placeholder("Project name")
                        .empty(self.project.read(cx).is_empty())
                        .focused(self.project.read(cx).is_focused(window))
                        .child(self.project.clone()),
                ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme.spacing.sm)
                    .child(
                        Label::new("Track")
                            .size(TextSize::Small)
                            .color(TextColor::Muted),
                    )
                    .child(
                        div()
                            .id("track-name")
                            .relative()
                            .child(Label::new(self.track_name.clone()))
                            .child(self.spots.spot("track-name"))
                            .on_click(cx.listener(|this, event: &gpui::ClickEvent, _, cx| {
                                if event.click_count() >= 2 {
                                    let name = this.track_name.to_string();
                                    this.rename.update(cx, |input, cx| input.set_text(name, cx));
                                    this.renaming = true;
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        Label::new("double-click to rename in place")
                            .size(TextSize::Small)
                            .color(TextColor::Muted),
                    )
                    .children(
                        self.renaming
                            .then(|| self.spots.get("track-name", window))
                            .flatten()
                            .map(|over| {
                                InlineEdit::new("rename-track", &self.rename, over)
                                    .original(self.track_name.clone())
                                    .on_commit(cx.listener(|this, text: &SharedString, _, cx| {
                                        this.track_name = text.trim().to_owned().into();
                                        this.renaming = false;
                                        cx.notify();
                                    }))
                                    .on_close(cx.listener(|this, _, _, cx| {
                                        this.renaming = false;
                                        cx.notify();
                                    }))
                            }),
                    ),
            )
            .child(
                div()
                    .w(px(240.))
                    .flex()
                    .flex_col()
                    .gap(theme.spacing.md)
                    .child(Slider::new(0.6).label("Tempo").readout("120.0"))
                    .child(ProgressBar::new(0.42)),
            );

        let marks = row()
            .child(Badge::new("Draft"))
            .child(Badge::new("Rendered").tone(Tone::Success).dot())
            .child(Badge::new("Clipping").tone(Tone::Danger).dot())
            .child(
                Badge::new("AI draft")
                    .tone(Tone::Agent)
                    .icon(IconName::Sparkles),
            )
            .child(Kbd::new(if cfg!(target_os = "macos") {
                "⌘B"
            } else {
                "Ctrl+B"
            }))
            .child(Avatar::person("UT"))
            .child(Avatar::agent())
            .child(Avatar::logo(ProviderLogo::Anthropic))
            .child(Spinner::new("spinner"))
            .child(
                div()
                    .w(px(160.))
                    .child(LevelMeter::new(0.82).peak(0.88).horizontal()),
            );

        let surfaces = div()
            .flex()
            .flex_wrap()
            .gap(theme.spacing.lg)
            .items_start()
            .child(
                Popover::new()
                    .w(px(220.))
                    .child(
                        MenuItem::new("m-cut", "Cut")
                            .icon(IconName::Copy)
                            .shortcut("Ctrl+X"),
                    )
                    .child(MenuItem::new("m-dup", "Duplicate").shortcut("Ctrl+D"))
                    .child(MenuItem::new("m-snap", "Snap to grid").checked(true))
                    .child(menu_separator(cx))
                    .child(
                        MenuItem::new("m-ask", "Ask assistant about clip…")
                            .icon(IconName::Sparkles),
                    )
                    .child(menu_separator(cx))
                    .child(
                        MenuItem::new("m-del", "Delete")
                            .icon(IconName::Trash)
                            .danger(),
                    ),
            )
            .child(
                Card::new()
                    .w(px(260.))
                    .p(theme.spacing.lg)
                    .gap(theme.spacing.sm)
                    .children(
                        ["Automation", "Sends", "Plug-ins"]
                            .into_iter()
                            .enumerate()
                            .map(|(index, label)| {
                                Disclosure::new(("disc", index), label, self.disclosures[index])
                                    .on_toggle(cx.listener(move |this, _, _, cx| {
                                        this.disclosures[index] = !this.disclosures[index];
                                        cx.notify();
                                    }))
                            }),
                    ),
            )
            .child(
                Card::new().w(px(300.)).child(
                    EmptyState::new(IconName::Music, "No clips yet")
                        .description("Record, drop a file, or ask the assistant to draft one.")
                        .action(
                            Button::new("e-ask", "Draft a clip…")
                                .variant(ButtonVariant::Agent)
                                .icon(IconName::Sparkles),
                        ),
                ),
            );

        let agent = div()
            .flex()
            .flex_wrap()
            .gap(theme.spacing.lg)
            .items_start()
            .child(
                div()
                    .w(px(360.))
                    .flex()
                    .flex_col()
                    .gap(theme.spacing.md)
                    .child(
                        ThinkingIndicator::new("Reading the score").logo(ProviderLogo::Anthropic),
                    )
                    .child(
                        ChatNotice::error(
                            "Couldn't reach the model. Check your connection and try again.",
                        )
                        .action(Button::new("retry2", "Retry").size(ControlSize::Small)),
                    )
                    .child(
                        ApprovalCard::new("approval2", "Replace \u{201c}Verse 2\u{201d} melody?")
                            .body("The current melody is kept as an alternative take.")
                            .allow_label("Replace")
                            .destructive(),
                    ),
            )
            .child(
                div()
                    .w(px(360.))
                    .flex()
                    .flex_col()
                    .gap(theme.spacing.md)
                    .child(
                        Label::new("Permission mode: the menu opens upwards, as in a composer row")
                            .size(TextSize::Small)
                            .color(TextColor::Muted),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(theme.spacing.sm)
                            .child(PermissionPicker::new("permissions", PermissionMode::Ask))
                            .child(PermissionPicker::new("bypass", PermissionMode::Bypass)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(theme.spacing.sm)
                            .child(Button::new("open-approval", "Approval dialog…").on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.dialog = Some(OpenDialog::Approval);
                                    this.reviewing = false;
                                    cx.notify();
                                }),
                            ))
                            .child(Button::new("open-reviewing", "While reviewing…").on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.dialog = Some(OpenDialog::Approval);
                                    this.reviewing = true;
                                    cx.notify();
                                }),
                            ))
                            .child(
                                Button::new("open-delete", "Delete clip…")
                                    .variant(ButtonVariant::Danger)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.dialog = Some(OpenDialog::Delete);
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .w(px(360.))
                    .flex()
                    .flex_col()
                    .gap(theme.spacing.sm)
                    .child(
                        SuggestionChip::new("sg1", "Suggest a bridge that modulates up a tone")
                            .icon(IconName::Sparkles),
                    )
                    .child(
                        SuggestionChip::new("sg2", "Tighten the drum groove in the chorus")
                            .icon(IconName::Sparkles),
                    )
                    .child(
                        Composer::new("composer2")
                            .value("Make the bass follow the kick in bars 9–16")
                            .model("Claude Sonnet 5.5")
                            .model_logo(ProviderLogo::Anthropic)
                            .busy(true),
                    ),
            );

        div()
            .id("components")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p(theme.spacing.xl)
            .gap(theme.spacing.xl)
            .bg(theme.colors.surface)
            .child(section("Buttons", buttons.into_any_element()))
            .child(section("Controls", controls.into_any_element()))
            .child(section("Fields", fields.into_any_element()))
            .child(section("Event list", event_list))
            .child(section("Following the playhead", follow))
            .child(section("Figures", figures.into_any_element()))
            .child(section("Status and identity", marks.into_any_element()))
            .child(section("Progress and loading", progress))
            .child(section(
                "Menus, cards and empty states",
                surfaces.into_any_element(),
            ))
            .child(section("Agent pieces", agent.into_any_element()))
    }
}

impl Showcase {
    /// The sample dialog, drawn over everything while open.
    fn dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        Some(match self.dialog? {
            OpenDialog::Delete => Dialog::new("delete-dialog", "Delete \u{201c}Verse 2\u{201d}?")
                .icon(IconName::Trash, Tone::Danger)
                .body("The clip and its 48 notes are removed from Piano. Undo brings them back.")
                .on_dismiss(cx.listener(|this, _: &(), _, cx| {
                    this.dialog = None;
                    cx.notify();
                }))
                .action(Button::new("delete-cancel", "Cancel").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.dialog = None;
                        cx.notify();
                    },
                )))
                .action(
                    Button::new("delete-confirm", "Delete")
                        .variant(ButtonVariant::Danger)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.dialog = None;
                            cx.notify();
                        })),
                )
                .into_any_element(),
            OpenDialog::Approval => ApprovalDialog::new(
                "approval-dialog",
                "Render the mix to disk?",
            )
            .body("Writes mixdown.wav (about 60 MB) next to the project. Nothing is replaced.")
            .command("auris render --mix --out mixdown.wav")
            .allow_label("Render")
            .offer_always()
            .reviewing(self.reviewing)
            .on_decide(cx.listener(|this, _: &Decision, _, cx| {
                this.dialog = None;
                cx.notify();
            }))
            .into_any_element(),
        })
    }
}

/// The transport. A narrower window keeps the position and drops the metronome, the metre and the
/// tempo, in that order from the end.
fn transport(fit: Fit, cx: &App) -> impl IntoElement + use<> {
    let readout = fit.window >= 1100.;
    let metre = fit.window >= 1320.;
    let theme = cx.theme();
    let colors = &theme.colors;
    div()
        .flex()
        .items_center()
        .gap(theme.spacing.xxs)
        .child(Button::icon_only("rewind", IconName::SkipBack).tooltip("Go to start"))
        .child(
            Button::icon_only("play", IconName::Play)
                .tooltip("Play")
                .selected(true),
        )
        .child(Button::icon_only("record", IconName::Record).tooltip("Record"))
        .child(
            Button::icon_only("loop", IconName::Loop)
                .tooltip("Cycle")
                .selected(true)
                .tint(colors.warning),
        )
        .when(readout, |transport| {
            transport.child(Button::icon_only("click", IconName::Metronome).tooltip("Metronome"))
        })
        .when(readout, |transport| {
            transport.child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme.spacing.md)
                    .ml(theme.spacing.sm)
                    .h(theme.density.control_md)
                    .px(theme.spacing.md)
                    .rounded(theme.control_radius(theme.density.control_md))
                    .bg(colors.sunken)
                    .border_1()
                    .border_color(colors.border)
                    .child(
                        Label::new("013.2.1")
                            .mono()
                            .weight(theme.typography.label_weight),
                    )
                    .child(
                        Label::new("120 BPM")
                            .size(TextSize::Small)
                            .mono()
                            .color(TextColor::Muted),
                    )
                    .when(metre, |readout| {
                        readout.child(
                            Label::new("4/4 · C maj")
                                .size(TextSize::Small)
                                .mono()
                                .color(TextColor::Muted),
                        )
                    }),
            )
        })
}

fn clip(
    theme: &Theme,
    color: Hsla,
    start: f32,
    length: f32,
    name: &'static str,
    by_agent: bool,
) -> impl IntoElement + use<> {
    let colors = &theme.colors;
    let fill = color.opacity(if theme.appearance.is_dark() {
        0.32
    } else {
        0.24
    });
    div()
        .absolute()
        .left(relative(start))
        .w(relative(length))
        .top(px(4.))
        .bottom(px(4.))
        .rounded(theme.shape.radius_sm)
        .bg(fill)
        .border_1()
        .border_color(if by_agent {
            colors.agent
        } else {
            color.opacity(0.8)
        })
        .when(by_agent, |clip| clip.border_dashed())
        .overflow_hidden()
        .child(
            div()
                .flex()
                .items_center()
                .gap(theme.spacing.xs)
                .h(theme.density.control_sm)
                .px(theme.spacing.sm)
                .bg(color.opacity(0.55))
                .text_size(theme.typography.size_xs)
                .font_weight(theme.typography.label_weight)
                .text_color(theme.text_on(color))
                .when(by_agent, |header| {
                    header.child(
                        Icon::new(IconName::Sparkles)
                            .size(px(10.))
                            .color(theme.text_on(color)),
                    )
                })
                .child(name),
        )
        .child(
            // A stand-in for note or waveform content.
            div()
                .flex()
                .items_end()
                .gap(px(3.))
                .h(px(22.))
                .px(theme.spacing.sm)
                .mt(theme.spacing.xs)
                .children((0..28).map(move |i| {
                    let height = 4. + ((i * 37 + (start * 100.) as usize * 11) % 17) as f32;
                    div()
                        .w(px(3.))
                        .h(px(height))
                        .rounded(px(1.))
                        .bg(color.opacity(0.9))
                })),
        )
}

fn suggestion_row(
    theme: &Theme,
    title: &'static str,
    detail: &'static str,
) -> impl IntoElement + use<> {
    div()
        .flex()
        .items_center()
        .gap(theme.spacing.md)
        .p(theme.spacing.sm)
        .rounded(theme.shape.radius_md)
        .bg(theme.colors.agent_wash)
        .child(
            Icon::new(IconName::Sparkles)
                .size(px(14.))
                .color(theme.colors.agent),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(
                    Label::new(title)
                        .size(TextSize::Small)
                        .weight(theme.typography.label_weight),
                )
                .child(
                    Label::new(detail)
                        .size(TextSize::XSmall)
                        .color(TextColor::Muted)
                        .truncate(),
                ),
        )
        .child(
            Button::new(SharedString::from(format!("keep-{title}")), "Keep")
                .size(ControlSize::Small),
        )
        .child(
            Button::new(SharedString::from(format!("undo-{title}")), "Undo")
                .size(ControlSize::Small)
                .ghost(),
        )
}

impl Showcase {
    /// A MIDI event list: notes, a controller and a bend, each family marked down its edge, the
    /// second note sounding. A double-click edits a cell in place; Return walks down the column.
    fn event_list(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let family = |event: &str| match event {
            event if event.starts_with("CC") => 1,
            event if event.starts_with("Pitch") => 2,
            _ => 0,
        };
        let rows = self.events.iter().enumerate().map(|(index, row)| {
            let family = family(&row[1]);
            let row = TableRow::new(row.iter().cloned())
                .mark(RowMark::Category(family))
                .active(index == 1)
                .selected(self.event_row == Some(index));
            if family > 0 { row.tinted() } else { row }
        });
        let table = Table::new(
            "event-list",
            [
                Column::new("Pos").width(px(48.)).end().mono(),
                Column::new("Event"),
                Column::new("Value").width(px(56.)).end().mono(),
            ],
            rows,
        )
        .track_scroll(&self.event_scroll)
        .spots(&self.spots)
        .on_select(cx.listener(|this, row: &usize, _, cx| {
            this.event_row = Some(*row);
            cx.notify();
        }))
        .on_activate(cx.listener(|this, cell: &TableCell, _, cx| this.edit_event(*cell, cx)));
        let editor = self.event_edit.and_then(|cell| {
            let over = self
                .spots
                .get(&Table::cell_key("event-list", cell), window)?;
            let original = self.events[cell.row][cell.column].clone();
            Some(
                InlineEdit::new("edit-event", &self.cell_input, over)
                    .min_width(px(80.))
                    .original(original)
                    .on_commit(cx.listener(move |this, text: &SharedString, _, cx| {
                        this.events[cell.row][cell.column] = text.trim().to_owned().into();
                        this.event_edit = None;
                        cx.notify();
                    }))
                    .on_return(
                        cx.listener(move |this, text: &Option<SharedString>, _, cx| {
                            if let Some(text) = text {
                                this.events[cell.row][cell.column] = text.trim().to_owned().into();
                            }
                            // On to the same cell of the next row, as a list editor walks.
                            match cell.row + 1 < this.events.len() {
                                true => this.edit_event(
                                    TableCell {
                                        row: cell.row + 1,
                                        ..cell
                                    },
                                    cx,
                                ),
                                false => this.event_edit = None,
                            }
                            cx.notify();
                        }),
                    )
                    .on_close(cx.listener(|this, _, _, cx| {
                        this.event_edit = None;
                        cx.notify();
                    })),
            )
        });
        div()
            .w(px(360.))
            .h(px(200.))
            .child(table)
            .children(editor)
            .into_any_element()
    }

    fn edit_event(&mut self, cell: TableCell, cx: &mut Context<Self>) {
        let text = self.events[cell.row][cell.column].to_string();
        self.cell_input
            .update(cx, |input, cx| input.set_text(text, cx));
        self.event_scroll
            .scroll_to_item(cell.row, gpui::ScrollStrategy::Top);
        self.event_row = Some(cell.row);
        self.event_edit = Some(cell);
        cx.notify();
    }
}

/// The follow demo's length and how much of it its strip shows, in beats, and its tempo.
const FOLLOW_LENGTH: f64 = 64.;
const FOLLOW_SPAN: f64 = 16.;
const FOLLOW_BEATS_PER_SECOND: f64 = 4.;
const FOLLOW_TICK: Duration = Duration::from_millis(16);

impl Showcase {
    /// A strip of bars that pages along as it plays. Scroll it away and it stays where it was
    /// put, with a button back to the playhead in the corner the playhead went.
    fn follow_demo(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let width = 480.;
        let x = |beat: f64| px(((beat - self.follow_offset) / FOLLOW_SPAN * width) as f32);
        let playing = self.follow_run.is_some();
        let bars = (0..(FOLLOW_LENGTH / 4.) as usize).map(|bar| {
            let beat = bar as f64 * 4.;
            div()
                .absolute()
                .left(x(beat))
                .top_0()
                .bottom_0()
                .w(px(1.))
                .bg(colors.separator)
                .child(
                    div()
                        .absolute()
                        .left(px(4.))
                        .top(px(2.))
                        .whitespace_nowrap()
                        .child(
                            Label::new((bar + 1).to_string())
                                .size(TextSize::XSmall)
                                .color(TextColor::Faint)
                                .mono(),
                        ),
                )
        });
        let clips =
            [(2., 6.), (12., 10.), (26., 4.), (34., 12.), (50., 8.)].map(|(start, length)| {
                div()
                    .absolute()
                    .left(x(start))
                    .w(px((length / FOLLOW_SPAN * width) as f32))
                    .top(px(26.))
                    .h(px(28.))
                    .rounded(theme.shape.radius_sm)
                    .bg(colors.accent_wash)
                    .border_1()
                    .border_color(colors.accent)
            });
        let side = self
            .follow
            .side(self.follow_offset, FOLLOW_SPAN, self.follow_at);
        let strip = div()
            .id("follow-strip")
            .relative()
            .w(px(width as f32))
            .h(px(96.))
            .overflow_hidden()
            .rounded(theme.shape.radius_md)
            .border_1()
            .border_color(colors.border)
            .bg(colors.background)
            .on_scroll_wheel(
                cx.listener(|this, event: &gpui::ScrollWheelEvent, window, cx| {
                    let delta = event.delta.pixel_delta(window.line_height());
                    let along = if delta.x.abs() > delta.y.abs() {
                        delta.x
                    } else {
                        delta.y
                    };
                    let beats = -f64::from(f32::from(along)) / 480. * FOLLOW_SPAN;
                    this.follow_offset =
                        (this.follow_offset + beats).clamp(0., FOLLOW_LENGTH - FOLLOW_SPAN);
                    // The strip takes the wheel; the page around it stays put.
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .children(bars)
            .children(clips)
            .child(
                div()
                    .absolute()
                    .left(x(self.follow_at))
                    .top_0()
                    .bottom_0()
                    .w(px(1.5))
                    .bg(colors.playhead),
            )
            .children(side.map(|side| {
                BackToPlayhead::new("back-to-playhead", side).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.follow_offset = Follow::page(FOLLOW_SPAN, this.follow_at)
                            .min(FOLLOW_LENGTH - FOLLOW_SPAN);
                        this.follow.engage();
                        cx.notify();
                    },
                ))
            }));
        div()
            .flex()
            .items_center()
            .gap(theme.spacing.md)
            .child(strip)
            .child(
                Button::icon_only(
                    "follow-play",
                    if playing {
                        IconName::Pause
                    } else {
                        IconName::Play
                    },
                )
                .tooltip(if playing { "Pause" } else { "Play" })
                .on_click(cx.listener(|this, _, _, cx| this.toggle_follow_play(cx))),
            )
            .child(
                Label::new("Scroll the strip while it plays")
                    .size(TextSize::Small)
                    .color(TextColor::Muted),
            )
            .into_any_element()
    }

    fn toggle_follow_play(&mut self, cx: &mut Context<Self>) {
        if self.follow_run.take().is_some() {
            self.follow
                .update(self.follow_offset, FOLLOW_SPAN, self.follow_at, false);
            cx.notify();
            return;
        }
        self.follow_run = Some(cx.spawn(async move |this, cx| {
            let mut then = std::time::Instant::now();
            loop {
                cx.background_executor().timer(FOLLOW_TICK).await;
                let now = std::time::Instant::now();
                let elapsed = now - then;
                then = now;
                let going = this.update(cx, |this, cx| {
                    let step = elapsed.as_secs_f64() * FOLLOW_BEATS_PER_SECOND;
                    this.follow_at = (this.follow_at + step) % FOLLOW_LENGTH;
                    if let Some(to) =
                        this.follow
                            .update(this.follow_offset, FOLLOW_SPAN, this.follow_at, true)
                    {
                        this.follow_offset = to.min(FOLLOW_LENGTH - FOLLOW_SPAN);
                    }
                    cx.notify();
                });
                if going.is_err() {
                    return;
                }
            }
        }));
        cx.notify();
    }
}

impl Render for Showcase {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let fit = Fit::new(f32::from(window.viewport_size().width), self.agent_open);
        let page = match self.page {
            Page::Arrange => self.arrange(fit, cx).into_any_element(),
            Page::Components => self.components(window, cx).into_any_element(),
        };
        let tabs = Tabs::new("pages", ["Arrange", "Components"], self.page as usize).on_select(
            cx.listener(|this, index: &usize, _, cx| {
                this.page = if *index == 0 {
                    Page::Arrange
                } else {
                    Page::Components
                };
                cx.notify();
            }),
        );
        WindowFrame::new()
            .font(theme.typography.ui.clone())
            .text_color(theme.colors.text)
            .size_full()
            .flex()
            .flex_col()
            .child(self.title_bar(fit, cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .when(fit.sidebar, |row| row.child(self.sidebar(cx)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .bg(theme.colors.surface)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(theme.spacing.sm)
                                    .h(theme.density.control_lg + theme.spacing.sm)
                                    .px(theme.spacing.md)
                                    .border_b_1()
                                    .border_color(theme.colors.border)
                                    .child(tabs)
                                    .child(div().flex_1())
                                    .child(
                                        div().min_w_0().truncate().child(
                                            Label::new(format!(
                                                "{} · {}",
                                                theme.name(),
                                                if theme.material == atelier::Material::Translucent
                                                {
                                                    "translucent chrome"
                                                } else {
                                                    "opaque chrome"
                                                }
                                            ))
                                            .size(TextSize::XSmall)
                                            .color(TextColor::Faint),
                                        ),
                                    ),
                            )
                            .child(page),
                    )
                    .when(self.agent_open, |row| row.child(self.agent_panel(fit, cx))),
            )
            .children(self.dialog(cx))
    }
}

fn main() {
    let direction = argument("--direction")
        .and_then(|id| Direction::from_id(&id))
        .unwrap_or_default();
    let appearance = match argument("--appearance").as_deref() {
        Some("light") => Appearance::Light,
        _ => Appearance::Dark,
    };
    let page = match argument("--page").as_deref() {
        Some("components") => Page::Components,
        _ => Page::Arrange,
    };
    let agent_open = !std::env::args().any(|arg| arg == "--agent-closed");
    let (width, height) = common::size_from_args(1480., 920.);

    Application::new()
        .with_assets(Assets::default())
        .run(move |cx: &mut App| {
            let theme = Theme::new(direction, appearance);
            let background = theme.window_background();
            atelier::init(cx, theme);
            let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
            let options = atelier::custom_frame(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(common::MIN_WIDTH), px(common::MIN_HEIGHT))),
                    window_background: background,
                    titlebar: Some(TitlebarOptions {
                        title: Some("Atelier showcase".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                cx.theme(),
            );
            cx.open_window(options, |window, cx| {
                atelier::set_window_background(window, &Theme::new(direction, appearance));
                cx.new(|cx| Showcase {
                    direction,
                    appearance,
                    page,
                    agent_open,
                    reasoning_open: false,
                    tool_open: false,
                    selected_track: 1,
                    mute: [false, false, false, false, true],
                    solo: [false, true, false, false, false],
                    switches: [true, false],
                    checks: [true, false],
                    tool: 1,
                    tab: 1,
                    disclosures: [true, false, false],
                    progress: 0.35,
                    plan_step: 2,
                    dialog: match argument("--dialog").as_deref() {
                        Some("delete") => Some(OpenDialog::Delete),
                        Some("approval") => Some(OpenDialog::Approval),
                        _ => None,
                    },
                    reviewing: false,
                    project: cx.new(|cx| TextInput::new(cx).with_text("Midnight Drive", cx)),
                    track_name: "Lead Vocal".into(),
                    renaming: false,
                    rename: cx.new(TextInput::new),
                    spots: SpotBook::new(),
                    events: [
                        ["1.1", "C4", "96"],
                        ["1.2", "E4", "88"],
                        ["1.3", "CC11 Expression", "72"],
                        ["1.3", "G4", "90"],
                        ["2.1", "Pitch bend", "+2"],
                        ["2.1", "C5", "100"],
                        ["2.3", "B4", "84"],
                    ]
                    .map(|row| row.map(SharedString::from))
                    .to_vec(),
                    event_row: None,
                    event_edit: None,
                    cell_input: cx.new(TextInput::new),
                    event_scroll: gpui::UniformListScrollHandle::new(),
                    follow: Follow::new(),
                    follow_offset: 0.,
                    follow_at: 2.,
                    follow_run: None,
                })
            })
            .expect("failed to open the showcase window");
            cx.activate(true);
        });
}
