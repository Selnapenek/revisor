import { Show } from "solid-js";
import { prFiltersStore } from "~/stores";
import { FilterDropdown } from "./FilterDropdown";
import { ReviewStatusDropdown } from "./ReviewStatusDropdown";
import type { FilterOption } from "./FilterDropdown";

export function PRListAdvancedFilters() {
  const authorOptions = (): FilterOption[] =>
    prFiltersStore.knownAuthors.map((u) => ({
      value: u.login,
      label: u.login,
      avatar: u.avatar_url,
    }));

  const labelOptions = (): FilterOption[] =>
    prFiltersStore.availableLabels.map((l) => ({
      value: l.name,
      label: l.name,
      color: l.color,
    }));

  const assigneeOptions = (): FilterOption[] =>
    prFiltersStore.knownAssignees.map((u) => ({
      value: u.login,
      label: u.login,
      avatar: u.avatar_url,
    }));

  return (
    <div class="flex items-center gap-2 px-4 py-2 border-b border-(--color-border-muted) bg-(--color-bg-primary)">
      <FilterDropdown
        label="Author"
        options={authorOptions()}
        selected={prFiltersStore.selectedAuthors}
        onToggle={(v) => prFiltersStore.toggleAuthor(v)}
      />
      <FilterDropdown
        label="Label"
        options={labelOptions()}
        selected={prFiltersStore.selectedLabels}
        onToggle={(v) => prFiltersStore.toggleLabel(v)}
        loading={prFiltersStore.isLoadingLabels}
      />
      <FilterDropdown
        label="Assignee"
        options={assigneeOptions()}
        selected={prFiltersStore.selectedAssignees}
        onToggle={(v) => prFiltersStore.toggleAssignee(v)}
      />
      <ReviewStatusDropdown
        selected={prFiltersStore.selectedReviewStatus}
        onSelect={(v) => prFiltersStore.setReviewStatus(v)}
      />
      <Show when={prFiltersStore.hasActiveFilters}>
        <button
          class="ml-1 px-2 py-1 text-xs text-(--color-danger-fg) hover:bg-(--color-danger-emphasis)/10 rounded transition-colors cursor-pointer"
          onClick={() => prFiltersStore.clearAll()}
        >
          Clear filters
        </button>
      </Show>
    </div>
  );
}
