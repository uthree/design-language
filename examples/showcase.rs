//! A mock creator tool for judging the design candidates side by side.
//!
//! Nothing here works: the timeline, the mixer and the assistant show fixed sample data. Switch
//! the candidate and the appearance from the title bar.
//!
//! ```text
//! cargo run --example showcase -- [--direction studio|glass|mono] [--appearance dark|light]
//!                                 [--page arrange|components] [--agent-closed]
//! ```

use atelier::agent::{
    AgentPanel, AgentStatus, AgentToggle, ApprovalCard, AskBar, AssistantMessage, ChatNotice,
    Composer, ContextChip, ReasoningBlock, SuggestionChip, ThinkingIndicator, ToolCall, ToolGroup,
    ToolState, UserMessage,
};
use atelier::{
    ActiveTheme, Appearance, Assets, Avatar, Badge, Button, ButtonVariant, Card, Checkbox,
    ControlSize, Direction, Disclosure, Divider, Edge, EmptyState, Icon, IconName, Kbd, Knob,
    Label, LevelMeter, ListItem, MenuItem, Popover, ProgressBar, SectionHeader, Segment,
    SegmentedControl, Sidebar, Slider, Spinner, Switch, Tabs, TextColor, TextField, TextSize,
    Theme, TitleBar, Tone, menu_separator,
};
use gpui::{
    AnyElement, App, Application, Bounds, Context, Hsla, IntoElement, ParentElement, Render,
    SharedString, Styled, TitlebarOptions, Window, WindowBounds, WindowOptions, div, prelude::*,
    px, relative, rgb, size,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Arrange,
    Components,
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
        window.set_background_appearance(theme.window_background());
        theme.install(cx);
    }

    fn title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let direction_index = Direction::ALL
            .iter()
            .position(|d| *d == self.direction)
            .unwrap_or(0);
        TitleBar::new()
            .leading(Button::icon_only("sidebar", IconName::SidebarLeft).tooltip("Hide sidebar"))
            .title("Midnight Drive")
            .subtitle("Edited 2 min ago")
            .child(div().w(theme.spacing.lg))
            .child(transport(cx))
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
            .w(px(232.))
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

    fn arrange(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let colors = &theme.colors;
        let header_width = px(196.);
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
            .child(div().flex_1().relative().children((0..9).map(|bar| {
                div()
                    .absolute()
                    .left(relative(bar as f32 / 8.))
                    .top_0()
                    .bottom_0()
                    .pl(theme.spacing.xs)
                    .border_l_1()
                    .border_color(colors.border)
                    .flex()
                    .items_center()
                    .child(
                        Label::new(format!("{}", bar * 4 + 1))
                            .size(TextSize::XSmall)
                            .mono()
                            .color(TextColor::Faint),
                    )
            })));

        let rows = TRACKS.iter().enumerate().map(|(index, track)| {
            let color = track_color(track);
            let selected = index == self.selected_track;
            let mute = self.mute[index];
            let solo = self.solo[index];
            div()
                .flex()
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
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .bg(colors.background)
                    .children(rows)
                    .child(playhead),
            )
            .child(self.inspector(cx))
    }

    fn inspector(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
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
                    .w(px(220.))
                    .gap(theme.spacing.md)
                    .child(
                        SectionHeader::new(track.name)
                            .description("Instrument · Grand Piano (Physical)"),
                    )
                    .child(Slider::new(0.72).label("Volume").readout("-3.2 dB"))
                    .child(Slider::new(0.42).label("Pan").readout("L 16").bipolar())
                    .child(Switch::new("monitor", true).label("Input monitoring")),
            )
            .child(Divider::vertical().full())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.spacing.md)
                    .child(SectionHeader::caption("Channel EQ"))
                    .child(
                        div()
                            .flex()
                            .gap(theme.spacing.lg)
                            .child(Knob::new(0.62).label("Low").readout("+2.1"))
                            .child(Knob::new(0.5).bipolar().label("Mid").readout("0.0"))
                            .child(Knob::new(0.35).bipolar().label("High").readout("-1.8"))
                            .child(Knob::new(0.8).color(color).label("Drive").readout("80 %"))
                            .child(Knob::new(0.25).label("Mix").readout("25 %")),
                    ),
            )
            .child(Divider::vertical().full())
            .child(
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
            .child(
                div()
                    .flex()
                    .gap(theme.spacing.xs)
                    .h_full()
                    .child(LevelMeter::new(0.74).peak(0.8))
                    .child(LevelMeter::new(0.69).peak(0.77)),
            )
    }

    fn agent_panel(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let border = cx.theme().colors.border;
        let piano = track_color(&TRACKS[1]);
        let panel = AgentPanel::new("agent", "Reharmonise the chorus")
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
                    .focused(true),
            )
;
        div()
            .w(px(380.))
            .flex_none()
            .border_l_1()
            .border_color(border)
            .child(panel)
    }

    fn components(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
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

        let controls = row()
            .child(Switch::new("s-on", true).label("Snap to grid"))
            .child(Switch::new("s-off", false).label("Follow playhead"))
            .child(Checkbox::new("c-on", true).label("Count-in"))
            .child(Checkbox::new("c-off", false).label("Pre-roll"))
            .child(SegmentedControl::new(
                "tools",
                [
                    Segment::icon(IconName::Play),
                    Segment::label("Pencil"),
                    Segment::label("Scissors"),
                    Segment::label("Glue"),
                ],
                1,
            ))
            .child(Tabs::new("tabs", ["Score", "Piano roll", "Audio"], 1));

        let fields = div()
            .flex()
            .gap(theme.spacing.md)
            .child(
                div().w(px(240.)).child(
                    TextField::new()
                        .icon(IconName::Search)
                        .placeholder("Search sounds…"),
                ),
            )
            .child(
                div()
                    .w(px(200.))
                    .child(TextField::new().value("Midnight Drive").focused(true)),
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
            .child(Spinner::new("spinner"))
            .child(
                div()
                    .w(px(160.))
                    .child(LevelMeter::new(0.82).peak(0.88).horizontal()),
            );

        let surfaces = div()
            .flex()
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
                    .child(Disclosure::new("disc", "Automation", true).trailing(Badge::new("3")))
                    .child(Disclosure::new("disc2", "Sends", false))
                    .child(Disclosure::new("disc3", "Plug-ins", false)),
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
            .gap(theme.spacing.lg)
            .items_start()
            .child(
                div()
                    .w(px(360.))
                    .flex()
                    .flex_col()
                    .gap(theme.spacing.md)
                    .child(ThinkingIndicator::new("Reading the score"))
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
            .child(section("Status and identity", marks.into_any_element()))
            .child(section(
                "Menus, cards and empty states",
                surfaces.into_any_element(),
            ))
            .child(section("Agent pieces", agent.into_any_element()))
    }
}

fn transport(cx: &App) -> impl IntoElement + use<> {
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
        .child(Button::icon_only("click", IconName::Metronome).tooltip("Metronome"))
        .child(
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
                .child(
                    Label::new("4/4 · C maj")
                        .size(TextSize::Small)
                        .mono()
                        .color(TextColor::Muted),
                ),
        )
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

impl Render for Showcase {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let page = match self.page {
            Page::Arrange => self.arrange(cx).into_any_element(),
            Page::Components => self.components(cx).into_any_element(),
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
        div()
            .font(theme.typography.ui.clone())
            .text_color(theme.colors.text)
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.window_fill())
            .child(self.title_bar(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.sidebar(cx))
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
                                        Label::new(format!(
                                            "{} · {}",
                                            theme.name(),
                                            if theme.material == atelier::Material::Translucent {
                                                "translucent chrome"
                                            } else {
                                                "opaque chrome"
                                            }
                                        ))
                                        .size(TextSize::XSmall)
                                        .color(TextColor::Faint),
                                    ),
                            )
                            .child(page),
                    )
                    .when(self.agent_open, |row| row.child(self.agent_panel(cx))),
            )
    }
}

fn argument(name: &str) -> Option<String> {
    let mut args = std::env::args();
    while let Some(arg) = args.next() {
        if arg == name {
            return args.next();
        }
    }
    None
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

    Application::new()
        .with_assets(Assets::default())
        .run(move |cx: &mut App| {
            let theme = Theme::new(direction, appearance);
            let background = theme.window_background();
            atelier::init(cx, theme);
            let bounds = Bounds::centered(None, size(px(1480.), px(920.)), cx);
            let options = atelier::custom_frame(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_background: background,
                    titlebar: Some(TitlebarOptions {
                        title: Some("Atelier showcase".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                cx.theme(),
            );
            cx.open_window(options, |_, cx| {
                cx.new(|_| Showcase {
                    direction,
                    appearance,
                    page,
                    agent_open,
                    reasoning_open: false,
                    tool_open: false,
                    selected_track: 1,
                    mute: [false, false, false, false, true],
                    solo: [false, true, false, false, false],
                })
            })
            .expect("failed to open the showcase window");
            cx.activate(true);
        });
}
