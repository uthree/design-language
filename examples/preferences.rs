//! A preferences window in the shape Auris Studio's settings take, built on
//! `PreferencesWindow`: pages in a sidebar with a search over every setting, each page a
//! header and groups of labelled fields in a readable column, its actions at the bottom with
//! the primary at the trailing end, a status bar that reports what the last action did, and a
//! key bindings page that records new keys.
//!
//! The search box takes typing (a stand-in for the application's own text input: no caret
//! movement or input method). Every control is reachable with Tab.
//!
//! ```text
//! cargo run --example preferences -- [--page general|voice|assistant|keys] [--search 保存]
//!                                     [--direction glass|studio|mono] [--appearance dark|light]
//! ```

mod common;

use atelier::{
    ActiveTheme, Appearance, Button, Captured, Checkbox, ControlSize, Direction, Divider, IconName,
    KeyBinding, KeybindingList, PreferencesWindow, ProviderLogo, Segment, SegmentedControl,
    Setting, SettingsGroup, SettingsPage, StatusBar, Switch, TextField, TitleBar, Tone,
    WindowFrame, keystroke_label,
};
use common::Themed;
use gpui::{
    Context, FocusHandle, IntoElement, KeyDownEvent, ParentElement, Render, SharedString, Styled,
    Window, div, prelude::*,
};

/// The pages, in sidebar order.
const GENERAL: usize = 0;
const VOICE: usize = 1;
const ASSISTANT: usize = 2;
const KEYS: usize = 3;

/// A command and the keys it starts with.
struct Command {
    group: &'static str,
    label: &'static str,
    scope: Option<&'static str>,
    defaults: &'static [&'static str],
}

const COMMANDS: [Command; 6] = [
    Command {
        group: "トランスポート",
        label: "再生 / 停止",
        scope: None,
        defaults: &["Space"],
    },
    Command {
        group: "トランスポート",
        label: "録音",
        scope: None,
        defaults: &["R"],
    },
    Command {
        group: "トランスポート",
        label: "先頭に戻る",
        scope: None,
        defaults: &["Home", "Enter"],
    },
    Command {
        group: "編集",
        label: "取り消す",
        scope: None,
        defaults: &["Ctrl+Z"],
    },
    Command {
        group: "編集",
        label: "やり直す",
        scope: None,
        defaults: &["Ctrl+Shift+Z"],
    },
    Command {
        group: "編集",
        label: "音符を量子化",
        scope: Some("ピアノロール"),
        defaults: &["Q"],
    },
];

struct Preferences {
    choice: (Direction, Appearance),
    page: usize,
    /// What is typed into the search box.
    query: String,
    search: FocusHandle,
    backend: usize,
    gpu: bool,
    open_last: bool,
    autosave: bool,
    learn: bool,
    fast: bool,
    saved: bool,
    /// Each command's keys, as the platform prints them.
    keys: Vec<Vec<SharedString>>,
    /// The row and slot waiting for a key press.
    recording: Option<(usize, usize)>,
    status: Option<SharedString>,
    /// Focus the search box on the first frame.
    autofocus: bool,
}

impl Themed for Preferences {
    fn choice(&mut self) -> &mut (Direction, Appearance) {
        &mut self.choice
    }
}

impl Preferences {
    /// Typing into the search box: a stand-in for the application's own text input.
    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        if keystroke.modifiers.control || keystroke.modifiers.platform {
            return;
        }
        match keystroke.key.as_str() {
            "backspace" => {
                self.query.pop();
            }
            "escape" => self.query.clear(),
            "tab" | "enter" => return,
            _ => {
                let Some(typed) = keystroke.key_char.as_deref() else {
                    return;
                };
                self.query.push_str(typed);
            }
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn search_field(&self, window: &Window) -> impl IntoElement + use<> {
        let focused = self.search.is_focused(window);
        TextField::new()
            .id("search")
            .icon(IconName::Search)
            .placeholder("設定を検索")
            .size(ControlSize::Small)
            .focused(focused)
            .empty(self.query.is_empty())
            .when(!self.query.is_empty(), |field| {
                field.child(div().child(self.query.clone()))
            })
    }

    fn general(&self, cx: &mut Context<Self>) -> SettingsPage {
        SettingsPage::new("一般")
            .icon(IconName::Settings)
            .description("言語、保存先、起動時の動作")
            .group(
                SettingsGroup::new("プロジェクト")
                    .setting(
                        Setting::new("保存先")
                            .description("新しいプロジェクトを置くフォルダです。")
                            .keywords(["フォルダ", "folder"])
                            .child(
                                TextField::new()
                                    .id("project-folder")
                                    .value("~/Music/Auris")
                                    .trailing(
                                        Button::new("choose", "選択…").size(ControlSize::Small),
                                    ),
                            ),
                    )
                    .setting(
                        Setting::new("自動保存")
                            .description("作業中のプロジェクトを1分ごとに保存します。")
                            .keywords(["autosave"])
                            .beside()
                            .child(
                                Switch::new("autosave", self.autosave).on_change(cx.listener(
                                    |this, on: &bool, _, cx| {
                                        this.autosave = *on;
                                        cx.notify();
                                    },
                                )),
                            ),
                    )
                    .setting(
                        Setting::new("起動時に最後のプロジェクトを開く")
                            .beside()
                            .child(
                                Switch::new("open-last", self.open_last).on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.open_last = !this.open_last;
                                        cx.notify();
                                    },
                                )),
                            ),
                    ),
            )
            .group(
                SettingsGroup::new("表示").setting(
                    Setting::new("言語")
                        .keywords(["language", "English"])
                        .child(
                            SegmentedControl::new(
                                "language",
                                [Segment::label("日本語"), Segment::label("English")],
                                0,
                            )
                            .size(ControlSize::Small),
                        ),
                ),
            )
    }

    fn voice(&self, cx: &mut Context<Self>) -> SettingsPage {
        SettingsPage::new("歌声合成")
            .icon(IconName::Mic)
            .description("歌声合成のバックエンドと声のモデルの保存先")
            .group(
                SettingsGroup::new("バックエンド")
                    .description("歌声を作るエンジンです。変更は保存したときに反映されます。")
                    .setting(
                        Setting::new("エンジン").keywords(["backend", "VOICEVOX"]).child(
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
                    .setting(
                        Setting::new("サーバーのアドレス")
                            .description("ローカルサーバーを使うときだけ必要です。")
                            .error("このアドレスに接続できません。サーバーが起動しているか確かめてください。")
                            .child(TextField::new().id("server").value("http://localhost:50021")),
                    )
                    .setting(Setting::new("GPUを使う").keywords(["GPU", "CUDA"]).beside().child(
                        Checkbox::new("gpu", self.gpu).on_click(cx.listener(|this, _, _, cx| {
                            this.gpu = !this.gpu;
                            this.saved = false;
                            cx.notify();
                        })),
                    )),
            )
            .group(
                SettingsGroup::new("声のモデル").setting(
                    Setting::new("モデルの保存先")
                        .description("ダウンロードした声のモデルを置くフォルダです。")
                        .child(
                            TextField::new()
                                .id("models")
                                .value("~/Auris/Voices")
                                .trailing(
                                    Button::new("models-choose", "選択…").size(ControlSize::Small),
                                ),
                        ),
                ),
            )
    }

    fn assistant(&self, cx: &mut Context<Self>) -> SettingsPage {
        SettingsPage::new("AIアシスタント")
            .icon(IconName::Sparkles)
            .description("作曲アシスタントが使うモデルと、好みの記録")
            .group(
                SettingsGroup::untitled()
                    .setting(
                        Setting::new("モデル").keywords(["model", "OpenAI"]).child(
                            Button::new("model", "GPT-6-Sol (Codex)")
                                .icon(ProviderLogo::OpenAi)
                                .icon_after(IconName::ChevronDown),
                        ),
                    )
                    .setting(
                        Setting::new("高速モード")
                            .description("案を早く出すかわりに、作り込みを浅くします。")
                            .keywords(["fast"])
                            .beside()
                            .child(Switch::new("fast", self.fast).on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.fast = !this.fast;
                                    cx.notify();
                                },
                            ))),
                    )
                    .setting(
                        Setting::new("好みを記録・活用")
                            .description("選んだ案から好みを学び、次の提案に使います。")
                            .beside()
                            .child(Switch::new("learn", self.learn).on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.learn = !this.learn;
                                    cx.notify();
                                },
                            ))),
                    ),
            )
    }

    fn keys(&self, cx: &mut Context<Self>) -> SettingsPage {
        let mut list = KeybindingList::new("bindings").recording(self.recording);
        let mut group: Vec<KeyBinding> = Vec::new();
        let mut title = COMMANDS[0].group;
        for (index, command) in COMMANDS.iter().enumerate() {
            if command.group != title {
                list = list.group(title, std::mem::take(&mut group));
                title = command.group;
            }
            let keys = &self.keys[index];
            let mut binding = KeyBinding::new(command.label, keys.iter().cloned()).changed(
                keys.iter()
                    .map(|key| key.as_ref())
                    .ne(command.defaults.iter().copied()),
            );
            if let Some(scope) = command.scope {
                binding = binding.scope(scope);
            }
            // A key another command already has.
            let others: Vec<&str> = COMMANDS
                .iter()
                .enumerate()
                .filter(|(other, _)| {
                    *other != index && self.keys[*other].iter().any(|key| keys.contains(key))
                })
                .map(|(_, other)| other.label)
                .collect();
            if !others.is_empty() {
                binding = binding.conflict(others.join("、"));
            }
            group.push(binding);
        }
        list = list.group(title, group);
        SettingsPage::new("キー割り当て")
            .icon(IconName::Keyboard)
            .description("再生や録音などの操作に割り当てるキー")
            .keywords(COMMANDS.iter().map(|command| command.label))
            .child(
                list.on_record(cx.listener(|this, slot: &(usize, usize), _, cx| {
                    this.recording = Some(*slot);
                    cx.notify();
                }))
                .on_capture(cx.listener(|this, captured: &Captured, window, cx| {
                    if let (Some((row, slot)), Some(keystroke)) = (this.recording, captured) {
                        let label = keystroke_label(keystroke);
                        let keys = &mut this.keys[row];
                        if slot < keys.len() {
                            keys[slot] = label.clone();
                        } else {
                            keys.push(label.clone());
                        }
                        this.status = Some(
                            format!("「{}」を {label} に割り当てました", COMMANDS[row].label)
                                .into(),
                        );
                    }
                    this.recording = None;
                    window.focus(&this.search);
                    cx.notify();
                }))
                .on_remove(cx.listener(|this, (row, slot): &(usize, usize), _, cx| {
                    this.keys[*row].remove(*slot);
                    cx.notify();
                }))
                .on_reset(cx.listener(|this, row: &usize, _, cx| {
                    this.keys[*row] = defaults(*row);
                    cx.notify();
                })),
            )
    }
}

fn defaults(row: usize) -> Vec<SharedString> {
    COMMANDS[row]
        .defaults
        .iter()
        .map(|key| SharedString::from(*key))
        .collect()
}

impl Render for Preferences {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.autofocus) {
            window.focus(&self.search);
        }
        let theme = cx.theme().clone();
        let switch = common::theme_switch(self, cx);
        let status = match (&self.status, self.saved) {
            (Some(status), _) => StatusBar::new().message(status.clone()),
            (None, true) => StatusBar::new()
                .message("設定を保存しました")
                .tone(Tone::Success),
            (None, false) => StatusBar::new()
                .message("保存していない変更があります")
                .tone(Tone::Warning),
        };
        // Settings that apply at once (the switches) need no Save; the page's Save commits the
        // rest together.
        let body = PreferencesWindow::new("preferences")
            .page(self.general(cx))
            .page(self.voice(cx))
            .page(self.assistant(cx))
            .page(self.keys(cx))
            .selected(self.page)
            .on_select(cx.listener(|this, page: &usize, _, cx| {
                this.page = *page;
                this.query.clear();
                this.recording = None;
                cx.notify();
            }))
            .search(
                self.query.clone(),
                div()
                    .track_focus(&self.search)
                    .on_key_down(cx.listener(Self::key_down))
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _, window, _| window.focus(&this.search)),
                    )
                    .child(self.search_field(window)),
            )
            .sidebar_footer(
                Button::new("setup", "セットアップガイド…")
                    .ghost()
                    .icon(IconName::Info)
                    .full_width(),
            )
            .action(Button::new("defaults", "初期設定に戻す").ghost())
            .action(Button::new("revert", "元に戻す").disabled(self.saved))
            .action(
                Button::new("save", "保存")
                    .primary()
                    .disabled(self.saved)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.saved = true;
                        this.status = None;
                        cx.notify();
                    })),
            )
            .status(status.trailing("v0.13.0"));
        WindowFrame::new()
            .font(theme.typography.ui.clone())
            .text_color(theme.colors.text)
            .size_full()
            .flex()
            .flex_col()
            .child(
                TitleBar::new()
                    .app_icon(common::app_icon("atelier"))
                    .title("設定")
                    .trailing(Divider::vertical())
                    .trailing(switch),
            )
            .child(body)
    }
}

fn main() {
    let page = match common::argument("--page").as_deref() {
        Some("voice") => VOICE,
        Some("assistant") => ASSISTANT,
        Some("keys") => KEYS,
        _ => GENERAL,
    };
    let query = common::argument("--search").unwrap_or_default();
    common::run("設定", 1100., 760., move |choice, cx| {
        let search = cx.focus_handle();
        Preferences {
            choice,
            page,
            query,
            search,
            backend: 1,
            gpu: true,
            open_last: true,
            autosave: true,
            learn: true,
            fast: false,
            saved: false,
            keys: (0..COMMANDS.len()).map(defaults).collect(),
            recording: None,
            status: None,
            autofocus: true,
        }
    });
}
