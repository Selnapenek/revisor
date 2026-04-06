import { Show, For } from "solid-js";
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
      class="px-4 py-2 border-b border-(--color-border-muted) hover:bg-(--color-bg-secondary) cursor-pointer transition-colors"
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

      {/* Meta + labels row */}
      <div class="flex items-center gap-2 text-xs text-(--color-text-muted) flex-wrap">
        <span>{pr().author_login}</span>
        <span>&middot;</span>
        <span>{relativeTime(pr().updated_at)}</span>
        <Show when={pr().labels.length > 0}>
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
        </Show>
      </div>
    </div>
  );
}
