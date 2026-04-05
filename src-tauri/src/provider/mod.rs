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

    /// Add a new review comment
    async fn add_comment(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        params: NewComment,
    ) -> Result<ReviewComment>;

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

    /// Get current rate limit status
    fn rate_limit_status(&self) -> RateLimitInfo;
}
