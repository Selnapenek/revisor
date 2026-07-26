import {
  getPullRequest,
  getPrFiles,
  getPrComments,
  getIssueComments,
  getPrReviews,
  getReviewedFiles,
  getThreadResolutions,
  getPrCommits,
} from "~/api";
import type { PullRequest, ReviewComment, ThreadResolutionInfo, Commit, IssueComment, PrReview } from "~/types/pr";
import type { ProcessedFile } from "~/api/files";

export interface PrPrefetchData {
  pr: PullRequest;
  files: ProcessedFile[];
  comments: ReviewComment[];
  issueComments: IssueComment[];
  reviews: PrReview[];
  reviewed: string[];
  resolutions: ThreadResolutionInfo[];
  commits: Commit[];
}

interface CacheEntry {
  promise: Promise<PrPrefetchData>;
  timestamp: number;
}

const MAX_ENTRIES = 5;
const STALE_MS = 60_000;

const cache = new Map<string, CacheEntry>();

let pendingTimer: ReturnType<typeof setTimeout> | null = null;

function makeKey(owner: string, repo: string, number: number): string {
  return `${owner}/${repo}/${number}`;
}

export async function fetchPrDetail(
  owner: string,
  repo: string,
  number: number,
): Promise<PrPrefetchData> {
  const [pr, files, comments, issueComments, reviews, reviewed, resolutions, commits] = await Promise.all([
    getPullRequest(owner, repo, number),
    getPrFiles(owner, repo, number),
    getPrComments(owner, repo, number),
    getIssueComments(owner, repo, number),
    getPrReviews(owner, repo, number),
    getReviewedFiles(owner, repo, number),
    getThreadResolutions(owner, repo, number),
    getPrCommits(owner, repo, number),
  ]);
  return { pr, files, comments, issueComments, reviews, reviewed, resolutions, commits };
}

export function prefetch(owner: string, repo: string, number: number): void {
  if (pendingTimer !== null) {
    clearTimeout(pendingTimer);
    pendingTimer = null;
  }

  pendingTimer = setTimeout(() => {
    pendingTimer = null;

    const key = makeKey(owner, repo, number);

    const existing = cache.get(key);
    if (existing && Date.now() - existing.timestamp < STALE_MS) {
      return;
    }

    // Evict oldest if at capacity
    if (cache.size >= MAX_ENTRIES) {
      let oldestKey: string | null = null;
      let oldestTime = Infinity;
      for (const [k, entry] of cache) {
        if (entry.timestamp < oldestTime) {
          oldestTime = entry.timestamp;
          oldestKey = k;
        }
      }
      if (oldestKey) cache.delete(oldestKey);
    }

    const promise = fetchPrDetail(owner, repo, number).catch((err) => {
      cache.delete(key);
      throw err;
    });

    cache.set(key, { promise, timestamp: Date.now() });
  }, 80);
}

export function cancelPrefetch(): void {
  if (pendingTimer !== null) {
    clearTimeout(pendingTimer);
    pendingTimer = null;
  }
}

export function consumePrefetch(
  owner: string,
  repo: string,
  number: number,
): Promise<PrPrefetchData> | null {
  const key = makeKey(owner, repo, number);
  const entry = cache.get(key);
  if (!entry || Date.now() - entry.timestamp >= STALE_MS) {
    if (entry) cache.delete(key);
    return null;
  }
  return entry.promise;
}
