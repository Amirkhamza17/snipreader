//! Talking to macOS: screen grab, clipboard, speech, opening links.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

/// Where the grab is written (deleted again after recognition).
pub fn grab_path() -> PathBuf {
    let dir = std::env::temp_dir().join("snipreader");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("screen_latest.png")
}

pub enum Grab {
    Done { path: PathBuf, area: Option<String> },
    Canceled,
    Failed(String),
}

/// Runs `/usr/sbin/screencapture`: interactive (`-i`) or a fixed area (`-R x,y,w,h`),
/// with `-x` when sound effects are off.
pub fn grab_screen(area: Option<&str>, silent: bool) -> Grab {
    let path = grab_path();
    let _ = std::fs::remove_file(&path);
    let mut cmd = Command::new("/usr/sbin/screencapture");
    match area {
        Some(r) => {
            cmd.arg("-R").arg(r);
        }
        None => {
            cmd.arg("-i");
        }
    }
    if silent {
        cmd.arg("-x");
    }
    cmd.arg(&path);
    match cmd.stdout(Stdio::null()).stderr(Stdio::piped()).output() {
        Err(e) => Grab::Failed(e.to_string()),
        Ok(out) if !path.exists() => {
            // Esc / a click outside ends quietly; a real failure (no Screen Recording
            // permission, an area on a display that is gone) prints why.
            let why = String::from_utf8_lossy(&out.stderr).trim().to_string();
            if why.is_empty() && screen_access() {
                Grab::Canceled
            } else if !screen_access() {
                Grab::Failed("SnipReader needs Screen Recording permission: System Settings → Privacy & Security → Screen & System Audio Recording.".into())
            } else {
                Grab::Failed(why)
            }
        }
        Ok(_) => {
            let area = area.map(|s| s.to_string()).or_else(|| read_capture_rect(&path));
            Grab::Done { path, area }
        }
    }
}

/// The last area comes from the grab's kMDItemScreenCaptureGlobalRect, which
/// screencapture writes as an extended attribute (a binary plist
/// [x, y, w, h]); reading it directly works at once, while `mdls` returns nothing for files
/// in the temp folder because Spotlight never indexes them.
fn read_capture_rect(path: &PathBuf) -> Option<String> {
    let raw = read_xattr(path, "com.apple.metadata:kMDItemScreenCaptureGlobalRect")?;
    let mut child = Command::new("/usr/bin/plutil")
        .args(["-convert", "json", "-o", "-", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let wrote = child.stdin.take().map(|mut i| i.write_all(&raw).is_ok()).unwrap_or(false);
    let out = child.wait_with_output().ok()?; // always reap plutil
    if !wrote {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    let nums: Vec<i64> = s
        .split(|c: char| !(c.is_ascii_digit() || c == '-' || c == '.'))
        .filter(|t| !t.is_empty())
        .filter_map(|t| t.parse::<f64>().ok())
        .map(|f| f.round() as i64)
        .collect();
    if nums.len() == 4 && nums[2] > 0 && nums[3] > 0 {
        Some(format!("{},{},{},{}", nums[0], nums[1], nums[2], nums[3]))
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn read_xattr(path: &PathBuf, name: &str) -> Option<Vec<u8>> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    extern "C" {
        fn getxattr(path: *const i8, name: *const i8, value: *mut u8, size: usize, position: u32, options: i32) -> isize;
    }
    let p = CString::new(path.as_os_str().as_bytes()).ok()?;
    let n = CString::new(name).ok()?;
    let mut buf = vec![0u8; 1024];
    let len = unsafe { getxattr(p.as_ptr(), n.as_ptr(), buf.as_mut_ptr(), buf.len(), 0, 0) };
    if len <= 0 {
        return None;
    }
    buf.truncate(len as usize);
    Some(buf)
}

#[cfg(not(target_os = "macos"))]
fn read_xattr(_path: &PathBuf, _name: &str) -> Option<Vec<u8>> {
    None
}

/// Keeps macOS from putting the app into App Nap while a capture runs. With no visible
/// window the app is napped after a while: its threads get background priority and the
/// text recognition can take seconds instead of a fraction of one. Ends on drop.
pub struct Activity(#[allow(dead_code)] ActivityToken);

#[cfg(target_os = "macos")]
type ActivityToken = objc2::rc::Retained<objc2::runtime::AnyObject>;
#[cfg(not(target_os = "macos"))]
type ActivityToken = ();

#[cfg(target_os = "macos")]
pub fn begin_activity(reason: &str) -> Option<Activity> {
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send, msg_send_id};
    // NSActivityUserInitiated | NSActivityLatencyCritical
    const OPTIONS: u64 = 0x00FF_FFFF | 0xFF_0000_0000;
    let why = objc2_foundation::NSString::from_str(reason);
    unsafe {
        let info: *mut AnyObject = msg_send![class!(NSProcessInfo), processInfo];
        if info.is_null() {
            return None;
        }
        let token: Option<Retained<AnyObject>> =
            msg_send_id![&*info, beginActivityWithOptions: OPTIONS, reason: &*why];
        token.map(Activity)
    }
}

#[cfg(target_os = "macos")]
impl Drop for Activity {
    fn drop(&mut self) {
        use objc2::runtime::AnyObject;
        use objc2::{class, msg_send};
        unsafe {
            let info: *mut AnyObject = msg_send![class!(NSProcessInfo), processInfo];
            if !info.is_null() {
                let _: () = msg_send![info, endActivity: &*self.0];
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn begin_activity(_reason: &str) -> Option<Activity> {
    None
}

/// Runs the current (capture) thread at user-initiated priority, so it is not left on
/// slow cores when the app sits in the background.
#[cfg(target_os = "macos")]
pub fn boost_thread() {
    extern "C" {
        fn pthread_set_qos_class_self_np(qos: u32, relative_priority: i32) -> i32;
    }
    const QOS_CLASS_USER_INITIATED: u32 = 0x19;
    unsafe {
        pthread_set_qos_class_self_np(QOS_CLASS_USER_INITIATED, 0);
    }
}

#[cfg(not(target_os = "macos"))]
pub fn boost_thread() {}

/// Runs the current thread (and the helpers it starts) at utility priority: background
/// upkeep that should never compete with what the user is doing.
#[cfg(target_os = "macos")]
pub fn low_priority_thread() {
    extern "C" {
        fn pthread_set_qos_class_self_np(qos: u32, relative_priority: i32) -> i32;
    }
    const QOS_CLASS_UTILITY: u32 = 0x11;
    unsafe {
        pthread_set_qos_class_self_np(QOS_CLASS_UTILITY, 0);
    }
}

#[cfg(not(target_os = "macos"))]
pub fn low_priority_thread() {}

pub fn clipboard_set(text: &str) {
    if let Ok(mut child) = Command::new("/usr/bin/pbcopy")
        .env("LANG", "en_US.UTF-8")
        .stdin(Stdio::piped())
        .spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();
    }
}

pub fn clipboard_get() -> String {
    Command::new("/usr/bin/pbpaste")
        .env("LANG", "en_US.UTF-8")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default()
}

pub fn clipboard_clear() {
    clipboard_set("");
}

pub fn speak(text: &str, rate: u32) -> Option<Child> {
    Command::new("/usr/bin/say")
        .arg("-r")
        .arg(rate.to_string())
        .arg("--")
        .arg(text)
        .spawn()
        .ok()
}

pub fn open_url(url: &str) {
    run_detached(Command::new("/usr/bin/open").arg(url));
}

/// Starts a command and reaps it on a background thread (no zombie processes).
fn run_detached(cmd: &mut Command) {
    if let Ok(mut child) = cmd.stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
}

/// Has the app been allowed to record the screen? (No prompt.)
#[cfg(target_os = "macos")]
pub fn screen_access() -> bool {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
    }
    unsafe { CGPreflightScreenCaptureAccess() }
}

/// Asks macOS for Screen Recording permission (shows the system prompt the first time).
#[cfg(target_os = "macos")]
pub fn request_screen_access() -> bool {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGRequestScreenCaptureAccess() -> bool;
    }
    unsafe { CGRequestScreenCaptureAccess() }
}

#[cfg(not(target_os = "macos"))]
pub fn screen_access() -> bool {
    true
}
#[cfg(not(target_os = "macos"))]
pub fn request_screen_access() -> bool {
    true
}

/// Is this point (global, points, origin top-left) on one of the connected displays?
#[cfg(target_os = "macos")]
pub fn point_on_a_display(p: eframe::egui::Pos2) -> bool {
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGRect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGGetActiveDisplayList(max: u32, displays: *mut u32, count: *mut u32) -> i32;
        fn CGDisplayBounds(display: u32) -> CGRect;
    }
    let mut ids = [0u32; 16];
    let mut n = 0u32;
    if unsafe { CGGetActiveDisplayList(16, ids.as_mut_ptr(), &mut n) } != 0 {
        return true; // can't tell: keep the saved place
    }
    ids[..n as usize].iter().any(|&d| {
        let b = unsafe { CGDisplayBounds(d) };
        let (x, y) = (p.x as f64, p.y as f64);
        x >= b.x && x < b.x + b.w && y >= b.y && y < b.y + b.h
    })
}

#[cfg(not(target_os = "macos"))]
pub fn point_on_a_display(_p: eframe::egui::Pos2) -> bool {
    true
}

/// The .app bundle we run from (or the binary itself outside a bundle).
fn bundle_path() -> Option<String> {
    let s = std::env::current_exe().ok()?.to_string_lossy().to_string();
    Some(match s.find(".app/") {
        Some(i) => s[..i + 4].to_string(),
        None => s,
    })
}

/// Starts a fresh copy of the app a moment after this one quits (to pick up a new
/// Screen Recording permission). The path goes in as an argument, not into the script.
pub fn relaunch() {
    if let Some(path) = bundle_path() {
        run_detached(Command::new("/bin/sh").args(["-c", "sleep 1; /usr/bin/open \"$0\"", &path]));
    }
}

/// Opens System Settings at Privacy & Security → Screen Recording.
pub fn open_screen_recording_settings() {
    open_url("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture");
}

/// "Automatically Open Links": take the first link found;
/// open it only when the whole text is at most `LINK_SLACK` characters longer than the link;
/// a link without a scheme gets "mailto" when it has "@", otherwise "https".
pub const LINK_SLACK: usize = 70;

pub fn as_link(text: &str) -> Option<String> {
    let link = first_link(text)?;
    if text.chars().count() > link.chars().count() + LINK_SLACK {
        return None;
    }
    let lower = link.to_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:") {
        Some(link)
    } else if link.contains(':') {
        None // only web and mail links are opened, never other app schemes
    } else if link.contains('@') {
        Some(format!("mailto:{link}"))
    } else {
        Some(format!("https://{link}"))
    }
}

/// A simple link finder: web addresses, domains and e-mail addresses.
fn first_link(text: &str) -> Option<String> {
    for raw in text.split_whitespace() {
        let t = raw.trim_matches(|c: char| "()[]<>\"'.,;:!?".contains(c));
        let lower = t.to_lowercase();
        let has_scheme = lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:");
        let is_email = t.contains('@') && t.split('@').nth(1).is_some_and(|d| d.contains('.'));
        let is_domain = !t.contains('@')
            && t.contains('.')
            && t.split('.').last().is_some_and(|tld| tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic()))
            && t.chars().all(|c| c.is_ascii_alphanumeric() || "-._~/?#=&%+".contains(c))
            && t.chars().next().is_some_and(|c| c.is_ascii_alphanumeric());
        if has_scheme || is_email || is_domain {
            return Some(t.to_string());
        }
    }
    None
}

pub fn set_launch_at_login(on: bool) {
    let exe = std::env::current_exe().ok();
    // Use the .app bundle when we are inside one.
    let target = exe.and_then(|p| {
        let s = p.to_string_lossy().to_string();
        match s.find(".app/") {
            Some(i) => Some(s[..i + 4].to_string()),
            None => Some(s),
        }
    });
    let Some(path) = target else { return };
    let name = std::path::Path::new(&path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    // The path and name go in as arguments, never pasted into the script text.
    let (script, arg) = if on {
        ("on run argv\ntell application \"System Events\" to make login item at end with properties {path:(item 1 of argv), hidden:false}\nend run", path)
    } else {
        ("on run argv\ntell application \"System Events\" to delete login item (item 1 of argv)\nend run", name)
    };
    run_detached(Command::new("/usr/bin/osascript").arg("-e").arg(script).arg(arg));
}

/// Ask macOS for an image file (used by "Read Text From Image…").
pub fn choose_image() -> Option<PathBuf> {
    let out = Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg("POSIX path of (choose file of type {\"public.image\"} with prompt \"Choose an image to read text from\")")
        .output()
        .ok()?;
    let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if p.is_empty() {
        None
    } else {
        Some(PathBuf::from(p))
    }
}

/// Show or hide the Dock icon (regular app vs. menu-bar-only app).
#[cfg(target_os = "macos")]
pub fn set_dock_icon(visible: bool) {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    unsafe {
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        if app.is_null() {
            return;
        }
        // NSApplicationActivationPolicyRegular = 0, Accessory = 1.
        let policy: isize = if visible { 0 } else { 1 };
        let _: bool = msg_send![app, setActivationPolicy: policy];
    }
    if visible {
        activate_app();
    }
}

/// Bring our windows to the front (used when opening Preferences from the menu bar).
#[cfg(target_os = "macos")]
pub fn activate_app() {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    unsafe {
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        if !app.is_null() {
            let _: () = msg_send![app, activateIgnoringOtherApps: true];
        }
    }
}

/// Runs `f` on each of our NSWindows with this exact title; returns whether one was found.
#[cfg(target_os = "macos")]
fn with_window(title: &str, mut f: impl FnMut(*mut objc2::runtime::AnyObject)) -> bool {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    let want = objc2_foundation::NSString::from_str(title);
    let mut found = false;
    unsafe {
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        if app.is_null() {
            return false;
        }
        let windows: *mut AnyObject = msg_send![app, windows];
        if windows.is_null() {
            return false;
        }
        let n: usize = msg_send![windows, count];
        for i in 0..n {
            let w: *mut AnyObject = msg_send![windows, objectAtIndex: i];
            let t: *mut AnyObject = msg_send![w, title];
            if t.is_null() {
                continue;
            }
            let same: bool = msg_send![t, isEqualToString: &*want];
            if same {
                f(w);
                found = true;
            }
        }
    }
    found
}

/// Sets up our window with this title as a HUD (eframe 0.29 has no options for these):
/// * no macOS window shadow: macOS shapes the shadow of a see-through borderless window
///   from everything not fully transparent, so the HUD's own soft shadow got a grey outline;
/// * left out of screenshots (sharing type "none"), so a HUD on screen never ends up in
///   the text you capture.
#[cfg(target_os = "macos")]
pub fn prepare_hud_window(title: &str) {
    use objc2::msg_send;
    with_window(title, |w| unsafe {
        let has: bool = msg_send![w, hasShadow];
        if has {
            let _: () = msg_send![w, setHasShadow: false];
            // NSWindowSharingNone = 0
            let _: () = msg_send![w, setSharingType: 0usize];
        }
    });
}

/// Makes the main window a normal opaque window again. It is created see-through only so
/// that eframe picks a graphics setup with transparency (which the HUD needs); left that
/// way, macOS draws its title bar on a clear background and it shows up black.
/// Returns whether the window was found.
#[cfg(target_os = "macos")]
pub fn make_window_opaque(title: &str) -> bool {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    with_window(title, |w| unsafe {
        let bg: *mut AnyObject = msg_send![class!(NSColor), windowBackgroundColor];
        let _: () = msg_send![w, setBackgroundColor: bg];
        let _: () = msg_send![w, setOpaque: true];
    })
}

#[cfg(not(target_os = "macos"))]
pub fn make_window_opaque(_title: &str) -> bool {
    true
}

#[cfg(not(target_os = "macos"))]
pub fn prepare_hud_window(_title: &str) {}
#[cfg(not(target_os = "macos"))]
pub fn set_dock_icon(_visible: bool) {}
#[cfg(not(target_os = "macos"))]
pub fn activate_app() {}

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceKeyState(state_id: i32, key: u16) -> bool;
    fn CGEventSourceFlagsState(state_id: i32) -> u64;
    fn CGEventSourceButtonState(state_id: i32, button: u32) -> bool;
}

/// Is the left mouse button held right now (anywhere on screen)?
#[cfg(target_os = "macos")]
pub fn left_mouse_down() -> bool {
    // kCGEventSourceStateCombinedSessionState = 0, kCGMouseButtonLeft = 0
    unsafe { CGEventSourceButtonState(0, 0) }
}

#[cfg(not(target_os = "macos"))]
pub fn left_mouse_down() -> bool {
    false
}

/// Watches for ⌘V while `active` is set and raises `pasted`. Polling the key state
/// avoids a key-down event monitor, which would need Accessibility permission.
#[cfg(target_os = "macos")]
pub fn start_paste_watcher(
    active: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pasted: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ctx: eframe::egui::Context,
) -> std::thread::Thread {
    use std::sync::atomic::Ordering;
    const COMBINED_SESSION: i32 = 0;
    const KEY_V: u16 = 9; // kVK_ANSI_V, same physical key on every layout
    const COMMAND: u64 = 0x0010_0000; // kCGEventFlagMaskCommand (bit 20)
    std::thread::spawn(move || {
        let mut was_down = false;
        loop {
            if !active.load(Ordering::Relaxed) {
                was_down = false;
                // Sleep until the app unparks us (when watching starts); no polling.
                std::thread::park_timeout(std::time::Duration::from_secs(60));
                continue;
            }
            let down = unsafe {
                CGEventSourceKeyState(COMBINED_SESSION, KEY_V)
                    && CGEventSourceFlagsState(COMBINED_SESSION) & COMMAND != 0
            };
            if down && !was_down {
                pasted.store(true, Ordering::Relaxed);
                ctx.request_repaint();
            }
            was_down = down;
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
    })
    .thread()
    .clone()
}

#[cfg(not(target_os = "macos"))]
pub fn start_paste_watcher(
    _active: std::sync::Arc<std::sync::atomic::AtomicBool>,
    _pasted: std::sync::Arc<std::sync::atomic::AtomicBool>,
    _ctx: eframe::egui::Context,
) -> std::thread::Thread {
    std::thread::current()
}
