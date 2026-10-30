//! Face detection for the Layout engine: YuNet (shared/models, MIT licence, opencv_zoo) on ONNX Runtime.
//! Samples a few frames across a Clip with ffmpeg, detects faces per frame, then keeps the typical set.

use crate::{layout::Rect, source::Source, tools};
use ort::{session::Session, value::Tensor};
use std::sync::{Mutex, OnceLock};

const MODEL: &[u8] = include_bytes!("../../../shared/models/face_detection_yunet_2023mar.onnx");
/// Model input is a square; frames are scaled to fit and padded bottom/right.
const SIZE: usize = 640;
const SAMPLES: usize = 5;
const SCORE_MIN: f32 = 0.7;
const NMS_IOU: f32 = 0.3;

fn err(e: impl std::fmt::Display) -> String {
    format!("Face detection failed: {e}")
}

/// Faces (left to right) that typically appear in `[start, end]` of the Source.
pub fn detect_clip_faces(source: &Source, start: f64, end: f64) -> Result<Vec<Rect>, String> {
    let mut session = session()?.lock().map_err(err)?;
    let mut per_frame = Vec::new();
    for i in 0..SAMPLES {
        let at = start + (end - start) * (i as f64 + 0.5) / SAMPLES as f64;
        let bgr = grab_frame(source, at)?;
        per_frame.push(detect(&mut *session, &bgr, source.width as usize, source.height as usize)?);
    }
    Ok(typical_faces(&per_frame))
}

/// ONNX Runtime allows one Environment per process, so the model session is created once and shared.
fn session() -> Result<&'static Mutex<Session>, String> {
    static S: OnceLock<Result<Mutex<Session>, String>> = OnceLock::new();
    S.get_or_init(|| {
        let env = ort::init().with_telemetry(false).build().map_err(err)?;
        let session = Session::builder(&env).and_then(|mut b| b.commit_from_memory(MODEL)).map_err(err)?;
        std::mem::forget(env); // the session needs it for the rest of the process
        Ok(Mutex::new(session))
    })
    .as_ref()
    .map_err(Clone::clone)
}

/// One frame at `at` seconds, scaled to fit SIZE x SIZE (top-left, padded black), raw BGR.
fn grab_frame(source: &Source, at: f64) -> Result<Vec<u8>, String> {
    let vf = format!("scale={SIZE}:{SIZE}:force_original_aspect_ratio=decrease,pad={SIZE}:{SIZE}:0:0:black");
    let out = tools::command("ffmpeg")
        .args(["-v", "error", "-ss", &format!("{at:.3}"), "-i", &source.path, "-frames:v", "1", "-vf", &vf])
        .args(["-f", "rawvideo", "-pix_fmt", "bgr24", "pipe:1"])
        .output()
        .map_err(|e| format!("Cannot run ffmpeg (is it installed?): {e}"))?;
    if !out.status.success() || out.stdout.len() != SIZE * SIZE * 3 {
        return Err(err(format!("cannot read a frame at {at:.1}s: {}", String::from_utf8_lossy(&out.stderr).trim())));
    }
    Ok(out.stdout)
}

/// Runs YuNet on a padded BGR frame; returns faces as Source-relative rects, left to right.
fn detect(session: &mut Session, bgr: &[u8], src_w: usize, src_h: usize) -> Result<Vec<Rect>, String> {
    // NCHW float 0..255 in BGR order, as YuNet expects.
    let mut input = vec![0f32; 3 * SIZE * SIZE];
    for (i, px) in bgr.chunks_exact(3).enumerate() {
        for c in 0..3 {
            input[c * SIZE * SIZE + i] = px[c] as f32;
        }
    }
    let tensor = Tensor::from_array(([1usize, 3, SIZE, SIZE], input)).map_err(err)?;
    let outputs = session.run(ort::inputs!["input" => tensor]).map_err(err)?;
    let get = |name: String| -> Result<Vec<f32>, String> {
        Ok(outputs[name.as_str()].try_extract_tensor::<f32>().map_err(err)?.1.to_vec())
    };
    let fit = (SIZE as f32 / src_w as f32).min(SIZE as f32 / src_h as f32);
    let (fw, fh) = (src_w as f32 * fit, src_h as f32 * fit);
    let mut found: Vec<(f32, [f32; 4])> = Vec::new();
    for stride in [8usize, 16, 32] {
        let (cls, obj, bbox) = (get(format!("cls_{stride}"))?, get(format!("obj_{stride}"))?, get(format!("bbox_{stride}"))?);
        let cols = SIZE / stride;
        for idx in 0..cols * cols {
            let score = (cls[idx].clamp(0.0, 1.0) * obj[idx].clamp(0.0, 1.0)).sqrt();
            if score < SCORE_MIN {
                continue;
            }
            let (r, c, s) = ((idx / cols) as f32, (idx % cols) as f32, stride as f32);
            let b = &bbox[idx * 4..idx * 4 + 4];
            let (cx, cy, w, h) = ((c + b[0]) * s, (r + b[1]) * s, b[2].exp() * s, b[3].exp() * s);
            found.push((score, [cx - w / 2.0, cy - h / 2.0, w, h]));
        }
    }
    let mut faces: Vec<Rect> = nms(found)
        .into_iter()
        .map(|[x, y, w, h]| {
            // Back to Source-relative units, clipped to the frame (the pad area is not Source).
            let (x0, y0) = ((x / fw).clamp(0.0, 1.0), (y / fh).clamp(0.0, 1.0));
            let (x1, y1) = (((x + w) / fw).clamp(0.0, 1.0), ((y + h) / fh).clamp(0.0, 1.0));
            Rect { x: x0 as f64, y: y0 as f64, w: (x1 - x0) as f64, h: (y1 - y0) as f64 }
        })
        .collect();
    faces.sort_by(|a, b| a.x.total_cmp(&b.x));
    Ok(faces)
}

fn nms(mut c: Vec<(f32, [f32; 4])>) -> Vec<[f32; 4]> {
    c.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut keep: Vec<[f32; 4]> = Vec::new();
    for (_, b) in c {
        if keep.iter().all(|k| iou(k, &b) < NMS_IOU) {
            keep.push(b);
        }
    }
    keep
}

fn iou(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let (iw, ih) = ((a[0] + a[2]).min(b[0] + b[2]) - a[0].max(b[0]), (a[1] + a[3]).min(b[1] + b[3]) - a[1].max(b[1]));
    if iw <= 0.0 || ih <= 0.0 {
        return 0.0;
    }
    iw * ih / (a[2] * a[3] + b[2] * b[3] - iw * ih)
}

/// The most common face count across sampled frames (ties go to fewer faces), with each face's box
/// the per-coordinate median over the frames that have that count. A single stray detection is ignored.
pub fn typical_faces(per_frame: &[Vec<Rect>]) -> Vec<Rect> {
    let max = per_frame.iter().map(Vec::len).max().unwrap_or(0);
    let count = (0..=max).rev().max_by_key(|n| per_frame.iter().filter(|f| f.len() == *n).count()).unwrap_or(0);
    let same: Vec<&Vec<Rect>> = per_frame.iter().filter(|f| f.len() == count).collect();
    let median = |mut v: Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    (0..count)
        .map(|i| Rect {
            x: median(same.iter().map(|f| f[i].x).collect()),
            y: median(same.iter().map(|f| f[i].y).collect()),
            w: median(same.iter().map(|f| f[i].w).collect()),
            h: median(same.iter().map(|f| f[i].h).collect()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f64) -> Rect {
        Rect { x, y: 0.2, w: 0.1, h: 0.1 }
    }

    #[test]
    fn typical_faces_uses_the_most_common_count_and_median_boxes() {
        let frames = vec![vec![r(0.3)], vec![r(0.32)], vec![], vec![r(0.31)], vec![r(0.1), r(0.6)]];
        assert_eq!(typical_faces(&frames), vec![r(0.31)]);
        assert_eq!(typical_faces(&[vec![], vec![r(0.5)]]).len(), 0, "a tie goes to fewer faces");
        assert!(typical_faces(&[]).is_empty());
    }
}
