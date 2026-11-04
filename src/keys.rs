//! Keyboard shortcuts: what can be bound, how they are shown and registered.

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Action {
    CaptureText,
    CaptureLast,
    CaptureWithBreaks,
    CaptureWithoutBreaks,
    CaptureWithSpeech,
    ReadCode,
    StopSpeaking,
    ToggleAdditive,
    ClearAdditive,
    // "Capture Text mode" keys: toggle settings while a capture is running.
    ModeKeepBreaks,
    ModeAdditive,
    ModeSpeech,
}

impl Action {
    pub const GLOBAL: [Action; 9] = [
        Action::CaptureText,
        Action::CaptureLast,
        Action::CaptureWithBreaks,
        Action::CaptureWithoutBreaks,
        Action::CaptureWithSpeech,
        Action::ReadCode,
        Action::StopSpeaking,
        Action::ToggleAdditive,
        Action::ClearAdditive,
    ];
    pub const MODE: [Action; 3] = [Action::ModeKeepBreaks, Action::ModeAdditive, Action::ModeSpeech];

    pub fn label(self) -> &'static str {
        match self {
            Action::CaptureText => "Capture Text",
            Action::CaptureLast => "Capture Last Selection",
            Action::CaptureWithBreaks => "Capture With Line Breaks",
            Action::CaptureWithoutBreaks => "Capture Without Line Breaks",
            Action::CaptureWithSpeech => "Capture With Text to Speech",
            Action::ReadCode => "Read QR/Bar Code",
            Action::StopSpeaking => "Stop Speaking",
            Action::ToggleAdditive => "Toggle Additive Clipboard",
            Action::ClearAdditive => "Clear Additive Clipboard History",
            Action::ModeKeepBreaks => "Keep Line Breaks",
            Action::ModeAdditive => "Additive Clipboard",
            Action::ModeSpeech => "Text to Speech",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Shortcut {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub cmd: bool,
    pub key: String,
}

impl Shortcut {
    /// Default shortcut for Capture Text: ⇧⌘2.
    pub fn default_capture() -> Self {
        Shortcut { ctrl: false, alt: false, shift: true, cmd: true, key: "2".into() }
    }

    pub fn display(&self) -> String {
        let mut s = String::new();
        if self.ctrl {
            s.push('⌃');
        }
        if self.alt {
            s.push('⌥');
        }
        if self.shift {
            s.push('⇧');
        }
        if self.cmd {
            s.push('⌘');
        }
        s.push_str(&self.key);
        s
    }

    pub fn from_egui(key: eframe::egui::Key, m: eframe::egui::Modifiers) -> Option<Self> {
        let name = key.name().to_string();
        code_for(&name)?;
        if !(m.mac_cmd || m.command || m.ctrl || m.alt) {
            return None;
        }
        Some(Shortcut {
            ctrl: m.ctrl, // the physical Control key on macOS, independent of Command
            alt: m.alt,
            shift: m.shift,
            cmd: m.mac_cmd || m.command,
            key: name.to_uppercase(),
        })
    }

    pub fn matches(&self, key: eframe::egui::Key, m: eframe::egui::Modifiers) -> bool {
        key.name().eq_ignore_ascii_case(&self.key)
            && self.shift == m.shift
            && self.alt == m.alt
            && self.ctrl == m.ctrl
            && self.cmd == (m.mac_cmd || m.command)
    }

    pub fn hotkey(&self) -> Option<HotKey> {
        let code = code_for(&self.key)?;
        let mut mods = Modifiers::empty();
        if self.ctrl {
            mods |= Modifiers::CONTROL;
        }
        if self.alt {
            mods |= Modifiers::ALT;
        }
        if self.shift {
            mods |= Modifiers::SHIFT;
        }
        if self.cmd {
            mods |= Modifiers::SUPER;
        }
        Some(HotKey::new(Some(mods), code))
    }
}

pub fn code_for(name: &str) -> Option<Code> {
    let up = name.to_uppercase();
    let c = match up.as_str() {
        "0" => Code::Digit0,
        "1" => Code::Digit1,
        "2" => Code::Digit2,
        "3" => Code::Digit3,
        "4" => Code::Digit4,
        "5" => Code::Digit5,
        "6" => Code::Digit6,
        "7" => Code::Digit7,
        "8" => Code::Digit8,
        "9" => Code::Digit9,
        "A" => Code::KeyA,
        "B" => Code::KeyB,
        "C" => Code::KeyC,
        "D" => Code::KeyD,
        "E" => Code::KeyE,
        "F" => Code::KeyF,
        "G" => Code::KeyG,
        "H" => Code::KeyH,
        "I" => Code::KeyI,
        "J" => Code::KeyJ,
        "K" => Code::KeyK,
        "L" => Code::KeyL,
        "M" => Code::KeyM,
        "N" => Code::KeyN,
        "O" => Code::KeyO,
        "P" => Code::KeyP,
        "Q" => Code::KeyQ,
        "R" => Code::KeyR,
        "S" => Code::KeyS,
        "T" => Code::KeyT,
        "U" => Code::KeyU,
        "V" => Code::KeyV,
        "W" => Code::KeyW,
        "X" => Code::KeyX,
        "Y" => Code::KeyY,
        "Z" => Code::KeyZ,
        "F1" => Code::F1,
        "F2" => Code::F2,
        "F3" => Code::F3,
        "F4" => Code::F4,
        "F5" => Code::F5,
        "F6" => Code::F6,
        "F7" => Code::F7,
        "F8" => Code::F8,
        "F9" => Code::F9,
        "F10" => Code::F10,
        "F11" => Code::F11,
        "F12" => Code::F12,
        "SPACE" => Code::Space,
        _ => return None,
    };
    Some(c)
}
