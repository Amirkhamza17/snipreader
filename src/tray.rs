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

fn icon() -> Icon {
    // Small template-style glyph: selection corners with two text lines.
    let n = 36usize;
    let mut px = vec![0u8; n * n * 4];
    let mut put = |x: usize, y: usize| {
        if x < n && y < n {
            let i = (y * n + x) * 4;
            px[i..i + 4].copy_from_slice(&[0, 0, 0, 255]);
        }
    };
    let (a, b, l, t) = (4usize, 31usize, 9usize, 3usize);
    for k in 0..l {
        for w in 0..t {
            for (cx, cy, dx, dy) in [(a, a, 1i32, 1i32), (b, a, -1, 1), (a, b, 1, -1), (b, b, -1, -1)] {
                let x1 = (cx as i32 + dx * k as i32) as usize;
                let y1 = (cy as i32 + dy * w as i32) as usize;
                put(x1, y1);
                let x2 = (cx as i32 + dx * w as i32) as usize;
                let y2 = (cy as i32 + dy * k as i32) as usize;
                put(x2, y2);
            }
        }
    }
    for y in 14..17 {
        for x in 11..26 {
            put(x, y);
        }
    }
    for y in 20..23 {
        for x in 11..21 {
            put(x, y);
        }
    }
    Icon::from_rgba(px, n as u32, n as u32).expect("icon")
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
