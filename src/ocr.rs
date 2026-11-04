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

/// How long one recognition may take before the helper is stopped.
const HELPER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Runs the helper; `Some(Ok(stdout))` on success, `Some(Err(..))` when it ran but failed,
/// crashed or hung, `None` only when it could not be started at all (then the caller
/// recognizes in-process). A crash is not retried in-process: whatever crashed the helper
/// (a broken image, say) would take the app down with it.
fn run_helper(args: &[&str]) -> Option<Result<String, String>> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let exe = std::env::current_exe().ok()?;
    let mut child = Command::new(exe)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    // Read the pipes on their own threads so a large result can't block the helper.
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
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) if started.elapsed() > HELPER_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Some(Err("Text recognition took too long and was stopped".into()));
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(10)),
            Err(e) => return Some(Err(e.to_string())),
        }
    };
    let out = out_t.join().unwrap_or_default();
    let err = err_t.join().unwrap_or_default();
    if status.success() {
        Some(Ok(String::from_utf8_lossy(&out).into_owned()))
    } else if status.code() == Some(1) {
        Some(Err(String::from_utf8_lossy(&err).trim().to_string()))
    } else {
        Some(Err("Text recognition stopped unexpectedly".into()))
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
