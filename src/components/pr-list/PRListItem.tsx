import { Show } from "solid-js";
import type { PullRequest } from "~/types/pr";
import { Badge } from "~/components/ui/Badge";
import { relativeTime } from "~/lib/time";

interface PRListItemProps {
  pr: PullRequest;
  onClick: () => void;
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

      {/* Meta row */}
      <div class="flex items-center gap-3 text-xs text-(--color-text-muted) ml-[52px]">
        <span>{pr().author_login}</span>
        <span>{relativeTime(pr().updated_at)}</span>

        <Show when={pr().changed_files > 0}>
          <span class="flex items-center gap-1">
            <span>{pr().changed_files} files</span>
          </span>
        </Show>

        <span class="flex items-center gap-1.5">
          <Show when={pr().additions > 0}>
            <span class="text-(--color-success-fg)">+{pr().additions}</span>
          </Show>
          <Show when={pr().deletions > 0}>
            <span class="text-(--color-danger-fg)">-{pr().deletions}</span>
          </Show>
        </span>

        <Show when={pr().review_comments_count > 0}>
          <span class="ml-auto">{pr().review_comments_count} comments</span>
        </Show>
      </div>
    </div>
  );
}
