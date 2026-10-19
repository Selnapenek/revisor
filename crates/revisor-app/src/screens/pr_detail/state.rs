//! Shared PR detail model. Sidebar panels and the diff view read it and call its methods.

use crate::backend;
use gpui_kit::*;
use revisor_core::comments::{build_threads, CommentThread, Side};
use revisor_core::diff::{diff_contents, fetch_pr_files, process_pr_files, search::CodeMatch, ProcessedFile};
use revisor_core::provider::types::{
    Commit, IssueComment, PrReview, PullRequest, ReviewComment, ThreadResolutionInfo,
};
use revisor_core::provider::github::pr_load_error;
use revisor_core::ai_review::{self, ReadingStep};
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
    /// A position recorded in the navigation history.
    Spot(Spot),
}

/// A diff position to go back to: a line (`None` = the file header) and how many rows showed above it.
#[derive(Clone, Debug, PartialEq)]
pub struct Spot {
    pub path: String,
    pub line: Option<(Side, u64)>,
    pub above: usize,
    pub offset_in_item: Pixels,
}

/// Navigation history entries kept; older ones drop.
const HISTORY_MAX: usize = 50;

pub enum PrEvent {
    ScrollTo(ScrollTarget),
    /// Open the new-comment form on a line, prefilled with `body`.
    DraftComment { path: String, side: Side, line: u64, body: String },
    /// Like `ScrollTo`, but the diff view records where it was first, for Back.
    JumpTo(ScrollTarget),
    /// `selected` followed the diff scroll position. Not a `notify`, so views that only
    /// depend on rows skip their rebuild while the user scrolls.
    SelectedFromScroll,
}

pub struct CodeSearch {
    pub query: String,
    /// Matches in the visible files; `CodeMatch::file` indexes `PrModel::visible`.
    pub matches: Vec<CodeMatch>,
    pub active: usize,
    /// A match was jumped to; later steps of this search add no history.
    jumped: bool,
}

pub struct PrModel {
    pub owner: SharedString,
    pub repo: SharedString,
    pub number: u64,
    /// Signed-in login. Passed in: reading `AppRoot` here panics while it opens this screen.
    pub viewer: Option<String>,
    pub pr: Option<PullRequest>,
    /// Files of the current commit selection.
    pub files: Vec<ProcessedFile>,
    /// Indexes into `files` that pass the sidebar filter, in display order.
    pub visible: Vec<usize>,
    all_files: Option<Vec<ProcessedFile>>,
    pub comments: Vec<ReviewComment>,
    pub issue_comments: Vec<IssueComment>,
    pub reviews: Vec<PrReview>,
    /// The viewer's PENDING review on GitHub; its comments are in `comments`.
    pub pending_review: Option<PrReview>,
    resolutions: Vec<ThreadResolutionInfo>,
    /// Inline threads of all files, rebuilt whenever comments change.
    pub threads: Vec<CommentThread>,
    pub reviewed: HashSet<String>,
    pub commits: Vec<Commit>,
    pub selection: CommitSelection,
    pub loading: bool,
    /// The PR failed to load; the screen shows it with a retry.
    pub error: Option<SharedString>,
    pub files_loading: bool,
    /// Changes when a new file set replaces the one shown in the diff.
    pub files_version: u64,
    pub selected: Option<String>,
    pub collapsed: HashSet<String>,
    pub diff_mode: DiffMode,
    pub filter_query: String,
    pub include: String,
    pub exclude: String,
    /// AI reading order and the head commit it was made for.
    reading_order: (String, Vec<ReadingStep>),
    /// Show files in the AI reading order when it is for the PR head.
    pub ai_order: bool,
    pub search: Option<CodeSearch>,
    /// Positions before search and comment jumps, newest last.
    pub history: Vec<Spot>,
    file_lines: HashMap<(String, String), Arc<Vec<String>>>,
    file_lines_pending: HashSet<(String, String)>,
    /// Large files whose diff is being loaded on demand.
    diff_loading: HashSet<String>,
    /// Why the last on-demand load of a large file's diff failed.
    diff_errors: HashMap<String, String>,
    _load: Task<()>,
    _files_load: Option<Task<()>>,
}

impl EventEmitter<PrEvent> for PrModel {}

impl PrModel {
    /// (owner, repo), e.g. for uploading comment attachments.
    pub fn repo_name(&self) -> (SharedString, SharedString) {
        (self.owner.clone(), self.repo.clone())
    }

    pub fn new(
        owner: SharedString,
        repo: SharedString,
        number: u64,
        viewer: Option<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut this = Self {
            owner,
            repo,
            number,
            viewer,
            pr: None,
            files: vec![],
            visible: vec![],
            all_files: None,
            comments: vec![],
            issue_comments: vec![],
            reviews: vec![],
            pending_review: None,
            resolutions: vec![],
            threads: vec![],
            reviewed: HashSet::new(),
            commits: vec![],
            selection: CommitSelection::All,
            loading: true,
            error: None,
            files_loading: false,
            files_version: 0,
            selected: None,
            collapsed: HashSet::new(),
            diff_mode: DiffMode::Split,
            filter_query: String::new(),
            include: String::new(),
            exclude: String::new(),
            reading_order: (String::new(), vec![]),
            ai_order: true,
            search: None,
            history: vec![],
            file_lines: HashMap::new(),
            file_lines_pending: HashSet::new(),
            diff_loading: HashSet::new(),
            diff_errors: HashMap::new(),
            _load: Task::ready(()),
            _files_load: None,
        };
        this.reload(cx);
        this
    }

    /// (Re)load the PR and everything the screen shows.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        self.error = None;
        let (o, r, number) = (self.owner.to_string(), self.repo.to_string(), self.number);
        let viewer = self.viewer.clone();
        let load = backend::call_cancellable(cx, move |s| async move {
            let p = s.provider().await?;
            // join, not try_join: the PR's own error (missing, no access) explains the others.
            let (pr, files, comments, issue_comments, reviews, resolutions, commits) = tokio::join!(
                p.get_pull_request(&o, &r, number),
                fetch_pr_files(&*p, &o, &r, number),
                async { p.get_pr_comments(&o, &r, number).await.map_err(backend::err) },
                async { p.get_issue_comments(&o, &r, number).await.map_err(backend::err) },
                async { p.get_pr_reviews(&o, &r, number).await.map_err(backend::err) },
                async { p.get_thread_resolutions(&o, &r, number).await.map_err(backend::err) },
                async { p.get_pr_commits(&o, &r, number).await.map_err(backend::err) },
            );
            let pr = pr.map_err(|e| pr_load_error(&e, &o, &r, number))?;
            let (files, mut comments, issue_comments, mut reviews, resolutions, commits) =
                (files?, comments?, issue_comments?, reviews?, resolutions?, commits?);
            let pending = take_pending_review(&mut reviews, viewer.as_deref());
            if let Some(review) = &pending {
                let extra = p.get_review_comments(&o, &r, number, review.id).await.map_err(backend::err)?;
                let known: HashSet<u64> = comments.iter().map(|c| c.id).collect();
                comments.extend(extra.into_iter().filter(|c| !known.contains(&c.id)));
            }
            drop(p);
            let reviewed = queries::get_reviewed_files(s.cache.lock().await.conn(), &o, &r, number)
                .map_err(backend::err)?;
            Ok((pr, files, comments, issue_comments, reviews, pending, resolutions, commits, reviewed))
        });
        self._load = cx.spawn(async move |this, cx| {
            let result = load.await;
            this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok((pr, files, comments, issue_comments, reviews, pending, resolutions, commits, reviewed)) => {
                        let same_head = this.pr.as_ref().is_some_and(|old| old.head_sha == pr.head_sha);
                        let same_revision = this.pr.as_ref().is_some_and(|old| old.head_sha == pr.head_sha && old.base_sha == pr.base_sha);
                        let selected = same_head.then(|| this.selected.clone()).flatten();
                        let collapsed = same_head.then(|| this.collapsed.clone());
                        this.pr = Some(pr);
                        this.comments = comments;
                        this.issue_comments = issue_comments;
                        this.reviews = reviews;
                        this.pending_review = pending;
                        this.resolutions = resolutions;
                        this.commits = commits;
                        this.reviewed = reviewed.into_iter().collect();
                        this.rebuild_threads();
                        if !same_revision {
                            this.selection = CommitSelection::All;
                            this.all_files = None;
                            this.diff_loading.clear();
                            this.set_files(files, cx);
                            if let Some(path) = selected.filter(|path| this.files.iter().any(|file| &file.filename == path)) {
                                this.selected = Some(path);
                            }
                            // Keep local expansion choices when the PR head did not change.
                            this.collapsed = collapsed.unwrap_or_else(|| {
                                let generated = this.files.iter().map(|f| &f.filename).filter(|p| files::is_generated(p));
                                this.reviewed.iter().chain(generated).cloned().collect()
                            });
                        }
                    }
                    Err(e) if this.pr.is_some() => notify_error(format!("Failed to refresh PR: {e}"), cx),
                    Err(e) => this.error = Some(e.into()),
                }
                cx.notify();
            })
            .ok();
        });
        cx.notify();
    }

    fn set_files(&mut self, files: Vec<ProcessedFile>, cx: &mut Context<Self>) {
        self.files_version += 1;
        // Recorded rows and load failures belong to the old file set.
        self.history.clear();
        self.diff_errors.clear();
        self.files = files;
        self.refilter(cx);
        let first = self.visible.first().copied().unwrap_or(0);
        self.selected = self.files.get(first).map(|f| f.filename.clone());
    }

    fn refilter(&mut self, cx: &mut Context<Self>) {
        let mut visible = files::filter_files(&self.files, &self.filter_query, &self.include, &self.exclude);
        if let Some(order) = self.reading_order() {
            ai_review::sort_by_reading_order(&mut visible, &self.files, order);
        }
        self.visible = visible;
        self.research();
        cx.notify();
    }

    /// The AI reading order for the PR head, when there is one.
    pub fn has_reading_order(&self) -> bool {
        let (head, order) = &self.reading_order;
        !order.is_empty() && self.pr.as_ref().is_some_and(|pr| pr.head_sha == *head)
    }

    /// The reading order the files are shown in, if any.
    pub fn reading_order(&self) -> Option<&[ReadingStep]> {
        (self.ai_order && self.has_reading_order()).then_some(&self.reading_order.1)
    }

    /// Step number and reason of `path` in the shown reading order.
    pub fn reading_step(&self, path: &str) -> Option<(usize, &str)> {
        let (i, step) = self.reading_order()?.iter().enumerate().find(|(_, s)| s.file == path)?;
        Some((i + 1, &step.reason))
    }

    pub fn set_reading_order(&mut self, head: String, order: Vec<ReadingStep>, cx: &mut Context<Self>) {
        self.reading_order = (head, order);
        self.reorder(cx);
    }

    pub fn toggle_ai_order(&mut self, cx: &mut Context<Self>) {
        self.ai_order = !self.ai_order;
        self.reorder(cx);
    }

    /// Apply a changed file order and keep the selected file in view.
    fn reorder(&mut self, cx: &mut Context<Self>) {
        self.refilter(cx);
        if let Some(path) = self.selected.clone() {
            cx.emit(PrEvent::ScrollTo(ScrollTarget::File(path)));
        }
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
        self.diff_loading.clear();
        if selection == CommitSelection::All {
            let files = self.all_files.clone().unwrap_or_default();
            self._files_load = None;
            self.files_loading = false;
            self.set_files(files, cx);
            return;
        }
        let (Some(base), Some(head)) = (self.base_sha(), self.head_sha()) else {
            return;
        };
        let (base, head) = (base.to_string(), head.to_string());
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
        self.open(path, ScrollTarget::File(path.to_string()), cx);
    }

    /// Clear sidebar filters, open the target's file and scroll to it.
    pub fn reveal(&mut self, path: &str, target: ScrollTarget, cx: &mut Context<Self>) {
        // Open first: its jump event must reach the diff before the refilter rebuilds the rows.
        let hidden = !self.visible.iter().any(|&i| self.files[i].filename == path);
        self.open(path, target.clone(), cx);
        if hidden {
            self.set_filter(String::new(), String::new(), String::new(), cx);
            // The first jump records history. Now the target has a row to scroll to.
            cx.emit(PrEvent::ScrollTo(target));
        }
    }

    /// Select and expand `path`, then scroll to `target` in it.
    pub fn open(&mut self, path: &str, target: ScrollTarget, cx: &mut Context<Self>) {
        self.selected = Some(path.to_string());
        self.collapsed.remove(path);
        cx.emit(PrEvent::JumpTo(target));
        cx.notify();
    }

    /// Add `spot` to the navigation history.
    pub fn push_history(&mut self, spot: Spot, cx: &mut Context<Self>) {
        if self.history.last() != Some(&spot) {
            if self.history.len() == HISTORY_MAX {
                self.history.remove(0);
            }
            self.history.push(spot);
            cx.notify();
        }
    }

    /// Return to the last recorded position. `false` when the history is empty.
    pub fn go_back(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(spot) = self.history.pop() else { return false };
        let path = spot.path.clone();
        if !self.visible.iter().any(|&i| self.files[i].filename == path) {
            self.set_filter(String::new(), String::new(), String::new(), cx);
        }
        self.selected = Some(path.clone());
        self.collapsed.remove(&path);
        cx.emit(PrEvent::ScrollTo(ScrollTarget::Spot(spot)));
        cx.notify();
        true
    }

    /// Diff scroll position moved into `path`; only updates the sidebar highlight.
    pub fn set_selected_from_scroll(&mut self, path: &str, cx: &mut Context<Self>) {
        if self.selected.as_deref() != Some(path) {
            self.selected = Some(path.to_string());
            cx.emit(PrEvent::SelectedFromScroll);
        }
    }

    /// Position of the selected file in `visible`.
    fn selected_visible(&self) -> Option<usize> {
        let path = self.selected.as_ref()?;
        self.visible.iter().position(|&i| &self.files[i].filename == path)
    }

    /// Step to the previous (-1) or next (1) visible file.
    pub fn navigate_file(&mut self, direction: isize, cx: &mut Context<Self>) {
        let current = self.selected_visible();
        if let Some(next) = files::navigate_file(self.visible.len(), current, direction) {
            let path = self.visible_file(next).filename.clone();
            self.select_file(&path, cx);
        }
    }

    /// Step to the next or previous visible file not marked viewed.
    pub fn navigate_unreviewed(&mut self, forward: bool, cx: &mut Context<Self>) {
        let current = self.selected_visible();
        let unreviewed = |i: usize| !self.reviewed.contains(&self.visible_file(i).filename);
        if let Some(next) = files::find_wrapping(self.visible.len(), current, forward, unreviewed) {
            let path = self.visible_file(next).filename.clone();
            self.select_file(&path, cx);
        }
    }

    /// Toggle viewed on the selected file. A newly viewed file collapses; pin its header so the next file follows.
    pub fn toggle_selected_reviewed(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.selected.clone() else { return };
        self.toggle_reviewed(&path, cx);
        if self.reviewed.contains(&path) {
            cx.emit(PrEvent::ScrollTo(ScrollTarget::File(path)));
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

    /// Optimistic toggle; rolls back if saving fails. Viewed files collapse, unviewed ones expand.
    pub fn toggle_reviewed(&mut self, path: &str, cx: &mut Context<Self>) {
        let was = !self.reviewed.insert(path.to_string());
        if was {
            self.reviewed.remove(path);
            self.collapsed.remove(path);
        } else {
            self.collapsed.insert(path.to_string());
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

    pub fn set_pending_review(&mut self, review: Option<PrReview>, cx: &mut Context<Self>) {
        self.pending_review = review;
        cx.notify();
    }

    pub fn is_pending(&self, comment: &ReviewComment) -> bool {
        self.pending_review.as_ref().is_some_and(|p| comment.review_id == Some(p.id))
    }

    /// Comments in the viewer's pending review.
    pub fn pending_count(&self) -> usize {
        self.comments.iter().filter(|c| self.is_pending(c)).count()
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
            self.search = Some(CodeSearch { query: String::new(), matches: vec![], active: 0, jumped: false });
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
        s.matches = search::search_files(self.visible.iter().map(|&i| &self.files[i]), &s.query);
        s.active = s.active.min(s.matches.len().saturating_sub(1));
    }

    /// Scroll to the active match. Only the first jump of a search is recorded, so Back returns to where it started.
    fn reveal_match(&mut self, cx: &mut Context<Self>) {
        let Some(m) = self.active_match() else { return };
        let path = self.visible_file(m.file).filename.clone();
        self.collapsed.remove(&path);
        self.selected = Some(path);
        let Some(s) = &mut self.search else { return };
        let first = !std::mem::replace(&mut s.jumped, true);
        cx.emit(if first { PrEvent::JumpTo(ScrollTarget::Match) } else { PrEvent::ScrollTo(ScrollTarget::Match) });
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

    /// Base commit of the current commit selection.
    // ponytail: for All this is the base branch tip, not the merge base, so base-branch changes show too;
    // read `merge_base_commit` from the compare API if that confuses.
    pub fn base_sha(&self) -> Option<&str> {
        let pr = self.pr.as_ref()?;
        let first = match self.selection {
            CommitSelection::All => return Some(&pr.base_sha),
            CommitSelection::Single(i) | CommitSelection::Range(i, _) => self.commits.get(i)?,
        };
        Some(first.parents.first().unwrap_or(&pr.base_sha))
    }

    /// Fetch both versions of a large file and diff them locally, since GitHub sends no patch for it.
    pub fn load_diff(&mut self, path: &str, cx: &mut Context<Self>) {
        let (Some(base), Some(head)) = (self.base_sha(), self.head_sha()) else { return };
        let Some(file) = self.files.iter().find(|f| f.filename == path) else { return };
        let (base, head, status) = (base.to_string(), head.to_string(), file.status.clone());
        let old_path = file.previous_filename.clone().unwrap_or_else(|| path.to_string());
        if !self.diff_loading.insert(path.to_string()) {
            return;
        }
        self.diff_errors.remove(path);
        let (o, r, p) = (self.owner.to_string(), self.repo.to_string(), path.to_string());
        let fetch = backend::call(cx, {
            let head = head.clone();
            move |s| async move {
                let provider = s.provider().await?;
                let old = match status.as_str() {
                    "added" => String::new(),
                    _ => provider.get_file_content(&o, &r, &old_path, &base).await.map_err(backend::err)?,
                };
                let new = match status.as_str() {
                    "removed" => String::new(),
                    _ => provider.get_file_content(&o, &r, &p, &head).await.map_err(backend::err)?,
                };
                Ok(diff_contents(&old, &new))
            }
        });
        let path = path.to_string();
        cx.spawn(async move |this, cx| {
            let result = fetch.await;
            this.update(cx, |this, cx| {
                if !this.diff_loading.remove(&path) || this.head_sha() != Some(head.as_str()) {
                    // The commit selection changed meanwhile.
                    return;
                }
                match result {
                    Ok(diff) => {
                        let all = (this.selection == CommitSelection::All).then_some(&mut this.all_files);
                        for files in [Some(&mut this.files), all.and_then(Option::as_mut)].into_iter().flatten() {
                            if let Some(f) = files.iter_mut().find(|f| f.filename == path) {
                                f.diff = Some(diff.clone());
                            }
                        }
                        this.research();
                    }
                    Err(e) => {
                        this.diff_errors.insert(path, e);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn diff_loading(&self, path: &str) -> bool {
        self.diff_loading.contains(path)
    }

    pub fn diff_error(&self, path: &str) -> Option<&str> {
        self.diff_errors.get(path).map(String::as_str)
    }

    /// Cached lines of `path` at `git_ref`, without fetching.
    pub fn cached_file_lines(&self, path: &str, git_ref: &str) -> Option<Arc<Vec<String>>> {
        self.file_lines.get(&(path.to_string(), git_ref.to_string())).cloned()
    }

    pub fn file_lines_loading(&self, path: &str, git_ref: &str) -> bool {
        self.file_lines_pending.contains(&(path.to_string(), git_ref.to_string()))
    }
}

/// Remove and return `viewer`'s PENDING review. GitHub shows a pending review only to its author.
fn take_pending_review(reviews: &mut Vec<PrReview>, viewer: Option<&str>) -> Option<PrReview> {
    let i = reviews.iter().position(|r| r.state == "PENDING" && Some(r.author_login.as_str()) == viewer)?;
    Some(reviews.remove(i))
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
