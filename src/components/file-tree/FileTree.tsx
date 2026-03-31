import { For, Show, createMemo } from "solid-js";
import type { ProcessedFile } from "~/api/files";
import { prDetailStore } from "~/stores";
import { searchStore } from "~/stores/search.store";
import { Badge } from "~/components/ui/Badge";

function useFileCommentCounts() {
  return createMemo(() => {
    const counts = new Map<string, number>();
    for (const c of prDetailStore.comments) {
      if (c.in_reply_to_id !== null) continue; // only count thread roots
      counts.set(c.path, (counts.get(c.path) ?? 0) + 1);
    }
    return counts;
  });
}

interface FileTreeProps {
  files: ProcessedFile[];
  selectedPath: string | null;
  onSelect: (path: string) => void;
  onToggleReviewed: (path: string) => void;
  fileFilterRef?: (el: HTMLInputElement) => void;
}

export function FileTree(props: FileTreeProps) {
  const progress = createMemo(() => prDetailStore.reviewProgress);
  const commentCounts = useFileCommentCounts();

  const progressPct = createMemo(() => {
    const p = progress();
    if (p.total === 0) return 0;
    return Math.round((p.reviewed / p.total) * 100);
  });

  return (
    <div class="h-full flex flex-col">
      {/* File filter */}
      <div class="px-3 py-2 border-b border-(--color-border-muted) shrink-0">
        <div class="flex items-center gap-1.5 px-2 py-1 rounded bg-(--color-bg-tertiary) border border-(--color-border-muted) focus-within:border-(--color-accent-emphasis) transition-colors">
          <svg class="w-3 h-3 text-(--color-text-muted) shrink-0" viewBox="0 0 16 16" fill="currentColor">
            <path d="M11.5 7a4.5 4.5 0 1 1-9 0 4.5 4.5 0 0 1 9 0Zm-.82 4.74a6 6 0 1 1 1.06-1.06l3.04 3.04a.75.75 0 1 1-1.06 1.06l-3.04-3.04Z" />
          </svg>
          <input
            ref={(el) => props.fileFilterRef?.(el)}
            type="text"
            class="flex-1 bg-transparent text-xs text-(--color-text-primary) placeholder:text-(--color-text-muted) outline-none min-w-0"
            placeholder="Filter files..."
            value={searchStore.fileFilterQuery}
            onInput={(e) => searchStore.setFileFilterQuery(e.currentTarget.value)}
          />
          <Show when={searchStore.fileFilterQuery}>
            <button
              class="w-3.5 h-3.5 flex items-center justify-center rounded text-(--color-text-muted) hover:text-(--color-text-primary) cursor-pointer"
              onClick={() => searchStore.setFileFilterQuery("")}
            >
              <svg class="w-3 h-3" viewBox="0 0 16 16" fill="currentColor">
                <path d="M3.72 3.72a.75.75 0 0 1 1.06 0L8 6.94l3.22-3.22a.749.749 0 0 1 1.275.326.749.749 0 0 1-.215.734L9.06 8l3.22 3.22a.749.749 0 0 1-.326 1.275.749.749 0 0 1-.734-.215L8 9.06l-3.22 3.22a.751.751 0 0 1-1.042-.018.751.751 0 0 1-.018-1.042L6.94 8 3.72 4.78a.75.75 0 0 1 0-1.06Z" />
              </svg>
            </button>
          </Show>
          {/* Advanced filter toggle */}
          <button
            class={`w-3.5 h-3.5 flex items-center justify-center rounded shrink-0 cursor-pointer transition-colors ${
              searchStore.hasActiveGlobFilters
                ? "text-(--color-accent-fg)"
                : "text-(--color-text-muted) hover:text-(--color-text-primary)"
            }`}
            onClick={() => searchStore.toggleAdvancedFilters()}
            title="Toggle file glob filters"
          >
            <svg class="w-3 h-3" viewBox="0 0 16 16" fill="currentColor">
              <path d="M.75 3h14.5a.75.75 0 0 1 0 1.5H.75a.75.75 0 0 1 0-1.5ZM3 7.75A.75.75 0 0 1 3.75 7h8.5a.75.75 0 0 1 0 1.5h-8.5A.75.75 0 0 1 3 7.75Zm3 4a.75.75 0 0 1 .75-.75h2.5a.75.75 0 0 1 0 1.5h-2.5a.75.75 0 0 1-.75-.75Z" />
            </svg>
          </button>
        </div>

        {/* Advanced glob filter fields */}
        <Show when={searchStore.advancedFiltersOpen}>
          <div class="mt-2 flex flex-col gap-2">
            <div>
              <label class="block text-[10px] text-(--color-text-muted) uppercase tracking-wider mb-0.5">
                files to include
              </label>
              <input
                type="text"
                class="w-full px-2 py-1 text-xs rounded bg-(--color-bg-tertiary) border border-(--color-border-muted) text-(--color-text-primary) placeholder:text-(--color-text-muted) outline-none focus:border-(--color-accent-emphasis) transition-colors"
                placeholder="e.g. *.ts, src/**"
                value={searchStore.includePattern}
                onInput={(e) => searchStore.setIncludePattern(e.currentTarget.value)}
              />
            </div>
            <div>
              <label class="block text-[10px] text-(--color-text-muted) uppercase tracking-wider mb-0.5">
                files to exclude
              </label>
              <input
                type="text"
                class="w-full px-2 py-1 text-xs rounded bg-(--color-bg-tertiary) border border-(--color-border-muted) text-(--color-text-primary) placeholder:text-(--color-text-muted) outline-none focus:border-(--color-accent-emphasis) transition-colors"
                placeholder="e.g. *.test.ts, dist/**"
                value={searchStore.excludePattern}
                onInput={(e) => searchStore.setExcludePattern(e.currentTarget.value)}
              />
            </div>
          </div>
        </Show>
      </div>

      {/* Progress header */}
      <div class="px-3 py-2 border-b border-(--color-border-muted) shrink-0">
        <div class="flex items-center justify-between text-xs text-(--color-text-secondary) mb-1.5">
          <span>Files reviewed</span>
          <span>{progress().reviewed}/{progress().total}</span>
        </div>
        <div class="h-1.5 bg-(--color-bg-tertiary) rounded-full overflow-hidden">
          <div
            class="h-full bg-(--color-success-emphasis) rounded-full transition-all duration-300"
            style={{ width: `${progressPct()}%` }}
          />
        </div>
      </div>

      {/* File list */}
      <div class="flex-1 overflow-auto">
        <Show
          when={props.files.length > 0}
          fallback={
            <Show when={searchStore.fileFilterQuery || searchStore.hasActiveGlobFilters}>
              <div class="px-3 py-4 text-xs text-(--color-text-muted) text-center">
                <p>No files match current filters</p>
                <Show when={searchStore.hasActiveGlobFilters}>
                  <button
                    class="mt-2 text-[10px] text-(--color-accent-fg) hover:underline cursor-pointer"
                    onClick={() => {
                      searchStore.setIncludePattern("");
                      searchStore.setExcludePattern("");
                    }}
                  >
                    Clear glob filters
                  </button>
                </Show>
              </div>
            </Show>
          }
        >
          <For each={props.files}>
            {(file) => (
              <FileTreeItem
                file={file}
                isSelected={props.selectedPath === file.filename}
                isReviewed={prDetailStore.isFileReviewed(file.filename)}
                commentCount={commentCounts().get(file.filename) ?? 0}
                searchMatchCount={searchStore.codeSearchActive ? (searchStore.matchCountByFile.get(file.filename) ?? 0) : 0}
                onSelect={() => props.onSelect(file.filename)}
                onToggleReviewed={() => props.onToggleReviewed(file.filename)}
              />
            )}
          </For>
        </Show>
      </div>
    </div>
  );
}

interface FileTreeItemProps {
  file: ProcessedFile;
  isSelected: boolean;
  isReviewed: boolean;
  commentCount: number;
  searchMatchCount: number;
  onSelect: () => void;
  onToggleReviewed: () => void;
}

function FileTreeItem(props: FileTreeItemProps) {
  const statusBadge = () => {
    switch (props.file.status) {
      case "added": return { label: "A", variant: "success" as const };
      case "removed": return { label: "D", variant: "danger" as const };
      case "renamed": return { label: "R", variant: "warning" as const };
      default: return { label: "M", variant: "info" as const };
    }
  };

  const filename = () => {
    const parts = props.file.filename.split("/");
    return parts[parts.length - 1];
  };

  const directory = () => {
    const parts = props.file.filename.split("/");
    if (parts.length <= 1) return "";
    return parts.slice(0, -1).join("/") + "/";
  };

  return (
    <div
      class={`flex items-center gap-2 px-3 py-1.5 text-xs cursor-pointer transition-colors group ${
        props.isSelected
          ? "bg-(--color-bg-tertiary)"
          : "hover:bg-(--color-bg-secondary)"
      }`}
      onClick={props.onSelect}
    >
      {/* Review checkbox */}
      <button
        class={`w-3.5 h-3.5 rounded border shrink-0 flex items-center justify-center transition-colors cursor-pointer ${
          props.isReviewed
            ? "bg-(--color-success-emphasis) border-(--color-success-emphasis)"
            : "border-(--color-border-default) hover:border-(--color-text-muted)"
        }`}
        onClick={(e) => {
          e.stopPropagation();
          props.onToggleReviewed();
        }}
      >
        <Show when={props.isReviewed}>
          <svg width="8" height="8" viewBox="0 0 8 8" fill="none">
            <path d="M1.5 4L3 5.5L6.5 2" stroke="white" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        </Show>
      </button>

      {/* Status badge */}
      <Badge variant={statusBadge().variant} class="text-[10px] px-1 py-0">
        {statusBadge().label}
      </Badge>

      {/* Filename */}
      <span class="truncate flex-1 min-w-0">
        <span class="text-(--color-text-muted)">{directory()}</span>
        <span class={`font-medium ${props.isReviewed ? "text-(--color-text-muted) line-through" : "text-(--color-text-primary)"}`}>
          {filename()}
        </span>
      </span>

      {/* Search match count */}
      <Show when={props.searchMatchCount > 0}>
        <span class="shrink-0 px-1 py-0 text-[10px] font-medium rounded bg-(--color-search-match) text-(--color-search-match-border) tabular-nums">
          {props.searchMatchCount}
        </span>
      </Show>

      {/* Comment count */}
      <Show when={props.commentCount > 0}>
        <span class="shrink-0 flex items-center gap-0.5 text-[10px] text-(--color-accent-fg)">
          <svg width="10" height="10" viewBox="0 0 16 16" fill="currentColor">
            <path d="M1 2.75C1 1.784 1.784 1 2.75 1h10.5c.966 0 1.75.784 1.75 1.75v7.5A1.75 1.75 0 0113.25 12H9.06l-2.573 2.573A1.458 1.458 0 014 13.543V12H2.75A1.75 1.75 0 011 10.25v-7.5z"/>
          </svg>
          {props.commentCount}
        </span>
      </Show>

      {/* Stats */}
      <span class="shrink-0 flex gap-1 text-[10px]">
        <Show when={props.file.additions > 0}>
          <span class="text-(--color-success-fg)">+{props.file.additions}</span>
        </Show>
        <Show when={props.file.deletions > 0}>
          <span class="text-(--color-danger-fg)">-{props.file.deletions}</span>
        </Show>
      </span>
    </div>
  );
}
