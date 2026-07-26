import type { PrReview } from "~/types/pr";
import { relativeTime } from "~/lib/time";
import { open } from "@tauri-apps/plugin-shell";

interface ReviewItemProps {
  review: PrReview;
}

function stripMarkdown(text: string): string {
  return text
    .replace(/[#*`~\[\]()>]/g, "")
    .replace(/\n+/g, " ")
    .trim();
}

const stateLabel: Record<string, string> = {
  APPROVED: "Approved",
  CHANGES_REQUESTED: "Changes requested",
  COMMENTED: "Reviewed",
  DISMISSED: "Dismissed",
};

const stateColor: Record<string, string> = {
  APPROVED: "text-(--color-success-fg)",
  CHANGES_REQUESTED: "text-(--color-danger-fg)",
  COMMENTED: "text-(--color-text-muted)",
  DISMISSED: "text-(--color-text-muted)",
};

export function ReviewItem(props: ReviewItemProps) {
  const preview = () => {
    const stripped = stripMarkdown(props.review.body);
    return stripped.length > 100 ? stripped.slice(0, 100) + "\u2026" : stripped;
  };

  const label = () => stateLabel[props.review.state] ?? props.review.state;
  const color = () => stateColor[props.review.state] ?? "text-(--color-text-muted)";

  return (
    <div
      class="px-3 py-2 cursor-pointer transition-colors border-b border-(--color-border-muted) hover:bg-(--color-bg-secondary)"
      onClick={() => open(props.review.html_url)}
    >
      {/* Row 1: Author + timestamp */}
      <div class="flex items-center gap-1.5 mb-1">
        <img
          src={props.review.author_avatar}
          alt={props.review.author_login}
          class="w-4 h-4 rounded-full shrink-0"
        />
        <span class="text-xs font-semibold text-(--color-text-primary) truncate">
          {props.review.author_login}
        </span>
        <span class="text-[10px] text-(--color-text-muted) shrink-0">
          {props.review.submitted_at ? relativeTime(props.review.submitted_at) : ""}
        </span>
      </div>

      {/* Row 2: Review state badge */}
      <div class="flex items-center gap-1 mb-0.5">
        <svg class={`w-3 h-3 shrink-0 ${color()}`} viewBox="0 0 16 16" fill="currentColor">
          <path d="M11.93 8.5a4.002 4.002 0 0 1-7.86 0H.75a.75.75 0 0 1 0-1.5h3.32a4.002 4.002 0 0 1 7.86 0h3.32a.75.75 0 0 1 0 1.5Zm-1.43-.75a2.5 2.5 0 1 0-5 0 2.5 2.5 0 0 0 5 0Z" />
        </svg>
        <span class={`text-[10px] font-medium ${color()}`}>
          {label()}
        </span>
      </div>

      {/* Row 3: Body preview (only if non-empty) */}
      {props.review.body.trim() && (
        <p class="text-xs text-(--color-text-secondary) line-clamp-2 leading-snug">
          {preview()}
        </p>
      )}
    </div>
  );
}
