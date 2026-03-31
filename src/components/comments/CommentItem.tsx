import { createSignal, Show } from "solid-js";
import type { ReviewComment } from "~/types/pr";
import { relativeTime } from "~/lib/time";
import { Button } from "~/components/ui/Button";
import { Markdown } from "./Markdown";

interface CommentItemProps {
  comment: ReviewComment;
  isOwn: boolean;
  onUpdate: (commentId: number, body: string) => Promise<void>;
  onDelete: (commentId: number) => Promise<void>;
}

export function CommentItem(props: CommentItemProps) {
  const [menuOpen, setMenuOpen] = createSignal(false);
  const [editing, setEditing] = createSignal(false);
  const [editBody, setEditBody] = createSignal("");
  const [saving, setSaving] = createSignal(false);
  const [confirmDelete, setConfirmDelete] = createSignal(false);
  const [deleting, setDeleting] = createSignal(false);

  const startEdit = () => {
    setEditBody(props.comment.body);
    setEditing(true);
    setMenuOpen(false);
  };

  const saveEdit = async () => {
    const text = editBody().trim();
    if (!text || text === props.comment.body) {
      setEditing(false);
      return;
    }
    setSaving(true);
    try {
      await props.onUpdate(props.comment.id, text);
      setEditing(false);
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async () => {
    setDeleting(true);
    try {
      await props.onDelete(props.comment.id);
    } finally {
      setDeleting(false);
      setConfirmDelete(false);
    }
  };

  const handleEditKeyDown = (e: KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
      e.preventDefault();
      saveEdit();
    }
    if (e.key === "Escape") {
      setEditing(false);
    }
  };

  return (
    <div class="px-3 py-2 group/comment">
      {/* Header */}
      <div class="flex items-center gap-2 mb-1">
        <img
          src={props.comment.author_avatar}
          alt={props.comment.author_login}
          class="w-4 h-4 rounded-full"
        />
        <span class="text-xs font-semibold text-(--color-text-primary)">
          {props.comment.author_login}
        </span>
        <span class="text-xs text-(--color-text-muted)">
          {relativeTime(props.comment.created_at)}
        </span>
        <div class="flex-1" />

        {/* Actions menu for own comments */}
        <Show when={props.isOwn && !editing()}>
          <div class="relative">
            <button
              class="opacity-0 group-hover/comment:opacity-100 text-xs text-(--color-text-muted) hover:text-(--color-text-primary) px-1 rounded cursor-pointer transition-opacity"
              onClick={() => setMenuOpen(!menuOpen())}
            >
              &middot;&middot;&middot;
            </button>
            <Show when={menuOpen()}>
              <div
                class="absolute right-0 top-full mt-1 z-20 bg-(--color-bg-secondary) border border-(--color-border-default) rounded shadow-lg py-1 min-w-[100px]"
              >
                <button
                  class="w-full text-left px-3 py-1 text-xs text-(--color-text-primary) hover:bg-(--color-bg-tertiary) cursor-pointer"
                  onClick={startEdit}
                >
                  Edit
                </button>
                <button
                  class="w-full text-left px-3 py-1 text-xs text-(--color-danger-fg) hover:bg-(--color-bg-tertiary) cursor-pointer"
                  onClick={() => { setConfirmDelete(true); setMenuOpen(false); }}
                >
                  Delete
                </button>
              </div>
              {/* Backdrop to close menu */}
              <div class="fixed inset-0 z-10" onClick={() => setMenuOpen(false)} />
            </Show>
          </div>
        </Show>
      </div>

      {/* Body or edit form */}
      <Show
        when={!editing()}
        fallback={
          <div class="flex flex-col gap-2 mt-1">
            <textarea
              ref={(el) => setTimeout(() => el.focus(), 0)}
              class="w-full min-h-[60px] px-2 py-1.5 text-xs font-mono rounded border border-(--color-border-default) bg-(--color-bg-primary) text-(--color-text-primary) resize-y focus:outline-none focus:ring-2 focus:ring-(--color-accent-emphasis)"
              value={editBody()}
              onInput={(e) => setEditBody(e.currentTarget.value)}
              onKeyDown={handleEditKeyDown}
              disabled={saving()}
            />
            <div class="flex gap-2 justify-end">
              <Button variant="ghost" size="sm" onClick={() => setEditing(false)} disabled={saving()}>
                Cancel
              </Button>
              <Button variant="primary" size="sm" onClick={saveEdit} loading={saving()} disabled={!editBody().trim() || saving()}>
                Save
              </Button>
            </div>
          </div>
        }
      >
        <Markdown content={props.comment.body} />
      </Show>

      {/* Delete confirmation */}
      <Show when={confirmDelete()}>
        <div class="flex items-center gap-2 mt-2 p-2 rounded bg-(--color-danger-emphasis)/10 border border-(--color-danger-fg)/20">
          <span class="text-xs text-(--color-danger-fg) flex-1">Delete this comment?</span>
          <Button variant="ghost" size="sm" onClick={() => setConfirmDelete(false)} disabled={deleting()}>
            No
          </Button>
          <Button variant="danger" size="sm" onClick={handleDelete} loading={deleting()}>
            Yes
          </Button>
        </div>
      </Show>
    </div>
  );
}
