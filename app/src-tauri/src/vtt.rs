//! WebVTT captions -> word-level Transcript.

use crate::transcript::{Transcript, Word};

/// Cues with inline word times (YouTube auto captions) keep those times and only their timed lines count,
/// because the other lines are the previous line rolled up again. Other cues spread their words evenly.
/// shortcut: no cue settings/regions, STYLE or NOTE handling beyond skipping non-cue blocks.
pub fn parse(vtt: &str, language: &str) -> Transcript {
    let cues = cues(vtt);
    let inline = cues.iter().any(|c| c.lines.iter().any(|l| segments(l, 0.0).len() > 1));
    let mut words = Vec::new();
    for cue in &cues {
        if inline {
            for line in &cue.lines {
                let segs = segments(line, cue.start);
                if segs.len() > 1 {
                    for (i, (s, text)) in segs.iter().enumerate() {
                        push_spread(&mut words, text, *s, segs.get(i + 1).map_or(cue.end, |n| n.0));
                    }
                }
            }
        } else {
            let text = cue.lines.iter().map(|l| segments(l, 0.0).into_iter().map(|s| s.1).collect::<String>()).collect::<Vec<_>>().join(" ");
            push_spread(&mut words, &text, cue.start, cue.end);
        }
    }
    Transcript { language: language.into(), words }
}

struct Cue {
    start: f64,
    end: f64,
    lines: Vec<String>,
}

fn cues(vtt: &str) -> Vec<Cue> {
    let mut out: Vec<Cue> = Vec::new();
    let mut in_cue = false;
    for line in vtt.lines() {
        if let Some((a, b)) = line.split_once("-->") {
            let end = b.split_whitespace().next().and_then(time);
            if let (Some(start), Some(end)) = (time(a.trim()), end) {
                out.push(Cue { start, end, lines: Vec::new() });
                in_cue = true;
                continue;
            }
        }
        if line.trim().is_empty() {
            in_cue = false;
        } else if in_cue {
            out.last_mut().unwrap().lines.push(line.to_string());
        }
    }
    out
}

/// `hh:mm:ss.mmm` or `mm:ss.mmm` in seconds.
fn time(s: &str) -> Option<f64> {
    s.split(':').try_fold(0.0, |acc, p| p.parse::<f64>().ok().map(|v| acc * 60.0 + v)).filter(|_| s.matches(':').count() >= 1)
}

/// Splits a cue line at its inline `<hh:mm:ss.mmm>` tags into (start, text) pieces; other tags are dropped.
/// The first piece starts at `first_start`.
fn segments(line: &str, first_start: f64) -> Vec<(f64, String)> {
    let mut out = vec![(first_start, String::new())];
    let mut rest = line;
    while let Some(i) = rest.find('<') {
        out.last_mut().unwrap().1.push_str(&rest[..i]);
        let Some(j) = rest[i..].find('>') else { rest = ""; break };
        if let Some(t) = time(&rest[i + 1..i + j]) {
            out.push((t, String::new()));
        }
        rest = &rest[i + j + 1..];
    }
    out.last_mut().unwrap().1.push_str(rest);
    out
}

fn push_spread(words: &mut Vec<Word>, text: &str, start: f64, end: f64) {
    let text = text.replace("&nbsp;", " ").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&");
    let parts: Vec<&str> = text.split_whitespace().collect();
    let step = (end - start).max(0.0) / parts.len().max(1) as f64;
    for (i, p) in parts.iter().enumerate() {
        words.push(Word { text: p.to_string(), start: start + step * i as f64, end: start + step * (i + 1) as f64 });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (text, start, end) rounded to milliseconds.
    fn words(t: &Transcript) -> Vec<(&str, f64, f64)> {
        let ms = |s: f64| (s * 1000.0).round() / 1000.0;
        t.words.iter().map(|w| (w.text.as_str(), ms(w.start), ms(w.end))).collect()
    }

    #[test]
    fn plain_cues_spread_their_words_evenly_over_the_cue() {
        let vtt = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:03.000\nHello there, <i>friends</i>.\nHow are you?\n\n00:01:00.000 --> 00:01:01.000\nFine &amp; well\n";
        let t = parse(vtt, "en");
        assert_eq!(t.language, "en");
        assert_eq!(
            words(&t),
            [
                ("Hello", 1.0, 1.333),
                ("there,", 1.333, 1.667),
                ("friends.", 1.667, 2.0),
                ("How", 2.0, 2.333),
                ("are", 2.333, 2.667),
                ("you?", 2.667, 3.0),
                ("Fine", 60.0, 60.333),
                ("&", 60.333, 60.667),
                ("well", 60.667, 61.0),
            ]
        );
    }

    #[test]
    fn youtube_auto_captions_use_inline_word_times_and_skip_rolled_up_repeats() {
        let vtt = "WEBVTT\nKind: captions\nLanguage: en\n\n\
00:00:00.160 --> 00:00:02.230 align:start position:0%\nhello<00:00:00.640><c> everyone</c><00:00:01.040><c> welcome</c>\n\n\
00:00:02.230 --> 00:00:02.240 align:start position:0%\nhello everyone welcome\n \n\n\
00:00:02.240 --> 00:00:04.000 align:start position:0%\nhello everyone welcome\nback<00:00:02.800><c> again</c>\n";
        let t = parse(vtt, "en");
        assert_eq!(
            words(&t),
            [("hello", 0.16, 0.64), ("everyone", 0.64, 1.04), ("welcome", 1.04, 2.23), ("back", 2.24, 2.8), ("again", 2.8, 4.0)]
        );
    }

    #[test]
    fn no_cues_gives_no_words() {
        assert!(parse("WEBVTT\n\nNOTE nothing here\n", "en").words.is_empty());
        assert!(parse("", "en").words.is_empty());
    }
}
