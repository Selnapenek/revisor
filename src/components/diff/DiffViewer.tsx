import { Show, Switch, Match, createSignal, createMemo, createEffect } from "solid-js";
import type { ProcessedFile } from "~/api/files";
import type { ReviewComment } from "~/types/pr";
import type { SearchRange } from "~/lib/search";
import { addComment, replyToComment, updateComment, deleteComment, resolveThread, unresolveThread } from "~/api";
import { prDetailStore, uiStore } from "~/stores";
import { searchStore } from "~/stores/search.store";
import { authStore } from "~/stores/auth.store";
import { UnifiedDiff } from "./UnifiedDiff";
import { SplitDiff } from "./SplitDiff";

export interface RepoContext {
  owner: string;
  repo: string;
  number: number;
}

export interface CommentActions {
  onAddComment: (line: number, side: "LEFT" | "RIGHT") => void;
  onSubmitNewComment: (body: string) => Promise<void>;
  onCancelNewComment: () => void;
  onReply: (commentId: number, body: string) => Promise<void>;
  onUpdate: (commentId: number, body: string) => Promise<void>;
  onDelete: (commentId: number) => Promise<void>;
  onResolve: (rootCommentId: number, threadNodeId: string) => Promise<void>;
  onUnresolve: (rootCommentId: number, threadNodeId: string) => Promise<void>;
}

export interface NewCommentTarget {
  line: number;
  side: "LEFT" | "RIGHT";
}

interface DiffViewerProps {
  file: ProcessedFile;
  comments: ReviewComment[];
  repo: RepoContext;
  isCollapsed: boolean;
  onToggleCollapsed: () => void;
  isReviewed: boolean;
  onToggleReviewed: () => void;
}

export function DiffViewer(props: DiffViewerProps) {
  const [newCommentTarget, setNewCommentTarget] = createSignal<NewCommentTarget | null>(null);

  const currentUser = () => authStore.user?.login ?? "";

  const commentActions: CommentActions = {
    onAddComment(line, side) {
      setNewCommentTarget({ line, side });
    },

    async onSubmitNewComment(body: string) {
      const target = newCommentTarget();
      const pr = prDetailStore.pr;
      if (!target || !pr) return;

      try {
        const result = await addComment(
          props.repo.owner,
          props.repo.repo,
          props.repo.number,
          props.file.filename,
          target.line,
          target.side,
          body,
          pr.head_sha,
        );
        prDetailStore.addComment(result);
        setNewCommentTarget(null);
      } catch (err) {
        uiStore.showError(`Failed to add comment: ${err}`);
      }
    },

    onCancelNewComment() {
      setNewCommentTarget(null);
    },

    async onReply(commentId: number, body: string) {
      try {
        const result = await replyToComment(
          props.repo.owner,
          props.repo.repo,
          props.repo.number,
          commentId,
          body,
        );
        prDetailStore.addComment(result);
      } catch (err) {
        uiStore.showError(`Failed to reply: ${err}`);
      }
    },

    async onUpdate(commentId: number, body: string) {
      try {
        await updateComment(
          props.repo.owner,
          props.repo.repo,
          commentId,
          body,
        );
        prDetailStore.updateComment(commentId, body);
      } catch (err) {
        uiStore.showError(`Failed to update comment: ${err}`);
      }
    },

    async onDelete(commentId: number) {
      try {
        await deleteComment(
          props.repo.owner,
          props.repo.repo,
          commentId,
        );
        prDetailStore.deleteComment(commentId);
      } catch (err) {
        uiStore.showError(`Failed to delete comment: ${err}`);
      }
    },

    async onResolve(rootCommentId: number, threadNodeId: string) {
      prDetailStore.setThreadResolved(rootCommentId, true);
      try {
        await resolveThread(threadNodeId);
      } catch (err) {
        prDetailStore.setThreadResolved(rootCommentId, false);
        uiStore.showError(`Failed to resolve thread: ${err}`);
      }
    },

    async onUnresolve(rootCommentId: number, threadNodeId: string) {
      prDetailStore.setThreadResolved(rootCommentId, false);
      try {
        await unresolveThread(threadNodeId);
      } catch (err) {
        prDetailStore.setThreadResolved(rootCommentId, true);
        uiStore.showError(`Failed to unresolve thread: ${err}`);
      }
    },
  };

  /**
   * Convert search store's hunkIdx:lineIdx keyed matches into a lineno-keyed map
   * that diff components can look up efficiently.
   * Keys: "old:N" for removed/context lines, "new:N" for added/context lines.
   */
  const searchMatchesByKey = createMemo((): Map<string, SearchRange[]> | undefined => {
    const fileMap = searchStore.matchesByFileAndLine.get(props.file.filename);
    if (!fileMap || !props.file.diff) return undefined;

    const active = searchStore.activeMatch;
    const result = new Map<string, SearchRange[]>();
    const hunks = props.file.diff.hunks;

    for (let hunkIdx = 0; hunkIdx < hunks.length; hunkIdx++) {
      const hunk = hunks[hunkIdx];
      for (let lineIdx = 0; lineIdx < hunk.lines.length; lineIdx++) {
        const key = `${hunkIdx}:${lineIdx}`;
        const matches = fileMap.get(key);
        if (!matches) continue;

        const line = hunk.lines[lineIdx];
        const ranges: SearchRange[] = matches.map((m) => ({
          start: m.start,
          end: m.end,
          isActive: active !== null && m.globalIdx === active.globalIdx,
        }));

        // Store under the appropriate lineno key(s)
        if (line.kind === "removed" && line.old_lineno != null) {
          result.set(`old:${line.old_lineno}`, ranges);
        } else if (line.kind === "added" && line.new_lineno != null) {
          result.set(`new:${line.new_lineno}`, ranges);
        } else if (line.kind === "context") {
          // Context lines appear on both sides in split view
          if (line.old_lineno != null) result.set(`old:${line.old_lineno}`, ranges);
          if (line.new_lineno != null) result.set(`new:${line.new_lineno}`, ranges);
        }
      }
    }

    return result.size > 0 ? result : undefined;
  });

  // Auto-scroll to active match
  createEffect(() => {
    const active = searchStore.activeMatch;
    if (!active || active.filePath !== props.file.filename) return;

    requestAnimationFrame(() => {
      const el = document.querySelector('[data-search-match="active"]');
      el?.scrollIntoView({ behavior: "smooth", block: "center" });
    });
  });

  return (
    <div data-file-section={props.file.filename} class="border-b border-(--color-border-muted)">
      {/* Sticky file header */}
      <div
        class="sticky top-0 z-10 flex items-center gap-3 px-4 py-2 border-b border-(--color-border-muted) bg-(--color-bg-secondary) cursor-pointer"
        onClick={props.onToggleCollapsed}
      >
        {/* Collapse chevron */}
        <svg
          class={`w-3.5 h-3.5 shrink-0 text-(--color-text-muted) transition-transform ${props.isCollapsed ? "-rotate-90" : ""}`}
          viewBox="0 0 16 16"
          fill="currentColor"
        >
          <path d="M12.78 6.22a.75.75 0 0 1 0 1.06l-4.25 4.25a.75.75 0 0 1-1.06 0L3.22 7.28a.75.75 0 0 1 1.06-1.06L8 9.94l3.72-3.72a.75.75 0 0 1 1.06 0Z" />
        </svg>

        <span class="text-sm font-mono text-(--color-text-primary) truncate flex-1">
          {props.file.filename}
        </span>

        <span class="flex items-center gap-1.5 text-xs shrink-0">
          <Show when={props.file.additions > 0}>
            <span class="text-(--color-success-fg)">+{props.file.additions}</span>
          </Show>
          <Show when={props.file.deletions > 0}>
            <span class="text-(--color-danger-fg)">-{props.file.deletions}</span>
          </Show>
        </span>

        {/* Reviewed checkbox */}
        <label
          class="flex items-center gap-1.5 text-xs text-(--color-text-muted) shrink-0 cursor-pointer"
          onClick={(e) => e.stopPropagation()}
        >
          <input
            type="checkbox"
            checked={props.isReviewed}
            onChange={props.onToggleReviewed}
            class="accent-(--color-accent-emphasis)"
          />
          Viewed
        </label>
      </div>

      {/* Diff content */}
      <Show when={!props.isCollapsed}>
        <div>
          <Show
            when={props.file.diff}
            fallback={
              <div class="flex items-center justify-center py-8 text-sm text-(--color-text-muted)">
                No diff available
              </div>
            }
          >
            {(diff) => (
              <Switch>
                <Match when={diff().is_binary}>
                  <div class="flex items-center justify-center py-8 text-sm text-(--color-text-muted)">
                    Binary file — not shown
                  </div>
                </Match>
                <Match when={diff().is_too_large}>
                  <div class="flex items-center justify-center py-8 text-sm text-(--color-text-muted)">
                    File too large to display
                  </div>
                </Match>
                <Match when={prDetailStore.diffMode === "split"}>
                  <SplitDiff
                    rows={diff().side_by_side}
                    wordDiffs={diff().word_diffs}
                    comments={props.comments}
                    commentActions={commentActions}
                    currentUser={currentUser()}
                    newCommentTarget={newCommentTarget()}
                    searchMatchesByKey={searchMatchesByKey()}
                    threadResolutions={prDetailStore.threadResolutions}
                  />
                </Match>
                <Match when={prDetailStore.diffMode === "unified"}>
                  <UnifiedDiff
                    hunks={diff().hunks}
                    wordDiffs={diff().word_diffs}
                    comments={props.comments}
                    commentActions={commentActions}
                    currentUser={currentUser()}
                    newCommentTarget={newCommentTarget()}
                    searchMatchesByKey={searchMatchesByKey()}
                    threadResolutions={prDetailStore.threadResolutions}
                  />
                </Match>
              </Switch>
            )}
          </Show>
        </div>
      </Show>
    </div>
  );
}
