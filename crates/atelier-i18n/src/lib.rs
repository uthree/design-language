//! Interface text in one TOML file per language, compiled into the binary.
//!
//! An application keeps a `locales/` folder beside its `Cargo.toml`, one file per language, and
//! a build script turns the folder into Rust: a `Language` enum, a `Key` enum with one variant
//! per fixed string, and a function per string that takes values. The files never ship and are
//! never parsed at run time. A key that one language has and another lacks, or a placeholder
//! that one translation names and another does not, stops the build with the file and the key
//! in the message, so a hole is found by whoever adds the string, not by a user.
//!
//! # Setting it up
//!
//! ```toml
//! [dependencies]
//! atelier-i18n = { git = "https://github.com/uthree/design-language", tag = "vX.Y.Z" }
//!
//! [build-dependencies]
//! atelier-i18n = { git = "https://github.com/uthree/design-language", tag = "vX.Y.Z", features = ["build"] }
//! ```
//!
//! ```ignore
//! // build.rs
//! fn main() {
//!     atelier_i18n::build::Catalog::new("locales")
//!         .fallback("en")
//!         .generate()
//!         .unwrap_or_else(|error| panic!("{error}"));
//! }
//! ```
//!
//! ```ignore
//! // src/lib.rs
//! mod text {
//!     include!(concat!(env!("OUT_DIR"), "/atelier_i18n.rs"));
//! }
//! pub use text::{Key, Language, messages, t};
//! ```
//!
//! # The files
//!
//! Each file is named by its language tag (`en.toml`, `ja.toml`, `pt-BR.toml`) and opens with a
//! `_language` line naming the enum variant and the language's name in itself, which is what a
//! language picker shows. It is an inline table so that the keys after it stay at the top level:
//!
//! ```toml
//! _language = { variant = "Japanese", endonym = "日本語" }
//!
//! mixer = "ミキサー"
//! track_count = "{count}トラック"
//!
//! [piano_roll]
//! title = "ピアノロール — {clip}"
//! ```
//!
//! - A key becomes a `CamelCase` name: `mixer` is `Key::Mixer`, and a table groups keys, so
//!   `piano_roll.title` is `piano_roll_title`. A key may also be written in `CamelCase` already.
//! - A string with no placeholder is a [`Key`](#generated-code) variant. A string with one is a
//!   function in `messages`, `messages::track_count(language, count)`, whose parameters are the
//!   placeholders in the order the fallback language uses them. Each is `impl Display`, and a
//!   placeholder may carry a format spec: `{bpm:.1}`.
//! - `{{` and `}}` are literal braces. A placeholder has to be named, since another language may
//!   need the values in another order; `{}` is an error.
//!
//! # Generated code
//!
//! - `Language`, with `ALL`, `FALLBACK`, `code`, `endonym`, `from_locale`, `from_system` and
//!   `resolve`. With `Catalog::serde` it also derives `Serialize` and `Deserialize`, as
//!   its tag (`"ja"`).
//! - `Key`, with `ALL`, `get(language)` and `name`, and `t(key, language)`, the same lookup with
//!   the language last.
//! - `messages`, one `pub fn` per string with placeholders.
//!
//! Every item carries a doc comment, the fallback text, so `#![warn(missing_docs)]` holds.
//!
//! # Which language
//!
//! [`preferred_locales`] lists what the person reads, best first: the locale variables, then
//! the operating system. Windows sets no locale variables, and an application that macOS starts
//! from Finder or the Dock gets none either, so asking the system is what lets a Japanese install
//! open in Japanese. [`negotiate`] picks the best match out of the languages a build has.

#![warn(missing_docs)]

#[cfg(feature = "build")]
pub mod build;

/// The locales the person asked for, best first, as the environment and the system name them.
///
/// The first of `LC_ALL`, `LC_MESSAGES` and `LANG` that is set comes first, the order POSIX gives
/// them, because someone who exports `LANG` for one program means it for that program. Then come
/// [`system_locales`]. `C` and `POSIX` are skipped: they ask for no language at all.
pub fn preferred_locales() -> Vec<String> {
    let mut locales: Vec<String> = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .find_map(|name| std::env::var(name).ok().filter(|value| !value.is_empty()))
        .filter(|value| !matches!(value.as_str(), "C" | "POSIX") && !value.starts_with("C."))
        .into_iter()
        .collect();
    locales.extend(system_locales());
    locales
}

/// The languages the operating system is set to, best first, where there is a way to ask.
///
/// macOS answers with the ordered list in System Settings and Windows with the user's default
/// locale. Elsewhere the answer is empty, since the locale variables [`preferred_locales`] reads
/// are already how those systems say it.
pub fn system_locales() -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;

        // `LOCALE_NAME_MAX_LENGTH`, fixed at 85 since Vista.
        let mut name = [0u16; 85];
        // SAFETY: the pointer is to `name`, which is writable for the length passed as the
        // second argument. The call writes UTF-16 within that bound and nothing else.
        let written = unsafe { GetUserDefaultLocaleName(name.as_mut_ptr(), name.len() as i32) };
        // The count includes the terminating null; 0 means the call failed.
        usize::try_from(written)
            .ok()
            .and_then(|count| count.checked_sub(1))
            .and_then(|end| name.get(..end))
            .map(|name| vec![String::from_utf16_lossy(name)])
            .unwrap_or_default()
    }
    #[cfg(target_os = "macos")]
    {
        objc2_foundation::NSLocale::preferredLanguages()
            .to_vec()
            .iter()
            .map(|language| language.to_string())
            .collect()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Vec::new()
    }
}

/// Which of `available` (language tags such as `en`, `ja`, `pt-BR`) best serves `locale`.
///
/// `locale` may be written the POSIX way (`ja_JP.UTF-8`, `de_DE@euro`) or the BCP 47 way
/// (`en-GB`); case does not matter. An exact match wins, then a tag that names the same language
/// with no region, then any tag of the same language: a regional variant of a language the build
/// has is still that language, and one it lacks is not improved by its region.
///
/// ```
/// let available = ["en", "ja", "pt-BR"];
/// assert_eq!(atelier_i18n::negotiate("ja_JP.UTF-8", &available), Some(1));
/// assert_eq!(atelier_i18n::negotiate("en-GB", &available), Some(0));
/// assert_eq!(atelier_i18n::negotiate("pt_PT", &available), Some(2));
/// assert_eq!(atelier_i18n::negotiate("de", &available), None);
/// ```
pub fn negotiate(locale: &str, available: &[&str]) -> Option<usize> {
    let wanted = normalise(locale);
    if wanted.is_empty() {
        return None;
    }
    let primary = |tag: &str| tag.split('-').next().unwrap_or_default().to_owned();
    let tags: Vec<String> = available.iter().map(|tag| normalise(tag)).collect();
    tags.iter()
        .position(|tag| *tag == wanted)
        .or_else(|| tags.iter().position(|tag| *tag == primary(&wanted)))
        .or_else(|| tags.iter().position(|tag| primary(tag) == primary(&wanted)))
}

/// The first of `locales` that [`negotiate`] can serve from `available`.
pub fn negotiate_first<S: AsRef<str>>(locales: &[S], available: &[&str]) -> Option<usize> {
    locales
        .iter()
        .find_map(|locale| negotiate(locale.as_ref(), available))
}

/// `ja_JP.UTF-8` → `ja-jp`: separators unified, encoding and modifier dropped, lower case.
fn normalise(locale: &str) -> String {
    locale
        .split(['.', '@'])
        .next()
        .unwrap_or_default()
        .trim()
        .replace('_', "-")
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posix_and_bcp47_spellings_match_the_same_tag() {
        let available = ["en", "ja"];
        for locale in [
            "ja",
            "JA",
            "ja_JP",
            "ja_JP.UTF-8",
            "ja-JP",
            "ja_JP@modifier",
        ] {
            assert_eq!(negotiate(locale, &available), Some(1), "{locale}");
        }
        assert_eq!(negotiate("en_US.UTF-8", &available), Some(0));
    }

    #[test]
    fn an_exact_region_outranks_the_bare_language_and_any_other_region() {
        let available = ["pt-PT", "pt", "pt-BR"];
        assert_eq!(negotiate("pt_BR", &available), Some(2));
        assert_eq!(negotiate("pt_AO", &available), Some(1));
        assert_eq!(negotiate("pt_PT", &available), Some(0));
        assert_eq!(negotiate("pt_AO", &["pt-BR"]), Some(0));
    }

    #[test]
    fn a_language_the_build_lacks_or_an_empty_locale_matches_nothing() {
        assert_eq!(negotiate("de_DE", &["en", "ja"]), None);
        assert_eq!(negotiate("", &["en"]), None);
        assert_eq!(negotiate(".UTF-8", &["en"]), None);
    }

    #[test]
    fn the_first_locale_that_can_be_served_wins() {
        let locales = ["de_DE", "ja_JP", "en_US"];
        assert_eq!(negotiate_first(&locales, &["en", "ja"]), Some(1));
        assert_eq!(negotiate_first::<&str>(&[], &["en"]), None);
    }

    #[test]
    fn asking_the_system_never_panics() {
        let system = system_locales();
        assert!(cfg!(any(target_os = "windows", target_os = "macos")) || system.is_empty());
        let _ = preferred_locales();
    }
}
