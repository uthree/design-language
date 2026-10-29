//! Emoji typed as shortcodes: `:thinking:` becomes 🤔.
//!
//! The kit leaves the editable text to the application, so this module is the part around it:
//! [`shortcode_at`] finds the `:thin` being typed before the caret, [`search`] ranks the emoji
//! it could mean, [`completions`] draws them as a [`Completions`] list, and
//! [`ShortcodeQuery::accept`] puts the chosen emoji in place of what was typed. A shortcode the
//! person finishes by hand, or pastes, is turned into its emoji by [`replace_shortcodes`].
//!
//! The emoji themselves are drawn by the platform's colour emoji font, which
//! [`crate::ui_font`] lists as a fallback: Apple Color Emoji on macOS and Segoe UI Emoji on
//! Windows. On Linux gpui chooses fallbacks itself and draws only Noto Color Emoji in colour, so
//! an application that ships there depends on it (`fonts-noto-color-emoji`, `noto-fonts-emoji`);
//! [`color_font_available`] says whether it is installed. Even then cosmic-text, which gpui 0.2
//! uses there, tries DejaVu Sans, FreeFont and Noto Sans Symbols first, so the emoji those also
//! cover (many smileys, ❤️, ✅) draw in one colour on Linux.

use std::ops::Range;

use gpui::{App, ElementId};

use crate::overlay::{Completion, Completions};

/// One emoji and the name it is typed by, without the colons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Emoji {
    /// The shortcode, such as `thinking`.
    pub name: &'static str,
    /// What it inserts, such as `🤔`.
    pub glyph: &'static str,
}

impl Emoji {
    /// The shortcode with its colons, as the person types it: `:thinking:`.
    pub fn shortcode(&self) -> String {
        format!(":{}:", self.name)
    }
}

/// The emoji [`search`] and [`lookup`] know, by the shortcodes GitHub and Slack use. A glyph
/// with two common names is listed under both.
pub const EMOJI: &[Emoji] = &{
    const fn e(name: &'static str, glyph: &'static str) -> Emoji {
        Emoji { name, glyph }
    }
    [
        // Faces
        e("smile", "😄"),
        e("smiley", "😃"),
        e("grinning", "😀"),
        e("grin", "😁"),
        e("laughing", "😆"),
        e("sweat_smile", "😅"),
        e("joy", "😂"),
        e("rofl", "🤣"),
        e("slightly_smiling_face", "🙂"),
        e("upside_down_face", "🙃"),
        e("wink", "😉"),
        e("blush", "😊"),
        e("innocent", "😇"),
        e("smiling_face_with_three_hearts", "🥰"),
        e("heart_eyes", "😍"),
        e("star_struck", "🤩"),
        e("kissing_heart", "😘"),
        e("yum", "😋"),
        e("stuck_out_tongue", "😛"),
        e("stuck_out_tongue_winking_eye", "😜"),
        e("zany_face", "🤪"),
        e("money_mouth_face", "🤑"),
        e("hugs", "🤗"),
        e("hand_over_mouth", "🤭"),
        e("shushing_face", "🤫"),
        e("thinking", "🤔"),
        e("zipper_mouth_face", "🤐"),
        e("raised_eyebrow", "🤨"),
        e("neutral_face", "😐"),
        e("expressionless", "😑"),
        e("no_mouth", "😶"),
        e("smirk", "😏"),
        e("unamused", "😒"),
        e("roll_eyes", "🙄"),
        e("grimacing", "😬"),
        e("lying_face", "🤥"),
        e("relieved", "😌"),
        e("pensive", "😔"),
        e("sleepy", "😪"),
        e("drooling_face", "🤤"),
        e("sleeping", "😴"),
        e("mask", "😷"),
        e("face_with_thermometer", "🤒"),
        e("nauseated_face", "🤢"),
        e("sneezing_face", "🤧"),
        e("hot_face", "🥵"),
        e("cold_face", "🥶"),
        e("woozy_face", "🥴"),
        e("dizzy_face", "😵"),
        e("exploding_head", "🤯"),
        e("cowboy_hat_face", "🤠"),
        e("partying_face", "🥳"),
        e("sunglasses", "😎"),
        e("nerd_face", "🤓"),
        e("monocle_face", "🧐"),
        e("confused", "😕"),
        e("worried", "😟"),
        e("slightly_frowning_face", "🙁"),
        e("open_mouth", "😮"),
        e("hushed", "😯"),
        e("astonished", "😲"),
        e("flushed", "😳"),
        e("pleading_face", "🥺"),
        e("frowning", "😦"),
        e("anguished", "😧"),
        e("fearful", "😨"),
        e("cold_sweat", "😰"),
        e("disappointed_relieved", "😥"),
        e("cry", "😢"),
        e("sob", "😭"),
        e("scream", "😱"),
        e("confounded", "😖"),
        e("persevere", "😣"),
        e("disappointed", "😞"),
        e("sweat", "😓"),
        e("weary", "😩"),
        e("tired_face", "😫"),
        e("yawning_face", "🥱"),
        e("triumph", "😤"),
        e("rage", "😡"),
        e("angry", "😠"),
        e("cursing_face", "🤬"),
        e("smiling_imp", "😈"),
        e("skull", "💀"),
        e("poop", "💩"),
        e("clown_face", "🤡"),
        e("ghost", "👻"),
        e("alien", "👽"),
        e("robot", "🤖"),
        e("smiley_cat", "😺"),
        e("see_no_evil", "🙈"),
        e("hear_no_evil", "🙉"),
        e("speak_no_evil", "🙊"),
        e("melting_face", "🫠"),
        e("saluting_face", "🫡"),
        e("face_holding_back_tears", "🥹"),
        // Hands and people
        e("+1", "👍"),
        e("thumbsup", "👍"),
        e("-1", "👎"),
        e("thumbsdown", "👎"),
        e("ok_hand", "👌"),
        e("pinched_fingers", "🤌"),
        e("v", "✌️"),
        e("crossed_fingers", "🤞"),
        e("metal", "🤘"),
        e("call_me_hand", "🤙"),
        e("point_left", "👈"),
        e("point_right", "👉"),
        e("point_up", "☝️"),
        e("point_down", "👇"),
        e("wave", "👋"),
        e("raised_hand", "✋"),
        e("clap", "👏"),
        e("raised_hands", "🙌"),
        e("open_hands", "👐"),
        e("handshake", "🤝"),
        e("pray", "🙏"),
        e("muscle", "💪"),
        e("writing_hand", "✍️"),
        e("eyes", "👀"),
        e("eye", "👁️"),
        e("brain", "🧠"),
        e("bow", "🙇"),
        e("facepalm", "🤦"),
        e("shrug", "🤷"),
        e("raising_hand", "🙋"),
        e("technologist", "🧑‍💻"),
        e("detective", "🕵️"),
        e("ninja", "🥷"),
        e("mage", "🧙"),
        e("runner", "🏃"),
        e("dancer", "💃"),
        // Hearts and symbols
        e("heart", "❤️"),
        e("orange_heart", "🧡"),
        e("yellow_heart", "💛"),
        e("green_heart", "💚"),
        e("blue_heart", "💙"),
        e("purple_heart", "💜"),
        e("black_heart", "🖤"),
        e("white_heart", "🤍"),
        e("broken_heart", "💔"),
        e("sparkling_heart", "💖"),
        e("two_hearts", "💕"),
        e("100", "💯"),
        e("anger", "💢"),
        e("boom", "💥"),
        e("collision", "💥"),
        e("dizzy", "💫"),
        e("sweat_drops", "💦"),
        e("dash", "💨"),
        e("zzz", "💤"),
        e("speech_balloon", "💬"),
        e("thought_balloon", "💭"),
        e("white_check_mark", "✅"),
        e("heavy_check_mark", "✔️"),
        e("ballot_box_with_check", "☑️"),
        e("x", "❌"),
        e("negative_squared_cross_mark", "❎"),
        e("warning", "⚠️"),
        e("no_entry", "⛔"),
        e("no_entry_sign", "🚫"),
        e("question", "❓"),
        e("grey_question", "❔"),
        e("exclamation", "❗"),
        e("bangbang", "‼️"),
        e("interrobang", "⁉️"),
        e("heavy_plus_sign", "➕"),
        e("heavy_minus_sign", "➖"),
        e("arrow_right", "➡️"),
        e("arrow_left", "⬅️"),
        e("arrow_up", "⬆️"),
        e("arrow_down", "⬇️"),
        e("arrows_counterclockwise", "🔄"),
        e("repeat", "🔁"),
        e("red_circle", "🔴"),
        e("orange_circle", "🟠"),
        e("yellow_circle", "🟡"),
        e("green_circle", "🟢"),
        e("blue_circle", "🔵"),
        e("purple_circle", "🟣"),
        e("white_circle", "⚪"),
        e("black_circle", "⚫"),
        e("copyright", "©️"),
        e("registered", "®️"),
        e("tm", "™️"),
        e("infinity", "♾️"),
        e("recycle", "♻️"),
        e("musical_note", "🎵"),
        e("notes", "🎶"),
        // Nature and weather
        e("sparkles", "✨"),
        e("star", "⭐"),
        e("star2", "🌟"),
        e("fire", "🔥"),
        e("zap", "⚡"),
        e("sunny", "☀️"),
        e("partly_sunny", "⛅"),
        e("cloud", "☁️"),
        e("rainbow", "🌈"),
        e("umbrella", "☔"),
        e("snowflake", "❄️"),
        e("snowman", "⛄"),
        e("droplet", "💧"),
        e("ocean", "🌊"),
        e("crescent_moon", "🌙"),
        e("full_moon", "🌕"),
        e("new_moon", "🌑"),
        e("earth_asia", "🌏"),
        e("earth_americas", "🌎"),
        e("comet", "☄️"),
        e("milky_way", "🌌"),
        e("seedling", "🌱"),
        e("herb", "🌿"),
        e("four_leaf_clover", "🍀"),
        e("maple_leaf", "🍁"),
        e("fallen_leaf", "🍂"),
        e("cherry_blossom", "🌸"),
        e("rose", "🌹"),
        e("sunflower", "🌻"),
        e("tulip", "🌷"),
        e("evergreen_tree", "🌲"),
        e("palm_tree", "🌴"),
        e("cactus", "🌵"),
        e("mushroom", "🍄"),
        // Animals
        e("dog", "🐶"),
        e("cat", "🐱"),
        e("mouse", "🐭"),
        e("rabbit", "🐰"),
        e("fox_face", "🦊"),
        e("bear", "🐻"),
        e("panda_face", "🐼"),
        e("koala", "🐨"),
        e("tiger", "🐯"),
        e("lion", "🦁"),
        e("cow", "🐮"),
        e("pig", "🐷"),
        e("frog", "🐸"),
        e("monkey_face", "🐵"),
        e("chicken", "🐔"),
        e("penguin", "🐧"),
        e("bird", "🐦"),
        e("hatching_chick", "🐣"),
        e("owl", "🦉"),
        e("eagle", "🦅"),
        e("bat", "🦇"),
        e("wolf", "🐺"),
        e("unicorn", "🦄"),
        e("bee", "🐝"),
        e("bug", "🐛"),
        e("butterfly", "🦋"),
        e("snail", "🐌"),
        e("lady_beetle", "🐞"),
        e("ant", "🐜"),
        e("spider", "🕷️"),
        e("turtle", "🐢"),
        e("snake", "🐍"),
        e("dragon", "🐉"),
        e("t-rex", "🦖"),
        e("octopus", "🐙"),
        e("crab", "🦀"),
        e("tropical_fish", "🐠"),
        e("fish", "🐟"),
        e("blowfish", "🐡"),
        e("dolphin", "🐬"),
        e("whale", "🐳"),
        e("shark", "🦈"),
        e("jellyfish", "🪼"),
        e("feet", "🐾"),
        // Food and drink
        e("apple", "🍎"),
        e("green_apple", "🍏"),
        e("tangerine", "🍊"),
        e("lemon", "🍋"),
        e("banana", "🍌"),
        e("watermelon", "🍉"),
        e("grapes", "🍇"),
        e("strawberry", "🍓"),
        e("peach", "🍑"),
        e("cherries", "🍒"),
        e("avocado", "🥑"),
        e("tomato", "🍅"),
        e("hot_pepper", "🌶️"),
        e("corn", "🌽"),
        e("bread", "🍞"),
        e("cheese", "🧀"),
        e("egg", "🥚"),
        e("hamburger", "🍔"),
        e("fries", "🍟"),
        e("pizza", "🍕"),
        e("hotdog", "🌭"),
        e("taco", "🌮"),
        e("ramen", "🍜"),
        e("sushi", "🍣"),
        e("rice_ball", "🍙"),
        e("rice", "🍚"),
        e("curry", "🍛"),
        e("bento", "🍱"),
        e("dango", "🍡"),
        e("oden", "🍢"),
        e("icecream", "🍦"),
        e("doughnut", "🍩"),
        e("cookie", "🍪"),
        e("cake", "🍰"),
        e("birthday", "🎂"),
        e("chocolate_bar", "🍫"),
        e("candy", "🍬"),
        e("coffee", "☕"),
        e("tea", "🍵"),
        e("beer", "🍺"),
        e("beers", "🍻"),
        e("wine_glass", "🍷"),
        e("sake", "🍶"),
        e("tumbler_glass", "🥃"),
        e("bubble_tea", "🧋"),
        // Activities and celebration
        e("tada", "🎉"),
        e("confetti_ball", "🎊"),
        e("balloon", "🎈"),
        e("gift", "🎁"),
        e("trophy", "🏆"),
        e("1st_place_medal", "🥇"),
        e("medal_sports", "🏅"),
        e("dart", "🎯"),
        e("video_game", "🎮"),
        e("game_die", "🎲"),
        e("jigsaw", "🧩"),
        e("art", "🎨"),
        e("performing_arts", "🎭"),
        e("microphone", "🎤"),
        e("headphones", "🎧"),
        e("musical_keyboard", "🎹"),
        e("guitar", "🎸"),
        e("violin", "🎻"),
        e("trumpet", "🎺"),
        e("saxophone", "🎷"),
        e("drum", "🥁"),
        e("soccer", "⚽"),
        e("basketball", "🏀"),
        e("tennis", "🎾"),
        e("fireworks", "🎆"),
        e("sparkler", "🎇"),
        e("crystal_ball", "🔮"),
        // Objects and work
        e("rocket", "🚀"),
        e("airplane", "✈️"),
        e("car", "🚗"),
        e("bike", "🚲"),
        e("ship", "🚢"),
        e("construction", "🚧"),
        e("rotating_light", "🚨"),
        e("house", "🏠"),
        e("office", "🏢"),
        e("japan", "🗾"),
        e("mount_fuji", "🗻"),
        e("tokyo_tower", "🗼"),
        e("hourglass", "⌛"),
        e("hourglass_flowing_sand", "⏳"),
        e("stopwatch", "⏱️"),
        e("alarm_clock", "⏰"),
        e("calendar", "📆"),
        e("date", "📅"),
        e("memo", "📝"),
        e("pencil2", "✏️"),
        e("pen", "🖊️"),
        e("paintbrush", "🖌️"),
        e("crayon", "🖍️"),
        e("book", "📖"),
        e("books", "📚"),
        e("notebook", "📓"),
        e("bookmark", "🔖"),
        e("page_facing_up", "📄"),
        e("clipboard", "📋"),
        e("pushpin", "📌"),
        e("paperclip", "📎"),
        e("scissors", "✂️"),
        e("file_folder", "📁"),
        e("open_file_folder", "📂"),
        e("package", "📦"),
        e("mailbox", "📫"),
        e("email", "📧"),
        e("envelope", "✉️"),
        e("inbox_tray", "📥"),
        e("outbox_tray", "📤"),
        e("bar_chart", "📊"),
        e("chart_with_upwards_trend", "📈"),
        e("chart_with_downwards_trend", "📉"),
        e("computer", "💻"),
        e("desktop_computer", "🖥️"),
        e("keyboard", "⌨️"),
        e("iphone", "📱"),
        e("telephone_receiver", "📞"),
        e("battery", "🔋"),
        e("electric_plug", "🔌"),
        e("bulb", "💡"),
        e("flashlight", "🔦"),
        e("candle", "🕯️"),
        e("mag", "🔍"),
        e("lock", "🔒"),
        e("unlock", "🔓"),
        e("key", "🔑"),
        e("hammer", "🔨"),
        e("wrench", "🔧"),
        e("hammer_and_wrench", "🛠️"),
        e("gear", "⚙️"),
        e("nut_and_bolt", "🔩"),
        e("link", "🔗"),
        e("magnet", "🧲"),
        e("test_tube", "🧪"),
        e("microscope", "🔬"),
        e("telescope", "🔭"),
        e("satellite", "📡"),
        e("pill", "💊"),
        e("broom", "🧹"),
        e("wastebasket", "🗑️"),
        e("bell", "🔔"),
        e("no_bell", "🔕"),
        e("loudspeaker", "📢"),
        e("mega", "📣"),
        e("speaker", "🔈"),
        e("mute", "🔇"),
        e("sound", "🔉"),
        e("loud_sound", "🔊"),
        e("camera", "📷"),
        e("movie_camera", "🎥"),
        e("clapper", "🎬"),
        e("tv", "📺"),
        e("radio", "📻"),
        e("film_strip", "🎞️"),
        e("moneybag", "💰"),
        e("gem", "💎"),
        e("crown", "👑"),
        e("ribbon", "🎀"),
        e("label", "🏷️"),
        e("triangular_flag_on_post", "🚩"),
        e("checkered_flag", "🏁"),
        e("white_flag", "🏳️"),
        e("jp", "🇯🇵"),
        e("us", "🇺🇸"),
        e("gb", "🇬🇧"),
    ]
};

/// The emoji typed by this shortcode, without its colons: `lookup("thinking")` is 🤔.
pub fn lookup(name: &str) -> Option<&'static Emoji> {
    EMOJI.iter().find(|emoji| emoji.name == name)
}

/// An emoji that matches what was typed, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmojiMatch {
    /// The emoji.
    pub emoji: &'static Emoji,
    /// Byte offsets in [`Emoji::shortcode`], colons included, of the letters that matched, for
    /// [`Completion::matched`].
    pub matched: Vec<usize>,
}

/// The emoji whose shortcodes match `query`, best first, at most `limit` of them.
///
/// A shortcode that starts with the query ranks first (`thi` finds `thinking`), then one with a
/// word that starts with it (`hand` finds `raised_hand`), then one that holds it anywhere.
/// Within a rank shorter shortcodes come first, so the plain name wins over its variants. Case
/// is ignored.
pub fn search(query: &str, limit: usize) -> Vec<EmojiMatch> {
    let query = query.to_ascii_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    let mut found: Vec<(u8, usize, usize, &'static Emoji)> = Vec::new();
    for (order, emoji) in EMOJI.iter().enumerate() {
        let Some((rank, at)) = rank(emoji.name, &query) else {
            continue;
        };
        // A glyph already found under a better name is not listed twice.
        if let Some(seen) = found.iter_mut().find(|seen| seen.3.glyph == emoji.glyph) {
            if (rank, emoji.name.len()) < (seen.0, seen.3.name.len()) {
                *seen = (rank, at, order, emoji);
            }
            continue;
        }
        found.push((rank, at, order, emoji));
    }
    found.sort_by_key(|&(rank, _, order, emoji)| (rank, emoji.name.len(), order));
    found
        .into_iter()
        .take(limit)
        .map(|(_, at, _, emoji)| EmojiMatch {
            emoji,
            // One past the opening colon.
            matched: (at + 1..at + 1 + query.len()).collect(),
        })
        .collect()
}

/// How well `name` matches: 0 for a prefix, 1 for a word prefix, 2 for anywhere; and where.
fn rank(name: &str, query: &str) -> Option<(u8, usize)> {
    if name.starts_with(query) {
        return Some((0, 0));
    }
    let mut anywhere = None;
    for (at, _) in name.match_indices(query) {
        if name[..at].ends_with(['_', '-']) {
            return Some((1, at));
        }
        anywhere.get_or_insert(at);
    }
    anywhere.map(|at| (2, at))
}

/// Whether `c` may appear in a shortcode.
fn is_shortcode_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '-')
}

/// A shortcode being typed: the `:thin` before the caret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShortcodeQuery {
    /// The byte range of the shortcode in the text, its opening colon included.
    pub range: Range<usize>,
    /// What follows the colon: `thin`.
    pub query: String,
}

impl ShortcodeQuery {
    /// The text with `emoji` in place of the shortcode, and the caret's byte offset just after
    /// the emoji.
    pub fn accept(&self, text: &str, emoji: &Emoji) -> (String, usize) {
        let mut text = text.to_owned();
        text.replace_range(self.range.clone(), emoji.glyph);
        (text, self.range.start + emoji.glyph.len())
    }
}

/// The shortcode being typed just before the byte offset `caret`, if there is one.
///
/// It opens with a colon anywhere but straight after a Latin letter, a digit, a colon or a
/// slash, so `いいね:+1` opens one and `10:30` and `https://` open nothing, and needs two letters after it before it counts, so a lone
/// colon at the end of a sentence opens no list.
pub fn shortcode_at(text: &str, caret: usize) -> Option<ShortcodeQuery> {
    let before = text.get(..caret)?;
    let colon = before.rfind(':')?;
    let query = &before[colon + 1..];
    if query.len() < 2 || !query.chars().all(is_shortcode_char) {
        return None;
    }
    if let Some(previous) = before[..colon].chars().next_back() {
        // Only Latin letters and digits hold it closed: Japanese runs on without spaces.
        if previous.is_ascii_alphanumeric() || previous == ':' || previous == '/' {
            return None;
        }
    }
    Some(ShortcodeQuery {
        range: colon..caret,
        query: query.to_owned(),
    })
}

/// The text with every known `:shortcode:` in it replaced by its emoji. Unknown ones are left
/// as typed, so `:not_an_emoji:` and `12:30:45` survive.
pub fn replace_shortcodes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find(':') {
        let (head, tail) = rest.split_at(open);
        out.push_str(head);
        let after = &tail[1..];
        let end = after.find(|c: char| !is_shortcode_char(c));
        let emoji = match end {
            Some(end) if after[end..].starts_with(':') => lookup(&after[..end]),
            _ => None,
        };
        match (emoji, end) {
            (Some(emoji), Some(end)) => {
                out.push_str(emoji.glyph);
                rest = &after[end + 1..];
            }
            _ => {
                out.push(':');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The matches as a [`Completions`] list: the emoji on the leading side, then its shortcode
/// with the typed letters in the accent. Add `.selected(...)` and `.on_select(...)` as for any
/// completion list, and hand it to [`crate::agent::Composer::completions`].
pub fn completions(id: impl Into<ElementId>, matches: &[EmojiMatch]) -> Completions {
    Completions::new(id).items(matches.iter().map(|found| {
        Completion::new(found.emoji.shortcode())
            .glyph(found.emoji.glyph)
            .matched(found.matched.clone())
    }))
}

/// The family gpui draws emoji in colour with on this platform.
pub fn color_font_family() -> &'static str {
    if cfg!(target_os = "macos") {
        "Apple Color Emoji"
    } else if cfg!(target_os = "windows") {
        "Segoe UI Emoji"
    } else {
        "Noto Color Emoji"
    }
}

/// Whether the colour emoji font gpui needs here is installed or loaded. Without it emoji show
/// as empty boxes or in one colour; offer the shortcodes as text instead, or load the font.
pub fn color_font_available(cx: &App) -> bool {
    let family = color_font_family();
    cx.text_system()
        .all_font_names()
        .iter()
        .any(|name| name == family)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_shortcode_being_typed() {
        let text = "hmm :thin";
        let query = shortcode_at(text, text.len()).unwrap();
        assert_eq!(query.query, "thin");
        assert_eq!(query.range, 4..9);
        assert_eq!(shortcode_at(":th", 3).unwrap().range, 0..3);
        assert_eq!(
            shortcode_at("いいね:+1", "いいね:+1".len()).unwrap().query,
            "+1"
        );
    }

    #[test]
    fn ignores_colons_that_open_nothing() {
        for text in [
            "10:30",
            "see https://x",
            "end:",
            "a :b",
            "x::ab",
            ":ab cd",
            "word:ab",
        ] {
            assert_eq!(shortcode_at(text, text.len()), None, "{text}");
        }
    }

    #[test]
    fn accepts_a_match_in_place() {
        let text = "so :thin and more";
        let query = shortcode_at(text, 8).unwrap();
        let (text, caret) = query.accept(text, lookup("thinking").unwrap());
        assert_eq!(text, "so 🤔 and more");
        assert_eq!(&text[..caret], "so 🤔");
    }

    #[test]
    fn ranks_prefixes_first() {
        let found = search("thi", 5);
        assert_eq!(found[0].emoji.glyph, "🤔");
        assert_eq!(found[0].matched, vec![1, 2, 3]);
        let found = search("hand", 10);
        let names: Vec<_> = found.iter().map(|found| found.emoji.name).collect();
        assert!(names.contains(&"raised_hand"), "{names:?}");
        assert!(search("zzzz_nothing", 5).is_empty());
    }

    #[test]
    fn lists_each_glyph_once() {
        let found = search("thumbs", 10);
        assert_eq!(found.iter().filter(|f| f.emoji.glyph == "👍").count(), 1);
    }

    #[test]
    fn replaces_finished_shortcodes() {
        assert_eq!(replace_shortcodes("いいね :+1: :tada:"), "いいね 👍 🎉");
        assert_eq!(
            replace_shortcodes(":not_an_emoji: 12:30:45"),
            ":not_an_emoji: 12:30:45"
        );
        assert_eq!(replace_shortcodes("::thinking::"), ":🤔:");
    }

    #[test]
    fn names_are_unique() {
        for (index, emoji) in EMOJI.iter().enumerate() {
            assert!(
                EMOJI[..index].iter().all(|other| other.name != emoji.name),
                "{} is listed twice",
                emoji.name
            );
            assert!(emoji.name.chars().all(is_shortcode_char), "{}", emoji.name);
        }
    }
}
