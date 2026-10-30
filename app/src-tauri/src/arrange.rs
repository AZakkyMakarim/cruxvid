//! Clip list view: order by timeline or Hook score, and hide Clips below the threshold slider.
//! Hidden Clips stay in the Project; lowering the threshold shows them again.

use crate::{clips::Clip, engine::Engine};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

pub const DEFAULT_THRESHOLD: u8 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClipOrder {
    Timeline,
    Hook,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArrangedClip {
    pub id: String,
    pub hidden: bool,
}

/// Every Clip in the requested order; `hidden` when its Hook score is below `threshold`.
pub fn arrange(clips: &[Clip], order: ClipOrder, threshold: u8) -> Vec<ArrangedClip> {
    let mut sorted: Vec<&Clip> = clips.iter().collect();
    match order {
        ClipOrder::Timeline => sorted.sort_by(|a, b| a.start.total_cmp(&b.start)),
        ClipOrder::Hook => sorted.sort_by(|a, b| b.hook_score.cmp(&a.hook_score).then(a.start.total_cmp(&b.start))),
    }
    sorted.into_iter().map(|c| ArrangedClip { id: c.id.clone(), hidden: c.hook_score < threshold }).collect()
}

#[tauri::command]
pub fn arrange_clips(engine: State<'_, Arc<Engine>>, project_id: String, order: ClipOrder, threshold: u8) -> Result<Vec<ArrangedClip>, String> {
    let arranged = arrange(&engine.load_project(&project_id)?.clips, order, threshold);
    crate::history::save_view(&engine, &project_id, order, threshold)?;
    Ok(arranged)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(id: &str, start: f64, hook_score: u8) -> Clip {
        Clip { id: id.into(), number: 0, start, end: start + 60.0, title: String::new(), description: String::new(), hook_score, hook_reason: String::new() }
    }

    fn view(order: ClipOrder, threshold: u8) -> Vec<(String, bool)> {
        let clips = [clip("a", 0.0, 82), clip("b", 100.0, 64), clip("c", 200.0, 91), clip("d", 300.0, 47), clip("e", 400.0, 64)];
        arrange(&clips, order, threshold).into_iter().map(|c| (c.id, c.hidden)).collect()
    }

    #[test]
    fn orders_by_timeline_or_by_hook_score_with_timeline_breaking_ties() {
        let ids = |v: Vec<(String, bool)>| v.into_iter().map(|(id, _)| id).collect::<String>();
        assert_eq!(ids(view(ClipOrder::Timeline, 0)), "abcde");
        assert_eq!(ids(view(ClipOrder::Hook, 0)), "cabed");
    }

    #[test]
    fn clips_below_the_threshold_are_hidden_not_removed_and_return_when_it_drops() {
        let hidden = |t| view(ClipOrder::Timeline, t).into_iter().filter(|(_, h)| *h).map(|(id, _)| id).collect::<String>();
        assert_eq!(hidden(50), "d");
        assert_eq!(hidden(80), "bde");
        assert_eq!(hidden(20), "");
        assert_eq!(view(ClipOrder::Timeline, 80).len(), 5);
        assert_eq!(hidden(47), "", "a Clip at the threshold stays visible");
    }
}
