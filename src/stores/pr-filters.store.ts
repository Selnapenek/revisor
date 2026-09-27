import { batch, createSignal, createMemo } from "solid-js";
import type { Label, PrUser, PullRequest, PrSearchFilters } from "~/types/pr";
import { listRepoLabels } from "~/api";
import { withAuthorPreset, withReviewRequestedPreset } from "~/lib/pr-filter-presets";
import { loadRepositoryFilters, saveRepositoryFilters } from "~/lib/pr-filter-persistence";

const [selectedAuthors, setSelectedAuthors] = createSignal<string[]>([]);
const [selectedLabels, setSelectedLabels] = createSignal<string[]>([]);
const [selectedAssignees, setSelectedAssignees] = createSignal<string[]>([]);
const [selectedReviewStatus, setSelectedReviewStatus] = createSignal<string | null>(null);

const [availableLabels, setAvailableLabels] = createSignal<Label[]>([]);
const [knownAuthors, setKnownAuthors] = createSignal<PrUser[]>([]);
const [knownAssignees, setKnownAssignees] = createSignal<PrUser[]>([]);
const [isLoadingLabels, setIsLoadingLabels] = createSignal(false);
let activeRepositoryKey: string | null = null;

function persistActiveFilters() {
  if (!activeRepositoryKey) return;
  try {
    saveRepositoryFilters(localStorage, activeRepositoryKey, activeFilters());
  } catch {
    // localStorage may not be available in the current environment.
  }
}

const hasActiveFilters = createMemo(() =>
  selectedAuthors().length > 0 ||
  selectedLabels().length > 0 ||
  selectedAssignees().length > 0 ||
  selectedReviewStatus() !== null
);

const activeFilters = createMemo((): PrSearchFilters => ({
  authors: selectedAuthors(),
  labels: selectedLabels(),
  assignees: selectedAssignees(),
  review_status: selectedReviewStatus(),
}));

function toggleValue(arr: string[], value: string): string[] {
  return arr.includes(value) ? arr.filter((v) => v !== value) : [...arr, value];
}

function mergeUsers(existing: PrUser[], incoming: PrUser[]): PrUser[] {
  const seen = new Set(existing.map((u) => u.login));
  const merged = [...existing];
  for (const u of incoming) {
    if (!seen.has(u.login)) {
      seen.add(u.login);
      merged.push(u);
    }
  }
  return merged.sort((a, b) => a.login.localeCompare(b.login));
}

export const REVIEW_STATUS_OPTIONS = [
  { value: "none", label: "No reviews" },
  { value: "required", label: "Review required" },
  { value: "approved", label: "Approved" },
  { value: "changes_requested", label: "Changes requested" },
  { value: "reviewed-by:@me", label: "Reviewed by you" },
  { value: "-reviewed-by:@me", label: "Not reviewed by you" },
  { value: "review-requested:@me", label: "Awaiting your review" },
] as const;

export const prFiltersStore = {
  get selectedAuthors() { return selectedAuthors(); },
  get selectedLabels() { return selectedLabels(); },
  get selectedAssignees() { return selectedAssignees(); },
  get selectedReviewStatus() { return selectedReviewStatus(); },
  get availableLabels() { return availableLabels(); },
  get knownAuthors() { return knownAuthors(); },
  get knownAssignees() { return knownAssignees(); },
  get isLoadingLabels() { return isLoadingLabels(); },
  get hasActiveFilters() { return hasActiveFilters(); },
  get activeFilters() { return activeFilters(); },

  toggleAuthor(login: string) {
    setSelectedAuthors((prev) => toggleValue(prev, login));
    persistActiveFilters();
  },
  toggleLabel(name: string) {
    setSelectedLabels((prev) => toggleValue(prev, name));
    persistActiveFilters();
  },
  toggleAssignee(login: string) {
    setSelectedAssignees((prev) => toggleValue(prev, login));
    persistActiveFilters();
  },
  setReviewStatus(status: string | null) {
    setSelectedReviewStatus((prev) => prev === status ? null : status);
    persistActiveFilters();
  },
  applyAuthorPreset(login: string) {
    setSelectedAuthors(withAuthorPreset(activeFilters(), login).authors);
    persistActiveFilters();
  },
  applyNeedsReviewPreset() {
    setSelectedReviewStatus(withReviewRequestedPreset(activeFilters()).review_status);
    persistActiveFilters();
  },

  async fetchLabels(owner: string, repo: string) {
    if (availableLabels().length > 0 || isLoadingLabels()) return;
    setIsLoadingLabels(true);
    try {
      const labels = await listRepoLabels(owner, repo);
      setAvailableLabels(labels);
    } catch {
      // Silently fail — labels are non-critical
    } finally {
      setIsLoadingLabels(false);
    }
  },

  updateKnownUsersFromPrs(prs: PullRequest[]) {
    const authors: PrUser[] = prs.map((pr) => ({
      login: pr.author_login,
      avatar_url: pr.author_avatar,
    }));
    const assignees: PrUser[] = prs.flatMap((pr) => pr.assignees ?? []);

    setKnownAuthors((prev) => mergeUsers(prev, authors));
    setKnownAssignees((prev) => mergeUsers(prev, assignees));
  },

  clearAll() {
    batch(() => {
      setSelectedAuthors([]);
      setSelectedLabels([]);
      setSelectedAssignees([]);
      setSelectedReviewStatus(null);
    });
    persistActiveFilters();
  },

  loadForRepository(repositoryKey: string) {
    activeRepositoryKey = repositoryKey;
    let filters: PrSearchFilters;
    try {
      filters = loadRepositoryFilters(localStorage, repositoryKey);
    } catch {
      filters = { authors: [], labels: [], assignees: [], review_status: null };
    }
    batch(() => {
      setSelectedAuthors(filters.authors);
      setSelectedLabels(filters.labels);
      setSelectedAssignees(filters.assignees);
      setSelectedReviewStatus(filters.review_status);
    });
  },

  reset() {
    activeRepositoryKey = null;
    setSelectedAuthors([]);
    setSelectedLabels([]);
    setSelectedAssignees([]);
    setSelectedReviewStatus(null);
    setAvailableLabels([]);
    setKnownAuthors([]);
    setKnownAssignees([]);
  },
};
