use crate::data;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub mode: String,
    pub seconds: u32,
    pub words: usize,
    pub language: String,
    pub theme: String,
    pub punctuation: bool,
    pub numbers: bool,
    pub font_size: u16,
    pub live_wpm: bool,
    pub blind: bool,
    pub freedom: bool,
    pub difficulty: String,
    pub caret: String,
    pub quote_length: String,
    pub custom: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mode: "time".into(),
            seconds: 30,
            words: 25,
            language: "english".into(),
            theme: "serika_dark".into(),
            punctuation: false,
            numbers: false,
            font_size: 32,
            live_wpm: false,
            blind: false,
            freedom: false,
            difficulty: "normal".into(),
            caret: "line".into(),
            quote_length: "short".into(),
            custom: "The quick brown fox jumps over the lazy dog.".into(),
        }
    }
}

impl Config {
    pub fn validate(&mut self) {
        if !["time", "words", "quote", "zen", "custom"].contains(&self.mode.as_str()) {
            self.mode = "time".into();
        }
        if !data::themes().contains_key(&self.theme) {
            self.theme = "serika_dark".into();
        }
        if !data::LANGUAGES
            .iter()
            .any(|(name, _)| *name == self.language)
        {
            self.language = "english".into();
        }
        self.seconds = self.seconds.clamp(1, 3600);
        self.words = self.words.clamp(1, 10000);
        self.font_size = self.font_size.clamp(16, 64);
        if !["normal", "expert", "master"].contains(&self.difficulty.as_str()) {
            self.difficulty = "normal".into();
        }
        if !["line", "block", "underline"].contains(&self.caret.as_str()) {
            self.caret = "line".into();
        }
        if !["short", "medium", "long", "all"].contains(&self.quote_length.as_str()) {
            self.quote_length = "short".into();
        }
        if self.custom.trim().is_empty() {
            self.custom = Self::default().custom;
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Sample {
    pub second: f64,
    pub wpm: f64,
    pub raw: f64,
    pub errors: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Stats {
    pub wpm: f64,
    pub raw: f64,
    pub accuracy: f64,
    pub consistency: f64,
    pub correct: usize,
    pub incorrect: usize,
    pub extra: usize,
    pub missed: usize,
    pub duration: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Result {
    #[serde(flatten)]
    pub stats: Stats,
    pub date: f64,
    pub mode: String,
    pub limit: usize,
    pub language: String,
    pub punctuation: bool,
    pub numbers: bool,
    pub failed: bool,
    #[serde(default)]
    pub samples: Vec<Sample>,
}

pub struct Store {
    pub directory: PathBuf,
}
impl Store {
    pub fn new(directory: Option<PathBuf>) -> Self {
        let directory = directory.unwrap_or_else(|| {
            let base = std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share")
                });
            base.join("monkeytype-tui")
        });
        Self { directory }
    }
    pub fn config(&self) -> Config {
        let mut c = fs::read(self.directory.join("config.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Config>(&b).ok())
            .unwrap_or_default();
        c.validate();
        c
    }
    pub fn history(&self) -> Vec<Result> {
        fs::read(self.directory.join("history.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }
    pub fn save(&self, name: &str, value: &impl Serialize) -> std::io::Result<()> {
        fs::create_dir_all(&self.directory)?;
        let temp = self.directory.join(format!("{name}.tmp"));
        fs::write(&temp, serde_json::to_vec(value)?)?;
        fs::rename(temp, self.directory.join(name))
    }
}

pub struct Test {
    pub config: Config,
    pub targets: Vec<Vec<char>>,
    pub inputs: Vec<Vec<char>>,
    pub index: usize,
    pub started: Option<f64>,
    pub ended: Option<f64>,
    pub attempts: usize,
    pub correct_attempts: usize,
    pub samples: Vec<Sample>,
    input_times: Vec<f64>,
    pub source: String,
    pub failed: bool,
    rng: u64,
}

impl Test {
    pub fn new(config: Config, seed: u64) -> Self {
        let mut t = Self {
            config,
            targets: vec![],
            inputs: vec![vec![]],
            index: 0,
            started: None,
            ended: None,
            attempts: 0,
            correct_attempts: 0,
            samples: vec![],
            input_times: vec![],
            source: String::new(),
            failed: false,
            rng: seed,
        };
        match t.config.mode.as_str() {
            "quote" => {
                let bounds = match t.config.quote_length.as_str() {
                    "medium" => (101, 300),
                    "long" => (301, 600),
                    "all" => (0, 10000),
                    _ => (0, 100),
                };
                let candidates: Vec<_> = data::quotes()
                    .iter()
                    .filter(|q| q.length >= bounds.0 && q.length <= bounds.1)
                    .collect();
                let quote = candidates[t.random(candidates.len())];
                t.targets = quote
                    .text
                    .split_whitespace()
                    .map(|w| w.chars().collect())
                    .collect();
                t.source = quote.source.clone();
            }
            "custom" => {
                t.targets = t
                    .config
                    .custom
                    .split_whitespace()
                    .map(|w| w.chars().collect())
                    .collect()
            }
            "zen" => t.targets = vec![vec![]; 100],
            _ => t.extend(if t.config.mode == "words" {
                t.config.words
            } else {
                200
            }),
        }
        if t.targets.is_empty() {
            t.targets.push("monkeytype".chars().collect());
        }
        t
    }
    fn random(&mut self, n: usize) -> usize {
        self.rng = self.rng.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        ((z ^ (z >> 31)) % (n.max(1) as u64)) as usize
    }
    fn extend(&mut self, count: usize) {
        let words = &data::dictionaries()[&self.config.language];
        for _ in 0..count {
            let mut word: Vec<char> = words[self.random(words.len())].chars().collect();
            while self.targets.last() == Some(&word) && words.len() > 1 {
                word = words[self.random(words.len())].chars().collect();
            }
            if self.config.numbers && self.random(100) < 12 {
                word = self.random(10000).to_string().chars().collect();
            }
            if self.config.punctuation {
                if (self.targets.is_empty()
                    || self
                        .targets
                        .last()
                        .and_then(|w| w.last())
                        .is_some_and(|c| ".!?".contains(*c)))
                    && let Some(first) = word.first_mut()
                {
                    *first = first.to_uppercase().next().unwrap_or(*first);
                }
                if self.random(100) < 20 {
                    word.push([',', '.', '.', '?', '!', ';', ':'][self.random(7)]);
                }
            }
            self.targets.push(word);
        }
    }
    pub fn active(&self) -> bool {
        self.started.is_some() && self.ended.is_none()
    }
    pub fn finished(&self) -> bool {
        self.ended.is_some()
    }
    pub fn current(&self) -> &[char] {
        &self.inputs[self.index]
    }
    pub fn target(&self) -> &[char] {
        self.targets
            .get(self.index)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
    pub fn elapsed(&self, now: f64) -> f64 {
        self.started
            .map(|s| (self.ended.unwrap_or(now) - s).max(0.))
            .unwrap_or(0.)
    }
    pub fn finish(&mut self, now: f64) {
        if self.started.is_some() && !self.finished() {
            self.ended = Some(now);
        }
    }
    pub fn tick(&mut self, now: f64) {
        if !self.active() {
            return;
        }
        if self.config.mode == "time" && self.elapsed(now) >= self.config.seconds as f64 {
            self.finish(self.started.unwrap() + self.config.seconds as f64);
        }
        if self.elapsed(now).floor() as usize > self.samples.len() {
            let stats = self.stats(now);
            self.samples.push(Sample {
                second: stats.duration,
                wpm: stats.wpm,
                raw: stats.raw,
                errors: self.attempts - self.correct_attempts,
            });
        }
    }
    pub fn insert(&mut self, ch: char, now: f64) {
        self.tick(now);
        if self.finished() || (ch == ' ' && self.current().is_empty()) {
            return;
        }
        self.started.get_or_insert(now);
        self.input_times.push(now);
        let zen = self.config.mode == "zen";
        let good = zen
            || if ch == ' ' {
                self.current() == self.target()
            } else {
                self.target().get(self.current().len()) == Some(&ch)
            };
        self.attempts += 1;
        self.correct_attempts += usize::from(good);
        if ch == ' ' {
            if self.config.difficulty == "expert" && !good {
                self.failed = true;
                self.finish(now);
                return;
            }
            self.index += 1;
            self.inputs.push(vec![]);
            if self.index >= self.targets.len() {
                if zen {
                    self.targets.extend(vec![vec![]; 100]);
                } else if self.config.mode == "time" {
                    self.extend(100);
                } else {
                    self.finish(now);
                }
            } else if self.config.mode == "time" && self.targets.len() - self.index < 30 {
                self.extend(100);
            }
        } else {
            if self.current().len() < 256 {
                self.inputs[self.index].push(ch);
            }
            if !good && self.config.difficulty == "master" {
                self.failed = true;
                self.finish(now);
            } else if ["words", "quote", "custom"].contains(&self.config.mode.as_str())
                && self.index == self.targets.len() - 1
                && self.current() == self.target()
            {
                self.finish(now);
            }
        }
    }
    pub fn backspace(&mut self, word: bool, now: f64) {
        self.tick(now);
        if self.finished() {
            return;
        }
        if !self.current().is_empty() {
            if word {
                self.inputs[self.index].clear();
            } else {
                self.inputs[self.index].pop();
            }
        } else if self.index > 0
            && (self.config.freedom
                || self.config.mode == "zen"
                || self.inputs[self.index - 1] != self.targets[self.index - 1])
        {
            self.inputs.pop();
            self.index -= 1;
        }
    }
    pub fn stats(&self, now: f64) -> Stats {
        let mut s = Stats {
            accuracy: 100.,
            consistency: 0.,
            duration: self.elapsed(now),
            ..Stats::default()
        };
        let zen = self.config.mode == "zen";
        let mut wpm_chars = 0;
        let mut raw_chars = self.index;
        for (i, typed) in self.inputs.iter().enumerate() {
            let target = self.targets.get(i).map(Vec::as_slice).unwrap_or(&[]);
            raw_chars += typed.len();
            for (j, ch) in typed.iter().enumerate() {
                if zen || target.get(j) == Some(ch) {
                    s.correct += 1;
                } else if j < target.len() {
                    s.incorrect += 1;
                } else {
                    s.extra += 1;
                }
            }
            if i < self.index {
                if !zen && typed != target {
                    s.extra += 1;
                }
                if !zen {
                    s.missed += target.len().saturating_sub(typed.len());
                }
            }
            let partial = i == self.index
                && (self.active() || self.config.mode == "time")
                && target.starts_with(typed);
            if zen || typed == target || partial {
                wpm_chars += typed.len() + usize::from(i < self.index);
            }
        }
        s.correct = wpm_chars;
        if s.duration > 0. {
            s.wpm = wpm_chars as f64 * 12. / s.duration;
            s.raw = raw_chars as f64 * 12. / s.duration;
        }
        if self.attempts > 0 {
            s.accuracy = 100. * self.correct_attempts as f64 / self.attempts as f64;
        }
        let mut boundaries: Vec<f64> = (1..=s.duration.floor() as usize)
            .map(|i| i as f64)
            .collect();
        if self.finished() && self.config.mode != "time" && s.duration.fract() >= 0.5 {
            boundaries.push(s.duration);
        }
        let mut speeds = Vec::with_capacity(boundaries.len());
        let mut event_index = 0;
        let mut previous = 0.;
        for boundary in boundaries {
            let mut count = 0;
            while event_index < self.input_times.len()
                && self.input_times[event_index] - self.started.unwrap_or(0.) <= boundary
            {
                count += 1;
                event_index += 1;
            }
            speeds.push((count as f64 * 12. / (boundary - previous)).round());
            previous = boundary;
        }
        if !speeds.is_empty() {
            let mean = speeds.iter().sum::<f64>() / speeds.len() as f64;
            if mean > 0. {
                let variance =
                    speeds.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / speeds.len() as f64;
                let cov = variance.sqrt() / mean;
                s.consistency = 100. * (1. - (cov + cov.powi(3) / 3. + cov.powi(5) / 5.).tanh());
            }
        }
        s
    }
    pub fn result(&self, now: f64) -> Result {
        Result {
            stats: self.stats(now),
            date: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
            mode: self.config.mode.clone(),
            limit: if self.config.mode == "time" {
                self.config.seconds as usize
            } else {
                self.targets.len()
            },
            language: self.config.language.clone(),
            punctuation: self.config.punctuation,
            numbers: self.config.numbers,
            failed: self.failed,
            samples: self.samples.clone(),
        }
    }
    pub fn remaining(&self, now: f64) -> String {
        match self.config.mode.as_str() {
            "time" => format!(
                "{}",
                (self.config.seconds as f64 - self.elapsed(now))
                    .max(0.)
                    .ceil() as u32
            ),
            "zen" => self.index.to_string(),
            _ => format!(
                "{}/{}",
                self.index.min(self.targets.len()),
                self.targets.len()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn custom() -> Test {
        Test::new(
            Config {
                mode: "custom".into(),
                custom: "hello world".into(),
                ..Config::default()
            },
            42,
        )
    }
    #[test]
    fn first_key_starts_last_key_finishes() {
        let mut t = custom();
        t.tick(40.);
        assert!(t.started.is_none());
        for ch in "hello world".chars() {
            t.insert(ch, 50.);
        }
        assert!(t.finished());
        assert_eq!(t.stats(60.).correct, 11);
    }
    #[test]
    fn correction_costs_accuracy() {
        let mut t = custom();
        t.insert('x', 0.);
        t.backspace(false, 1.);
        for ch in "hello world".chars() {
            t.insert(ch, 10.);
        }
        let s = t.stats(10.);
        assert!((s.accuracy - 91.666666).abs() < 0.001);
        assert_eq!(s.incorrect, 0);
        assert!((s.wpm - 13.2).abs() < 0.001);
    }
    #[test]
    fn deadline_is_exact() {
        let mut t = Test::new(
            Config {
                seconds: 15,
                ..Config::default()
            },
            42,
        );
        t.insert(t.target()[0], 0.);
        t.tick(19.);
        assert_eq!(t.elapsed(19.), 15.);
        let attempts = t.attempts;
        t.insert('x', 20.);
        assert_eq!(t.attempts, attempts);
    }
    #[test]
    fn backtracking_respects_freedom() {
        let mut t = custom();
        for ch in "hellx ".chars() {
            t.insert(ch, 0.);
        }
        t.backspace(false, 1.);
        assert_eq!(t.index, 0);
        t.backspace(false, 1.);
        t.insert('o', 1.);
        t.insert(' ', 1.);
        t.backspace(false, 1.);
        assert_eq!(t.index, 1);
        t.config.freedom = true;
        t.backspace(false, 1.);
        assert_eq!(t.index, 0);
    }
    #[test]
    fn master_fails_immediately() {
        let mut t = custom();
        t.config.difficulty = "master".into();
        t.insert('x', 0.);
        assert!(t.failed && t.finished());
    }
    #[test]
    fn unicode_counts_characters_not_bytes() {
        let mut t = Test::new(
            Config {
                mode: "custom".into(),
                custom: "été été".into(),
                ..Config::default()
            },
            1,
        );
        for ch in "été été".chars() {
            t.insert(ch, 1.);
        }
        assert_eq!(t.stats(1.).correct, 7);
        assert_eq!(t.stats(1.).accuracy, 100.);
    }
    #[test]
    fn leading_space_does_not_start() {
        let mut t = custom();
        t.insert(' ', 0.);
        assert!(t.started.is_none());
    }
    #[test]
    fn words_are_deterministic_without_neighbors_repeating() {
        let c = Config {
            mode: "words".into(),
            words: 100,
            ..Config::default()
        };
        let a = Test::new(c.clone(), 5);
        let b = Test::new(c, 5);
        assert_eq!(a.targets, b.targets);
        assert!(a.targets.windows(2).all(|w| w[0] != w[1]));
    }

    #[test]
    fn timed_test_credits_a_correct_partial_last_word() {
        let mut t = Test::new(
            Config {
                seconds: 15,
                ..Config::default()
            },
            42,
        );
        let first = t.target()[0];
        t.insert(first, 0.);
        t.tick(15.);
        assert!((t.stats(15.).wpm - 0.8).abs() < 0.001);
        assert_eq!(t.stats(15.).correct, 1);
    }

    #[test]
    fn wrong_words_do_not_receive_correct_word_credit() {
        let mut t = custom();
        for ch in "hellx ".chars() {
            t.insert(ch, 0.);
        }
        t.finish(10.);
        let stats = t.stats(10.);
        assert_eq!(stats.correct, 0);
        assert_eq!(stats.incorrect, 1);
        assert_eq!(stats.extra, 1); // separator after an incorrect word
        assert_eq!(stats.wpm, 0.);
        assert_eq!(stats.raw, 7.2);
    }

    #[test]
    fn zen_runs_until_explicit_finish() {
        let mut t = Test::new(
            Config {
                mode: "zen".into(),
                ..Config::default()
            },
            1,
        );
        for ch in "anything at all".chars() {
            t.insert(ch, 0.);
        }
        assert!(!t.finished());
        t.finish(10.);
        assert_eq!(t.stats(10.).wpm, 18.);
        assert_eq!(t.stats(10.).accuracy, 100.);
    }

    #[test]
    fn expert_fails_on_submission_not_on_a_wrong_letter() {
        let mut t = custom();
        t.config.difficulty = "expert".into();
        t.insert('x', 0.);
        assert!(!t.finished());
        t.insert(' ', 1.);
        assert!(t.failed && t.finished());
    }
}
