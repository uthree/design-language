fn main() {
    atelier_i18n::build::Catalog::new("locales")
        .fallback("en")
        .serde(true)
        .generate()
        .unwrap_or_else(|error| panic!("{error}"));
}
