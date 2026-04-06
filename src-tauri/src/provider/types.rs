use serde::{Deserialize, Serialize};

/// Authenticated user info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderUser {
    pub login: String,
    pub avatar_url: String,
    pub name: Option<String>,
}

/// Repository info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInfo {
    pub owner: String,
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub private: bool,
    pub fork: bool,
    pub stargazers_count: u64,
    pub updated_at: String,
    pub html_url: String,
}

/// Pull request state filter
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrState {
    Open,
    Closed,
    All,
}

impl std::fmt::Display for PrState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrState::Open => write!(f, "open"),
            PrState::Closed => write!(f, "closed"),
            PrState::All => write!(f, "all"),
        }
    }
}

/// Paginated API result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResult<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub has_next_page: bool,
    pub total_count: Option<u64>,
}

/// Pull request label
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub name: String,
    pub color: String,
}

/// Lightweight user reference (for assignees, reviewers)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrUser {
    pub login: String,
    pub avatar_url: String,
}

/// Filters for searching pull requests via GitHub Search API
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrSearchFilters {
    pub authors: Vec<String>,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    pub review_status: Option<String>,
}

impl PrSearchFilters {
    pub fn is_empty(&self) -> bool {
        self.authors.is_empty()
            && self.labels.is_empty()
            && self.assignees.is_empty()
            && self.review_status.is_none()
    }
}

/// Pull request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub id: u64,
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub draft: bool,
    pub author_login: String,
    pub author_avatar: String,
    pub labels: Vec<Label>,
    pub created_at: String,
    pub updated_at: String,
    pub merged_at: Option<String>,
    pub comments_count: u64,
    pub review_comments_count: u64,
    pub additions: u64,
    pub deletions: u64,
    pub changed_files: u64,
    pub head_sha: String,
    pub base_sha: String,
    pub head_ref: String,
    pub base_ref: String,
    pub html_url: String,
    pub assignees: Vec<PrUser>,
    pub requested_reviewers: Vec<PrUser>,
}

/// File changed in a PR (metadata only — diff loaded separately)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrFile {
    pub sha: String,
    pub filename: String,
    pub status: String,
    pub additions: u64,
    pub deletions: u64,
    pub patch: Option<String>,
}

/// Review comment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewComment {
    pub id: u64,
    pub path: String,
    pub line: Option<u64>,
    pub original_line: Option<u64>,
    pub side: Option<String>,
    pub diff_hunk: String,
    pub body: String,
    pub author_login: String,
    pub author_avatar: String,
    pub created_at: String,
    pub updated_at: String,
    pub in_reply_to_id: Option<u64>,
    pub review_id: Option<u64>,
    pub html_url: String,
}

/// Parameters for creating a new comment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewComment {
    pub path: String,
    pub line: u64,
    pub side: String,
    pub body: String,
    pub commit_id: String,
}

/// Thread resolution info (from GraphQL API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadResolutionInfo {
    pub root_comment_id: u64,
    pub thread_node_id: String,
    pub is_resolved: bool,
}

/// Commit in a pull request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub sha: String,
    pub short_sha: String,
    pub message: String,
    pub author_login: String,
    pub author_avatar: String,
    pub authored_date: String,
    pub parents: Vec<String>,
}

/// Rate limit info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitInfo {
    pub remaining: u32,
    pub limit: u32,
    pub reset_at: u64,
}
