//! Shared PR detail model. Sidebar panels and the diff view read it and call its methods.

use crate::backend;
use gpui_kit::*;
use revisor_core::comments::{build_threads, CommentThread, Side};
use revisor_core::diff::{fetch_pr_files, process_pr_files, search::CodeMatch, ProcessedFile};
use revisor_core::provider::types::{
    Commit, IssueComment, PrReview, PullRequest, ReviewComment, ThreadResolutionInfo,
};
use revisor_core::{cache::queries, diff::search, files};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CommitSelection {
    All,
    Single(usize),
    /// Inclusive commit index range.
    Range(usize, usize),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DiffMode {
    Split,
    Unified,
}

/// Where the diff view should scroll to.
#[derive(Clone, Debug)]
pub enum ScrollTarget {
    File(String),
    Thread(u64),
    Line { path: String, side: Side, line: u64 },
    /// The active code search match.
    Match,
}

pub enum PrEvent {
    ScrollTo(ScrollTarget),
}

pub struct CodeSearch {
    pub query: String,
    /// Matches in the visible files; `CodeMatch::file` indexes `PrModel::visible`.
    pub matches: Vec<CodeMatch>,
    pub active: usize,
}

pub struct PrModel {
    pub owner: SharedString,
    pub repo: SharedString,
    pub number: u64,
    pub pr: Option<PullRequest>,
    /// Files of the current commit selection.
    pub files: Vec<ProcessedFile>,
    /// Indexes into `files` that pass the sidebar filter, in display order.
    pub visible: Vec<usize>,
    all_files: Option<Vec<ProcessedFile>>,
    pub comments: Vec<ReviewComment>,
    pub issue_comments: Vec<IssueComment>,
    pub reviews: Vec<PrReview>,
    resolutions: Vec<ThreadResolutionInfo>,
    /// Inline threads of all files, rebuilt whenever comments change.
    pub threads: Vec<CommentThread>,
    pub reviewed: HashSet<String>,
    pub commits: Vec<Commit>,
    pub selection: CommitSelection,
    pub loading: bool,
    pub files_loading: bool,
    pub selected: Option<String>,
    pub collapsed: HashSet<String>,
    pub diff_mode: DiffMode,
    pub filter_query: String,
    pub include: String,
    pub exclude: String,
    pub search: Option<CodeSearch>,
    file_lines: HashMap<(String, String), Arc<Vec<String>>>,
    file_lines_pending: HashSet<(String, String)>,
    _load: Task<()>,
    _files_load: Option<Task<()>>,
}

impl EventEmitter<PrEvent> for PrModel {}

impl PrModel {
    pub fn new(owner: SharedString, repo: SharedString, number: u64, cx: &mut Context<Self>) -> Self {
        let (o, r) = (owner.to_string(), repo.to_string());
        let load = backend::call(cx, move |s| async move {
            let p = s.provider().await?;
            let (pr, files, comments, issue_comments, reviews, resolutions, commits) = tokio::try_join!(
                async { p.get_pull_request(&o, &r, number).await.map_err(backend::err) },
                fetch_pr_files(&*p, &o, &r, number),
                async { p.get_pr_comments(&o, &r, number).await.map_err(backend::err) },
                async { p.get_issue_comments(&o, &r, number).await.map_err(backend::err) },
                async { p.get_pr_reviews(&o, &r, number).await.map_err(backend::err) },
                async { p.get_thread_resolutions(&o, &r, number).await.map_err(backend::err) },
                async { p.get_pr_commits(&o, &r, number).await.map_err(backend::err) },
            )?;
            drop(p);
            let reviewed = queries::get_reviewed_files(s.cache.lock().await.conn(), &o, &r, number)
                .map_err(backend::err)?;
            Ok((pr, files, comments, issue_comments, reviews, resolutions, commits, reviewed))
        });
        let task = cx.spawn(async move |this, cx| {
            let result = load.await;
            this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok((pr, files, comments, issue_comments, reviews, resolutions, commits, reviewed)) => {
                        this.pr = Some(pr);
                        this.comments = comments;
                        this.issue_comments = issue_comments;
                        this.reviews = reviews;
                        this.resolutions = resolutions;
                        this.commits = commits;
                        this.reviewed = reviewed.into_iter().collect();
                        this.rebuild_threads();
                        this.set_files(files, cx);
                    }
                    Err(e) => notify_error(format!("Failed to load PR: {e}"), cx),
                }
                cx.notify();
            })
            .ok();
        });
        Self {
            owner,
            repo,
            number,
            pr: None,
            files: vec![],
            visible: vec![],
            all_files: None,
            comments: vec![],
            issue_comments: vec![],
            reviews: vec![],
            resolutions: vec![],
            threads: vec![],
            reviewed: HashSet::new(),
            commits: vec![],
            selection: CommitSelection::All,
            loading: true,
            files_loading: false,
            selected: None,
            collapsed: HashSet::new(),
            diff_mode: DiffMode::Split,
            filter_query: String::new(),
            include: String::new(),
            exclude: String::new(),
            search: None,
            file_lines: HashMap::new(),
            file_lines_pending: HashSet::new(),
            _load: task,
            _files_load: None,
        }
    }

    fn set_files(&mut self, files: Vec<ProcessedFile>, cx: &mut Context<Self>) {
        self.selected = files.first().map(|f| f.filename.clone());
        self.files = files;
        self.refilter(cx);
    }

    fn refilter(&mut self, cx: &mut Context<Self>) {
        self.visible = files::filter_files(&self.files, &self.filter_query, &self.include, &self.exclude);
        self.research();
        cx.notify();
    }

    /// The `i`-th visible file.
    pub fn visible_file(&self, i: usize) -> &ProcessedFile {
        &self.files[self.visible[i]]
    }

    pub fn set_filter(&mut self, query: String, include: String, exclude: String, cx: &mut Context<Self>) {
        self.filter_query = query;
        self.include = include;
        self.exclude = exclude;
        self.refilter(cx);
    }

    pub fn conversation_count(&self) -> usize {
        self.threads.len()
            + self.issue_comments.len()
            + self.reviews.iter().filter(|r| !r.body.trim().is_empty()).count()
    }

    pub fn commit_selection_label(&self) -> String {
        match self.selection {
            CommitSelection::All => format!("All commits ({})", self.commits.len()),
            CommitSelection::Single(i) => self
                .commits
                .get(i)
                .map_or("1 commit".into(), |c| format!("{} {}", c.short_sha, c.message)),
            CommitSelection::Range(from, to) => format!("{} commits", to - from + 1),
        }
    }

    pub fn select_commits(&mut self, selection: CommitSelection, cx: &mut Context<Self>) {
        if self.all_files.is_none() {
            self.all_files = Some(self.files.clone());
        }
        self.selection = selection;
        let range = match selection {
            CommitSelection::All => {
                let files = self.all_files.clone().unwrap_or_default();
                self._files_load = None;
                self.files_loading = false;
                self.set_files(files, cx);
                return;
            }
            CommitSelection::Single(i) => (i, i),
            CommitSelection::Range(from, to) => (from, to),
        };
        let (Some(first), Some(last), Some(pr)) =
            (self.commits.get(range.0), self.commits.get(range.1), self.pr.as_ref())
        else {
            return;
        };
        let base = first.parents.first().cloned().unwrap_or_else(|| pr.base_sha.clone());
        let head = last.sha.clone();
        let (o, r) = (self.owner.to_string(), self.repo.to_string());
        let fetch = backend::call(cx, move |s| async move {
            let files = s.provider().await?.get_compare_files(&o, &r, &base, &head).await;
            files.map(process_pr_files).map_err(backend::err)
        });
        self.files_loading = true;
        // Replacing the task drops a request for an older selection.
        self._files_load = Some(cx.spawn(async move |this, cx| {
            let result = fetch.await;
            this.update(cx, |this, cx| {
                this.files_loading = false;
                match result {
                    Ok(files) => this.set_files(files, cx),
                    Err(e) => notify_error(format!("Failed to load commit files: {e}"), cx),
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Select `path` in the sidebar and scroll the diff to it.
    pub fn select_file(&mut self, path: &str, cx: &mut Context<Self>) {
        self.selected = Some(path.to_string());
        self.collapsed.remove(path);
        cx.emit(PrEvent::ScrollTo(ScrollTarget::File(path.to_string())));
        cx.notify();
    }

    /// Clear sidebar filters, open the target's file and scroll to it.
    pub fn reveal(&mut self, path: &str, target: ScrollTarget, cx: &mut Context<Self>) {
        if !(self.filter_query.is_empty() && self.include.is_empty() && self.exclude.is_empty()) {
            self.set_filter(String::new(), String::new(), String::new(), cx);
        }
        self.selected = Some(path.to_string());
        self.collapsed.remove(path);
        cx.emit(PrEvent::ScrollTo(target));
        cx.notify();
    }

    /// Diff scroll position moved into `path`; only updates the sidebar highlight.
    pub fn set_selected_from_scroll(&mut self, path: &str, cx: &mut Context<Self>) {
        if self.selected.as_deref() != Some(path) {
            self.selected = Some(path.to_string());
            cx.notify();
        }
    }

    /// Step to the previous (-1) or next (1) visible file.
    pub fn navigate_file(&mut self, direction: isize, cx: &mut Context<Self>) {
        let current = self
            .selected
            .as_ref()
            .and_then(|p| self.visible.iter().position(|&i| &self.files[i].filename == p));
        if let Some(next) = files::navigate_file(self.visible.len(), current, direction) {
            let path = self.visible_file(next).filename.clone();
            self.select_file(&path, cx);
        }
    }

    pub fn toggle_collapsed(&mut self, path: &str, cx: &mut Context<Self>) {
        if !self.collapsed.remove(path) {
            self.collapsed.insert(path.to_string());
        }
        cx.notify();
    }

    pub fn collapse_all(&mut self, collapse: bool, cx: &mut Context<Self>) {
        self.collapsed = if collapse {
            self.visible.iter().map(|&i| self.files[i].filename.clone()).collect()
        } else {
            HashSet::new()
        };
        cx.notify();
    }

    pub fn set_diff_mode(&mut self, mode: DiffMode, cx: &mut Context<Self>) {
        self.diff_mode = mode;
        cx.notify();
    }

    /// Optimistic toggle; rolls back if saving fails.
    pub fn toggle_reviewed(&mut self, path: &str, cx: &mut Context<Self>) {
        let was = !self.reviewed.insert(path.to_string());
        if was {
            self.reviewed.remove(path);
        }
        let (o, r, n, p) = (self.owner.to_string(), self.repo.to_string(), self.number, path.to_string());
        let save = backend::call(cx, move |s| async move {
            let cache = s.cache.lock().await;
            if was {
                queries::unmark_file_reviewed(cache.conn(), &o, &r, n, &p)
            } else {
                queries::mark_file_reviewed(cache.conn(), &o, &r, n, &p)
            }
            .map_err(backend::err)
        });
        let path = path.to_string();
        cx.spawn(async move |this, cx| {
            if save.await.is_err() {
                this.update(cx, |this, cx| {
                    if was {
                        this.reviewed.insert(path);
                    } else {
                        this.reviewed.remove(&path);
                    }
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
        cx.notify();
    }

    // --- Comments ---

    fn rebuild_threads(&mut self) {
        self.threads = build_threads(&self.comments, &self.resolutions);
    }

    /// Threads of `path`, in line order.
    pub fn threads_for<'a>(&'a self, path: &'a str) -> impl Iterator<Item = &'a CommentThread> {
        self.threads.iter().filter(move |t| t.root.path == path)
    }

    /// Add a new comment or replace an edited one.
    pub fn upsert_comment(&mut self, comment: ReviewComment, cx: &mut Context<Self>) {
        match self.comments.iter_mut().find(|c| c.id == comment.id) {
            Some(existing) => *existing = comment,
            None => self.comments.push(comment),
        }
        self.rebuild_threads();
        cx.notify();
    }

    pub fn remove_comment(&mut self, id: u64, cx: &mut Context<Self>) {
        self.comments.retain(|c| c.id != id);
        self.rebuild_threads();
        cx.notify();
    }

    /// Set a thread's resolved flag locally (call the API separately).
    pub fn set_resolved(&mut self, thread_node_id: &str, resolved: bool, cx: &mut Context<Self>) {
        for r in &mut self.resolutions {
            if r.thread_node_id == thread_node_id {
                r.is_resolved = resolved;
            }
        }
        self.rebuild_threads();
        cx.notify();
    }

    // --- Code search ---

    pub fn open_search(&mut self, cx: &mut Context<Self>) {
        if self.search.is_none() {
            self.search = Some(CodeSearch { query: String::new(), matches: vec![], active: 0 });
            cx.notify();
        }
    }

    pub fn close_search(&mut self, cx: &mut Context<Self>) {
        self.search = None;
        cx.notify();
    }

    pub fn set_search_query(&mut self, query: String, cx: &mut Context<Self>) {
        if let Some(s) = &mut self.search {
            s.query = query;
            s.active = 0;
        }
        self.research();
        self.reveal_match(cx);
        cx.notify();
    }

    /// Enter / Shift+Enter in the search bar.
    pub fn step_match(&mut self, forward: bool, cx: &mut Context<Self>) {
        if let Some(s) = &mut self.search {
            s.active = search::step(s.active, s.matches.len(), forward);
        }
        self.reveal_match(cx);
        cx.notify();
    }

    pub fn active_match(&self) -> Option<&CodeMatch> {
        self.search.as_ref().and_then(|s| s.matches.get(s.active))
    }

    fn research(&mut self) {
        let Some(s) = &mut self.search else { return };
        let visible: Vec<ProcessedFile> = self.visible.iter().map(|&i| self.files[i].clone()).collect();
        // ponytail: clones visible files per query; search over indexes if large PRs feel slow.
        s.matches = search::search_files(&visible, &s.query);
        s.active = s.active.min(s.matches.len().saturating_sub(1));
    }

    fn reveal_match(&mut self, cx: &mut Context<Self>) {
        if let Some(m) = self.active_match() {
            let path = self.visible_file(m.file).filename.clone();
            self.collapsed.remove(&path);
            self.selected = Some(path);
            cx.emit(PrEvent::ScrollTo(ScrollTarget::Match));
        }
    }

    // --- File content (expand context, AI features) ---

    /// Lines of `path` at `git_ref`, fetched once and cached. `None` while loading; notifies when ready.
    pub fn file_lines(&mut self, path: &str, git_ref: &str, cx: &mut Context<Self>) -> Option<Arc<Vec<String>>> {
        let key = (path.to_string(), git_ref.to_string());
        if let Some(lines) = self.file_lines.get(&key) {
            return Some(lines.clone());
        }
        if self.file_lines_pending.insert(key.clone()) {
            let (o, r, p, g) = (self.owner.to_string(), self.repo.to_string(), key.0.clone(), key.1.clone());
            let fetch = backend::call(cx, move |s| async move {
                s.provider().await?.get_file_content(&o, &r, &p, &g).await.map_err(backend::err)
            });
            cx.spawn(async move |this, cx| {
                let result = fetch.await;
                this.update(cx, |this, cx| {
                    this.file_lines_pending.remove(&key);
                    match result {
                        Ok(text) => {
                            let lines = text.split('\n').map(str::to_string).collect();
                            this.file_lines.insert(key, Arc::new(lines));
                        }
                        Err(e) => notify_error(format!("Failed to load file: {e}"), cx),
                    }
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        None
    }

    pub fn resolutions(&self) -> &[ThreadResolutionInfo] {
        &self.resolutions
    }

    /// Head commit of the current commit selection.
    pub fn head_sha(&self) -> Option<&str> {
        match self.selection {
            CommitSelection::All => self.pr.as_ref().map(|pr| pr.head_sha.as_str()),
            CommitSelection::Single(i) | CommitSelection::Range(_, i) => self.commits.get(i).map(|c| c.sha.as_str()),
        }
    }

    /// Cached lines of `path` at `git_ref`, without fetching.
    pub fn cached_file_lines(&self, path: &str, git_ref: &str) -> Option<Arc<Vec<String>>> {
        self.file_lines.get(&(path.to_string(), git_ref.to_string())).cloned()
    }

    pub fn file_lines_loading(&self, path: &str, git_ref: &str) -> bool {
        self.file_lines_pending.contains(&(path.to_string(), git_ref.to_string()))
    }
}

/// Show an error in the active window. Models have no window of their own.
pub fn notify_error(message: String, cx: &mut App) {
    use gpui_kit::component::{notification::Notification, WindowExt as _};
    if let Some(window) = cx.active_window() {
        window
            .update(cx, |_, window, cx| window.push_notification(Notification::error(message), cx))
            .ok();
    }
}
