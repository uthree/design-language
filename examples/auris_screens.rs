//! Two Auris Studio windows redrawn in Atelier: the project chooser and "AIで曲を作る…".
//! The text is Auris's Japanese interface; the takes, players and generation are mock data.
//!
//! "AIで作り始める…" hands the chooser over to the compose screen with a [`Morph`]: the AI card's
//! shape grows to fill the window and the compose screen, laid out at its final size, fades in
//! over it.
//!
//! ```text
//! cargo run --example auris_screens -- [--screen chooser|compose] [--effort] [--slash]
//!                                       [--direction glass|studio|mono] [--appearance dark|light]
//! ```

mod common;

use atelier::agent::{
    AgentLabel, AssistantMessage, Composer, EffortButton, EffortPicker, UsageLimit, UsageMeter,
    UserMessage,
};
use atelier::{
    ActiveTheme, Appearance, AudioPlayer, Badge, Button, Completion, Completions, ControlSize,
    CopyButton, Direction, Divider, Edge, IconName, Label, ListItem, Measure, Morph, OptionCard,
    ProgressBar, ProviderLogo, SectionHeader, Segment, SegmentedControl, Sidebar, Spinner,
    StepList, StepState, Switch, TextColor, TextSize, TitleBar, WindowFrame, motion::appear,
};
use common::Themed;
use gpui::{
    AnyElement, Context, Corner, IntoElement, MouseDownEvent, ParentElement, Render, Styled,
    Window, anchored, deferred, div, point, prelude::*, px,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Chooser,
    Compose,
}

struct AurisScreens {
    choice: (Direction, Appearance),
    screen: Screen,
    /// Chooser: 0 new project, 1 recent.
    chooser_page: usize,
    /// Chooser: which way to start.
    start: usize,
    /// Compose: which take is playing.
    playing: Option<usize>,
    learn_preferences: bool,
    /// Compose: the reasoning effort, 0 low to 3 highest.
    effort: usize,
    /// Compose: fast mode.
    fast: bool,
    /// Compose: whether the effort picker is open.
    effort_open: bool,
    /// Where the effort button was laid out, so a press on it does not count as outside.
    effort_button: Measure,
    /// Where the composer was laid out, for the effort picker to open over.
    composer: Measure,
    /// Compose: where each take's scrubber is, the preview volume, and whether it is muted.
    positions: [f32; 2],
    volume: f32,
    muted: bool,
    /// Compose: the slash command being typed, with its suggestions open.
    slash: Option<usize>,
    /// Where the AI card was laid out, for the hand-off.
    ai_card: Measure,
    /// The hand-off in progress: its number, so each one plays once, and where it started.
    handoff: Option<(usize, gpui::Bounds<gpui::Pixels>)>,
    handoffs: usize,
}

impl Themed for AurisScreens {
    fn choice(&mut self) -> &mut (Direction, Appearance) {
        &mut self.choice
    }
}

/// Deterministic peaks for a take's mock waveform: an intro, a build, a loud body, an ending.
fn take_peaks(seed: f32) -> Vec<f32> {
    (0..72)
        .map(|index| {
            let x = index as f32 / 71.;
            let shape = if x < 0.12 {
                0.25 + x * 2.
            } else if x < 0.85 {
                0.6 + 0.3 * (x * 9. + seed).sin().abs()
            } else {
                (1. - x) * 4.
            };
            let grain = ((index as f32 * 12.9898 + seed * 78.233).sin() * 43_758.547)
                .fract()
                .abs();
            (shape * (0.7 + 0.3 * grain)).clamp(0.05, 1.)
        })
        .collect()
}

impl AurisScreens {
    fn chooser(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let sidebar = Sidebar::new(Edge::Left)
            .w(px(232.))
            .p(theme.spacing.sm)
            .gap(theme.spacing.xxs)
            .child(
                ListItem::new("page-new", "新規プロジェクト")
                    .icon(IconName::Plus)
                    .selected(self.chooser_page == 0)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.chooser_page = 0;
                        cx.notify();
                    })),
            )
            .child(
                ListItem::new("page-recent", "最近使った項目")
                    .icon(IconName::History)
                    .trailing(
                        Label::new("4")
                            .size(TextSize::XSmall)
                            .color(TextColor::Faint),
                    )
                    .selected(self.chooser_page == 1)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.chooser_page = 1;
                        cx.notify();
                    })),
            )
            .child(div().flex_1())
            // Setup is help, not a way to start: it lives with navigation, not with the commit.
            .child(
                Button::new("setup", "セットアップガイド…")
                    .ghost()
                    .icon(IconName::Info)
                    .full_width(),
            );

        let body = if self.chooser_page == 0 {
            let card = |index: usize, card: OptionCard, cx: &Context<Self>| {
                div()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .max_w(px(340.))
                    .when(index == 1, |slot| slot.child(self.ai_card.probe()))
                    .child(card.selected(self.start == index).on_click(cx.listener(
                        move |this, _, _, cx| {
                            this.start = index;
                            cx.notify();
                        },
                    )))
            };
            div()
                .flex()
                .flex_col()
                .gap(theme.spacing.xl)
                .child(
                    SectionHeader::new("新しいプロジェクト")
                        .description("始め方を選んでください。あとから変えられます。"),
                )
                .child(
                    div()
                        .flex()
                        .gap(theme.spacing.lg)
                        .child(card(
                            0,
                            OptionCard::new("start-empty", IconName::Music, "空のプロジェクト")
                                .description("音源トラックから、自由に曲を作り始めます。"),
                            cx,
                        ))
                        .child(card(
                            1,
                            OptionCard::new("start-ai", IconName::Sparkles, "AIで曲を作る…")
                                .description(
                                    "雰囲気を伝えると、作曲アシスタントが候補を3つ作ります。",
                                )
                                .agent(),
                            cx,
                        )),
                )
                .into_any_element()
        } else {
            let recent = [
                (
                    "成層圏突破",
                    "~/Music/Auris/stratosphere.auris",
                    "今日 11:20",
                ),
                ("雲海の決戦", "~/Music/Auris/sea-of-clouds.auris", "昨日"),
                (
                    "Midnight Drive",
                    "~/Music/Auris/midnight-drive.auris",
                    "10月21日",
                ),
                ("合唱テスト", "~/Music/Auris/choir-test.auris", "10月18日"),
            ];
            div()
                .flex()
                .flex_col()
                .gap(theme.spacing.md)
                .child(SectionHeader::new("最近使った項目"))
                .child(
                    div().flex().flex_col().gap(theme.spacing.xxs).children(
                        recent
                            .into_iter()
                            .enumerate()
                            .map(|(index, (name, path, when))| {
                                ListItem::new(("recent", index), name)
                                    .icon(IconName::File)
                                    .description(path)
                                    .trailing(
                                        Label::new(when)
                                            .size(TextSize::Small)
                                            .color(TextColor::Faint),
                                    )
                                    .selected(index == 0)
                            }),
                    ),
                )
                .into_any_element()
        };

        let footer = div()
            .flex()
            .items_center()
            .gap(theme.spacing.sm)
            .h(theme.density.toolbar)
            .px(theme.spacing.lg)
            .border_t_1()
            .border_color(theme.colors.border)
            .child(Button::new("open", "既存のプロジェクトを開く…").icon(IconName::Folder))
            .child(div().flex_1())
            .child(Button::new("close", "閉じる"))
            .child(
                Button::new(
                    "create",
                    if self.start == 1 {
                        "AIで作り始める…"
                    } else {
                        "作成"
                    },
                )
                .primary()
                .on_click(cx.listener(|this, _, _, cx| {
                    if this.start == 1 {
                        this.handoffs += 1;
                        this.handoff = this.ai_card.bounds().map(|from| (this.handoffs, from));
                        this.screen = Screen::Compose;
                        cx.notify();
                    }
                })),
            );

        div()
            .flex()
            .flex_1()
            .min_h_0()
            .child(sidebar)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .bg(theme.colors.surface)
                    .child(div().flex_1().p(theme.spacing.xxl).child(body))
                    .child(footer),
            )
            .into_any_element()
    }

    fn take_card(
        &self,
        index: usize,
        title: &'static str,
        description: &'static str,
        tags: [&'static str; 3],
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let playing = self.playing == Some(index);
        let position = self.positions[index];
        let seconds = (position * 120.).round() as u32;
        let elapsed = format!("{}:{:02}", seconds / 60, seconds % 60);
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap(theme.spacing.md)
            .p(theme.spacing.lg)
            .rounded(theme.shape.radius_lg)
            .bg(theme.colors.elevated)
            .border_1()
            .border_color(if playing {
                theme.colors.accent
            } else {
                theme.colors.border
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme.spacing.xs)
                    // The assistant named the take: its mark stays until the person renames it.
                    .child(AgentLabel::new(title).size(TextSize::Large).tooltip(
                        ("named", index),
                        "AI が付けた名前 — 変えると自分の名前になります",
                    ))
                    .child(div().flex_1())
                    .child(
                        CopyButton::new(("copy-title", index), title).tooltip("タイトルをコピー"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(theme.spacing.xs)
                    .children(tags.map(Badge::new)),
            )
            .child(
                Label::new(description)
                    .size(TextSize::Small)
                    .color(TextColor::Muted),
            )
            .child(
                AudioPlayer::new(("player", index), position, elapsed, "2:00")
                    .peaks(take_peaks(index as f32 * 3.1))
                    .playing(playing)
                    .seek_step(5. / 120.)
                    .on_seek(cx.listener(move |this, to: &f32, _, cx| {
                        this.positions[index] = *to;
                        cx.notify();
                    }))
                    .volume(self.volume)
                    .muted(self.muted)
                    .on_volume(cx.listener(|this, volume: &f32, _, cx| {
                        this.volume = *volume;
                        this.muted = false;
                        cx.notify();
                    }))
                    .on_mute(cx.listener(|this, _, _, cx| {
                        this.muted = !this.muted;
                        cx.notify();
                    }))
                    .on_play(cx.listener(move |this, _, _, cx| {
                        this.playing = if this.playing == Some(index) {
                            None
                        } else {
                            Some(index)
                        };
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(theme.spacing.sm)
                    .child(Button::new(("continue", index), "この曲で続ける").icon(IconName::Check))
                    .child(
                        Button::new(("redo", index), "作り直す")
                            .ghost()
                            .icon(IconName::Refresh),
                    ),
            )
    }

    fn generating_card(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap(theme.spacing.md)
            .p(theme.spacing.lg)
            .rounded(theme.shape.radius_lg)
            .bg(theme.colors.agent_wash)
            .border_1()
            .border_color(theme.colors.agent.opacity(0.35))
            .child(
                Label::new("成層圏突破")
                    .size(TextSize::Large)
                    .heading_weight(),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(theme.spacing.xs)
                    .child(Badge::new("BPM 174"))
                    .child(Badge::new("F#m"))
                    .child(Badge::new("生成中").tone(atelier::Tone::Agent).dot()),
            )
            .child(
                Label::new("電子的なパルスとロックの推進力を融合した、未来的で攻撃的な高速曲。")
                    .size(TextSize::Small)
                    .color(TextColor::Muted),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.spacing.sm)
                    .p(theme.spacing.md)
                    .rounded(theme.shape.radius_md)
                    .bg(theme.colors.surface)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(theme.spacing.sm)
                            .child(Spinner::new("generating"))
                            .child(Label::new("編曲を進めています").size(TextSize::Small))
                            .child(div().flex_1())
                            .child(
                                Label::new("約40秒")
                                    .size(TextSize::XSmall)
                                    .color(TextColor::Faint),
                            ),
                    )
                    .child(ProgressBar::indeterminate("generating-bar").tone(atelier::Tone::Agent))
                    .child(
                        StepList::new("generation")
                            .step("メロディと和声", StepState::Done)
                            .step("編曲", StepState::Running)
                            .step("ミックスと書き出し", StepState::Pending),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(theme.spacing.sm)
                    .child(
                        Button::new("continue-3", "この曲で続ける")
                            .icon(IconName::Check)
                            .disabled(true),
                    )
                    .child(
                        Button::new("stop-3", "生成を停止")
                            .ghost()
                            .icon(IconName::Stop),
                    ),
            )
    }

    /// The effort button in the composer row, and the picker it opens above it.
    fn effort_control(&self, cx: &mut Context<Self>) -> AnyElement {
        const LEVELS: [&str; 4] = ["低", "中", "高", "最高"];
        let button = EffortButton::new("effort", LEVELS[self.effort])
            .fast(self.fast)
            .open(self.effort_open)
            .on_click(cx.listener(|this, _, _, cx| {
                this.effort_open = !this.effort_open;
                cx.notify();
            }));
        let picker = self.effort_open.then(|| {
            let button = self.effort_button.clone();
            let picker = EffortPicker::new("effort-picker", LEVELS, self.effort)
                .default_level(1)
                .fast(self.fast)
                .model("GPT-6-Sol (Codex)")
                .on_select(cx.listener(|this, index: &usize, _, cx| {
                    this.effort = *index;
                    cx.notify();
                }))
                .on_fast(cx.listener(|this, _, _, cx| {
                    this.fast = !this.fast;
                    cx.notify();
                }))
                .on_reset(cx.listener(|this, _, _, cx| {
                    this.effort = 1;
                    this.fast = false;
                    cx.notify();
                }));
            // Over the composer's top edge, lined up with the button; a press anywhere else
            // closes it. The positions are the last frame's, which the open card keeps fresh.
            let at = match (self.effort_button.bounds(), self.composer.bounds()) {
                (Some(button), Some(composer)) => {
                    point(button.origin.x - px(12.), composer.origin.y - px(8.))
                }
                _ => point(px(0.), px(0.)),
            };
            deferred(
                anchored()
                    .position(at)
                    .anchor(Corner::BottomLeft)
                    .snap_to_window_with_margin(px(8.))
                    .child(div().occlude().child(picker).on_mouse_down_out(cx.listener(
                        move |this, event: &MouseDownEvent, _, cx| {
                            if !button.bounds().is_some_and(|b| b.contains(&event.position)) {
                                this.effort_open = false;
                                cx.notify();
                            }
                        },
                    ))),
            )
        });
        div()
            .relative()
            .child(self.effort_button.probe())
            .child(button)
            .children(picker)
            .into_any_element()
    }

    fn compose(&self, cx: &mut Context<Self>) -> AnyElement {
        let effort = self.effort_control(cx);
        let theme = cx.theme();
        let transcript = div()
            .flex()
            .flex_col()
            .gap(theme.spacing.xl)
            .w_full()
            .max_w(px(1040.))
            .child(
                UserMessage::new(REQUEST)
                    .id("request")
                    .action(CopyButton::new("copy-request", REQUEST)),
            )
            .child(
                AssistantMessage::new(REPLY)
                    .id("reply")
                    .action(CopyButton::new("copy-reply", REPLY))
                    // Two voices only: the bubble is the person, the prose is the assistant.
                    .anonymous()
                    .meta("GPT-6-Sol (Codex) · 2分14秒"),
            )
            .child(
                div()
                    .flex()
                    .gap(theme.spacing.lg)
                    .items_start()
                    .child(self.take_card(
                        0,
                        "蒼穹の迎撃",
                        "鋭いギターと重厚なオーケストラが一気に加速する、正面突破の空戦テーマ。",
                        ["BPM 160", "Dm", "2:00"],
                        cx,
                    ))
                    .child(self.take_card(
                        1,
                        "雲海の決戦",
                        "静かなピアノから始まり、弦とブラスが広がるドラマチックな決戦曲。",
                        ["BPM 148", "Cm", "2:00"],
                        cx,
                    ))
                    .child(self.generating_card(cx)),
            );

        let composer = Composer::new("composer")
            .placeholder("要望を追加する（例：もっとブラスを前に）…")
            .model("GPT-6-Sol (Codex)")
            .model_logo(ProviderLogo::OpenAi)
            .control(effort)
            .busy(self.slash.is_none())
            .usage(
                UsageMeter::new("usage")
                    .limit(UsageLimit::new("5時間枠", 0.88).resets("1時間40分後"))
                    .limit(UsageLimit::new("週間", 0.52).resets("月曜 9:00")),
            );
        let composer = match self.slash {
            Some(selected) => composer.value("/mo").completions(
                Completions::new("slash")
                    .items(COMMANDS.iter().map(|(name, help, matched)| {
                        Completion::new(*name)
                            .description(*help)
                            .matched(matched.to_vec())
                    }))
                    .selected(Some(selected))
                    .on_select(cx.listener(|this, index: &usize, _, cx| {
                        this.slash = Some(*index);
                        cx.notify();
                    })),
            ),
            None => composer,
        };

        let footer = div()
            .flex()
            .items_center()
            .gap(theme.spacing.md)
            .w_full()
            .max_w(px(1040.))
            .child(
                Button::new("back", "戻る")
                    .ghost()
                    .icon(IconName::ChevronLeft),
            )
            .child(
                Switch::new("learn", self.learn_preferences)
                    .label("好みを記録・活用")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.learn_preferences = !this.learn_preferences;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("history", "選択履歴…")
                    .ghost()
                    .icon(IconName::History),
            )
            .child(div().flex_1())
            .child(
                Button::new("open-editor", "エディタで開く")
                    .disabled(true)
                    .tooltip("案を選ぶと開けます"),
            );

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .bg(theme.colors.surface)
            .child(
                div()
                    .id("compose-transcript")
                    .flex()
                    .flex_col()
                    .items_center()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(theme.spacing.xxl)
                    .py(theme.spacing.xl)
                    .child(transcript),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(theme.spacing.md)
                    .px(theme.spacing.xxl)
                    .pb(theme.spacing.lg)
                    .child(
                        div()
                            .relative()
                            .w_full()
                            .max_w(px(1040.))
                            .child(self.composer.probe())
                            .child(composer),
                    )
                    .child(footer),
            )
            .into_any_element()
    }
}

impl Render for AurisScreens {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let switch = common::theme_switch(self, cx);
        let title = match self.screen {
            Screen::Chooser => "プロジェクトを選択",
            Screen::Compose => "AIで曲を作る",
        };
        let body = match (self.screen, self.handoff) {
            (Screen::Chooser, _) => self.chooser(cx),
            (Screen::Compose, None) => self.compose(cx),
            // Only the card's shape travels; the compose screen fades in over it at its final
            // size.
            (Screen::Compose, Some((number, _))) => {
                let screen = div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .child(self.compose(cx));
                appear(screen, ("compose", number), cx, |screen, delta| {
                    screen.opacity(delta)
                })
            }
        };
        let morph =
            self.handoff
                .filter(|_| self.screen == Screen::Compose)
                .map(|(number, from)| {
                    Morph::new(("handoff", number), from)
                        .radius(theme.shape.radius_lg, px(0.))
                        .fill(theme.colors.elevated)
                });
        WindowFrame::new()
            .font(theme.typography.ui.clone())
            .text_color(theme.colors.text)
            .size_full()
            .flex()
            .flex_col()
            .child(
                TitleBar::new()
                    .title(title)
                    .trailing(
                        SegmentedControl::new(
                            "screen",
                            [
                                Segment::label("プロジェクト選択"),
                                Segment::label("AIで曲を作る"),
                            ],
                            self.screen as usize,
                        )
                        .size(ControlSize::Small)
                        .on_select(cx.listener(
                            |this, index: &usize, _, cx| {
                                this.screen = if *index == 0 {
                                    Screen::Chooser
                                } else {
                                    Screen::Compose
                                };
                                cx.notify();
                            },
                        )),
                    )
                    .trailing(Divider::vertical())
                    .trailing(switch),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .children(morph)
                    .child(body),
            )
    }
}

const REQUEST: &str = "Ace Combatみたいな超かっこいい曲作ってほしい";
const REPLY: &str = "空戦の疾走感と英雄的なスケールを軸に、方向性の異なるオリジナル曲を3案作ります。気に入った案を選ぶと、その曲で続けて編集できます。";

/// The slash commands the composer completes, with the letters of `/mo` they match.
const COMMANDS: [(&str, &str, &[usize]); 3] = [
    ("/model", "使うモデルを切り替える", &[1, 2]),
    ("/mood", "曲の雰囲気を指定する", &[1, 2]),
    ("/more", "同じ方向で案を増やす", &[1, 2]),
];

fn main() {
    let screen = match common::argument("--screen").as_deref() {
        Some("compose") => Screen::Compose,
        _ => Screen::Chooser,
    };
    common::run("Auris Studio", 1320., 860., move |choice, cx| {
        // Atelier's own words (tooltips, "Copied", Send) in the application's language.
        atelier::Strings::japanese().install(cx);
        AurisScreens {
            choice,
            screen,
            chooser_page: 0,
            start: 1,
            playing: Some(0),
            learn_preferences: true,
            effort: 1,
            fast: false,
            positions: [0.76, 0.],
            volume: 0.8,
            muted: false,
            slash: common::argument("--slash").map(|_| 0),
            effort_open: std::env::args().any(|arg| arg == "--effort"),
            effort_button: Measure::new(),
            composer: Measure::new(),
            ai_card: Measure::new(),
            handoff: None,
            handoffs: 0,
        }
    });
}
