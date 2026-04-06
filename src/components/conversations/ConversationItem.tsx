import { Show } from "solid-js";
import type { CommentThread } from "~/lib/comments";
import { relativeTime } from "~/lib/time";

interface ConversationItemProps {
  thread: CommentThread;
  isActive: boolean;
  onClick: () => void;
}

function stripMarkdown(text: string): string {
  return text
    .replace(/[#*`~\[\]()>]/g, "")
    .replace(/\n+/g, " ")
    .trim();
}

export function ConversationItem(props: ConversationItemProps) {
  const filePath = () => {
    const parts = props.thread.root.path.split("/");
    if (parts.length <= 2) return props.thread.root.path;
    return "…/" + parts.slice(-2).join("/");
  };

  const preview = () => {
    const stripped = stripMarkdown(props.thread.root.body);
    return stripped.length > 100 ? stripped.slice(0, 100) + "…" : stripped;
  };

  const replyCount = () => props.thread.replies.length;

  return (
    <div
      class={`px-3 py-2 cursor-pointer transition-colors border-b border-(--color-border-muted) ${
        props.isActive
          ? "bg-(--color-bg-tertiary)"
          : "hover:bg-(--color-bg-secondary)"
      } ${props.thread.isResolved ? "opacity-60" : ""}`}
      onClick={props.onClick}
    >
      {/* Row 1: Author + timestamp + reply count */}
      <div class="flex items-center gap-1.5 mb-1">
        <Show when={props.thread.isResolved}>
          <svg class="w-3 h-3 text-(--color-success-fg) shrink-0" viewBox="0 0 16 16" fill="currentColor">
            <path d="M8 16A8 8 0 1 1 8 0a8 8 0 0 1 0 16Zm3.78-9.72a.751.751 0 0 0-.018-1.042.751.751 0 0 0-1.042-.018L6.75 9.19 5.28 7.72a.751.751 0 0 0-1.042.018.751.751 0 0 0-.018 1.042l2 2a.75.75 0 0 0 1.06 0Z" />
          </svg>
        </Show>
        <img
          src={props.thread.root.author_avatar}
          alt={props.thread.root.author_login}
          class="w-4 h-4 rounded-full shrink-0"
        />
        <span class="text-xs font-semibold text-(--color-text-primary) truncate">
          {props.thread.root.author_login}
        </span>
        <span class="text-[10px] text-(--color-text-muted) shrink-0">
          {relativeTime(props.thread.root.created_at)}
        </span>
        <div class="flex-1" />
        <Show when={replyCount() > 0}>
          <span class="shrink-0 flex items-center gap-0.5 text-[10px] text-(--color-accent-fg)">
            <svg width="10" height="10" viewBox="0 0 16 16" fill="currentColor">
              <path d="M1 2.75C1 1.784 1.784 1 2.75 1h10.5c.966 0 1.75.784 1.75 1.75v7.5A1.75 1.75 0 0113.25 12H9.06l-2.573 2.573A1.458 1.458 0 014 13.543V12H2.75A1.75 1.75 0 011 10.25v-7.5z" />
            </svg>
            {replyCount()}
          </span>
        </Show>
      </div>

      {/* Row 2: File path + line reference */}
      <div class="flex items-center gap-1.5 mb-0.5">
        <span class="text-[10px] font-mono text-(--color-text-muted) truncate">
          {filePath()}
        </span>
        <span class="text-[10px] text-(--color-text-muted) shrink-0">
          L{props.thread.line}
        </span>
      </div>

      {/* Row 3: Body preview */}
      <p class="text-xs text-(--color-text-secondary) line-clamp-2 leading-snug">
        {preview()}
      </p>
    </div>
  );
}
