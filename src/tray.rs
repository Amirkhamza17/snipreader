//! The menu bar icon and its menu.

use crate::app::Settings;
use crate::keys::{code_for, Action};
use tray_icon::menu::accelerator::{Accelerator, Modifiers};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

pub enum Cmd {
    Run(Action),
    ReadImage,
    KeepBreaks,
    Additive,
    ClearHistory,
    Speech,
    Prefs,
    ShowWindow,
    Quit,
}

pub struct Tray {
    _icon: TrayIcon,
    capture: MenuItem,
    code: MenuItem,
    keep: CheckMenuItem,
    additive: CheckMenuItem,
    speech: CheckMenuItem,
    stop: MenuItem,
}

fn accel(s: &Settings, a: Action) -> Option<Accelerator> {
    let sc = s.shortcuts.get(&a)?;
    let code = code_for(&sc.key)?;
    let mut m = Modifiers::empty();
    if sc.ctrl {
        m |= Modifiers::CONTROL;
    }
    if sc.alt {
        m |= Modifiers::ALT;
    }
    if sc.shift {
        m |= Modifiers::SHIFT;
    }
    if sc.cmd {
        m |= Modifiers::SUPER;
    }
    Some(Accelerator::new(Some(m), code))
}

/// The menu-bar glyph (a template image: black + alpha, macOS tints it): selection corners
/// with rounded bends and two text lines, all strokes with round ends, anti-aliased.
fn icon() -> Icon {
    Icon::from_rgba(icon_pixels(), ICON_SIZE as u32, ICON_SIZE as u32).expect("icon")
}

const ICON_SIZE: usize = 36;

fn icon_pixels() -> Vec<u8> {
    const N: usize = ICON_SIZE;
    const W: f32 = 3.0; // stroke width in pixels
    // Centre lines of the strokes, in pixels.
    let mut paths: Vec<Vec<(f32, f32)>> = Vec::new();
    let (a, b, arm, bend) = (5.5_f32, 30.5_f32, 8.0_f32, 4.0_f32);
    for (cx, cy, dx, dy) in [(a, a, 1.0, 1.0), (b, a, -1.0, 1.0), (a, b, 1.0, -1.0), (b, b, -1.0, -1.0)] {
        let mut pts = vec![(cx + dx * arm, cy)];
        let (ox, oy) = (cx + dx * bend, cy + dy * bend); // centre of the bend
        for i in 0..=8 {
            let t = i as f32 / 8.0 * std::f32::consts::FRAC_PI_2;
            pts.push((ox - dx * bend * t.sin(), oy - dy * bend * t.cos()));
        }
        pts.push((cx, cy + dy * arm));
        paths.push(pts);
    }
    paths.push(vec![(12.5, 15.5), (24.0, 15.5)]);
    paths.push(vec![(12.5, 21.5), (19.5, 21.5)]);

    // Distance from a point to a segment.
    let dist = |px: f32, py: f32, (x1, y1): (f32, f32), (x2, y2): (f32, f32)| {
        let (vx, vy) = (x2 - x1, y2 - y1);
        let len2 = vx * vx + vy * vy;
        let t = if len2 > 0.0 { (((px - x1) * vx + (py - y1) * vy) / len2).clamp(0.0, 1.0) } else { 0.0 };
        let (qx, qy) = (x1 + t * vx - px, y1 + t * vy - py);
        (qx * qx + qy * qy).sqrt()
    };
    let mut px = vec![0u8; N * N * 4];
    for y in 0..N {
        for x in 0..N {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let d = paths
                .iter()
                .flat_map(|p| p.windows(2).map(move |s| (s[0], s[1])))
                .map(|(s0, s1)| dist(fx, fy, s0, s1))
                .fold(f32::MAX, f32::min);
            let alpha = (W / 2.0 + 0.5 - d).clamp(0.0, 1.0);
            px[(y * N + x) * 4 + 3] = (alpha * 255.0).round() as u8;
        }
    }
    px
}

#[cfg(test)]
mod tests {
    /// The glyph is drawn, symmetric left/right, and anti-aliased (has partial pixels).
    /// Set SNIPREADER_TRAY_PNG=<file> to also save it for a look.
    #[test]
    fn tray_glyph() {
        let px = super::icon_pixels();
        let n = super::ICON_SIZE;
        let alpha = |x: usize, y: usize| px[(y * n + x) * 4 + 3];
        assert!(px.chunks(4).filter(|p| p[3] == 255).count() > 100);
        assert!(px.chunks(4).any(|p| p[3] > 0 && p[3] < 255));
        assert_eq!(alpha(5, 10), alpha(30, 10)); // corner arms mirror
        if let Ok(path) = std::env::var("SNIPREADER_TRAY_PNG") {
            image::RgbaImage::from_raw(n as u32, n as u32, px.clone()).unwrap().save(path).unwrap();
        }
    }
}

impl Tray {
    pub fn new() -> Option<Self> {
        let s = Settings::default();
        let menu = Menu::new();
        let capture = MenuItem::with_id("capture", "Capture Text", true, accel(&s, Action::CaptureText));
        let code = MenuItem::with_id("code", "Read QR/Bar Code", true, None);
        let image = MenuItem::with_id("image", "Read Text From Image…", true, None);
        let keep = CheckMenuItem::with_id("keep", "Keep Line Breaks", true, s.keep_line_breaks, None);
        let additive = CheckMenuItem::with_id("additive", "Additive Clipboard", true, s.additive, None);
        let clear = MenuItem::with_id("clear", "Clear Clipboard History", true, None);
        let speech = CheckMenuItem::with_id("speech", "Text to Speech", true, false, None);
        let stop = MenuItem::with_id("stop", "Stop Speaking", false, None);
        let window = MenuItem::with_id("window", "Show Window", true, None);
        let prefs = MenuItem::with_id("prefs", "Preferences...", true, None);
        let quit = MenuItem::with_id("quit", "Quit SnipReader", true, None);
        let sep = || PredefinedMenuItem::separator();
        menu.append_items(&[
            &capture, &code, &image, &sep(), &keep, &additive, &clear, &sep(), &speech, &stop, &sep(), &window, &prefs, &quit,
        ])
        .ok()?;
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_icon(icon())
            .with_icon_as_template(true)
            .with_tooltip("SnipReader")
            .build()
            .ok()?;
        Some(Tray { _icon: tray, capture, code, keep, additive, speech, stop })
    }

    pub fn sync(&self, s: &Settings, speaking: bool) {
        if self.keep.is_checked() != s.keep_line_breaks {
            self.keep.set_checked(s.keep_line_breaks);
        }
        if self.additive.is_checked() != s.additive {
            self.additive.set_checked(s.additive);
        }
        if self.speech.is_checked() != s.speak_after_capture {
            self.speech.set_checked(s.speak_after_capture);
        }
        if self.stop.is_enabled() != speaking {
            self.stop.set_enabled(speaking);
        }
        let _ = self.capture.set_accelerator(accel(s, Action::CaptureText));
        let _ = self.code.set_accelerator(accel(s, Action::ReadCode));
    }

    /// Which command a menu click means (events arrive through the handler set in App::new).
    pub fn command_for(e: &MenuEvent) -> Option<Cmd> {
        let cmd = match e.id().0.as_str() {
            "capture" => Cmd::Run(Action::CaptureText),
            "code" => Cmd::Run(Action::ReadCode),
            "image" => Cmd::ReadImage,
            "keep" => Cmd::KeepBreaks,
            "additive" => Cmd::Additive,
            "clear" => Cmd::ClearHistory,
            "speech" => Cmd::Speech,
            "stop" => Cmd::Run(Action::StopSpeaking),
            "window" => Cmd::ShowWindow,
            "prefs" => Cmd::Prefs,
            "quit" => Cmd::Quit,
            _ => return None,
        };
        Some(cmd)
    }
}
