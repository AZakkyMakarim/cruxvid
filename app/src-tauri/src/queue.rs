//! Export queue: sequential jobs per Project with waiting/running/done/failed status, saved next to the Project.
//! Jobs left `running` by a closed app are shown as `waiting` again (nothing finished them).

use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::PathBuf, sync::Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Waiting,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Job {
    pub clip_id: String,
    pub status: Status,
    /// Saved MP4 path once done.
    pub output: Option<String>,
    /// User-readable reason once failed.
    pub error: Option<String>,
}

pub struct JobQueue {
    root: PathBuf,
    /// Serializes load-modify-save between the worker and UI commands.
    lock: Mutex<()>,
    /// Projects whose worker is running in this process.
    active: Mutex<HashSet<String>>,
}

impl JobQueue {
    pub fn new(data_dir: PathBuf) -> Self {
        Self { root: data_dir.join("projects"), lock: Mutex::new(()), active: Mutex::new(HashSet::new()) }
    }

    /// `project_id` must already be validated by the ProjectStore.
    fn file(&self, project_id: &str) -> PathBuf {
        self.root.join(project_id).join("queue.json")
    }

    fn read(&self, project_id: &str) -> Vec<Job> {
        let mut jobs: Vec<Job> = std::fs::read(self.file(project_id)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        if !self.active.lock().unwrap().contains(project_id) {
            for j in jobs.iter_mut().filter(|j| j.status == Status::Running) {
                j.status = Status::Waiting;
            }
        }
        jobs
    }

    fn write(&self, project_id: &str, jobs: &[Job]) -> Result<(), String> {
        let file = self.file(project_id);
        std::fs::create_dir_all(file.parent().unwrap()).map_err(|e| format!("Cannot save queue: {e}"))?;
        std::fs::write(file, serde_json::to_vec_pretty(jobs).map_err(|e| e.to_string())?).map_err(|e| format!("Cannot save queue: {e}"))
    }

    fn update<T>(&self, project_id: &str, f: impl FnOnce(&mut Vec<Job>) -> T) -> Result<T, String> {
        let _g = self.lock.lock().unwrap();
        let mut jobs = self.read(project_id);
        let out = f(&mut jobs);
        self.write(project_id, &jobs)?;
        Ok(out)
    }

    pub fn jobs(&self, project_id: &str) -> Vec<Job> {
        let _g = self.lock.lock().unwrap();
        self.read(project_id)
    }

    /// Adds a waiting job per Clip. A Clip already waiting/running is left alone; a finished one is queued again.
    pub fn enqueue(&self, project_id: &str, clip_ids: &[String]) -> Result<Vec<Job>, String> {
        self.update(project_id, |jobs| {
            for id in clip_ids {
                match jobs.iter_mut().find(|j| &j.clip_id == id) {
                    Some(j) if matches!(j.status, Status::Waiting | Status::Running) => {}
                    Some(j) => *j = waiting(id),
                    None => jobs.push(waiting(id)),
                }
            }
            jobs.clone()
        })
    }

    /// Failed jobs go back to waiting; successful ones are untouched.
    pub fn retry_failed(&self, project_id: &str) -> Result<Vec<Job>, String> {
        self.update(project_id, |jobs| {
            for j in jobs.iter_mut().filter(|j| j.status == Status::Failed) {
                *j = waiting(&j.clip_id.clone());
            }
            jobs.clone()
        })
    }

    /// Runs every waiting job in order, one at a time, saving each status change.
    /// Returns false (and does nothing) when this Project's queue is already running.
    pub fn run(&self, project_id: &str, export: impl Fn(&str) -> Result<String, String>) -> Result<bool, String> {
        if !self.active.lock().unwrap().insert(project_id.to_string()) {
            return Ok(false);
        }
        let result = (|| -> Result<(), String> {
            loop {
                let next = self.update(project_id, |jobs| {
                    let j = jobs.iter_mut().find(|j| j.status == Status::Waiting)?;
                    j.status = Status::Running;
                    Some(j.clip_id.clone())
                })?;
                let Some(clip_id) = next else { return Ok(()) };
                let outcome = export(&clip_id);
                self.update(project_id, |jobs| {
                    if let Some(j) = jobs.iter_mut().find(|j| j.clip_id == clip_id) {
                        match outcome {
                            Ok(path) => (j.status, j.output, j.error) = (Status::Done, Some(path), None),
                            Err(e) => (j.status, j.output, j.error) = (Status::Failed, None, Some(e)),
                        }
                    }
                })?;
            }
        })();
        self.active.lock().unwrap().remove(project_id);
        result.map(|()| true)
    }
}

fn waiting(clip_id: &str) -> Job {
    Job { clip_id: clip_id.to_string(), status: Status::Waiting, output: None, error: None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn status_is_accurate_while_jobs_run_one_at_a_time() {
        let tmp = tempfile::tempdir().unwrap();
        let q = JobQueue::new(tmp.path().to_path_buf());
        q.enqueue("p1", &ids(&["a", "b", "c"])).unwrap();
        let seen = RefCell::new(Vec::new());
        q.run("p1", |id| {
            seen.borrow_mut().push(q.jobs("p1").iter().map(|j| j.status).collect::<Vec<_>>());
            if id == "b" { Err("boom".into()) } else { Ok(format!("{id}.mp4")) }
        })
        .unwrap();
        use Status::*;
        assert_eq!(*seen.borrow(), [vec![Running, Waiting, Waiting], vec![Done, Running, Waiting], vec![Done, Failed, Running]]);
        let jobs = q.jobs("p1");
        assert_eq!((jobs[1].status, jobs[1].error.as_deref()), (Failed, Some("boom")));
        assert_eq!(jobs[0].output.as_deref(), Some("a.mp4"));
    }

    #[test]
    fn second_run_while_active_is_a_noop_and_enqueue_skips_pending() {
        let tmp = tempfile::tempdir().unwrap();
        let q = JobQueue::new(tmp.path().to_path_buf());
        q.enqueue("p1", &ids(&["a"])).unwrap();
        q.run("p1", |_| {
            assert!(!q.run("p1", |_| unreachable!()).unwrap());
            assert_eq!(q.enqueue("p1", &ids(&["a"])).unwrap().len(), 1);
            Ok("x".into())
        })
        .unwrap();
        assert_eq!(q.jobs("p1")[0].status, Status::Done);
    }
}
