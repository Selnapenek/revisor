import { Show } from "solid-js";
import { authStore, prFiltersStore } from "~/stores";
import { FilterDropdown } from "./FilterDropdown";
import { ReviewStatusDropdown } from "./ReviewStatusDropdown";
import type { FilterOption } from "./FilterDropdown";

export function PRListAdvancedFilters() {
  const authorOptions = (): FilterOption[] => {
    const users = prFiltersStore.knownAuthors;
    const me = authStore.user;
    return me && !users.some((u) => u.login === me.login)
      ? [...users, { login: me.login, avatar_url: me.avatar_url }]
          .map((u) => ({ value: u.login, label: u.login, avatar: u.avatar_url }))
      : users.map((u) => ({ value: u.login, label: u.login, avatar: u.avatar_url }));
  };

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
    <div class="flex flex-wrap items-center gap-2 px-4 py-2 border-b border-(--color-border-muted) bg-(--color-bg-primary)">
      <button
        class="px-2 py-1 text-xs rounded border border-(--color-border-muted) hover:bg-(--color-bg-secondary) transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
        disabled={!authStore.user}
        onClick={() => authStore.user && prFiltersStore.applyAuthorPreset(authStore.user.login)}
      >
        Authored by me
      </button>
      <button
        class="px-2 py-1 text-xs rounded border border-(--color-border-muted) hover:bg-(--color-bg-secondary) transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
        disabled={!authStore.user}
        onClick={() => prFiltersStore.applyNeedsReviewPreset()}
      >
        Needs review from me
      </button>
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
