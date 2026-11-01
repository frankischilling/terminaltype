use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};

pub const MONO: &[u8] = include_bytes!("../assets/RobotoMono-Regular.ttf");
pub const LOGO_FONT: &[u8] = include_bytes!("../assets/LexendDeca-Regular.ttf");
pub const LOGO: &[u8] = include_bytes!("../assets/logo.png");
pub const ICONS: &[u8] = include_bytes!("../assets/fa-solid-900.ttf");
pub const LANGUAGES: &[(&str, &str)] = &[
    ("english", include_str!("../assets/languages/english.json")),
    (
        "english_1k",
        include_str!("../assets/languages/english_1k.json"),
    ),
    (
        "english_5k",
        include_str!("../assets/languages/english_5k.json"),
    ),
    ("spanish", include_str!("../assets/languages/spanish.json")),
    ("french", include_str!("../assets/languages/french.json")),
    ("german", include_str!("../assets/languages/german.json")),
    ("italian", include_str!("../assets/languages/italian.json")),
    (
        "portuguese",
        include_str!("../assets/languages/portuguese.json"),
    ),
    (
        "code_python",
        include_str!("../assets/languages/code_python.json"),
    ),
    (
        "code_javascript",
        include_str!("../assets/languages/code_javascript.json"),
    ),
    (
        "code_rust",
        include_str!("../assets/languages/code_rust.json"),
    ),
];

pub fn dictionaries() -> &'static BTreeMap<String, Vec<String>> {
    static DATA: OnceLock<BTreeMap<String, Vec<String>>> = OnceLock::new();
    DATA.get_or_init(|| {
        #[derive(Deserialize)]
        struct Dictionary {
            words: Vec<String>,
        }
        LANGUAGES
            .iter()
            .map(|(name, json)| {
                let dictionary: Dictionary =
                    serde_json::from_str(json).expect("bundled dictionary");
                (name.to_string(), dictionary.words)
            })
            .collect()
    })
}

#[derive(Clone, Deserialize)]
pub struct Quote {
    pub text: String,
    pub source: String,
    pub length: usize,
}

pub fn quotes() -> &'static [Quote] {
    static DATA: OnceLock<Vec<Quote>> = OnceLock::new();
    DATA.get_or_init(|| {
        #[derive(Deserialize)]
        struct Quotes {
            quotes: Vec<Quote>,
        }
        serde_json::from_str::<Quotes>(include_str!("../assets/quotes.json"))
            .expect("bundled quotes")
            .quotes
    })
}

pub type Color = [u8; 3];

pub fn hex(value: &str) -> Color {
    let value = value.trim_start_matches('#');
    if value.len() == 3 {
        let digits: Vec<_> = value
            .chars()
            .map(|c| c.to_digit(16).unwrap_or(0) as u8 * 17)
            .collect();
        return [digits[0], digits[1], digits[2]];
    }
    let read = |i| u8::from_str_radix(value.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0);
    [read(0), read(2), read(4)]
}

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub bg: Color,
    pub main: Color,
    pub caret: Color,
    pub sub: Color,
    pub panel: Color,
    pub text: Color,
    pub error: Color,
    pub extra: Color,
}

pub fn themes() -> &'static BTreeMap<String, Theme> {
    static DATA: OnceLock<BTreeMap<String, Theme>> = OnceLock::new();
    DATA.get_or_init(|| {
        let raw: BTreeMap<String, BTreeMap<String, String>> =
            serde_json::from_str(include_str!("../assets/themes.json")).expect("bundled themes");
        raw.into_iter()
            .map(|(name, c)| {
                let get = |key: &str| hex(c.get(key).map(String::as_str).unwrap_or("#646669"));
                (
                    name,
                    Theme {
                        bg: get("bg"),
                        main: get("main"),
                        caret: get("caret"),
                        sub: get("sub"),
                        panel: get("subAlt"),
                        text: get("text"),
                        error: get("error"),
                        extra: get("errorExtra"),
                    },
                )
            })
            .collect()
    })
}
