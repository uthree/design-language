# atelier-i18n

An application's words in one TOML file per language, compiled into the binary. A key that one
language has and another lacks, or a placeholder one translation names and another does not,
stops the build and names the file and the key. Nothing is parsed at run time and the files do
not ship. No UI and no gpui, so a command-line tool and the window can share one table.

```toml
[dependencies]
atelier-i18n = { git = "https://github.com/uthree/design-language", tag = "v0.4.0" }

[build-dependencies]
atelier-i18n = { git = "https://github.com/uthree/design-language", tag = "v0.4.0", features = ["build"] }
```

```rust
// build.rs
fn main() {
    atelier_i18n::build::Catalog::new("locales")
        .fallback("en")
        .serde(true) // derive Serialize/Deserialize on Language, as its tag
        .generate()
        .unwrap_or_else(|error| panic!("{error}"));
}
```

```rust
// src/lib.rs
mod text {
    include!(concat!(env!("OUT_DIR"), "/atelier_i18n.rs"));
}
pub use text::{Key, Language, messages, t};
```

## The files

`locales/en.toml`, `locales/ja.toml`, `locales/pt-BR.toml`: the file name is the language tag.

```toml
_language = { variant = "Japanese", endonym = "日本語" }

mixer = "ミキサー"
track_count = "{count}トラック"
tempo = "{bpm:.1} BPM"
braces = "{{ と }} で囲む"

[piano_roll]
title = "ピアノロール — {clip}"
moved = "{bar}小節目へ{count}音を移動"
```

- The first line is an inline table, so that the keys after it stay at the top level: `variant`
  is the enum variant, `endonym` the language's name in itself, which a picker shows.
- A string without a placeholder is a `Key`: `mixer` is `Key::Mixer`, `piano_roll.title`
  would be `Key::PianoRollTitle`. Keys already written in `CamelCase` stay as they are.
- A string with placeholders is a function: `messages::piano_roll_moved(language, count, bar)`.
  The parameters follow the fallback language's order, each is `impl Display`, and a
  placeholder may carry a format spec. Placeholders are named, since another language may need
  the values in another order.
- `{{` and `}}` are braces.

## The code

| Item | What it is |
| --- | --- |
| `Language` | `ALL` (the fallback first), `FALLBACK`, `code`, `endonym`, `from_locale`, `from_system`, `resolve(Option<Language>)` |
| `Key` | `ALL`, `get(language)`, `name()` (the key as the file writes it) |
| `t(key, language)` | `Key::get` with the language last |
| `messages::*` | one function per string with placeholders |

Every item carries a doc comment, the fallback text, so `#![warn(missing_docs)]` holds. The
fixed strings are two static tables indexed by the enums, which keeps a few thousand keys quick
to compile.

## Which language

`preferred_locales()` lists what the person reads, best first: the first of `LC_ALL`,
`LC_MESSAGES` and `LANG` that is set, then the operating system. Windows sets none of those
variables, and neither does an application macOS starts from Finder or the Dock, so asking the
system (`GetUserDefaultLocaleName`, `NSLocale.preferredLanguages`) is what opens a Japanese
install in Japanese. `negotiate` matches `ja_JP.UTF-8` or `en-GB` against the build's tags:
an exact match, then the bare language, then any region of it.

## What stays in code

Text that branches on more than a value, such as a plural chosen by count or a sentence built
from optional parts, is still a function the application writes; the files hold sentences.
