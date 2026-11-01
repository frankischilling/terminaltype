mod app;
mod data;
mod gfx;
mod model;
mod terminal;
mod ui;

use app::{App, Page};
use crossterm::event;
use model::Store;
use std::{
    collections::BTreeMap,
    io::{self, IsTerminal},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use ui::Renderer;

fn main() {
    if let Err(error) = run() {
        eprintln!("monkeytype-tui: {error}");
        std::process::exit(1);
    }
}
fn fail(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn run() -> io::Result<()> {
    let mut options = BTreeMap::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            println!(
                "Monkeytype, natively rendered with Kitty graphics.\n\nUsage: monkeytype-tui [options]\n\n  --mode time|words|quote|zen|custom\n  --time SECONDS          timed test (1–3600)\n  --words COUNT           word test (1–10000)\n  --custom TEXT           custom text test\n  --theme NAME            original Monkeytype theme\n  --language NAME         bundled dictionary\n  --fps HZ                maximum caret animation rate (default: 120)\n  --scale NUMBER          UI scale (default: 1)\n  --seed NUMBER           reproducible words\n  --data-dir PATH         local settings and results\n  --preview FILE.png      render without a terminal\n  --screen NAME           test, result, settings, history, about, palette\n  --size WIDTHxHEIGHT     preview / benchmark size (default: 1440x900)\n  --benchmark             measure input, rendering, and graphics encoding\n\nEsc: commands · Tab, Enter: restart · Ctrl+C: quit"
            );
            return Ok(());
        }
        if arg == "--benchmark" {
            options.insert(arg, "true".into());
            continue;
        }
        if ![
            "--mode",
            "--time",
            "--words",
            "--custom",
            "--theme",
            "--language",
            "--fps",
            "--scale",
            "--seed",
            "--data-dir",
            "--preview",
            "--screen",
            "--size",
        ]
        .contains(&arg.as_str())
        {
            return Err(fail(format!("unknown option {arg}; see --help")));
        }
        let value = args
            .next()
            .ok_or_else(|| fail(format!("missing value for {arg}")))?;
        options.insert(arg, value);
    }
    let get = |key: &str| options.get(key).map(String::as_str);
    let store = Store::new(get("--data-dir").map(PathBuf::from));
    let mut config = store.config();
    if let Some(value) = get("--mode") {
        if !["time", "words", "quote", "zen", "custom"].contains(&value) {
            return Err(fail("invalid mode"));
        }
        config.mode = value.into();
    }
    if let Some(value) = get("--time") {
        let n = value.parse::<u32>().map_err(|_| fail("invalid --time"))?;
        if !(1..=3600).contains(&n) {
            return Err(fail("--time must be 1–3600"));
        }
        config.seconds = n;
        if get("--mode").is_none() {
            config.mode = "time".into();
        }
    }
    if let Some(value) = get("--words") {
        let n = value
            .parse::<usize>()
            .map_err(|_| fail("invalid --words"))?;
        if !(1..=10000).contains(&n) {
            return Err(fail("--words must be 1–10000"));
        }
        config.words = n;
        if get("--mode").is_none() {
            config.mode = "words".into();
        }
    }
    if let Some(value) = get("--custom") {
        if value.trim().is_empty() {
            return Err(fail("custom text must not be empty"));
        }
        config.custom = value.into();
        config.mode = "custom".into();
    }
    if let Some(value) = get("--theme") {
        if !data::themes().contains_key(value) {
            return Err(fail("unknown theme; open Esc → theme to browse"));
        }
        config.theme = value.into();
    }
    if let Some(value) = get("--language") {
        if !data::LANGUAGES.iter().any(|(name, _)| *name == value) {
            return Err(fail("unknown language; open Esc → language to browse"));
        }
        config.language = value.into();
    }
    let fps = get("--fps")
        .unwrap_or("120")
        .parse::<u32>()
        .map_err(|_| fail("invalid --fps"))?;
    if !(30..=240).contains(&fps) {
        return Err(fail("--fps must be 30–240"));
    }
    let scale = get("--scale")
        .unwrap_or("1")
        .parse::<f32>()
        .map_err(|_| fail("invalid --scale"))?;
    if !(0.5..=3.).contains(&scale) {
        return Err(fail("--scale must be 0.5–3"));
    }
    let seed = get("--seed")
        .map(|v| v.parse::<u64>().map_err(|_| fail("invalid --seed")))
        .transpose()?
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64
        });
    let size = get("--size").unwrap_or("1440x900");
    let (width, height) = size
        .split_once('x')
        .ok_or_else(|| fail("--size must be WIDTHxHEIGHT"))?;
    let width = width.parse::<usize>().map_err(|_| fail("invalid width"))?;
    let height = height
        .parse::<usize>()
        .map_err(|_| fail("invalid height"))?;
    if !(320..=7680).contains(&width) || !(200..=4320).contains(&height) {
        return Err(fail("--size must be within 320x200–7680x4320"));
    }
    let mut app = App::new(store, config, seed);
    let mut renderer = Renderer::new(scale);
    if get("--benchmark").is_some() {
        benchmark(&mut app, &mut renderer, width, height)?;
        return Ok(());
    }
    if let Some(path) = get("--preview") {
        match get("--screen").unwrap_or("test") {
            "test" => {}
            "settings" => app.page = Page::Settings,
            "history" => app.page = Page::History,
            "about" => app.page = Page::About,
            "palette" => app.palette("commands"),
            "result" => {
                let t = &mut app.test;
                t.started = Some(0.);
                t.ended = Some(30.);
                t.inputs = t.targets.iter().take(40).cloned().collect();
                t.index = t.inputs.len().saturating_sub(1);
                if let Some(last) = t.inputs.last_mut() {
                    last.pop();
                }
                t.attempts = 203;
                t.correct_attempts = 199;
                t.samples = (1..=30)
                    .map(|i| {
                        let second = i as f64;
                        model::Sample {
                            second,
                            wpm: 72. + (second * 0.5).sin() * 9. + second * 0.2,
                            raw: 82. + ((second + 2.) * 0.5).sin() * 13.,
                            errors: i / 7,
                        }
                    })
                    .collect();
            }
            _ => return Err(fail("unknown preview screen")),
        }
        renderer.render(&app, width, height, 0.);
        renderer.canvas.save(&PathBuf::from(path))?;
        println!("{path}");
        return Ok(());
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(fail("run inside Kitty, or use --preview FILE.png"));
    }
    interactive(&mut app, &mut renderer, fps)
}

fn interactive(app: &mut App, renderer: &mut Renderer, fps: u32) -> io::Result<()> {
    let stop = Arc::new(AtomicBool::new(false));
    for signal in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGHUP] {
        signal_hook::flag::register(signal, Arc::clone(&stop))?;
    }
    let mut terminal = terminal::Terminal::enter()?;
    let origin = Instant::now();
    // Read independently of frame deadlines. In particular, the TTY backend
    // does not inspect its parser when poll() receives a zero timeout.
    let (sender, input) = mpsc::channel();
    std::thread::Builder::new()
        .name("terminal-input".into())
        .spawn(move || {
            loop {
                let event = event::read();
                let failed = event.is_err();
                if sender
                    .send((origin.elapsed().as_secs_f64(), event))
                    .is_err()
                    || failed
                {
                    break;
                }
            }
        })?;
    let frame_interval = 1. / fps as f64;
    let mut last_frame = -1.;
    let mut last_tick = None;
    let mut geometry = terminal::size()?;
    while app.running && !stop.load(Ordering::Relaxed) {
        let current_geometry = terminal::size()?;
        if current_geometry != geometry {
            geometry = current_geometry;
            app.dirty = true;
        }
        let now = origin.elapsed().as_secs_f64();
        let animating = renderer.animating
            && app.test.active()
            && app.page == Page::Test
            && app.modal.is_empty();
        let mut timeout: f64 = 0.1;
        if app.dirty || animating {
            timeout = (last_frame + frame_interval - now).max(0.);
        } else if app.page == Page::Test && app.modal.is_empty() && !app.test.finished() {
            let until = if app.test.active() {
                1. - app.test.elapsed(now).fract()
            } else {
                0.5 - now % 0.5
            };
            timeout = timeout.min(until);
        }
        let mut key_input = false;
        match input.recv_timeout(Duration::from_secs_f64(timeout)) {
            Ok((at, received)) => {
                let received = received?;
                key_input |= matches!(&received, event::Event::Key(k) if k.kind != event::KeyEventKind::Release);
                app.event(received, at, geometry);
                // Coalesce bursts, but never let a stream of mouse events
                // starve the next paint. The reader keeps collecting input.
                let deadline = Instant::now() + Duration::from_millis(1);
                while Instant::now() < deadline {
                    let Ok((at, received)) = input.try_recv() else {
                        break;
                    };
                    let received = received?;
                    key_input |= matches!(&received, event::Event::Key(k) if k.kind != event::KeyEventKind::Release);
                    app.event(received, at, geometry);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(io::Error::other("terminal input reader stopped"));
            }
        }
        let now = origin.elapsed().as_secs_f64();
        app.update(now);
        let tick = if app.page == Page::Test && app.modal.is_empty() && !app.test.finished() {
            Some(if app.test.active() {
                app.test.elapsed(now).floor() as u64
            } else {
                (now * 2.) as u64
            })
        } else {
            None
        };
        if tick != last_tick {
            last_tick = tick;
            app.dirty = true;
        }
        if (app.dirty || animating) && (key_input || now - last_frame >= frame_interval) {
            renderer.render(app, geometry.2, geometry.3, now);
            app.hits.clone_from(&renderer.hits);
            terminal.draw(&renderer.canvas, geometry.0, geometry.1, renderer.damage)?;
            last_frame = now;
            app.dirty = false;
        }
    }
    Ok(())
}

fn benchmark(
    app: &mut App,
    renderer: &mut Renderer,
    width: usize,
    height: usize,
) -> io::Result<()> {
    let mut encoder = terminal::Encoder::new();
    let mut renders = vec![];
    let mut encodes = vec![];
    let mut total = vec![];
    let mut bytes = vec![];
    for i in 0..720 {
        let now = i as f64 / 120.;
        let start = Instant::now();
        if i % 6 == 0 {
            let ch = app
                .test
                .target()
                .get(app.test.current().len())
                .copied()
                .unwrap_or(' ');
            app.test.insert(ch, now);
        }
        app.test.tick(now);
        renderer.render(app, width, height, now);
        let render = start.elapsed();
        let packet = encoder.encode(
            &renderer.canvas,
            (width / 10) as u16,
            (height / 20) as u16,
            renderer.damage,
        )?;
        let elapsed = start.elapsed();
        if i >= 120 {
            renders.push(render.as_secs_f64() * 1000.);
            encodes.push((elapsed - render).as_secs_f64() * 1000.);
            total.push(elapsed.as_secs_f64() * 1000.);
            bytes.push(packet.len());
        }
    }
    let percentile = |values: &[f64], p: f64| {
        let mut v = values.to_vec();
        v.sort_by(f64::total_cmp);
        v[((v.len() - 1) as f64 * p).round() as usize]
    };
    let mut traffic = bytes.clone();
    traffic.sort_unstable();
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"viewport":[width,height],"frames":total.len(),"simulated_keys_per_second":20,"render_median_ms":percentile(&renders,0.5),"encode_median_ms":percentile(&encodes,0.5),"update_median_ms":percentile(&total,0.5),"update_p95_ms":percentile(&total,0.95),"update_p99_ms":percentile(&total,0.99),"update_max_ms":percentile(&total,1.),"median_bytes_per_update":traffic[traffic.len()/2],"average_bytes_per_update":bytes.iter().sum::<usize>()/bytes.len(),"note":"CPU input + render + encode; excludes terminal GPU, display scanout and transport latency"})
        )?
    );
    Ok(())
}
