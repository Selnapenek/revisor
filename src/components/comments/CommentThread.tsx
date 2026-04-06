import { createSignal, For, Show } from "solid-js";
import type { CommentThread as CommentThreadType } from "~/lib/comments";
import { CommentItem } from "./CommentItem";
import { CommentForm } from "./CommentForm";

interface CommentThreadProps {
  thread: CommentThreadType;
  currentUser: string;
  onReply: (commentId: number, body: string) => Promise<void>;
  onUpdate: (commentId: number, body: string) => Promise<void>;
  onDelete: (commentId: number) => Promise<void>;
  onResolve?: (rootCommentId: number, threadNodeId: string) => Promise<void>;
  onUnresolve?: (rootCommentId: number, threadNodeId: string) => Promise<void>;
}

function stripMarkdown(text: string): string {
  return text
    .replace(/[#*`~\[\]()>]/g, "")
    .replace(/\n+/g, " ")
    .trim();
}

export function CommentThreadView(props: CommentThreadProps) {
  const [replying, setReplying] = createSignal(false);
  const [collapsed, setCollapsed] = createSignal(props.thread.isResolved);
  const [resolving, setResolving] = createSignal(false);

  const allComments = () => [props.thread.root, ...props.thread.replies];

  const handleReply = async (body: string) => {
    await props.onReply(props.thread.root.id, body);
    setReplying(false);
  };

  const handleResolve = async () => {
    if (!props.thread.threadNodeId || !props.onResolve) return;
    setResolving(true);
    try {
      await props.onResolve(props.thread.root.id, props.thread.threadNodeId);
      setCollapsed(true);
    } finally {
      setResolving(false);
    }
  };

  const handleUnresolve = async () => {
    if (!props.thread.threadNodeId || !props.onUnresolve) return;
    setResolving(true);
    try {
      await props.onUnresolve(props.thread.root.id, props.thread.threadNodeId);
      setCollapsed(false);
    } finally {
      setResolving(false);
    }
  };

  const preview = () => {
    const stripped = stripMarkdown(props.thread.root.body);
    return stripped.length > 80 ? stripped.slice(0, 80) + "…" : stripped;
  };

  return (
    <div
      class={`rounded border overflow-hidden min-w-0 transition-opacity ${
        props.thread.isResolved
          ? "border-(--color-success-emphasis)/30 bg-(--color-bg-secondary)"
          : "border-(--color-border-default) bg-(--color-bg-secondary)"
      }`}
    >
      {/* Collapsed resolved state */}
      <Show when={props.thread.isResolved && collapsed()}>
        <button
          class="w-full flex items-center gap-2 px-3 py-2 text-left cursor-pointer hover:bg-(--color-bg-tertiary) transition-colors"
          onClick={() => setCollapsed(false)}
        >
          {/* Check icon */}
          <svg class="w-3.5 h-3.5 text-(--color-success-fg) shrink-0" viewBox="0 0 16 16" fill="currentColor">
            <path d="M8 16A8 8 0 1 1 8 0a8 8 0 0 1 0 16Zm3.78-9.72a.751.751 0 0 0-.018-1.042.751.751 0 0 0-1.042-.018L6.75 9.19 5.28 7.72a.751.751 0 0 0-1.042.018.751.751 0 0 0-.018 1.042l2 2a.75.75 0 0 0 1.06 0Z" />
          </svg>
          <img
            src={props.thread.root.author_avatar}
            alt={props.thread.root.author_login}
            class="w-4 h-4 rounded-full shrink-0"
          />
          <span class="text-xs text-(--color-text-muted) truncate">
            <span class="font-medium text-(--color-text-secondary)">{props.thread.root.author_login}</span>
            {" — "}
            {preview()}
          </span>
          <Show when={props.thread.replies.length > 0}>
            <span class="shrink-0 text-[10px] text-(--color-text-muted)">
              +{props.thread.replies.length}
            </span>
          </Show>
          <div class="flex-1" />
          {/* Expand chevron */}
          <svg class="w-3 h-3 text-(--color-text-muted) shrink-0" viewBox="0 0 16 16" fill="currentColor">
            <path d="M12.78 5.22a.749.749 0 0 1 0 1.06l-4.25 4.25a.749.749 0 0 1-1.06 0L3.22 6.28a.751.751 0 0 1 .018-1.042.751.751 0 0 1 1.042-.018L8 8.94l3.72-3.72a.749.749 0 0 1 1.06 0Z" />
          </svg>
        </button>
      </Show>

      {/* Expanded state */}
      <Show when={!collapsed() || !props.thread.isResolved}>
        {/* Resolved badge bar */}
        <Show when={props.thread.isResolved}>
          <div class="flex items-center gap-1.5 px-3 py-1.5 bg-(--color-success-subtle) border-b border-(--color-success-emphasis)/20">
            <svg class="w-3 h-3 text-(--color-success-fg)" viewBox="0 0 16 16" fill="currentColor">
              <path d="M8 16A8 8 0 1 1 8 0a8 8 0 0 1 0 16Zm3.78-9.72a.751.751 0 0 0-.018-1.042.751.751 0 0 0-1.042-.018L6.75 9.19 5.28 7.72a.751.751 0 0 0-1.042.018.751.751 0 0 0-.018 1.042l2 2a.75.75 0 0 0 1.06 0Z" />
            </svg>
            <span class="text-[11px] font-medium text-(--color-success-fg)">Resolved</span>
            <div class="flex-1" />
            <button
              class="text-[10px] text-(--color-text-muted) hover:text-(--color-text-secondary) cursor-pointer"
              onClick={() => setCollapsed(true)}
            >
              Collapse
            </button>
          </div>
        </Show>

        <div class={props.thread.isResolved ? "opacity-70" : ""}>
          <For each={allComments()}>
            {(comment, i) => (
              <div class={i() > 0 ? "border-t border-(--color-border-muted)" : ""}>
                <CommentItem
                  comment={comment}
                  isOwn={comment.author_login === props.currentUser}
                  onUpdate={props.onUpdate}
                  onDelete={props.onDelete}
                />
              </div>
            )}
          </For>
        </div>

        {/* Reply + Resolve area */}
        <div class="border-t border-(--color-border-muted)">
          <Show
            when={replying()}
            fallback={
              <div class="flex items-center">
                <button
                  class="flex-1 px-3 py-2 text-left text-xs text-(--color-text-muted) hover:bg-(--color-bg-tertiary) cursor-pointer transition-colors"
                  onClick={() => setReplying(true)}
                >
                  Reply...
                </button>
                <Show when={props.thread.threadNodeId}>
                  <Show
                    when={!props.thread.isResolved}
                    fallback={
                      <button
                        class="flex items-center gap-1 px-3 py-2 text-xs text-(--color-text-muted) hover:text-(--color-text-primary) hover:bg-(--color-bg-tertiary) cursor-pointer transition-colors disabled:opacity-50"
                        onClick={handleUnresolve}
                        disabled={resolving()}
                      >
                        <svg class="w-3 h-3" viewBox="0 0 16 16" fill="currentColor">
                          <path d="M3.38 3.012a.749.749 0 0 1 1.06-.012L8 6.44l3.56-3.44a.749.749 0 1 1 1.04 1.08L9.12 7.52l3.48 3.44a.749.749 0 1 1-1.04 1.08L8 8.6 4.44 12.04a.749.749 0 1 1-1.04-1.08l3.48-3.44L3.4 4.08a.749.749 0 0 1-.02-1.068Z" />
                        </svg>
                        Unresolve
                      </button>
                    }
                  >
                    <button
                      class="flex items-center gap-1 px-3 py-2 text-xs text-(--color-success-fg) hover:bg-(--color-bg-tertiary) cursor-pointer transition-colors disabled:opacity-50"
                      onClick={handleResolve}
                      disabled={resolving()}
                    >
                      <svg class="w-3 h-3" viewBox="0 0 16 16" fill="currentColor">
                        <path d="M13.78 4.22a.75.75 0 0 1 0 1.06l-7.25 7.25a.75.75 0 0 1-1.06 0L2.22 9.28a.751.751 0 0 1 .018-1.042.751.751 0 0 1 1.042-.018L6 10.94l6.72-6.72a.75.75 0 0 1 1.06 0Z" />
                      </svg>
                      Resolve
                    </button>
                  </Show>
                </Show>
              </div>
            }
          >
            <div class="p-3">
              <CommentForm
                onSubmit={handleReply}
                onCancel={() => setReplying(false)}
                placeholder="Write a reply..."
                submitLabel="Reply"
              />
            </div>
          </Show>
        </div>
      </Show>
    </div>
  );
}
