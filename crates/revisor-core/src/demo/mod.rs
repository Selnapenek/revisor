//! Sample mode: a bundled repository with pull requests, served through the normal `GitProvider` path,
//! so every feature works without GitHub sign-in or an AI CLI. Writes stay in memory until the sample ends.

use crate::ai_review::{self, Review};
use crate::cache::queries;
use crate::provider::{types::*, GitProvider};
use crate::AppState;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{TimeDelta, Utc};
use rusqlite::Connection;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Mutex;

pub const OWNER: &str = "revisor-demo";
pub const REPO: &str = "shop";
/// The pull request the getting-started checklist walks through.
pub const PR: u64 = 42;
const LOGIN: &str = "you";
const BASE_SHA: &str = "4e1c0a7b9d2f63e8a5c1b0d9e7f6a3c2b1d0e9f8";
const REVIEW: &str = include_str!("sample/review.json");

pub fn is_sample(owner: &str) -> bool {
    owner == OWNER
}

pub fn user() -> ProviderUser {
    ProviderUser { login: LOGIN.into(), avatar_url: String::new(), name: Some("You".into()) }
}

/// Start the sample with fresh data and return its user.
pub async fn start(s: &AppState) -> Result<ProviderUser, String> {
    clear(s.cache.lock().await.conn()).map_err(|e| e.to_string())?;
    *s.provider.write().await = Some(Box::new(DemoProvider::new()));
    Ok(user())
}

pub async fn stop(s: &AppState) -> Result<(), String> {
    *s.provider.write().await = None;
    clear(s.cache.lock().await.conn()).map_err(|e| e.to_string())
}

/// Forget what the sample saved: viewed files, AI reviews, filters, views.
pub fn clear(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(&format!(
        "DELETE FROM settings WHERE key LIKE '%{OWNER}/{REPO}%';
         DELETE FROM reviewed_files WHERE owner = '{OWNER}';
         DELETE FROM pr_views WHERE owner = '{OWNER}';
         DELETE FROM saved_repos WHERE owner = '{OWNER}';"
    ))
}

/// Getting-started steps done on the sample PR, read from the data the steps leave behind.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Progress {
    pub ai: bool,
    pub viewed: bool,
    pub commented: bool,
    pub submitted: bool,
}

pub async fn progress(s: &AppState) -> Result<Progress, String> {
    let (comments, reviews) = {
        let p = s.provider().await?;
        let (comments, reviews) = tokio::join!(p.get_pr_comments(OWNER, REPO, PR), p.get_pr_reviews(OWNER, REPO, PR));
        (comments.map_err(|e| e.to_string())?, reviews.map_err(|e| e.to_string())?)
    };
    let db = s.cache.lock().await;
    let ai = queries::get_setting(db.conn(), &ai_review::review_key(OWNER, REPO, PR)).map_err(|e| e.to_string())?;
    let viewed = queries::get_reviewed_files(db.conn(), OWNER, REPO, PR).map_err(|e| e.to_string())?;
    Ok(Progress {
        ai: ai.is_some(),
        viewed: !viewed.is_empty(),
        commented: comments.iter().any(|c| c.author_login == LOGIN),
        submitted: reviews.iter().any(|r| r.author_login == LOGIN && r.state != "PENDING"),
    })
}

/// The recorded AI review instead of a CLI run. Waits a moment so the run is visible.
pub async fn replay_review(number: u64, head_sha: String) -> Review {
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    let mut review = if number == PR {
        serde_json::from_str(REVIEW).expect("bundled sample review is valid")
    } else {
        Review {
            summary: format!("The sample has a recorded AI review for #{PR} only. Open #{PR} to see findings and suggested comments."),
            evidence: String::new(),
            danger: Default::default(),
            spec: Default::default(),
            standards_check: Default::default(),
            standards_status: String::new(),
            comments: vec![],
            reading_order: vec![],
            head_sha: String::new(),
            backend: String::new(),
            standards: vec![],
            skipped: vec![],
            seconds: 0,
            cost_usd: None,
        }
    };
    review.head_sha = head_sha;
    review.backend = "Sample replay".into();
    review.seconds = 2;
    review
}

// --- Fixtures ---

type Files = &'static [(&'static str, Option<&'static str>)];

/// `(path, contents)` of a fixture file under `sample/<dir>/`.
macro_rules! file {
    ($dir:literal, $path:literal) => {
        ($path, Some(include_str!(concat!("sample/", $dir, "/", $path))))
    };
}

const BASE: Files = &[
    file!("base", "CONTRIBUTING.md"),
    file!("base", "README.md"),
    file!("base", "package-lock.json"),
    file!("base", "package.json"),
    file!("base", "src/cart.ts"),
    file!("base", "src/checkout.ts"),
    file!("base", "src/legacy/priceUtils.ts"),
    file!("base", "src/money.ts"),
    file!("base", "tests/cart.test.ts"),
];

struct CommitFixture {
    sha: &'static str,
    message: &'static str,
    hours_ago: i64,
    /// Changed files; `None` deletes the file.
    files: Files,
}

struct PrFixture {
    number: u64,
    title: &'static str,
    body: &'static str,
    author: &'static str,
    branch: &'static str,
    labels: &'static [&'static str],
    draft: bool,
    hours_ago: i64,
    assignees: &'static [&'static str],
    reviewers: &'static [&'static str],
    commits: &'static [CommitFixture],
}

const LABELS: [(&str, &str); 3] = [("feature", "1f883d"), ("bug", "d73a4a"), ("enhancement", "a2eeef")];

const PRS: &[PrFixture] = &[
    PrFixture {
        number: 42,
        title: "Add coupon codes to checkout",
        body: "Adds coupon codes to checkout for the spring sale.\n\n\
               - Customers enter a code at checkout. Codes are **case-insensitive**.\n\
               - Percent and fixed-amount coupons.\n\
               - Tax is charged on the discounted price.\n\
               - Expired coupons are ignored.\n\n\
               Also moves the money helpers to `src/lib` and removes the deprecated `legacy/priceUtils`.",
        author: "mira-dev",
        branch: "coupons",
        labels: &["feature"],
        draft: false,
        hours_ago: 3,
        assignees: &["mira-dev"],
        reviewers: &[LOGIN, "sam-reviewer"],
        commits: &[
            CommitFixture {
                sha: "b7d24e91c0a3f58e6d1b2c4a9e8f7d6c5b4a3921",
                message: "Move money helpers to src/lib",
                hours_ago: 30,
                files: &[
                    ("src/money.ts", None),
                    file!("pr42-1", "src/lib/money.ts"),
                    file!("pr42-1", "src/cart.ts"),
                    file!("pr42-1", "src/checkout.ts"),
                ],
            },
            CommitFixture {
                sha: "c3a91f0e2b7d4c58a6e1f9b0d2c3e4f5a6b7c8d9",
                message: "Add coupon codes",
                hours_ago: 28,
                files: &[
                    file!("pr42-2", "src/coupons.ts"),
                    file!("pr42-2", "src/cart.ts"),
                    file!("pr42-2", "src/checkout.ts"),
                    file!("pr42-2", "tests/coupons.test.ts"),
                    file!("pr42-2", "tests/cart.test.ts"),
                    file!("pr42-2", "package.json"),
                    file!("pr42-2", "package-lock.json"),
                    ("src/legacy/priceUtils.ts", None),
                ],
            },
            CommitFixture {
                sha: "d58e3b7a1c9f20e4b6a8d0c2e1f3a5b7c9d1e3f5",
                message: "Keep coupons valid through their last day",
                hours_ago: 3,
                files: &[file!("pr42-3", "src/coupons.ts")],
            },
        ],
    },
    PrFixture {
        number: 41,
        title: "Format negative amounts with a leading minus",
        body: "Refunds showed `($5.00)` in some locales. Put the sign in front ourselves.",
        author: "sam-reviewer",
        branch: "negative-amounts",
        labels: &["bug"],
        draft: false,
        hours_ago: 26,
        assignees: &[],
        reviewers: &["mira-dev"],
        commits: &[CommitFixture {
            sha: "e9f1a2b3c4d5e6f708192a3b4c5d6e7f80912a3b",
            message: "Format negative amounts with a leading minus",
            hours_ago: 27,
            files: &[file!("pr41-1", "src/money.ts")],
        }],
    },
    PrFixture {
        number: 40,
        title: "Add currency setting to checkout",
        body: "Work in progress: lets the shop pass a currency. Rates come later.",
        author: "mira-dev",
        branch: "currency",
        labels: &["enhancement"],
        draft: true,
        hours_ago: 75,
        assignees: &["mira-dev"],
        reviewers: &[],
        commits: &[CommitFixture {
            sha: "f0a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3",
            message: "Pass currency through checkout",
            hours_ago: 76,
            files: &[file!("pr40-1", "src/checkout.ts")],
        }],
    },
];

fn ago(hours: i64) -> String {
    (Utc::now() - TimeDelta::hours(hours)).format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn label(name: &str) -> Label {
    let color = LABELS.iter().find(|(n, _)| *n == name).map_or("ededed", |(_, c)| c);
    Label { name: name.into(), color: color.into() }
}

fn person(login: &str) -> PrUser {
    PrUser { login: login.into(), avatar_url: String::new() }
}

fn not_found() -> anyhow::Error {
    anyhow!("Not Found (404)")
}

/// Content hash in place of a git blob SHA.
fn blob(text: &str) -> String {
    let mut h = DefaultHasher::new();
    text.hash(&mut h);
    format!("{:016x}", h.finish())
}

fn changed_file(path: &str, previous: Option<&str>, status: &str, old: &str, new: &str) -> PrFile {
    let patch = similar::TextDiff::from_lines(old, new).unified_diff().context_radius(3).to_string();
    let count = |sign| patch.lines().filter(|l| l.starts_with(sign)).count() as u64;
    PrFile {
        sha: blob(new),
        filename: path.into(),
        previous_filename: previous.map(Into::into),
        status: status.into(),
        additions: count('+'),
        deletions: count('-'),
        patch: Some(patch),
    }
}

/// In-memory comments, reviews and thread states; starts with the fixture conversation.
struct Store {
    next_id: u64,
    /// `(pr number, comment)`
    comments: Vec<(u64, ReviewComment)>,
    issue_comments: Vec<(u64, IssueComment)>,
    reviews: Vec<(u64, PrReview)>,
    resolutions: Vec<(u64, ThreadResolutionInfo)>,
}

impl Store {
    fn seed() -> Self {
        let mut s = Store { next_id: 100, comments: vec![], issue_comments: vec![], reviews: vec![], resolutions: vec![] };
        let review = s.review(42, "sam-reviewer", "COMMENTED", "Close. Two notes inline.", Some(ago(20)));
        let open = s.comment(42, "sam-reviewer", "src/checkout.ts", 7, None, Some(review),
            "What does the customer see when the code is unknown or expired? Right now it is ignored without a message.", 20);
        s.resolutions.push((42, ThreadResolutionInfo { root_comment_id: open, thread_node_id: format!("T_{open}"), is_resolved: false }));
        let resolved = s.comment(42, "sam-reviewer", "src/coupons.ts", 17, None, Some(review),
            "A coupon that expires on 2026-06-01 stops at midnight of that day. Customers expect it to work the whole day.", 20);
        s.comment(42, "mira-dev", "src/coupons.ts", 17, Some(resolved), None, "Good catch. Fixed in the last commit.", 3);
        s.resolutions.push((42, ThreadResolutionInfo { root_comment_id: resolved, thread_node_id: format!("T_{resolved}"), is_resolved: true }));
        let id = s.id();
        s.issue_comments.push((42, IssueComment {
            id,
            body: "Product sign-off: SPRING10 must be live before the sale starts. Thanks!".into(),
            author_login: "lena-pm".into(),
            author_avatar: String::new(),
            created_at: ago(26),
            updated_at: ago(26),
            html_url: String::new(),
        }));
        s.review(41, "mira-dev", "APPROVED", "", Some(ago(24)));
        s
    }

    fn id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    fn review(&mut self, pr: u64, author: &str, state: &str, body: &str, submitted_at: Option<String>) -> u64 {
        let id = self.id();
        self.reviews.push((pr, PrReview {
            id,
            node_id: format!("R_{id}"),
            body: body.into(),
            state: state.into(),
            author_login: author.into(),
            author_avatar: String::new(),
            submitted_at,
            html_url: String::new(),
        }));
        id
    }

    #[allow(clippy::too_many_arguments)]
    fn comment(&mut self, pr: u64, author: &str, path: &str, line: u64, reply_to: Option<u64>, review: Option<u64>, body: &str, hours_ago: i64) -> u64 {
        let id = self.id();
        self.comments.push((pr, ReviewComment {
            id,
            path: path.into(),
            line: Some(line),
            original_line: Some(line),
            side: Some("RIGHT".into()),
            diff_hunk: String::new(),
            body: body.into(),
            author_login: author.into(),
            author_avatar: String::new(),
            created_at: ago(hours_ago),
            updated_at: ago(hours_ago),
            in_reply_to_id: reply_to,
            review_id: review,
            html_url: String::new(),
            outdated: false,
        }));
        id
    }

    /// A new comment by the sample user, with a thread to resolve when it starts one.
    fn add(&mut self, pr: u64, params: NewComment, reply_to: Option<u64>, review: Option<u64>) -> ReviewComment {
        let line = Some(params.line);
        let id = self.id();
        let comment = ReviewComment {
            id,
            path: params.path,
            line,
            original_line: line,
            side: Some(params.side),
            diff_hunk: String::new(),
            body: params.body,
            author_login: LOGIN.into(),
            author_avatar: String::new(),
            created_at: ago(0),
            updated_at: ago(0),
            in_reply_to_id: reply_to,
            review_id: review,
            html_url: String::new(),
            outdated: false,
        };
        if reply_to.is_none() {
            self.resolutions.push((pr, ThreadResolutionInfo { root_comment_id: id, thread_node_id: format!("T_{id}"), is_resolved: false }));
        }
        self.comments.push((pr, comment.clone()));
        comment
    }

    fn comment_mut(&mut self, id: u64) -> Result<&mut ReviewComment> {
        self.comments.iter_mut().map(|(_, c)| c).find(|c| c.id == id).ok_or_else(not_found)
    }
}

pub struct DemoProvider {
    /// Commit SHA to its files.
    snapshots: HashMap<&'static str, BTreeMap<&'static str, &'static str>>,
    store: Mutex<Store>,
}

impl DemoProvider {
    pub fn new() -> Self {
        let apply = |tree: &mut BTreeMap<&'static str, &'static str>, files: Files| {
            for &(path, text) in files {
                match text {
                    Some(text) => tree.insert(path, text),
                    None => tree.remove(path),
                };
            }
        };
        let mut base = BTreeMap::new();
        apply(&mut base, BASE);
        let mut snapshots = HashMap::from([(BASE_SHA, base.clone())]);
        for pr in PRS {
            let mut tree = base.clone();
            for commit in pr.commits {
                apply(&mut tree, commit.files);
                snapshots.insert(commit.sha, tree.clone());
            }
        }
        Self { snapshots, store: Mutex::new(Store::seed()) }
    }

    fn store(&self) -> std::sync::MutexGuard<'_, Store> {
        self.store.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn fixture(&self, owner: &str, repo: &str, number: u64) -> Result<&'static PrFixture> {
        if owner != OWNER || repo != REPO {
            return Err(not_found());
        }
        PRS.iter().find(|p| p.number == number).ok_or_else(not_found)
    }

    /// Files at a commit SHA or a branch name.
    fn snapshot(&self, git_ref: &str) -> Result<&BTreeMap<&'static str, &'static str>> {
        let sha = match git_ref {
            "main" => BASE_SHA,
            r => PRS.iter().find(|p| p.branch == r).and_then(|p| p.commits.last()).map_or(r, |c| c.sha),
        };
        self.snapshots.get(sha).ok_or_else(not_found)
    }

    fn compare(&self, base: &str, head: &str) -> Result<Vec<PrFile>> {
        let (old, new) = (self.snapshot(base)?, self.snapshot(head)?);
        let name = |path: &str| path.rsplit('/').next().unwrap_or(path).to_string();
        let mut removed: Vec<&str> = old.keys().filter(|p| !new.contains_key(*p)).copied().collect();
        let mut files = Vec::new();
        for (&path, &text) in new {
            match old.get(path) {
                Some(&before) if before == text => {}
                Some(&before) => files.push(changed_file(path, None, "modified", before, text)),
                // ponytail: a rename is a removed and an added file with the same name; enough for the fixture.
                None => match removed.iter().position(|p| name(p) == name(path)) {
                    Some(i) => {
                        let from = removed.remove(i);
                        files.push(changed_file(path, Some(from), "renamed", old[from], text));
                    }
                    None => files.push(changed_file(path, None, "added", "", text)),
                },
            }
        }
        files.extend(removed.into_iter().map(|path| changed_file(path, None, "removed", old[path], "")));
        files.sort_by(|a, b| a.filename.cmp(&b.filename));
        Ok(files)
    }

    fn pull_request(&self, pr: &PrFixture) -> Result<PullRequest> {
        let head = pr.commits.last().ok_or_else(not_found)?.sha;
        let files = self.compare(BASE_SHA, head)?;
        let store = self.store();
        let review_comments = store.comments.iter().filter(|(n, _)| *n == pr.number).count() as u64;
        let comments = store.issue_comments.iter().filter(|(n, _)| *n == pr.number).count() as u64;
        Ok(PullRequest {
            id: 1000 + pr.number,
            number: pr.number,
            title: pr.title.into(),
            body: Some(pr.body.into()),
            state: "open".into(),
            draft: pr.draft,
            author_login: pr.author.into(),
            author_avatar: String::new(),
            labels: pr.labels.iter().map(|l| label(l)).collect(),
            created_at: ago(pr.commits[0].hours_ago),
            updated_at: ago(pr.hours_ago),
            merged_at: None,
            comments_count: comments,
            review_comments_count: review_comments,
            additions: files.iter().map(|f| f.additions).sum(),
            deletions: files.iter().map(|f| f.deletions).sum(),
            changed_files: files.len() as u64,
            head_sha: head.into(),
            base_sha: BASE_SHA.into(),
            head_ref: pr.branch.into(),
            base_ref: "main".into(),
            html_url: String::new(),
            assignees: pr.assignees.iter().map(|l| person(l)).collect(),
            requested_reviewers: pr.reviewers.iter().map(|l| person(l)).collect(),
        })
    }

    /// The PR list filters that the GitHub search query would apply.
    fn matches(&self, pr: &PrFixture, filters: &PrSearchFilters, query: &str) -> bool {
        let store = self.store();
        let reviews: Vec<&PrReview> = store.reviews.iter().filter(|(n, _)| *n == pr.number).map(|(_, r)| r).collect();
        let by_me = reviews.iter().any(|r| r.author_login == LOGIN && r.state != "PENDING");
        let has = |state: &str| reviews.iter().any(|r| r.state == state);
        let query = query.to_lowercase();
        (query.is_empty() || pr.title.to_lowercase().contains(&query) || query.trim_start_matches('#') == pr.number.to_string())
            && (filters.authors.is_empty() || filters.authors.iter().any(|a| a == pr.author))
            && filters.labels.iter().all(|l| pr.labels.contains(&l.as_str()))
            && (filters.assignees.is_empty() || filters.assignees.iter().any(|a| pr.assignees.contains(&a.as_str())))
            && match filters.review_status.as_deref() {
                None => true,
                Some("none") => reviews.is_empty(),
                Some("required") => !has("APPROVED"),
                Some("approved") => has("APPROVED"),
                Some("changes_requested") => has("CHANGES_REQUESTED"),
                Some("reviewed-by:@me") => by_me,
                Some("-reviewed-by:@me") => !by_me,
                Some(_) => pr.reviewers.contains(&LOGIN),
            }
    }
}

impl Default for DemoProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GitProvider for DemoProvider {
    async fn validate_token(&self) -> Result<ProviderUser> {
        Ok(user())
    }

    async fn list_pull_requests(&self, owner: &str, repo: &str, state: PrState, page: u32, per_page: u32) -> Result<PaginatedResult<PullRequest>> {
        self.search_pull_requests(owner, repo, state, &PrSearchFilters::default(), "", page, per_page).await
    }

    async fn get_pull_request(&self, owner: &str, repo: &str, number: u64) -> Result<PullRequest> {
        self.pull_request(self.fixture(owner, repo, number)?)
    }

    async fn get_pr_files(&self, owner: &str, repo: &str, number: u64, _page: u32) -> Result<PaginatedResult<PrFile>> {
        let pr = self.fixture(owner, repo, number)?;
        let items = self.compare(BASE_SHA, pr.commits.last().ok_or_else(not_found)?.sha)?;
        Ok(PaginatedResult { total_count: Some(items.len() as u64), items, page: 1, has_next_page: false })
    }

    async fn get_pr_comments(&self, owner: &str, repo: &str, number: u64) -> Result<Vec<ReviewComment>> {
        self.fixture(owner, repo, number)?;
        Ok(self.store().comments.iter().filter(|(n, _)| *n == number).map(|(_, c)| c.clone()).collect())
    }

    async fn get_issue_comments(&self, owner: &str, repo: &str, number: u64) -> Result<Vec<IssueComment>> {
        self.fixture(owner, repo, number)?;
        Ok(self.store().issue_comments.iter().filter(|(n, _)| *n == number).map(|(_, c)| c.clone()).collect())
    }

    async fn get_pr_reviews(&self, owner: &str, repo: &str, number: u64) -> Result<Vec<PrReview>> {
        self.fixture(owner, repo, number)?;
        Ok(self.store().reviews.iter().filter(|(n, _)| *n == number).map(|(_, r)| r.clone()).collect())
    }

    async fn add_comment(&self, owner: &str, repo: &str, number: u64, params: NewComment) -> Result<ReviewComment> {
        self.fixture(owner, repo, number)?;
        Ok(self.store().add(number, params, None, None))
    }

    async fn create_pending_review(&self, owner: &str, repo: &str, number: u64, _commit_id: &str) -> Result<PrReview> {
        self.fixture(owner, repo, number)?;
        let mut store = self.store();
        let id = store.review(number, LOGIN, "PENDING", "", None);
        Ok(store.reviews.iter().find(|(_, r)| r.id == id).map(|(_, r)| r.clone()).ok_or_else(not_found)?)
    }

    async fn add_review_comment(&self, review_node_id: &str, params: NewComment) -> Result<ReviewComment> {
        let mut store = self.store();
        let (pr, id) = store.reviews.iter().find(|(_, r)| r.node_id == review_node_id).map(|(n, r)| (*n, r.id)).ok_or_else(not_found)?;
        Ok(store.add(pr, params, None, Some(id)))
    }

    async fn get_review_comments(&self, owner: &str, repo: &str, number: u64, review_id: u64) -> Result<Vec<ReviewComment>> {
        self.fixture(owner, repo, number)?;
        Ok(self.store().comments.iter().filter(|(_, c)| c.review_id == Some(review_id)).map(|(_, c)| c.clone()).collect())
    }

    async fn submit_review(&self, owner: &str, repo: &str, number: u64, pending_id: Option<u64>, event: ReviewEvent, body: &str) -> Result<()> {
        self.fixture(owner, repo, number)?;
        let state = match event {
            ReviewEvent::Comment => "COMMENTED",
            ReviewEvent::Approve => "APPROVED",
            ReviewEvent::RequestChanges => "CHANGES_REQUESTED",
        };
        let mut store = self.store();
        match pending_id {
            Some(id) => {
                let (_, review) = store.reviews.iter_mut().find(|(_, r)| r.id == id).ok_or_else(not_found)?;
                (review.state, review.body, review.submitted_at) = (state.into(), body.into(), Some(ago(0)));
            }
            None => {
                store.review(number, LOGIN, state, body, Some(ago(0)));
            }
        }
        Ok(())
    }

    async fn delete_pending_review(&self, owner: &str, repo: &str, number: u64, review_id: u64) -> Result<()> {
        self.fixture(owner, repo, number)?;
        let mut store = self.store();
        let removed: HashSet<u64> = store.comments.iter().filter(|(_, c)| c.review_id == Some(review_id)).map(|(_, c)| c.id).collect();
        store.comments.retain(|(_, c)| !removed.contains(&c.id));
        store.resolutions.retain(|(_, r)| !removed.contains(&r.root_comment_id));
        store.reviews.retain(|(_, r)| r.id != review_id);
        Ok(())
    }

    async fn reply_to_comment(&self, owner: &str, repo: &str, number: u64, comment_id: u64, body: &str) -> Result<ReviewComment> {
        self.fixture(owner, repo, number)?;
        let mut store = self.store();
        let parent = store.comment_mut(comment_id)?.clone();
        let params = NewComment {
            path: parent.path,
            line: parent.line.or(parent.original_line).unwrap_or(1),
            side: parent.side.unwrap_or_else(|| "RIGHT".into()),
            body: body.into(),
            commit_id: String::new(),
            start_line: None,
        };
        Ok(store.add(number, params, Some(parent.in_reply_to_id.unwrap_or(comment_id)), None))
    }

    async fn update_comment(&self, _owner: &str, _repo: &str, comment_id: u64, body: &str) -> Result<ReviewComment> {
        let mut store = self.store();
        let comment = store.comment_mut(comment_id)?;
        (comment.body, comment.updated_at) = (body.into(), ago(0));
        Ok(comment.clone())
    }

    async fn delete_comment(&self, _owner: &str, _repo: &str, comment_id: u64) -> Result<()> {
        self.store().comments.retain(|(_, c)| c.id != comment_id);
        Ok(())
    }

    async fn list_user_repos(&self, _page: u32, _per_page: u32) -> Result<PaginatedResult<RepoInfo>> {
        let repo = RepoInfo {
            owner: OWNER.into(),
            name: REPO.into(),
            full_name: format!("{OWNER}/{REPO}"),
            description: Some("Sample repository bundled with Revisor".into()),
            private: false,
            fork: false,
            stargazers_count: 0,
            updated_at: ago(3),
            html_url: String::new(),
        };
        Ok(PaginatedResult { items: vec![repo], page: 1, has_next_page: false, total_count: Some(1) })
    }

    async fn search_repos(&self, _query: &str, page: u32, per_page: u32) -> Result<PaginatedResult<RepoInfo>> {
        self.list_user_repos(page, per_page).await
    }

    async fn search_pull_requests(
        &self, owner: &str, repo: &str, state: PrState, filters: &PrSearchFilters, query: &str, _page: u32, _per_page: u32,
    ) -> Result<PaginatedResult<PullRequest>> {
        if owner != OWNER || repo != REPO {
            return Err(not_found());
        }
        // Every sample PR is open.
        let mut items = Vec::new();
        for pr in PRS.iter().filter(|pr| !matches!(state, PrState::Closed) && self.matches(pr, filters, query)) {
            items.push(self.pull_request(pr)?);
        }
        Ok(PaginatedResult { total_count: Some(items.len() as u64), items, page: 1, has_next_page: false })
    }

    async fn list_repo_labels(&self, _owner: &str, _repo: &str) -> Result<Vec<Label>> {
        Ok(LABELS.iter().map(|(name, _)| label(name)).collect())
    }

    async fn get_thread_resolutions(&self, owner: &str, repo: &str, number: u64) -> Result<Vec<ThreadResolutionInfo>> {
        self.fixture(owner, repo, number)?;
        Ok(self.store().resolutions.iter().filter(|(n, _)| *n == number).map(|(_, r)| r.clone()).collect())
    }

    async fn resolve_thread(&self, thread_node_id: &str) -> Result<()> {
        self.store().resolutions.iter_mut().filter(|(_, r)| r.thread_node_id == thread_node_id).for_each(|(_, r)| r.is_resolved = true);
        Ok(())
    }

    async fn unresolve_thread(&self, thread_node_id: &str) -> Result<()> {
        self.store().resolutions.iter_mut().filter(|(_, r)| r.thread_node_id == thread_node_id).for_each(|(_, r)| r.is_resolved = false);
        Ok(())
    }

    async fn get_pr_commits(&self, owner: &str, repo: &str, number: u64) -> Result<Vec<Commit>> {
        let pr = self.fixture(owner, repo, number)?;
        let parents = std::iter::once(BASE_SHA).chain(pr.commits.iter().map(|c| c.sha));
        Ok(pr
            .commits
            .iter()
            .zip(parents)
            .map(|(c, parent)| Commit {
                sha: c.sha.into(),
                short_sha: c.sha[..7].into(),
                message: c.message.into(),
                author_login: pr.author.into(),
                author_avatar: String::new(),
                authored_date: ago(c.hours_ago),
                parents: vec![parent.into()],
            })
            .collect())
    }

    async fn get_compare_files(&self, owner: &str, repo: &str, base: &str, head: &str) -> Result<Vec<PrFile>> {
        if owner != OWNER || repo != REPO {
            return Err(not_found());
        }
        self.compare(base, head)
    }

    async fn get_file_content(&self, owner: &str, repo: &str, path: &str, ref_: &str) -> Result<String> {
        if owner != OWNER || repo != REPO {
            return Err(not_found());
        }
        self.snapshot(ref_)?.get(path).map(|text| text.to_string()).ok_or_else(not_found)
    }

    async fn upload_attachment(&self, _: &str, _: &str, _: &str, _: &str, _: Vec<u8>) -> Result<String> {
        Err(anyhow!("Attachments need a GitHub account. The sample cannot upload files."))
    }

    async fn resolve_attachment(&self, url: &str) -> Result<String> {
        Ok(url.into())
    }

    fn rate_limit_status(&self) -> RateLimitInfo {
        RateLimitInfo { remaining: 5000, limit: 5000, reset_at: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comments::build_threads;
    use crate::diff::{fetch_pr_files, parser::DiffLineKind};

    /// The bundled review and threads must point at lines in the sample diff, or the sample shows them as "not in diff".
    #[tokio::test]
    async fn sample_pr_loads_and_fixtures_point_at_diff_lines() {
        let p = DemoProvider::new();
        let files = fetch_pr_files(&p, OWNER, REPO, PR).await.unwrap();
        let status = |name: &str| files.iter().find(|f| f.filename == name).map(|f| f.status.as_str());
        assert_eq!(status("src/lib/money.ts"), Some("renamed"));
        assert_eq!(status("src/legacy/priceUtils.ts"), Some("removed"));
        assert_eq!(status("src/coupons.ts"), Some("added"));
        assert_eq!(status("package-lock.json"), Some("modified"));
        assert_eq!(status("src/money.ts"), None);

        let review = replay_review(PR, String::new()).await;
        assert!(review.comments.len() >= 3);
        for c in &review.comments {
            assert!(ai_review::in_diff(&files, c), "{}:{} not in diff", c.file, c.line);
        }
        let named: HashSet<_> = review.reading_order.iter().map(|s| s.file.as_str()).collect();
        assert!(files.iter().all(|f| named.contains(f.filename.as_str())), "reading order misses a file");

        let comments = p.get_pr_comments(OWNER, REPO, PR).await.unwrap();
        let resolutions = p.get_thread_resolutions(OWNER, REPO, PR).await.unwrap();
        let threads = build_threads(&comments, &resolutions);
        assert_eq!(threads.iter().filter(|t| t.is_resolved).count(), 1);
        for t in &threads {
            let file = files.iter().find(|f| f.filename == t.root.path).unwrap();
            let lines = file.diff.iter().flat_map(|d| &d.hunks).flat_map(|h| &h.lines);
            assert!(lines.filter(|l| l.kind != DiffLineKind::Removed).any(|l| l.new_lineno == Some(t.line)));
        }
    }

    #[tokio::test]
    async fn single_commit_compare_and_review_round_trip() {
        let p = DemoProvider::new();
        let commits = p.get_pr_commits(OWNER, REPO, PR).await.unwrap();
        let last = commits.last().unwrap();
        let files = p.get_compare_files(OWNER, REPO, &last.parents[0], &last.sha).await.unwrap();
        assert_eq!(files.iter().map(|f| f.filename.as_str()).collect::<Vec<_>>(), ["src/coupons.ts"]);

        let pending = p.create_pending_review(OWNER, REPO, PR, &last.sha).await.unwrap();
        let params = NewComment { path: "src/cart.ts".into(), line: 23, side: "RIGHT".into(), body: "x".into(), commit_id: last.sha.clone(), start_line: None };
        p.add_review_comment(&pending.node_id, params).await.unwrap();
        p.submit_review(OWNER, REPO, PR, Some(pending.id), ReviewEvent::Approve, "").await.unwrap();
        let reviews = p.get_pr_reviews(OWNER, REPO, PR).await.unwrap();
        assert!(reviews.iter().any(|r| r.author_login == LOGIN && r.state == "APPROVED"));
        assert!(p.get_pr_comments(OWNER, REPO, PR).await.unwrap().iter().any(|c| c.author_login == LOGIN));
    }

    #[test]
    fn clear_removes_only_sample_rows() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT);
             CREATE TABLE reviewed_files (owner TEXT, repo TEXT, pr_number INTEGER, file_path TEXT);
             CREATE TABLE pr_views (owner TEXT, repo TEXT, pr_number INTEGER, viewed_at TEXT);
             CREATE TABLE saved_repos (owner TEXT, repo TEXT);",
        )
        .unwrap();
        queries::set_setting(&conn, &ai_review::review_key(OWNER, REPO, PR), "{}").unwrap();
        queries::set_setting(&conn, &ai_review::review_key("me", "app", 1), "{}").unwrap();
        clear(&conn).unwrap();
        assert!(queries::get_setting(&conn, &ai_review::review_key(OWNER, REPO, PR)).unwrap().is_none());
        assert!(queries::get_setting(&conn, &ai_review::review_key("me", "app", 1)).unwrap().is_some());
    }
}
