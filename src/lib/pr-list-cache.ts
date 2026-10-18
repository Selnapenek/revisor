import type { PrSearchFilters, PullRequest } from "../types/pr";

export interface PrListSnapshot {
  items: PullRequest[];
  page: number;
  hasNextPage: boolean;
  totalCount: number | null;
}

export function prListQueryKey(repo: string, state: string, filters: PrSearchFilters, search: string): string {
  const sorted = (values: string[]) => [...values].sort();
  return JSON.stringify([
    repo,
    state,
    search.trim(),
    sorted(filters.authors),
    sorted(filters.labels),
    sorted(filters.assignees),
    filters.review_status,
  ]);
}

export function createPrListCache(limit = 12) {
  const entries = new Map<string, PrListSnapshot>();
  return {
    get(key: string): PrListSnapshot | undefined {
      const snapshot = entries.get(key);
      if (!snapshot) return undefined;
      entries.delete(key);
      entries.set(key, snapshot);
      return snapshot;
    },
    set(key: string, snapshot: PrListSnapshot): void {
      entries.delete(key);
      entries.set(key, snapshot);
      if (entries.size > limit) entries.delete(entries.keys().next().value!);
    },
  };
}
