//! Turning recognized lines (text + position) into the copied text.
//! Paragraphs are found from the vertical gaps between lines; hyphenated words are rejoined.

/// One recognized line. Box in Vision's normalized coordinates (0..1, origin bottom-left).
#[derive(Clone, Debug)]
pub struct Line {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Line {
    fn min_y(&self) -> f64 {
        self.y
    }
    fn max_y(&self) -> f64 {
        self.y + self.h
    }
}

/// Paragraph gap: a gap above `PARAGRAPH_GAP` × line height starts a new paragraph.
pub const PARAGRAPH_GAP: f64 = 2.375;
/// A line ending in "." that is narrower than this share of the widest line ends a paragraph.
pub const SHORT_LINE: f64 = 0.8;
/// Marker put where a hyphenated word is rejoined (removed in `clean`).
const CLEAN_AFTER: &str = "$#CLEAN_AFTER$#";

/// Top-to-bottom, then left-to-right.
///
/// Lines are first sorted by their top edge, then grouped into rows in one pass (a line
/// joins the current row when its centre is within half a line height of the row's first
/// line), and each row is sorted left to right. A pairwise "same row" comparator would not
/// be a total order (A~B and B~C don't make A~C), and Rust's sort may panic on that.
pub fn sort_lines(lines: &mut Vec<Line>) {
    let centre = |l: &Line| l.y + l.h / 2.0;
    lines.sort_by(|a, b| b.max_y().total_cmp(&a.max_y()));
    let mut rows: Vec<Vec<Line>> = Vec::new();
    for line in lines.drain(..) {
        match rows.last_mut() {
            Some(row) if (centre(&row[0]) - centre(&line)).abs() < row[0].h.min(line.h) * 0.5 => row.push(line),
            _ => rows.push(vec![line]),
        }
    }
    for mut row in rows {
        row.sort_by(|a, b| a.x.total_cmp(&b.x));
        lines.extend(row);
    }
}

/// Builds both texts: (with line breaks, without line breaks).
pub fn join(lines: &[Line]) -> (String, String) {
    let mut with_breaks = String::new();
    let mut without = String::new();
    let widest = lines.iter().map(|l| l.w).fold(0.0_f64, f64::max);
    for (i, line) in lines.iter().enumerate() {
        let text = line.text.trim();
        if i == 0 {
            with_breaks.push_str(text);
            without.push_str(text);
            continue;
        }
        let prev = &lines[i - 1];
        let height = prev.h.min(line.h);
        let gap = (prev.min_y() - line.max_y()).max(0.0);
        let paragraph = gap > PARAGRAPH_GAP * height;

        // With line breaks: one break per line, a blank line between paragraphs.
        with_breaks.push_str(if paragraph { "\n\n" } else { "\n" });
        with_breaks.push_str(text);

        // Without line breaks: a space, a new line where a paragraph ends,
        // and hyphenated words joined back together.
        let prev_text = prev.text.trim_end();
        let starts_lower = text.chars().next().is_some_and(|c| c.is_lowercase());
        let ends_sentence = prev_text.ends_with('.') && prev.w < SHORT_LINE * widest;
        if prev_text.ends_with('-') && starts_lower && !paragraph {
            without.push_str(CLEAN_AFTER);
        } else if paragraph || ends_sentence {
            without.push('\n');
        } else {
            without.push(' ');
        }
        without.push_str(text);
    }
    (clean(&with_breaks, false), clean(&without, true))
}

/// Both texts from Live Text's transcript, which has no line boxes: (with line breaks,
/// without). Live Text already puts lines that wrap inside a paragraph together; without
/// line breaks, the remaining lines of a block are joined with spaces (a hyphen at a line
/// end before a lowercase start is joined back into one word), and blank lines stay
/// paragraph breaks.
pub fn join_transcript(t: &str) -> (String, String) {
    let lines: Vec<&str> = t.lines().map(|l| l.trim()).collect();
    // With line breaks: as recognized, at most one blank line in a row, no edge blanks.
    let mut keep: Vec<&str> = Vec::new();
    for l in &lines {
        if l.is_empty() && keep.last().map_or(true, |p| p.is_empty()) {
            continue;
        }
        keep.push(l);
    }
    while keep.last().is_some_and(|l| l.is_empty()) {
        keep.pop();
    }
    let with_breaks = keep.join("\n");
    // Without: paragraphs (split at blank lines) on their own line, lines inside joined.
    let mut paragraphs: Vec<String> = Vec::new();
    for block in with_breaks.split("\n\n") {
        let mut p = String::new();
        for l in block.lines() {
            if p.is_empty() {
                p.push_str(l);
            } else if p.ends_with('-') && l.chars().next().is_some_and(|c| c.is_lowercase()) {
                p.pop();
                p.push_str(l);
            } else {
                p.push(' ');
                p.push_str(l);
            }
        }
        if !p.is_empty() {
            paragraphs.push(p);
        }
    }
    (clean(&with_breaks, false), clean(&paragraphs.join("\n"), false))
}

/// Final clean-up: "  " → " ", then the hyphen marker.
fn clean(s: &str, rejoin_hyphens: bool) -> String {
    let mut out = s.to_string();
    while out.contains("  ") {
        out = out.replace("  ", " ");
    }
    if rejoin_hyphens {
        // "exam-" + marker + "ple" → "example"
        out = out.replace(&format!("-{CLEAN_AFTER}"), "");
    }
    out.replace(CLEAN_AFTER, "")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn l(t: &str, y: f64, w: f64) -> Line {
        Line { text: t.into(), x: 0.0, y, w, h: 0.05 }
    }
    #[test]
    fn joins() {
        let mut v = vec![l("Hello wor-", 0.9, 1.0), l("ld again.", 0.84, 0.5), l("New part", 0.5, 1.0)];
        sort_lines(&mut v);
        let (keep, flat) = join(&v);
        assert_eq!(keep, "Hello wor-\nld again.\n\nNew part");
        assert_eq!(flat, "Hello world again.\nNew part");
    }

    /// Tables and skewed columns used to make the sort comparator inconsistent (panic).
    #[test]
    fn sorts_tables_without_panic() {
        let mut seed = 7u64;
        let mut rnd = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) as f64 / (1u64 << 31) as f64
        };
        for _ in 0..300 {
            let mut v = Vec::new();
            for r in 0..8 {
                for c in 0..4 {
                    let h = 0.02 + rnd() * 0.02;
                    v.push(Line { text: format!("{r}{c}"), x: c as f64 * 0.25, y: 0.9 - r as f64 * 0.03 + rnd() * 0.012, w: 0.2, h });
                }
            }
            sort_lines(&mut v);
            assert_eq!(v.len(), 32);
        }
    }

    #[test]
    fn joins_live_text() {
        let t = "Title\n\n\nFirst line of a para-\ngraph goes on\nand ends.\n\nNext   block\n";
        let (keep, flat) = join_transcript(t);
        assert_eq!(keep, "Title\n\nFirst line of a para-\ngraph goes on\nand ends.\n\nNext block");
        assert_eq!(flat, "Title\nFirst line of a paragraph goes on and ends.\nNext block");
        assert_eq!(join_transcript(""), (String::new(), String::new()));
        assert_eq!(join_transcript("\n \n"), (String::new(), String::new()));
    }

    #[test]
    fn keeps_reading_order_in_a_row() {
        let mut v = vec![
            Line { text: "b".into(), x: 0.5, y: 0.5, w: 0.1, h: 0.05 },
            Line { text: "a".into(), x: 0.1, y: 0.505, w: 0.1, h: 0.05 },
            Line { text: "c".into(), x: 0.1, y: 0.3, w: 0.1, h: 0.05 },
        ];
        sort_lines(&mut v);
        let order: Vec<&str> = v.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(order, ["a", "b", "c"]);
    }
}
