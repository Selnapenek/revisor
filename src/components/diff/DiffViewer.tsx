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
import { CodeSearchBar } from "./CodeSearchBar";

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
    <div class="h-full flex flex-col">
      {/* File header */}
      <div class="flex items-center gap-3 px-4 py-2 border-b border-(--color-border-muted) bg-(--color-bg-secondary) shrink-0">
        {/* File navigation */}
        <div class="flex items-center gap-1 shrink-0">
          <button
            class="w-5 h-5 flex items-center justify-center rounded text-(--color-text-muted) hover:text-(--color-text-primary) hover:bg-(--color-bg-tertiary) transition-colors cursor-pointer disabled:opacity-30 disabled:cursor-default"
            onClick={() => prDetailStore.navigateFile(-1, searchStore.filteredFiles)}
            disabled={searchStore.filteredFiles.length <= 1}
            title="Previous file (←)"
          >
            <svg class="w-3 h-3" viewBox="0 0 16 16" fill="currentColor">
              <path d="M9.78 3.22a.75.75 0 0 1 0 1.06L6.06 8l3.72 3.72a.75.75 0 1 1-1.06 1.06L4.47 8.53a.75.75 0 0 1 0-1.06l4.25-4.25a.75.75 0 0 1 1.06 0Z" />
            </svg>
          </button>
          <span class="text-[10px] text-(--color-text-muted) tabular-nums min-w-[3ch] text-center">
            {prDetailStore.selectedFileIndex(searchStore.filteredFiles) + 1}/{searchStore.filteredFiles.length}
          </span>
          <button
            class="w-5 h-5 flex items-center justify-center rounded text-(--color-text-muted) hover:text-(--color-text-primary) hover:bg-(--color-bg-tertiary) transition-colors cursor-pointer disabled:opacity-30 disabled:cursor-default"
            onClick={() => prDetailStore.navigateFile(1, searchStore.filteredFiles)}
            disabled={searchStore.filteredFiles.length <= 1}
            title="Next file (→)"
          >
            <svg class="w-3 h-3" viewBox="0 0 16 16" fill="currentColor">
              <path d="M6.22 3.22a.75.75 0 0 1 1.06 0l4.25 4.25a.75.75 0 0 1 0 1.06l-4.25 4.25a.751.751 0 0 1-1.042-.018.751.751 0 0 1-.018-1.042L9.94 8 6.22 4.28a.75.75 0 0 1 0-1.06Z" />
            </svg>
          </button>
        </div>

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

        {/* Diff mode toggle */}
        <div class="flex gap-0.5 bg-(--color-bg-tertiary) rounded p-0.5 shrink-0">
          <button
            class={`px-2 py-0.5 text-xs rounded cursor-pointer transition-colors ${
              prDetailStore.diffMode === "split"
                ? "bg-(--color-bg-secondary) text-(--color-text-primary) shadow-sm"
                : "text-(--color-text-secondary) hover:text-(--color-text-primary)"
            }`}
            onClick={() => prDetailStore.setDiffMode("split")}
          >
            Split
          </button>
          <button
            class={`px-2 py-0.5 text-xs rounded cursor-pointer transition-colors ${
              prDetailStore.diffMode === "unified"
                ? "bg-(--color-bg-secondary) text-(--color-text-primary) shadow-sm"
                : "text-(--color-text-secondary) hover:text-(--color-text-primary)"
            }`}
            onClick={() => prDetailStore.setDiffMode("unified")}
          >
            Unified
          </button>
        </div>
      </div>

      {/* Code search bar */}
      <Show when={searchStore.codeSearchActive}>
        <CodeSearchBar
          query={searchStore.codeSearchQuery}
          onQueryChange={(q) => searchStore.setCodeSearchQuery(q)}
          totalMatches={searchStore.totalMatchCount}
          activeIndex={searchStore.activeMatchIndex}
          onNext={() => searchStore.nextMatch()}
          onPrev={() => searchStore.prevMatch()}
          onClose={() => searchStore.closeCodeSearch()}
        />
      </Show>

      {/* Diff content */}
      <div class="flex-1 overflow-auto">
        <Show
          when={props.file.diff}
          fallback={
            <div class="flex items-center justify-center h-full text-sm text-(--color-text-muted)">
              No diff available
            </div>
          }
        >
          {(diff) => (
            <Switch>
              <Match when={diff().is_binary}>
                <div class="flex items-center justify-center h-full text-sm text-(--color-text-muted)">
                  Binary file — not shown
                </div>
              </Match>
              <Match when={diff().is_too_large}>
                <div class="flex items-center justify-center h-full text-sm text-(--color-text-muted)">
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
    </div>
  );
}
