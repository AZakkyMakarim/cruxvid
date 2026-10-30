//! Manual Clips, trimming and preview (cruxvid-b2t.17). None of it needs a Gemini key: a Project can start
//! from a local Source with no Transcript, a Clip is just a start-end range, and edits are saved in the Project
//! so Layout, Caption and Export pick them up. A range outside 60-120 s only warns.

use crate::{
    clips::{Clip, MAX_SECS, MIN_SECS},
    engine::Engine,
    layout_store::LayoutStore,
    project::{Project, ProjectStore},
    source::{self, Source},
    transcript::Transcript,
    translate::TranslationStore,
};
use serde::Serialize;
use std::path::Path;

/// Shortest range a Clip can have; below this a range is a typing mistake, not a Clip.
const MIN_RANGE: f64 = 1.0;
/// Same tolerance `clips.rs` uses at the 60/120 s edges.
const EPS: f64 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RangeWarning {
    TooShort,
    TooLong,
}

/// `Some` when the range is outside the 60-120 s a Clip should have. The Clip can still be exported.
pub fn warning(start: f64, end: f64) -> Option<RangeWarning> {
    match end - start {
        d if d < MIN_SECS - EPS => Some(RangeWarning::TooShort),
        d if d > MAX_SECS + EPS => Some(RangeWarning::TooLong),
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RangeEdit {
    pub clip: Clip,
    pub warning: Option<RangeWarning>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Added {
    pub project: Project,
    pub clip_id: String,
    pub warning: Option<RangeWarning>,
}

/// What the player needs: a playable file and the exact range to play in it.
#[derive(Debug, Clone, Serialize)]
pub struct Preview {
    pub path: String,
    pub start: f64,
    pub end: f64,
}

fn check(source: &Source, start: f64, end: f64) -> Result<(), String> {
    if !(start.is_finite() && end.is_finite()) || start < 0.0 || end > source.duration + EPS {
        return Err(format!("The range must lie inside the Source (0 - {:.1} s).", source.duration));
    }
    if end - start < MIN_RANGE {
        return Err(format!("A Clip must be at least {MIN_RANGE} s long."));
    }
    Ok(())
}

fn store(engine: &Engine) -> ProjectStore {
    ProjectStore::new(engine.data_dir().to_path_buf())
}

/// A saved Project for a local Source with no Transcript and no Clips, made without calling Gemini.
pub fn new_manual_project(engine: &Engine, path: &Path) -> Result<Project, String> {
    let project = Project {
        id: ProjectStore::new_id(),
        source: source::probe_local(path)?,
        transcript: Transcript { language: "und".into(), words: Vec::new() },
        clips: Vec::new(),
    };
    store(engine).save(&project)?;
    Ok(project)
}

/// Adds a Clip for `start`-`end` of the Source. Clips stay numbered in timeline order.
pub fn add_manual_clip(engine: &Engine, project_id: &str, start: f64, end: f64) -> Result<Added, String> {
    let store = store(engine);
    store.update(project_id, |p| {
        check(&p.source, start, end)?;
        let mut n = p.clips.len() + 1;
        while p.clips.iter().any(|c| c.id == format!("manual-{n}")) {
            n += 1;
        }
        let clip_id = format!("manual-{n}");
        p.clips.push(Clip {
            id: clip_id.clone(),
            number: 0,
            start,
            end,
            title: format!("Clip {n}"),
            description: String::new(),
            // The user chose this range, so it must not vanish under the Hook score threshold.
            hook_score: 100,
            hook_reason: "Manual Clip".into(),
        });
        p.clips.sort_by(|a, b| a.start.total_cmp(&b.start));
        for (i, c) in p.clips.iter_mut().enumerate() {
            c.number = i as u32 + 1;
        }
        Ok(Added { project: p.clone(), clip_id, warning: warning(start, end) })
    })
}

/// Changes a Clip's start and end. What was made for the old range is dropped so it is made again for the new one:
/// the Clip's translation (its words are timed for the old range) and a Layout that was detected rather than chosen.
/// A Layout the user chose is kept; its speaker follow keys move with the start so they stay on the same words.
/// shortcut: a kept Layout's crop is not re-detected for newly included footage, use Layout presets to redo it.
pub fn set_range(engine: &Engine, project_id: &str, clip_id: &str, start: f64, end: f64) -> Result<RangeEdit, String> {
    let store = store(engine);
    let (clip, shift, changed) = store.update(project_id, |p| {
        check(&p.source, start, end)?;
        let clip = p.clips.iter_mut().find(|c| c.id == clip_id).ok_or_else(|| format!("Clip not found: {clip_id}"))?;
        let (old_start, old_end) = (clip.start, clip.end);
        (clip.start, clip.end) = (start, end);
        Ok((clip.clone(), old_start - start, (old_start, old_end) != (start, end)))
    })?;
    if changed {
        TranslationStore::new(engine.data_dir().to_path_buf()).drop_clip(project_id, clip_id)?;
        let layouts = LayoutStore::new(engine.data_dir().to_path_buf());
        if let Some(mut layout) = layouts.get(project_id, clip_id)? {
            if !layout.user {
                layouts.remove(project_id, clip_id)?;
            } else if !layout.follow.is_empty() && shift != 0.0 {
                layout.follow.iter_mut().for_each(|k| k.t = (k.t + shift).max(0.0));
                layouts.put(project_id, clip_id, &layout)?;
            }
        }
    }
    Ok(RangeEdit { warning: warning(start, end), clip })
}

/// The file to play and the range inside it. A link Source is stream-cut first, so `start`/`end` are relative to that cut.
pub fn preview(engine: &Engine, project_id: &str, clip_id: &str) -> Result<Preview, String> {
    let p = engine.load_project(project_id)?;
    let clip = p.clips.iter().find(|c| c.id == clip_id).ok_or_else(|| format!("Clip not found: {clip_id}"))?;
    let (media, view) = engine.clip_media(project_id, &p, clip)?;
    Ok(Preview { path: media.path, start: view.start, end: view.end })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_ranges_outside_60_to_120_seconds_warn() {
        assert_eq!(warning(0.0, 59.0), Some(RangeWarning::TooShort));
        assert_eq!(warning(0.0, 121.0), Some(RangeWarning::TooLong));
        assert_eq!([warning(0.0, 60.0), warning(0.0, 90.0), warning(10.0, 130.0), warning(0.0, 120.04)], [None; 4]);
    }
}
