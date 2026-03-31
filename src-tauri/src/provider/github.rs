use super::rate_limit::RateLimitTracker;
use super::types::*;
use super::GitProvider;
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

pub struct GithubProvider {
    client: Client,
    token: String,
    rate_limit: RateLimitTracker,
}

impl GithubProvider {
    pub fn new(token: String) -> Result<Self> {
        let client = Client::builder()
            .user_agent("Revisor/1.0.0")
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            client,
            token,
            rate_limit: RateLimitTracker::new(),
        })
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        if self.rate_limit.should_throttle() {
            bail!("GitHub API rate limit nearly exhausted. Resets at timestamp {}.",
                  self.rate_limit.info().reset_at);
        }

        let response = self
            .client
            .get(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .context("Network request failed")?;

        self.rate_limit.update_from_headers(response.headers());

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("GitHub API error {}: {}", status, body);
        }

        response.json::<T>().await.context("Failed to parse response")
    }

    async fn post<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        body: &impl serde::Serialize,
    ) -> Result<T> {
        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(body)
            .send()
            .await
            .context("Network request failed")?;

        self.rate_limit.update_from_headers(response.headers());

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("GitHub API error {}: {}", status, body);
        }

        response.json::<T>().await.context("Failed to parse response")
    }

    async fn patch<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        body: &impl serde::Serialize,
    ) -> Result<T> {
        let response = self
            .client
            .patch(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(body)
            .send()
            .await
            .context("Network request failed")?;

        self.rate_limit.update_from_headers(response.headers());

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("GitHub API error {}: {}", status, body);
        }

        response.json::<T>().await.context("Failed to parse response")
    }

    async fn delete_request(&self, url: &str) -> Result<()> {
        let response = self
            .client
            .delete(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .context("Network request failed")?;

        self.rate_limit.update_from_headers(response.headers());

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("GitHub API error {}: {}", status, body);
        }

        Ok(())
    }

    /// Check if response has a next page via Link header
    fn has_next_page(headers: &reqwest::header::HeaderMap) -> bool {
        headers
            .get("link")
            .and_then(|v| v.to_str().ok())
            .map(|link| link.contains("rel=\"next\""))
            .unwrap_or(false)
    }

    async fn get_with_pagination<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
    ) -> Result<(Vec<T>, bool)> {
        if self.rate_limit.should_throttle() {
            bail!("GitHub API rate limit nearly exhausted.");
        }

        let response = self
            .client
            .get(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .context("Network request failed")?;

        self.rate_limit.update_from_headers(response.headers());
        let has_next = Self::has_next_page(response.headers());

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("GitHub API error {}: {}", status, body);
        }

        let items = response
            .json::<Vec<T>>()
            .await
            .context("Failed to parse response")?;

        Ok((items, has_next))
    }
}

// GitHub API response types (mapped to provider-agnostic types)
#[derive(Deserialize)]
struct GhUser {
    login: String,
    avatar_url: String,
    name: Option<String>,
}

#[derive(Deserialize)]
struct GhLabel {
    name: String,
    color: String,
}

#[derive(Deserialize)]
struct GhPrUser {
    login: String,
    avatar_url: String,
}

#[derive(Deserialize)]
struct GhPullRequest {
    id: u64,
    number: u64,
    title: String,
    body: Option<String>,
    state: String,
    draft: Option<bool>,
    user: GhPrUser,
    labels: Vec<GhLabel>,
    created_at: String,
    updated_at: String,
    merged_at: Option<String>,
    comments: Option<u64>,
    review_comments: Option<u64>,
    additions: Option<u64>,
    deletions: Option<u64>,
    changed_files: Option<u64>,
    head: GhRef,
    base: GhRef,
    html_url: String,
}

#[derive(Deserialize)]
struct GhRepo {
    full_name: String,
    name: String,
    owner: GhPrUser,
    description: Option<String>,
    private: bool,
    fork: bool,
    stargazers_count: u64,
    updated_at: String,
    html_url: String,
}

#[derive(Deserialize)]
struct GhSearchResult {
    items: Vec<GhRepo>,
    total_count: u64,
}

impl From<GhRepo> for RepoInfo {
    fn from(r: GhRepo) -> Self {
        Self {
            owner: r.owner.login,
            name: r.name,
            full_name: r.full_name,
            description: r.description,
            private: r.private,
            fork: r.fork,
            stargazers_count: r.stargazers_count,
            updated_at: r.updated_at,
            html_url: r.html_url,
        }
    }
}

#[derive(Deserialize)]
struct GhRef {
    sha: String,
    #[serde(rename = "ref")]
    ref_name: String,
}

#[derive(Deserialize)]
struct GhPrFile {
    sha: String,
    filename: String,
    status: String,
    additions: u64,
    deletions: u64,
    patch: Option<String>,
}

#[derive(Deserialize)]
struct GhReviewComment {
    id: u64,
    path: String,
    line: Option<u64>,
    original_line: Option<u64>,
    side: Option<String>,
    diff_hunk: String,
    body: String,
    user: GhPrUser,
    created_at: String,
    updated_at: String,
    in_reply_to_id: Option<u64>,
    pull_request_review_id: Option<u64>,
    html_url: String,
}

impl From<GhPullRequest> for PullRequest {
    fn from(pr: GhPullRequest) -> Self {
        Self {
            id: pr.id,
            number: pr.number,
            title: pr.title,
            body: pr.body,
            state: pr.state,
            draft: pr.draft.unwrap_or(false),
            author_login: pr.user.login,
            author_avatar: pr.user.avatar_url,
            labels: pr.labels.into_iter().map(|l| Label { name: l.name, color: l.color }).collect(),
            created_at: pr.created_at,
            updated_at: pr.updated_at,
            merged_at: pr.merged_at,
            comments_count: pr.comments.unwrap_or(0),
            review_comments_count: pr.review_comments.unwrap_or(0),
            additions: pr.additions.unwrap_or(0),
            deletions: pr.deletions.unwrap_or(0),
            changed_files: pr.changed_files.unwrap_or(0),
            head_sha: pr.head.sha,
            base_sha: pr.base.sha,
            head_ref: pr.head.ref_name,
            base_ref: pr.base.ref_name,
            html_url: pr.html_url,
        }
    }
}

impl From<GhPrFile> for PrFile {
    fn from(f: GhPrFile) -> Self {
        Self {
            sha: f.sha,
            filename: f.filename,
            status: f.status,
            additions: f.additions,
            deletions: f.deletions,
            patch: f.patch,
        }
    }
}

impl From<GhReviewComment> for ReviewComment {
    fn from(c: GhReviewComment) -> Self {
        Self {
            id: c.id,
            path: c.path,
            line: c.line,
            original_line: c.original_line,
            side: c.side,
            diff_hunk: c.diff_hunk,
            body: c.body,
            author_login: c.user.login,
            author_avatar: c.user.avatar_url,
            created_at: c.created_at,
            updated_at: c.updated_at,
            in_reply_to_id: c.in_reply_to_id,
            review_id: c.pull_request_review_id,
            html_url: c.html_url,
        }
    }
}

#[async_trait]
impl GitProvider for GithubProvider {
    async fn validate_token(&self) -> Result<ProviderUser> {
        let user: GhUser = self.get("https://api.github.com/user").await?;
        Ok(ProviderUser {
            login: user.login,
            avatar_url: user.avatar_url,
            name: user.name,
        })
    }

    async fn list_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        state: PrState,
        page: u32,
        per_page: u32,
    ) -> Result<PaginatedResult<PullRequest>> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls?state={}&per_page={}&page={}&sort=updated&direction=desc",
            owner, repo, state, per_page, page
        );

        let (items, has_next): (Vec<GhPullRequest>, bool) =
            self.get_with_pagination(&url).await?;

        Ok(PaginatedResult {
            items: items.into_iter().map(Into::into).collect(),
            page,
            has_next_page: has_next,
            total_count: None,
        })
    }

    async fn get_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<PullRequest> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls/{}",
            owner, repo, number
        );
        let pr: GhPullRequest = self.get(&url).await?;
        Ok(pr.into())
    }

    async fn get_pr_files(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        page: u32,
    ) -> Result<PaginatedResult<PrFile>> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls/{}/files?per_page=100&page={}",
            owner, repo, number, page
        );

        let (items, has_next): (Vec<GhPrFile>, bool) =
            self.get_with_pagination(&url).await?;

        Ok(PaginatedResult {
            items: items.into_iter().map(Into::into).collect(),
            page,
            has_next_page: has_next,
            total_count: None,
        })
    }

    async fn get_pr_comments(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<ReviewComment>> {
        let mut all_comments = Vec::new();
        let mut page = 1u32;

        loop {
            let url = format!(
                "https://api.github.com/repos/{}/{}/pulls/{}/comments?per_page=100&page={}",
                owner, repo, number, page
            );

            let (items, has_next): (Vec<GhReviewComment>, bool) =
                self.get_with_pagination(&url).await?;

            all_comments.extend(items.into_iter().map(ReviewComment::from));

            if !has_next {
                break;
            }
            page += 1;
        }

        Ok(all_comments)
    }

    async fn add_comment(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        params: NewComment,
    ) -> Result<ReviewComment> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls/{}/comments",
            owner, repo, number
        );

        let body = serde_json::json!({
            "body": params.body,
            "commit_id": params.commit_id,
            "path": params.path,
            "line": params.line,
            "side": params.side,
        });

        let comment: GhReviewComment = self.post(&url, &body).await?;
        Ok(comment.into())
    }

    async fn reply_to_comment(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        comment_id: u64,
        body: &str,
    ) -> Result<ReviewComment> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls/{}/comments",
            owner, repo, number
        );

        let payload = serde_json::json!({
            "body": body,
            "in_reply_to": comment_id,
        });

        let comment: GhReviewComment = self.post(&url, &payload).await?;
        Ok(comment.into())
    }

    async fn list_user_repos(
        &self,
        page: u32,
        per_page: u32,
    ) -> Result<PaginatedResult<RepoInfo>> {
        let url = format!(
            "https://api.github.com/user/repos?sort=pushed&direction=desc&per_page={}&page={}&type=all",
            per_page, page
        );

        let (items, has_next): (Vec<GhRepo>, bool) =
            self.get_with_pagination(&url).await?;

        Ok(PaginatedResult {
            items: items.into_iter().map(Into::into).collect(),
            page,
            has_next_page: has_next,
            total_count: None,
        })
    }

    async fn search_repos(
        &self,
        query: &str,
        page: u32,
        per_page: u32,
    ) -> Result<PaginatedResult<RepoInfo>> {
        let url = format!(
            "https://api.github.com/search/repositories?q={}&sort=updated&per_page={}&page={}",
            urlencoding(query), per_page, page
        );

        let result: GhSearchResult = self.get(&url).await?;
        let has_next = (page * per_page) < result.total_count as u32;

        Ok(PaginatedResult {
            items: result.items.into_iter().map(Into::into).collect(),
            page,
            has_next_page: has_next,
            total_count: Some(result.total_count),
        })
    }

    async fn update_comment(
        &self,
        owner: &str,
        repo: &str,
        comment_id: u64,
        body: &str,
    ) -> Result<ReviewComment> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls/comments/{}",
            owner, repo, comment_id
        );
        let payload = serde_json::json!({ "body": body });
        let comment: GhReviewComment = self.patch(&url, &payload).await?;
        Ok(comment.into())
    }

    async fn delete_comment(
        &self,
        owner: &str,
        repo: &str,
        comment_id: u64,
    ) -> Result<()> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls/comments/{}",
            owner, repo, comment_id
        );
        self.delete_request(&url).await
    }

    fn rate_limit_status(&self) -> RateLimitInfo {
        self.rate_limit.info()
    }
}

/// Simple URL encoding for query parameters
fn urlencoding(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                String::from(b as char)
            }
            b' ' => "+".to_string(),
            _ => format!("%{:02X}", b),
        })
        .collect()
}
