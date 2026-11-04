//! The welcome guide shown on first launch (and from Preferences → About): four pages,
//! each with an illustration drawn in code, so it is sharp at any size and adds no images
//! to the app.

use crate::app::*;
use crate::keys::Action;
use crate::system;
use eframe::egui::{self, epaint, Align2, Color32, FontId, Pos2, Rect, RichText, Rounding, Sense, Shape, Stroke, Vec2};

const W: f32 = 600.0;
const H: f32 = 500.0;
const ART_H: f32 = 260.0;
const PAGES: usize = 4;

const GREEN: Color32 = Color32::from_rgb(52, 199, 89);
const ORANGE: Color32 = Color32::from_rgb(255, 149, 0);

pub fn show(app: &mut App, ctx: &egui::Context) {
    ctx.show_viewport_immediate(
        egui::ViewportId::from_hash_of("welcome"),
        egui::ViewportBuilder::default()
            .with_title("Welcome to SnipReader")
            .with_inner_size([W, H])
            .with_resizable(false)
            .with_minimize_button(false)
            .with_maximize_button(false),
        |ctx, _| {
            draw(app, ctx);
            if ctx.input(|i| i.viewport().close_requested()) {
                finish(app);
            }
        },
    );
}

fn finish(app: &mut App) {
    app.welcome_open = false;
    app.welcome_page = 0;
    app.s.onboarded = true;
}

fn draw(app: &mut App, ctx: &egui::Context) {
    let page = app.welcome_page.min(PAGES - 1);
    let t = ctx.input(|i| i.time) as f32;

    egui::TopBottomPanel::bottom("welcome_nav")
        .exact_height(64.0)
        .show_separator_line(false)
        .frame(egui::Frame::none().fill(WINDOW_BG).inner_margin(egui::Margin::symmetric(28.0, 0.0)))
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                // Back
                ui.allocate_ui(Vec2::new(110.0, 32.0), |ui| {
                    if page > 0 && button(ui, "Back", false) {
                        app.welcome_page -= 1;
                    }
                });
                // Page dots, centred in the window
                let gap = 14.0;
                let x0 = W / 2.0 - gap * (PAGES as f32 - 1.0) / 2.0;
                let y = ui.max_rect().center().y;
                for i in 0..PAGES {
                    let c = if i == page { ACCENT } else { Color32::from_gray(0xC4) };
                    let r = if i == page { 4.5 } else { 3.5 };
                    ui.painter().circle_filled(egui::pos2(x0 + gap * i as f32, y), r, c);
                }
                // Continue / Start
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let last = page == PAGES - 1;
                    if button(ui, if last { "Start Using SnipReader" } else { "Continue" }, true) {
                        if last {
                            finish(app);
                        } else {
                            app.welcome_page += 1;
                        }
                    }
                });
            });
        });

    egui::CentralPanel::default().frame(egui::Frame::none().fill(WINDOW_BG)).show(ctx, |ui| {
        let (art, _) = ui.allocate_exact_size(Vec2::new(W, ART_H), Sense::hover());
        let p = ui.painter_at(art);
        backdrop(&p, art, page);
        match page {
            0 => art_welcome(&p, art),
            1 => art_permission(&p, art, allowed(), t),
            2 => art_capture(&p, art, &shortcut_label(app)),
            _ => art_menu_bar(&p, art, &shortcut_label(app)),
        }

        ui.add_space(22.0);
        egui::Frame::none().inner_margin(egui::Margin::symmetric(36.0, 0.0)).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            match page {
                0 => text_welcome(ui),
                1 => text_permission(app, ui),
                2 => text_capture(app, ui),
                _ => text_menu_bar(app, ui),
            }
        });
    });

    // The permission page watches for the switch being turned on.
    if page == 1 {
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }
}

/// Screen Recording allowed? (SNIPREADER_WELCOME_DENIED pretends not, to check the page.)
fn allowed() -> bool {
    system::screen_access() && std::env::var("SNIPREADER_WELCOME_DENIED").is_err()
}

fn shortcut_label(app: &App) -> String {
    app.s.shortcuts.get(&Action::CaptureText).map(|s| s.display()).unwrap_or_else(|| "⇧⌘2".into())
}

// ---------- text of each page ----------

fn title(ui: &mut egui::Ui, s: &str) {
    ui.label(RichText::new(s).size(22.0).strong().color(TEXT));
}

fn body(ui: &mut egui::Ui, s: &str) {
    ui.label(RichText::new(s).size(13.5).color(MUTED));
}

fn text_welcome(ui: &mut egui::Ui) {
    title(ui, "Welcome to SnipReader");
    body(
        ui,
        "Copy text from anywhere on your screen: images, videos, PDFs, screenshots, \
         and apps that won't let you select text. Pick an area and the text lands in \
         your clipboard.",
    );
    body(ui, "Recognition runs on your Mac with Apple's Vision framework. Nothing is sent anywhere.");
}

fn text_permission(app: &mut App, ui: &mut egui::Ui) {
    title(ui, "Allow Screen Recording");
    body(
        ui,
        "To read the area you select, macOS needs your permission. SnipReader only \
         looks at the screen when you capture.",
    );
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if allowed() {
            pill(ui, "✓  Allowed", GREEN);
        } else {
            let label = if app.welcome_asked { "Open System Settings" } else { "Allow Screen Recording…" };
            if button(ui, label, true) {
                if app.welcome_asked {
                    system::open_screen_recording_settings();
                } else {
                    app.welcome_asked = true;
                    system::request_screen_access(); // shows the macOS prompt
                }
            }
            ui.add_space(8.0);
            pill(ui, "Not allowed yet", ORANGE);
        }
    });
    if app.welcome_asked && !allowed() {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Switched it on but still see this? macOS applies it after a restart.").size(12.0).color(MUTED));
            if ui.link(RichText::new("Quit & Reopen").size(12.0)).clicked() {
                system::relaunch();
                app.quit(ui.ctx());
            }
        });
    }
}

fn text_capture(app: &mut App, ui: &mut egui::Ui) {
    let sc = shortcut_label(app);
    title(ui, "Capture text in two moves");
    body(
        ui,
        &format!(
            "Press {sc} (or click Capture Text), then drag over any text. A message \
             confirms it is copied, ready to paste. Press Esc to cancel."
        ),
    );
    ui.add_space(4.0);
    if button(ui, "Try It Now", true) {
        app.run(Action::CaptureText);
    }
}

fn text_menu_bar(app: &mut App, ui: &mut egui::Ui) {
    title(ui, "It lives in your menu bar");
    body(
        ui,
        "Closing the window keeps SnipReader running. Click its icon in the menu bar to \
         capture, read QR codes, switch modes or open Preferences.",
    );
    ui.add_space(4.0);
    let before = app.s.launch_at_login;
    ui.checkbox(&mut app.s.launch_at_login, RichText::new("Open SnipReader when I log in").size(13.0).color(TEXT));
    if app.s.launch_at_login != before {
        system::set_launch_at_login(app.s.launch_at_login);
    }
}

// ---------- controls ----------

fn button(ui: &mut egui::Ui, label: &str, primary: bool) -> bool {
    let (fill, fg, stroke) = if primary {
        (ACCENT, Color32::WHITE, Stroke::NONE)
    } else {
        (Color32::WHITE, TEXT, Stroke::new(1.0, BORDER))
    };
    ui.add(
        egui::Button::new(RichText::new(label).size(13.5).color(fg))
            .fill(fill)
            .stroke(stroke)
            .rounding(Rounding::same(8.0))
            .min_size(Vec2::new(96.0, 32.0)),
    )
    .clicked()
}

fn pill(ui: &mut egui::Ui, label: &str, color: Color32) {
    egui::Frame::none()
        .fill(color.gamma_multiply(0.14))
        .rounding(Rounding::same(12.0))
        .inner_margin(egui::Margin::symmetric(10.0, 4.0))
        .show(ui, |ui| {
            // Darker text than the tint, so it stays readable.
            let dark = Color32::from_rgb((color.r() as f32 * 0.72) as u8, (color.g() as f32 * 0.72) as u8, (color.b() as f32 * 0.72) as u8);
            ui.label(RichText::new(label).size(12.5).color(dark));
        });
}

// ---------- drawing helpers ----------

fn backdrop(p: &egui::Painter, r: Rect, page: usize) {
    let tints = [(0xEAF2FF, 0xF3EEFF), (0xEAF7F0, 0xEAF2FF), (0xFFF3E6, 0xEAF2FF), (0xE9EEFF, 0xF6ECFF)];
    let (a, b) = tints[page];
    let ca = hex(a);
    let cb = hex(b);
    let steps = 24;
    for i in 0..steps {
        let t0 = i as f32 / steps as f32;
        let band = Rect::from_min_max(
            egui::pos2(r.left(), r.top() + r.height() * t0),
            egui::pos2(r.right(), r.top() + r.height() * (t0 + 1.0 / steps as f32) + 0.5),
        );
        p.rect_filled(band, Rounding::ZERO, mix(ca, cb, t0));
    }
    p.line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, BORDER));
}

fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()))
}

/// White card with a soft shadow.
fn card(p: &egui::Painter, r: Rect, radius: f32) {
    p.add(epaint::Shadow { offset: Vec2::new(0.0, 6.0), blur: 20.0, spread: 0.0, color: Color32::from_black_alpha(34) }.as_shape(r, Rounding::same(radius)));
    p.rect(r, Rounding::same(radius), Color32::WHITE, Stroke::new(1.0, Color32::from_black_alpha(18)));
}

fn text(p: &egui::Painter, pos: Pos2, s: &str, size: f32, color: Color32) -> Rect {
    p.text(pos, Align2::LEFT_CENTER, s, FontId::proportional(size), color)
}

fn arrow(p: &egui::Painter, from: Pos2, to: Pos2, color: Color32) {
    let s = Stroke::new(2.5, color);
    let d = (to - from).normalized();
    let n = Vec2::new(-d.y, d.x);
    let tip = to;
    let base = to - d * 10.0;
    p.line_segment([from, base], s);
    p.add(Shape::convex_polygon(vec![tip, base + n * 6.0, base - n * 6.0], color, Stroke::NONE));
}

fn check_badge(p: &egui::Painter, c: Pos2, r: f32) {
    p.circle_filled(c, r, GREEN);
    let s = Stroke::new(r * 0.24, Color32::WHITE);
    let a = c + Vec2::new(-r * 0.42, r * 0.02);
    let b = c + Vec2::new(-r * 0.12, r * 0.32);
    let d = c + Vec2::new(r * 0.44, -r * 0.30);
    p.line_segment([a, b], s);
    p.line_segment([b, d], s);
}

fn dashed_rect(p: &egui::Painter, r: Rect, color: Color32) {
    let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    p.extend(Shape::dashed_line(&pts, Stroke::new(1.5, color), 5.0, 3.5));
}

fn crosshair(p: &egui::Painter, c: Pos2) {
    let s = Stroke::new(1.5, TEXT);
    p.line_segment([c - Vec2::new(8.0, 0.0), c + Vec2::new(8.0, 0.0)], s);
    p.line_segment([c - Vec2::new(0.0, 8.0), c + Vec2::new(0.0, 8.0)], s);
}

fn window_dots(p: &egui::Painter, at: Pos2) {
    for (i, c) in [(255, 95, 87), (254, 188, 46), (40, 200, 64)].iter().enumerate() {
        p.circle_filled(at + Vec2::new(i as f32 * 13.0, 0.0), 4.5, Color32::from_rgb(c.0, c.1, c.2));
    }
}

fn keycap(p: &egui::Painter, r: Rect, label: &str) {
    p.rect_filled(r.translate(Vec2::new(0.0, 2.0)), Rounding::same(7.0), Color32::from_gray(0xC8));
    p.rect(r, Rounding::same(7.0), Color32::WHITE, Stroke::new(1.0, Color32::from_gray(0xCF)));
    p.text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(r.height() * 0.48), TEXT);
}

// ---------- illustrations ----------

/// A window with text; two lines selected; an arrow into a clipboard card.
fn art_welcome(p: &egui::Painter, r: Rect) {
    let win = Rect::from_min_size(r.left_top() + Vec2::new(60.0, 40.0), Vec2::new(300.0, 190.0));
    card(p, win, 12.0);
    window_dots(p, win.left_top() + Vec2::new(16.0, 16.0));
    // A picture on the left.
    let pic = Rect::from_min_size(win.left_top() + Vec2::new(18.0, 38.0), Vec2::new(84.0, 64.0));
    p.rect_filled(pic, Rounding::same(8.0), hex(0xDCE6FF));
    p.add(Shape::convex_polygon(
        vec![pic.left_bottom() + Vec2::new(8.0, -8.0), pic.center() + Vec2::new(-6.0, -6.0), pic.right_bottom() + Vec2::new(-20.0, -8.0)],
        hex(0x8FA8E8),
        Stroke::NONE,
    ));
    p.circle_filled(pic.right_top() + Vec2::new(-18.0, 16.0), 7.0, hex(0xFFC857));
    // Text.
    let tx = win.left() + 116.0;
    text(p, egui::pos2(tx, win.top() + 46.0), "Q3 Planning", 13.0, TEXT);
    text(p, egui::pos2(tx, win.top() + 66.0), "Notes from Monday", 10.5, MUTED);
    let lines = ["Meeting moved to 10:30", "Room 4B, second floor", "Bring the printed agenda"];
    for (i, l) in lines.iter().enumerate() {
        text(p, egui::pos2(win.left() + 18.0, win.top() + 122.0 + i as f32 * 20.0), l, 12.0, Color32::from_gray(0x44));
    }
    // Selection over the first two lines.
    let sel = Rect::from_min_max(egui::pos2(win.left() + 12.0, win.top() + 110.0), egui::pos2(win.left() + 168.0, win.top() + 154.0));
    p.rect_filled(sel, Rounding::same(3.0), ACCENT.gamma_multiply(0.10));
    dashed_rect(p, sel, ACCENT);
    for c in [sel.left_top(), sel.right_top(), sel.left_bottom(), sel.right_bottom()] {
        p.rect(Rect::from_center_size(c, Vec2::splat(6.0)), Rounding::same(1.0), Color32::WHITE, Stroke::new(1.5, ACCENT));
    }
    crosshair(p, sel.right_bottom() + Vec2::new(10.0, 10.0));

    // Clipboard card.
    let clip = Rect::from_min_size(r.left_top() + Vec2::new(400.0, 92.0), Vec2::new(160.0, 96.0));
    arrow(p, egui::pos2(win.right() + 8.0, clip.center().y), egui::pos2(clip.left() - 8.0, clip.center().y), ACCENT);
    card(p, clip, 12.0);
    text(p, clip.left_top() + Vec2::new(14.0, 18.0), "CLIPBOARD", 9.5, MUTED);
    text(p, clip.left_top() + Vec2::new(14.0, 44.0), "Meeting moved to 10:30", 11.0, TEXT);
    text(p, clip.left_top() + Vec2::new(14.0, 62.0), "Room 4B, second floor", 11.0, TEXT);
    check_badge(p, clip.right_top() + Vec2::new(-4.0, 4.0), 12.0);
}

/// A System Settings mock-up with SnipReader's switch; a pulsing ring until it is on.
fn art_permission(p: &egui::Painter, r: Rect, allowed: bool, t: f32) {
    let win = Rect::from_center_size(r.center() + Vec2::new(0.0, 6.0), Vec2::new(420.0, 190.0));
    card(p, win, 12.0);
    // Sidebar
    let side = Rect::from_min_max(win.left_top(), egui::pos2(win.left() + 130.0, win.bottom()));
    p.rect_filled(side, Rounding { nw: 12.0, sw: 12.0, ne: 0.0, se: 0.0 }, Color32::from_gray(0xF2));
    window_dots(p, win.left_top() + Vec2::new(16.0, 16.0));
    let items = ["General", "Appearance", "Privacy & Security", "Displays"];
    for (i, it) in items.iter().enumerate() {
        let y = win.top() + 50.0 + i as f32 * 26.0;
        if i == 2 {
            p.rect_filled(Rect::from_min_size(egui::pos2(side.left() + 8.0, y - 11.0), Vec2::new(114.0, 22.0)), Rounding::same(6.0), ACCENT);
            text(p, egui::pos2(side.left() + 16.0, y), it, 11.0, Color32::WHITE);
        } else {
            text(p, egui::pos2(side.left() + 16.0, y), it, 11.0, Color32::from_gray(0x55));
        }
    }
    // Content
    let cx = side.right() + 20.0;
    text(p, egui::pos2(cx, win.top() + 32.0), "Screen & System Audio Recording", 13.0, TEXT);
    text(p, egui::pos2(cx, win.top() + 52.0), "Allow the apps below to record your screen.", 10.0, MUTED);
    let row = Rect::from_min_max(egui::pos2(cx, win.top() + 76.0), egui::pos2(win.right() - 18.0, win.top() + 124.0));
    p.rect(row, Rounding::same(8.0), Color32::from_gray(0xFA), Stroke::new(1.0, Color32::from_gray(0xE4)));
    crate::ui::paint_app_icon(p, Rect::from_min_size(row.left_top() + Vec2::new(10.0, 10.0), Vec2::splat(28.0)));
    text(p, egui::pos2(row.left() + 48.0, row.center().y), "SnipReader", 12.5, TEXT);
    // The switch (shown on: this is what to do).
    let sw = Rect::from_center_size(egui::pos2(row.right() - 34.0, row.center().y), Vec2::new(38.0, 22.0));
    if !allowed {
        let pulse = (t * 2.2).sin() * 0.5 + 0.5;
        p.rect_stroke(sw.expand(5.0 + pulse * 4.0), Rounding::same(16.0), Stroke::new(2.0, ACCENT.gamma_multiply(0.6 - pulse * 0.4)));
    }
    p.rect_filled(sw, Rounding::same(11.0), if allowed { GREEN } else { ACCENT });
    p.circle_filled(egui::pos2(sw.right() - 11.0, sw.center().y), 9.0, Color32::WHITE);
    // Other apps, greyed
    for i in 0..2 {
        let y = row.bottom() + 16.0 + i as f32 * 18.0;
        p.rect_filled(Rect::from_min_size(egui::pos2(cx + 10.0, y - 5.0), Vec2::new(100.0 - i as f32 * 24.0, 8.0)), Rounding::same(4.0), Color32::from_gray(0xE6));
    }
    if allowed {
        check_badge(p, win.right_top() + Vec2::new(-6.0, 6.0), 14.0);
    }
}

/// Three tiles: the shortcut keys → dragging over text → the "Copied" message.
fn art_capture(p: &egui::Painter, r: Rect, shortcut: &str) {
    let tile = Vec2::new(150.0, 150.0);
    let gap = 38.0;
    let x0 = r.center().x - (tile.x * 3.0 + gap * 2.0) / 2.0;
    let y0 = r.center().y - tile.y / 2.0 + 4.0;
    let tiles: Vec<Rect> = (0..3).map(|i| Rect::from_min_size(egui::pos2(x0 + i as f32 * (tile.x + gap), y0), tile)).collect();
    for (i, t) in tiles.iter().enumerate() {
        card(p, *t, 14.0);
        let badge = t.left_top() + Vec2::new(18.0, 18.0);
        p.circle_filled(badge, 10.0, ACCENT);
        p.text(badge, Align2::CENTER_CENTER, format!("{}", i + 1), FontId::proportional(11.0), Color32::WHITE);
        if i < 2 {
            arrow(p, egui::pos2(t.right() + 8.0, t.center().y), egui::pos2(t.right() + gap - 8.0, t.center().y), ACCENT.gamma_multiply(0.7));
        }
    }
    // 1: keys
    let keys: Vec<String> = split_shortcut(shortcut);
    let kw = 34.0;
    let total = keys.len() as f32 * kw + (keys.len() as f32 - 1.0) * 6.0;
    for (i, k) in keys.iter().enumerate() {
        let kr = Rect::from_min_size(egui::pos2(tiles[0].center().x - total / 2.0 + i as f32 * (kw + 6.0), tiles[0].center().y - 12.0), Vec2::new(kw, kw));
        keycap(p, kr, k);
    }
    p.text(tiles[0].center_bottom() - Vec2::new(0.0, 22.0), Align2::CENTER_CENTER, "Press", FontId::proportional(11.5), MUTED);
    // 2: drag over text
    let t2 = tiles[1];
    for i in 0..4 {
        let w = [96.0, 110.0, 80.0, 100.0][i];
        p.rect_filled(Rect::from_min_size(egui::pos2(t2.left() + 22.0, t2.top() + 44.0 + i as f32 * 15.0), Vec2::new(w, 6.0)), Rounding::same(3.0), Color32::from_gray(0xCF));
    }
    let sel = Rect::from_min_max(egui::pos2(t2.left() + 16.0, t2.top() + 52.0), egui::pos2(t2.left() + 136.0, t2.top() + 85.0));
    p.rect_filled(sel, Rounding::same(3.0), ACCENT.gamma_multiply(0.12));
    dashed_rect(p, sel, ACCENT);
    crosshair(p, sel.right_bottom() + Vec2::new(-4.0, 8.0));
    p.text(t2.center_bottom() - Vec2::new(0.0, 22.0), Align2::CENTER_CENTER, "Drag over text", FontId::proportional(11.5), MUTED);
    // 3: the HUD
    let t3 = tiles[2];
    let hud = Rect::from_center_size(t3.center() - Vec2::new(0.0, 4.0), Vec2::new(124.0, 40.0));
    p.add(epaint::Shadow { offset: Vec2::new(0.0, 4.0), blur: 12.0, spread: 0.0, color: Color32::from_black_alpha(40) }.as_shape(hud, Rounding::same(20.0)));
    p.rect(hud, Rounding::same(20.0), Color32::WHITE, Stroke::new(1.0, Color32::from_black_alpha(20)));
    check_badge(p, hud.left_center() + Vec2::new(20.0, 0.0), 12.0);
    text(p, hud.left_center() + Vec2::new(38.0, 0.0), "Copied", 12.5, TEXT);
    p.text(t3.center_bottom() - Vec2::new(0.0, 22.0), Align2::CENTER_CENTER, "Paste anywhere", FontId::proportional(11.5), MUTED);
}

/// "⇧⌘2" → ["⇧", "⌘", "2"]; a key name like "F5" stays together.
fn split_shortcut(s: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut rest = String::new();
    for c in s.chars() {
        if "⌃⌥⇧⌘".contains(c) {
            keys.push(c.to_string());
        } else {
            rest.push(c);
        }
    }
    if !rest.is_empty() {
        keys.push(rest);
    }
    keys
}

/// A menu bar with SnipReader's icon and its open menu.
fn art_menu_bar(p: &egui::Painter, r: Rect, shortcut: &str) {
    // Menu bar strip
    let bar = Rect::from_min_size(r.left_top() + Vec2::new(40.0, 30.0), Vec2::new(r.width() - 80.0, 26.0));
    p.rect(bar, Rounding::same(8.0), Color32::from_white_alpha(235), Stroke::new(1.0, Color32::from_black_alpha(14)));
    for (i, w) in [28.0, 34.0, 26.0, 40.0].iter().enumerate() {
        p.rect_filled(Rect::from_min_size(egui::pos2(bar.left() + 16.0 + i as f32 * 52.0, bar.center().y - 4.0), Vec2::new(*w, 8.0)), Rounding::same(4.0), Color32::from_gray(0xC8));
    }
    text(p, egui::pos2(bar.right() - 70.0, bar.center().y), "Wed 9:41", 11.0, Color32::from_gray(0x44));
    // Our icon, highlighted (menu-bar template glyph)
    let icon = Rect::from_center_size(egui::pos2(bar.right() - 110.0, bar.center().y), Vec2::new(26.0, 20.0));
    p.rect_filled(icon, Rounding::same(5.0), Color32::from_black_alpha(26));
    let g = Rect::from_center_size(icon.center(), Vec2::splat(12.0));
    let s = Stroke::new(1.6, TEXT);
    for (c, dx, dy) in [(g.left_top(), 1.0, 1.0), (g.right_top(), -1.0, 1.0), (g.left_bottom(), 1.0, -1.0), (g.right_bottom(), -1.0, -1.0)] {
        p.line_segment([c, c + Vec2::new(4.0 * dx, 0.0)], s);
        p.line_segment([c, c + Vec2::new(0.0, 4.0 * dy)], s);
    }
    p.line_segment([g.center() + Vec2::new(-3.0, -1.5), g.center() + Vec2::new(3.0, -1.5)], s);
    p.line_segment([g.center() + Vec2::new(-3.0, 2.0), g.center() + Vec2::new(1.5, 2.0)], s);
    for i in 0..3 {
        p.circle_filled(egui::pos2(bar.right() - 160.0 + i as f32 * -22.0, bar.center().y), 5.0, Color32::from_gray(0xBE));
    }
    // The open menu
    let menu = Rect::from_min_size(egui::pos2(icon.left() - 150.0, bar.bottom() + 8.0), Vec2::new(210.0, 170.0));
    card(p, menu, 10.0);
    let rows: [(&str, &str, bool); 8] = [
        ("Capture Text", shortcut, false),
        ("Read QR/Bar Code", "", false),
        ("-", "", false),
        ("Keep Line Breaks", "", true),
        ("Additive Clipboard", "", false),
        ("-", "", false),
        ("Preferences…", "⌘,", false),
        ("Quit SnipReader", "⌘Q", false),
    ];
    let mut y = menu.top() + 10.0;
    for (i, (label, sc, checked)) in rows.iter().enumerate() {
        if *label == "-" {
            p.line_segment([egui::pos2(menu.left() + 12.0, y + 4.0), egui::pos2(menu.right() - 12.0, y + 4.0)], Stroke::new(1.0, Color32::from_gray(0xE2)));
            y += 9.0;
            continue;
        }
        let row = Rect::from_min_size(egui::pos2(menu.left() + 6.0, y), Vec2::new(menu.width() - 12.0, 22.0));
        let hot = i == 0;
        if hot {
            p.rect_filled(row, Rounding::same(5.0), ACCENT);
        }
        let fg = if hot { Color32::WHITE } else { TEXT };
        if *checked {
            text(p, egui::pos2(row.left() + 6.0, row.center().y), "✓", 11.5, fg);
        }
        text(p, egui::pos2(row.left() + 22.0, row.center().y), label, 12.0, fg);
        if !sc.is_empty() {
            p.text(egui::pos2(row.right() - 8.0, row.center().y), Align2::RIGHT_CENTER, *sc, FontId::proportional(11.5), if hot { Color32::WHITE } else { MUTED });
        }
        y += 22.0;
    }
}
