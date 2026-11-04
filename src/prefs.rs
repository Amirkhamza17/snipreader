//! The Preferences window: General and Shortcuts panes.
//! Layout: a 510 × 622 pane, labels right-aligned in a fixed column.

use crate::app::*;
use crate::keys::{Action, Shortcut};
use eframe::egui::{self, Align, Color32, Layout, RichText, Rounding, Sense, Stroke, Vec2};

const PANE_W: f32 = 510.0;
const PANE_H: f32 = 622.0;
const LABEL_RIGHT: f32 = 147.0;
const GRID_LEFT: f32 = 30.0;
const CONTROL_GAP: f32 = 8.0; // controls start at x 155
const SC_LABEL_W: f32 = 230.0; // Shortcuts pane label column

pub fn show(app: &mut App, ctx: &egui::Context) {
    let title = if app.prefs_tab == 0 { "General" } else { "Shortcuts" };
    ctx.show_viewport_immediate(
        egui::ViewportId::from_hash_of("preferences"),
        egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size([PANE_W, PANE_H + 64.0])
            .with_resizable(false),
        |ctx, _class| {
            draw(app, ctx);
            if ctx.input(|i| i.viewport().close_requested()) {
                app.prefs_open = false;
                app.recording = None;
                app.register_hotkeys();
            }
        },
    );
    if app.custom_words_open {
        custom_words(app, ctx);
    }
}

/// The toolbar and the selected pane (also used to preview the pane in the main window).
pub fn draw(app: &mut App, ctx: &egui::Context) {
            egui::TopBottomPanel::top("prefs_toolbar")
                .exact_height(60.0)
                .frame(egui::Frame::none().fill(Color32::from_rgb(0xF4, 0xF4, 0xF5)).stroke(Stroke::new(1.0, BORDER)))
                .show(ctx, |ui| {
                    ui.horizontal_centered(|ui| {
                        let total = 2.0 * 76.0;
                        ui.add_space((ui.available_width() - total) / 2.0);
                        for (i, (icon, name)) in [("⚙", "General"), ("⌘", "Shortcuts")].iter().enumerate() {
                            if toolbar_button(ui, icon, name, app.prefs_tab == i) {
                                app.prefs_tab = i;
                                app.recording = None;
                                app.register_hotkeys();
                            }
                        }
                    });
                });
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(Color32::from_rgb(0xEC, 0xEC, 0xEE)).inner_margin(egui::Margin { left: GRID_LEFT, right: 30.0, top: 18.0, bottom: 12.0 }))
                .show(ctx, |ui| {
                    if app.prefs_tab == 0 {
                        general(app, ui);
                    } else {
                        shortcuts(app, ui);
                    }
                });
}

fn toolbar_button(ui: &mut egui::Ui, icon: &str, name: &str, selected: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(76.0, 50.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, Rounding::same(6.0), Color32::from_gray(0xDE));
    } else if resp.hovered() {
        p.rect_filled(rect, Rounding::same(6.0), Color32::from_gray(0xE8));
    }
    let c = if selected { ACCENT } else { MUTED };
    p.text(rect.center_top() + Vec2::new(0.0, 16.0), egui::Align2::CENTER_CENTER, icon, egui::FontId::proportional(20.0), c);
    p.text(rect.center_bottom() - Vec2::new(0.0, 11.0), egui::Align2::CENTER_CENTER, name, egui::FontId::proportional(11.5), if selected { ACCENT } else { TEXT });
    resp.clicked()
}

/// A row: right-aligned label ending at x 147, then the control from x 155.
fn row(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    row_w(ui, LABEL_RIGHT - GRID_LEFT, label, add)
}

/// Fixed-width label column, text right-aligned to its edge.
fn row_w(ui: &mut egui::Ui, label_w: f32, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal_top(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(label_w, 20.0), Sense::hover());
        if !label.is_empty() {
            ui.painter().text(
                rect.right_center(),
                egui::Align2::RIGHT_CENTER,
                label,
                egui::FontId::proportional(13.0),
                TEXT,
            );
        }
        ui.add_space(CONTROL_GAP - ui.spacing().item_spacing.x);
        ui.vertical(|ui| add(ui));
    });
}

fn note(ui: &mut egui::Ui, text: &str) {
    ui.scope(|ui| {
        ui.set_max_width(299.0);
        ui.with_layout(Layout::top_down(Align::Min), |ui| {
            ui.add(egui::Label::new(RichText::new(text).size(11.0).color(MUTED)).wrap());
        });
    });
}

fn section_break(ui: &mut egui::Ui) {
    ui.add_space(8.0);
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 5.0), Sense::hover());
    ui.painter().line_segment([rect.left_center(), rect.right_center()], Stroke::new(1.0, Color32::from_gray(0xD2)));
    ui.add_space(8.0);
}

fn general(app: &mut App, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing.y = 6.0;
    row(ui, "System:", |ui| {
        if ui.checkbox(&mut app.s.launch_at_login, "Launch at Login").changed() {
            crate::system::set_launch_at_login(app.s.launch_at_login);
        }
        ui.checkbox(&mut app.s.show_in_menu_bar, "Show in Menu Bar");
        note(ui, "Without the menu bar icon, closing the window quits SnipReader.");
        ui.checkbox(&mut app.s.disable_sound, "Disable Sound Effects");
        ui.checkbox(&mut app.s.disable_success, "Disable Success Notification");
        ui.checkbox(&mut app.s.open_links, "Automatically Open Links");
    });
    section_break(ui);
    row(ui, "Recognition Language:", |ui| {
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("lang")
                .width(150.0)
                .selected_text(app.s.language.clone())
                .show_ui(ui, |ui| {
                    for (name, _) in LANGUAGES {
                        ui.selectable_value(&mut app.s.language, name.to_string(), name);
                    }
                });
            if ui.button("Custom Words…").clicked() {
                app.custom_words_open = true;
            }
        });
        note(ui, "Primary text recognition language.");
        ui.checkbox(&mut app.s.auto_detect_language, "Automatically Detect Language");
    });
    section_break(ui);
    row(ui, "Additive Clipboard:", |ui| {
        ui.checkbox(&mut app.s.clear_additive_auto, "Clear Automatically");
        note(ui, "Additive Clipboard will get cleared on each ⌘V press. May need Input Monitoring permission.");
    });
    section_break(ui);
    row(ui, "Text to Speech Rate:", |ui| {
        let mut rate = app.s.tts_rate as f32;
        let w = 200.0;
        ui.spacing_mut().slider_width = w;
        let resp = ui.add(egui::Slider::new(&mut rate, TTS_RATE_MIN..=TTS_RATE_MAX).show_value(false));
        // Tick marks under the slider.
        let r = resp.rect;
        let p = ui.painter();
        for i in 0..TTS_TICKS {
            let x = r.left() + 6.0 + (r.width() - 12.0) * i as f32 / (TTS_TICKS - 1) as f32;
            p.line_segment([egui::pos2(x, r.bottom() + 1.0), egui::pos2(x, r.bottom() + 5.0)], Stroke::new(1.0, Color32::from_gray(0xA0)));
        }
        ui.add_space(4.0);
        app.s.tts_rate = rate.round() as u32;
        ui.horizontal(|ui| {
            ui.add_space(w / 2.0 - 26.0);
            ui.label(RichText::new(format!("{} WPM", app.s.tts_rate)).size(11.0).color(MUTED));
        });
    });
    section_break(ui);
    row(ui, "About:", |ui| {
        ui.label(format!("SnipReader {}", env!("CARGO_PKG_VERSION")));
        note(ui, "Open source, MIT license. Built with Rust, egui and Apple Vision.");
        if ui.button("Show Welcome Guide…").clicked() {
            app.welcome_page = 0;
            app.welcome_open = true;
        }
    });
}

fn shortcut_field(app: &mut App, ui: &mut egui::Ui, a: Action) {
    let recording = app.recording == Some(a);
    let text = if recording {
        "Type shortcut…".to_string()
    } else {
        app.s.shortcuts.get(&a).map(|s| s.display()).unwrap_or_else(|| "Record Shortcut".into())
    };
    ui.horizontal(|ui| {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(160.0, 22.0), Sense::click());
        let p = ui.painter();
        p.rect_filled(rect, Rounding::same(11.0), Color32::WHITE);
        p.rect_stroke(rect, Rounding::same(11.0), Stroke::new(if recording { 2.0 } else { 1.0 }, if recording { ACCENT } else { BORDER }));
        let has = app.s.shortcuts.contains_key(&a);
        let c = if recording || !has { MUTED } else { TEXT };
        p.text(rect.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(12.5), c);
        if resp.clicked() {
            app.recording = if recording { None } else { Some(a) };
            app.register_hotkeys();
        }
        if has && !recording && ui.small_button("✕").on_hover_text("Remove shortcut").clicked() {
            app.s.shortcuts.remove(&a);
            app.register_hotkeys();
        }
    });
}

fn shortcuts(app: &mut App, ui: &mut egui::Ui) {
    // Capture a key press while recording.
    if let Some(a) = app.recording {
        let ev = ui.ctx().input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Key { key, pressed: true, modifiers, .. } => Some((*key, *modifiers)),
                _ => None,
            })
        });
        if let Some((k, m)) = ev {
            if k == egui::Key::Escape {
                app.recording = None;
            } else if k == egui::Key::Backspace || k == egui::Key::Delete {
                app.s.shortcuts.remove(&a);
                app.recording = None;
            } else if let Some(s) = Shortcut::from_egui(k, m) {
                // One shortcut per action: drop it from anything else using it.
                app.s.shortcuts.retain(|_, v| *v != s);
                app.s.shortcuts.insert(a, s);
                app.recording = None;
            }
            if app.recording.is_none() {
                app.register_hotkeys();
            }
        }
    }
    ui.spacing_mut().item_spacing.y = 7.0;
    row_w(ui, SC_LABEL_W, "", |ui| {
        ui.label(RichText::new("Global").strong());
    });
    for a in Action::GLOBAL {
        row_w(ui, SC_LABEL_W, &format!("{}:", a.label()), |ui| shortcut_field(app, ui, a));
    }
    section_break(ui);
    row_w(ui, SC_LABEL_W, "", |ui| {
        ui.label(RichText::new("Capture Text mode").strong());
        note(ui, "Toggles preferences when Capture Text operation is active (works while this app is in front).");
    });
    for a in Action::MODE {
        row_w(ui, SC_LABEL_W, &format!("{}:", a.label()), |ui| shortcut_field(app, ui, a));
    }
    ui.add_space(6.0);
    row_w(ui, SC_LABEL_W, "", |ui| note(ui, "Click a field, then press the keys. Esc cancels, Delete removes."));
}

fn custom_words(app: &mut App, ctx: &egui::Context) {
    ctx.show_viewport_immediate(
        egui::ViewportId::from_hash_of("custom_words"),
        egui::ViewportBuilder::default().with_title("Custom Words").with_inner_size([420.0, 220.0]).with_resizable(false),
        |ctx, _| {
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(WINDOW_BG).inner_margin(egui::Margin::same(18.0)))
                .show(ctx, |ui| {
                    ui.label(RichText::new("Custom Words").size(15.0).strong());
                    note(ui, "If the text you capture has unusual words or names, add them here so they are read correctly.");
                    ui.add_space(6.0);
                    ui.add(
                        egui::TextEdit::multiline(&mut app.s.custom_words)
                            .hint_text("Type custom words separated by comma...")
                            .desired_rows(4)
                            .desired_width(f32::INFINITY),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Done").clicked() {
                            app.custom_words_open = false;
                        }
                    });
                });
            if ctx.input(|i| i.viewport().close_requested()) {
                app.custom_words_open = false;
            }
        },
    );
}
