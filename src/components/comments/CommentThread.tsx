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
}

export function CommentThreadView(props: CommentThreadProps) {
  const [replying, setReplying] = createSignal(false);

  const allComments = () => [props.thread.root, ...props.thread.replies];

  const handleReply = async (body: string) => {
    await props.onReply(props.thread.root.id, body);
    setReplying(false);
  };

  return (
    <div class="rounded border border-(--color-border-default) bg-(--color-bg-secondary) overflow-hidden min-w-0">
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

      {/* Reply area */}
      <div class="border-t border-(--color-border-muted)">
        <Show
          when={replying()}
          fallback={
            <button
              class="w-full px-3 py-2 text-left text-xs text-(--color-text-muted) hover:bg-(--color-bg-tertiary) cursor-pointer transition-colors"
              onClick={() => setReplying(true)}
            >
              Reply...
            </button>
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
    </div>
  );
}
