//! Drawing the main window and the HUD message.

use crate::app::*;
use crate::keys::Action;
use eframe::egui::{self, Align, Align2, Color32, FontId, Layout, RichText, Rounding, Sense, Stroke, Vec2};

pub fn app_icon(ui: &mut egui::Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    paint_app_icon(ui.painter(), rect);
}

/// The app icon (gradient rounded square, selection corners, two text lines) in `rect`.
pub fn paint_app_icon(p: &egui::Painter, rect: egui::Rect) {
    let size = rect.width();
    // Gradient rounded square, same colours as the Dock icon. The gradient is one mesh in
    // the rounded shape (horizontal bands used to square off the corners); an anti-aliased
    // fill underneath keeps the edge smooth.
    let top = Color32::from_rgb(77, 158, 255);
    let bottom = Color32::from_rgb(26, 92, 230);
    let r = size * 0.22;
    p.rect_filled(rect, Rounding::same(r), lerp(top, bottom, 0.5));
    let inner = rect.shrink(0.5);
    let ri = (r - 0.5).max(0.0);
    let mut outline = Vec::new();
    for (cx, cy, a0) in [
        (inner.right() - ri, inner.top() + ri, -90.0_f32),
        (inner.right() - ri, inner.bottom() - ri, 0.0),
        (inner.left() + ri, inner.bottom() - ri, 90.0),
        (inner.left() + ri, inner.top() + ri, 180.0),
    ] {
        for i in 0..=8 {
            let a = (a0 + 90.0 * i as f32 / 8.0).to_radians();
            outline.push(egui::pos2(cx + ri * a.cos(), cy + ri * a.sin()));
        }
    }
    let mut mesh = egui::epaint::Mesh::default();
    let shade = |y: f32| lerp(top, bottom, ((y - rect.top()) / rect.height()).clamp(0.0, 1.0));
    mesh.colored_vertex(rect.center(), shade(rect.center().y));
    for q in &outline {
        mesh.colored_vertex(*q, shade(q.y));
    }
    let n = outline.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    p.add(egui::Shape::mesh(mesh));
    // Selection corners + text lines.
    let s = Stroke::new(size * 0.06, Color32::WHITE);
    let inset = size * 0.22;
    let l = size * 0.18;
    let a = rect.shrink(inset);
    // Each selection corner: arm, rounded bend, arm, with round ends (like the Dock icon).
    let bend = l * 0.45;
    for (c, dx, dy) in [
        (a.left_top(), 1.0, 1.0),
        (a.right_top(), -1.0, 1.0),
        (a.left_bottom(), 1.0, -1.0),
        (a.right_bottom(), -1.0, -1.0),
    ] {
        let centre = c + Vec2::new(bend * dx, bend * dy);
        let mut pts = vec![c + Vec2::new(l * dx, 0.0)];
        for i in 0..=8 {
            let t = i as f32 / 8.0 * std::f32::consts::FRAC_PI_2;
            pts.push(centre + Vec2::new(-t.sin() * bend * dx, -t.cos() * bend * dy));
        }
        pts.push(c + Vec2::new(0.0, l * dy));
        p.circle_filled(pts[0], s.width / 2.0, s.color);
        p.circle_filled(*pts.last().unwrap(), s.width / 2.0, s.color);
        p.add(egui::Shape::line(pts, s));
    }
    let ls = Stroke::new(size * 0.05, Color32::from_white_alpha(230));
    let cx = a.center();
    p.line_segment([cx + Vec2::new(-size * 0.16, -size * 0.06), cx + Vec2::new(size * 0.16, -size * 0.06)], ls);
    p.line_segment([cx + Vec2::new(-size * 0.16, size * 0.06), cx + Vec2::new(size * 0.08, size * 0.06)], ls);
}

fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()))
}

/// One row of a macOS-style menu.
fn menu_item(ui: &mut egui::Ui, label: &str, shortcut: Option<String>, checked: Option<bool>, enabled: bool) -> bool {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 24.0), if enabled { Sense::click() } else { Sense::hover() });
    let hovered = enabled && resp.hovered();
    let p = ui.painter();
    if hovered {
        p.rect_filled(rect.shrink2(Vec2::new(4.0, 0.0)), Rounding::same(4.0), ACCENT);
    }
    let color = if !enabled {
        Color32::from_gray(0xA8)
    } else if hovered {
        Color32::WHITE
    } else {
        TEXT
    };
    if checked == Some(true) {
        p.text(rect.left_center() + Vec2::new(12.0, 0.0), Align2::LEFT_CENTER, "✓", FontId::proportional(13.0), color);
    }
    p.text(rect.left_center() + Vec2::new(28.0, 0.0), Align2::LEFT_CENTER, label, FontId::proportional(13.5), color);
    if let Some(sc) = shortcut {
        let sc_color = if hovered { Color32::WHITE } else { Color32::from_gray(0x8A) };
        p.text(rect.right_center() - Vec2::new(14.0, 0.0), Align2::RIGHT_CENTER, sc, FontId::proportional(13.0), sc_color);
    }
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    enabled && resp.clicked()
}

fn separator(ui: &mut egui::Ui) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 9.0), Sense::hover());
    ui.painter().line_segment(
        [rect.left_center() + Vec2::new(12.0, 0.0), rect.right_center() - Vec2::new(12.0, 0.0)],
        Stroke::new(1.0, Color32::from_gray(0xDD)),
    );
}

fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::none()
        .fill(CARD_BG)
        .rounding(Rounding::same(10.0))
        .stroke(Stroke::new(1.0, BORDER))
        .inner_margin(egui::Margin::same(12.0))
        .show(ui, add)
        .inner
}

fn sc(app: &App, a: Action) -> Option<String> {
    app.s.shortcuts.get(&a).map(|s| s.display())
}

pub fn main_window(app: &mut App, ctx: &egui::Context) {
    egui::TopBottomPanel::top("header").show_separator_line(false)
        .exact_height(68.0)
        .frame(egui::Frame::none().fill(WINDOW_BG).inner_margin(egui::Margin::symmetric(18.0, 12.0)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                app_icon(ui, 42.0);
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.add_space(2.0);
                    ui.label(RichText::new("SnipReader").size(19.0).strong());
                    ui.label(RichText::new("Grab text from anywhere on your screen").size(12.0).color(MUTED));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let label = if app.busy { "Capturing…" } else { "Capture Text" };
                    let b = egui::Button::new(RichText::new(label).color(Color32::WHITE).size(14.0).strong())
                        .fill(ACCENT)
                        .rounding(Rounding::same(7.0))
                        .min_size(Vec2::new(150.0, 34.0));
                    if ui.add_enabled(!app.busy, b).clicked() {
                        app.run(Action::CaptureText);
                    }
                    if let Some(s) = sc(app, Action::CaptureText) {
                        ui.label(RichText::new(s).size(13.0).color(MUTED));
                    }
                });
            });
        });

    egui::TopBottomPanel::bottom("status").show_separator_line(false)
        .exact_height(34.0)
        .frame(egui::Frame::none().fill(WINDOW_BG).inner_margin(egui::Margin::symmetric(18.0, 7.0)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                chip(ui, "Keep Line Breaks", app.s.keep_line_breaks);
                chip(ui, "Additive Clipboard", app.s.additive);
                chip(ui, "Text to Speech", app.s.speak_after_capture);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(format!("Language: {}", app.s.language)).size(11.5).color(MUTED));
                });
            });
        });

    egui::SidePanel::left("menu").show_separator_line(false)
        .exact_width(268.0)
        .resizable(false)
        .frame(egui::Frame::none().fill(WINDOW_BG).inner_margin(egui::Margin { left: 18.0, right: 8.0, top: 4.0, bottom: 10.0 }))
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(Color32::from_rgb(0xF6, 0xF6, 0xF7))
                .rounding(Rounding::same(10.0))
                .stroke(Stroke::new(1.0, BORDER))
                .shadow(egui::Shadow { offset: Vec2::new(0.0, 3.0), blur: 12.0, spread: 0.0, color: Color32::from_black_alpha(28) })
                .inner_margin(egui::Margin::symmetric(0.0, 6.0))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let idle = !app.busy;
                    if menu_item(ui, "Capture Text", sc(app, Action::CaptureText), None, idle) {
                        app.run(Action::CaptureText);
                    }
                    if menu_item(ui, "Read QR/Bar Code", sc(app, Action::ReadCode), None, idle) {
                        app.run(Action::ReadCode);
                    }
                    if menu_item(ui, "Read Text From Image…", None, None, idle) {
                        app.read_image_file();
                    }
                    separator(ui);
                    if menu_item(ui, "Keep Line Breaks", None, Some(app.s.keep_line_breaks), true) {
                        app.toggle_keep_breaks();
                    }
                    if menu_item(ui, "Additive Clipboard", sc(app, Action::ToggleAdditive), Some(app.s.additive), true) {
                        app.toggle_additive();
                    }
                    if menu_item(ui, "Clear Clipboard History", None, None, true) {
                        app.clear_history();
                    }
                    separator(ui);
                    if menu_item(ui, "Text to Speech", None, Some(app.s.speak_after_capture), true) {
                        app.run(Action::ModeSpeech);
                    }
                    let speaking = app.is_speaking();
                    if menu_item(ui, "Stop Speaking", sc(app, Action::StopSpeaking), None, speaking) {
                        app.stop_speaking();
                    }
                    separator(ui);
                    if menu_item(ui, "Preferences...", Some("⌘,".into()), None, true) {
                        app.prefs_open = true;
                    }
                    if menu_item(ui, "Quit SnipReader", Some("⌘Q".into()), None, true) {
                        app.quit(ctx);
                    }
                });
            ui.add_space(10.0);
            ui.label(
                RichText::new("Tip: pick an area with the mouse; press Esc to cancel. The text goes straight to your clipboard.")
                    .size(11.5)
                    .color(MUTED),
            );
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(WINDOW_BG).inner_margin(egui::Margin { left: 8.0, right: 18.0, top: 4.0, bottom: 10.0 }))
        .show(ctx, |ui| {
            let total_h = ui.available_height();
            let selected = app.selected.and_then(|i| app.history.get(i)).cloned();
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.set_height((total_h * 0.48).max(150.0) - 24.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Captured Text").size(14.0).strong());
                    if let Some(e) = &selected {
                        ui.label(RichText::new(e.kind.clone()).size(11.0).color(MUTED));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if let Some(e) = &selected {
                            if ui.button("Speak").clicked() {
                                let t = e.text.clone();
                                app.speak(&t);
                            }
                            if ui.button("Copy").clicked() {
                                crate::system::clipboard_set(&e.text);
                                app.show_hud(HudKind::Success, "Copied to Clipboard", "");
                            }
                        }
                    });
                });
                ui.add_space(4.0);
                match &selected {
                    Some(e) => {
                        egui::ScrollArea::vertical().id_salt("current").show(ui, |ui| {
                            let mut t = e.text.clone();
                            ui.add(
                                egui::TextEdit::multiline(&mut t)
                                    .desired_width(f32::INFINITY)
                                    .frame(false)
                                    .font(FontId::proportional(14.0)),
                            );
                        });
                    }
                    None => {
                        ui.add_space(18.0);
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new("⌖").size(34.0).color(Color32::from_gray(0xB8)));
                            ui.label(RichText::new("No text captured yet").size(15.0).color(MUTED));
                            let hint = match sc(app, Action::CaptureText) {
                                Some(s) => format!("Press {s} or click Capture Text, then drag over any text on screen."),
                                None => "Click Capture Text, then drag over any text on screen.".to_string(),
                            };
                            ui.label(RichText::new(hint).size(12.0).color(MUTED));
                        });
                    }
                }
            });
            ui.add_space(10.0);
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.set_height(ui.available_height().max(80.0) - 24.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("History").size(14.0).strong());
                    ui.label(RichText::new(format!("{}", app.history.len())).size(11.0).color(MUTED));
                    if app.s.additive && !app.additive_buffer.is_empty() {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.small_button("Clear Additive").clicked() {
                                app.clear_additive();
                            }
                        });
                    }
                });
                ui.add_space(2.0);
                if app.history.is_empty() {
                    ui.label(RichText::new("Your captures will be listed here.").size(12.0).color(MUTED));
                }
                let mut clicked = None;
                egui::ScrollArea::vertical().id_salt("history").show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for (i, e) in app.history.iter().enumerate() {
                        let first: String = e.text.lines().next().unwrap_or("").chars().take(70).collect();
                        let sel = app.selected == Some(i);
                        let w = ui.available_width();
                        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 26.0), Sense::click());
                        let p = ui.painter();
                        if sel {
                            p.rect_filled(rect, Rounding::same(5.0), Color32::from_rgb(0xDC, 0xE9, 0xFB));
                        } else if resp.hovered() {
                            p.rect_filled(rect, Rounding::same(5.0), Color32::from_gray(0xF2));
                        }
                        p.text(rect.left_center() + Vec2::new(8.0, 0.0), Align2::LEFT_CENTER, &first, FontId::proportional(13.0), TEXT);
                        p.text(rect.right_center() - Vec2::new(8.0, 0.0), Align2::RIGHT_CENTER, &e.kind, FontId::proportional(11.0), MUTED);
                        if resp.clicked() {
                            clicked = Some(i);
                        }
                    }
                });
                if let Some(i) = clicked {
                    app.selected = Some(i);
                }
                if !app.status_note.is_empty() {
                    ui.label(RichText::new(&app.status_note).size(11.0).color(Color32::from_rgb(0xC0, 0x39, 0x2B)));
                }
            });
        });
}

fn chip(ui: &mut egui::Ui, label: &str, on: bool) {
    let (fill, fg) = if on {
        (Color32::from_rgb(0xDC, 0xE9, 0xFB), ACCENT)
    } else {
        (Color32::from_gray(0xE0), MUTED)
    };
    egui::Frame::none()
        .fill(fill)
        .rounding(Rounding::same(9.0))
        .inner_margin(egui::Margin::symmetric(9.0, 2.0))
        .show(ui, |ui| {
            ui.label(RichText::new(format!("{label}: {}", if on { "ON" } else { "OFF" })).size(11.0).color(fg));
        });
}

/// Floating message over the screen (its own small, click-through window), so it shows
/// even while the main window is hidden. A coloured icon on the left says what happened
/// before you read anything: green = copied, orange = nothing found, red = failed,
/// grey = canceled, blue/grey switch = mode ON/OFF, purple = cleared.
pub fn hud(app: &mut App, ctx: &egui::Context) {
    let Some(h) = &app.hud else { return };
    let real_age = h.started.elapsed().as_secs_f32();
    // A loading HUD stays until the result replaces it.
    let age = if h.kind == HudKind::Working { real_age.min(1.0) } else { real_age };
    if age > HUD_SECONDS {
        app.hud = None;
        return;
    }
    let (kind, title, subtitle, morph) = (h.kind, h.title.clone(), h.subtitle.clone(), h.morph);
    let spin = real_age * std::f32::consts::TAU * 1.1; // spinner angle
    let dark = match std::env::var("SNIPREADER_HUD_THEME").as_deref() {
        Ok("dark") => true,
        Ok("light") => false,
        _ => ctx.system_theme() != Some(egui::Theme::Light),
    };

    // Motion: slide up + fade in, icon pops with a small overshoot, fade out at the end.
    // A result that replaces the loading HUD keeps the card in place.
    let t_in = if morph { 1.0 } else { (age / 0.22).min(1.0) };
    let ease = 1.0 - (1.0 - t_in).powi(3);
    let alpha = ease * if age > HUD_SECONDS - 0.3 { (HUD_SECONDS - age) / 0.3 } else { 1.0 };
    let pop = if kind == HudKind::Working { 1.0 } else { back_out((age / 0.38).min(1.0)) };
    let dy = (1.0 - ease) * 10.0;

    // Size from the text.
    let title_font = FontId::proportional(15.0);
    let sub_font = FontId::proportional(12.5);
    let text_w = ctx.fonts(|f| {
        let t = f.layout_no_wrap(title.clone(), title_font.clone(), Color32::WHITE).size().x;
        let s = f.layout_no_wrap(subtitle.clone(), sub_font.clone(), Color32::WHITE).size().x;
        t.max(s)
    });
    const MARGIN: f32 = 28.0; // room for the shadow inside the transparent window
    const ICON: f32 = 32.0;
    let card = Vec2::new(
        (14.0 + ICON + 12.0 + text_w + 22.0).clamp(HUD_WIDTH, 360.0),
        if subtitle.is_empty() { 52.0 } else { 60.0 },
    );
    let size = card + Vec2::splat(MARGIN * 2.0);
    let pos = egui::pos2((app.monitor.x - size.x) / 2.0, app.monitor.y * 0.72 - MARGIN);

    ctx.show_viewport_immediate(
        egui::ViewportId::from_hash_of("hud"),
        egui::ViewportBuilder::default()
            .with_title("SnipReader HUD")
            .with_inner_size(size)
            .with_position(pos)
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_resizable(false)
            .with_active(false)
            .with_taskbar(false)
            .with_mouse_passthrough(true),
        |ctx, _| {
            egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
                let p = ui.painter();
                let full = ui.max_rect();
                let rect = egui::Rect::from_min_size(full.min + Vec2::new(MARGIN, MARGIN + dy), card);
                let a = |c: Color32| c.gamma_multiply(alpha);
                let (bg, border, fg, muted, shadow) = if dark {
                    (rgba(44, 44, 46, 246), rgba(255, 255, 255, 30), rgba(245, 245, 247, 255), rgba(160, 160, 166, 255), rgba(0, 0, 0, 90))
                } else {
                    (rgba(255, 255, 255, 248), rgba(0, 0, 0, 18), rgba(29, 29, 31, 255), rgba(110, 110, 115, 255), rgba(0, 0, 0, 45))
                };
                let round = Rounding::same(20.0);
                p.add(egui::epaint::Shadow { offset: Vec2::new(0.0, 8.0), blur: 24.0, spread: 0.0, color: a(shadow) }.as_shape(rect, round));
                p.rect(rect, round, a(bg), Stroke::new(1.0, a(border)));

                // Icon: halo + coloured disc + white glyph.
                let accent = kind_color(kind, dark);
                let c = egui::pos2(rect.left() + 14.0 + ICON / 2.0, rect.center().y);
                let r = ICON / 2.0 * pop;
                p.circle_filled(c, r + 4.0, a(accent.gamma_multiply(0.18)));
                p.circle_filled(c, r, a(accent));
                glyph(p, kind, c, r, a(Color32::WHITE), a(accent), spin);

                // Text: semibold title (drawn twice, half a point apart), lighter subtitle.
                let x = rect.left() + 14.0 + ICON + 12.0;
                let sub_color = if matches!(kind, HudKind::Toggle(true)) { accent } else { muted };
                if subtitle.is_empty() {
                    for dx in [0.0, 0.45] {
                        p.text(egui::pos2(x + dx, rect.center().y), Align2::LEFT_CENTER, &title, title_font.clone(), a(fg));
                    }
                } else {
                    for dx in [0.0, 0.45] {
                        p.text(egui::pos2(x + dx, rect.center().y - 9.0), Align2::LEFT_CENTER, &title, title_font.clone(), a(fg));
                    }
                    p.text(egui::pos2(x, rect.center().y + 10.0), Align2::LEFT_CENTER, &subtitle, sub_font.clone(), a(sub_color));
                }
            });
            ctx.request_repaint();
        },
    );
    // Only our drawn shadow, and never part of a screenshot.
    crate::system::prepare_hud_window("SnipReader HUD");
    ctx.request_repaint();
}

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// Apple system colours (light / dark variants).
fn kind_color(kind: HudKind, dark: bool) -> Color32 {
    let (l, d) = match kind {
        HudKind::Success | HudKind::Added => ((52, 199, 89), (48, 209, 88)),
        HudKind::Empty => ((255, 149, 0), (255, 159, 10)),
        HudKind::Error => ((255, 59, 48), (255, 69, 58)),
        HudKind::Canceled | HudKind::Toggle(false) => ((142, 142, 147), (110, 110, 115)),
        HudKind::Toggle(true) | HudKind::Working => ((0, 122, 255), (10, 132, 255)),
        HudKind::Cleared => ((175, 82, 222), (191, 90, 242)),
    };
    let (r, g, b) = if dark { d } else { l };
    Color32::from_rgb(r, g, b)
}

/// Overshooting ease-out (goes slightly past 1, then settles).
fn back_out(t: f32) -> f32 {
    let (c1, c3) = (1.70158, 2.70158);
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

/// White glyph inside the icon disc of radius `r` centred at `c`.
fn glyph(p: &egui::Painter, kind: HudKind, c: egui::Pos2, r: f32, white: Color32, accent: Color32, spin: f32) {
    let w = (r * 0.17).max(1.5);
    let at = |x: f32, y: f32| c + Vec2::new(x * r, y * r);
    // Line with round ends.
    let seg = |a: egui::Pos2, b: egui::Pos2| {
        p.line_segment([a, b], Stroke::new(w, white));
        p.circle_filled(a, w / 2.0, white);
        p.circle_filled(b, w / 2.0, white);
    };
    match kind {
        HudKind::Success => {
            seg(at(-0.40, 0.02), at(-0.12, 0.30));
            seg(at(-0.12, 0.30), at(0.42, -0.28));
        }
        HudKind::Added => {
            seg(at(-0.42, 0.0), at(0.42, 0.0));
            seg(at(0.0, -0.42), at(0.0, 0.42));
        }
        HudKind::Empty => {
            seg(at(0.0, -0.46), at(0.0, 0.10));
            p.circle_filled(at(0.0, 0.40), w * 0.62, white);
        }
        HudKind::Error | HudKind::Canceled => {
            seg(at(-0.32, -0.32), at(0.32, 0.32));
            seg(at(-0.32, 0.32), at(0.32, -0.32));
        }
        HudKind::Toggle(on) => {
            let track = egui::Rect::from_center_size(c, Vec2::new(r * 1.2, r * 0.68));
            let knob_r = r * 0.24;
            let knob_x = if on { track.right() - r * 0.34 } else { track.left() + r * 0.34 };
            if on {
                p.rect_filled(track, Rounding::same(track.height() / 2.0), white);
                p.circle_filled(egui::pos2(knob_x, c.y), knob_r, accent);
            } else {
                p.rect_stroke(track, Rounding::same(track.height() / 2.0), Stroke::new(w * 0.8, white));
                p.circle_filled(egui::pos2(knob_x, c.y), knob_r, white);
            }
        }
        HudKind::Working => {
            // Faint full ring + a bright 100° arc that turns.
            let ring = r * 0.44;
            p.circle_stroke(c, ring, Stroke::new(w, white.gamma_multiply(0.35)));
            let n = 24;
            let pts: Vec<egui::Pos2> = (0..=n)
                .map(|i| {
                    let t = spin + (i as f32 / n as f32) * 1.75;
                    c + Vec2::new(t.cos(), t.sin()) * ring
                })
                .collect();
            p.circle_filled(pts[0], w / 2.0, white);
            p.circle_filled(pts[n], w / 2.0, white);
            p.add(egui::Shape::line(pts, Stroke::new(w, white)));
        }
        HudKind::Cleared => {
            seg(at(-0.44, -0.30), at(0.44, -0.30)); // lid
            seg(at(-0.14, -0.44), at(0.14, -0.44)); // handle
            seg(at(-0.32, -0.18), at(-0.24, 0.44)); // body
            seg(at(-0.24, 0.44), at(0.24, 0.44));
            seg(at(0.24, 0.44), at(0.32, -0.18));
        }
    }
}
