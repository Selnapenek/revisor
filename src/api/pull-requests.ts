import { invoke } from "@tauri-apps/api/core";
import type { Label, PaginatedResult, PrSearchFilters, PullRequest, RateLimitInfo } from "~/types/pr";

export async function listPullRequests(
  owner: string,
  repo: string,
  prState: string,
  page: number,
  perPage: number = 30,
): Promise<PaginatedResult<PullRequest>> {
  return invoke<PaginatedResult<PullRequest>>("list_pull_requests", {
    owner,
    repo,
    prState,
    page,
    perPage,
  });
}

export async function getPullRequest(
  owner: string,
  repo: string,
  number: number,
): Promise<PullRequest> {
  return invoke<PullRequest>("get_pull_request", { owner, repo, number });
}

export async function searchPullRequests(
  owner: string,
  repo: string,
  prState: string,
  filters: PrSearchFilters,
  query: string,
  page: number,
  perPage: number = 30,
): Promise<PaginatedResult<PullRequest>> {
  return invoke<PaginatedResult<PullRequest>>("search_pull_requests", {
    owner,
    repo,
    prState,
    filters,
    query,
    page,
    perPage,
  });
}

export async function listRepoLabels(
  owner: string,
  repo: string,
): Promise<Label[]> {
  return invoke<Label[]>("list_repo_labels", { owner, repo });
}

export async function getRateLimit(): Promise<RateLimitInfo> {
  return invoke<RateLimitInfo>("get_rate_limit");
}
