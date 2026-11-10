//! The words Atelier's components draw themselves: tooltips, button labels, states.
//!
//! An application in another language installs its own [`Strings`] once, next to the theme;
//! every component reads them from there. [`Strings::japanese`] is built in. A component that
//! takes a label of its own (`CopyButton::label`, `ApprovalCard::allow_label`) still wins.
//!
//! ```ignore
//! atelier::init(cx, theme);
//! atelier::Strings::japanese().install(cx);
//! ```

use gpui::{App, Global, SharedString};

/// Every piece of text a component draws without being given it.
#[derive(Clone, Debug, PartialEq)]
pub struct Strings {
    /// The agent's name in its identity line and on the title-bar toggle.
    pub assistant: SharedString,
    /// The agent panel's close button.
    pub hide_assistant: SharedString,
    /// The composer's placeholder.
    pub ask_anything: SharedString,
    /// The composer's attach button.
    pub attach: SharedString,
    /// The composer's send button.
    pub send: SharedString,
    /// The composer's stop button.
    pub stop: SharedString,
    /// The composer's send button while a turn runs and the message will wait for it.
    pub queue: SharedString,
    /// A context chip's remove button; `{}` is the chip's label.
    pub remove: SharedString,
    /// A copy button's tooltip.
    pub copy: SharedString,
    /// What a copy button says once it has copied.
    pub copied: SharedString,
    /// The player's play button.
    pub play: SharedString,
    /// The player's pause button.
    pub pause: SharedString,
    /// The player's mute button.
    pub mute: SharedString,
    /// The player's mute button while muted.
    pub unmute: SharedString,
    /// An approval's default allow, deny and always-allow buttons.
    pub allow: SharedString,
    /// See [`Strings::allow`].
    pub deny: SharedString,
    /// See [`Strings::allow`].
    pub always_allow: SharedString,
    /// A task's cancel button.
    pub cancel: SharedString,
    /// A task's states: queued, working, done, failed, cancelled.
    pub task_states: [SharedString; 5],
    /// The agent's states: ready, working, needs approval, done, stopped.
    pub agent_states: [SharedString; 5],
    /// The heading over a usage meter's limits.
    pub usage: SharedString,
    /// A usage meter's label; `{}` is the whole percentage left.
    pub usage_left: SharedString,
    /// A limit's share spent; `{}` is the whole percentage.
    pub usage_used: SharedString,
    /// When a limit fills again; `{}` is the time the application gives.
    pub usage_resets: SharedString,
    /// A spent limit whose reset time is not known.
    pub usage_reached: SharedString,
    /// The usual limits' names: the five-hour window, the week.
    pub usage_windows: [SharedString; 2],
    /// Another window's name; `{}` is its length in hours.
    pub usage_window_hours: SharedString,
    /// Another window's name; `{}` is its length in days.
    pub usage_window_days: SharedString,
    /// How soon a limit resets: in minutes, in hours and minutes, in days and hours. Each `{}`
    /// takes one number, in that order.
    pub usage_in: [SharedString; 3],
    /// The effort button's tooltip, and the effort picker's name.
    pub effort: SharedString,
    /// The bolt's tooltip while fast mode is on.
    pub fast_mode_on: SharedString,
    /// The bolt's tooltip while fast mode is off.
    pub fast_mode_off: SharedString,
    /// The effort picker's reset button.
    pub reset: SharedString,
    /// A conversation's button back to its end, while the person reads further up.
    pub jump_to_latest: SharedString,
    /// The same button once something arrived meanwhile; `{}` is how many entries.
    pub new_messages: SharedString,
    /// The button that takes a timeline the person scrolled away from back to its playhead.
    pub back_to_playhead: SharedString,
    /// A colour picker's sliders: hue, saturation, brightness.
    pub hue: SharedString,
    /// See [`Strings::hue`].
    pub saturation: SharedString,
    /// See [`Strings::hue`].
    pub brightness: SharedString,
    /// Beside a colour code that could not be read.
    pub color_code_invalid: SharedString,
    /// A group of tool calls; `{}` is how many.
    pub used_tools: SharedString,
    /// An answered approval: allowed, always allowed, denied.
    pub approval_answers: [SharedString; 3],
    /// An approval while an automatic reviewer decides it.
    pub reviewing: SharedString,
    /// The heading of the permission mode menu.
    pub permissions: SharedString,
    /// The permission modes' names: plan, ask, accept edits, auto-review, bypass.
    pub permission_modes: [SharedString; 5],
    /// What each permission mode lets the agent do, in the same order.
    pub permission_details: [SharedString; 5],
    /// A running turn's phases: waiting, reading, thinking, writing, running a tool, awaiting
    /// approval, compacting, reviewing.
    pub agent_phases: [SharedString; 8],
    /// The heading of the agent's plan.
    pub plan: SharedString,
    /// The heading of the list of files the agent changed.
    pub changed_files: SharedString,
    /// The note under a graph node the assistant added.
    pub added_by_agent: SharedString,
    /// Keep what the assistant added.
    pub keep: SharedString,
    /// Undo what the assistant added.
    pub undo: SharedString,
    /// Pan a canvas back to where it started.
    pub reset_view: SharedString,
    /// A graph node whose last result was reused.
    pub cached: SharedString,
    /// The placeholder of the preferences search.
    pub search_settings: SharedString,
    /// A search that found nothing; `{}` is the search text.
    pub nothing_matches: SharedString,
    /// How many settings a search found; `{}` is the count.
    pub settings_found: SharedString,
    /// A key binding chip waiting for its key.
    pub press_a_key: SharedString,
    /// Add another key to a command.
    pub add_shortcut: SharedString,
    /// Put a command's keys back.
    pub reset_shortcut: SharedString,
    /// A command without a key.
    pub no_shortcut: SharedString,
    /// A key that also runs other commands; `{}` names them.
    pub also_runs: SharedString,
    /// Under a question that takes several answers.
    pub pick_any: SharedString,
    /// Messages waiting to be sent; `{}` is the count.
    pub queued: SharedString,
    /// Messages waiting while the queue is paused; `{}` is the count.
    pub queue_paused: SharedString,
    /// Start a paused queue again.
    pub resume: SharedString,
    /// Send a queued message ahead of the rest.
    pub send_now: SharedString,
    /// What [`Strings::send_now`] does to the turn under way.
    pub send_now_hint: SharedString,
    /// Take a queued message back into the composer.
    pub edit: SharedString,
    /// Start a new conversation.
    pub new_conversation: SharedString,
    /// A conversation without a title yet.
    pub untitled: SharedString,
    /// The conversations still in use.
    pub recent: SharedString,
    /// The conversations put away.
    pub archived: SharedString,
    /// An empty conversation list.
    pub no_conversations: SharedString,
    /// The placeholder of the conversation search.
    pub search_conversations: SharedString,
    /// A button that opens more actions.
    pub more_actions: SharedString,
}

impl Default for Strings {
    fn default() -> Self {
        Self::english()
    }
}

impl Global for Strings {}

impl Strings {
    /// English, the default.
    pub fn english() -> Self {
        Self {
            assistant: "Assistant".into(),
            hide_assistant: "Hide assistant".into(),
            ask_anything: "Ask the assistant…".into(),
            attach: "Attach context…".into(),
            send: "Send".into(),
            stop: "Stop".into(),
            queue: "Queue message".into(),
            remove: "Remove {}".into(),
            copy: "Copy".into(),
            copied: "Copied".into(),
            play: "Play".into(),
            pause: "Pause".into(),
            mute: "Mute".into(),
            unmute: "Unmute".into(),
            allow: "Allow".into(),
            deny: "Deny".into(),
            always_allow: "Always allow".into(),
            cancel: "Cancel".into(),
            task_states: [
                "Queued".into(),
                "Working".into(),
                "Done".into(),
                "Failed".into(),
                "Cancelled".into(),
            ],
            agent_states: [
                "Ready".into(),
                "Working".into(),
                "Needs approval".into(),
                "Done".into(),
                "Stopped".into(),
            ],
            usage: "Plan usage".into(),
            usage_left: "{} % left".into(),
            usage_used: "{} % used".into(),
            usage_resets: "Resets {}".into(),
            usage_reached: "Limit reached".into(),
            usage_windows: ["5-hour window".into(), "Weekly".into()],
            usage_window_hours: "{}-hour window".into(),
            usage_window_days: "{}-day window".into(),
            usage_in: [
                "in {} min".into(),
                "in {} h {} min".into(),
                "in {} d {} h".into(),
            ],
            effort: "Effort and fast mode".into(),
            fast_mode_on: "Fast mode: on".into(),
            fast_mode_off: "Fast mode: off".into(),
            reset: "Reset".into(),
            jump_to_latest: "Jump to latest".into(),
            new_messages: "{} new".into(),
            back_to_playhead: "Back to playhead".into(),
            hue: "Hue".into(),
            saturation: "Saturation".into(),
            brightness: "Brightness".into(),
            color_code_invalid: "Not a colour code".into(),
            used_tools: "Used {} tools".into(),
            approval_answers: ["Allowed".into(), "Always allowed".into(), "Denied".into()],
            reviewing: "Reviewing…".into(),
            permissions: "Permissions".into(),
            permission_modes: [
                "Plan only".into(),
                "Ask before changes".into(),
                "Accept edits".into(),
                "Auto-review".into(),
                "Bypass approvals".into(),
            ],
            permission_details: [
                "Reads and proposes. Changes nothing.".into(),
                "Asks before every edit and command.".into(),
                "Edits files without asking. Asks before commands.".into(),
                "A reviewer model answers each request. Unsure ones come to you.".into(),
                "Runs everything without asking. Use only in a sandbox.".into(),
            ],
            agent_phases: [
                "Waiting".into(),
                "Reading".into(),
                "Thinking".into(),
                "Writing".into(),
                "Running".into(),
                "Needs approval".into(),
                "Compacting".into(),
                "Reviewing".into(),
            ],
            plan: "Plan".into(),
            changed_files: "Changed files".into(),
            added_by_agent: "Added by the assistant".into(),
            keep: "Keep".into(),
            undo: "Undo".into(),
            reset_view: "Reset view".into(),
            cached: "Cached".into(),
            search_settings: "Search settings".into(),
            nothing_matches: "Nothing matches “{}”".into(),
            settings_found: "{} found".into(),
            press_a_key: "Press a key…".into(),
            add_shortcut: "Add a shortcut".into(),
            reset_shortcut: "Reset to default".into(),
            no_shortcut: "None".into(),
            also_runs: "Also runs {}".into(),
            pick_any: "Choose any that apply".into(),
            queued: "{} queued".into(),
            queue_paused: "{} queued, paused".into(),
            resume: "Resume".into(),
            send_now: "Send now".into(),
            send_now_hint: "Stop the current reply and send this next".into(),
            edit: "Edit".into(),
            new_conversation: "New conversation".into(),
            untitled: "Untitled conversation".into(),
            recent: "Recent".into(),
            archived: "Archived".into(),
            no_conversations: "No conversations yet".into(),
            search_conversations: "Search conversations".into(),
            more_actions: "More actions".into(),
        }
    }

    /// Japanese.
    pub fn japanese() -> Self {
        Self {
            assistant: "アシスタント".into(),
            hide_assistant: "アシスタントを隠す".into(),
            ask_anything: "アシスタントに質問…".into(),
            attach: "コンテキストを添付…".into(),
            send: "送信".into(),
            stop: "停止".into(),
            queue: "予約して送る".into(),
            remove: "{}を外す".into(),
            copy: "コピー".into(),
            copied: "コピーしました".into(),
            play: "再生".into(),
            pause: "一時停止".into(),
            mute: "ミュート".into(),
            unmute: "ミュートを解除".into(),
            allow: "許可".into(),
            deny: "拒否".into(),
            always_allow: "常に許可".into(),
            cancel: "キャンセル".into(),
            task_states: [
                "待機中".into(),
                "処理中".into(),
                "完了".into(),
                "失敗".into(),
                "キャンセル済み".into(),
            ],
            agent_states: [
                "待機中".into(),
                "作業中".into(),
                "承認待ち".into(),
                "完了".into(),
                "停止".into(),
            ],
            usage: "利用枠".into(),
            usage_left: "残り{}%".into(),
            usage_used: "{}%使用".into(),
            usage_resets: "{}にリセット".into(),
            usage_reached: "上限に達しました".into(),
            usage_windows: ["5時間枠".into(), "週間枠".into()],
            usage_window_hours: "{}時間枠".into(),
            usage_window_days: "{}日間枠".into(),
            usage_in: [
                "{}分後".into(),
                "{}時間{}分後".into(),
                "{}日{}時間後".into(),
            ],
            effort: "推論の深さと高速モード".into(),
            fast_mode_on: "高速モード: オン".into(),
            fast_mode_off: "高速モード: オフ".into(),
            reset: "リセット".into(),
            jump_to_latest: "最新へ移動".into(),
            new_messages: "新着{}件".into(),
            back_to_playhead: "再生位置に戻る".into(),
            hue: "色相".into(),
            saturation: "彩度".into(),
            brightness: "明度".into(),
            color_code_invalid: "カラーコードではありません".into(),
            used_tools: "ツールを{}回使用".into(),
            approval_answers: [
                "許可しました".into(),
                "常に許可しました".into(),
                "拒否しました".into(),
            ],
            reviewing: "レビュー中…".into(),
            permissions: "権限".into(),
            permission_modes: [
                "計画のみ".into(),
                "変更前に確認".into(),
                "編集を自動承認".into(),
                "自動レビュー".into(),
                "承認をスキップ".into(),
            ],
            permission_details: [
                "読み取りと提案だけで、何も変更しません。".into(),
                "編集やコマンドの前に毎回確認します。".into(),
                "ファイルの編集は確認なしで行い、コマンドの前は確認します。".into(),
                "レビュー用のモデルが各リクエストに答え、迷うものはあなたに回します。".into(),
                "すべて確認なしで実行します。サンドボックスの中でだけ使ってください。".into(),
            ],
            agent_phases: [
                "応答待ち".into(),
                "読み込み中".into(),
                "考え中".into(),
                "書き込み中".into(),
                "実行中".into(),
                "承認待ち".into(),
                "圧縮中".into(),
                "レビュー中".into(),
            ],
            plan: "計画".into(),
            changed_files: "変更したファイル".into(),
            added_by_agent: "アシスタントが追加".into(),
            keep: "残す".into(),
            undo: "元に戻す".into(),
            reset_view: "表示をリセット".into(),
            cached: "キャッシュ".into(),
            search_settings: "設定を検索".into(),
            nothing_matches: "「{}」に一致する設定はありません".into(),
            settings_found: "{} 件".into(),
            press_a_key: "キーを押してください…".into(),
            add_shortcut: "ショートカットを追加".into(),
            reset_shortcut: "初期設定に戻す".into(),
            no_shortcut: "なし".into(),
            also_runs: "{}にも割り当て済み".into(),
            pick_any: "当てはまるものをすべて選択".into(),
            queued: "送信待ち {} 件".into(),
            queue_paused: "送信待ち {} 件（一時停止中）".into(),
            resume: "再開".into(),
            send_now: "今すぐ送信".into(),
            send_now_hint: "今の応答を止めて、これを次に送ります".into(),
            edit: "編集".into(),
            new_conversation: "新しい会話".into(),
            untitled: "無題の会話".into(),
            recent: "最近".into(),
            archived: "アーカイブ".into(),
            no_conversations: "会話はまだありません".into(),
            search_conversations: "会話を検索".into(),
            more_actions: "その他の操作".into(),
        }
    }

    /// Install these as the application's and redraw.
    pub fn install(self, cx: &mut App) {
        cx.set_global(self);
        cx.refresh_windows();
    }

    /// [`Strings::remove`] with the chip's label put in.
    pub fn remove(&self, label: &str) -> SharedString {
        self.remove.replace("{}", label).into()
    }

    /// [`Strings::nothing_matches`] with the search text put in.
    pub fn nothing_matches(&self, query: &str) -> SharedString {
        self.nothing_matches.replace("{}", query).into()
    }

    /// [`Strings::settings_found`] with the count put in.
    pub fn settings_found(&self, count: usize) -> SharedString {
        self.settings_found.replace("{}", &count.to_string()).into()
    }

    /// [`Strings::queued`] or [`Strings::queue_paused`] with the count put in.
    pub fn queued(&self, count: usize, paused: bool) -> SharedString {
        let words = if paused {
            &self.queue_paused
        } else {
            &self.queued
        };
        words.replace("{}", &count.to_string()).into()
    }

    /// [`Strings::also_runs`] with the other commands put in.
    pub fn also_runs(&self, others: &str) -> SharedString {
        self.also_runs.replace("{}", others).into()
    }

    /// [`Strings::usage_left`] with the percentage put in.
    pub fn usage_left(&self, percent: &str) -> SharedString {
        self.usage_left.replace("{}", percent).into()
    }

    /// [`Strings::usage_used`] with the percentage put in.
    pub fn usage_used(&self, percent: &str) -> SharedString {
        self.usage_used.replace("{}", percent).into()
    }

    /// [`Strings::usage_resets`] with the time put in.
    pub fn usage_resets(&self, time: &str) -> SharedString {
        self.usage_resets.replace("{}", time).into()
    }

    /// The name of a limit whose window is `minutes` long: one of [`Strings::usage_windows`]
    /// for five hours and a week, else its length in hours or days.
    pub fn usage_window(&self, minutes: u32) -> SharedString {
        match minutes {
            300 => self.usage_windows[0].clone(),
            10_080 => self.usage_windows[1].clone(),
            minutes if minutes % 1440 == 0 => {
                fill(&self.usage_window_days, &[minutes / 1440]).into()
            }
            minutes => fill(&self.usage_window_hours, &[minutes.div_ceil(60)]).into(),
        }
    }

    /// How soon something `seconds` away happens, from [`Strings::usage_in`]: "in 42 min",
    /// "in 2 h 10 min", "in 3 d 4 h". Rounds up, so a time still ahead never reads as 0.
    pub fn usage_in(&self, seconds: u64) -> SharedString {
        let minutes = u32::try_from(seconds.div_ceil(60))
            .unwrap_or(u32::MAX)
            .max(1);
        let text = if minutes < 60 {
            fill(&self.usage_in[0], &[minutes])
        } else if minutes < 1440 {
            fill(&self.usage_in[1], &[minutes / 60, minutes % 60])
        } else {
            fill(&self.usage_in[2], &[minutes / 1440, minutes % 1440 / 60])
        };
        text.into()
    }
}

/// `words` with each `{}` replaced by the next of `numbers`.
fn fill(words: &str, numbers: &[u32]) -> String {
    let mut text = words.to_owned();
    for number in numbers {
        text = text.replacen("{}", &number.to_string(), 1);
    }
    text
}

/// Read access to the installed [`Strings`].
pub trait ActiveStrings {
    /// The strings in force; English if none were installed.
    fn strings(&self) -> &Strings;
}

impl ActiveStrings for App {
    fn strings(&self) -> &Strings {
        static FALLBACK: std::sync::OnceLock<Strings> = std::sync::OnceLock::new();
        self.try_global::<Strings>()
            .unwrap_or_else(|| FALLBACK.get_or_init(Strings::english))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_names_the_chip() {
        assert_eq!(Strings::english().remove("a.rs"), "Remove a.rs");
        assert_eq!(Strings::japanese().remove("a.rs"), "a.rsを外す");
    }

    #[test]
    fn usage_puts_the_numbers_in() {
        assert_eq!(Strings::english().usage_left("28"), "28 % left");
        assert_eq!(Strings::japanese().usage_left("28"), "残り28%");
        assert_eq!(Strings::japanese().usage_resets("14:30"), "14:30にリセット");
        let english = Strings::english();
        assert_eq!(english.usage_window(300), "5-hour window");
        assert_eq!(english.usage_window(10_080), "Weekly");
        assert_eq!(english.usage_window(43_200), "30-day window");
        assert_eq!(english.usage_window(120), "2-hour window");
        assert_eq!(english.usage_in(59), "in 1 min");
        assert_eq!(english.usage_in(2 * 3600 + 600), "in 2 h 10 min");
        assert_eq!(english.usage_in(3 * 86_400 + 4 * 3600), "in 3 d 4 h");
        let japanese = Strings::japanese();
        assert_eq!(japanese.usage_window(10_080), "週間枠");
        assert_eq!(japanese.usage_in(2 * 3600 + 600), "2時間10分後");
    }
}
