//! Reading text and QR/bar codes from an image file with Apple Vision.
//! Uses VNRecognizeTextRequest (revision 3 where available) and VNDetectBarcodesRequest.

pub use crate::layout::Line;

pub struct RecognizeOptions {
    pub language: String,
    pub auto_detect_language: bool,
    pub custom_words: String,
}

/// Custom words: spaces removed, then split on ",".
pub fn parse_custom_words(raw: &str) -> Vec<String> {
    let no_spaces: String = raw.chars().filter(|c| *c != ' ').collect();
    no_spaces
        .split(',')
        .filter(|w| !w.is_empty())
        .map(|w| w.to_string())
        .collect()
}

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use objc2::rc::Retained;
    use objc2::ClassType;
    use objc2_foundation::{NSArray, NSDictionary, NSProcessInfo, NSString, NSURL};
    use objc2_vision::{
        VNDetectBarcodesRequest, VNImageRequestHandler, VNRecognizeTextRequest, VNRequest,
        VNRequestTextRecognitionLevel,
    };

    fn handler(path: &str) -> Retained<VNImageRequestHandler> {
        unsafe {
            let url = NSURL::fileURLWithPath(&NSString::from_str(path));
            let opts = NSDictionary::new();
            VNImageRequestHandler::initWithURL_options(VNImageRequestHandler::alloc(), &url, &opts)
        }
    }

    /// Returns each recognized line with its box (the box drives line order and paragraphs).
    /// Runs inside an autorelease pool so Vision's objects are freed right after, not
    /// whenever the thread ends.
    pub fn recognize_lines(path: &str, o: &RecognizeOptions) -> Result<Vec<Line>, String> {
        objc2::rc::autoreleasepool(|_| recognize_lines_in_pool(path, o))
    }

    fn recognize_lines_in_pool(path: &str, o: &RecognizeOptions) -> Result<Vec<Line>, String> {
        unsafe {
            let req = VNRecognizeTextRequest::new();
            // Request setup:
            // revision 3 on macOS 13+, else 2; the default revision if that one isn't supported.
            let v = NSProcessInfo::processInfo().operatingSystemVersion();
            let macos13 = v.majorVersion >= 13;
            let wanted = if macos13 { 3 } else { 2 };
            let cls = VNRecognizeTextRequest::class();
            let supported: Retained<objc2_foundation::NSIndexSet> = objc2::msg_send_id![cls, supportedRevisions];
            let revision: usize = if supported.containsIndex(wanted) {
                wanted
            } else {
                objc2::msg_send![cls, defaultRevision]
            };
            req.setRevision(revision);
            req.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
            // Automatic language detection only on macOS 13+ and only when switched on.
            if macos13 && o.auto_detect_language {
                req.setAutomaticallyDetectsLanguage(true);
            }
            // Custom words: spaces removed, split on ","; language correction turned on with them.
            let words: Vec<Retained<NSString>> = parse_custom_words(&o.custom_words)
                .iter()
                .map(|w| NSString::from_str(w))
                .collect();
            if !words.is_empty() {
                req.setCustomWords(&NSArray::from_vec(words));
                req.setUsesLanguageCorrection(true);
            }
            // Recognition languages are set when the revision is 2 or higher.
            if req.revision() >= 2 {
                let lang = NSString::from_str(&o.language);
                req.setRecognitionLanguages(&NSArray::from_vec(vec![lang]));
            }
            let h = handler(path);
            let as_req: Retained<VNRequest> = Retained::cast(req.clone());
            h.performRequests_error(&NSArray::from_vec(vec![as_req]))
                .map_err(|e| e.localizedDescription().to_string())?;
            let mut out = Vec::new();
            if let Some(results) = req.results() {
                for obs in results.iter() {
                    let cands = obs.topCandidates(1);
                    if let Some(c) = cands.firstObject() {
                        let b = obs.boundingBox();
                        out.push(Line {
                            text: c.string().to_string(),
                            x: b.origin.x,
                            y: b.origin.y,
                            w: b.size.width,
                            h: b.size.height,
                        });
                    }
                }
            }
            Ok(out)
        }
    }

    pub fn read_codes(path: &str) -> Result<Vec<String>, String> {
        objc2::rc::autoreleasepool(|_| read_codes_in_pool(path))
    }

    fn read_codes_in_pool(path: &str) -> Result<Vec<String>, String> {
        unsafe {
            let req = VNDetectBarcodesRequest::new();
            let h = handler(path);
            let as_req: Retained<VNRequest> = Retained::cast(req.clone());
            h.performRequests_error(&NSArray::from_vec(vec![as_req]))
                .map_err(|e| e.localizedDescription().to_string())?;
            let mut out = Vec::new();
            if let Some(results) = req.results() {
                for obs in results.iter() {
                    if let Some(s) = obs.payloadStringValue() {
                        let s = s.to_string();
                        if !out.contains(&s) {
                            out.push(s);
                        }
                    }
                }
            }
            Ok(out)
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;
    pub fn recognize_lines(_p: &str, _o: &RecognizeOptions) -> Result<Vec<Line>, String> {
        Err("Text recognition needs macOS".into())
    }
    pub fn read_codes(_p: &str) -> Result<Vec<String>, String> {
        Err("Code reading needs macOS".into())
    }
}

pub use imp::{read_codes, recognize_lines};

// ---------- recognition in a short-lived helper process ----------
//
// Vision loads its models into the process on first use and keeps them (plus Neural
// Engine buffers) until the process ends: 40–100 MB that never comes back. So the app
// runs each recognition in a copy of itself started with a hidden flag; when it exits,
// macOS takes all of that memory back. If the helper cannot run, recognition happens
// in-process as before.

/// Hidden command-line flags for the helper.
pub const HELPER_LINES: &str = "--ocr-lines";
pub const HELPER_CODES: &str = "--ocr-codes";

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\n', "\\n").replace('\t', "\\t")
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Helper side: run the request and print one result per line; returns the exit code.
pub fn helper_main(args: &[String]) -> i32 {
    let flag = args.get(1).map(String::as_str).unwrap_or("");
    let Some(path) = args.get(2) else { return 2 };
    if flag == HELPER_LINES {
        let opts = RecognizeOptions {
            language: args.get(3).cloned().unwrap_or_else(|| "en-US".into()),
            auto_detect_language: args.get(4).map(|v| v == "1").unwrap_or(false),
            custom_words: args.get(5).cloned().unwrap_or_default(),
        };
        match recognize_lines(path, &opts) {
            Ok(lines) => {
                for l in lines {
                    println!("{} {} {} {}\t{}", l.x, l.y, l.w, l.h, escape(&l.text));
                }
                0
            }
            Err(e) => {
                eprintln!("{e}");
                1
            }
        }
    } else {
        match read_codes(path) {
            Ok(codes) => {
                for c in codes {
                    println!("{}", escape(&c));
                }
                0
            }
            Err(e) => {
                eprintln!("{e}");
                1
            }
        }
    }
}

// ---------- keeping Vision prepared ----------
//
// Before Vision can recognize text for an app, macOS prepares its models for that app
// (cached in ~/Library/Caches/<bundle id>/com.apple.e5rt.e5bundlecache): 30–60 seconds of
// work, whichever compute device is used. It happens after install and every update, and
// macOS also throws the prepared models away again after some hours. A capture that ran
// into it used to sit there and was cut off at 30 s, which threw that work away too.
// So: the app prepares Vision in the background right after launch and again every
// WARM_EVERY (cheap when nothing needs preparing), never cuts a recognition short while
// it may be preparing, and the HUD says so when a recognition takes long.

/// How often the background warm-up runs (keeps the prepared models from going stale).
const WARM_EVERY: std::time::Duration = std::time::Duration::from_secs(45 * 60);

static READY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// One helper at a time: a capture that starts while the warm-up is still running waits
/// for it (then takes a fraction of a second) instead of compiling the models again.
static HELPER_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Is text recognition prepared for this build (fast from now on)?
pub fn is_ready() -> bool {
    READY.load(std::sync::atomic::Ordering::Relaxed)
}

/// Identifies this build: size and modification time of the executable.
fn build_id() -> Option<String> {
    let m = std::fs::metadata(std::env::current_exe().ok()?).ok()?;
    let t = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    Some(format!("{} {}", m.len(), t))
}

fn ready_file() -> Option<std::path::PathBuf> {
    eframe::storage_dir("SnipReader").map(|d| d.join("ocr-ready.txt"))
}

fn mark_ready() {
    READY.store(true, std::sync::atomic::Ordering::Relaxed);
    if let (Some(f), Some(id)) = (ready_file(), build_id()) {
        if let Some(dir) = f.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(f, id);
    }
}

/// Reads the "ready" mark; if this build isn't prepared yet, prepares it on a background
/// thread. Returns at once.
pub fn warm_up(o: &RecognizeOptions) {
    remember_language(o);
    if live_text_possible() {
        READY.store(true, std::sync::atomic::Ordering::Relaxed);
        keep_live_text_ready();
        return;
    }
    let ready = matches!((ready_file().and_then(|f| std::fs::read_to_string(f).ok()), build_id()), (Some(a), Some(b)) if a == b);
    if ready {
        READY.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    std::thread::spawn(|| {
        // Low priority: this must never compete with what you are doing.
        crate::system::low_priority_thread();
        loop {
            warm_once();
            std::thread::sleep(WARM_EVERY);
        }
    });
}

/// Recognizes a blank image (enough to make macOS prepare everything) and logs how long it
/// took: a fraction of a second when Vision is prepared, ~a minute when it had to redo it.
fn warm_once() {
    let started = std::time::Instant::now();
    crate::app::trace("warm-up start");
    let path = std::env::temp_dir().join("snipreader").join("warm-up.png");
    let _ = std::fs::create_dir_all(path.parent().unwrap_or(std::path::Path::new("/tmp")));
    let blank = image::RgbaImage::from_pixel(64, 64, image::Rgba([255, 255, 255, 255]));
    if blank.save(&path).is_err() {
        return;
    }
    let p = path.to_string_lossy().to_string();
    let ok = matches!(run_helper(&[HELPER_LINES, &p, "en-US", "0", ""]), Some(Ok(_)));
    let _ = std::fs::remove_file(&path);
    if ok {
        mark_ready();
    }
    let secs = started.elapsed().as_secs_f32();
    crate::app::trace(&format!("warm-up {} in {secs:.1}s", if ok { "done" } else { "failed" }));
}

/// How long one recognition may take before the helper is stopped. Long, because macOS may
/// be preparing Vision (up to about a minute, at any time); stopping it would only make the
/// next capture start that work over again.
fn helper_timeout() -> std::time::Duration {
    std::time::Duration::from_secs(180)
}

/// Runs the helper; `Some(Ok(stdout))` on success, `Some(Err(..))` when it ran but failed,
/// crashed or hung, `None` only when it could not be started at all (then the caller
/// recognizes in-process). A crash is not retried in-process: whatever crashed the helper
/// (a broken image, say) would take the app down with it.
fn run_helper(args: &[&str]) -> Option<Result<String, String>> {
    let _one_at_a_time = HELPER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // Decided once, at the start: long while this build may still be preparing.
    let limit = helper_timeout();
    let exe = std::env::current_exe().ok()?;
    let run = run_with_limit(&exe, args, limit)?;
    match run.code {
        Some(0) => {
            mark_ready_once();
            Some(Ok(run.stdout))
        }
        Some(1) => Some(Err(run.stderr)),
        _ if run.timed_out => Some(Err("Text recognition took too long and was stopped".into())),
        _ => Some(Err("Text recognition stopped unexpectedly".into())),
    }
}

struct Run {
    code: Option<i32>,
    timed_out: bool,
    stdout: String,
    stderr: String,
}

/// Runs a program with a time limit, reading its output on separate threads (a large
/// result can't block it). `None` only when it could not be started.
fn run_with_limit(program: &std::path::Path, args: &[&str], limit: std::time::Duration) -> Option<Run> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let mut child = Command::new(program).args(args).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().ok()?;
    let mut out_pipe = child.stdout.take()?;
    let mut err_pipe = child.stderr.take()?;
    let out_t = std::thread::spawn(move || {
        let mut s = Vec::new();
        let _ = out_pipe.read_to_end(&mut s);
        s
    });
    let err_t = std::thread::spawn(move || {
        let mut s = Vec::new();
        let _ = err_pipe.read_to_end(&mut s);
        s
    });
    let started = std::time::Instant::now();
    let mut timed_out = false;
    let code = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st.code(),
            Ok(None) if started.elapsed() > limit => {
                timed_out = true;
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(10)),
            Err(_) => break None,
        }
    };
    Some(Run {
        code,
        timed_out,
        stdout: String::from_utf8_lossy(&out_t.join().unwrap_or_default()).into_owned(),
        stderr: String::from_utf8_lossy(&err_t.join().unwrap_or_default()).trim().to_string(),
    })
}

// ---------- Live Text (VisionKit), the first choice ----------
//
// `snipreader-ocr` (packaging/ocr-helper, Swift) reads text with VisionKit's ImageAnalyzer,
// which uses the models macOS keeps ready for Live Text: well under a second even right
// after a long break, where plain Vision may first spend a minute preparing (see above).
// It gives the text, not line boxes, so line joining works on the text itself.

/// What recognition produced: lines with their boxes (Vision) or finished text (Live Text).
pub enum Recognized {
    Lines(Vec<Line>),
    Transcript(String),
}

static LIVE_TEXT_MISSING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn live_text_helper() -> Option<std::path::PathBuf> {
    let p = std::env::current_exe().ok()?.with_file_name("snipreader-ocr");
    p.exists().then_some(p)
}

/// Can Live Text be used here (helper present, macOS 13 or later)?
pub fn live_text_possible() -> bool {
    !LIVE_TEXT_MISSING.load(std::sync::atomic::Ordering::Relaxed) && live_text_helper().is_some() && macos_major() >= 13
}

#[cfg(target_os = "macos")]
fn macos_major() -> isize {
    objc2_foundation::NSProcessInfo::processInfo().operatingSystemVersion().majorVersion
}
#[cfg(not(target_os = "macos"))]
fn macos_major() -> isize {
    0
}

/// Language settings of the last capture, so the warm-up prepares the same model.
static LANGUAGE: std::sync::Mutex<(String, bool)> = std::sync::Mutex::new((String::new(), false));

fn remember_language(o: &RecognizeOptions) {
    *LANGUAGE.lock().unwrap_or_else(|e| e.into_inner()) = (o.language.clone(), o.auto_detect_language);
}

/// macOS drops the compiled Live Text model when nothing has used it for a while (seen after
/// a night's sleep); the next request then rebuilds it, ~30 s. Read a sample text after the
/// Mac wakes and every LIVE_TEXT_EVERY, so that happens here and not during a capture.
/// It costs well under a second when the model is ready.
const LIVE_TEXT_EVERY: std::time::Duration = std::time::Duration::from_secs(30 * 60);

fn keep_live_text_ready() {
    std::thread::spawn(|| {
        use std::time::{Duration, SystemTime};
        // The wall clock keeps running while the Mac sleeps, the thread's sleep does not:
        // a check that comes far later than planned means the Mac just woke up.
        const TICK: Duration = Duration::from_secs(60);
        let mut last_warm: Option<SystemTime> = None;
        let mut last_tick = SystemTime::now();
        loop {
            let now = SystemTime::now();
            let woke = now.duration_since(last_tick).unwrap_or_default() > TICK * 3;
            let due = last_warm.map_or(true, |t| now.duration_since(t).map_or(true, |d| d >= LIVE_TEXT_EVERY));
            if woke || due {
                if woke {
                    // Give the Mac a moment after waking before adding work.
                    std::thread::sleep(Duration::from_secs(5));
                }
                warm_live_text();
                last_warm = Some(SystemTime::now());
            }
            last_tick = SystemTime::now();
            std::thread::sleep(TICK);
        }
    });
}

fn warm_live_text() {
    let Some(helper) = live_text_helper() else { return };
    let (language, auto) = LANGUAGE.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let auto = if auto { "1" } else { "0" };
    let started = std::time::Instant::now();
    crate::app::trace("live text warm-up start");
    let run = run_with_limit(&helper, &["--warm", &language, auto], helper_timeout());
    let ok = matches!(run, Some(Run { code: Some(0), .. }));
    crate::app::trace(&format!("live text warm-up {} in {:.1}s", if ok { "done" } else { "failed" }, started.elapsed().as_secs_f32()));
}

/// Live Text first; Vision (in its helper) if Live Text is unavailable or fails.
pub fn recognize_text(path: &str, o: &RecognizeOptions) -> Result<Recognized, String> {
    remember_language(o);
    if live_text_possible() {
        if let Some(helper) = live_text_helper() {
            let auto = if o.auto_detect_language { "1" } else { "0" };
            // Not cut short: a slow Live Text is macOS rebuilding its model (~30 s), and Vision
            // would then be no faster; stopping it would only throw that work away.
            match run_with_limit(&helper, &[path, &o.language, auto], helper_timeout()) {
                Some(Run { code: Some(0), stdout, .. }) => return Ok(Recognized::Transcript(stdout)),
                Some(Run { code: Some(3), .. }) => LIVE_TEXT_MISSING.store(true, std::sync::atomic::Ordering::Relaxed),
                Some(r) => crate::app::trace(&format!("live text failed ({:?}): {}; using Vision", r.code, r.stderr)),
                None => crate::app::trace("live text helper did not start; using Vision"),
            }
        }
    }
    recognize_lines_isolated(path, o).map(Recognized::Lines)
}

/// A successful recognition also means this build is prepared.
fn mark_ready_once() {
    if !is_ready() {
        mark_ready();
    }
}

/// Runs the helper, once more if Vision itself reported an error (a rare one-off image
/// reader error was seen on a first run); a timeout or crash is not retried.
fn run_helper_retry(args: &[&str]) -> Option<Result<String, String>> {
    match run_helper(args) {
        Some(Err(e)) if !e.contains("too long") && !e.contains("unexpectedly") => run_helper(args).or(Some(Err(e))),
        other => other,
    }
}

/// Like `recognize_lines`, but in the helper process (see above).
pub fn recognize_lines_isolated(path: &str, o: &RecognizeOptions) -> Result<Vec<Line>, String> {
    let auto = if o.auto_detect_language { "1" } else { "0" };
    match run_helper_retry(&[HELPER_LINES, path, &o.language, auto, &o.custom_words]) {
        Some(Ok(text)) => Ok(text
            .lines()
            .filter_map(|row| {
                let (nums, t) = row.split_once('\t')?;
                let v: Vec<f64> = nums.split(' ').filter_map(|n| n.parse().ok()).collect();
                (v.len() == 4).then(|| Line { text: unescape(t), x: v[0], y: v[1], w: v[2], h: v[3] })
            })
            .collect()),
        Some(Err(e)) => Err(e),
        None => recognize_lines(path, o),
    }
}

/// Like `read_codes`, but in the helper process (see above).
pub fn read_codes_isolated(path: &str) -> Result<Vec<String>, String> {
    match run_helper_retry(&[HELPER_CODES, path]) {
        Some(Ok(text)) => Ok(text.lines().map(unescape).collect()),
        Some(Err(e)) => Err(e),
        None => read_codes(path),
    }
}
