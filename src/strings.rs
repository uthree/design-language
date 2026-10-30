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
    /// The effort button's tooltip, and the effort picker's name.
    pub effort: SharedString,
    /// The bolt's tooltip while fast mode is on.
    pub fast_mode_on: SharedString,
    /// The bolt's tooltip while fast mode is off.
    pub fast_mode_off: SharedString,
    /// The effort picker's reset button.
    pub reset: SharedString,
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
            ask_anything: "Ask anything…".into(),
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
            effort: "Effort and fast mode".into(),
            fast_mode_on: "Fast mode: on".into(),
            fast_mode_off: "Fast mode: off".into(),
            reset: "Reset".into(),
        }
    }

    /// Japanese.
    pub fn japanese() -> Self {
        Self {
            assistant: "アシスタント".into(),
            hide_assistant: "アシスタントを隠す".into(),
            ask_anything: "なんでも聞いてください…".into(),
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
            effort: "推論の深さと高速モード".into(),
            fast_mode_on: "高速モード: オン".into(),
            fast_mode_off: "高速モード: オフ".into(),
            reset: "リセット".into(),
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
    }
}
