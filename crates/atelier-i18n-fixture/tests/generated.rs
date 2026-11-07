use atelier_i18n_fixture::{Key, Language, messages, t};

#[test]
fn fixed_strings_come_back_in_each_language() {
    assert_eq!(Key::Mixer.get(Language::English), "Mixer");
    assert_eq!(t(Key::Mixer, Language::Japanese), "ミキサー");
    assert_eq!(
        Key::LiteralBraces.get(Language::English),
        "Wrap a chord in { and }"
    );
    assert_eq!(
        Key::MultiLine.get(Language::English),
        "First line\nSecond line with \"quotes\""
    );
    assert_eq!(Key::Mixer.name(), "mixer");
    assert_eq!(Key::ALL.len(), 3);
    for key in Key::ALL {
        for language in Language::ALL {
            assert!(!key.get(language).is_empty());
        }
    }
}

#[test]
fn messages_take_their_values_by_name_in_each_order() {
    assert_eq!(
        messages::piano_roll_moved(Language::English, 3, 12),
        "Moved 3 notes to bar 12"
    );
    assert_eq!(
        messages::piano_roll_moved(Language::Japanese, 3, 12),
        "12小節目へ3音を移動"
    );
    assert_eq!(
        messages::piano_roll_title(Language::Japanese, "Verse"),
        "ピアノロール — Verse"
    );
    assert_eq!(messages::tempo(Language::English, 119.96), "120.0 BPM");
    assert_eq!(messages::track_count(Language::Japanese, 8), "8トラック");
}

#[test]
fn languages_know_their_tags_names_and_locales() {
    assert_eq!(Language::ALL, [Language::English, Language::Japanese]);
    assert_eq!(Language::FALLBACK, Language::English);
    assert_eq!(Language::default(), Language::English);
    assert_eq!(Language::Japanese.code(), "ja");
    assert_eq!(Language::Japanese.endonym(), "日本語");
    assert_eq!(
        Language::from_locale("ja_JP.UTF-8"),
        Some(Language::Japanese)
    );
    assert_eq!(Language::from_locale("en-GB"), Some(Language::English));
    assert_eq!(Language::from_locale("de_DE"), None);
    assert_eq!(
        Language::resolve(Some(Language::Japanese)),
        Language::Japanese
    );
    assert!(Language::ALL.contains(&Language::from_system()));
}

#[test]
fn a_language_is_stored_as_its_tag_and_read_by_its_variant_too() {
    assert_eq!(
        serde_json::to_string(&Language::Japanese).unwrap(),
        "\"ja\""
    );
    for stored in ["\"ja\"", "\"japanese\""] {
        assert_eq!(
            serde_json::from_str::<Language>(stored).unwrap(),
            Language::Japanese
        );
    }
}
