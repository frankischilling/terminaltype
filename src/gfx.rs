use crate::data::{self, Color};
use fontdue::{Font, FontSettings, Metrics};
use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufWriter, Cursor},
    path::Path,
};

#[derive(Clone)]
pub struct Canvas {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}
impl Canvas {
    pub fn new(width: usize, height: usize, color: Color) -> Self {
        let mut pixels = vec![0; width * height * 3];
        for p in pixels.as_chunks_mut::<3>().0 {
            p.copy_from_slice(&color);
        }
        Self {
            width,
            height,
            pixels,
        }
    }
    pub fn blend(&mut self, x: i32, y: i32, color: Color, alpha: u8) {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height || alpha == 0 {
            return;
        }
        let i = (y as usize * self.width + x as usize) * 3;
        if alpha == 255 {
            self.pixels[i..i + 3].copy_from_slice(&color);
            return;
        }
        let a = alpha as u32;
        for (j, c) in color.iter().enumerate() {
            self.pixels[i + j] =
                (((*c as u32) * a + self.pixels[i + j] as u32 * (255 - a) + 127) / 255) as u8;
        }
    }
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
        let (x0, y0, x1, y1) = (
            x.floor() as i32,
            y.floor() as i32,
            (x + w).ceil() as i32,
            (y + h).ceil() as i32,
        );
        let r = r.min(w / 2.).min(h / 2.);
        for py in y0.max(0)..y1.min(self.height as i32) {
            for px in x0.max(0)..x1.min(self.width as i32) {
                let cx = (px as f32 + 0.5).clamp(x + r, (x + w - r).max(x + r));
                let cy = (py as f32 + 0.5).clamp(y + r, (y + h - r).max(y + r));
                let distance =
                    ((px as f32 + 0.5 - cx).powi(2) + (py as f32 + 0.5 - cy).powi(2)).sqrt();
                let alpha = if r == 0. {
                    255
                } else {
                    ((r + 0.5 - distance).clamp(0., 1.) * 255.) as u8
                };
                self.blend(px, py, color, alpha);
            }
        }
    }
    pub fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32, color: Color) {
        let dx = x2 - x1;
        let dy = y2 - y1;
        let steps = dx.abs().max(dy.abs()).ceil().max(1.) as usize;
        for i in 0..=steps {
            let p = i as f32 / steps as f32;
            self.rect(
                x1 + dx * p - width / 2.,
                y1 + dy * p - width / 2.,
                width,
                width,
                width / 2.,
                color,
            );
        }
    }
    pub fn circle(&mut self, x: f32, y: f32, r: f32, color: Color) {
        self.rect(x - r, y - r, r * 2., r * 2., r, color);
    }
    pub fn dim(&mut self) {
        for channel in &mut self.pixels {
            *channel = (*channel as u16 * 43 / 100) as u8;
        }
    }
    pub fn crop(&self, box_: Rect) -> Vec<u8> {
        let mut data = Vec::with_capacity(box_.w * box_.h * 3);
        for y in box_.y..box_.y + box_.h {
            let start = (y * self.width + box_.x) * 3;
            data.extend_from_slice(&self.pixels[start..start + box_.w * 3]);
        }
        data
    }
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let mut encoder = png::Encoder::new(
            BufWriter::new(File::create(path)?),
            self.width as u32,
            self.height as u32,
        );
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&self.pixels)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}
impl Rect {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x as f32
            && y >= self.y as f32
            && x < (self.x + self.w) as f32
            && y < (self.y + self.h) as f32
    }
}

struct Glyph {
    metrics: Metrics,
    mask: Vec<u8>,
}
pub struct Fonts {
    mono: Font,
    logo: Font,
    icons: Font,
    cache: HashMap<(char, u16, bool), Glyph>,
    logo_mask: Vec<u8>,
    logo_width: usize,
    logo_height: usize,
    scaled_logos: HashMap<(usize, usize), Vec<u8>>,
}
impl Fonts {
    fn display_char(ch: char) -> char {
        match ch {
            '↑' => '\u{f062}',
            '↓' => '\u{f063}',
            '←' => '\u{f060}',
            '→' => '\u{f061}',
            '│' => '|',
            _ => ch,
        }
    }
    pub fn new() -> Self {
        let mono =
            Font::from_bytes(data::MONO, FontSettings::default()).expect("original Roboto Mono");
        let logo = Font::from_bytes(data::LOGO_FONT, FontSettings::default())
            .expect("original Lexend Deca");
        let icons =
            Font::from_bytes(data::ICONS, FontSettings::default()).expect("Font Awesome icons");
        let decoder = png::Decoder::new(Cursor::new(data::LOGO));
        let mut reader = decoder.read_info().expect("logo PNG");
        let mut output = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut output).expect("logo frame");
        let logo_mask = output[..info.buffer_size()]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| p[3])
            .collect();
        Self {
            mono,
            logo,
            icons,
            cache: HashMap::new(),
            logo_mask,
            logo_width: info.width as usize,
            logo_height: info.height as usize,
            scaled_logos: HashMap::new(),
        }
    }
    pub fn advance(&self, ch: char, size: u16, logo: bool) -> f32 {
        let ch = Self::display_char(ch);
        if ch as u32 >= 0xf000 {
            &self.icons
        } else if logo {
            &self.logo
        } else {
            &self.mono
        }
        .metrics(ch, size as f32)
        .advance_width
    }
    pub fn width(&self, text: &str, size: u16, logo: bool) -> f32 {
        text.chars().map(|c| self.advance(c, size, logo)).sum()
    }
    pub fn baseline(&self, size: u16) -> f32 {
        let m = self.mono.horizontal_line_metrics(size as f32).unwrap();
        (size as f32 - m.ascent + m.descent) / 2. + m.ascent
    }
    #[allow(clippy::too_many_arguments)]
    pub fn text(
        &mut self,
        canvas: &mut Canvas,
        x: f32,
        y: f32,
        text: &str,
        size: u16,
        color: Color,
        logo: bool,
    ) {
        // A line box, rather than per-glyph ink bounds, defines the baseline.
        let baseline = if logo {
            size as f32 * 0.84
        } else {
            self.baseline(size)
        };
        self.text_baseline(canvas, x, y + baseline, text, size, color, logo);
    }
    #[allow(clippy::too_many_arguments)]
    pub fn text_baseline(
        &mut self,
        canvas: &mut Canvas,
        mut x: f32,
        baseline: f32,
        text: &str,
        size: u16,
        color: Color,
        logo: bool,
    ) {
        for ch in text.chars() {
            let ch = Self::display_char(ch);
            let key = (ch, size, logo);
            let f = if ch as u32 >= 0xf000 {
                &self.icons
            } else if logo {
                &self.logo
            } else {
                &self.mono
            };
            let glyph = self.cache.entry(key).or_insert_with(|| {
                let (metrics, mask) = f.rasterize(ch, size as f32);
                Glyph { metrics, mask }
            });
            let m = glyph.metrics;
            let ox = x.floor() as i32 + m.xmin;
            let oy = baseline.floor() as i32 - m.ymin - m.height as i32;
            for gy in 0..m.height {
                for gx in 0..m.width {
                    canvas.blend(
                        ox + gx as i32,
                        oy + gy as i32,
                        color,
                        glyph.mask[gy * m.width + gx],
                    );
                }
            }
            x += m.advance_width;
        }
    }
    pub fn logo(&mut self, canvas: &mut Canvas, x: f32, y: f32, w: usize, h: usize, color: Color) {
        let mask = self.scaled_logos.entry((w, h)).or_insert_with(|| {
            let mut mask = Vec::with_capacity(w * h);
            for py in 0..h {
                for px in 0..w {
                    let (x0, x1) = (
                        px * self.logo_width / w,
                        ((px + 1) * self.logo_width / w).max(px * self.logo_width / w + 1),
                    );
                    let (y0, y1) = (
                        py * self.logo_height / h,
                        ((py + 1) * self.logo_height / h).max(py * self.logo_height / h + 1),
                    );
                    let mut sum = 0u32;
                    let mut count = 0;
                    for sy in y0..y1 {
                        for sx in x0..x1 {
                            sum += self.logo_mask[sy * self.logo_width + sx] as u32;
                            count += 1;
                        }
                    }
                    mask.push((sum / count) as u8);
                }
            }
            mask
        });
        for py in 0..h {
            for px in 0..w {
                canvas.blend(
                    x as i32 + px as i32,
                    y as i32 + py as i32,
                    color,
                    mask[py * w + px],
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_font_metrics() {
        let f = Fonts::new();
        assert!((f.advance('m', 32, false) - 19.203125).abs() < 0.001);
        assert_eq!(f.advance('i', 32, false), f.advance('W', 32, false));
    }
    #[test]
    fn baselines_preserve_lowercase_height_and_descenders() {
        let mut f = Fonts::new();
        let mut c = Canvas::new(100, 80, [0, 0, 0]);
        f.text_baseline(&mut c, 0., 40., "aAp", 32, [255, 255, 255], false);
        let bounds = |x: usize| {
            let mut top = 80;
            let mut bottom = 0;
            for y in 0..80 {
                for px in x..x + 19 {
                    if c.pixels[(y * 100 + px) * 3] > 0 {
                        top = top.min(y);
                        bottom = bottom.max(y);
                    }
                }
            }
            (top, bottom)
        };
        let a = bounds(0);
        let upper = bounds(19);
        let p = bounds(38);
        assert!(a.0 > upper.0);
        assert!(a.1.abs_diff(upper.1) <= 1);
        assert!(p.1 > a.1);
    }
}
