//! AI reviews running in the background, at most one per PR. A run keeps going when the user leaves
//! the PR, so the PR list and the AI tab show the same state. Keys are `ai_review::review_key`.

use crate::backend;
use gpui_kit::*;
use revisor_core::{ai_review::Review, AppState};
use std::{collections::HashMap, future::Future, sync::Arc};

pub struct AiRuns {
    running: HashMap<String, Task<()>>,
    /// Why the last run of a PR failed; cleared when it runs again.
    failed: HashMap<String, String>,
}

/// A run finished with the review or the error.
pub struct Finished {
    pub key: String,
    pub result: Result<Review, String>,
}

impl EventEmitter<Finished> for AiRuns {}

struct GlobalRuns(Entity<AiRuns>);
impl Global for GlobalRuns {}

pub fn init(cx: &mut App) {
    let runs = cx.new(|_| AiRuns { running: HashMap::new(), failed: HashMap::new() });
    cx.set_global(GlobalRuns(runs));
}

pub fn runs(cx: &App) -> Entity<AiRuns> {
    cx.global::<GlobalRuns>().0.clone()
}

impl AiRuns {
    pub fn is_running(&self, key: &str) -> bool {
        self.running.contains_key(key)
    }

    pub fn error(&self, key: &str) -> Option<&str> {
        self.failed.get(key).map(String::as_str)
    }

    /// Run `f` on tokio unless this PR already has a run. `f` saves the review itself.
    pub fn start<Fut>(&mut self, key: String, cx: &mut Context<Self>, f: impl FnOnce(Arc<AppState>) -> Fut)
    where
        Fut: Future<Output = Result<Review, String>> + Send + 'static,
    {
        if self.running.contains_key(&key) {
            return;
        }
        self.failed.remove(&key);
        let run = backend::call_cancellable(cx, f);
        let task_key = key.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = run.await;
            this.update(cx, |this, cx| {
                // Detach, not drop: this task is still running.
                if let Some(task) = this.running.remove(&task_key) {
                    task.detach();
                }
                if let Err(e) = &result {
                    this.failed.insert(task_key.clone(), e.clone());
                }
                cx.emit(Finished { key: task_key, result });
                cx.notify();
            })
            .ok();
        });
        self.running.insert(key, task);
        cx.notify();
    }

    /// Dropping the task stops the CLI.
    pub fn cancel(&mut self, key: &str, cx: &mut Context<Self>) {
        if self.running.remove(key).is_some() {
            cx.notify();
        }
    }
}
