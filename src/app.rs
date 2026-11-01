use crate::{
    data,
    gfx::Rect,
    model::{Config, Result, Store, Test},
};
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Test,
    Settings,
    History,
    About,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Home,
    Restart(bool),
    Page(Page),
    Palette(String),
    Set(&'static str, String),
    Toggle(&'static str),
    Input(String),
    SaveInput,
    Review,
    Quit,
}
pub type OptionRow = (String, Action, String);

pub struct App {
    pub config: Config,
    pub test: Test,
    pub store: Store,
    pub history: Vec<Result>,
    pub page: Page,
    pub modal: String,
    pub query: String,
    pub selected: usize,
    pub scroll: usize,
    pub review: bool,
    pub running: bool,
    pub dirty: bool,
    pub pointer: (f32, f32),
    pub toast: String,
    pub toast_until: f64,
    pub generation: u64,
    pub saved: bool,
    pub restart_armed: bool,
    pub hits: Vec<(Rect, Action)>,
    seed: u64,
}
impl App {
    pub fn new(store: Store, config: Config, seed: u64) -> Self {
        let history = store.history();
        let test = Test::new(config.clone(), seed);
        Self {
            config,
            test,
            store,
            history,
            page: Page::Test,
            modal: String::new(),
            query: String::new(),
            selected: 0,
            scroll: 0,
            review: false,
            running: true,
            dirty: true,
            pointer: (-1., -1.),
            toast: String::new(),
            toast_until: 0.,
            generation: 0,
            saved: false,
            restart_armed: false,
            hits: vec![],
            seed,
        }
    }
    pub fn notify(&mut self, message: impl Into<String>, now: f64) {
        self.toast = message.into();
        self.toast_until = now + 3.;
        self.dirty = true;
    }
    fn persist(&mut self, now: f64) {
        if let Err(error) = self.store.save("config.json", &self.config) {
            self.notify(format!("Could not save: {error}"), now);
        }
    }
    pub fn restart(&mut self, repeat: bool) {
        self.seed = self.seed.wrapping_add(1);
        let mut test = Test::new(self.config.clone(), self.seed);
        if repeat {
            test.targets = self.test.targets.clone();
            test.source = self.test.source.clone();
        }
        self.test = test;
        self.generation += 1;
        self.page = Page::Test;
        self.modal.clear();
        self.query.clear();
        self.review = false;
        self.saved = false;
        self.restart_armed = false;
        self.dirty = true;
        self.toast.clear();
    }
    pub fn palette(&mut self, kind: &str) {
        self.modal = kind.into();
        self.query.clear();
        self.selected = 0;
        self.dirty = true;
    }
    pub fn options(&self) -> Vec<OptionRow> {
        let options: Vec<OptionRow> = match self.modal.as_str() {
            "theme" => {
                let favorites = [
                    "serika_dark",
                    "serika",
                    "catppuccin",
                    "nord",
                    "dracula",
                    "rose_pine",
                    "gruvbox_dark",
                ];
                let mut names: Vec<&str> = favorites
                    .into_iter()
                    .filter(|t| data::themes().contains_key(*t))
                    .collect();
                names.extend(
                    data::themes()
                        .keys()
                        .map(String::as_str)
                        .filter(|t| !favorites.contains(t)),
                );
                names
                    .into_iter()
                    .map(|t| {
                        (
                            t.replace('_', " "),
                            Action::Set("theme", t.into()),
                            String::new(),
                        )
                    })
                    .collect()
            }
            "language" => data::LANGUAGES
                .iter()
                .map(|(v, _)| {
                    (
                        v.replace('_', " "),
                        Action::Set("language", (*v).into()),
                        String::new(),
                    )
                })
                .collect(),
            "seconds" | "words" => {
                let key = if self.modal == "seconds" {
                    "seconds"
                } else {
                    "words"
                };
                let values = if key == "seconds" {
                    [15, 30, 60, 120, 300]
                } else {
                    [10, 25, 50, 100, 200]
                };
                let mut rows: Vec<_> = values
                    .into_iter()
                    .map(|v| (v.to_string(), Action::Set(key, v.to_string()), key.into()))
                    .collect();
                rows.push(("custom…".into(), Action::Input(key.into()), String::new()));
                rows
            }
            _ => {
                let mut rows = vec![
                    (
                        "restart test".into(),
                        Action::Restart(false),
                        "ctrl+r".into(),
                    ),
                    (
                        "theme".into(),
                        Action::Palette("theme".into()),
                        self.config.theme.replace('_', " "),
                    ),
                    (
                        "language".into(),
                        Action::Palette("language".into()),
                        self.config.language.replace('_', " "),
                    ),
                    (
                        "time".into(),
                        Action::Palette("seconds".into()),
                        format!("{}s", self.config.seconds),
                    ),
                    (
                        "word count".into(),
                        Action::Palette("words".into()),
                        self.config.words.to_string(),
                    ),
                ];
                for mode in ["time", "words", "quote", "zen", "custom"] {
                    rows.push((
                        format!("mode {mode}"),
                        Action::Set("mode", mode.into()),
                        String::new(),
                    ));
                }
                rows.push((
                    "custom text".into(),
                    Action::Input("custom".into()),
                    "edit".into(),
                ));
                for (label, key, value) in [
                    ("punctuation", "punctuation", self.config.punctuation),
                    ("numbers", "numbers", self.config.numbers),
                    ("live wpm", "live_wpm", self.config.live_wpm),
                    ("blind mode", "blind", self.config.blind),
                    ("freedom mode", "freedom", self.config.freedom),
                ] {
                    rows.push((
                        label.into(),
                        Action::Toggle(key),
                        if value { "on" } else { "off" }.into(),
                    ));
                }
                rows.extend([
                    (
                        "settings".into(),
                        Action::Page(Page::Settings),
                        String::new(),
                    ),
                    (
                        "local history".into(),
                        Action::Page(Page::History),
                        format!("{} tests", self.history.len()),
                    ),
                    (
                        "about / shortcuts".into(),
                        Action::Page(Page::About),
                        String::new(),
                    ),
                    ("quit".into(), Action::Quit, "ctrl+c".into()),
                ]);
                rows
            }
        };
        let query = self.query.to_lowercase();
        options
            .into_iter()
            .filter(|(label, _, _)| {
                query
                    .split_whitespace()
                    .all(|token| label.to_lowercase().contains(token))
            })
            .collect()
    }
    fn save_input(&mut self, now: f64) {
        if self.modal == "custom_input" {
            if self.query.trim().is_empty() {
                self.notify("Enter some text first", now);
                return;
            }
            self.config.custom = self.query.split_whitespace().collect::<Vec<_>>().join(" ");
            self.config.mode = "custom".into();
        } else {
            let max = if self.modal == "seconds_input" {
                3600
            } else {
                10000
            };
            let Ok(value) = self.query.parse::<usize>() else {
                self.notify("Enter a positive number", now);
                return;
            };
            if value == 0 || value > max {
                self.notify(format!("Enter a number from 1 to {max}"), now);
                return;
            }
            if self.modal == "seconds_input" {
                self.config.seconds = value as u32;
                self.config.mode = "time".into();
            } else {
                self.config.words = value;
                self.config.mode = "words".into();
            }
        }
        self.persist(now);
        self.restart(false);
    }
    pub fn action(&mut self, action: Action, now: f64) {
        self.dirty = true;
        match action {
            Action::Home => self.restart(false),
            Action::Restart(repeat) => self.restart(repeat),
            Action::Page(page) => {
                if self.test.active() {
                    self.restart(false);
                }
                self.page = page;
                self.modal.clear();
                self.scroll = 0;
            }
            Action::Palette(kind) => self.palette(&kind),
            Action::Review => self.review = !self.review,
            Action::Quit => self.running = false,
            Action::Input(kind) => {
                self.modal = format!("{kind}_input");
                self.query = if kind == "custom" {
                    self.config.custom.clone()
                } else {
                    String::new()
                };
            }
            Action::SaveInput => self.save_input(now),
            Action::Toggle(key) => {
                let value = match key {
                    "punctuation" => !self.config.punctuation,
                    "numbers" => !self.config.numbers,
                    "blind" => !self.config.blind,
                    "freedom" => !self.config.freedom,
                    "live_wpm" => !self.config.live_wpm,
                    _ => false,
                };
                self.action(Action::Set(key, value.to_string()), now);
            }
            Action::Set(key, value) => {
                match key {
                    "theme" => self.config.theme = value,
                    "language" => self.config.language = value,
                    "mode" => self.config.mode = value,
                    "seconds" => {
                        self.config.seconds = value.parse().unwrap_or(30);
                        self.config.mode = "time".into();
                    }
                    "words" => {
                        self.config.words = value.parse().unwrap_or(25);
                        self.config.mode = "words".into();
                    }
                    "font_size" => self.config.font_size = value.parse().unwrap_or(32),
                    "caret" => self.config.caret = value,
                    "difficulty" => self.config.difficulty = value,
                    "quote_length" => self.config.quote_length = value,
                    "punctuation" => self.config.punctuation = value == "true",
                    "numbers" => self.config.numbers = value == "true",
                    "live_wpm" => self.config.live_wpm = value == "true",
                    "blind" => self.config.blind = value == "true",
                    "freedom" => self.config.freedom = value == "true",
                    _ => {}
                }
                self.config.validate();
                self.persist(now);
                let page = self.page;
                self.restart(false);
                self.page = page;
            }
        }
    }
    pub fn update(&mut self, now: f64) {
        self.test.tick(now);
        if self.test.finished() && !self.saved {
            self.history.push(self.test.result(now));
            if self.history.len() > 1000 {
                self.history.remove(0);
            }
            if let Err(e) = self.store.save("history.json", &self.history) {
                self.notify(format!("Could not save: {e}"), now);
            }
            self.saved = true;
            self.dirty = true;
        }
        if !self.toast.is_empty() && now >= self.toast_until {
            self.toast.clear();
            self.dirty = true;
        }
    }
    pub fn event(&mut self, event: Event, now: f64, geometry: (u16, u16, usize, usize)) {
        match event {
            Event::Resize(_, _) => self.dirty = true,
            Event::Paste(text) => {
                if self.modal == "custom_input" {
                    self.query.extend(
                        text.chars()
                            .take(10000 - self.query.chars().count().min(10000)),
                    );
                    self.dirty = true;
                } else {
                    self.notify("Pasting is disabled during typing tests", now);
                }
            }
            Event::Mouse(mouse) => {
                let pointer = (
                    (mouse.column as f32 + 0.5) * geometry.2 as f32 / geometry.0 as f32,
                    (mouse.row as f32 + 0.5) * geometry.3 as f32 / geometry.1 as f32,
                );
                let action_at = |p: (f32, f32)| {
                    self.hits
                        .iter()
                        .rev()
                        .find(|(r, _)| r.contains(p.0, p.1))
                        .map(|(_, a)| a.clone())
                };
                let previous = action_at(self.pointer);
                let action = action_at(pointer);
                self.pointer = pointer;
                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        if let Some(action) = action {
                            self.action(action, now);
                        }
                    }
                    MouseEventKind::Moved => {
                        if previous != action {
                            self.dirty = true;
                        }
                    }
                    MouseEventKind::ScrollDown => self.scroll(1),
                    MouseEventKind::ScrollUp => self.scroll(-1),
                    _ => {}
                }
            }
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    match key.code {
                        KeyCode::Char('c' | 'd') => {
                            self.running = false;
                            return;
                        }
                        KeyCode::Char('r') => {
                            self.restart(false);
                            return;
                        }
                        _ => {}
                    }
                }
                self.dirty = true;
                if !self.modal.is_empty() {
                    match key.code {
                        KeyCode::Esc => {
                            if self.modal == "commands" || self.modal.ends_with("_input") {
                                self.modal.clear();
                            } else {
                                self.palette("commands");
                            }
                        }
                        KeyCode::Up | KeyCode::BackTab => {
                            self.selected = self.selected.saturating_sub(1)
                        }
                        KeyCode::Down | KeyCode::Tab => {
                            self.selected =
                                (self.selected + 1).min(self.options().len().saturating_sub(1))
                        }
                        KeyCode::Enter => {
                            if self.modal.ends_with("_input") {
                                self.save_input(now);
                            } else if let Some((_, action, _)) = self.options().get(self.selected) {
                                self.action(action.clone(), now);
                            }
                        }
                        KeyCode::Backspace => {
                            self.query.pop();
                            self.selected = 0;
                        }
                        KeyCode::Char('w' | 'u')
                            if key.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            self.query.clear();
                            self.selected = 0;
                        }
                        KeyCode::Char(ch)
                            if !key
                                .modifiers
                                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                                && self.query.chars().count()
                                    < if self.modal == "custom_input" {
                                        10000
                                    } else {
                                        100
                                    } =>
                        {
                            self.query.push(ch);
                            self.selected = 0;
                        }
                        _ => {}
                    }
                    return;
                }
                if key.code == KeyCode::Esc {
                    self.palette("commands");
                    return;
                }
                if self.page != Page::Test {
                    match key.code {
                        KeyCode::Up => self.scroll(-1),
                        KeyCode::Down => self.scroll(1),
                        KeyCode::Enter | KeyCode::Tab => self.restart(false),
                        _ => {}
                    }
                    return;
                }
                if key.code == KeyCode::Tab {
                    self.restart_armed = true;
                    self.notify("enter to restart", now);
                    return;
                }
                if key.code == KeyCode::Enter {
                    if self.restart_armed || self.test.finished() {
                        self.restart(false);
                    } else if self.config.mode == "zen" {
                        self.test.finish(now);
                    }
                    return;
                }
                self.restart_armed = false;
                match key.code {
                    KeyCode::Backspace => self.test.backspace(
                        key.modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT),
                        now,
                    ),
                    KeyCode::Char('w' | 'u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.test.backspace(true, now)
                    }
                    KeyCode::Char(ch)
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                    {
                        self.test.insert(ch, now)
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    fn scroll(&mut self, direction: i32) {
        if !self.modal.is_empty() {
            self.selected = self
                .selected
                .saturating_add_signed(direction as isize)
                .min(self.options().len().saturating_sub(1));
        } else {
            let max = match self.page {
                Page::History => self.history.len().saturating_sub(1),
                Page::Settings => 7,
                Page::About => 12,
                _ => 0,
            };
            self.scroll = self
                .scroll
                .saturating_add_signed(direction as isize)
                .min(max);
        }
        self.dirty = true;
    }
}
