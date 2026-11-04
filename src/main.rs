mod app;
mod keys;
mod layout;
mod ocr;
mod onboarding;
mod prefs;
mod system;
mod tray;
mod ui;

use eframe::egui;

pub const DEFAULT_WINDOW: egui::Vec2 = egui::vec2(780.0, 560.0);
pub const MIN_WINDOW: egui::Vec2 = egui::vec2(640.0, 460.0);

fn geometry_file() -> Option<std::path::PathBuf> {
    eframe::storage_dir("SnipReader").map(|d| d.join("window.txt"))
}

/// Main window [width, height, x, y] saved by `save_window_geometry`.
pub fn load_window_geometry() -> Option<[f32; 4]> {
    let text = std::fs::read_to_string(geometry_file()?).ok()?;
    let v: Vec<f32> = text.split_whitespace().filter_map(|t| t.parse().ok()).collect();
    if !(v.len() == 4 && v[0] >= MIN_WINDOW.x && v[1] >= MIN_WINDOW.y) {
        return None;
    }
    // A monitor may have been unplugged since: only reuse a place that is still on screen.
    let title_bar = egui::pos2(v[2] + 40.0, v[3] + 10.0);
    system::point_on_a_display(title_bar).then(|| [v[0], v[1], v[2], v[3]])
}

pub fn save_window_geometry(g: [f32; 4]) {
    if let Some(f) = geometry_file() {
        if let Some(dir) = f.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(f, format!("{} {} {} {}", g[0], g[1], g[2], g[3]));
    }
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    // Recognition helper (see ocr.rs): no window, no Dock icon, exits right after.
    if matches!(args.get(1).map(String::as_str), Some(ocr::HELPER_LINES) | Some(ocr::HELPER_CODES)) {
        std::process::exit(ocr::helper_main(&args));
    }
    let screenshot = args
        .iter()
        .position(|a| a == "--screenshot")
        .and_then(|i| args.get(i + 1))
        .map(std::path::PathBuf::from);

    // `--read <image>`: print the text (and any QR/bar codes) found in an image, then exit.
    if let Some(file) = args.iter().position(|a| a == "--read").and_then(|i| args.get(i + 1)) {
        let opts = ocr::RecognizeOptions {
            language: "en-US".into(),
            auto_detect_language: false,
            custom_words: String::new(),
        };
        match ocr::recognize_lines(file, &opts) {
            Ok(mut lines) => {
                layout::sort_lines(&mut lines);
                let (keep, flat) = layout::join(&lines);
                println!("--- with line breaks ---\n{keep}\n--- without line breaks ---\n{flat}");
            }
            Err(e) => eprintln!("text: {e}"),
        }
        match ocr::read_codes(file) {
            Ok(codes) if !codes.is_empty() => println!("codes: {}", codes.join(", ")),
            Ok(_) => {}
            Err(e) => eprintln!("codes: {e}"),
        }
        return Ok(());
    }

    let saved = load_window_geometry();
    let mut viewport = egui::ViewportBuilder::default();
    if let Some(g) = saved {
        viewport = viewport.with_position(egui::pos2(g[2], g[3]));
    }
    let size = saved.map(|g| egui::vec2(g[0], g[1])).unwrap_or(DEFAULT_WINDOW);
    let options = eframe::NativeOptions {
        viewport: viewport
            .with_title("SnipReader")
            // An empty icon stops eframe from replacing the Dock icon with its
            // own "e" logo, so macOS shows AppIcon.icns from the .app bundle.
            .with_icon(egui::IconData::default())
            // eframe picks one GL config for all windows from the main one: without this
            // the HUD window cannot be see-through and shows black corners. The main
            // window stays opaque because its panels fill it completely.
            .with_transparent(true)
            .with_inner_size(size)
            .with_min_inner_size(MIN_WINDOW),
        // The app keeps the window size itself (window.txt): it shrinks the hidden window
        // to free its graphics memory, and eframe would remember that tiny size.
        persist_window: false,
        ..Default::default()
    };
    eframe::run_native(
        "SnipReader",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, screenshot.clone())))),
    )
}

/// System fonts so the ⌘ ⇧ ⌥ ⌃ key symbols and text look like macOS.
/// Only what is used is loaded (each font file stays in memory): SF as the text font
/// (Helvetica only if SF is missing) and Apple Symbols for the key symbols.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let candidates = [
        ("sf", "/System/Library/Fonts/SFNS.ttf", true),
        ("helvetica", "/System/Library/Fonts/Helvetica.ttc", true),
        ("symbols", "/System/Library/Fonts/Apple Symbols.ttf", false),
    ];
    let mut primary_set = false;
    for (name, path, primary) in candidates {
        if primary && primary_set {
            continue; // a text font is already loaded
        }
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert(name.to_string(), egui::FontData::from_owned(bytes));
            let fam = fonts.families.entry(egui::FontFamily::Proportional).or_default();
            if primary && !primary_set {
                fam.insert(0, name.to_string());
                primary_set = true;
            } else if !primary {
                fam.push(name.to_string());
            }
            if name == "symbols" {
                fonts
                    .families
                    .entry(egui::FontFamily::Monospace)
                    .or_default()
                    .push(name.to_string());
            }
        }
    }
    ctx.set_fonts(fonts);
}
