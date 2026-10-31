//! A legend of what a person handles and the control that handles it: turning something on or
//! off, choosing one of several, setting and reporting an amount, entering text, doing
//! something, showing a state, and what Atelier has no component for yet. Each row names the
//! concept, draws the control live, and names the neighbour that is easy to pick by mistake.
//! The same table, for agents, is `skills/atelier-gpui/references/choosing.md`.
//!
//! ```text
//! cargo run --example control_legend -- [--page toggle|choose|amount|report|text|act|state|missing]
//!                                        [--effort 0..3] [--menu]
//!                                        [--direction glass|studio|mono] [--appearance dark|light]
//! ```

mod common;

use atelier::agent::{
    AgentLabel, ApprovalCard, Composer, EffortButton, EffortPicker, UsageLimit, UsageMeter,
};
use atelier::{
    ActiveTheme, Appearance, AudioPlayer, Badge, Button, ButtonVariant, Checkbox, Completion,
    Completions, ControlSize, CopyButton, Direction, Disclosure, Divider, Edge, EmptyState,
    IconName, Kbd, Knob, Label, LevelMeter, ListItem, MenuItem, OptionCard, Popover, ProgressBar,
    ProgressRing, ProviderLogo, SectionHeader, Segment, SegmentedControl, SelectableText, Sidebar,
    Skeleton, Slider, Spinner, StatusBar, StatusDot, StepList, StepState, Switch, Tabs, TextColor,
    TextField, TextSize, TitleBar, Tone, WindowFrame, menu_separator,
};
use common::Themed;
use gpui::{
    AnyElement, Context, IntoElement, ParentElement, Pixels, Render, Styled, Window, deferred, div,
    prelude::*, px,
};

/// One page of the legend: a family of concepts.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Toggle,
    Choose,
    Amount,
    Report,
    Text,
    Act,
    State,
    Missing,
}

impl Page {
    const ALL: [Self; 8] = [
        Self::Toggle,
        Self::Choose,
        Self::Amount,
        Self::Report,
        Self::Text,
        Self::Act,
        Self::State,
        Self::Missing,
    ];

    fn title(self) -> &'static str {
        match self {
            Self::Toggle => "オン・オフ",
            Self::Choose => "ひとつを選ぶ",
            Self::Amount => "量を決める",
            Self::Report => "量を見せる",
            Self::Text => "文字を入れる",
            Self::Act => "実行する",
            Self::State => "状態を見せる",
            Self::Missing => "まだない部品",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Toggle => "ずっと残る真偽の値。すぐ効くか、保存で確定するかで分ける",
            Self::Choose => "決まった選択肢からひとつ。数と、全部を見せる必要があるかで分ける",
            Self::Amount => "連続した値。行の広さと、正確な数を打つかで分ける",
            Self::Report => "人は動かさず、アプリが知らせる量",
            Self::Text => "人が打つ文字と、読んで写す文字",
            Self::Act => "一度だけ何かをする命令。値ではない",
            Self::State => "読むだけの状態。色だけにしない",
            Self::Missing => "Atelier にまだ部品がない概念と、当面の代わり",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Self::Toggle => IconName::Check,
            Self::Choose => IconName::ChevronDown,
            Self::Amount => IconName::Sliders,
            Self::Report => IconName::Waveform,
            Self::Text => IconName::Notes,
            Self::Act => IconName::Play,
            Self::State => IconName::Dot,
            Self::Missing => IconName::Wrench,
        }
    }

    fn entries(self) -> &'static [Entry] {
        match self {
            Self::Toggle => &TOGGLE,
            Self::Choose => &CHOOSE,
            Self::Amount => &AMOUNT,
            Self::Report => &REPORT,
            Self::Text => &TEXT,
            Self::Act => &ACT,
            Self::State => &STATE,
            Self::Missing => &MISSING,
        }
    }
}

/// One row: a concept, its everyday examples, the control to use and the one to avoid.
struct Entry {
    sample: Sample,
    concept: &'static str,
    examples: &'static str,
    usage: &'static str,
    avoid: &'static str,
}

/// The live control drawn in a row.
#[derive(Clone, Copy)]
enum Sample {
    Switch,
    Checkbox,
    Checkboxes,
    Latch,
    MenuToggle,
    Disclosure,
    Segmented,
    MenuButton,
    Completions,
    OptionCards,
    Tabs,
    Places,
    Slider,
    Knobs,
    Number,
    Player,
    Effort,
    Progress,
    Waiting,
    Level,
    Usage,
    Steps,
    Field,
    Search,
    Path,
    Composer,
    Selectable,
    Primary,
    Secondary,
    Toolbar,
    Ellipsis,
    Danger,
    Agent,
    Menu,
    Copy,
    Approval,
    Confirm,
    Badges,
    Status,
    AgentMarks,
    Categories,
    Empty,
    Keys,
    None,
}

const TOGGLE: [Entry; 6] = [
    Entry {
        sample: Sample::Switch,
        concept: "すぐ反映される設定",
        examples: "自動保存、グリッドにスナップ",
        usage: "Switch（フォームでは FormField::beside）",
        avoid: "Checkbox — 保存を待っているように読める",
    },
    Entry {
        sample: Sample::Checkbox,
        concept: "保存でまとめて確定する yes / no",
        examples: "GPUを使う、カウントイン",
        usage: "Checkbox（FormField::beside）",
        avoid: "Switch のあとに保存ボタン",
    },
    Entry {
        sample: Sample::Checkboxes,
        concept: "ひとつの集合から、いくつでも",
        examples: "書き出す内容：ステム、MIDI、歌詞",
        usage: "Checkbox を縦に並べ、FormField / FormGroup で束ねる",
        avoid: "SegmentedControl — ひとつしか選べない",
    },
    Entry {
        sample: Sample::Latch,
        concept: "密な列にある、ひとつの物の状態",
        examples: "ミュート、ソロ、録音待機、ループ、再生",
        usage: "Button::selected(..).tint(..) のラッチ",
        avoid: "Switch — 幅を取り、設定に見える",
    },
    Entry {
        sample: Sample::MenuToggle,
        concept: "メニューの中の切り替え",
        examples: "クリップのメニューの「グリッドにスナップ」",
        usage: "MenuItem::checked(..)",
        avoid: "ポップオーバーに置いた Switch",
    },
    Entry {
        sample: Sample::Disclosure,
        concept: "詳しい内容の表示・非表示",
        examples: "オートメーション、センド、プラグイン",
        usage: "Disclosure",
        avoid: "Switch — 何も変えず、見せるだけだから",
    },
];

const CHOOSE: [Entry; 6] = [
    Entry {
        sample: Sample::Segmented,
        concept: "2〜5個の短い選択肢を全部見せる",
        examples: "言語、エンジン、編集ツール",
        usage: "SegmentedControl",
        avoid: "Tabs — タブは値ではなく見え方を変える",
    },
    Entry {
        sample: Sample::MenuButton,
        concept: "6個以上、名前が長い、ロゴが付く",
        examples: "モデル、グリッド、サンプルレート",
        usage: "Button::icon_after(ChevronDown) で開く Popover と MenuItem::checked",
        avoid: "長い SegmentedControl、ラジオ風のチェックボックス",
    },
    Entry {
        sample: Sample::Completions,
        concept: "打って探す、何百もの候補",
        examples: "絵文字、コマンド、音色",
        usage: "TextField と、その下（上）の Completions",
        avoid: "いつまでもスクロールするメニュー",
    },
    Entry {
        sample: Sample::OptionCards,
        concept: "選ぶこと自体が画面の目的で、一文ずつ説明がいる",
        examples: "新規プロジェクトの始め方、AIに任せるか自分でやるか",
        usage: "OptionCard を並べる（AIに渡すものは .agent()）",
        avoid: "ツールチップ付きの SegmentedControl",
    },
    Entry {
        sample: Sample::Tabs,
        concept: "同じものの別の見方",
        examples: "楽譜 / ピアノロール / オーディオ",
        usage: "Tabs",
        avoid: "フォームの中の SegmentedControl",
    },
    Entry {
        sample: Sample::Places,
        concept: "いる場所、ページ、文書",
        examples: "設定のページ、トラック、ファイルの木",
        usage: "Sidebar に ListItem::selected(..)（木は .indent(..)）",
        avoid: "十数個の Tabs",
    },
];

const AMOUNT: [Entry; 5] = [
    Entry {
        sample: Sample::Slider,
        concept: "ラベルと読み値が一行に並ぶ連続値",
        examples: "音量、テンポ、強さ、パン（中央から ±）",
        usage: "Slider（.label .readout、パンは .bipolar()）",
        avoid: "広い行に置いた Knob",
    },
    Entry {
        sample: Sample::Knobs,
        concept: "詰めて並ぶたくさんの音色パラメータ",
        examples: "ドライブ、トーン、ディケイ",
        usage: "Knob（± は .bipolar()）",
        avoid: "インスペクタに積んだスライダーの山",
    },
    Entry {
        sample: Sample::Number,
        concept: "人が打ち込む正確な数",
        examples: "BPM 128.00、ポート番号",
        usage: "TextField に等幅の数、単位は .trailing(..)",
        avoid: "読み値に打ち込めないスライダー",
    },
    Entry {
        sample: Sample::Player,
        concept: "音の中の位置と、その音量",
        examples: "テイクの試聴、プレビュー",
        usage: "AudioPlayer（.on_seek .on_volume）",
        avoid: "再生ボタンの横に置いた Slider",
    },
    Entry {
        sample: Sample::Effort,
        concept: "AIがかける手間",
        examples: "推論の深さ、高速モード",
        usage: "EffortButton で開く EffortPicker（稲妻が高速モード、段階のスライダー、リセット）",
        avoid: "汎用の Slider、カードの中のモデル切り替え（モデルは別のボタン）",
    },
];

const REPORT: [Entry; 5] = [
    Entry {
        sample: Sample::Progress,
        concept: "総量のわかる仕事の進み",
        examples: "書き出し、ダウンロード",
        usage: "ProgressBar / ProgressRing、名前のある長い仕事は TaskProgress",
        avoid: "Slider — 動かせそうに見える",
    },
    Entry {
        sample: Sample::Waiting,
        concept: "長さのわからない待ち",
        examples: "接続、読み込み、これから届く内容",
        usage: "ProgressBar::indeterminate、Spinner、届く場所に Skeleton",
        avoid: "90 % で止まるバー",
    },
    Entry {
        sample: Sample::Level,
        concept: "生きている信号の大きさ",
        examples: "トラックのレベル、入力",
        usage: "LevelMeter",
        avoid: "ProgressBar",
    },
    Entry {
        sample: Sample::Usage,
        concept: "割り当て・残量",
        examples: "5時間枠、週の上限",
        usage: "agent::UsageMeter",
        avoid: "パーセントだけのラベル",
    },
    Entry {
        sample: Sample::Steps,
        concept: "計画の段階",
        examples: "AIの作業計画、セットアップ",
        usage: "StepList",
        avoid: "番号を振ったラベルの列",
    },
];

const TEXT: [Entry; 5] = [
    Entry {
        sample: Sample::Field,
        concept: "短い一行",
        examples: "プロジェクト名、アドレス",
        usage: "FormField のラベルの下に TextField（編集はアプリが持つ）",
        avoid: "プレースホルダーをラベル代わりにする",
    },
    Entry {
        sample: Sample::Search,
        concept: "探す・絞り込む",
        examples: "音色、設定",
        usage: "TextField::icon(Search) を絞る一覧の上に",
        avoid: "「検索」ボタン",
    },
    Entry {
        sample: Sample::Path,
        concept: "ファイルやフォルダ",
        examples: "保存先、モデルの置き場所",
        usage: "パスを見せる TextField と .trailing(「選択…」)",
        avoid: "今のパスを隠すボタンだけ",
    },
    Entry {
        sample: Sample::Composer,
        concept: "AIへの依頼",
        examples: "「もっとブラスを前に」",
        usage: "agent::Composer",
        avoid: "TextField の横に送信ボタン",
    },
    Entry {
        sample: Sample::Selectable,
        concept: "読んで写す文字",
        examples: "メッセージ、ログ、エラーの詳細",
        usage: "SelectableText と CopyButton",
        avoid: "読み取り専用にした TextField",
    },
];

const ACT: [Entry; 9] = [
    Entry {
        sample: Sample::Primary,
        concept: "決定の場のいつもの確定",
        examples: "保存、書き出し…、作成",
        usage: "Button::primary() — 場にひとつ、Enter の動作",
        avoid: "「大事だから」ふたつめの primary",
    },
    Entry {
        sample: Sample::Secondary,
        concept: "ふつうの命令",
        examples: "その場でバウンス、複製",
        usage: "Button（Secondary）",
        avoid: "primary",
    },
    Entry {
        sample: Sample::Toolbar,
        concept: "ツールバーのよく使う命令",
        examples: "元に戻す、設定、サイドバー",
        usage: "Button::ghost()、または icon_only(..).tooltip(..)",
        avoid: "ツールチップのないアイコン",
    },
    Entry {
        sample: Sample::Ellipsis,
        concept: "まだ入力が要る命令",
        examples: "書き出し…、選択…",
        usage: "ラベルの最後に「…」",
        avoid: "すぐ実行する命令に「…」",
    },
    Entry {
        sample: Sample::Danger,
        concept: "壊す・捨てる命令",
        examples: "トラックを削除",
        usage: "ButtonVariant::Danger / MenuItem::danger()、動詞と対象で名付ける",
        avoid: "「よろしいですか？」に OK",
    },
    Entry {
        sample: Sample::Agent,
        concept: "AIに仕事を渡す",
        examples: "アシスタントに頼む、AIで曲を作る…",
        usage: "ButtonVariant::Agent と Sparkles",
        avoid: "それ以外へのAIの色",
    },
    Entry {
        sample: Sample::Menu,
        concept: "物に対する二次的な命令",
        examples: "切り取り、複製、削除",
        usage: "見えるメニューボタンかコンテキストメニュー（Popover と MenuItem）",
        avoid: "ホバーでだけ出るアイコン列（会話のメッセージ操作だけは例外）",
    },
    Entry {
        sample: Sample::Copy,
        concept: "写す",
        examples: "タイトル、コード、メッセージ",
        usage: "CopyButton",
        avoid: "黙って写す Button",
    },
    Entry {
        sample: Sample::Approval,
        concept: "AIが提案した変更",
        examples: "トラックの作成、ファイルの削除",
        usage: "agent::ApprovalCard、変わった物そのものに残す / 取り消す",
        avoid: "「元に戻す」だけのトースト",
    },
];

const STATE: [Entry; 6] = [
    Entry {
        sample: Sample::Badges,
        concept: "ひとつの物の状態",
        examples: "書き出し済み、クリップ、下書き",
        usage: "Badge::tone(..).dot() と言葉",
        avoid: "色だけ（言葉のない StatusDot）",
    },
    Entry {
        sample: Sample::Status,
        concept: "直前の操作の結果",
        examples: "「保存しました」「モデルを2つダウンロードしました」",
        usage: "ウインドウ下端の StatusBar",
        avoid: "モーダル、質問をする StatusBar",
    },
    Entry {
        sample: Sample::AgentMarks,
        concept: "AIが作った・書いたもの",
        examples: "AIの下書き、AIが付けた説明",
        usage: "Badge（Tone::Agent）、AgentLabel、ListItem::description_agent",
        avoid: "飾りとしてのAIの色",
    },
    Entry {
        sample: Sample::Categories,
        concept: "画面をまたぐ分類",
        examples: "音のクラスタ、タグ",
        usage: "theme.categorical(index)、ひとつの物にずっと同じ番号、名前も添える",
        avoid: "画面ごとに新しい色",
    },
    Entry {
        sample: Sample::Empty,
        concept: "まだ何もない",
        examples: "クリップのないトラック、空の検索結果",
        usage: "EmptyState と、ひとつの行動",
        avoid: "空っぽのパネル",
    },
    Entry {
        sample: Sample::Keys,
        concept: "それをするキー",
        examples: "Ctrl+B、Space",
        usage: "Kbd",
        avoid: "地の文に書いたショートカット",
    },
];

const MISSING: [Entry; 7] = [
    Entry {
        sample: Sample::Confirm,
        concept: "確認のダイアログ",
        examples: "保存せずに閉じる、まとめて削除",
        usage: "ApprovalCard の言い方（動詞と対象、確定は Danger）",
        avoid: "Card と影を手で組んだダイアログ",
    },
    Entry {
        sample: Sample::MenuButton,
        concept: "ドロップダウンのセレクト",
        examples: "モデル、サンプルレート",
        usage: "メニューボタン（Button + ChevronDown + Popover の MenuItem::checked）",
        avoid: "ネイティブ風のセレクトを描き起こす",
    },
    Entry {
        sample: Sample::Segmented,
        concept: "ラジオボタンの群れ",
        examples: "書き出し形式",
        usage: "少なければ SegmentedControl、言葉が要れば OptionCard",
        avoid: "丸いチェックボックスを自作する",
    },
    Entry {
        sample: Sample::None,
        concept: "複数行のテキストエリア",
        examples: "歌詞、メモ",
        usage: "TextField の枠に、アプリのエディタを入れる",
        avoid: "TextField を縦に伸ばすだけ",
    },
    Entry {
        sample: Sample::Number,
        concept: "増減ボタン付きの数値欄",
        examples: "小節数、ボイス数",
        usage: "数を入れた TextField、単位を後ろに",
        avoid: "＋／− ボタンの手作り",
    },
    Entry {
        sample: Sample::Places,
        concept: "ドラッグで並べ替える一覧",
        examples: "トラック順、プレイリスト",
        usage: "ListItem にアプリのドラッグと「上へ / 下へ」のメニュー",
        avoid: "並べ替えの手がかりがない一覧",
    },
    Entry {
        sample: Sample::None,
        concept: "色・日付・時刻のピッカー",
        examples: "トラックの色、締め切り",
        usage: "アプリ自身のものをトークンで。Atelier への追加を頼む",
        avoid: "リテラルの色で描いたピッカー",
    },
];

/// The width of the page list.
const SIDEBAR: Pixels = px(220.);
/// The widths of a row's columns: the concept, its control, and the least the notes need.
const CONCEPT: Pixels = px(220.);
const SAMPLE: Pixels = px(340.);
const NOTES_MIN: Pixels = px(300.);

struct Legend {
    choice: (Direction, Appearance),
    page: Page,
    autosave: bool,
    gpu: bool,
    exports: [bool; 3],
    latches: [bool; 3],
    snap: bool,
    disclosure: bool,
    engine: usize,
    model: usize,
    model_menu: bool,
    start: usize,
    tab: usize,
    place: usize,
    effort: usize,
    fast: bool,
}

impl Themed for Legend {
    fn choice(&mut self) -> &mut (Direction, Appearance) {
        &mut self.choice
    }
}

/// The effort levels, lowest first.
const EFFORT: [&str; 4] = ["軽", "中", "高", "最大"];

const MODELS: [(&str, ProviderLogo); 5] = [
    ("Claude Opus", ProviderLogo::Anthropic),
    ("Claude Sonnet", ProviderLogo::Anthropic),
    ("GPT-6-Sol", ProviderLogo::OpenAi),
    ("Gemini Pro", ProviderLogo::Google),
    ("ローカル（Qwen）", ProviderLogo::Local),
];

impl Legend {
    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let pages = Page::ALL.map(|page| {
            ListItem::new(page.title(), page.title())
                .icon(page.icon())
                .selected(self.page == page)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.page = page;
                    this.model_menu = false;
                    cx.notify();
                }))
        });
        Sidebar::new(Edge::Left)
            .id("pages")
            .overflow_y_scroll()
            .w(SIDEBAR)
            .p(theme.spacing.sm)
            .gap(theme.spacing.xxs)
            .children(pages)
    }

    fn sample(&self, sample: Sample, row: usize, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let column = || div().flex().flex_col().gap(theme.spacing.sm);
        let line = || {
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(theme.spacing.sm)
        };
        match sample {
            Sample::Switch => Switch::new("autosave", self.autosave)
                .label("自動保存")
                .on_change(cx.listener(|this, on: &bool, _, cx| {
                    this.autosave = *on;
                    cx.notify();
                }))
                .into_any_element(),
            Sample::Checkbox => Checkbox::new("gpu", self.gpu)
                .label("GPUを使う")
                .on_change(cx.listener(|this, on: &bool, _, cx| {
                    this.gpu = *on;
                    cx.notify();
                }))
                .into_any_element(),
            Sample::Checkboxes => column()
                .children(["ステム", "MIDI", "歌詞"].into_iter().enumerate().map(
                    |(index, label)| {
                        Checkbox::new(("export", index), self.exports[index])
                            .label(label)
                            .on_change(cx.listener(move |this, on: &bool, _, cx| {
                                this.exports[index] = *on;
                                cx.notify();
                            }))
                    },
                ))
                .into_any_element(),
            Sample::Latch => {
                let tints = [theme.colors.info, theme.colors.warning, theme.colors.record];
                line()
                    .children(
                        ["M", "S", "R"]
                            .into_iter()
                            .enumerate()
                            .map(|(index, label)| {
                                Button::new(("latch", index), label)
                                    .size(ControlSize::Small)
                                    .selected(self.latches[index])
                                    .tint(tints[index])
                                    .tooltip(["ミュート", "ソロ", "録音待機"][index])
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.latches[index] = !this.latches[index];
                                        cx.notify();
                                    }))
                            }),
                    )
                    .into_any_element()
            }
            Sample::MenuToggle => Popover::new()
                .w(px(220.))
                .child(MenuItem::new("mt-dup", "複製").shortcut("Ctrl+D"))
                .child(
                    MenuItem::new("mt-snap", "グリッドにスナップ")
                        .checked(self.snap)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.snap = !this.snap;
                            cx.notify();
                        })),
                )
                .into_any_element(),
            Sample::Disclosure => {
                Disclosure::new("disclosure", "オートメーション", self.disclosure)
                    .on_toggle(cx.listener(|this, _, _, cx| {
                        this.disclosure = !this.disclosure;
                        cx.notify();
                    }))
                    .into_any_element()
            }
            Sample::Segmented => SegmentedControl::new(
                ("engine", row),
                [
                    Segment::label("内蔵"),
                    Segment::label("ローカル"),
                    Segment::label("クラウド"),
                ],
                self.engine,
            )
            .size(ControlSize::Small)
            .on_select(cx.listener(|this, index: &usize, _, cx| {
                this.engine = *index;
                cx.notify();
            }))
            .into_any_element(),
            Sample::MenuButton => {
                let (name, logo) = MODELS[self.model];
                let mut menu = Popover::new().id(("model-menu", row)).w(px(240.));
                let mut provider = None;
                for (index, (name, logo)) in MODELS.into_iter().enumerate() {
                    if provider != Some(logo) {
                        provider = Some(logo);
                        menu = menu.child(
                            div()
                                .px(theme.spacing.sm)
                                .pt(if index == 0 {
                                    theme.spacing.xs
                                } else {
                                    theme.spacing.sm
                                })
                                .pb(theme.spacing.xxs)
                                .child(Label::caption(logo.label())),
                        );
                    }
                    menu = menu.child(
                        MenuItem::new(("model", index), name)
                            .icon(logo)
                            .checked(index == self.model)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.model = index;
                                this.model_menu = false;
                                cx.notify();
                            })),
                    );
                }
                let open = self.model_menu;
                div()
                    .relative()
                    .child(
                        Button::new(("model-button", row), name)
                            .icon(logo)
                            .icon_after(IconName::ChevronDown)
                            .size(ControlSize::Small)
                            .selected(open)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.model_menu = !this.model_menu;
                                cx.notify();
                            })),
                    )
                    // The menu floats over the rows below it rather than pushing them down.
                    .children(open.then(|| {
                        deferred(
                            div()
                                .absolute()
                                .top(theme.density.control_sm + theme.spacing.xs)
                                .left_0()
                                .child(menu),
                        )
                        .with_priority(1)
                    }))
                    .into_any_element()
            }
            Sample::Completions => column()
                .w_full()
                .child(
                    TextField::new()
                        .id("command")
                        .value("/mo")
                        .focused(true)
                        .size(ControlSize::Small),
                )
                .child(
                    Completions::new("commands")
                        .items([
                            Completion::new("/model")
                                .description("モデルを切り替える")
                                .matched(vec![0, 1, 2]),
                            Completion::new("/mood")
                                .description("曲の雰囲気を伝える")
                                .matched(vec![0, 1, 2]),
                            Completion::new("/mode")
                                .description("作曲モードを選ぶ")
                                .matched(vec![0, 1, 2]),
                        ])
                        .selected(Some(0))
                        .rows(3),
                )
                .into_any_element(),
            Sample::OptionCards => column()
                .w_full()
                .children(
                    [
                        (
                            IconName::Music,
                            "空のプロジェクト",
                            "自由に作り始めます。",
                            false,
                        ),
                        (
                            IconName::Sparkles,
                            "AIで曲を作る…",
                            "要望から下書きを作ります。",
                            true,
                        ),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(index, (icon, title, description, agent))| {
                        let card = OptionCard::new(("start", index), icon, title)
                            .description(description)
                            .selected(self.start == index)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.start = index;
                                cx.notify();
                            }));
                        if agent { card.agent() } else { card }
                    }),
                )
                .into_any_element(),
            Sample::Tabs => Tabs::new("views", ["楽譜", "ピアノロール", "オーディオ"], self.tab)
                .on_select(cx.listener(|this, index: &usize, _, cx| {
                    this.tab = *index;
                    cx.notify();
                }))
                .into_any_element(),
            Sample::Places => column()
                .w_full()
                .gap(theme.spacing.xxs)
                .children(
                    [
                        (IconName::Folder, "Midnight Drive", 0),
                        (IconName::Music, "Strings", 1),
                        (IconName::Music, "Pad swell", 1),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(index, (icon, label, depth))| {
                        ListItem::new(("place", row * 10 + index), label)
                            .icon(icon)
                            .indent(depth)
                            .selected(self.place == index)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.place = index;
                                cx.notify();
                            }))
                    }),
                )
                .into_any_element(),
            Sample::Slider => column()
                .w_full()
                .gap(theme.spacing.md)
                .child(Slider::new(0.72).label("音量").readout("-3.2 dB"))
                .child(Slider::new(0.4).bipolar().label("パン").readout("L20"))
                .into_any_element(),
            Sample::Knobs => line()
                .gap(theme.spacing.lg)
                .child(Knob::new(0.8).label("ドライブ").readout("80 %"))
                .child(Knob::new(0.35).label("トーン").readout("2.4k"))
                .child(Knob::new(0.62).bipolar().label("チューン").readout("+12"))
                .into_any_element(),
            Sample::Number => div()
                .w(px(180.))
                .child(
                    TextField::new()
                        .id(("number", row))
                        .value("128.00")
                        .size(ControlSize::Small)
                        .trailing(
                            Label::new("BPM")
                                .size(TextSize::Small)
                                .color(TextColor::Muted),
                        ),
                )
                .into_any_element(),
            Sample::Player => div()
                .w_full()
                .child(
                    AudioPlayer::new("player", 0.38, "0:46", "2:00")
                        .peaks(
                            (0..96)
                                .map(|i| {
                                    let x = i as f32 * 0.21;
                                    0.25 + 0.6 * (x.sin() * (x * 0.37).cos()).abs()
                                })
                                .collect(),
                        )
                        .volume(0.8),
                )
                .into_any_element(),
            // The effort card as it opens over the composer, with its button and, beside it,
            // the model's own button: switching the model is a separate choice.
            Sample::Effort => column()
                .child(
                    EffortPicker::new("effort-picker", EFFORT, self.effort)
                        .width(SAMPLE)
                        .default_level(1)
                        .fast(self.fast)
                        .on_select(cx.listener(|this, level: &usize, _, cx| {
                            this.effort = *level;
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
                        })),
                )
                .child(
                    line()
                        .child(
                            Button::new("effort-model", MODELS[self.model].0)
                                .ghost()
                                .icon(MODELS[self.model].1)
                                .icon_after(IconName::ChevronDown)
                                .size(ControlSize::Small),
                        )
                        .child(
                            EffortButton::new("effort", EFFORT[self.effort])
                                .fast(self.fast)
                                .open(true),
                        ),
                )
                .into_any_element(),
            Sample::Progress => line()
                .gap(theme.spacing.md)
                .child(div().w(px(180.)).child(ProgressBar::new(0.42)))
                .child(ProgressRing::new(0.68).show_value())
                .into_any_element(),
            Sample::Waiting => column()
                .child(
                    div()
                        .w(px(220.))
                        .child(ProgressBar::indeterminate("waiting")),
                )
                .child(
                    line()
                        // Not the agent's wait, so not the agent's colour.
                        .child(Spinner::new("spinner").color(theme.colors.text_muted))
                        .child(Skeleton::new("skeleton").w(px(160.)).h(px(12.))),
                )
                .into_any_element(),
            Sample::Level => div()
                .w(px(220.))
                .child(LevelMeter::new(0.74).peak(0.86).horizontal())
                .into_any_element(),
            Sample::Usage => UsageMeter::new("usage")
                .limit(UsageLimit::new("5時間枠", 0.62).resets("1時間40分後"))
                .limit(UsageLimit::new("週", 0.41).resets("月 09:00"))
                .into_any_element(),
            Sample::Steps => StepList::new("steps")
                .step("コード進行を決める", StepState::Done)
                .step("ストリングスを書く", StepState::Running)
                .step("ミックスを整える", StepState::Pending)
                .into_any_element(),
            Sample::Field => div()
                .w(px(240.))
                .child(TextField::new().id("name").placeholder("無題").empty(true))
                .into_any_element(),
            Sample::Search => div()
                .w(px(240.))
                .child(
                    TextField::new()
                        .id("search")
                        .icon(IconName::Search)
                        .placeholder("音色を検索")
                        .empty(true),
                )
                .into_any_element(),
            Sample::Path => div()
                .w(px(300.))
                .child(
                    TextField::new()
                        .id("folder")
                        .value("~/Music/Auris")
                        .trailing(Button::new("choose", "選択…").size(ControlSize::Small)),
                )
                .into_any_element(),
            Sample::Composer => div()
                .w(px(320.))
                .child(
                    Composer::new("composer")
                        .placeholder("要望を書く（例：もっとブラスを前に）…")
                        .model(MODELS[self.model].0)
                        .model_logo(MODELS[self.model].1),
                )
                .into_any_element(),
            Sample::Selectable => line()
                .items_start()
                .child(
                    div().w(px(240.)).child(
                        SelectableText::new(
                            "log",
                            "書き出しに失敗しました: ディスクの空きが足りません",
                        )
                        .size(TextSize::Small),
                    ),
                )
                .child(CopyButton::new(
                    "copy-log",
                    "書き出しに失敗しました: ディスクの空きが足りません",
                ))
                .into_any_element(),
            Sample::Primary => Button::new("save", "保存").primary().into_any_element(),
            Sample::Secondary => line()
                .child(Button::new("bounce", "その場でバウンス"))
                .child(Button::new("dup", "複製"))
                .into_any_element(),
            Sample::Toolbar => line()
                .child(Button::new("undo", "元に戻す").ghost().icon(IconName::Undo))
                .child(
                    Button::icon_only("settings", IconName::Settings)
                        .tooltip("プロジェクトの設定…"),
                )
                .child(
                    Button::icon_only("sidebar", IconName::SidebarLeft).tooltip("サイドバーを隠す"),
                )
                .into_any_element(),
            Sample::Ellipsis => line()
                .child(Button::new("export", "書き出し…"))
                .child(Button::new("render", "書き出す"))
                .into_any_element(),
            Sample::Danger => Button::new("delete", "トラックを削除")
                .variant(ButtonVariant::Danger)
                .icon(IconName::Trash)
                .into_any_element(),
            Sample::Agent => Button::new("ask", "アシスタントに頼む")
                .variant(ButtonVariant::Agent)
                .icon(IconName::Sparkles)
                .into_any_element(),
            Sample::Menu => Popover::new()
                .w(px(220.))
                .child(MenuItem::new("m-cut", "切り取り").shortcut("Ctrl+X"))
                .child(MenuItem::new("m-dup", "複製").shortcut("Ctrl+D"))
                .child(menu_separator(cx))
                .child(
                    MenuItem::new("m-del", "削除")
                        .icon(IconName::Trash)
                        .danger(),
                )
                .into_any_element(),
            Sample::Copy => line()
                .child(CopyButton::new("copy-icon", "Midnight Drive"))
                .child(CopyButton::new("copy-title", "Midnight Drive").label("タイトルをコピー"))
                .into_any_element(),
            Sample::Approval => div()
                .w(px(320.))
                .child(
                    ApprovalCard::new(
                        ("approval", row),
                        "「Strings」に「Pad swell」を作りますか？",
                    )
                    .body("8小節、Cメジャー、ゆっくりした立ち上がり")
                    .allow_label("作る"),
                )
                .into_any_element(),
            Sample::Confirm => div()
                .w(px(320.))
                .child(
                    ApprovalCard::new("confirm", "トラック「Strings」を削除しますか？")
                        .body("クリップ12個とオートメーションも消えます。")
                        .allow_label("削除")
                        .destructive(),
                )
                .into_any_element(),
            Sample::Badges => line()
                .child(Badge::new("下書き"))
                .child(Badge::new("書き出し済み").tone(Tone::Success).dot())
                .child(Badge::new("クリップ").tone(Tone::Danger).dot())
                .into_any_element(),
            Sample::Status => div()
                .w(px(300.))
                .child(StatusBar::new().message("保存しました").tone(Tone::Success))
                .into_any_element(),
            Sample::AgentMarks => column()
                .child(
                    Badge::new("AIの下書き")
                        .tone(Tone::Agent)
                        .icon(IconName::Sparkles),
                )
                .child(AgentLabel::new("明るいピアノのリフ、Cメジャー").size(TextSize::Small))
                .into_any_element(),
            Sample::Categories => line()
                .gap(theme.spacing.md)
                .children(
                    ["打楽器", "ベース", "パッド", "声"]
                        .into_iter()
                        .enumerate()
                        .map(|(index, label)| {
                            div()
                                .flex()
                                .items_center()
                                .gap(theme.spacing.xs)
                                .child(StatusDot::new(theme.categorical(index)))
                                .child(Label::new(label).size(TextSize::Small))
                        }),
                )
                .into_any_element(),
            Sample::Empty => div()
                .w(px(300.))
                .child(
                    EmptyState::new(IconName::Music, "クリップがまだありません")
                        .description("録音するか、ファイルを置いてください。")
                        .action(Button::new("record", "録音").size(ControlSize::Small)),
                )
                .into_any_element(),
            Sample::Keys => line()
                .child(Kbd::new("Ctrl+B"))
                .child(Kbd::new("Space"))
                .into_any_element(),
            Sample::None => Label::new("（見本なし）")
                .size(TextSize::Small)
                .color(TextColor::Faint)
                .into_any_element(),
        }
    }

    fn row(&self, index: usize, entry: &Entry, narrow: bool, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let concept = div()
            .flex()
            .flex_col()
            .gap(theme.spacing.xs)
            .w(CONCEPT)
            .flex_none()
            .child(Label::new(entry.concept).heading_weight())
            .child(
                Label::new(entry.examples)
                    .size(TextSize::Small)
                    .color(TextColor::Muted),
            );
        // Each control keeps its own width, as it would in a form.
        let sample = div()
            .flex()
            .flex_col()
            .items_start()
            .w(SAMPLE)
            .flex_none()
            .min_w_0()
            .child(self.sample(entry.sample, index, cx));
        let notes = div()
            .flex()
            .flex_col()
            .gap(theme.spacing.xs)
            .flex_1()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .gap(theme.spacing.sm)
                    .child(Badge::new("使う").tone(Tone::Success))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Label::new(entry.usage).size(TextSize::Small)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(theme.spacing.sm)
                    .child(Badge::new("避ける"))
                    .child(
                        div().flex_1().min_w_0().child(
                            Label::new(entry.avoid)
                                .size(TextSize::Small)
                                .color(TextColor::Muted),
                        ),
                    ),
            );
        let body = if narrow {
            // Below the split width the notes go under the concept and its control.
            div()
                .flex()
                .flex_col()
                .gap(theme.spacing.md)
                .child(
                    div()
                        .flex()
                        .gap(theme.spacing.lg)
                        .child(concept)
                        .child(sample),
                )
                .child(notes)
        } else {
            div()
                .flex()
                .items_start()
                .gap(theme.spacing.lg)
                .child(concept)
                .child(sample)
                .child(notes)
        };
        div()
            .flex_none()
            .py(theme.spacing.lg)
            .when(index > 0, |row| {
                row.border_t_1().border_color(theme.colors.separator)
            })
            .child(body)
            .into_any_element()
    }
}

impl Render for Legend {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let switch = common::theme_switch(self, cx);
        let width = window.viewport_size().width;
        let show_sidebar = width >= theme.density.sidebar_width;
        // The notes sit beside the concept and its control while the page has room for all three
        // columns, and go under them otherwise.
        let page_width = width - if show_sidebar { SIDEBAR } else { px(0.) };
        let row_width = CONCEPT + SAMPLE + NOTES_MIN + theme.spacing.lg * 2.;
        let narrow = page_width - theme.spacing.xxl * 2. < row_width;
        let rows: Vec<AnyElement> = self
            .page
            .entries()
            .iter()
            .enumerate()
            .map(|(index, entry)| self.row(index, entry, narrow, cx))
            .collect();
        let page = div()
            .id("page")
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .overflow_y_scroll()
            .bg(theme.colors.surface)
            .px(theme.spacing.xxl)
            .py(theme.spacing.xl)
            .gap(theme.spacing.md)
            .child(SectionHeader::new(self.page.title()).description(self.page.description()))
            .child(div().flex().flex_col().children(rows));
        WindowFrame::new()
            .font(theme.typography.ui.clone())
            .text_color(theme.colors.text)
            .size_full()
            .flex()
            .flex_col()
            .child(
                TitleBar::new()
                    .title("操作と部品の凡例")
                    .trailing(Divider::vertical())
                    .trailing(switch),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .when(show_sidebar, |row| row.child(self.sidebar(cx)))
                    .child(page),
            )
    }
}

fn main() {
    let page = match common::argument("--page").as_deref() {
        Some("choose") => Page::Choose,
        Some("amount") => Page::Amount,
        Some("report") => Page::Report,
        Some("text") => Page::Text,
        Some("act") => Page::Act,
        Some("state") => Page::State,
        Some("missing") => Page::Missing,
        _ => Page::Toggle,
    };
    common::run(
        "操作と部品の凡例",
        1280.,
        820.,
        move |choice, cx| {
            // Atelier's own words (tooltips, "Copied", usage) in the legend's language.
            atelier::Strings::japanese().install(cx);
            Legend {
                choice,
                page,
                autosave: true,
                gpu: false,
                exports: [true, true, false],
                latches: [false, true, false],
                snap: true,
                disclosure: false,
                engine: 0,
                model: 0,
                model_menu: common::argument("--menu").is_some(),
                start: 1,
                tab: 0,
                place: 1,
                effort: common::argument("--effort")
                    .and_then(|level| level.parse().ok())
                    .map_or(1, |level: usize| level.min(EFFORT.len() - 1)),
                fast: false,
            }
        },
    );
}
