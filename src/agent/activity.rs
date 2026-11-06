//! What the agent is doing now, what it plans to do, and what it has changed.

use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyElement, App, ElementId, Hsla, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};

use crate::controls::held;
use crate::display::{Spinner, StatusDot, Tone};
use crate::focus::focusable;
use crate::icon::{Icon, IconName};
use crate::progress::{ProgressBar, ProgressRing, StepList, StepState};
use crate::strings::ActiveStrings;
use crate::text::{Label, TextColor, TextSize};
use crate::theme::{ActiveTheme, Theme};

type IndexHandler = Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>;
type ToggleHandler = Rc<dyn Fn(&bool, &mut Window, &mut App) + 'static>;

/// Where a running turn is, finer than [`super::AgentStatus::Running`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AgentPhase {
    /// The request is sent and nothing has come back yet.
    #[default]
    Waiting,
    /// The model is reading the prompt (prefill).
    Reading,
    /// The model is reasoning before it answers.
    Thinking,
    /// The model is writing its answer (decode).
    Writing,
    /// A tool is running; name it with [`AgentActivity::detail`].
    Tool,
    /// The turn is paused for the person's decision.
    Approval,
    /// Earlier turns are being summarised to make room in the context.
    Compacting,
    /// A reviewer model is deciding a request.
    Reviewing,
}

impl AgentPhase {
    /// Every phase, in the order a turn usually passes through them.
    pub const ALL: [Self; 8] = [
        Self::Waiting,
        Self::Reading,
        Self::Thinking,
        Self::Writing,
        Self::Tool,
        Self::Approval,
        Self::Compacting,
        Self::Reviewing,
    ];

    /// The phase's words, from [`crate::Strings::agent_phases`].
    pub fn label(self, cx: &App) -> SharedString {
        cx.strings().agent_phases[self as usize].clone()
    }

    /// The phase's colour: the agent's own, or the warning colour while it waits on the person.
    pub fn color(self, theme: &Theme) -> Hsla {
        match self {
            Self::Approval => theme.colors.warning,
            _ => theme.colors.agent,
        }
    }

    /// Whether the agent is waiting on the person rather than working.
    pub fn waits_on_person(self) -> bool {
        self == Self::Approval
    }
}

/// One line of what a running turn is doing: its phase and what it is working on, then how
/// long it has run, how fast it is writing and how full the context is.
///
/// Put it above the composer or in the status bar while a turn runs, and take it away when the
/// turn ends; [`super::AgentStatusLabel`] says how it ended. Every figure is optional, so an
/// app shows what its backend reports. The phase leads and the figures give way first when it
/// runs out of room.
#[derive(IntoElement)]
pub struct AgentActivity {
    id: ElementId,
    phase: AgentPhase,
    detail: Option<SharedString>,
    progress: Option<f32>,
    elapsed: Option<Duration>,
    rate: Option<f32>,
    tokens: Option<u64>,
    context: Option<f32>,
}

impl AgentActivity {
    /// The activity strip for a turn in `phase`.
    pub fn new(id: impl Into<ElementId>, phase: AgentPhase) -> Self {
        Self {
            id: id.into(),
            phase,
            detail: None,
            progress: None,
            elapsed: None,
            rate: None,
            tokens: None,
            context: None,
        }
    }

    /// What the phase is working on: the tool's command, the file being read.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// How far through the phase it is, 0–1, for a phase that knows (reading a long prompt).
    pub fn progress(mut self, progress: f32) -> Self {
        self.progress = Some(progress.clamp(0., 1.));
        self
    }

    /// How long the turn has run.
    pub fn elapsed(mut self, elapsed: Duration) -> Self {
        self.elapsed = Some(elapsed);
        self
    }

    /// Tokens per second while it writes.
    pub fn rate(mut self, tokens_per_second: f32) -> Self {
        self.rate = Some(tokens_per_second);
        self
    }

    /// Tokens the turn has written so far.
    pub fn tokens(mut self, tokens: u64) -> Self {
        self.tokens = Some(tokens);
        self
    }

    /// How full the context window is, 0–1. Shown as a ring that warns past 80 % and alarms
    /// past 95 %, when compacting is near.
    pub fn context(mut self, used: f32) -> Self {
        self.context = Some(used.clamp(0., 1.));
        self
    }
}

/// "12 s", "1:05", "1:02:05".
pub(crate) fn format_elapsed(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    if seconds < 60 {
        format!("{seconds} s")
    } else if seconds < 3600 {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    } else {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            (seconds / 60) % 60,
            seconds % 60
        )
    }
}

/// "820", "1.2k", "34k", "1.5M".
pub(crate) fn format_count(count: u64) -> String {
    match count {
        0..=999 => count.to_string(),
        1_000..=9_999 => format!("{:.1}k", count as f64 / 1_000.),
        10_000..=999_999 => format!("{}k", count / 1_000),
        _ => format!("{:.1}M", count as f64 / 1_000_000.),
    }
}

fn context_tone(used: f32) -> Tone {
    if used >= 0.95 {
        Tone::Danger
    } else if used >= 0.8 {
        Tone::Warning
    } else {
        Tone::Neutral
    }
}

impl RenderOnce for AgentActivity {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let color = self.phase.color(theme);
        let mut label = self.phase.label(cx).to_string();
        if let Some(progress) = self.progress {
            label.push_str(&format!(" {} %", (progress * 100.).round() as u32));
        }
        let mark = if self.phase.waits_on_person() {
            StatusDot::new(color).size(px(7.)).into_any_element()
        } else {
            Spinner::new((self.id.clone(), "spinner"))
                .size(theme.typography.size_sm)
                .into_any_element()
        };
        let figure = |text: String| {
            div()
                .flex_none()
                .whitespace_nowrap()
                .font(theme.typography.mono.clone())
                .child(text)
        };
        let figures = [
            self.tokens
                .map(|tokens| format!("{} tok", format_count(tokens))),
            self.rate.map(|rate| format!("{rate:.0} tok/s")),
            self.elapsed.map(format_elapsed),
        ];
        let context = self.context.map(|used| {
            let tone = context_tone(used);
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(theme.spacing.xxs)
                .when(tone != Tone::Neutral, |row| {
                    row.text_color(tone.colors(theme).0)
                })
                .child(
                    ProgressRing::new(used)
                        .id((self.id.clone(), "context"))
                        .size(px(12.))
                        .tone(match tone {
                            Tone::Neutral => Tone::Accent,
                            tone => tone,
                        }),
                )
                .child(figure(format!("{} %", (used * 100.).round() as u32)))
        });
        crate::focus::selector(div().id(self.id.clone()), &self.id)
            .flex()
            .items_center()
            .min_w_0()
            .gap(theme.spacing.sm)
            .h(theme.density.control_sm)
            .text_size(theme.typography.size_xs)
            .text_color(colors.text_faint)
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(theme.spacing.xs)
                    .child(mark)
                    .child(
                        div()
                            .whitespace_nowrap()
                            .text_color(color)
                            .font_weight(theme.typography.label_weight)
                            .child(label),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(colors.text_muted)
                    .children(self.detail),
            )
            .children(figures.into_iter().flatten().map(figure))
            .children(context)
    }
}

struct PlanStep {
    label: SharedString,
    detail: Option<SharedString>,
    state: StepState,
}

/// The agent's plan: its steps, how many are done and which one it is on.
///
/// Feed it the whole plan each time the agent updates it. Open, it lists every step; closed, it
/// keeps only the step in progress (or the next one) under the header, so the plan stays in
/// view without taking the panel. It holds its own open state unless you pass
/// [`PlanView::open`] and [`PlanView::on_toggle`].
#[derive(IntoElement)]
pub struct PlanView {
    id: ElementId,
    title: Option<SharedString>,
    steps: Vec<PlanStep>,
    open: bool,
    on_toggle: Option<ToggleHandler>,
}

impl PlanView {
    /// An empty plan, open.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            title: None,
            steps: Vec::new(),
            open: true,
            on_toggle: None,
        }
    }

    /// The header's words; [`crate::Strings::plan`] by default.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Add a step.
    pub fn step(mut self, label: impl Into<SharedString>, state: StepState) -> Self {
        self.steps.push(PlanStep {
            label: label.into(),
            detail: None,
            state,
        });
        self
    }

    /// Add a step with a line under it, such as what the running step is doing.
    pub fn step_with_detail(
        mut self,
        label: impl Into<SharedString>,
        detail: impl Into<SharedString>,
        state: StepState,
    ) -> Self {
        self.steps.push(PlanStep {
            label: label.into(),
            detail: Some(detail.into()),
            state,
        });
        self
    }

    /// Whether every step shows. Open by default.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Called with the new open state when the header is pressed.
    pub fn on_toggle(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }

    /// How many steps are finished or passed over.
    fn settled(&self) -> usize {
        self.steps
            .iter()
            .filter(|step| matches!(step.state, StepState::Done | StepState::Skipped))
            .count()
    }

    /// The step a closed plan keeps: the running or failed one, else the next pending one.
    fn current(&self) -> Option<usize> {
        self.steps
            .iter()
            .position(|step| matches!(step.state, StepState::Running | StepState::Failed))
            .or_else(|| {
                self.steps
                    .iter()
                    .position(|step| step.state == StepState::Pending)
            })
    }
}

impl RenderOnce for PlanView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (open, set_open) = held(&self.id, self.open, window, cx);
        let on_toggle = self.on_toggle.clone();
        let theme = cx.theme().clone();
        let colors = &theme.colors;
        let title = self
            .title
            .clone()
            .unwrap_or_else(|| cx.strings().plan.clone());
        let total = self.steps.len();
        let settled = self.settled();
        let current = self.current();
        let fraction = if total == 0 {
            0.
        } else {
            settled as f32 / total as f32
        };
        let header_id = ElementId::NamedChild(Box::new(self.id.clone()), "header".into());
        let hover = colors.hover;
        let header = focusable(
            div().id(header_id.clone()),
            &header_id,
            false,
            theme.shape.radius_sm,
            window,
            cx,
        )
        .flex()
        .items_center()
        .gap(theme.spacing.sm)
        .h(theme.density.row)
        .px(theme.spacing.sm)
        .rounded(theme.shape.radius_sm)
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .on_click(move |_, window, cx| {
            let next = !open;
            set_open(&next, window, cx);
            if let Some(handler) = &on_toggle {
                handler(&next, window, cx);
            }
        })
        .child(
            Icon::new(if open {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            })
            .size(px(12.))
            .color(colors.text_faint),
        )
        .child(
            Label::new(title)
                .size(TextSize::Small)
                .weight(theme.typography.label_weight),
        )
        .child(
            Label::new(format!("{settled}/{total}"))
                .size(TextSize::XSmall)
                .mono()
                .color(TextColor::Faint),
        )
        .child(div().flex_1())
        .child(
            div()
                .w(px(64.))
                .child(ProgressBar::new(fraction).height(px(3.)).tone(
                    if settled == total && total > 0 {
                        Tone::Success
                    } else {
                        Tone::Accent
                    },
                )),
        );
        let mut list = StepList::new((self.id.clone(), "steps"));
        for (index, step) in self.steps.into_iter().enumerate() {
            if !open && Some(index) != current {
                continue;
            }
            list = match step.detail {
                Some(detail) => list.step_with_detail(step.label, detail, step.state),
                None => list.step(step.label, step.state),
            };
        }
        let shows_steps = open || current.is_some();
        crate::focus::selector(div().id(self.id.clone()), &self.id)
            .flex()
            .flex_col()
            .gap(theme.spacing.xs)
            .child(header)
            .when(shows_steps, |plan| {
                plan.child(div().px(theme.spacing.sm).pb(theme.spacing.xs).child(list))
            })
    }
}

/// How a file changed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChangeKind {
    /// A new file.
    Added,
    /// An existing file, edited.
    #[default]
    Modified,
    /// A removed file.
    Deleted,
    /// A file moved or renamed.
    Renamed,
}

impl ChangeKind {
    /// The one-letter mark, as version control writes it.
    pub fn mark(self) -> &'static str {
        match self {
            Self::Added => "A",
            Self::Modified => "M",
            Self::Deleted => "D",
            Self::Renamed => "R",
        }
    }

    fn color(self, theme: &Theme) -> Hsla {
        let colors = &theme.colors;
        match self {
            Self::Added => colors.success,
            Self::Modified => colors.warning,
            Self::Deleted => colors.danger,
            Self::Renamed => colors.text_muted,
        }
    }
}

/// One file the agent changed, with its line counts.
#[derive(Clone, Debug, PartialEq)]
pub struct FileChange {
    /// The path, relative to the project.
    pub path: SharedString,
    /// How it changed.
    pub kind: ChangeKind,
    /// Lines added.
    pub added: u32,
    /// Lines removed.
    pub removed: u32,
}

impl FileChange {
    /// An edited file.
    pub fn new(path: impl Into<SharedString>, added: u32, removed: u32) -> Self {
        Self {
            path: path.into(),
            kind: ChangeKind::Modified,
            added,
            removed,
        }
    }

    /// Say how it changed; [`ChangeKind::Modified`] by default.
    pub fn kind(mut self, kind: ChangeKind) -> Self {
        self.kind = kind;
        self
    }
}

/// The files a turn or a session changed, with lines added and removed: what to review before
/// keeping the agent's work.
///
/// Each row names the file before its folder, so the name survives truncation. Pass
/// [`ChangedFiles::on_open`] to open a file's diff on press, and put the bulk choices ("Keep
/// all", "Undo all") in [`ChangedFiles::action`].
#[derive(IntoElement)]
pub struct ChangedFiles {
    id: ElementId,
    title: Option<SharedString>,
    files: Vec<FileChange>,
    actions: Vec<AnyElement>,
    on_open: Option<IndexHandler>,
}

impl ChangedFiles {
    /// An empty list.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            title: None,
            files: Vec::new(),
            actions: Vec::new(),
            on_open: None,
        }
    }

    /// The header's words; [`crate::Strings::changed_files`] by default.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Add a file.
    pub fn file(mut self, file: FileChange) -> Self {
        self.files.push(file);
        self
    }

    /// Add files.
    pub fn files(mut self, files: impl IntoIterator<Item = FileChange>) -> Self {
        self.files.extend(files);
        self
    }

    /// A control at the header's trailing end.
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }

    /// Called with the file's index when its row is pressed. Makes each row a tab stop.
    pub fn on_open(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_open = Some(Rc::new(handler));
        self
    }

    /// Lines added and removed across every file.
    pub fn totals(&self) -> (u32, u32) {
        self.files.iter().fold((0, 0), |(added, removed), file| {
            (added + file.added, removed + file.removed)
        })
    }
}

/// `src/agent/chat.rs` as ("chat.rs", "src/agent").
fn split_path(path: &str) -> (&str, &str) {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(at) => (&trimmed[at + 1..], &trimmed[..at]),
        None => (trimmed, ""),
    }
}

fn counts(added: u32, removed: u32, theme: &Theme) -> impl IntoElement {
    let colors = &theme.colors;
    div()
        .flex()
        .flex_none()
        .gap(theme.spacing.xs)
        .font(theme.typography.mono.clone())
        .text_size(theme.typography.size_xs)
        .whitespace_nowrap()
        .child(div().text_color(colors.success).child(format!("+{added}")))
        .child(div().text_color(colors.danger).child(format!("−{removed}")))
}

impl RenderOnce for ChangedFiles {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let colors = &theme.colors;
        let (added, removed) = self.totals();
        let title = self
            .title
            .clone()
            .unwrap_or_else(|| cx.strings().changed_files.clone());
        let header = div()
            .flex()
            .items_center()
            .gap(theme.spacing.sm)
            .min_h(theme.density.row)
            .px(theme.spacing.sm)
            .child(
                Label::new(title)
                    .size(TextSize::Small)
                    .weight(theme.typography.label_weight),
            )
            .child(
                Label::new(self.files.len().to_string())
                    .size(TextSize::XSmall)
                    .mono()
                    .color(TextColor::Faint),
            )
            .child(counts(added, removed, &theme))
            .child(div().flex_1())
            .children(self.actions);
        let hover = colors.hover;
        let rows = self.files.into_iter().enumerate().map(|(index, file)| {
            let row_id = ElementId::NamedChild(Box::new(self.id.clone()), index.to_string().into());
            let (name, folder) = split_path(&file.path);
            let (name, folder) = (name.to_string(), folder.to_string());
            let open = self.on_open.clone();
            let deleted = file.kind == ChangeKind::Deleted;
            focusable(
                div().id(row_id.clone()),
                &row_id,
                open.is_none(),
                theme.shape.radius_sm,
                window,
                cx,
            )
            .flex()
            .items_center()
            .gap(theme.spacing.sm)
            .h(theme.density.row)
            .px(theme.spacing.sm)
            .rounded(theme.shape.radius_sm)
            .text_size(theme.typography.size_sm)
            .when_some(open, |row, open| {
                row.cursor_pointer()
                    .hover(move |style| style.bg(hover))
                    .on_click(move |_, window, cx| open(&index, window, cx))
            })
            .child(
                div()
                    .flex_none()
                    .w(px(12.))
                    .font(theme.typography.mono.clone())
                    .text_size(theme.typography.size_xs)
                    .font_weight(theme.typography.label_weight)
                    .text_color(file.kind.color(&theme))
                    .child(file.kind.mark()),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_baseline()
                    .gap(theme.spacing.xs)
                    .child(
                        div()
                            .flex_none()
                            .max_w_full()
                            .truncate()
                            .text_color(if deleted {
                                colors.text_muted
                            } else {
                                colors.text
                            })
                            .when(deleted, |name| name.line_through())
                            .child(name),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(theme.typography.size_xs)
                            .text_color(colors.text_faint)
                            .child(folder),
                    ),
            )
            .child(counts(file.added, file.removed, &theme))
        });
        crate::focus::selector(div().id(self.id.clone()), &self.id)
            .flex()
            .flex_col()
            .child(header)
            .children(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elapsed_reads_as_seconds_then_a_clock() {
        assert_eq!(format_elapsed(Duration::from_secs(12)), "12 s");
        assert_eq!(format_elapsed(Duration::from_secs(65)), "1:05");
        assert_eq!(format_elapsed(Duration::from_secs(3725)), "1:02:05");
    }

    #[test]
    fn counts_shorten() {
        assert_eq!(format_count(820), "820");
        assert_eq!(format_count(1_240), "1.2k");
        assert_eq!(format_count(34_500), "34k");
        assert_eq!(format_count(1_520_000), "1.5M");
    }

    #[test]
    fn the_context_warns_as_it_fills() {
        assert_eq!(context_tone(0.5), Tone::Neutral);
        assert_eq!(context_tone(0.85), Tone::Warning);
        assert_eq!(context_tone(0.97), Tone::Danger);
    }

    #[test]
    fn a_closed_plan_keeps_the_step_in_progress() {
        let plan = PlanView::new("plan")
            .step("Read", StepState::Done)
            .step("Edit", StepState::Running)
            .step("Test", StepState::Pending);
        assert_eq!(plan.current(), Some(1));
        assert_eq!(plan.settled(), 1);
        let waiting = PlanView::new("plan")
            .step("Read", StepState::Done)
            .step("Test", StepState::Pending);
        assert_eq!(waiting.current(), Some(1));
    }

    #[test]
    fn a_path_splits_into_name_and_folder() {
        assert_eq!(split_path("src/agent/chat.rs"), ("chat.rs", "src/agent"));
        assert_eq!(split_path("README.md"), ("README.md", ""));
    }

    #[test]
    fn totals_add_up() {
        let files = ChangedFiles::new("files")
            .file(FileChange::new("a.rs", 3, 1))
            .file(FileChange::new("b.rs", 10, 0).kind(ChangeKind::Added));
        assert_eq!(files.totals(), (13, 1));
    }
}
