import { createSignal } from "solid-js";
import type { PullRequest } from "~/types/pr";

export type PrFilter = "open" | "closed" | "all";

const [prs, setPrs] = createSignal<PullRequest[]>([]);
const [filter, setFilter] = createSignal<PrFilter>("open");
const [searchQuery, setSearchQuery] = createSignal("");
const [page, setPage] = createSignal(1);
const [hasNextPage, setHasNextPage] = createSignal(false);
const [totalCount, setTotalCount] = createSignal<number | null>(null);
const [isLoadingMore, setIsLoadingMore] = createSignal(false);
const [repoKey, setRepoKey] = createSignal("");

export const prListStore = {
  get prs() { return prs(); },
  get filter() { return filter(); },
  get searchQuery() { return searchQuery(); },
  get page() { return page(); },
  get hasNextPage() { return hasNextPage(); },
  get totalCount() { return totalCount(); },
  get isLoadingMore() { return isLoadingMore(); },
  get repoKey() { return repoKey(); },

  setPrs,
  setFilter,
  setSearchQuery,
  setPage,
  setHasNextPage,
  setTotalCount,
  setIsLoadingMore,
  setRepoKey,

  appendPrs(newPrs: PullRequest[]) {
    setPrs((prev) => [...prev, ...newPrs]);
  },

  clearAll() {
    setPrs([]);
    setPage(1);
    setHasNextPage(false);
    setTotalCount(null);
    setSearchQuery("");
  },
};
