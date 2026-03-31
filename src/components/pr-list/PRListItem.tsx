import { Show, For } from "solid-js";
import type { PullRequest } from "~/types/pr";
import { Badge } from "~/components/ui/Badge";
import { relativeTime } from "~/lib/time";

interface PRListItemProps {
  pr: PullRequest;
  onClick: () => void;
  isSearchMode?: boolean;
}

export function PRListItem(props: PRListItemProps) {
  const pr = () => props.pr;

  const statusVariant = () => {
    if (pr().merged_at) return "info" as const;
    if (pr().state === "closed") return "danger" as const;
    if (pr().draft) return "default" as const;
    return "success" as const;
  };

  const statusLabel = () => {
    if (pr().merged_at) return "Merged";
    if (pr().state === "closed") return "Closed";
    if (pr().draft) return "Draft";
    return "Open";
  };

  return (
    <div
      class="px-4 py-3 border-b border-(--color-border-muted) hover:bg-(--color-bg-secondary) cursor-pointer transition-colors"
      onClick={props.onClick}
    >
      {/* Title row */}
      <div class="flex items-start gap-2 mb-1">
        <Badge variant={statusVariant()} class="shrink-0 mt-0.5">
          {statusLabel()}
        </Badge>
        <span class="text-sm font-medium text-(--color-text-primary) line-clamp-1">
          {pr().title}
        </span>
        <span class="text-xs text-(--color-text-muted) shrink-0 ml-auto">
          #{pr().number}
        </span>
      </div>

      {/* Labels row */}
      <Show when={pr().labels.length > 0}>
        <div class="flex items-center gap-1.5 ml-[52px] mb-1 flex-wrap">
          <For each={pr().labels}>
            {(label) => (
              <span
                class="inline-flex items-center gap-1 px-1.5 py-0.5 text-[10px] font-medium rounded-full border border-black/10"
                style={{ "background-color": `#${label.color}20`, color: `#${label.color}` }}
              >
                <span
                  class="w-2 h-2 rounded-full"
                  style={{ "background-color": `#${label.color}` }}
                />
                {label.name}
              </span>
            )}
          </For>
        </div>
      </Show>

      {/* Meta row */}
      <div class="flex items-center gap-3 text-xs text-(--color-text-muted) ml-[52px]">
        <span>{pr().author_login}</span>
        <span>{relativeTime(pr().updated_at)}</span>

        <Show when={!props.isSearchMode && pr().changed_files > 0}>
          <span class="flex items-center gap-1">
            <span>{pr().changed_files} files</span>
          </span>
        </Show>

        <Show when={!props.isSearchMode}>
          <span class="flex items-center gap-1.5">
            <Show when={pr().additions > 0}>
              <span class="text-(--color-success-fg)">+{pr().additions}</span>
            </Show>
            <Show when={pr().deletions > 0}>
              <span class="text-(--color-danger-fg)">-{pr().deletions}</span>
            </Show>
          </span>
        </Show>

        <Show when={!props.isSearchMode && pr().review_comments_count > 0}>
          <span class="ml-auto">{pr().review_comments_count} comments</span>
        </Show>
      </div>
    </div>
  );
}
