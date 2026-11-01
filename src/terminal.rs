use crate::gfx::{Canvas, Rect};
use base64::{Engine, engine::general_purpose::STANDARD};
use crossterm::{
    cursor::{Hide, Show},
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use flate2::{Compression, write::ZlibEncoder};
use std::io::{self, Write};

// Separate textures prevent Kitty from reconstructing and uploading the entire
// screen for each caret movement. Tiles align with terminal cells, so placement
// stays exact without scaling or fractional pixel offsets.
const TILE_COLUMNS: usize = 16;
const TILE_ROWS: usize = 4;
const IMAGE_BASE: u32 = 73100;

pub struct Encoder {
    previous: Option<Canvas>,
    geometry: (u16, u16),
    tile_count: usize,
    pub regions: Vec<Rect>,
}
impl Encoder {
    pub fn new() -> Self {
        Self {
            previous: None,
            geometry: (0, 0),
            tile_count: 0,
            regions: vec![],
        }
    }
    fn deletes(&self, output: &mut Vec<u8>) {
        for i in 0..self.tile_count {
            output.extend_from_slice(
                format!("\x1b_Ga=d,d=I,i={},q=2;\x1b\\", IMAGE_BASE + i as u32).as_bytes(),
            );
        }
    }
    pub fn encode(
        &mut self,
        canvas: &Canvas,
        columns: u16,
        rows: u16,
        damage: Option<Rect>,
    ) -> io::Result<Vec<u8>> {
        self.regions.clear();
        let columns = columns.max(1) as usize;
        let rows = rows.max(1) as usize;
        let geometry = (columns as u16, rows as u16);
        let resized = self
            .previous
            .as_ref()
            .is_none_or(|p| p.width != canvas.width || p.height != canvas.height)
            || geometry != self.geometry;
        let mut output = Vec::with_capacity(4096);
        if resized {
            self.deletes(&mut output);
        }
        let mut tile_index = 0;
        for row in (0..rows).step_by(TILE_ROWS) {
            for col in (0..columns).step_by(TILE_COLUMNS) {
                let x = col * canvas.width / columns;
                let y = row * canvas.height / rows;
                let rect = Rect {
                    x,
                    y,
                    w: (col + TILE_COLUMNS).min(columns) * canvas.width / columns - x,
                    h: (row + TILE_ROWS).min(rows) * canvas.height / rows - y,
                };
                let id = IMAGE_BASE + tile_index as u32;
                tile_index += 1;
                if rect.w == 0 || rect.h == 0 {
                    continue;
                }
                if !resized {
                    if damage.is_some_and(|d| {
                        rect.x >= d.x + d.w
                            || d.x >= rect.x + rect.w
                            || rect.y >= d.y + d.h
                            || d.y >= rect.y + rect.h
                    }) {
                        continue;
                    }
                    let previous = self.previous.as_ref().unwrap();
                    let changed = (rect.y..rect.y + rect.h).any(|y| {
                        let start = (y * canvas.width + rect.x) * 3;
                        let end = start + rect.w * 3;
                        canvas.pixels[start..end] != previous.pixels[start..end]
                    });
                    if !changed {
                        continue;
                    }
                }
                self.regions.push(rect);
                output.extend_from_slice(format!("\x1b[{};{}H", row + 1, col + 1).as_bytes());
                let metadata = format!("a=T,f=24,o=z,s={},v={},i={id},p=1,C=1,q=2", rect.w, rect.h);
                packet(&mut output, &canvas.crop(rect), &metadata)?;
            }
        }
        self.tile_count = tile_index;
        if output.is_empty() {
            return Ok(output);
        }
        // Synchronize only this small batch of texture replacements.
        let mut bytes = Vec::with_capacity(output.len() + 16);
        bytes.extend_from_slice(b"\x1b[?2026h");
        bytes.extend_from_slice(&output);
        bytes.extend_from_slice(b"\x1b[?2026l");
        if resized {
            self.previous = Some(canvas.clone());
        } else {
            let previous = self.previous.as_mut().unwrap();
            for r in &self.regions {
                for y in r.y..r.y + r.h {
                    let start = (y * canvas.width + r.x) * 3;
                    let end = start + r.w * 3;
                    previous.pixels[start..end].copy_from_slice(&canvas.pixels[start..end]);
                }
            }
        }
        self.geometry = geometry;
        Ok(bytes)
    }
}

fn packet(output: &mut Vec<u8>, pixels: &[u8], metadata: &str) -> io::Result<()> {
    let mut compressor = ZlibEncoder::new(Vec::new(), Compression::fast());
    compressor.write_all(pixels)?;
    let encoded = STANDARD.encode(compressor.finish()?);
    let chunks = encoded.as_bytes().chunks(4096);
    let count = chunks.len();
    for (i, chunk) in chunks.enumerate() {
        output.extend_from_slice(b"\x1b_G");
        output.extend_from_slice(if i == 0 { metadata.as_bytes() } else { b"q=2" });
        output.extend_from_slice(if i + 1 < count { b",m=1;" } else { b",m=0;" });
        output.extend_from_slice(chunk);
        output.extend_from_slice(b"\x1b\\");
    }
    Ok(())
}

pub struct Terminal {
    pub encoder: Encoder,
    keyboard: bool,
}
impl Terminal {
    pub fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        let mut t = Self {
            encoder: Encoder::new(),
            keyboard: false,
        };
        execute!(
            io::stdout(),
            EnterAlternateScreen,
            Hide,
            EnableMouseCapture,
            EnableBracketedPaste
        )?;
        if execute!(
            io::stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )
        .is_ok()
        {
            t.keyboard = true;
        }
        Ok(t)
    }
    pub fn draw(
        &mut self,
        canvas: &Canvas,
        columns: u16,
        rows: u16,
        damage: Option<Rect>,
    ) -> io::Result<usize> {
        let bytes = self.encoder.encode(canvas, columns, rows, damage)?;
        if !bytes.is_empty() {
            let mut stdout = io::stdout().lock();
            stdout.write_all(&bytes)?;
            stdout.flush()?;
        }
        Ok(bytes.len())
    }
}
impl Drop for Terminal {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let mut cleanup = Vec::new();
        self.encoder.deletes(&mut cleanup);
        cleanup.extend_from_slice(b"\x1b[?2026l");
        let _ = stdout.write_all(&cleanup);
        if self.keyboard {
            let _ = execute!(stdout, PopKeyboardEnhancementFlags);
        }
        let _ = execute!(
            stdout,
            DisableBracketedPaste,
            DisableMouseCapture,
            Show,
            LeaveAlternateScreen
        );
        let _ = terminal::disable_raw_mode();
    }
}

pub fn size() -> io::Result<(u16, u16, usize, usize)> {
    let s = terminal::window_size()?;
    Ok((
        s.columns.max(1),
        s.rows.max(1),
        if s.width > 0 {
            s.width as usize
        } else {
            s.columns as usize * 10
        },
        if s.height > 0 {
            s.height as usize
        } else {
            s.rows as usize * 20
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unchanged_frames_send_no_bytes() {
        let c = Canvas::new(100, 100, [32, 34, 37]);
        let mut e = Encoder::new();
        assert!(!e.encode(&c, 10, 5, None).unwrap().is_empty());
        assert!(e.encode(&c, 10, 5, None).unwrap().is_empty());
    }
    #[test]
    fn edits_replace_only_intersecting_tiles() {
        let mut c = Canvas::new(1440, 900, [50, 52, 55]);
        let mut e = Encoder::new();
        e.encode(&c, 144, 45, None).unwrap();
        c.rect(80., 380., 3., 38., 0., [226, 183, 20]);
        let packet = e
            .encode(
                &c,
                144,
                45,
                Some(Rect {
                    x: 40,
                    y: 200,
                    w: 1360,
                    h: 400,
                }),
            )
            .unwrap();
        let text = String::from_utf8_lossy(&packet);
        assert!(!text.contains("a=f"));
        assert!(text.contains("a=T"));
        assert!(text.contains("\x1b[17;1H"));
        assert!(e.regions.iter().all(|r| r.w <= 160 && r.h <= 80));
        assert!(e.regions.iter().map(|r| r.w * r.h).sum::<usize>() < 30_000);
        assert!(packet.len() < 2000);
    }
    #[test]
    fn resize_replaces_image() {
        let mut e = Encoder::new();
        e.encode(&Canvas::new(100, 100, [0, 0, 0]), 10, 5, None)
            .unwrap();
        let p = e
            .encode(&Canvas::new(200, 100, [0, 0, 0]), 20, 5, None)
            .unwrap();
        assert!(String::from_utf8_lossy(&p).contains("a=T"));
    }
}
