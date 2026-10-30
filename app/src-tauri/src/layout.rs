//! Layout engine, geometry half: preset rules (shared/layout/layouts.json) + face boxes -> panes -> ffmpeg graph.
//! Pure functions, no I/O. Face detection is `faces.rs`; persistence and the Engine glue are `layout_store.rs`.

use serde::{Deserialize, Serialize};

const RULES: &str = include_str!("../../../shared/layout/layouts.json");

/// Rectangle in Source-relative units (0..1 of width/height).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// One region of the 9:16 canvas: `dest` = [x, y, w, h] in canvas pixels, filled with `crop` of the Source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pane {
    pub dest: [u32; 4],
    pub crop: Rect,
}

/// Active-speaker follow: from clip time `t` (seconds) the Focus crop is centred on `cx` (Source-relative).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Key {
    pub t: f64,
    pub cx: f64,
}

/// Seconds a move to another speaker takes (eased, so the crop never jumps).
pub const FOLLOW_SECS: f64 = 0.8;

/// A Clip's Layout. `face` is kept so changing preset never needs another detection pass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub preset: String,
    /// Faces found in the Clip (the largest-count sample); 0 = none.
    pub faces: u32,
    pub face: Option<Rect>,
    pub panes: Vec<Pane>,
    /// Machine code the UI turns into a message: "no_face" | "multiple_faces".
    pub notice: Option<String>,
    pub src_w: u32,
    pub src_h: u32,
    /// Speaker keys for the Focus pane (see `Key`); empty = a fixed crop. Kept when the preset changes.
    #[serde(default)]
    pub follow: Vec<Key>,
    /// Manual correction of the followed crop, Source-relative, added to every key.
    #[serde(default)]
    pub follow_dx: f64,
}

#[derive(Deserialize)]
struct Rules {
    canvas: Canvas,
    default_preset: String,
    no_face_preset: String,
    multi_face_preset: String,
    webcam_face_widths: f64,
    presets: Vec<Preset>,
}
#[derive(Deserialize)]
struct Canvas {
    w: u32,
    h: u32,
}
#[derive(Deserialize)]
struct Preset {
    name: String,
    panes: Vec<PaneRule>,
}
#[derive(Deserialize)]
struct PaneRule {
    dest: [u32; 4],
    source: String,
}

fn rules() -> &'static Rules {
    static R: std::sync::OnceLock<Rules> = std::sync::OnceLock::new();
    R.get_or_init(|| serde_json::from_str(RULES).expect("shared/layout/layouts.json is valid"))
}

pub fn preset_names() -> Vec<String> {
    rules().presets.iter().map(|p| p.name.clone()).collect()
}

fn even(v: f64) -> u32 {
    ((v / 2.0).round() as u32 * 2).max(2)
}

/// Crop (Source-relative) and final dest for one pane rule.
fn resolve(rule: &PaneRule, face: Option<Rect>, sw: u32, sh: u32) -> Pane {
    let r = rules();
    let (sw, sh) = (sw as f64, sh as f64);
    if rule.source == "fit" {
        let (cw, ch) = (r.canvas.w as f64, r.canvas.h as f64);
        let (mut w, mut h) = (cw, even(cw * sh / sw) as f64);
        if h > ch {
            (w, h) = (even(ch * sw / sh) as f64, ch);
        }
        let dest = [((cw - w) / 2.0) as u32, ((ch - h) / 2.0) as u32, w as u32, h as u32];
        return Pane { dest, crop: Rect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 } };
    }
    let a = rule.dest[2] as f64 / rule.dest[3] as f64; // dest aspect
    // Largest window of dest aspect that fits the frame, in Source-relative units.
    let (mut w, mut h) = if a <= sw / sh { (a * sh / sw, 1.0) } else { (1.0, sw / (a * sh)) };
    if let (Some(f), "webcam") = (face, rule.source.as_str()) {
        let (ww, wh) = (r.webcam_face_widths * f.w, r.webcam_face_widths * f.w * sw / (a * sh));
        let k = 1.0f64.min(w / ww).min(h / wh);
        (w, h) = (ww * k, wh * k);
    }
    let (cx, cy) = face.filter(|_| rule.source != "center").map_or((0.5, 0.5), |f| (f.x + f.w / 2.0, f.y + f.h / 2.0));
    Pane { dest: rule.dest, crop: Rect { x: (cx - w / 2.0).clamp(0.0, 1.0 - w), y: (cy - h / 2.0).clamp(0.0, 1.0 - h), w, h } }
}

fn build(preset: &str, faces: u32, face: Option<Rect>, notice: Option<&str>, sw: u32, sh: u32) -> Result<Layout, String> {
    let p = rules().presets.iter().find(|p| p.name == preset).ok_or_else(|| format!("Unknown layout: {preset}"))?;
    Ok(Layout {
        preset: preset.to_string(),
        faces,
        face,
        panes: p.panes.iter().map(|r| resolve(r, face, sw, sh)).collect(),
        notice: notice.map(String::from),
        src_w: sw,
        src_h: sh,
        follow: Vec::new(),
        follow_dx: 0.0,
    })
}

impl Layout {
    /// Focus that follows the speaker through `follow` (from `speaker::follow_keys`); no keys = plain `auto`.
    pub fn auto_following(faces: &[Rect], follow: Vec<Key>, src_w: u32, src_h: u32) -> Layout {
        if follow.is_empty() {
            return Self::auto(faces, src_w, src_h);
        }
        // shortcut: the vertical position comes from the largest face; people in one shot sit at about the same height.
        let biggest = faces.iter().copied().max_by(|a, b| (a.w * a.h).total_cmp(&(b.w * b.h)));
        let mut l = build("focus", faces.len() as u32, biggest, Some("follow_speaker"), src_w, src_h).expect("rules name existing presets");
        l.follow = follow;
        l.panes[0].crop.x = l.crop_x(0.0);
        l
    }

    /// Only the Focus pane follows the speaker; every other preset (the manual split ones) stays fixed.
    fn following(&self) -> bool {
        self.preset == "focus" && !self.follow.is_empty()
    }

    /// Face centre the Focus crop is on `t` seconds into the Clip: each key eases in over FOLLOW_SECS.
    fn cx_at(&self, t: f64) -> f64 {
        let ease = |u: f64| u * u * (3.0 - 2.0 * u);
        self.follow[0].cx
            + self.follow.windows(2).map(|k| (k[1].cx - k[0].cx) * ease(((t - k[1].t) / FOLLOW_SECS).clamp(0.0, 1.0))).sum::<f64>()
    }

    fn crop_x(&self, t: f64) -> f64 {
        let w = self.panes[0].crop.w;
        (self.cx_at(t) - w / 2.0 + self.follow_dx).clamp(0.0, 1.0 - w)
    }

    /// ffmpeg `crop` x expression (pixels, quoted) for the followed crop; mirrors `crop_x`.
    fn x_expr(&self, w: u32) -> String {
        let sw = self.src_w as f64;
        let ease = |t: f64| {
            let u = format!("clip((t-{t:.3})/{FOLLOW_SECS},0,1)");
            format!("{u}*{u}*(3-2*{u})")
        };
        let mut e = format!("{:.5}", self.follow[0].cx);
        for k in self.follow.windows(2) {
            e += &format!("+({:.5})*({})", k[1].cx - k[0].cx, ease(k[1].t));
        }
        format!("'clip(({e})*{sw}-{}+{:.2},0,{})'", w as f64 / 2.0, self.follow_dx * sw, self.src_w - w)
    }

    /// The Layout as it looks `t` seconds into the Clip (only the followed crop moves).
    pub fn at(&self, t: f64) -> Layout {
        let mut l = self.clone();
        if self.following() {
            l.panes[0].crop.x = self.crop_x(t);
        }
        l
    }

    /// Initial Layout from the faces detected in a Clip.
    pub fn auto(faces: &[Rect], src_w: u32, src_h: u32) -> Layout {
        let r = rules();
        let biggest = faces.iter().copied().max_by(|a, b| (a.w * a.h).total_cmp(&(b.w * b.h)));
        let (preset, notice) = match faces.len() {
            0 => (&r.no_face_preset, Some("no_face")),
            1 => (&r.default_preset, None),
            _ => (&r.multi_face_preset, Some("multiple_faces")),
        };
        build(preset, faces.len() as u32, biggest, notice, src_w, src_h).expect("rules name existing presets")
    }

    /// Same Source and face, another preset. Manual crop shifts are reset and the notice is cleared.
    pub fn with_preset(&self, preset: &str) -> Result<Layout, String> {
        let mut l = build(preset, self.faces, self.face, None, self.src_w, self.src_h)?;
        l.follow = self.follow.clone(); // kept so coming back to Focus follows the speaker again
        if l.following() {
            l.panes[0].crop.x = l.crop_x(0.0);
        }
        Ok(l)
    }

    /// Moves pane `idx`'s crop by (dx, dy) Source-relative units, keeping it inside the frame.
    pub fn shift_pane(&mut self, idx: usize, dx: f64, dy: f64) -> Result<(), String> {
        let follow = idx == 0 && self.following();
        let preset = &self.preset;
        let c = &mut self.panes.get_mut(idx).ok_or_else(|| format!("No pane {idx} in layout {preset}"))?.crop;
        if !follow {
            c.x = (c.x + dx).clamp(0.0, 1.0 - c.w);
        }
        c.y = (c.y + dy).clamp(0.0, 1.0 - c.h);
        if follow {
            // A followed crop has no fixed x: the correction offsets the whole path.
            self.follow_dx = (self.follow_dx + dx).clamp(-1.0, 1.0);
            self.panes[0].crop.x = self.crop_x(0.0);
        }
        Ok(())
    }

    /// ffmpeg graph (one input, one unlabeled 1080x1920 output) for `-vf`; the exporter appends fps/captions.
    pub fn filter(&self, fps: u32) -> String {
        let (sw, sh) = (self.src_w as f64, self.src_h as f64);
        let (cw, ch) = (rules().canvas.w, rules().canvas.h);
        let part = |p: &Pane| {
            let (w, h) = (even(p.crop.w * sw).min(self.src_w & !1), even(p.crop.h * sh).min(self.src_h & !1));
            let x = even(p.crop.x * sw).min(self.src_w - w);
            let y = even(p.crop.y * sh).min(self.src_h - h);
            let x = if self.following() { self.x_expr(w) } else { x.to_string() };
            format!("crop={w}:{h}:{x}:{y},scale={}:{}:flags=lanczos,setsar=1", p.dest[2], p.dest[3])
        };
        let n = self.panes.len();
        if n == 1 && self.panes[0].dest == [0, 0, cw, ch] {
            return part(&self.panes[0]);
        }
        let mut g = String::new();
        if n > 1 {
            g += &format!("split={n}{};", (0..n).map(|i| format!("[s{i}]")).collect::<String>());
        }
        for (i, p) in self.panes.iter().enumerate() {
            let input = if n > 1 { format!("[s{i}]") } else { String::new() };
            g += &format!("{input}{}[p{i}];", part(p));
        }
        g += &format!("color=c=black:s={cw}x{ch}:r={fps}[o0];");
        for (i, p) in self.panes.iter().enumerate() {
            let (out, sep) = if i + 1 == n { (String::new(), "") } else { (format!("[o{}]", i + 1), ";") };
            g += &format!("[o{i}][p{i}]overlay={}:{}:shortest=1{out}{sep}", p.dest[0], p.dest[1]);
        }
        g
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 1920x1080 stream, face about 100 px wide at (1500, 200).
    const W: u32 = 1920;
    const H: u32 = 1080;
    fn face() -> Rect {
        Rect { x: 1450.0 / 1920.0, y: 150.0 / 1080.0, w: 100.0 / 1920.0, h: 100.0 / 1080.0 }
    }

    #[test]
    fn one_face_starts_in_focus_with_a_9x16_window_on_the_face() {
        let l = Layout::auto(&[face()], W, H);
        assert_eq!((l.preset.as_str(), l.faces, l.notice.as_deref()), ("focus", 1, None));
        let p = &l.panes[0];
        assert_eq!(p.dest, [0, 0, 1080, 1920]);
        // Full height, 9:16 wide, centred on the face centre x = 1500 px, clamped inside the frame.
        let (w, h) = (p.crop.w * W as f64, p.crop.h * H as f64);
        assert!((h - 1080.0).abs() < 1e-6 && (w / h - 9.0 / 16.0).abs() < 1e-6, "{w}x{h}");
        assert!(((p.crop.x + p.crop.w / 2.0) * W as f64 - 1500.0).abs() < 1.0);
    }

    #[test]
    fn crop_is_clamped_inside_the_frame_when_the_face_is_at_the_edge() {
        let edge = Rect { x: 0.97, y: 0.4, w: 0.03, h: 0.1 };
        let c = Layout::auto(&[edge], W, H).panes[0].crop;
        assert!(c.x >= 0.0 && c.x + c.w <= 1.0 + 1e-9 && c.y >= 0.0 && c.y + c.h <= 1.0 + 1e-9, "{c:?}");
    }

    #[test]
    fn no_face_uses_full_with_a_notice_and_two_faces_too() {
        let none = Layout::auto(&[], W, H);
        assert_eq!((none.preset.as_str(), none.notice.as_deref()), ("full", Some("no_face")));
        let two = Layout::auto(&[face(), Rect { x: 0.1, ..face() }], W, H);
        assert_eq!((two.preset.as_str(), two.notice.as_deref(), two.faces), ("full", Some("multiple_faces"), 2));
    }

    #[test]
    fn full_fits_the_whole_frame_letterboxed_at_canvas_width() {
        let p = &Layout::auto(&[], W, H).panes[0];
        assert_eq!(p.crop, Rect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 });
        assert_eq!(p.dest, [0, 656, 1080, 608]); // 1080x608 centred vertically
    }

    #[test]
    fn every_preset_is_selectable_and_keeps_the_detected_face() {
        let l = Layout::auto(&[face()], W, H);
        assert_eq!(preset_names(), ["focus", "stack", "webcam_game", "full"]);
        for name in preset_names() {
            let n = l.with_preset(&name).unwrap();
            assert_eq!((n.preset.clone(), n.face), (name, l.face));
        }
        assert!(l.with_preset("nope").is_err());
    }

    #[test]
    fn webcam_game_puts_a_tight_face_crop_on_top_and_the_game_below() {
        let l = Layout::auto(&[face()], W, H).with_preset("webcam_game").unwrap();
        let (top, bottom) = (&l.panes[0], &l.panes[1]);
        assert_eq!((top.dest, bottom.dest), ([0, 0, 1080, 640], [0, 640, 1080, 1280]));
        // Webcam crop is 3 face-widths wide (300 px) and shaped like the 1080x640 box.
        assert!((top.crop.w * W as f64 - 300.0).abs() < 1.0);
        assert!((top.crop.w * W as f64 / (top.crop.h * H as f64) - 1080.0 / 640.0).abs() < 1e-6);
        // The game pane is the frame centre, shaped like 1080x1280.
        assert!(((bottom.crop.x + bottom.crop.w / 2.0) - 0.5).abs() < 1e-9);
        assert!((bottom.crop.w * W as f64 / (bottom.crop.h * H as f64) - 1080.0 / 1280.0).abs() < 1e-6);
    }

    #[test]
    fn shifting_a_pane_moves_the_crop_and_stops_at_the_frame_edge() {
        let mut l = Layout::auto(&[face()], W, H).with_preset("webcam_game").unwrap();
        let before = l.panes[0].crop;
        l.shift_pane(0, -0.01, 0.02).unwrap();
        assert!((l.panes[0].crop.x - (before.x - 0.01)).abs() < 1e-9 && (l.panes[0].crop.y - (before.y + 0.02)).abs() < 1e-9);
        l.shift_pane(0, 5.0, 5.0).unwrap();
        let c = l.panes[0].crop;
        assert!((c.x + c.w - 1.0).abs() < 1e-9 && (c.y + c.h - 1.0).abs() < 1e-9);
        assert!(l.shift_pane(9, 0.0, 0.0).is_err());
    }

    #[test]
    fn filter_is_a_single_pass_graph_per_layout() {
        let focus = Layout::auto(&[face()], W, H).filter(30);
        assert!(focus.starts_with("crop=") && focus.contains("scale=1080:1920") && !focus.contains("overlay"), "{focus}");
        let split = Layout::auto(&[face()], W, H).with_preset("stack").unwrap().filter(30);
        assert!(split.contains("split=2") && split.matches("overlay=").count() == 2 && split.contains("color=c=black:s=1080x1920"), "{split}");
        assert!(!split.trim_end().ends_with(']'), "last chain must be unlabeled so the exporter can append filters");
    }

    // Two faces centred at 0.25 and 0.75; the speaker changes to the right face at 10 s.
    fn two_faces() -> [Rect; 2] {
        [Rect { x: 0.2, y: 0.3, w: 0.1, h: 0.1 }, Rect { x: 0.7, y: 0.3, w: 0.1, h: 0.1 }]
    }
    fn follow() -> Vec<Key> {
        vec![Key { t: 0.0, cx: 0.25 }, Key { t: 10.0, cx: 0.75 }]
    }
    fn following() -> Layout {
        Layout::auto_following(&two_faces(), follow(), W, H)
    }
    fn centre(l: &Layout, t: f64) -> f64 {
        let c = l.at(t).panes[0].crop;
        c.x + c.w / 2.0
    }

    #[test]
    fn two_faces_with_a_speaker_follow_start_in_focus_on_the_first_speaker() {
        let l = following();
        assert_eq!((l.preset.as_str(), l.faces, l.notice.as_deref()), ("focus", 2, Some("follow_speaker")));
        assert!((centre(&l, 0.0) - 0.25).abs() < 1e-9);
        assert_eq!(l.panes[0].crop, l.at(0.0).panes[0].crop, "the stored crop is the one at the start");
        assert_eq!(Layout::auto_following(&two_faces(), vec![], W, H), Layout::auto(&two_faces(), W, H), "no keys = old behaviour");
    }

    #[test]
    fn the_crop_moves_to_the_new_speaker_at_the_change_with_a_smooth_ease() {
        let l = following();
        assert!((centre(&l, 9.99) - 0.25).abs() < 1e-9, "stays until the speaker changes");
        assert!((centre(&l, 10.0 + FOLLOW_SECS / 2.0) - 0.5).abs() < 1e-9, "half way after half the ease");
        assert!((centre(&l, 10.0 + FOLLOW_SECS) - 0.75).abs() < 1e-9 && (centre(&l, 30.0) - 0.75).abs() < 1e-9);
        // No coarse jump: at 25 fps a frame never moves the window by more than 4% of the Source width.
        let steps: Vec<f64> = (0..400).map(|i| (centre(&l, 9.0 + (i + 1) as f64 * 0.04) - centre(&l, 9.0 + i as f64 * 0.04)).abs()).collect();
        assert!(steps.iter().all(|s| *s < 0.04), "{:?}", steps.iter().cloned().fold(0.0, f64::max));
        assert!(steps.iter().any(|s| *s > 0.01), "but it does move");
    }

    #[test]
    fn leaving_focus_keeps_the_follow_and_coming_back_restores_it() {
        let stack = following().with_preset("stack").unwrap();
        assert_eq!(stack.follow, follow());
        assert!((centre(&stack, 20.0) - centre(&stack, 0.0)).abs() < 1e-9, "other presets are not animated");
        let back = stack.with_preset("focus").unwrap();
        assert!((centre(&back, 0.0) - 0.25).abs() < 1e-9 && (centre(&back, 20.0) - 0.75).abs() < 1e-9);
    }

    #[test]
    fn a_manual_shift_moves_the_whole_followed_path() {
        let mut l = following();
        l.shift_pane(0, 0.05, 0.0).unwrap();
        assert!((centre(&l, 0.0) - 0.30).abs() < 1e-9 && (centre(&l, 20.0) - 0.80).abs() < 1e-9);
        assert!((l.panes[0].crop.x + l.panes[0].crop.w / 2.0 - 0.30).abs() < 1e-9, "the stored crop shows the shift");
        l.shift_pane(0, 5.0, 0.0).unwrap();
        assert!(centre(&l, 20.0) + l.panes[0].crop.w / 2.0 <= 1.0 + 1e-9, "still inside the frame");
    }

    #[test]
    fn follow_filter_moves_the_crop_with_a_time_expression_and_fixed_filter_does_not() {
        let f = following().filter(30);
        assert!(f.starts_with("crop=608:1080:'clip(") && f.contains("(t-10.000)") && f.contains("scale=1080:1920"), "{f}");
        assert!(!following().with_preset("full").unwrap().filter(30).contains("(t-"), "only Focus follows");
    }
}
