use crate::{
    app::{Action, App, Page},
    data::{self, Theme},
    gfx::{Canvas, Fonts, Rect},
};

type ConfigButton = (String, Action, bool, Option<&'static str>);

pub struct Renderer {
    pub fonts: Fonts,
    pub canvas: Canvas,
    pub hits: Vec<(Rect, Action)>,
    pub damage: Option<Rect>,
    pub animating: bool,
    scale: f32,
    w: f32,
    h: f32,
    left: f32,
    right: f32,
    pointer: (f32, f32),
    theme: Theme,
    chrome: Option<Canvas>,
    chrome_key: String,
    chrome_hits: Vec<(Rect, Action)>,
    word_layer: Option<(String, Rect, Vec<u8>)>,
    caret: Option<(f32, f32)>,
    caret_start: (f32, f32),
    caret_target: (f32, f32),
    caret_at: f64,
    caret_generation: u64,
}

impl Renderer {
    pub fn new(scale: f32) -> Self {
        Self {
            fonts: Fonts::new(),
            canvas: Canvas::new(1, 1, [0, 0, 0]),
            hits: vec![],
            damage: None,
            animating: false,
            scale,
            w: 0.,
            h: 0.,
            left: 0.,
            right: 0.,
            pointer: (-1., -1.),
            theme: data::themes()["serika_dark"],
            chrome: None,
            chrome_key: String::new(),
            chrome_hits: vec![],
            word_layer: None,
            caret: None,
            caret_start: (0., 0.),
            caret_target: (0., 0.),
            caret_at: 0.,
            caret_generation: u64::MAX,
        }
    }
    fn color(&self, key: &str) -> data::Color {
        match key {
            "bg" => self.theme.bg,
            "main" => self.theme.main,
            "caret" => self.theme.caret,
            "panel" => self.theme.panel,
            "text" => self.theme.text,
            "error" => self.theme.error,
            "extra" => self.theme.extra,
            _ => self.theme.sub,
        }
    }
    fn text(&mut self, x: f32, y: f32, text: &str, size: u16, color: &str) {
        let color = self.color(color);
        self.fonts.text(
            &mut self.canvas,
            x * self.scale,
            y * self.scale,
            text,
            (size as f32 * self.scale).round() as u16,
            color,
            false,
        );
    }
    fn logo_text(&mut self, x: f32, y: f32, text: &str, size: u16, color: &str) {
        let color = self.color(color);
        self.fonts.text(
            &mut self.canvas,
            x * self.scale,
            y * self.scale,
            text,
            (size as f32 * self.scale).round() as u16,
            color,
            true,
        );
    }
    fn centered(&mut self, x: f32, y: f32, text: &str, size: u16, color: &str) {
        let width = self.width(text, size);
        self.text(x - width / 2., y, text, size, color);
    }
    fn width(&self, text: &str, size: u16) -> f32 {
        self.fonts
            .width(text, (size as f32 * self.scale).round() as u16, false)
            / self.scale
    }
    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, color: &str) {
        self.canvas.rect(
            x * self.scale,
            y * self.scale,
            w * self.scale,
            h * self.scale,
            r * self.scale,
            self.color(color),
        );
    }
    fn line(&mut self, x: f32, y: f32, x2: f32, y2: f32, width: f32, color: &str) {
        self.canvas.line(
            x * self.scale,
            y * self.scale,
            x2 * self.scale,
            y2 * self.scale,
            width * self.scale,
            self.color(color),
        );
    }
    fn hit(&mut self, x: f32, y: f32, w: f32, h: f32, action: Action) {
        self.hits.push((
            Rect {
                x: (x * self.scale).max(0.) as usize,
                y: (y * self.scale).max(0.) as usize,
                w: (w * self.scale).ceil() as usize,
                h: (h * self.scale).ceil() as usize,
            },
            action,
        ));
    }
    fn hover(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        self.pointer.0 >= x
            && self.pointer.1 >= y
            && self.pointer.0 < x + w
            && self.pointer.1 < y + h
    }
    fn icon(&mut self, x: f32, y: f32, name: &str, size: u16, color: &str) {
        let ch = match name {
            "keyboard" => '\u{f11c}',
            "crown" => '\u{f521}',
            "info" => '\u{f05a}',
            "settings" => '\u{f013}',
            "user" => '\u{f007}',
            "restart" => '\u{f2f9}',
            "globe" => '\u{f0ac}',
            "clock" | "time" => '\u{f017}',
            "arrow" => '\u{f061}',
            "mountain" | "zen" => '\u{f6fc}',
            "palette" => '\u{f53f}',
            "quote" => '\u{f10d}',
            "numbers" => '\u{f292}',
            "punctuation" => '\u{f1fa}',
            "words" | "custom" => '\u{f031}',
            _ => '\u{f120}',
        };
        let color = self.color(color);
        self.fonts.text_baseline(
            &mut self.canvas,
            x * self.scale,
            (y + size as f32 * 0.85) * self.scale,
            &ch.to_string(),
            (size as f32 * self.scale).round() as u16,
            color,
            false,
        );
    }
    fn icon_button(&mut self, x: f32, y: f32, name: &str, action: Action) {
        self.icon(
            x,
            y,
            name,
            17,
            if self.hover(x - 10., y - 10., 38., 38.) {
                "text"
            } else {
                "sub"
            },
        );
        self.hit(x - 10., y - 10., 38., 38., action);
    }
    #[allow(clippy::too_many_arguments)]
    fn button(
        &mut self,
        x: f32,
        y: f32,
        label: &str,
        action: Action,
        active: bool,
        size: u16,
        padding: f32,
        bg: Option<&str>,
        icon: Option<&str>,
    ) -> f32 {
        let width = self.width(label, size) + padding * 2. + if icon.is_some() { 24. } else { 0. };
        let hover = self.hover(x, y, width, 36.);
        if hover || bg.is_some() {
            self.rect(
                x,
                y,
                width,
                36.,
                8.,
                if hover { "text" } else { bg.unwrap() },
            );
        }
        let color = if hover || bg == Some("main") {
            "bg"
        } else if active {
            "main"
        } else {
            "sub"
        };
        if let Some(name) = icon {
            self.icon(x + padding, y + 10., name, 14, color);
        }
        self.text(
            x + padding + if icon.is_some() { 24. } else { 0. },
            y + (36. - size as f32) / 2. - 2.,
            label,
            size,
            color,
        );
        self.hit(x, y, width, 36., action);
        width
    }
    pub fn render(&mut self, app: &App, width: usize, height: usize, now: f64) {
        self.w = width as f32 / self.scale;
        self.h = height as f32 / self.scale;
        self.left = ((self.w - 1280.) / 2.).max(24.);
        self.right = self.w - self.left;
        self.pointer = (app.pointer.0 / self.scale, app.pointer.1 / self.scale);
        self.theme = data::themes()[&app.config.theme];
        self.hits.clear();
        self.damage = None;
        self.animating = false;
        let typing = app.page == Page::Test
            && !app.test.finished()
            && app.modal.is_empty()
            && self.w >= 560.
            && self.h >= 340.;
        let key = format!(
            "{width}:{height}:{}:{}:{:?}:{}:{}",
            app.generation,
            app.test.active(),
            app.pointer,
            app.toast,
            app.config.theme
        );
        let reused = typing && self.chrome_key == key && self.chrome.is_some();
        self.canvas = if reused {
            self.chrome.as_ref().unwrap().clone()
        } else {
            Canvas::new(width, height, self.theme.bg)
        };
        if self.w < 560. || self.h < 340. {
            self.logo_text(
                (self.w - 220.) / 2.,
                self.h / 2. - 32.,
                "monkeytype",
                30,
                "main",
            );
            self.centered(
                self.w / 2.,
                self.h / 2. + 20.,
                "enlarge the terminal to begin",
                14,
                "sub",
            );
            return;
        }
        if typing {
            if reused {
                self.hits = self.chrome_hits.clone();
            } else {
                self.header(app);
                self.footer(app);
                if !app.test.active() {
                    self.config_bar(app);
                }
                self.chrome = Some(self.canvas.clone());
                self.chrome_key = key;
                self.chrome_hits = self.hits.clone();
            }
            self.typing(app, now);
            if reused {
                let top = self.typing_top();
                let bottom =
                    (top + (app.config.font_size as f32 * 1.5 + 2.) * 3. + 72.).min(self.h);
                self.damage = Some(Rect {
                    x: (self.left * self.scale).floor() as usize,
                    y: ((top - 55.) * self.scale).max(0.).floor() as usize,
                    w: ((self.right - self.left) * self.scale).ceil() as usize,
                    h: ((bottom - top + 55.) * self.scale).ceil() as usize,
                });
                if let Some(r) = self.damage.as_mut() {
                    r.w = r.w.min(width - r.x);
                    r.h = r.h.min(height - r.y);
                }
            }
        } else {
            self.chrome_key.clear();
            self.header(app);
            match app.page {
                Page::Settings => self.settings(app),
                Page::History => self.history(app),
                Page::About => self.about(app),
                Page::Test => {
                    if app.test.finished() {
                        self.results(app, now);
                    } else {
                        if !app.test.active() {
                            self.config_bar(app);
                        }
                        self.typing(app, now);
                    }
                }
            }
            self.footer(app);
        }
        if !app.modal.is_empty() {
            self.modal(app);
        }
        if !app.toast.is_empty() && now < app.toast_until {
            let width = (self.width(&app.toast, 13) + 40.).min(self.w - 40.);
            self.rect(self.w - width - 24., 90., width, 50., 8., "panel");
            self.text(self.w - width - 4., 105., &app.toast, 13, "text");
        }
    }
    fn header(&mut self, app: &App) {
        let (x, y) = (self.left, 46.);
        let focused = app.test.active() && app.page == Page::Test && app.modal.is_empty();
        self.fonts.logo(
            &mut self.canvas,
            x * self.scale,
            (y + 5.) * self.scale,
            (50. * self.scale) as usize,
            (30. * self.scale) as usize,
            if focused {
                self.theme.sub
            } else {
                self.theme.main
            },
        );
        if !focused {
            self.logo_text(x + 61., y - 8., "monkey see", 10, "sub");
        }
        self.logo_text(
            x + 60.,
            y + 3.,
            "monkeytype",
            32,
            if focused { "sub" } else { "text" },
        );
        self.hit(x, y - 5., 260., 50., Action::Home);
        if focused {
            return;
        }
        let mut nx = x + 280.;
        for (name, action) in [
            ("keyboard", Action::Home),
            ("crown", Action::Page(Page::History)),
            ("info", Action::Page(Page::About)),
            ("settings", Action::Page(Page::Settings)),
        ] {
            self.icon_button(nx, y + 13., name, action);
            nx += 46.;
        }
        self.icon_button(
            self.right - 20.,
            y + 13.,
            "user",
            Action::Page(Page::History),
        );
    }
    fn config_bar(&mut self, app: &App) {
        let c = &app.config;
        let size = if self.right - self.left > 1000. {
            12
        } else {
            11
        };
        let padding = if size == 12 { 11. } else { 8. };
        let mut groups: Vec<Vec<ConfigButton>> = vec![
            vec![
                (
                    "punctuation".into(),
                    Action::Toggle("punctuation"),
                    c.punctuation,
                    Some("punctuation"),
                ),
                (
                    "numbers".into(),
                    Action::Toggle("numbers"),
                    c.numbers,
                    Some("numbers"),
                ),
            ],
            ["time", "words", "quote", "zen", "custom"]
                .into_iter()
                .map(|m| {
                    (
                        m.into(),
                        Action::Set("mode", m.into()),
                        c.mode == m,
                        Some(m),
                    )
                })
                .collect(),
        ];
        match c.mode.as_str() {
            "time" | "words" => {
                let (key, values, current) = if c.mode == "time" {
                    ("seconds", [15, 30, 60, 120], c.seconds as usize)
                } else {
                    ("words", [10, 25, 50, 100], c.words)
                };
                let mut group: Vec<_> = values
                    .into_iter()
                    .map(|v| {
                        (
                            v.to_string(),
                            Action::Set(key, v.to_string()),
                            current == v,
                            None,
                        )
                    })
                    .collect();
                group.push((
                    String::new(),
                    Action::Palette(key.into()),
                    false,
                    Some("settings"),
                ));
                groups.push(group);
            }
            "quote" => groups.push(
                ["short", "medium", "long", "all"]
                    .into_iter()
                    .map(|v| {
                        (
                            v.into(),
                            Action::Set("quote_length", v.into()),
                            c.quote_length == v,
                            None,
                        )
                    })
                    .collect(),
            ),
            "custom" => groups.push(vec![(
                "change".into(),
                Action::Input("custom".into()),
                false,
                Some("settings"),
            )]),
            _ => {}
        }
        let widths: Vec<f32> = groups
            .iter()
            .map(|group| {
                group
                    .iter()
                    .map(|(label, _, _, icon)| {
                        self.width(label, size)
                            + padding * 2.
                            + if icon.is_some() { 24. } else { 0. }
                    })
                    .sum()
            })
            .collect();
        let total = widths.iter().sum::<f32>() + 16. * (groups.len() - 1) as f32;
        if total > self.right - self.left {
            self.button(
                self.w / 2. - 80.,
                142.,
                "test settings",
                Action::Palette("commands".into()),
                false,
                12,
                12.,
                Some("panel"),
                Some("settings"),
            );
            return;
        }
        let mut x = (self.w - total) / 2.;
        for (group, width) in groups.into_iter().zip(widths) {
            self.rect(x, 142., width, 36., 8., "panel");
            for (label, action, active, icon) in group {
                x += self.button(x, 142., &label, action, active, size, padding, None, icon);
            }
            x += 16.;
        }
    }
    fn typing_top(&self) -> f32 {
        if self.h < 620. {
            (self.h * 0.39).round().max(210.)
        } else {
            (self.h * 0.425).round().max(226.)
        }
    }
    fn typing(&mut self, app: &App, now: f64) {
        let t = &app.test;
        let c = &app.config;
        let size = c.font_size;
        let fs = size as f32;
        let cw = self
            .fonts
            .advance('m', (fs * self.scale).round() as u16, false)
            / self.scale;
        let line_height = fs * 1.5 + 2.;
        let margin = fs * 0.3;
        let top = self.typing_top();
        let mut positions = Vec::with_capacity(100);
        let mut row = 0usize;
        let mut x = self.left + margin;
        let mut cursor = (x, 0usize);
        let count = if c.mode == "zen" {
            t.inputs.len()
        } else {
            t.targets.len()
        };
        for i in 0..count {
            let typed = t.inputs.get(i).map(Vec::as_slice).unwrap_or(&[]);
            let target = &t.targets[i];
            let width = target.len().max(typed.len()) as f32 * cw;
            if x > self.left + margin && x + width + margin > self.right {
                row += 1;
                x = self.left + margin;
            }
            if i == t.index {
                cursor = (x + typed.len() as f32 * cw, row);
            }
            positions.push((i, x, row));
            x += width + margin * 2.;
            if i > t.index && row > cursor.1 + 2 {
                break;
            }
        }
        let scroll = cursor.1.saturating_sub(1);
        if t.active() {
            self.text(self.left, top - 50., &t.remaining(now), 24, "main");
            if c.live_wpm {
                self.text(
                    self.left + 150.,
                    top - 50.,
                    &format!("{:0.0} wpm", t.stats(now).wpm),
                    24,
                    "main",
                );
            }
        } else {
            let label = if c.mode == "zen" {
                "zen".into()
            } else {
                c.language.replace('_', " ")
            };
            let width = self.width(&label, 16);
            let left = self.w / 2. - width / 2. - 30.;
            let hover = self.hover(left, top - 50., width + 45., 34.);
            self.icon(
                left + 4.,
                top - 40.,
                "globe",
                16,
                if hover { "text" } else { "sub" },
            );
            self.text(
                left + 30.,
                top - 46.,
                &label,
                16,
                if hover { "text" } else { "sub" },
            );
            self.hit(
                left,
                top - 50.,
                width + 45.,
                34.,
                Action::Palette("language".into()),
            );
        }
        let actual_size = (fs * self.scale).round() as u16;
        let baseline = self.fonts.baseline(actual_size) / self.scale + fs * 0.25;
        let word_key = format!(
            "{}:{}:{}:{}:{}:{}:{:?}:{}:{}",
            app.generation,
            self.w,
            self.h,
            size,
            c.theme,
            t.index,
            t.current(),
            c.blind,
            scroll
        );
        if let Some((key, rect, pixels)) = self
            .word_layer
            .as_ref()
            .filter(|(key, _, _)| *key == word_key)
        {
            let _ = key;
            for row in 0..rect.h {
                let start = ((rect.y + row) * self.canvas.width + rect.x) * 3;
                self.canvas.pixels[start..start + rect.w * 3]
                    .copy_from_slice(&pixels[row * rect.w * 3..(row + 1) * rect.w * 3]);
            }
        } else {
            for (i, px, row) in positions {
                if row < scroll || row >= scroll + 3 {
                    continue;
                }
                let y = top + (row - scroll) as f32 * line_height;
                let target = &t.targets[i];
                let typed = t.inputs.get(i).map(Vec::as_slice).unwrap_or(&[]);
                for j in 0..target.len().max(typed.len()) {
                    let expected = target.get(j);
                    let actual = typed.get(j);
                    let ch = expected.or(actual).unwrap();
                    let role = if actual.is_some() {
                        if c.mode == "zen" || expected == actual || c.blind {
                            "text"
                        } else if expected.is_some() {
                            "error"
                        } else {
                            "extra"
                        }
                    } else {
                        "sub"
                    };
                    let gx = px + j as f32 * cw;
                    if gx + cw > self.right {
                        break;
                    }
                    let color = self.color(role);
                    self.fonts.text_baseline(
                        &mut self.canvas,
                        gx * self.scale,
                        (y + baseline) * self.scale,
                        &ch.to_string(),
                        actual_size,
                        color,
                        false,
                    );
                }
                if i < t.index && typed != target && c.mode != "zen" && !c.blind {
                    self.line(
                        px,
                        y + fs + 5.,
                        (px + target.len().max(typed.len()) as f32 * cw).min(self.right),
                        y + fs + 5.,
                        1.,
                        "error",
                    );
                }
            }
            let x = (self.left * self.scale).floor() as usize;
            let y = (top * self.scale).floor() as usize;
            let rect = Rect {
                x,
                y,
                w: ((self.right * self.scale).ceil() as usize - x).min(self.canvas.width - x),
                h: ((line_height * 3. * self.scale).ceil() as usize)
                    .min(self.canvas.height.saturating_sub(y)),
            };
            if y < self.canvas.height {
                self.word_layer = Some((word_key, rect, self.canvas.crop(rect)));
            }
        }
        let target = (cursor.0, top + (cursor.1 - scroll) as f32 * line_height);
        if self.caret_generation != app.generation
            || self.caret.is_none()
            || !t.active()
            || self
                .caret
                .is_some_and(|(_, y)| (y - target.1).abs() > line_height / 2.)
        {
            self.caret = Some(target);
            self.caret_target = target;
            self.caret_start = target;
            self.caret_at = now;
            self.caret_generation = app.generation;
        } else if self.caret_target != target {
            self.caret_start = self.caret.unwrap_or(target);
            self.caret_target = target;
            self.caret_at = now;
        }
        // Keep a short visual glide without trailing several keys behind a
        // fast typist. Letter colors are painted immediately on key events.
        let progress = ((now - self.caret_at) / 0.025).clamp(0., 1.) as f32;
        let eased = 1. - (1. - progress).powi(3);
        let cx = self.caret_start.0 + (self.caret_target.0 - self.caret_start.0) * eased;
        let cy = target.1;
        self.caret = Some((cx, cy));
        self.animating = t.active() && progress < 1.;
        if t.active() || ((now * 2.) as u64).is_multiple_of(2) {
            match c.caret.as_str() {
                "underline" => self.line(cx, cy + fs + 3., cx + cw, cy + fs + 3., 2., "caret"),
                "block" => {
                    self.line(cx, cy, cx + cw, cy, 2., "caret");
                    self.line(cx, cy + fs + 4., cx + cw, cy + fs + 4., 2., "caret");
                    self.line(cx, cy, cx, cy + fs + 4., 2., "caret");
                    self.line(cx + cw, cy, cx + cw, cy + fs + 4., 2., "caret");
                }
                _ => self.rect(cx - 2., cy, 2., fs + 4., 1., "caret"),
            }
        }
        if !t.active() {
            self.icon_button(
                self.w / 2. - 9.,
                top + line_height * 3. + 34.,
                "restart",
                Action::Restart(false),
            );
        }
        if c.mode == "zen" && !t.active() {
            self.text(
                self.left + margin,
                top + line_height,
                "type anything. find your flow.",
                size,
                "sub",
            );
        }
        if c.mode == "quote" && !t.source.is_empty() {
            let source: String = t.source.chars().take(70).collect();
            let width = self.width(&source, 12);
            self.text(
                self.right - width,
                top + line_height * 3. + 35.,
                &source,
                12,
                "sub",
            );
        }
    }
    fn footer(&mut self, app: &App) {
        if app.test.active() && app.page == Page::Test && app.modal.is_empty() {
            return;
        }
        if app.page == Page::Test {
            self.centered(
                self.w / 2.,
                self.h - 118.,
                if app.config.mode == "zen" {
                    "enter  finish zen     esc  command line"
                } else {
                    "tab + enter  restart test     esc  command line"
                },
                12,
                "sub",
            );
        }
        let y = self.h - 53.;
        let mut x = self.left - 10.;
        for (label, page) in [
            ("about", Page::About),
            ("history", Page::History),
            ("settings", Page::Settings),
        ] {
            x += self.button(x, y, label, Action::Page(page), false, 12, 10., None, None) + 16.;
        }
        let label = app.config.theme.replace('_', " ");
        let width = self.width(&label, 12) + 36.;
        self.icon(self.right - width - 125., y + 10., "palette", 14, "sub");
        self.button(
            self.right - width - 105.,
            y,
            &label,
            Action::Palette("theme".into()),
            false,
            12,
            12.,
            None,
            None,
        );
        self.text(self.right - 100., y + 10., "rust / v0.1.0", 11, "sub");
    }
    fn results(&mut self, app: &App, now: f64) {
        let t = &app.test;
        let s = t.stats(now);
        let top = (self.h * 0.28).max(135.);
        let compact = if self.h < 720. { 0.72 } else { 1. };
        self.text(self.left, top, "wpm", 30, "sub");
        self.text(
            self.left,
            top + 34. * compact,
            &format!("{:0.0}", s.wpm),
            (64. * compact.max(0.85)) as u16,
            "main",
        );
        self.text(self.left, top + 112. * compact, "acc", 30, "sub");
        self.text(
            self.left,
            top + 150. * compact,
            &format!("{:0.0}%", s.accuracy),
            (58. * compact.max(0.85)) as u16,
            "main",
        );
        let (x, y, right, bottom) = (
            self.left + 230.,
            top + 10.,
            self.right - 15.,
            top + 210. * compact,
        );
        let maximum = (t.samples.iter().map(|s| s.raw).fold(50., f64::max) / 25.).ceil() * 25.;
        for i in 0..5 {
            let py = bottom - (bottom - y) * i as f32 / 4.;
            self.line(x, py, right, py, 1., "panel");
            let text = format!("{:0.0}", maximum * i as f64 / 4.);
            let tw = self.width(&text, 10);
            self.text(x - 12. - tw, py - 7., &text, 10, "sub");
        }
        self.text(x, y - 28., "wpm", 11, "main");
        self.text(x + 55., y - 28., "raw", 11, "sub");
        self.text(right - 65., y - 28., "× errors", 11, "error");
        let duration = t.elapsed(now).max(1.);
        for (raw, color) in [(true, "sub"), (false, "main")] {
            let mut previous = None;
            for sample in &t.samples {
                let px = x + (right - x) * (sample.second / duration) as f32;
                let py = bottom
                    - (bottom - y) * (if raw { sample.raw } else { sample.wpm } / maximum) as f32;
                if let Some((lx, ly)) = previous {
                    self.line(lx, ly, px, py, 2., color);
                }
                self.canvas.circle(
                    px * self.scale,
                    py * self.scale,
                    2. * self.scale,
                    self.color(color),
                );
                previous = Some((px, py));
            }
        }
        let mut previous = 0;
        for sample in &t.samples {
            if sample.errors > previous {
                let px = x + (right - x) * (sample.second / duration) as f32;
                self.line(px - 3., bottom - 10., px + 3., bottom - 4., 2., "error");
                self.line(px - 3., bottom - 4., px + 3., bottom - 10., 2., "error");
            }
            previous = sample.errors;
        }
        for i in 1..=5 {
            self.centered(
                x + (right - x) * i as f32 / 5.,
                bottom + 12.,
                &format!("{:0.0}", duration * i as f64 / 5.),
                10,
                "sub",
            );
        }
        let y = top + 260. * compact;
        let columns = [
            (
                "test type",
                format!(
                    "{} {}",
                    c_mode(&t.config.mode),
                    if t.config.mode == "time" {
                        t.config.seconds.to_string()
                    } else if t.config.mode == "zen" {
                        String::new()
                    } else {
                        t.targets.len().to_string()
                    }
                ),
                t.config.language.replace('_', " "),
                0.2,
            ),
            ("raw", format!("{:0.0}", s.raw), String::new(), 0.12),
            (
                "characters",
                format!("{}/{}/{}/{}", s.correct, s.incorrect, s.extra, s.missed),
                "correct / incorrect / extra / missed".into(),
                0.32,
            ),
            (
                "consistency",
                format!("{:0.0}%", s.consistency),
                String::new(),
                0.22,
            ),
            ("time", format!("{:0.1}s", s.duration), String::new(), 0.14),
        ];
        let mut x = self.left;
        for (label, value, detail, ratio) in columns {
            self.text(x, y, label, 17, "sub");
            self.text(
                x,
                y + 29.,
                &value,
                if label == "characters" { 22 } else { 24 },
                "main",
            );
            if !detail.is_empty() {
                self.text(
                    x,
                    y + 65.,
                    &detail,
                    if label == "characters" { 10 } else { 13 },
                    "sub",
                );
            }
            x += (self.right - self.left) * ratio;
        }
        if t.failed {
            self.text(
                self.left,
                y + 98.,
                &format!("test failed · {} difficulty", t.config.difficulty),
                14,
                "error",
            );
        }
        let row = (y + 128. * compact).min(self.h - 145.);
        for (offset, name, action) in [
            (-110., "arrow", Action::Restart(false)),
            (-40., "restart", Action::Restart(true)),
            (30., "mountain", Action::Page(Page::History)),
            (100., "keyboard", Action::Review),
        ] {
            self.icon_button(self.w / 2. + offset, row, name, action);
        }
        if app.review {
            self.rect(
                self.left,
                row - 12.,
                self.right - self.left,
                (self.h - 140. - row + 12.).max(40.),
                8.,
                "panel",
            );
            self.text(
                self.left + 16.,
                row,
                "word history · red words contain errors",
                12,
                "sub",
            );
            let mut x = self.left + 16.;
            let mut y = row + 28.;
            for (i, typed) in t.inputs.iter().enumerate() {
                let text: String = typed.iter().collect();
                let width = self.width(&text, 16) + 12.;
                if x + width > self.right - 16. {
                    x = self.left + 16.;
                    y += 27.;
                }
                if y > self.h - 165. {
                    break;
                }
                self.text(
                    x,
                    y,
                    &text,
                    16,
                    if t.targets.get(i) == Some(typed) || t.config.mode == "zen" {
                        "text"
                    } else {
                        "error"
                    },
                );
                x += width;
            }
        }
    }
    fn title(&mut self, title: &str, subtitle: &str) {
        self.text(self.left, 140., title, 28, "text");
        self.text(self.left, 188., subtitle, 13, "sub");
    }
    fn settings(&mut self, app: &App) {
        self.title("settings", "make yourself at home");
        let c = &app.config;
        let bools = |key: &'static str, current: bool| {
            vec![
                (
                    "off".to_string(),
                    Action::Set(key, "false".into()),
                    !current,
                ),
                ("on".to_string(), Action::Set(key, "true".into()), current),
            ]
        };
        let rows = vec![
            (
                "theme",
                "colors for every mood",
                vec![(
                    c.theme.replace('_', " "),
                    Action::Palette("theme".into()),
                    true,
                )],
            ),
            (
                "language",
                "original Monkeytype dictionaries",
                vec![(
                    c.language.replace('_', " "),
                    Action::Palette("language".into()),
                    true,
                )],
            ),
            (
                "font size",
                "the size of the typing test",
                [24, 32, 40, 48]
                    .into_iter()
                    .map(|v| {
                        (
                            v.to_string(),
                            Action::Set("font_size", v.to_string()),
                            c.font_size == v,
                        )
                    })
                    .collect(),
            ),
            (
                "caret style",
                "your place in the words",
                ["line", "block", "underline"]
                    .into_iter()
                    .map(|v| (v.into(), Action::Set("caret", v.into()), c.caret == v))
                    .collect(),
            ),
            (
                "live wpm",
                "show your speed while typing",
                bools("live_wpm", c.live_wpm),
            ),
            (
                "blind mode",
                "hide incorrect letter colors during the test",
                bools("blind", c.blind),
            ),
            (
                "freedom mode",
                "backspace into any previous word",
                bools("freedom", c.freedom),
            ),
            (
                "difficulty",
                "expert: no wrong words · master: no wrong letters",
                ["normal", "expert", "master"]
                    .into_iter()
                    .map(|v| {
                        (
                            v.into(),
                            Action::Set("difficulty", v.into()),
                            c.difficulty == v,
                        )
                    })
                    .collect(),
            ),
        ];
        let visible = ((self.h - 320.) / 78.).floor().max(1.) as usize;
        let start = app.scroll.min(rows.len().saturating_sub(visible));
        for (i, (label, detail, options)) in rows.into_iter().skip(start).take(visible).enumerate()
        {
            let y = 225. + i as f32 * 78.;
            self.text(self.left, y, label, 17, "text");
            self.text(self.left, y + 28., detail, 11, "sub");
            let total = options
                .iter()
                .map(|(s, _, _)| self.width(s, 12) + 32.)
                .sum::<f32>()
                + 8. * options.len().saturating_sub(1) as f32;
            let mut x = self.right - total;
            for (label, action, active) in options {
                x += self.button(
                    x,
                    y,
                    &label,
                    action,
                    active,
                    12,
                    16.,
                    Some(if active { "main" } else { "panel" }),
                    None,
                ) + 8.;
            }
        }
        if visible < 8 {
            self.text(
                self.right - 165.,
                self.h - 145.,
                "scroll / ↑ ↓ for more",
                12,
                "sub",
            );
        }
    }
    fn history(&mut self, app: &App) {
        self.title(
            "local history",
            &format!(
                "{} tests completed · saved on this device",
                app.history.len()
            ),
        );
        if app.history.is_empty() {
            self.centered(
                self.w / 2.,
                self.h / 2.,
                "your first result belongs here",
                22,
                "sub",
            );
            self.button(
                self.w / 2. - 70.,
                self.h / 2. + 45.,
                "start a test",
                Action::Home,
                false,
                12,
                12.,
                Some("panel"),
                None,
            );
            return;
        }
        let valid: Vec<_> = app.history.iter().filter(|r| !r.failed).collect();
        let highest = valid.iter().map(|r| r.stats.wpm).fold(0., f64::max);
        let average = valid.iter().map(|r| r.stats.wpm).sum::<f64>() / valid.len().max(1) as f64;
        for (i, (label, value)) in [
            ("highest wpm", format!("{highest:0.0}")),
            ("average wpm", format!("{average:0.0}")),
            ("tests completed", app.history.len().to_string()),
        ]
        .into_iter()
        .enumerate()
        {
            let x = self.left + i as f32 * (self.right - self.left) / 3.;
            self.text(x, 220., label, 16, "sub");
            self.text(x, 252., &value, 42, "main");
        }
        let cols = [0., 0.19, 0.34, 0.48, 0.67, 0.84];
        for (ratio, label) in
            cols.into_iter()
                .zip(["date", "wpm", "accuracy", "test type", "language", "time"])
        {
            self.text(
                self.left + (self.right - self.left) * ratio,
                330.,
                label,
                12,
                "sub",
            );
        }
        let limit = ((self.h - 480.) / 42.).floor().max(1.) as usize;
        let start = app.scroll.min(app.history.len().saturating_sub(limit));
        for (i, r) in app.history.iter().rev().skip(start).take(limit).enumerate() {
            let y = 365. + i as f32 * 42.;
            if i % 2 == 0 {
                self.rect(
                    self.left - 12.,
                    y - 8.,
                    self.right - self.left + 24.,
                    36.,
                    4.,
                    "panel",
                );
            }
            let days = (now_epoch() - r.date).max(0.) / 86400.;
            let date = if days < 1. {
                "today".into()
            } else {
                format!("{}d ago", days.floor())
            };
            let values = [
                date,
                format!("{:0.0}", r.stats.wpm),
                format!("{:0.0}%", r.stats.accuracy),
                format!("{} {}", r.mode, r.limit),
                r.language.replace('_', " "),
                format!("{:0.1}s", r.stats.duration),
            ];
            for (j, (ratio, value)) in cols.into_iter().zip(values).enumerate() {
                self.text(
                    self.left + (self.right - self.left) * ratio,
                    y,
                    &value,
                    12,
                    if j == 1 { "main" } else { "text" },
                );
            }
        }
    }
    fn about(&mut self, app: &App) {
        self.title("about", "monkey see. monkey type. right in your terminal.");
        let lines = [
            ("a familiar place to find your flow", 23, "text"),
            (
                "A native Rust rewrite, rendered using Kitty graphics.",
                14,
                "sub",
            ),
            (
                "Original fonts, dictionaries, quotes, logo, and theme colors from Monkeytype.",
                14,
                "sub",
            ),
            (
                "Time, words, quote, zen, and custom tests. Settings and results stay local.",
                14,
                "sub",
            ),
            ("keyboard shortcuts", 20, "text"),
            (
                "esc                    open / close command line",
                14,
                "sub",
            ),
            ("tab then enter         restart test", 14, "sub"),
            ("ctrl + r               restart immediately", 14, "sub"),
            ("ctrl + w               delete current word", 14, "sub"),
            ("enter in zen           finish test", 14, "sub"),
            ("ctrl + c / ctrl + d     quit", 14, "sub"),
            (
                "Free software · GPL-3.0-or-later · no warranty. See LICENSE and NOTICE.",
                12,
                "sub",
            ),
            (
                "Monkeytype © Miodec and contributors · monkeytype.com",
                12,
                "sub",
            ),
        ];
        let visible = ((self.h - 350.) / 36.).floor().max(1.) as usize;
        let start = app.scroll.min(lines.len().saturating_sub(visible));
        for (i, (text, size, color)) in lines.into_iter().skip(start).take(visible).enumerate() {
            self.text(self.left, 235. + i as f32 * 36., text, size, color);
        }
    }
    fn modal(&mut self, app: &App) {
        self.canvas.dim();
        self.hits.clear();
        let width = (self.w - 64.).min(680.);
        let left = (self.w - width) / 2.;
        let top = (self.h * 0.15).clamp(60., 150.);
        if app.modal.ends_with("_input") {
            let height = if app.modal == "custom_input" {
                280.
            } else {
                215.
            };
            self.rect(left, top, width, height, 12., "bg");
            self.text(
                left + 26.,
                top + 25.,
                if app.modal == "custom_input" {
                    "custom text"
                } else if app.modal == "seconds_input" {
                    "test duration (seconds)"
                } else {
                    "word count"
                },
                22,
                "text",
            );
            self.rect(
                left + 24.,
                top + 78.,
                width - 48.,
                height - 153.,
                8.,
                "panel",
            );
            let count = ((width - 80.) / self.width("m", 15)).floor().max(1.) as usize;
            let chars: Vec<_> = app.query.chars().collect();
            let lines: Vec<String> = chars.chunks(count).map(|c| c.iter().collect()).collect();
            let start = lines.len().saturating_sub(3);
            for (i, line) in lines.iter().skip(start).enumerate() {
                self.text(left + 38., top + 88. + i as f32 * 25., line, 15, "text");
            }
            self.text(
                left + 26.,
                top + height - 45.,
                "enter  save     esc  cancel",
                12,
                "sub",
            );
            self.button(
                left + width - 105.,
                top + height - 54.,
                "save",
                Action::SaveInput,
                true,
                12,
                12.,
                Some("panel"),
                None,
            );
            return;
        }
        let options = app.options();
        let count = (((self.h - top - 190.) / 42.).floor().max(1.) as usize).min(9);
        let height = 110. + count.min(options.len().max(1)) as f32 * 42.;
        self.rect(left, top, width, height, 12., "bg");
        self.text(left + 24., top + 16., ">", 24, "main");
        let placeholder = format!(
            "{}…",
            if app.modal == "commands" {
                "command line"
            } else {
                &app.modal
            }
        );
        let query = if app.query.is_empty() {
            placeholder
        } else {
            format!("{}│", app.query)
        };
        self.text(
            left + 60.,
            top + 19.,
            &query,
            20,
            if app.query.is_empty() { "sub" } else { "text" },
        );
        self.line(
            left + 24.,
            top + 66.,
            left + width - 24.,
            top + 66.,
            1.,
            "panel",
        );
        let start = app.selected.saturating_sub(count - 1);
        for (i, (label, action, detail)) in options.iter().enumerate().skip(start).take(count) {
            let y = top + 78. + (i - start) as f32 * 42.;
            let hover = self.hover(left + 12., y, width - 24., 40.);
            if i == app.selected || hover {
                self.rect(left + 12., y, width - 24., 40., 4., "panel");
            }
            self.text(
                left + 28.,
                y + 8.,
                label,
                14,
                if i == app.selected { "main" } else { "text" },
            );
            if app.modal == "theme" {
                if let Action::Set(_, name) = action {
                    let t = data::themes()[name];
                    for (j, c) in [t.bg, t.sub, t.text, t.main].into_iter().enumerate() {
                        self.canvas.circle(
                            (left + width - 110. + j as f32 * 20.) * self.scale,
                            (y + 18.) * self.scale,
                            6. * self.scale,
                            c,
                        );
                    }
                }
            } else if !detail.is_empty() {
                let tw = self.width(detail, 11);
                self.text(left + width - 30. - tw, y + 9., detail, 11, "sub");
            }
            self.hit(left + 12., y, width - 24., 40., action.clone());
        }
        if options.is_empty() {
            self.text(left + 28., top + 95., "no matching commands", 14, "sub");
        }
        self.text(
            left + 25.,
            top + height - 28.,
            "↑ ↓ navigate     enter select     esc back",
            10,
            "sub",
        );
    }
}
fn c_mode(mode: &str) -> &str {
    mode
}
fn now_epoch() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Config, Store};
    #[test]
    fn all_screens_render_at_common_terminal_sizes() {
        let store = Store::new(Some(std::env::temp_dir().join("monkeytype-render-fixture")));
        let mut app = App::new(store, Config::default(), 42);
        let mut renderer = Renderer::new(1.);
        for (w, h) in [(1440, 900), (860, 600), (640, 480)] {
            for page in [Page::Test, Page::Settings, Page::History, Page::About] {
                app.page = page;
                renderer.render(&app, w, h, 0.);
                assert_eq!(renderer.canvas.pixels.len(), w * h * 3);
                assert!(!renderer.hits.is_empty());
            }
            app.page = Page::Test;
            app.palette("commands");
            renderer.render(&app, w, h, 0.);
            app.modal.clear();
        }
    }

    #[test]
    fn cached_chrome_matches_a_fresh_render() {
        let store = Store::new(Some(std::env::temp_dir().join("monkeytype-render-fixture")));
        let mut app = App::new(store, Config::default(), 42);
        let mut renderer = Renderer::new(1.);
        app.test.insert(app.test.target()[0], 0.);
        renderer.render(&app, 1440, 900, 1.);
        renderer.render(&app, 1440, 900, 1.);
        let pixels = renderer.canvas.pixels.clone();
        assert!(renderer.damage.is_some());
        renderer.chrome_key.clear();
        renderer.render(&app, 1440, 900, 1.);
        assert_eq!(pixels, renderer.canvas.pixels);
    }

    #[test]
    fn command_palette_preserves_the_unfinished_typing_screen() {
        let store = Store::new(Some(std::env::temp_dir().join("monkeytype-render-fixture")));
        let mut app = App::new(store, Config::default(), 42);
        let mut renderer = Renderer::new(1.);
        app.palette("commands");
        renderer.render(&app, 1440, 900, 0.);
        assert_eq!(renderer.caret_generation, app.generation);
        assert!(!app.test.finished());
    }
}
