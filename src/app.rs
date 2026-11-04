//! The app: settings, the capture flow and the main window.

use crate::keys::{Action, Shortcut};
use crate::{ocr, system, tray};
use eframe::egui::{self, Color32, Rounding, Stroke, Vec2};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};
use tray_icon::menu::MenuEvent;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

// Line joining lives in layout.rs.
pub const ADDITIVE_SEP: &str = "\n\n"; // between captures in Additive Clipboard
pub const HUD_WIDTH: f32 = 180.0; // minimum HUD card width
pub const HUD_SECONDS: f32 = 2.0; // how long a HUD message stays
pub const TTS_RATE_MIN: f32 = 50.0; // speech rate slider, words per minute
pub const TTS_RATE_MAX: f32 = 700.0;
pub const TTS_RATE_DEFAULT: u32 = 180;
pub const TTS_TICKS: usize = 14;

/// Appends a timestamped step of the capture flow to ~/Library/Logs/SnipReader/timing.log
/// (or to SNIPREADER_TRACE), so a slow capture shows which step took the time.
/// The log starts over once it passes 512 KB.
pub fn trace(step: &str) {
    let path = match std::env::var("SNIPREADER_TRACE") {
        Ok(p) => PathBuf::from(p),
        Err(_) => match std::env::var("HOME") {
            Ok(h) => {
                let dir = PathBuf::from(h).join("Library/Logs/SnipReader");
                let _ = std::fs::create_dir_all(&dir);
                dir.join("timing.log")
            }
            Err(_) => return,
        },
    };
    if std::fs::metadata(&path).map(|m| m.len() > 512 * 1024).unwrap_or(false) {
        let _ = std::fs::remove_file(&path);
    }
    let t = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        use std::io::Write;
        let _ = writeln!(f, "{t:.3} {step}");
    }
}

/// v2: Additive Clipboard was found switched on by accident in saved settings; turn it off once.
pub const SETTINGS_VERSION: u32 = 2;

pub const ACCENT: Color32 = Color32::from_rgb(0x0A, 0x64, 0xD8);
pub const WINDOW_BG: Color32 = Color32::from_rgb(0xEC, 0xEC, 0xEE);
pub const CARD_BG: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
pub const BORDER: Color32 = Color32::from_rgb(0xD4, 0xD4, 0xD8);
pub const TEXT: Color32 = Color32::from_rgb(0x1D, 0x1D, 0x1F);
pub const MUTED: Color32 = Color32::from_rgb(0x6E, 0x6E, 0x73);

pub const LANGUAGES: [(&str, &str); 8] = [
    ("English", "en-US"),
    ("French", "fr-FR"),
    ("Italian", "it-IT"),
    ("German", "de-DE"),
    ("Spanish", "es-ES"),
    ("Portuguese", "pt-BR"),
    ("Chinese Simplified", "zh-Hans"),
    ("Chinese Traditional", "zh-Hant"),
];

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub keep_line_breaks: bool,
    pub additive: bool,
    pub speak_after_capture: bool,
    pub launch_at_login: bool,
    /// The welcome guide has been seen (it opens on first launch until then).
    pub onboarded: bool,
    pub show_in_menu_bar: bool,
    pub disable_sound: bool,
    pub disable_success: bool,
    pub open_links: bool,
    pub language: String,
    pub auto_detect_language: bool,
    pub clear_additive_auto: bool,
    pub tts_rate: u32,
    pub custom_words: String,
    pub auto_check_updates: bool,
    pub shortcuts: HashMap<Action, Shortcut>,
    pub last_area: Option<String>,
    /// Saved-settings version; old saves are fixed up once on load.
    #[serde(default)]
    pub version: u32,
}

impl Default for Settings {
    fn default() -> Self {
        let mut shortcuts = HashMap::new();
        shortcuts.insert(Action::CaptureText, Shortcut::default_capture());
        Settings {
            keep_line_breaks: true,
            additive: false,
            speak_after_capture: false,
            launch_at_login: false,
            onboarded: false,
            show_in_menu_bar: true,
            disable_sound: false,
            disable_success: false,
            open_links: false,
            language: "English".into(),
            auto_detect_language: false,
            clear_additive_auto: false,
            tts_rate: TTS_RATE_DEFAULT,
            custom_words: String::new(),
            auto_check_updates: true,
            shortcuts,
            last_area: None,
            version: SETTINGS_VERSION,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Text,
    Code,
}

#[derive(Clone)]
pub struct Job {
    pub mode: Mode,
    pub keep_breaks: bool,
    pub speak: bool,
    pub use_last_area: bool,
    pub file: Option<PathBuf>,
}

pub enum JobResult {
    /// The screen grab is done and recognition has started (shows the loading HUD).
    Working(Mode),
    Text { lines: Vec<crate::layout::Line>, job: Job, area: Option<String> },
    Codes { codes: Vec<String>, area: Option<String> },
    Canceled,
    Failed(String),
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Entry {
    pub text: String,
    pub kind: String,
    pub when: u64,
}

/// What a HUD message means; picks its icon and colour so the result reads at a glance.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum HudKind {
    /// Copied: green check.
    Success,
    /// Appended to the Additive Clipboard: green plus.
    Added,
    /// Nothing found / nothing to repeat: orange "!".
    Empty,
    /// Something broke: red cross.
    Error,
    /// The user backed out: grey cross.
    Canceled,
    /// A mode switched ON (blue switch) or OFF (grey switch).
    Toggle(bool),
    /// Something was emptied: purple trash can.
    Cleared,
    /// Recognition in progress: blue disc with a spinning arc. Stays until the result.
    Working,
}

impl HudKind {
    /// For SNIPREADER_HUD_KIND in the self-test.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "success" => Self::Success,
            "added" => Self::Added,
            "empty" => Self::Empty,
            "error" => Self::Error,
            "canceled" => Self::Canceled,
            "on" => Self::Toggle(true),
            "off" => Self::Toggle(false),
            "cleared" => Self::Cleared,
            "working" => Self::Working,
            _ => return None,
        })
    }
}

pub struct Hud {
    pub kind: HudKind,
    /// Replaces a loading HUD in place: no slide-in, only the icon changes.
    pub morph: bool,
    pub title: String,
    pub subtitle: String,
    pub started: Instant,
}

/// Events from the hotkey and menu-bar handlers; they wake the app even while its window is hidden.
pub enum Wake {
    Key(GlobalHotKeyEvent),
    Menu(MenuEvent),
    /// An image picked in "Read Text From Image…".
    File(PathBuf),
}

pub struct App {
    pub s: Settings,
    pub history: Vec<Entry>,
    pub additive_buffer: String,
    pub last_written: String,
    pub hud: Option<Hud>,
    /// Welcome guide window: open, current page, and whether permission was requested.
    pub welcome_open: bool,
    pub welcome_page: usize,
    pub welcome_asked: bool,
    /// When the app started, and the step of the reopen test.
    started: Instant,
    test_step: u8,
    /// Main window [width, height, x, y] from before it was hidden (and shrunk).
    saved_geometry: Option<[f32; 4]>,
    /// The ⌘V watcher thread, woken when watching starts.
    paste_thread: Option<std::thread::Thread>,
    /// The main window has been turned back into a normal opaque window (title bar fix).
    main_opaque: bool,
    /// Held while a capture runs, so macOS does not nap the app (see system::Activity).
    activity: Option<system::Activity>,
    selftest_shown: Option<Instant>,
    test_trigger: Arc<AtomicBool>,
    pub busy: bool,
    tx: Sender<JobResult>,
    rx: Receiver<JobResult>,
    speaking: Option<std::process::Child>,
    pub prefs_open: bool,
    pub prefs_tab: usize,
    pub custom_words_open: bool,
    pub recording: Option<Action>,
    hotkeys: Option<GlobalHotKeyManager>,
    hotkey_ids: HashMap<u32, Action>,
    registered: Vec<global_hotkey::hotkey::HotKey>,
    tray: Option<tray::Tray>,
    tray_tried: bool,
    wake_rx: Receiver<Wake>,
    wake_tx: Sender<Wake>,
    ctx: egui::Context,
    pub window_visible: bool,
    quitting: bool,
    pub monitor: Vec2,
    paste_watch: Arc<AtomicBool>,
    pasted: Arc<AtomicBool>,
    screenshot: Option<PathBuf>,
    frame: u64,
    pub selected: Option<usize>,
    pub status_note: String,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>) -> Self {
        crate::install_fonts(&cc.egui_ctx);
        style(&cc.egui_ctx);
        let (mut s, mut history) = (Settings::default(), Vec::new());
        if screenshot.is_none() {
            if let Some(st) = cc.storage {
                s = eframe::get_value(st, "settings").unwrap_or_default();
                history = eframe::get_value(st, "history").unwrap_or_default();
                if s.version < SETTINGS_VERSION {
                    s.additive = false;
                    s.version = SETTINGS_VERSION;
                }
            }
        }
        let (tx, rx) = channel();
        let (wake_tx, wake_rx) = channel::<Wake>();
        if screenshot.is_none() {
            let (t, c) = (wake_tx.clone(), cc.egui_ctx.clone());
            GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
                let _ = t.send(Wake::Key(e));
                c.request_repaint();
            }));
            let (t, c) = (wake_tx.clone(), cc.egui_ctx.clone());
            MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
                let _ = t.send(Wake::Menu(e));
                c.request_repaint();
            }));
        }
        let mut app = App {
            s,
            history,
            additive_buffer: String::new(),
            last_written: String::new(),
            hud: None,
            welcome_open: false,
            welcome_page: 0,
            welcome_asked: false,
            started: Instant::now(),
            test_step: 0,
            saved_geometry: crate::load_window_geometry(),
            paste_thread: None,
            main_opaque: false,
            activity: None,
            selftest_shown: None,
            test_trigger: Arc::new(AtomicBool::new(false)),
            busy: false,
            tx,
            rx,
            speaking: None,
            prefs_open: false,
            prefs_tab: 0,
            custom_words_open: false,
            recording: None,
            hotkeys: None,
            hotkey_ids: HashMap::new(),
            registered: Vec::new(),
            tray: None,
            tray_tried: false,
            wake_rx,
            wake_tx,
            ctx: cc.egui_ctx.clone(),
            window_visible: true,
            quitting: false,
            monitor: Vec2::new(1440.0, 900.0),
            paste_watch: Arc::new(AtomicBool::new(false)),
            pasted: Arc::new(AtomicBool::new(false)),
            frame: 0,
            selected: None,
            status_note: String::new(),
            screenshot,
        };
        // SNIPREADER_TEST_AREA + SNIPREADER_TEST_DELAY: play a hotkey after N seconds with
        // the window hidden (timing test for the capture flow; see SNIPREADER_TRACE).
        if std::env::var("SNIPREADER_TEST_AREA").is_ok() {
            let delay = std::env::var("SNIPREADER_TEST_DELAY").ok().and_then(|v| v.parse().ok()).unwrap_or(0u64);
            let (flag, wake) = (app.test_trigger.clone(), cc.egui_ctx.clone());
            // SNIPREADER_TEST_REPEAT=n: repeat every 4 s (leak check).
            let repeat = std::env::var("SNIPREADER_TEST_REPEAT").ok().and_then(|v| v.parse().ok()).unwrap_or(1u32);
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(delay));
                for _ in 0..repeat {
                    trace("hotkey (simulated)");
                    flag.store(true, Ordering::Relaxed);
                    wake.request_repaint();
                    std::thread::sleep(Duration::from_secs(4));
                }
            });
        }
        let testing = ["SNIPREADER_SELFTEST", "SNIPREADER_TEST_AREA"].iter().any(|v| std::env::var(v).is_ok());
        if app.screenshot.is_none() && std::env::var("SNIPREADER_SELFTEST").is_err() {
            ocr::warm_up(); // prepares Vision for a new build in the background
        }
        app.welcome_open = !app.s.onboarded && app.screenshot.is_none() && !testing;
        // SNIPREADER_WELCOME_PAGE=n opens the guide at page n (for checking its look).
        if let Some(n) = std::env::var("SNIPREADER_WELCOME_PAGE").ok().and_then(|v| v.parse().ok()) {
            app.welcome_open = true;
            app.welcome_page = n;
        }
        if app.screenshot.is_none() {
            app.paste_thread = Some(system::start_paste_watcher(app.paste_watch.clone(), app.pasted.clone(), cc.egui_ctx.clone()));
            app.hotkeys = GlobalHotKeyManager::new().ok();
            app.register_hotkeys();
        }
        app
    }

    pub fn register_hotkeys(&mut self) {
        let Some(m) = &self.hotkeys else { return };
        if !self.registered.is_empty() {
            let _ = m.unregister_all(&self.registered);
        }
        self.registered.clear();
        self.hotkey_ids.clear();
        if self.recording.is_some() {
            return;
        }
        for a in Action::GLOBAL {
            if let Some(hk) = self.s.shortcuts.get(&a).and_then(|s| s.hotkey()) {
                if m.register(hk).is_ok() {
                    self.hotkey_ids.insert(hk.id(), a);
                    self.registered.push(hk);
                }
            }
        }
    }

    pub fn show_hud(&mut self, kind: HudKind, title: &str, subtitle: &str) {
        let morph = matches!(&self.hud, Some(h) if h.kind == HudKind::Working);
        self.hud = Some(Hud { kind, morph, title: title.into(), subtitle: subtitle.into(), started: Instant::now() });
    }

    // ---------- actions ----------

    pub fn run(&mut self, a: Action) {
        match a {
            Action::CaptureText => self.start(self.job(Mode::Text)),
            Action::CaptureLast => {
                let mut j = self.job(Mode::Text);
                j.use_last_area = true;
                self.start(j)
            }
            Action::CaptureWithBreaks => {
                let mut j = self.job(Mode::Text);
                j.keep_breaks = true;
                self.start(j)
            }
            Action::CaptureWithoutBreaks => {
                let mut j = self.job(Mode::Text);
                j.keep_breaks = false;
                self.start(j)
            }
            Action::CaptureWithSpeech => {
                let mut j = self.job(Mode::Text);
                j.speak = true;
                self.start(j)
            }
            Action::ReadCode => self.start(self.job(Mode::Code)),
            Action::StopSpeaking => self.stop_speaking(),
            Action::ToggleAdditive | Action::ModeAdditive => self.toggle_additive(),
            Action::ClearAdditive => self.clear_additive(),
            Action::ModeKeepBreaks => self.toggle_keep_breaks(),
            Action::ModeSpeech => {
                self.s.speak_after_capture = !self.s.speak_after_capture;
                let on = if self.s.speak_after_capture { "ON" } else { "OFF" };
                self.show_hud(HudKind::Toggle(self.s.speak_after_capture), "Text to Speech", on);
            }
        }
    }

    fn job(&self, mode: Mode) -> Job {
        Job {
            mode,
            keep_breaks: self.s.keep_line_breaks,
            speak: self.s.speak_after_capture,
            use_last_area: false,
            file: None,
        }
    }

    pub fn read_image_file(&mut self) {
        if self.busy {
            return;
        }
        // The file dialog runs in osascript; wait for it off the UI thread.
        let (tx, wake) = (self.wake_tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            if let Some(p) = system::choose_image() {
                let _ = tx.send(Wake::File(p));
                wake.request_repaint();
            }
        });
    }

    fn start(&mut self, job: Job) {
        if self.busy {
            return;
        }
        if job.use_last_area && self.s.last_area.is_none() {
            self.show_hud(HudKind::Empty, "No Previous Selection", "Capture some text first");
            return;
        }
        self.busy = true;
        let nap_fix = std::env::var("SNIPREADER_NO_NAPFIX").is_err();
        self.activity = if nap_fix { system::begin_activity("Capturing text") } else { None };
        trace(if job.use_last_area { "job start (last area)" } else if job.file.is_some() { "job start (image file)" } else { "job start (select area)" });
        let tx = self.tx.clone();
        let wake = self.ctx.clone();
        let area = if job.use_last_area { self.s.last_area.clone() } else { None };
        let silent = self.s.disable_sound;
        let opts = ocr::RecognizeOptions {
            language: lang_code(&self.s.language).to_string(),
            auto_detect_language: self.s.auto_detect_language,
            custom_words: self.s.custom_words.clone(),
        };
        // Interactive selection: show the loading HUD the moment the mouse is released,
        // not only once screencapture has written the file (that can lag behind).
        let selecting = Arc::new(AtomicBool::new(job.file.is_none() && area.is_none()));
        if selecting.load(Ordering::Relaxed) {
            let (still, tx2, wake2, mode) = (selecting.clone(), self.tx.clone(), self.ctx.clone(), job.mode);
            std::thread::spawn(move || {
                let mut pressed = false;
                while still.load(Ordering::Relaxed) {
                    let down = system::left_mouse_down();
                    if down && !pressed {
                        trace("mouse down");
                    }
                    if pressed && !down {
                        trace("mouse released");
                        let _ = tx2.send(JobResult::Working(mode));
                        wake2.request_repaint();
                        return;
                    }
                    pressed = down;
                    std::thread::sleep(Duration::from_millis(15));
                }
            });
        }
        std::thread::spawn(move || {
            if nap_fix {
                system::boost_thread();
            }
            let (path, area) = match &job.file {
                Some(f) => (f.clone(), None),
                None => match {
                    let g = system::grab_screen(area.as_deref(), silent);
                    selecting.store(false, Ordering::Relaxed);
                    g
                } {
                    system::Grab::Done { path, area } => (path, area),
                    system::Grab::Canceled => {
                        trace("canceled");
                        let _ = tx.send(JobResult::Canceled);
                        wake.request_repaint();
                        return;
                    }
                    system::Grab::Failed(e) => {
                        let _ = tx.send(JobResult::Failed(e));
                        wake.request_repaint();
                        return;
                    }
                },
            };
            trace("grab done");
            let _ = tx.send(JobResult::Working(job.mode));
            wake.request_repaint();
            let p = path.to_string_lossy().to_string();
            let own_grab = job.file.is_none();
            let res = match job.mode {
                Mode::Text => match ocr::recognize_lines_isolated(&p, &opts) {
                    Ok(lines) => JobResult::Text { lines, job, area },
                    Err(e) => JobResult::Failed(e),
                },
                Mode::Code => match ocr::read_codes_isolated(&p) {
                    Ok(codes) => JobResult::Codes { codes, area },
                    Err(e) => JobResult::Failed(e),
                },
            };
            trace("recognized");
            // The grab is a picture of your screen: don't leave it lying in the temp folder.
            if own_grab {
                let _ = std::fs::remove_file(&path);
            }
            let _ = tx.send(res);
            wake.request_repaint();
        });
    }

    fn finish(&mut self, r: JobResult) {
        let showing_working = matches!(&self.hud, Some(h) if h.kind == HudKind::Working);
        if let JobResult::Working(mode) = r {
            // Ignore a late "working" (the job already ended) or a repeat.
            if !self.busy || showing_working {
                return;
            }
            if ocr::is_ready() {
                let title = if mode == Mode::Code { "Reading Code…" } else { "Recognizing Text…" };
                self.show_hud(HudKind::Working, title, "");
            } else {
                // First use of a new build: macOS is still preparing Vision (about a minute).
                self.show_hud(HudKind::Working, "Preparing Text Recognition…", "First run after install, up to a minute");
            }
            trace("loading HUD shown");
            return;
        }
        trace("result shown");
        self.busy = false;
        self.activity = None;
        // The result replaces the loading HUD; if the result shows no HUD (e.g. success
        // messages are turned off) the spinner must still go away.
        if showing_working {
            self.hud = None;
        }
        match r {
            JobResult::Working(_) => {}
            JobResult::Canceled => self.show_hud(HudKind::Canceled, "Capture Text", "Operation Canceled"),
            JobResult::Failed(e) => {
                self.status_note = e;
                self.show_hud(HudKind::Error, "Capture Text Failed", "See the note below");
            }
            JobResult::Text { lines, job, area } => {
                if area.is_some() {
                    self.s.last_area = area;
                }
                let mut lines: Vec<_> = lines.into_iter().filter(|l| !l.text.trim().is_empty()).collect();
                crate::layout::sort_lines(&mut lines);
                let (with_breaks, without) = crate::layout::join(&lines);
                let text = if job.keep_breaks { with_breaks } else { without };
                if text.is_empty() {
                    self.show_hud(HudKind::Empty, "No Text Detected", "Try a larger area");
                    return;
                }
                self.deliver(&text, "Text");
                if job.speak {
                    self.speak(&text);
                }
            }
            JobResult::Codes { codes, area } => {
                if area.is_some() {
                    self.s.last_area = area;
                }
                if codes.is_empty() {
                    self.show_hud(HudKind::Empty, "No QR/Bar Code Detected", "");
                    return;
                }
                let text = codes.join("\n");
                self.deliver(&text, "QR/Bar Code");
            }
        }
    }

    /// Copy to the clipboard (with Additive Clipboard), show the HUD, open links.
    fn deliver(&mut self, text: &str, kind: &str) {
        let out = if self.s.additive {
            if self.additive_buffer.is_empty() {
                self.additive_buffer = text.to_string();
            } else {
                self.additive_buffer.push_str(ADDITIVE_SEP);
                self.additive_buffer.push_str(text);
            }
            self.additive_buffer.clone()
        } else {
            text.to_string()
        };
        // The timing test must not clobber the user's clipboard.
        if std::env::var("SNIPREADER_TEST_AREA").is_err() {
            system::clipboard_set(&out);
        }
        self.last_written = out;
        let when = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.history.insert(0, Entry { text: text.to_string(), kind: kind.into(), when });
        self.history.truncate(50);
        self.selected = Some(0);
        if !self.s.disable_success {
            if self.s.additive {
                // Make it obvious that the old text is still there.
                self.show_hud(HudKind::Added, "Added to Clipboard", "Additive Clipboard is ON");
            } else {
                self.show_hud(HudKind::Success, "Copied to Clipboard", "");
            }
        }
        if self.s.open_links {
            if let Some(url) = system::as_link(text) {
                system::open_url(&url);
            }
        }
    }

    pub fn toggle_keep_breaks(&mut self) {
        self.s.keep_line_breaks = !self.s.keep_line_breaks;
        let on = if self.s.keep_line_breaks { "ON" } else { "OFF" };
        self.show_hud(HudKind::Toggle(self.s.keep_line_breaks), "Keep Line Breaks", on);
    }

    pub fn toggle_additive(&mut self) {
        self.s.additive = !self.s.additive;
        // Each time it is switched, start from an empty additive buffer.
        self.additive_buffer.clear();
        let on = if self.s.additive { "ON" } else { "OFF" };
        self.show_hud(HudKind::Toggle(self.s.additive), "Additive Clipboard", on);
    }

    pub fn clear_additive(&mut self) {
        self.additive_buffer.clear();
        self.show_hud(HudKind::Cleared, "Additive Clipboard Cleared", "");
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
        self.additive_buffer.clear();
        self.selected = None;
        system::clipboard_clear();
        self.show_hud(HudKind::Cleared, "Clipboard Cleared", "");
    }

    pub fn speak(&mut self, text: &str) {
        self.stop_speaking();
        self.speaking = system::speak(text, self.s.tts_rate);
    }

    pub fn stop_speaking(&mut self) {
        if let Some(mut c) = self.speaking.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    pub fn is_speaking(&mut self) -> bool {
        match &mut self.speaking {
            Some(c) => match c.try_wait() {
                Ok(None) => true,
                _ => {
                    self.speaking = None;
                    false
                }
            },
            None => false,
        }
    }

    // ---------- per-frame plumbing ----------

    fn poll(&mut self, ctx: &egui::Context) {
        // "Clear Automatically": a ⌘V press while the clipboard still
        // holds our additive text empties the additive history.
        let watch = self.paste_watch.load(Ordering::Relaxed);
        if self.pasted.swap(false, Ordering::Relaxed)
            && watch
            && !self.additive_buffer.is_empty()
            && system::clipboard_get() == self.last_written
        {
            self.additive_buffer.clear();
        }
        // A finished capture only copies and shows the HUD; it never brings the window forward.
        while let Ok(r) = self.rx.try_recv() {
            self.finish(r);
        }
        while let Ok(w) = self.wake_rx.try_recv() {
            match w {
                Wake::Key(ev) => {
                    if ev.state == global_hotkey::HotKeyState::Pressed {
                        if let Some(a) = self.hotkey_ids.get(&ev.id).copied() {
                            self.run(a);
                        }
                    }
                }
                Wake::Menu(e) => {
                    if let Some(cmd) = tray::Tray::command_for(&e) {
                        self.tray_command(cmd, ctx);
                    }
                }
                Wake::File(p) => {
                    let mut j = self.job(Mode::Text);
                    j.file = Some(p);
                    self.start(j);
                }
            }
        }
        if let Some(m) = ctx.input(|i| i.viewport().monitor_size) {
            if m.x > 0.0 && m.y > 0.0 {
                self.monitor = m;
            }
        }
        // Local "Capture Text mode" and other keys while our window is focused.
        if self.recording.is_none() {
            let presses: Vec<(egui::Key, egui::Modifiers)> = ctx.input(|i| {
                i.events
                    .iter()
                    .filter_map(|e| match e {
                        egui::Event::Key { key, pressed: true, modifiers, repeat: false, .. } => {
                            Some((*key, *modifiers))
                        }
                        _ => None,
                    })
                    .collect()
            });
            for (k, m) in presses {
                for a in Action::MODE {
                    if self.s.shortcuts.get(&a).is_some_and(|s| s.matches(k, m)) {
                        self.run(a);
                    }
                }
                if m.command && k == egui::Key::Comma {
                    self.prefs_open = true;
                }
            }
        }
    }

    fn tray_command(&mut self, cmd: tray::Cmd, ctx: &egui::Context) {
        use tray::Cmd;
        match cmd {
            Cmd::Run(a) => self.run(a),
            Cmd::ReadImage => self.read_image_file(),
            Cmd::KeepBreaks => self.toggle_keep_breaks(),
            Cmd::Additive => self.toggle_additive(),
            Cmd::ClearHistory => self.clear_history(),
            Cmd::Speech => self.run(Action::ModeSpeech),
            Cmd::Prefs => {
                self.prefs_open = true;
                system::activate_app();
            }
            Cmd::ShowWindow => self.show_window(ctx),
            Cmd::Quit => self.quit(ctx),
        }
    }

    /// Close button: hide the window and the Dock icon, keep running in the menu bar.
    /// Hides the main window and shrinks it to a few points: macOS keeps a full-size
    /// graphics buffer for a hidden window (about 50 MB for a big Retina window), and a
    /// tiny window frees almost all of it. The real size and place come back on show.
    pub fn hide_window(&mut self, ctx: &egui::Context) {
        if self.window_visible {
            if let Some(g) = window_geometry(ctx) {
                self.saved_geometry = Some(g);
            }
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(Vec2::new(1.0, 1.0)));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(16.0, 16.0)));
        self.window_visible = false;
        system::set_dock_icon(false);
    }

    pub fn show_window(&mut self, ctx: &egui::Context) {
        system::set_dock_icon(true);
        if !self.window_visible {
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(crate::MIN_WINDOW));
            if let Some(g) = self.saved_geometry {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(g[0], g[1])));
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(g[2], g[3])));
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(crate::DEFAULT_WINDOW));
            }
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        self.window_visible = true;
    }

    pub fn quit(&mut self, ctx: &egui::Context) {
        self.quitting = true;
        // Always the main window (this may be called from inside another window).
        ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
    }

    fn handle_close(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.viewport().close_requested()) || self.quitting || self.screenshot.is_some() {
            return;
        }
        // Without a menu-bar icon there would be no way back, so then closing quits.
        if self.tray.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.hide_window(ctx);
        }
    }

    /// Turns the ⌘V watch on only while there is additive text that a paste could clear.
    /// Runs at the end of each frame, after any new capture result.
    fn update_paste_watch(&mut self) {
        // Only watch while there is additive text that a paste could clear.
        let watch = self.s.additive && self.s.clear_additive_auto && !self.additive_buffer.is_empty();
        let was = self.paste_watch.swap(watch, Ordering::Relaxed);
        if watch && !was {
            if let Some(t) = &self.paste_thread {
                t.unpark(); // the watcher sleeps until it is needed
            }
        }
    }

    fn sync_tray(&mut self) {
        if self.screenshot.is_some() {
            return;
        }
        if self.s.show_in_menu_bar && self.tray.is_none() && !self.tray_tried {
            self.tray_tried = true;
            self.tray = tray::Tray::new();
        }
        if !self.s.show_in_menu_bar && self.tray.is_some() {
            self.tray = None;
            self.tray_tried = false;
            // Without the menu-bar icon a hidden window could never be reached again.
            if !self.window_visible {
                let ctx = self.ctx.clone();
                self.show_window(&ctx);
            }
        }
        let speaking = self.is_speaking();
        if let Some(t) = &self.tray {
            t.sync(&self.s, speaking);
        }
    }

    fn screenshot_step(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot.clone() else { return };
        self.frame += 1;
        ctx.request_repaint();
        if self.frame == 120 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
        }
        let shot = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = shot {
            let [w, h] = img.size;
            let mut buf = Vec::with_capacity(w * h * 4);
            for p in &img.pixels {
                buf.extend_from_slice(&[p.r(), p.g(), p.b(), 255]);
            }
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Err(e) = image::save_buffer(&path, &buf, w as u32, h as u32, image::ColorType::Rgba8) {
                eprintln!("could not save screenshot: {e}");
            }
            std::process::exit(0);
        }
        if self.frame > 600 {
            eprintln!("screenshot never arrived");
            std::process::exit(1);
        }
    }
}

/// The main window's [inner width, inner height, outer x, outer y] in points.
fn window_geometry(ctx: &egui::Context) -> Option<[f32; 4]> {
    ctx.input(|i| {
        let vp = i.viewport();
        let size = vp.inner_rect?.size();
        let pos = vp.outer_rect?.min;
        (size.x >= 100.0 && size.y >= 100.0).then_some([size.x, size.y, pos.x, pos.y])
    })
}

pub fn lang_code(name: &str) -> &'static str {
    LANGUAGES.iter().find(|(n, _)| *n == name).map(|(_, c)| *c).unwrap_or("en-US")
}

fn style(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Light);
    ctx.all_styles_mut(|s| {
        s.visuals = egui::Visuals::light();
        s.visuals.panel_fill = WINDOW_BG;
        s.visuals.window_fill = WINDOW_BG;
        s.visuals.override_text_color = Some(TEXT);
        s.visuals.selection.bg_fill = ACCENT;
        s.visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
        s.visuals.widgets.inactive.rounding = Rounding::same(5.0);
        s.visuals.widgets.hovered.rounding = Rounding::same(5.0);
        s.visuals.widgets.active.rounding = Rounding::same(5.0);
        s.visuals.widgets.inactive.weak_bg_fill = Color32::WHITE;
        s.visuals.widgets.inactive.bg_fill = Color32::WHITE;
        s.visuals.widgets.hovered.bg_fill = Color32::from_gray(0xF4);
        s.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
        s.spacing.item_spacing = Vec2::new(8.0, 6.0);
        s.spacing.button_padding = Vec2::new(10.0, 4.0);
        s.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(13.0));
        s.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(13.0));
        s.text_styles.insert(egui::TextStyle::Small, egui::FontId::proportional(11.0));
        s.text_styles.insert(egui::TextStyle::Heading, egui::FontId::proportional(20.0));
    });
}

impl eframe::App for App {
    // Transparent so the floating HUD window has see-through corners; panels cover the main window.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // Test runs never save (no test history or test area in the user's settings).
        let test = std::env::var("SNIPREADER_SELFTEST").is_ok() || std::env::var("SNIPREADER_TEST_AREA").is_ok();
        if self.screenshot.is_none() && !test {
            // Our own window size/place (eframe's would record the shrunk hidden window).
            let g = if self.window_visible { window_geometry(&self.ctx) } else { None };
            if let Some(g) = g.or(self.saved_geometry) {
                crate::save_window_geometry(g);
            }
            eframe::set_value(storage, "settings", &self.s);
            eframe::set_value(storage, "history", &self.history);
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.main_opaque {
            self.main_opaque = system::make_window_opaque("SnipReader");
        }
        self.poll(ctx);
        self.sync_tray();
        self.handle_close(ctx);
        // SNIPREADER_SELFTEST=<png>: hide the window, show the HUD, photograph the screen once
        // its entrance animation has finished, quit. SNIPREADER_SELFTEST=hold instead keeps
        // the HUD frozen on screen for 6 s so another tool can photograph its window.
        // SNIPREADER_HUD="Title|Subtitle" and SNIPREADER_HUD_KIND=success|added|empty|error|
        // canceled|on|off|cleared pick the message.
        self.frame += 1;
        // Timing test (SNIPREADER_TEST_AREA): hide at once; a thread started in App::new
        // plays the hotkey after SNIPREADER_TEST_DELAY seconds.
        if std::env::var("SNIPREADER_TEST_AREA").is_ok() {
            if self.frame == 2 {
                // SNIPREADER_TEST_VISIBLE keeps the window open (e.g. behind other windows).
                if std::env::var("SNIPREADER_TEST_VISIBLE").is_err() {
                    self.hide_window(ctx);
                }
                self.s.last_area = std::env::var("SNIPREADER_TEST_AREA").ok();
            }
            if self.test_trigger.swap(false, Ordering::Relaxed) {
                trace("UI woke for hotkey");
                let mut j = self.job(Mode::Text);
                j.use_last_area = true;
                self.start(j);
            }
        }
        // SNIPREADER_TEST_REOPEN (with SNIPREADER_SELFTEST=none): hide the window after 3 s
        // and show it again after 9 s, to check the shrink-on-hide round trip.
        if std::env::var("SNIPREADER_TEST_REOPEN").is_ok() {
            let t = self.started.elapsed();
            if t > Duration::from_secs(3) && self.test_step == 0 {
                self.test_step = 1;
                self.hide_window(ctx);
            }
            if t > Duration::from_secs(9) && self.test_step == 1 {
                self.test_step = 2;
                self.show_window(ctx);
            }
            ctx.request_repaint_after(Duration::from_millis(200));
        }
        if let Ok(out) = std::env::var("SNIPREADER_SELFTEST") {
            if self.frame == 40 && std::env::var("SNIPREADER_TEST_REOPEN").is_err() && std::env::var("SNIPREADER_TEST_AREA").is_err() {
                self.hide_window(ctx);
                let msg = std::env::var("SNIPREADER_HUD").unwrap_or_else(|_| "Copied to Clipboard".into());
                let (title, subtitle) = msg.split_once('|').unwrap_or((msg.as_str(), ""));
                let kind = std::env::var("SNIPREADER_HUD_KIND").ok().and_then(|k| HudKind::parse(&k)).unwrap_or(HudKind::Success);
                self.show_hud(kind, title, subtitle);
                self.selftest_shown = Some(Instant::now());
            }
            if let Some(shown) = self.selftest_shown {
                let settled = Duration::from_millis(800);
                if out == "hold" {
                    if let Some(h) = &mut self.hud {
                        h.started = Instant::now() - settled; // freeze after the animation
                    }
                    if shown.elapsed() > Duration::from_secs(6) {
                        std::process::exit(0);
                    }
                } else if shown.elapsed() > settled {
                    let _ = std::process::Command::new("/usr/sbin/screencapture").arg("-x").arg(&out).status();
                    std::process::exit(0);
                }
            }
        }
        // SNIPREADER_PANE=0|1 shows a Preferences pane in the main window (for checking its look).
        let preview = std::env::var("SNIPREADER_PANE").ok().and_then(|v| v.parse::<usize>().ok());
        if let Some(tab) = preview {
            self.prefs_tab = tab.min(1);
            crate::prefs::draw(self, ctx);
        } else {
            crate::ui::main_window(self, ctx);
        }
        if self.prefs_open {
            crate::prefs::show(self, ctx);
        }
        // A recognition that takes long is macOS preparing Vision: say so on the HUD.
        if let Some(h) = &mut self.hud {
            if h.kind == HudKind::Working && h.subtitle.is_empty() && h.started.elapsed() > Duration::from_millis(2500) {
                h.title = "Preparing Text Recognition…".into();
                h.subtitle = "macOS is preparing it, up to a minute".into();
                trace("HUD: preparing");
            }
        }
        if self.welcome_open {
            crate::onboarding::show(self, ctx);
        }
        crate::ui::hud(self, ctx);
        self.screenshot_step(ctx);
        self.update_paste_watch();
        // No steady redraw loop: hotkeys, the menu bar, the capture thread and the ⌘V watch
        // all wake the app themselves, and the HUD animates itself. Only speech needs a
        // slow check (to notice when it ends), and the self-tests count frames.
        if std::env::var("SNIPREADER_SELFTEST").is_ok() {
            ctx.request_repaint();
        } else if self.speaking.is_some() {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
    }
}
