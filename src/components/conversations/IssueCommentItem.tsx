import type { IssueComment } from "~/types/pr";
import { relativeTime } from "~/lib/time";
import { open } from "@tauri-apps/plugin-shell";

interface IssueCommentItemProps {
  comment: IssueComment;
}

function stripMarkdown(text: string): string {
  return text
    .replace(/[#*`~\[\]()>]/g, "")
    .replace(/\n+/g, " ")
    .trim();
}

export function IssueCommentItem(props: IssueCommentItemProps) {
  const preview = () => {
    const stripped = stripMarkdown(props.comment.body);
    return stripped.length > 100 ? stripped.slice(0, 100) + "\u2026" : stripped;
  };

  return (
    <div
      class="px-3 py-2 cursor-pointer transition-colors border-b border-(--color-border-muted) hover:bg-(--color-bg-secondary)"
      onClick={() => open(props.comment.html_url)}
    >
      {/* Row 1: Author + timestamp */}
      <div class="flex items-center gap-1.5 mb-1">
        <img
          src={props.comment.author_avatar}
          alt={props.comment.author_login}
          class="w-4 h-4 rounded-full shrink-0"
        />
        <span class="text-xs font-semibold text-(--color-text-primary) truncate">
          {props.comment.author_login}
        </span>
        <span class="text-[10px] text-(--color-text-muted) shrink-0">
          {relativeTime(props.comment.created_at)}
        </span>
      </div>

      {/* Row 2: "Conversation" label */}
      <div class="flex items-center gap-1 mb-0.5">
        <svg class="w-3 h-3 text-(--color-text-muted) shrink-0" viewBox="0 0 16 16" fill="currentColor">
          <path d="M1 2.75C1 1.784 1.784 1 2.75 1h10.5c.966 0 1.75.784 1.75 1.75v7.5A1.75 1.75 0 0113.25 12H9.06l-2.573 2.573A1.458 1.458 0 014 13.543V12H2.75A1.75 1.75 0 011 10.25v-7.5z" />
        </svg>
        <span class="text-[10px] text-(--color-text-muted)">
          Conversation
        </span>
      </div>

      {/* Row 3: Body preview */}
      <p class="text-xs text-(--color-text-secondary) line-clamp-2 leading-snug">
        {preview()}
      </p>
    </div>
  );
}
