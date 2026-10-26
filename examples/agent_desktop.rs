//! A mock coding-agent desktop app in the shape of Claude Code: sessions on the left, the
//! conversation in the middle, the changes it made on the right. Nothing here runs.
//!
//! ```text
//! cargo run --example agent_desktop -- [--direction glass|studio|mono] [--appearance dark|light]
//! ```

mod common;

use atelier::agent::{
    AgentStatus, ApprovalCard, AssistantMessage, CodeBlock, Composer, ContextChip, ConversationRow,
    DiffBlock, DiffLine, ReasoningBlock, ToolCall, ToolGroup, ToolState, UserMessage,
};
use atelier::{
    ActiveTheme, Appearance, Avatar, Badge, Button, ControlSize, Direction, Divider, Edge,
    IconName, Label, ListItem, Sidebar, StepList, StepState, Tabs, TaskProgress, TextColor,
    TextField, TextSize, TitleBar, Tone,
};
use common::Themed;
use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div, prelude::*, px};

struct AgentDesktop {
    choice: (Direction, Appearance),
    session: usize,
    pane_tab: usize,
    reasoning_open: bool,
    tool_open: bool,
}

impl Themed for AgentDesktop {
    fn choice(&mut self) -> &mut (Direction, Appearance) {
        &mut self.choice
    }
}

const SESSIONS: [(&str, &str, AgentStatus); 6] = [
    (
        "Fix flaky playback test",
        "now",
        AgentStatus::AwaitingApproval,
    ),
    ("Add stems export to CLI", "4 min", AgentStatus::Running),
    ("Port mixer graph to Atelier", "11:20", AgentStatus::Idle),
    ("Profile physical piano", "09:05", AgentStatus::Idle),
    (
        "Update VOICEVOX pronunciation",
        "Yesterday",
        AgentStatus::Idle,
    ),
    ("Review CLAP host PR", "Yesterday", AgentStatus::Idle),
];

impl AgentDesktop {
    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let section = |label: &'static str| {
            div()
                .px(theme.spacing.sm)
                .pt(theme.spacing.md)
                .pb(theme.spacing.xs)
                .child(Label::caption(label))
        };
        let row = |index: usize, cx: &Context<Self>| {
            let (title, time, status) = SESSIONS[index];
            ConversationRow::new(("session", index), title, time)
                .status(status)
                .selected(index == self.session)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.session = index;
                    cx.notify();
                }))
        };
        Sidebar::new(Edge::Left)
            .w(px(272.))
            .p(theme.spacing.sm)
            .gap(theme.spacing.xxs)
            .child(
                Button::new("new-session", "New session")
                    .icon(IconName::Plus)
                    .full_width(),
            )
            .child(
                div().pt(theme.spacing.sm).child(
                    TextField::new()
                        .icon(IconName::Search)
                        .placeholder("Search sessions"),
                ),
            )
            .child(section("Running"))
            .child(row(0, cx))
            .child(row(1, cx))
            .child(section("Today"))
            .child(row(2, cx))
            .child(row(3, cx))
            .child(section("Yesterday"))
            .child(row(4, cx))
            .child(row(5, cx))
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme.spacing.sm)
                    .p(theme.spacing.sm)
                    .border_t_1()
                    .border_color(theme.colors.separator)
                    .child(Avatar::person("UT"))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .child(Label::new("uthree").size(TextSize::Small))
                            .child(
                                Label::new("auris-studio")
                                    .size(TextSize::XSmall)
                                    .color(TextColor::Faint),
                            ),
                    )
                    .child(Button::icon_only("settings", IconName::Settings).tooltip("Settings…")),
            )
    }

    fn transcript(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let entries = div()
            .flex()
            .flex_col()
            .gap(theme.spacing.lg)
            .w_full()
            .max_w(px(780.))
            .child(
                UserMessage::new(
                    "The playback test fails about one run in ten on Windows. Find out why and fix it.",
                )
                .id("m1")
                .attachment(ContextChip::new("ctx-test", IconName::File, "engine/tests/playback.rs")),
            )
            .child(
                ReasoningBlock::new(
                    "reasoning",
                    "Thought for 9 s",
                    "One in ten points at a race rather than a logic error. The test starts the transport and reads the position straight away; if the audio thread has not taken the first block yet, the position is still zero.",
                )
                .open(self.reasoning_open)
                .on_toggle(cx.listener(|this, _, _, cx| {
                    this.reasoning_open = !this.reasoning_open;
                    cx.notify();
                })),
            )
            .child(
                ToolGroup::new("Used 4 tools")
                    .call(
                        ToolCall::new("t1", "bash", "Ran the test 50 times", ToolState::Succeeded)
                            .icon(IconName::Code)
                            .detail("$ for i in $(seq 50); do cargo test -p auris-engine playback; done\n→ 6 failures in 50 runs")
                            .open(self.tool_open)
                            .on_toggle(cx.listener(|this, _, _, cx| {
                                this.tool_open = !this.tool_open;
                                cx.notify();
                            })),
                    )
                    .call(ToolCall::new("t2", "read", "Read engine/src/transport.rs", ToolState::Succeeded).icon(IconName::Eye))
                    .call(ToolCall::new("t3", "grep", "Searched for position()", ToolState::Succeeded).icon(IconName::Search))
                    .call(ToolCall::new("t4", "edit", "Edited engine/src/transport.rs", ToolState::Succeeded).icon(IconName::File)),
            )
            .child(
                CodeBlock::new(
                    "output",
                    "test playback::starts_from_zero ... FAILED\n\nassertion `left == right` failed\n  left: 0\n right: 512\n\nfailures: 1 of 50 runs",
                )
                .title("cargo test -p auris-engine playback")
                .copyable(),
            )
            .child(
                AssistantMessage::new(
                    "Found it. `play()` returns before the audio thread has taken its first block, so a position read right after it can still be zero.\n\nThe fix makes `play()` wait for the first block to be acknowledged, with a timeout so a stalled device cannot hang the UI:",
                )
                .id("m2")
                .meta("Claude Opus 5.5 · 41 s"),
            )
            .child(
                DiffBlock::new("engine/src/transport.rs")
                    .lines([
                        DiffLine::Hunk("@@ -88,7 +88,11 @@ impl Transport".into()),
                        DiffLine::Context(88, "    pub fn play(&mut self) -> Result<()> {".into()),
                        DiffLine::Context(89, "        self.commands.send(Command::Play)?;".into()),
                        DiffLine::Removed(90, "        Ok(())".into()),
                        DiffLine::Added(90, "        // The audio thread acknowledges the first block it renders.".into()),
                        DiffLine::Added(91, "        self.started".into()),
                        DiffLine::Added(92, "            .recv_timeout(START_TIMEOUT)".into()),
                        DiffLine::Added(93, "            .map_err(|_| Error::DeviceStalled)".into()),
                        DiffLine::Context(94, "    }".into()),
                    ])
                    .action(Button::new("open-file", "Open").size(ControlSize::Small).ghost()),
            )
            .child(
                StepList::new("tasks")
                    .step("Reproduce the failure", StepState::Done)
                    .step("Find the cause", StepState::Done)
                    .step("Fix the start-up race", StepState::Done)
                    .step_with_detail(
                        "Run the full test suite",
                        "Waiting for your approval",
                        StepState::Running,
                    ),
            )
            .child(
                ApprovalCard::new("approve-tests", "Run `cargo test --workspace`?")
                    .body("Runs about 2,400 tests. Nothing is written outside target/.")
                    .allow_label("Run")
                    .on_always(|_, _, _| {}),
            );
        div()
            .id("transcript")
            .flex()
            .flex_col()
            .items_center()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(theme.spacing.xl)
            .py(theme.spacing.xl)
            .child(entries)
    }

    fn changes_pane(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let file = |index: usize, path: &'static str, added: u32, removed: u32| {
            ListItem::new(("changed", index), path)
                .icon(IconName::File)
                .trailing(
                    div()
                        .flex()
                        .gap(px(6.))
                        .child(
                            Label::new(format!("+{added}"))
                                .size(TextSize::XSmall)
                                .mono()
                                .color(TextColor::Success),
                        )
                        .child(
                            Label::new(format!("−{removed}"))
                                .size(TextSize::XSmall)
                                .mono()
                                .color(TextColor::Danger),
                        ),
                )
        };
        Sidebar::new(Edge::Right)
            .w(px(320.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(theme.density.toolbar)
                    .px(theme.spacing.sm)
                    .border_b_1()
                    .border_color(theme.colors.border)
                    .child(
                        Tabs::new("pane-tabs", ["Changes", "Tasks", "Terminal"], self.pane_tab)
                            .on_select(cx.listener(|this, index: &usize, _, cx| {
                                this.pane_tab = *index;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .gap(theme.spacing.xxs)
                    .p(theme.spacing.sm)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .px(theme.spacing.sm)
                            .py(theme.spacing.xs)
                            .child(Label::caption("3 files changed"))
                            .child(div().flex_1())
                            .child(Badge::new("claude/fix-playback").icon(IconName::Code)),
                    )
                    .child(file(0, "engine/src/transport.rs", 9, 1))
                    .child(file(1, "engine/tests/playback.rs", 4, 6))
                    .child(file(2, "engine/src/error.rs", 3, 0))
                    .child(div().h(theme.spacing.lg))
                    .child(
                        TaskProgress::new("tests", "Workspace tests")
                            .icon(IconName::Code)
                            .state(atelier::TaskState::Queued)
                            .detail("Starts when you allow it"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .flex()
                            .gap(theme.spacing.sm)
                            .p(theme.spacing.sm)
                            .child(
                                div()
                                    .flex_1()
                                    .child(Button::new("commit", "Commit…").full_width()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .child(Button::new("pr", "Create PR…").full_width()),
                            ),
                    ),
            )
    }
}

impl Render for AgentDesktop {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let switch = common::theme_switch(self, cx);
        let (title, _, status) = SESSIONS[self.session];
        div()
            .font(theme.typography.ui.clone())
            .text_color(theme.colors.text)
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.window_fill())
            .child(
                TitleBar::new()
                    .leading(
                        Button::icon_only("toggle-sidebar", IconName::SidebarLeft)
                            .tooltip("Hide sidebar"),
                    )
                    .leading(Button::icon_only("back", IconName::ChevronLeft).tooltip("Back"))
                    .leading(
                        Button::icon_only("forward", IconName::ChevronRight).tooltip("Forward"),
                    )
                    .title(title)
                    .subtitle("auris-studio · claude/fix-playback")
                    .child(atelier::agent::AgentStatusLabel::new(status))
                    .trailing(switch)
                    .trailing(Divider::vertical())
                    .trailing(Button::icon_only("terminal", IconName::Code).tooltip("Terminal"))
                    .trailing(Button::icon_only("files", IconName::Folder).tooltip("Files"))
                    .trailing(Button::icon_only("more", IconName::More).tooltip("More…")),
            )
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
                            .child(self.transcript(cx))
                            .child(
                                div()
                                    .flex()
                                    .justify_center()
                                    .px(theme.spacing.xl)
                                    .pb(theme.spacing.lg)
                                    .child(
                                        div().w_full().max_w(px(780.)).child(
                                            Composer::new("composer")
                                                .placeholder("Reply, or describe the next change…")
                                                .context(
                                                    ContextChip::new(
                                                        "ctx-file",
                                                        IconName::File,
                                                        "transport.rs",
                                                    )
                                                    .on_remove(|_, _, _| {}),
                                                )
                                                .model("Claude Opus 5.5"),
                                        ),
                                    ),
                            ),
                    )
                    .child(self.changes_pane(cx)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme.spacing.md)
                    .h(px(24.))
                    .px(theme.spacing.md)
                    .bg(theme.colors.chrome)
                    .border_t_1()
                    .border_color(theme.colors.border)
                    .child(
                        Label::new("Auto mode")
                            .size(TextSize::XSmall)
                            .color(TextColor::Muted),
                    )
                    .child(
                        Label::new("Context 38 %")
                            .size(TextSize::XSmall)
                            .color(TextColor::Faint),
                    )
                    .child(div().flex_1())
                    .child(Badge::new("1 waiting").tone(Tone::Warning).dot()),
            )
    }
}

fn main() {
    common::run("Agent", 1500., 940., |choice, _| AgentDesktop {
        choice,
        session: 0,
        pane_tab: 0,
        reasoning_open: false,
        tool_open: true,
    });
}
