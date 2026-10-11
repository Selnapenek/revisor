pub mod github;
pub mod rate_limit;
pub mod types;

use anyhow::Result;
use async_trait::async_trait;
use types::*;

#[async_trait]
pub trait GitProvider: Send + Sync {
    /// Validate the stored token and return the authenticated user
    async fn validate_token(&self) -> Result<ProviderUser>;

    /// List pull requests for a repository
    async fn list_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        state: PrState,
        page: u32,
        per_page: u32,
    ) -> Result<PaginatedResult<PullRequest>>;

    /// Get a single pull request by number
    async fn get_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<PullRequest>;

    /// Get files changed in a pull request
    async fn get_pr_files(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        page: u32,
    ) -> Result<PaginatedResult<PrFile>>;

    /// Get review comments on a pull request
    async fn get_pr_comments(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<ReviewComment>>;

    /// Get top-level conversation comments on a pull request (issue comments)
    async fn get_issue_comments(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<IssueComment>>;

    /// Get reviews on a pull request (submitted via "Finish your review")
    async fn get_pr_reviews(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<PrReview>>;

    /// Add a new review comment
    async fn add_comment(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        params: NewComment,
    ) -> Result<ReviewComment>;

    /// Start a PENDING review (no event); comments are added with `add_review_comment`
    async fn create_pending_review(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        commit_id: &str,
    ) -> Result<PrReview>;

    /// Add a line comment to a pending review (by review GraphQL node ID)
    async fn add_review_comment(
        &self,
        review_node_id: &str,
        params: NewComment,
    ) -> Result<ReviewComment>;

    /// Get comments of one review (pending review comments included)
    async fn get_review_comments(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        review_id: u64,
    ) -> Result<Vec<ReviewComment>>;

    /// Submit the pending review, or create and submit a review at once when `pending_id` is None
    async fn submit_review(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        pending_id: Option<u64>,
        event: ReviewEvent,
        body: &str,
    ) -> Result<()>;

    /// Discard a pending review and its comments
    async fn delete_pending_review(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        review_id: u64,
    ) -> Result<()>;

    /// Reply to an existing comment
    async fn reply_to_comment(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        comment_id: u64,
        body: &str,
    ) -> Result<ReviewComment>;

    /// Update an existing review comment's body
    async fn update_comment(
        &self,
        owner: &str,
        repo: &str,
        comment_id: u64,
        body: &str,
    ) -> Result<ReviewComment>;

    /// Delete a review comment
    async fn delete_comment(
        &self,
        owner: &str,
        repo: &str,
        comment_id: u64,
    ) -> Result<()>;

    /// List repositories the authenticated user has access to
    async fn list_user_repos(
        &self,
        page: u32,
        per_page: u32,
    ) -> Result<PaginatedResult<RepoInfo>>;

    /// Search repositories by query
    async fn search_repos(
        &self,
        query: &str,
        page: u32,
        per_page: u32,
    ) -> Result<PaginatedResult<RepoInfo>>;

    /// Search pull requests using GitHub Search API (supports advanced filters)
    async fn search_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        state: PrState,
        filters: &PrSearchFilters,
        page: u32,
        per_page: u32,
    ) -> Result<PaginatedResult<PullRequest>>;

    /// List all labels for a repository
    async fn list_repo_labels(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<Label>>;

    /// Get thread resolution status for all review threads on a PR (via GraphQL)
    async fn get_thread_resolutions(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<ThreadResolutionInfo>>;

    /// Resolve a review thread by its GraphQL node ID
    async fn resolve_thread(&self, thread_node_id: &str) -> Result<()>;

    /// Unresolve a review thread by its GraphQL node ID
    async fn unresolve_thread(&self, thread_node_id: &str) -> Result<()>;

    /// List commits in a pull request
    async fn get_pr_commits(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<Commit>>;

    /// Get files changed between two refs (compare API)
    async fn get_compare_files(
        &self,
        owner: &str,
        repo: &str,
        base: &str,
        head: &str,
    ) -> Result<Vec<PrFile>>;

    /// Get raw file content at a specific ref
    async fn get_file_content(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        ref_: &str,
    ) -> Result<String>;

    /// Upload a file for use in comment markdown; returns its URL
    async fn upload_attachment(
        &self,
        owner: &str,
        repo: &str,
        name: &str,
        content_type: &str,
        bytes: Vec<u8>,
    ) -> Result<String>;

    /// Short-lived direct link for an attachment URL that needs the user's token to load
    async fn resolve_attachment(&self, url: &str) -> Result<String>;

    /// Get current rate limit status
    fn rate_limit_status(&self) -> RateLimitInfo;
}
