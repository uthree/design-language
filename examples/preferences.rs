//! A preferences window in the shape Auris Studio's settings take: pages in a sidebar, each page
//! a header, groups of labelled fields in a readable column, its actions at the bottom with the
//! primary at the trailing end, and a status bar that reports what the last action did.
//!
//! Every control is reachable with Tab; the ring shows once the keyboard moves focus.
//!
//! ```text
//! cargo run --example preferences -- [--page general|voice|assistant|keys]
//!                                     [--direction glass|studio|mono] [--appearance dark|light]
//! ```

mod common;

use atelier::{
    ActiveTheme, Appearance, Button, Checkbox, ControlSize, Direction, Divider, Edge, FormField,
    FormGroup, IconName, Kbd, ListItem, ProviderLogo, SectionHeader, Segment, SegmentedControl,
    Sidebar, StatusBar, Switch, TextField, TitleBar, Tone, WindowFrame,
};
use common::Themed;
use gpui::{
    AnyElement, Context, IntoElement, ParentElement, Render, Styled, Window, div, prelude::*, px,
};

/// One page of the window.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    General,
    Voice,
    Assistant,
    Keys,
}

impl Page {
    const ALL: [Self; 4] = [Self::General, Self::Voice, Self::Assistant, Self::Keys];

    fn title(self) -> &'static str {
        match self {
            Self::General => "一般",
            Self::Voice => "歌声合成",
            Self::Assistant => "AIアシスタント",
            Self::Keys => "キー割り当て",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::General => "言語、保存先、起動時の動作",
            Self::Voice => "歌声合成のバックエンドと声のモデルの保存先",
            Self::Assistant => "作曲アシスタントが使うモデルと、好みの記録",
            Self::Keys => "再生や録音などの操作に割り当てるキー",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Self::General => IconName::Settings,
            Self::Voice => IconName::Mic,
            Self::Assistant => IconName::Sparkles,
            Self::Keys => IconName::Keyboard,
        }
    }
}

struct Preferences {
    choice: (Direction, Appearance),
    page: Page,
    backend: usize,
    gpu: bool,
    open_last: bool,
    autosave: bool,
    learn: bool,
    fast: bool,
    saved: bool,
}

impl Themed for Preferences {
    fn choice(&mut self) -> &mut (Direction, Appearance) {
        &mut self.choice
    }
}

impl Preferences {
    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let pages = Page::ALL.map(|page| {
            ListItem::new(page.title(), page.title())
                .icon(page.icon())
                .description(page.description())
                // Japanese runs long without spaces: let the description take two lines.
                .description_lines(2)
                .selected(self.page == page)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.page = page;
                    cx.notify();
                }))
        });
        // A sidebar with an id scrolls when its pages outgrow the window.
        Sidebar::new(Edge::Left)
            .id("pages")
            .overflow_y_scroll()
            .w(px(260.))
            .p(theme.spacing.sm)
            .gap(theme.spacing.xxs)
            .child(
                div().pb(theme.spacing.xs).child(
                    TextField::new()
                        .id("search")
                        .icon(IconName::Search)
                        .placeholder("設定を検索")
                        .size(ControlSize::Small),
                ),
            )
            .children(pages)
            .child(div().flex_1().min_h(theme.spacing.lg))
            .child(
                Button::new("setup", "セットアップガイド…")
                    .ghost()
                    .icon(IconName::Info)
                    .full_width(),
            )
    }

    fn general(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        vec![
            FormGroup::new("プロジェクト")
                .child(
                    FormField::new("保存先")
                        .description("新しいプロジェクトを置くフォルダです。")
                        .child(
                            TextField::new()
                                .id("project-folder")
                                .value("~/Music/Auris")
                                .trailing(Button::new("choose", "選択…").size(ControlSize::Small)),
                        ),
                )
                .child(
                    FormField::new("自動保存")
                        .description("作業中のプロジェクトを1分ごとに保存します。")
                        .beside()
                        .note("保存は数秒おきに行われます")
                        .child(
                            Switch::new("autosave", self.autosave).on_change(cx.listener(
                                |this, on: &bool, _, cx| {
                                    this.autosave = *on;
                                    cx.notify();
                                },
                            )),
                        ),
                )
                .child(
                    FormField::new("起動時に最後のプロジェクトを開く")
                        .beside()
                        .child(
                            Switch::new("open-last", self.open_last).on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.open_last = !this.open_last;
                                    cx.notify();
                                },
                            )),
                        ),
                )
                .into_any_element(),
            FormGroup::new("表示")
                .child(
                    FormField::new("言語").child(
                        SegmentedControl::new(
                            "language",
                            [Segment::label("日本語"), Segment::label("English")],
                            0,
                        )
                        .size(ControlSize::Small),
                    ),
                )
                .into_any_element(),
        ]
    }

    fn voice(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        vec![
            FormGroup::new("バックエンド")
                .description("歌声を作るエンジンです。変更は保存したときに反映されます。")
                .child(
                    FormField::new("エンジン").child(
                        SegmentedControl::new(
                            "backend",
                            [
                                Segment::label("内蔵"),
                                Segment::label("ローカルサーバー"),
                                Segment::label("クラウド"),
                            ],
                            self.backend,
                        )
                        .size(ControlSize::Small)
                        .on_select(cx.listener(|this, index: &usize, _, cx| {
                            this.backend = *index;
                            this.saved = false;
                            cx.notify();
                        })),
                    ),
                )
                .child(
                    FormField::new("サーバーのアドレス")
                        .description("ローカルサーバーを使うときだけ必要です。")
                        .error("このアドレスに接続できません。サーバーが起動しているか確かめてください。")
                        .child(TextField::new().id("server").value("http://localhost:50021")),
                )
                .child(FormField::new("GPUを使う").beside().child(
                    Checkbox::new("gpu", self.gpu).on_click(cx.listener(|this, _, _, cx| {
                        this.gpu = !this.gpu;
                        this.saved = false;
                        cx.notify();
                    })),
                ))
                .into_any_element(),
            FormGroup::new("声のモデル")
                .child(
                    FormField::new("モデルの保存先")
                        .description("ダウンロードした声のモデルを置くフォルダです。")
                        .child(
                            TextField::new()
                                .id("models")
                                .value("~/Auris/Voices")
                                .trailing(Button::new("models-choose", "選択…").size(ControlSize::Small)),
                        ),
                )
                .into_any_element(),
        ]
    }

    fn assistant(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        vec![
            FormGroup::untitled()
                .child(
                    FormField::new("モデル").child(
                        Button::new("model", "GPT-6-Sol (Codex)")
                            .icon(ProviderLogo::OpenAi)
                            .icon_after(IconName::ChevronDown),
                    ),
                )
                .child(
                    FormField::new("高速モード")
                        .description("案を早く出すかわりに、作り込みを浅くします。")
                        .beside()
                        .child(Switch::new("fast", self.fast).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.fast = !this.fast;
                                cx.notify();
                            },
                        ))),
                )
                .child(
                    FormField::new("好みを記録・活用")
                        .description("選んだ案から好みを学び、次の提案に使います。")
                        .beside()
                        .child(Switch::new("learn", self.learn).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.learn = !this.learn;
                                cx.notify();
                            },
                        ))),
                )
                .into_any_element(),
        ]
    }

    fn keys(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let theme = cx.theme();
        let row = |label: &'static str, keys: &'static str| {
            div()
                .flex()
                .items_center()
                .h(theme.density.row)
                .child(div().flex_1().child(label))
                .child(Kbd::new(keys))
        };
        vec![
            FormGroup::new("トランスポート")
                .child(row("再生 / 停止", "Space"))
                .child(row("録音", "R"))
                .child(row("先頭に戻る", "Home"))
                .into_any_element(),
            FormGroup::new("編集")
                .child(row("取り消す", "Ctrl+Z"))
                .child(row("やり直す", "Ctrl+Shift+Z"))
                .into_any_element(),
        ]
    }
}

impl Render for Preferences {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let switch = common::theme_switch(self, cx);
        let groups = match self.page {
            Page::General => self.general(cx),
            Page::Voice => self.voice(cx),
            Page::Assistant => self.assistant(cx),
            Page::Keys => self.keys(cx),
        };
        // Settings that apply at once (the switches) need no Save; the page's Save commits the
        // rest together.
        let actions = div()
            .flex()
            .items_center()
            .gap(theme.spacing.sm)
            .pt(theme.spacing.lg)
            .child(Button::new("defaults", "初期設定に戻す").ghost())
            .child(div().flex_1())
            .child(Button::new("revert", "元に戻す").disabled(self.saved))
            .child(
                Button::new("save", "保存")
                    .primary()
                    .disabled(self.saved)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.saved = true;
                        cx.notify();
                    })),
            );
        let page = div()
            .id("page")
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .overflow_y_scroll()
            .bg(theme.colors.surface)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.spacing.xl)
                    .w_full()
                    .max_w(theme.density.form_width)
                    .px(theme.spacing.xxl)
                    .py(theme.spacing.xl)
                    .child(
                        SectionHeader::new(self.page.title()).description(self.page.description()),
                    )
                    .children(groups)
                    .child(actions),
            );
        let status = if self.saved {
            StatusBar::new()
                .message("設定を保存しました")
                .tone(Tone::Success)
        } else {
            StatusBar::new()
                .message("保存していない変更があります")
                .tone(Tone::Warning)
        };
        WindowFrame::new()
            .font(theme.typography.ui.clone())
            .text_color(theme.colors.text)
            .size_full()
            .flex()
            .flex_col()
            .child(
                TitleBar::new()
                    .title("設定")
                    .trailing(Divider::vertical())
                    .trailing(switch),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.sidebar(cx))
                    .child(page),
            )
            .child(status.trailing("v0.13.0"))
    }
}

fn main() {
    let page = match common::argument("--page").as_deref() {
        Some("voice") => Page::Voice,
        Some("assistant") => Page::Assistant,
        Some("keys") => Page::Keys,
        _ => Page::General,
    };
    common::run("設定", 1100., 760., move |choice, _| Preferences {
        choice,
        page,
        backend: 1,
        gpu: true,
        open_last: true,
        autosave: true,
        learn: true,
        fast: false,
        saved: false,
    });
}
