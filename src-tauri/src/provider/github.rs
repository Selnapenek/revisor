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

    /// GET request that returns raw text (used for fetching file content)
    async fn get_raw(&self, url: &str) -> Result<String> {
        if self.rate_limit.should_throttle() {
            bail!("GitHub API rate limit nearly exhausted. Resets at timestamp {}.",
                  self.rate_limit.info().reset_at);
        }

        let response = self
            .client
            .get(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.raw+json")
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

        response.text().await.context("Failed to read response body")
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

    /// Send a GraphQL query to the GitHub API
    async fn graphql(
        &self,
        query: &str,
        variables: serde_json::Value,
    ) -> Result<serde_json::Value> {
        if self.rate_limit.should_throttle() {
            bail!(
                "GitHub API rate limit nearly exhausted. Resets at timestamp {}.",
                self.rate_limit.info().reset_at
            );
        }

        let body = serde_json::json!({
            "query": query,
            "variables": variables,
        });

        let response = self
            .client
            .post("https://api.github.com/graphql")
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .json(&body)
            .send()
            .await
            .context("GraphQL request failed")?;

        self.rate_limit.update_from_headers(response.headers());

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            bail!("GitHub GraphQL error {}: {}", status, text);
        }

        let data: serde_json::Value = response.json().await.context("Failed to parse GraphQL response")?;

        if let Some(errors) = data.get("errors") {
            if let Some(arr) = errors.as_array() {
                if let Some(first) = arr.first() {
                    let msg = first
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Unknown GraphQL error");
                    bail!("GitHub GraphQL error: {}", msg);
                }
            }
        }

        data.get("data")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("GraphQL response missing 'data' field"))
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
    assignees: Option<Vec<GhPrUser>>,
    requested_reviewers: Option<Vec<GhPrUser>>,
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

/// GitHub Search API response for issues/PRs
#[derive(Deserialize)]
struct GhSearchResult2 {
    items: Vec<GhSearchIssueItem>,
    total_count: u64,
}

/// A single item from the GitHub Search Issues API (used for PR search)
#[derive(Deserialize)]
struct GhSearchIssueItem {
    id: u64,
    number: u64,
    title: String,
    body: Option<String>,
    state: String,
    draft: Option<bool>,
    user: GhPrUser,
    labels: Vec<GhLabel>,
    assignees: Option<Vec<GhPrUser>>,
    created_at: String,
    updated_at: String,
    pull_request: Option<GhSearchPrRef>,
    html_url: String,
}

#[derive(Deserialize)]
struct GhSearchPrRef {
    merged_at: Option<String>,
}

/// GitHub commit response (from PR commits endpoint)
#[derive(Deserialize)]
struct GhCommit {
    sha: String,
    commit: GhCommitDetail,
    author: Option<GhPrUser>,
    parents: Vec<GhCommitParent>,
}

#[derive(Deserialize)]
struct GhCommitDetail {
    message: String,
    author: GhCommitAuthor,
}

#[derive(Deserialize)]
struct GhCommitAuthor {
    date: String,
}

#[derive(Deserialize)]
struct GhCommitParent {
    sha: String,
}

impl From<GhCommit> for Commit {
    fn from(c: GhCommit) -> Self {
        let short_sha = c.sha.chars().take(7).collect();
        let message = c.commit.message.lines().next().unwrap_or("").to_string();
        let (author_login, author_avatar) = match c.author {
            Some(u) => (u.login, u.avatar_url),
            None => ("unknown".to_string(), String::new()),
        };
        Self {
            sha: c.sha,
            short_sha,
            message,
            author_login,
            author_avatar,
            authored_date: c.commit.author.date,
            parents: c.parents.into_iter().map(|p| p.sha).collect(),
        }
    }
}

/// GitHub compare API response
#[derive(Deserialize)]
struct GhCompareResponse {
    files: Option<Vec<GhPrFile>>,
}

impl From<GhSearchIssueItem> for PullRequest {
    fn from(item: GhSearchIssueItem) -> Self {
        let merged_at = item.pull_request.as_ref().and_then(|pr| pr.merged_at.clone());
        Self {
            id: item.id,
            number: item.number,
            title: item.title,
            body: item.body,
            state: if merged_at.is_some() { "closed".to_string() } else { item.state },
            draft: item.draft.unwrap_or(false),
            author_login: item.user.login.clone(),
            author_avatar: item.user.avatar_url.clone(),
            labels: item.labels.into_iter().map(|l| Label { name: l.name, color: l.color }).collect(),
            assignees: item.assignees.unwrap_or_default().into_iter().map(|u| PrUser { login: u.login, avatar_url: u.avatar_url }).collect(),
            requested_reviewers: Vec::new(), // Search API doesn't return this
            created_at: item.created_at,
            updated_at: item.updated_at,
            merged_at,
            // Search API doesn't return these fields — zero-fill
            comments_count: 0,
            review_comments_count: 0,
            additions: 0,
            deletions: 0,
            changed_files: 0,
            head_sha: String::new(),
            base_sha: String::new(),
            head_ref: String::new(),
            base_ref: String::new(),
            html_url: item.html_url,
        }
    }
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
            author_login: pr.user.login.clone(),
            author_avatar: pr.user.avatar_url.clone(),
            labels: pr.labels.into_iter().map(|l| Label { name: l.name, color: l.color }).collect(),
            assignees: pr.assignees.unwrap_or_default().into_iter().map(|u| PrUser { login: u.login, avatar_url: u.avatar_url }).collect(),
            requested_reviewers: pr.requested_reviewers.unwrap_or_default().into_iter().map(|u| PrUser { login: u.login, avatar_url: u.avatar_url }).collect(),
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
            outdated: c.line.is_none() && c.original_line.is_some(),
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

    async fn search_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        state: PrState,
        filters: &PrSearchFilters,
        page: u32,
        per_page: u32,
    ) -> Result<PaginatedResult<PullRequest>> {
        let mut qualifiers = vec![
            format!("is:pr"),
            format!("repo:{}/{}", owner, repo),
        ];

        match state {
            PrState::Open => qualifiers.push("state:open".to_string()),
            PrState::Closed => qualifiers.push("state:closed".to_string()),
            PrState::All => {} // no state qualifier = all
        }

        for author in &filters.authors {
            qualifiers.push(format!("author:{}", author));
        }
        for label in &filters.labels {
            qualifiers.push(format!("label:{}", label));
        }
        for assignee in &filters.assignees {
            qualifiers.push(format!("assignee:{}", assignee));
        }
        if let Some(ref review) = filters.review_status {
            // Handle special cases like "reviewed-by:@me", "-reviewed-by:@me", "review-requested:@me"
            if review.contains(':') {
                qualifiers.push(review.clone());
            } else {
                qualifiers.push(format!("review:{}", review));
            }
        }

        let query = qualifiers.join("+");
        let url = format!(
            "https://api.github.com/search/issues?q={}&sort=updated&order=desc&per_page={}&page={}",
            urlencoding(&query), per_page, page
        );

        let result: GhSearchResult2 = self.get(&url).await?;
        let has_next = (page as u64 * per_page as u64) < result.total_count;

        Ok(PaginatedResult {
            items: result.items.into_iter().map(Into::into).collect(),
            page,
            has_next_page: has_next,
            total_count: Some(result.total_count),
        })
    }

    async fn list_repo_labels(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<Label>> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/labels?per_page=100",
            owner, repo
        );
        let (items, _): (Vec<GhLabel>, bool) = self.get_with_pagination(&url).await?;
        Ok(items.into_iter().map(|l| Label { name: l.name, color: l.color }).collect())
    }

    async fn get_thread_resolutions(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<ThreadResolutionInfo>> {
        const QUERY: &str = r#"
            query($owner: String!, $repo: String!, $number: Int!, $cursor: String) {
                repository(owner: $owner, name: $repo) {
                    pullRequest(number: $number) {
                        reviewThreads(first: 100, after: $cursor) {
                            pageInfo { hasNextPage endCursor }
                            nodes {
                                id
                                isResolved
                                comments(first: 1) {
                                    nodes { databaseId }
                                }
                            }
                        }
                    }
                }
            }
        "#;

        let mut results = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let variables = serde_json::json!({
                "owner": owner,
                "repo": repo,
                "number": number as i64,
                "cursor": cursor,
            });

            let data = self.graphql(QUERY, variables).await?;

            let threads = &data["repository"]["pullRequest"]["reviewThreads"];
            let nodes = threads["nodes"]
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("Invalid GraphQL response structure"))?;

            for node in nodes {
                let thread_node_id = node["id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let is_resolved = node["isResolved"].as_bool().unwrap_or(false);
                let root_comment_id = node["comments"]["nodes"]
                    .as_array()
                    .and_then(|arr| arr.first())
                    .and_then(|c| c["databaseId"].as_u64())
                    .unwrap_or(0);

                if root_comment_id > 0 {
                    results.push(ThreadResolutionInfo {
                        root_comment_id,
                        thread_node_id,
                        is_resolved,
                    });
                }
            }

            let page_info = &threads["pageInfo"];
            if page_info["hasNextPage"].as_bool().unwrap_or(false) {
                cursor = page_info["endCursor"].as_str().map(String::from);
            } else {
                break;
            }
        }

        Ok(results)
    }

    async fn resolve_thread(&self, thread_node_id: &str) -> Result<()> {
        const MUTATION: &str = r#"
            mutation($threadId: ID!) {
                resolveReviewThread(input: { threadId: $threadId }) {
                    thread { isResolved }
                }
            }
        "#;

        let variables = serde_json::json!({ "threadId": thread_node_id });
        self.graphql(MUTATION, variables).await?;
        Ok(())
    }

    async fn unresolve_thread(&self, thread_node_id: &str) -> Result<()> {
        const MUTATION: &str = r#"
            mutation($threadId: ID!) {
                unresolveReviewThread(input: { threadId: $threadId }) {
                    thread { isResolved }
                }
            }
        "#;

        let variables = serde_json::json!({ "threadId": thread_node_id });
        self.graphql(MUTATION, variables).await?;
        Ok(())
    }

    async fn get_pr_commits(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<Commit>> {
        let mut all_commits = Vec::new();
        let mut page = 1u32;

        loop {
            let url = format!(
                "https://api.github.com/repos/{}/{}/pulls/{}/commits?per_page=100&page={}",
                owner, repo, number, page
            );

            let (items, has_next): (Vec<GhCommit>, bool) =
                self.get_with_pagination(&url).await?;

            all_commits.extend(items.into_iter().map(Commit::from));

            if !has_next {
                break;
            }
            page += 1;
        }

        Ok(all_commits)
    }

    async fn get_compare_files(
        &self,
        owner: &str,
        repo: &str,
        base: &str,
        head: &str,
    ) -> Result<Vec<PrFile>> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/compare/{}...{}",
            owner, repo, base, head
        );

        let result: GhCompareResponse = self.get(&url).await?;
        Ok(result.files.unwrap_or_default().into_iter().map(Into::into).collect())
    }

    async fn get_file_content(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        ref_: &str,
    ) -> Result<String> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/contents/{}?ref={}",
            owner, repo, path, ref_
        );
        self.get_raw(&url).await
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
