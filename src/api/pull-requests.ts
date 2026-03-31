import { invoke } from "@tauri-apps/api/core";
import type { PaginatedResult, PullRequest, RateLimitInfo } from "~/types/pr";

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

export async function getRateLimit(): Promise<RateLimitInfo> {
  return invoke<RateLimitInfo>("get_rate_limit");
}
