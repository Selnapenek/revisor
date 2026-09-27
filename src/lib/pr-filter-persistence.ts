import type { PrSearchFilters } from "~/types/pr";

export interface FilterStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

const STORAGE_PREFIX = "revisor:pr-filters:";

function emptyFilters(): PrSearchFilters {
  return { authors: [], labels: [], assignees: [], review_status: null };
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === "string");
}

function isPrSearchFilters(value: unknown): value is PrSearchFilters {
  if (typeof value !== "object" || value === null) return false;
  const filters = value as Partial<PrSearchFilters>;
  return isStringArray(filters.authors)
    && isStringArray(filters.labels)
    && isStringArray(filters.assignees)
    && (typeof filters.review_status === "string" || filters.review_status === null);
}

function storageKey(repositoryKey: string): string {
  return `${STORAGE_PREFIX}${encodeURIComponent(repositoryKey)}`;
}

export function loadRepositoryFilters(storage: FilterStorage, repositoryKey: string): PrSearchFilters {
  try {
    const saved = storage.getItem(storageKey(repositoryKey));
    if (!saved) return emptyFilters();
    const parsed: unknown = JSON.parse(saved);
    return isPrSearchFilters(parsed) ? parsed : emptyFilters();
  } catch {
    return emptyFilters();
  }
}

export function saveRepositoryFilters(
  storage: FilterStorage,
  repositoryKey: string,
  filters: PrSearchFilters,
): void {
  try {
    storage.setItem(storageKey(repositoryKey), JSON.stringify(filters));
  } catch {
    // Storage can be unavailable. Keep the filter controls usable in memory.
  }
}
