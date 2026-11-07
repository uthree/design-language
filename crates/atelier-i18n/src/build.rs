//! The code generator, for an application's build script.
//!
//! [`Catalog::generate`] reads every `*.toml` in a folder, checks that the languages agree, and
//! writes `$OUT_DIR/atelier_i18n.rs`. The crate root describes the files and the code.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write as _};
use std::path::{Path, PathBuf};

/// The name of the file [`Catalog::generate`] writes in `OUT_DIR`.
pub const OUTPUT: &str = "atelier_i18n.rs";

/// The table each language file opens with.
const META: &str = "_language";

/// A folder of language files, and how to turn it into Rust.
#[derive(Clone, Debug)]
pub struct Catalog {
    dir: PathBuf,
    fallback: Option<String>,
    serde: bool,
}

impl Catalog {
    /// The files in `dir`, relative to the package's `Cargo.toml` unless absolute.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            fallback: None,
            serde: false,
        }
    }

    /// The language that is the default and the last resort, by its file's tag (`"en"`).
    ///
    /// Its strings decide which keys are messages and the order of their parameters, and the
    /// other files are checked against it. Without it there must be an `en.toml`.
    pub fn fallback(mut self, tag: impl Into<String>) -> Self {
        self.fallback = Some(tag.into());
        self
    }

    /// Derive `serde::Serialize` and `Deserialize` on the generated `Language`.
    ///
    /// A language is written as its tag (`"ja"`), and its variant name in lower case
    /// (`"japanese"`) is also read. The application then needs `serde` with `derive`.
    pub fn serde(mut self, on: bool) -> Self {
        self.serde = on;
        self
    }

    /// Reads the folder, checks it, and writes [`OUTPUT`] into `OUT_DIR`.
    ///
    /// Tells Cargo to run the build script again when anything in the folder changes, so a
    /// new language file is picked up as well as an edited one. Returns the written path.
    pub fn generate(self) -> Result<PathBuf, Error> {
        let dir = self.resolved_dir();
        println!("cargo:rerun-if-changed={}", dir.display());
        let code = self.render()?;
        let out = std::env::var_os("OUT_DIR")
            .map(PathBuf::from)
            .ok_or_else(|| Error::one("OUT_DIR is not set: call this from a build script"))?;
        let path = out.join(OUTPUT);
        std::fs::write(&path, code)
            .map_err(|error| Error::one(format!("cannot write {}: {error}", path.display())))?;
        Ok(path)
    }

    /// The Rust [`Catalog::generate`] would write, without writing it.
    pub fn render(&self) -> Result<String, Error> {
        let dir = self.resolved_dir();
        let entries = std::fs::read_dir(&dir)
            .map_err(|error| Error::one(format!("cannot read {}: {error}", dir.display())))?;
        let mut sources = Vec::new();
        for entry in entries {
            let path = entry
                .map_err(|error| Error::one(format!("cannot read {}: {error}", dir.display())))?
                .path();
            if path.extension().is_none_or(|extension| extension != "toml") {
                continue;
            }
            let tag = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or_default()
                .to_owned();
            let text = std::fs::read_to_string(&path)
                .map_err(|error| Error::one(format!("cannot read {}: {error}", path.display())))?;
            sources.push(Source { tag, text });
        }
        render(
            &sources,
            self.fallback.as_deref().unwrap_or("en"),
            self.serde,
        )
    }

    fn resolved_dir(&self) -> PathBuf {
        if self.dir.is_absolute() {
            return self.dir.clone();
        }
        std::env::var_os("CARGO_MANIFEST_DIR")
            .map(|root| Path::new(&root).join(&self.dir))
            .unwrap_or_else(|| self.dir.clone())
    }
}

/// Everything wrong with a folder of language files, one problem per line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    problems: Vec<String>,
}

impl Error {
    fn one(problem: impl Into<String>) -> Self {
        Self {
            problems: vec![problem.into()],
        }
    }

    /// Each problem, as a sentence naming the file and the key.
    pub fn problems(&self) -> &[String] {
        &self.problems
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "the language files do not agree:")?;
        for problem in &self.problems {
            writeln!(f, "  - {problem}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

/// One language file's tag and contents.
#[derive(Clone, Debug)]
pub(crate) struct Source {
    pub(crate) tag: String,
    pub(crate) text: String,
}

/// One language, read.
struct Language {
    tag: String,
    variant: String,
    endonym: String,
    /// Dotted key path → text.
    strings: BTreeMap<String, String>,
}

/// One piece of a string: literal text, or a placeholder and its format spec.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Piece {
    Text(String),
    Value { name: String },
}

/// The generator itself, on sources already read: what the tests drive.
pub(crate) fn render(sources: &[Source], fallback: &str, serde: bool) -> Result<String, Error> {
    let mut problems = Vec::new();
    let mut languages = Vec::new();
    for source in sources {
        match read_language(source) {
            Ok(language) => languages.push(language),
            Err(mut found) => problems.append(&mut found),
        }
    }
    if !problems.is_empty() {
        return Err(Error { problems });
    }
    let Some(position) = languages.iter().position(|l| l.tag == fallback) else {
        return Err(Error::one(format!(
            "there is no {fallback}.toml for the fallback language"
        )));
    };
    // The fallback first, then the rest by tag: the order of `Language::ALL`.
    let first = languages.remove(position);
    languages.sort_by(|a, b| a.tag.cmp(&b.tag));
    languages.insert(0, first);

    let mut variants = BTreeSet::new();
    for language in &languages {
        if !variants.insert(language.variant.as_str()) {
            problems.push(format!(
                "{}.toml: another file already uses the variant {}",
                language.tag, language.variant
            ));
        }
    }

    // Every key, its Rust name, and whether it takes values.
    let base = &languages[0];
    let mut keys: BTreeMap<String, Entry> = BTreeMap::new();
    let mut by_ident: BTreeMap<String, String> = BTreeMap::new();
    for (path, text) in &base.strings {
        let ident = camel(path);
        if let Some(other) = by_ident.insert(ident.clone(), path.clone()) {
            problems.push(format!(
                "{}.toml: `{path}` and `{other}` both become `{ident}`",
                base.tag
            ));
            continue;
        }
        let pieces = match parse(text) {
            Ok(pieces) => pieces,
            Err(problem) => {
                problems.push(format!("{}.toml: `{path}`: {problem}", base.tag));
                continue;
            }
        };
        let parameters = parameters(&pieces);
        let function = snake(&ident);
        if !parameters.is_empty() && is_keyword(&function) {
            problems.push(format!(
                "{}.toml: `{path}` takes values, and `{function}` is a Rust keyword",
                base.tag
            ));
        }
        for parameter in &parameters {
            if is_keyword(parameter) || parameter == "language" {
                problems.push(format!(
                    "{}.toml: `{path}`: a placeholder cannot be called `{parameter}`",
                    base.tag
                ));
            }
        }
        keys.insert(
            path.clone(),
            Entry {
                ident,
                function,
                parameters,
            },
        );
    }

    for language in &languages[1..] {
        for path in base.strings.keys() {
            if !language.strings.contains_key(path) {
                problems.push(format!(
                    "{}.toml: `{path}` is missing (it is in {}.toml)",
                    language.tag, base.tag
                ));
            }
        }
        for (path, text) in &language.strings {
            let Some(entry) = keys.get(path) else {
                if !base.strings.contains_key(path) {
                    problems.push(format!(
                        "{}.toml: `{path}` is not in {}.toml",
                        language.tag, base.tag
                    ));
                }
                continue;
            };
            match parse(text) {
                Ok(pieces) => {
                    let theirs: BTreeSet<_> = parameters(&pieces).into_iter().collect();
                    let ours: BTreeSet<_> = entry.parameters.iter().cloned().collect();
                    if theirs != ours {
                        problems.push(format!(
                            "{}.toml: `{path}` uses {} but {}.toml uses {}",
                            language.tag,
                            describe(&theirs),
                            base.tag,
                            describe(&ours)
                        ));
                    }
                }
                Err(problem) => {
                    problems.push(format!("{}.toml: `{path}`: {problem}", language.tag))
                }
            }
        }
    }

    if !problems.is_empty() {
        return Err(Error { problems });
    }
    Ok(emit(&languages, &keys, serde))
}

/// A key as the generated code names it.
struct Entry {
    ident: String,
    function: String,
    /// Placeholder names, in the order the fallback first uses them.
    parameters: Vec<String>,
}

fn read_language(source: &Source) -> Result<Language, Vec<String>> {
    let file = format!("{}.toml", source.tag);
    if source.tag.is_empty()
        || !source
            .tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(vec![format!(
            "{file}: a language file is named by its tag, such as en.toml or pt-BR.toml"
        )]);
    }
    let table: toml::Table = source
        .text
        .parse()
        .map_err(|error| vec![format!("{file}: {error}")])?;
    let mut problems = Vec::new();
    let meta = table.get(META).and_then(|value| value.as_table());
    let field = |name: &str| {
        meta.and_then(|meta| meta.get(name))
            .and_then(|value| value.as_str())
            .map(str::to_owned)
    };
    let variant = field("variant").unwrap_or_default();
    let endonym = field("endonym").unwrap_or_default();
    if meta.is_none() {
        problems.push(format!(
            "{file}: opens with no [{META}] table naming `variant` and `endonym`"
        ));
    } else {
        if !is_type_name(&variant) {
            problems.push(format!(
                "{file}: [{META}] variant must be a CamelCase Rust name, such as \"Japanese\""
            ));
        }
        for key in meta.into_iter().flat_map(|meta| meta.keys()) {
            if key != "variant" && key != "endonym" {
                problems.push(format!(
                    "{file}: `{key}` is inside [{META}]; write the table inline on the first line, \
                     `{META} = {{ variant = \"…\", endonym = \"…\" }}`, so the keys after it stay at the top"
                ));
            }
        }
        if endonym.is_empty() {
            problems.push(format!(
                "{file}: [{META}] endonym is the language's name in itself, such as \"日本語\""
            ));
        }
    }
    let mut strings = BTreeMap::new();
    for (key, value) in &table {
        if key == META {
            continue;
        }
        flatten(&file, key, value, &mut strings, &mut problems);
    }
    if problems.is_empty() {
        Ok(Language {
            tag: source.tag.clone(),
            variant,
            endonym,
            strings,
        })
    } else {
        Err(problems)
    }
}

fn flatten(
    file: &str,
    path: &str,
    value: &toml::Value,
    strings: &mut BTreeMap<String, String>,
    problems: &mut Vec<String>,
) {
    if !is_key_segment(path.rsplit('.').next().unwrap_or_default()) {
        problems.push(format!(
            "{file}: `{path}`: a key is letters, digits and underscores, starting with a letter"
        ));
        return;
    }
    match value {
        toml::Value::String(text) => {
            strings.insert(path.to_owned(), text.clone());
        }
        toml::Value::Table(table) => {
            for (key, value) in table {
                flatten(file, &format!("{path}.{key}"), value, strings, problems);
            }
        }
        _ => problems.push(format!(
            "{file}: `{path}` is not a string; every entry is text or a table of them"
        )),
    }
}

/// Splits a string into text and placeholders, the way `format!` reads it.
fn parse(text: &str) -> Result<Vec<Piece>, String> {
    let mut pieces = Vec::new();
    let mut literal = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                literal.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                literal.push('}');
            }
            '}' => return Err("a lone `}`; write `}}` for a brace".into()),
            '{' => {
                let mut inside = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some('{') | None => {
                            return Err("a `{` that is never closed; write `{{` for a brace".into());
                        }
                        Some(c) => inside.push(c),
                    }
                }
                let name = inside.split(':').next().unwrap_or_default().trim();
                if name.is_empty() {
                    return Err(
                        "a placeholder without a name; write `{count}`, since another language may need the values in another order"
                            .into(),
                    );
                }
                if !is_value_name(name) {
                    return Err(format!(
                        "`{{{inside}}}`: a placeholder is named in snake_case, such as `{{count}}`"
                    ));
                }
                if !literal.is_empty() {
                    pieces.push(Piece::Text(std::mem::take(&mut literal)));
                }
                pieces.push(Piece::Value {
                    name: name.to_owned(),
                });
            }
            c => literal.push(c),
        }
    }
    if !literal.is_empty() {
        pieces.push(Piece::Text(literal));
    }
    Ok(pieces)
}

fn parameters(pieces: &[Piece]) -> Vec<String> {
    let mut seen = Vec::new();
    for piece in pieces {
        if let Piece::Value { name } = piece
            && !seen.contains(name)
        {
            seen.push(name.clone());
        }
    }
    seen
}

/// The text of a string with no placeholders, its `{{` and `}}` read as braces.
fn literal(text: &str) -> String {
    text.replace("{{", "{").replace("}}", "}")
}

fn describe(names: &BTreeSet<String>) -> String {
    if names.is_empty() {
        "no placeholder".into()
    } else {
        names
            .iter()
            .map(|name| format!("{{{name}}}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// `piano_roll.title` → `PianoRollTitle`; `IssueReport` stays as it is.
fn camel(path: &str) -> String {
    path.split(['.', '_', '-'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            let first = chars.next().map(|c| c.to_ascii_uppercase());
            first.into_iter().chain(chars).collect::<String>()
        })
        .collect()
}

/// `PianoRollTitle` → `piano_roll_title`.
fn snake(ident: &str) -> String {
    let mut out = String::new();
    for (index, c) in ident.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if index > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn is_key_segment(segment: &str) -> bool {
    let mut chars = segment.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_type_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_uppercase())
        && chars.all(|c| c.is_ascii_alphanumeric())
        && name != "Self"
}

fn is_value_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c == '_')
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && name != "_"
}

fn is_keyword(name: &str) -> bool {
    matches!(
        name,
        "as" | "async"
            | "await"
            | "break"
            | "const"
            | "continue"
            | "crate"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "gen"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "yield"
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "macro"
            | "override"
            | "priv"
            | "typeof"
            | "unsized"
            | "virtual"
            | "try"
    )
}

/// A Rust string literal holding `text`.
fn quote(text: &str) -> String {
    format!("{text:?}")
}

/// A doc attribute showing `text` verbatim, fenced so rustdoc reads no markup in it.
fn doc(text: &str) -> String {
    format!("#[doc = {}]", quote(&format!("```text\n{text}\n```")))
}

fn emit(languages: &[Language], keys: &BTreeMap<String, Entry>, serde: bool) -> String {
    let base = &languages[0];
    let fixed: Vec<(&String, &Entry)> = keys
        .iter()
        .filter(|(_, entry)| entry.parameters.is_empty())
        .collect();
    let valued: Vec<(&String, &Entry)> = keys
        .iter()
        .filter(|(_, entry)| !entry.parameters.is_empty())
        .collect();
    let count = languages.len();
    let mut out = String::new();
    let w = &mut out;

    let _ = writeln!(
        w,
        "// Generated by atelier-i18n from the language files. Edit those, not this.\n"
    );

    // Language.
    let _ = writeln!(w, "/// A language the interface is available in.");
    let _ = writeln!(
        w,
        "#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]"
    );
    if serde {
        let _ = writeln!(w, "#[derive(::serde::Serialize, ::serde::Deserialize)]");
    }
    let _ = writeln!(w, "pub enum Language {{");
    for (index, language) in languages.iter().enumerate() {
        let _ = writeln!(
            w,
            "    #[doc = {}]",
            quote(&format!("{} (`{}`).", language.endonym, language.tag))
        );
        if index == 0 {
            let _ = writeln!(w, "    #[default]");
        }
        if serde {
            let _ = writeln!(
                w,
                "    #[serde(rename = {}, alias = {})]",
                quote(&language.tag),
                quote(&language.variant.to_ascii_lowercase())
            );
        }
        let _ = writeln!(w, "    {},", language.variant);
    }
    let _ = writeln!(w, "}}\n");

    let all: Vec<String> = languages
        .iter()
        .map(|l| format!("Language::{}", l.variant))
        .collect();
    let codes: Vec<String> = languages.iter().map(|l| quote(&l.tag)).collect();
    let _ = writeln!(
        w,
        "impl Language {{
    /// Every language, the fallback first: the order a picker lists them in.
    pub const ALL: [Language; {count}] = [{all}];

    /// The default, and what is shown when nothing the person reads is available.
    pub const FALLBACK: Language = Language::{fallback};

    const CODES: [&'static str; {count}] = [{codes}];

    /// The language's tag, which names its file: `en`, `ja`, `pt-BR`.
    pub const fn code(self) -> &'static str {{
        Self::CODES[self as usize]
    }}

    /// The language's name in itself, which is what a language picker must show.
    pub const fn endonym(self) -> &'static str {{
        match self {{",
        all = all.join(", "),
        fallback = base.variant,
        codes = codes.join(", "),
    );
    for language in languages {
        let _ = writeln!(
            w,
            "            Language::{} => {},",
            language.variant,
            quote(&language.endonym)
        );
    }
    let _ = writeln!(
        w,
        "        }}
    }}

    /// The language that best serves a locale such as `ja_JP.UTF-8` or `en-GB`.
    pub fn from_locale(locale: &str) -> Option<Language> {{
        ::atelier_i18n::negotiate(locale, &Self::CODES).map(|index| Self::ALL[index])
    }}

    /// The best language for what the environment and the system ask for, or the fallback.
    pub fn from_system() -> Language {{
        ::atelier_i18n::negotiate_first(&::atelier_i18n::preferred_locales(), &Self::CODES)
            .map_or(Self::FALLBACK, |index| Self::ALL[index])
    }}

    /// A stored preference, where `None` means \"follow the system\".
    pub fn resolve(preference: Option<Language>) -> Language {{
        preference.unwrap_or_else(Self::from_system)
    }}
}}
"
    );

    // Key.
    let _ = writeln!(w, "/// A fixed string in the interface.");
    let _ = writeln!(w, "#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]");
    let _ = writeln!(w, "pub enum Key {{");
    for (path, entry) in &fixed {
        let _ = writeln!(w, "    {}", doc(&literal(&base.strings[*path])));
        let _ = writeln!(w, "    {},", entry.ident);
    }
    let _ = writeln!(w, "}}\n");

    let total = fixed.len();
    let _ = writeln!(w, "impl Key {{");
    let _ = writeln!(w, "    /// Every key, for tests that sweep the table.");
    let _ = write!(w, "    pub const ALL: &'static [Key] = &[");
    for (_, entry) in &fixed {
        let _ = write!(w, "Key::{}, ", entry.ident);
    }
    let _ = writeln!(w, "];\n");
    let _ = writeln!(w, "}}\n");
    // Statics rather than consts, so the tables exist once instead of at every use.
    if total > 0 {
        let _ = write!(w, "static KEY_NAMES: [&str; {total}] = [");
        for (path, _) in &fixed {
            let _ = write!(w, "{}, ", quote(path));
        }
        let _ = writeln!(w, "];\n");
        let _ = writeln!(w, "static KEY_TEXT: [[&str; {total}]; {count}] = [");
        for language in languages {
            let _ = write!(w, "    [");
            for (path, _) in &fixed {
                let _ = write!(w, "{}, ", quote(&literal(&language.strings[*path])));
            }
            let _ = writeln!(w, "],");
        }
        let _ = writeln!(w, "];\n");
    }
    let _ = writeln!(w, "impl Key {{");
    if total == 0 {
        let _ = writeln!(
            w,
            "    /// This string in `language`.
    pub fn get(self, _language: Language) -> &'static str {{
        match self {{}}
    }}

    /// The key as its file writes it, such as `piano_roll.title`.
    pub fn name(self) -> &'static str {{
        match self {{}}
    }}
}}
"
        );
    } else {
        let _ = writeln!(
            w,
            "    /// This string in `language`.
    pub fn get(self, language: Language) -> &'static str {{
        KEY_TEXT[language as usize][self as usize]
    }}

    /// The key as its file writes it, such as `piano_roll.title`.
    pub fn name(self) -> &'static str {{
        KEY_NAMES[self as usize]
    }}
}}
"
        );
    }
    let _ = writeln!(
        w,
        "/// The text for `key` in `language`: [`Key::get`] with the language last.
pub fn t(key: Key, language: Language) -> &'static str {{
    key.get(language)
}}
"
    );

    // Messages.
    let _ = writeln!(w, "/// Strings that take values, one function each.");
    let _ = writeln!(w, "pub mod messages {{");
    if !valued.is_empty() {
        let _ = writeln!(w, "    use super::Language;");
    }
    for (path, entry) in &valued {
        let _ = writeln!(w);
        let _ = writeln!(w, "    {}", doc(&base.strings[*path]));
        let parameters: Vec<String> = entry
            .parameters
            .iter()
            .map(|name| format!("{name}: impl ::core::fmt::Display"))
            .collect();
        let _ = writeln!(
            w,
            "    pub fn {}(language: Language, {}) -> ::std::string::String {{",
            entry.function,
            parameters.join(", ")
        );
        let _ = writeln!(w, "        match language {{");
        for language in languages {
            let _ = writeln!(
                w,
                "            Language::{} => ::std::format!({}),",
                language.variant,
                quote(&language.strings[*path])
            );
        }
        let _ = writeln!(w, "        }}");
        let _ = writeln!(w, "    }}");
    }
    let _ = writeln!(w, "}}");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const EN: &str = r#"
_language = { variant = "English", endonym = "English" }

mixer = "Mixer"
braces = "Use {{ and }}"
track_count = "{count} tracks"

[piano_roll]
title = "Piano roll — {clip}"
moved = "Moved {count} notes to bar {bar}"
"#;

    const JA: &str = r#"
_language = { variant = "Japanese", endonym = "日本語" }

mixer = "ミキサー"
braces = "{{ と }} を使う"
track_count = "{count}トラック"

[piano_roll]
title = "ピアノロール — {clip}"
moved = "{bar}小節目へ{count}音を移動"
"#;

    fn source(tag: &str, text: &str) -> Source {
        Source {
            tag: tag.into(),
            text: text.into(),
        }
    }

    /// `EN` with `lines` added at the top level, ahead of its tables.
    fn english_with(lines: &str) -> String {
        EN.replacen("\nmixer = ", &format!("\n{lines}\nmixer = "), 1)
    }

    fn problems(sources: &[Source]) -> Vec<String> {
        render(sources, "en", false)
            .expect_err("these files should not agree")
            .problems
    }

    #[test]
    fn agreeing_files_render_every_kind_of_item() {
        let code = render(&[source("ja", JA), source("en", EN)], "en", false).unwrap();
        assert!(
            code.contains(
                "#[default]\n    English,\n    #[doc = \"日本語 (`ja`).\"]\n    Japanese,"
            )
        );
        assert!(code.contains("    Mixer,"));
        assert!(code.contains("    Braces,"));
        // `{{` is a brace in a fixed string and stays escaped in a format string.
        assert!(code.contains("\"Use { and }\""));
        assert!(
            code.contains(
                "pub fn track_count(language: Language, count: impl ::core::fmt::Display)"
            )
        );
        // Parameters follow the fallback's order even where a translation reverses it.
        assert!(code.contains(
            "pub fn piano_roll_moved(language: Language, count: impl ::core::fmt::Display, bar: impl ::core::fmt::Display)"
        ));
        assert!(code.contains("::std::format!(\"{bar}小節目へ{count}音を移動\")"));
        assert!(!code.contains("serde"));
    }

    #[test]
    fn serde_names_a_language_by_its_tag_and_accepts_its_variant() {
        let code = render(&[source("en", EN), source("ja", JA)], "en", true).unwrap();
        assert!(code.contains("#[serde(rename = \"ja\", alias = \"japanese\")]"));
    }

    #[test]
    fn a_missing_or_extra_key_names_the_file_and_the_key() {
        let ja = JA.replace("mixer = \"ミキサー\"\n", "mixr = \"ミキサー\"\n");
        let found = problems(&[source("en", EN), source("ja", &ja)]);
        assert_eq!(
            found,
            [
                "ja.toml: `mixer` is missing (it is in en.toml)",
                "ja.toml: `mixr` is not in en.toml",
            ]
        );
    }

    #[test]
    fn translations_must_use_the_same_placeholders() {
        let ja = JA.replace("{count}トラック", "{number}トラック");
        let found = problems(&[source("en", EN), source("ja", &ja)]);
        assert_eq!(
            found,
            ["ja.toml: `track_count` uses {number} but en.toml uses {count}"]
        );
        let ja = JA.replace("\"ミキサー\"", "\"ミキサー{x}\"");
        let found = problems(&[source("en", EN), source("ja", &ja)]);
        assert_eq!(
            found,
            ["ja.toml: `mixer` uses {x} but en.toml uses no placeholder"]
        );
    }

    #[test]
    fn placeholders_are_named_and_braces_are_balanced() {
        assert_eq!(
            parse("{bpm:.1} BPM").unwrap(),
            [
                Piece::Value { name: "bpm".into() },
                Piece::Text(" BPM".into())
            ]
        );
        assert!(parse("{} tracks").unwrap_err().contains("without a name"));
        assert!(parse("{0} tracks").unwrap_err().contains("snake_case"));
        assert!(parse("{count tracks").unwrap_err().contains("never closed"));
        assert!(parse("tracks}").unwrap_err().contains("lone"));
    }

    #[test]
    fn a_file_without_its_language_table_or_fallback_is_refused() {
        let found = problems(&[source("en", "mixer = \"Mixer\"")]);
        assert_eq!(
            found,
            ["en.toml: opens with no [_language] table naming `variant` and `endonym`"]
        );
        let error = render(&[source("ja", JA)], "en", false).unwrap_err();
        assert_eq!(
            error.problems,
            ["there is no en.toml for the fallback language"]
        );
    }

    #[test]
    fn keys_that_collide_or_are_not_text_are_refused() {
        let en = format!("{EN}\n[track]\ncount = \"{{count}} tracks\"\n");
        // (A table at the end is a table of its own, so this one is reached as `track.count`.)
        let found = problems(&[source("en", &en)]);
        assert_eq!(
            found,
            ["en.toml: `track_count` and `track.count` both become `TrackCount`"]
        );
        let en = english_with("level = 3");
        assert_eq!(
            problems(&[source("en", &en)]),
            ["en.toml: `level` is not a string; every entry is text or a table of them"]
        );
    }

    #[test]
    fn keys_caught_inside_the_language_table_are_pointed_out() {
        let en = "[_language]\nvariant = \"English\"\nendonym = \"English\"\nmixer = \"Mixer\"\n";
        let found = problems(&[source("en", en)]);
        assert_eq!(found.len(), 1);
        assert!(
            found[0].starts_with("en.toml: `mixer` is inside [_language]"),
            "{found:?}"
        );
    }

    #[test]
    fn names_convert_between_cases() {
        assert_eq!(camel("piano_roll.title"), "PianoRollTitle");
        assert_eq!(camel("IssueReport"), "IssueReport");
        assert_eq!(snake("PianoRollTitle"), "piano_roll_title");
        assert_eq!(snake("Cc74Name"), "cc74_name");
    }

    #[test]
    fn a_rust_keyword_cannot_name_a_message_or_a_value() {
        let en = english_with("type = \"Type {name}\"\nfoo = \"{match}\"");
        let found = problems(&[source("en", &en)]);
        assert!(
            found.iter().any(|p| p.contains("`type` is a Rust keyword")),
            "{found:?}"
        );
        assert!(
            found.iter().any(|p| p.contains("cannot be called `match`")),
            "{found:?}"
        );
    }
}
