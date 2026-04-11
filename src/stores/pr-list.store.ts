import { createSignal } from "solid-js";
import type { PullRequest } from "~/types/pr";

export type PrFilter = "open" | "closed" | "all";

const [prs, setPrs] = createSignal<PullRequest[]>([]);
const [filter, setFilter] = createSignal<PrFilter>("open");
const [searchQuery, setSearchQuery] = createSignal("");
const [page, setPage] = createSignal(1);
const [hasNextPage, setHasNextPage] = createSignal(false);
const [isLoadingMore, setIsLoadingMore] = createSignal(false);
const [repoKey, setRepoKey] = createSignal("");

export const prListStore = {
  get prs() { return prs(); },
  get filter() { return filter(); },
  get searchQuery() { return searchQuery(); },
  get page() { return page(); },
  get hasNextPage() { return hasNextPage(); },
  get isLoadingMore() { return isLoadingMore(); },
  get repoKey() { return repoKey(); },

  get filteredPrs() {
    const q = searchQuery().toLowerCase();
    if (!q) return prs();
    return prs().filter(
      (pr) =>
        pr.title.toLowerCase().includes(q) ||
        pr.author_login.toLowerCase().includes(q) ||
        String(pr.number).includes(q),
    );
  },

  setPrs,
  setFilter,
  setSearchQuery,
  setPage,
  setHasNextPage,
  setIsLoadingMore,
  setRepoKey,

  appendPrs(newPrs: PullRequest[]) {
    setPrs((prev) => [...prev, ...newPrs]);
  },

  clearAll() {
    setPrs([]);
    setPage(1);
    setHasNextPage(false);
    setSearchQuery("");
  },
};
