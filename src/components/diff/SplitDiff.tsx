import { For, Show, createMemo } from "solid-js";
import type { SideBySideRow, WordDiffPair } from "~/api/files";
import type { ReviewComment, ThreadResolutionInfo } from "~/types/pr";
import type { CommentActions, NewCommentTarget } from "./DiffViewer";
import type { SearchRange } from "~/lib/search";
import { buildThreads, threadsByLine } from "~/lib/comments";
import { CommentThreadView } from "~/components/comments/CommentThread";
import { CommentForm } from "~/components/comments/CommentForm";
import { DiffLineContent } from "./DiffLineContent";

interface SplitDiffProps {
  rows: SideBySideRow[];
  wordDiffs: WordDiffPair[];
  comments: ReviewComment[];
  commentActions: CommentActions;
  currentUser: string;
  newCommentTarget: NewCommentTarget | null;
  /** Map<"old:N" | "new:N", SearchRange[]> for the current file */
  searchMatchesByKey?: Map<string, SearchRange[]>;
  threadResolutions?: Map<number, ThreadResolutionInfo>;
}

export function SplitDiff(props: SplitDiffProps) {
  // Index word diffs by line numbers for O(1) lookup
  const wordDiffMap = createMemo(() => {
    const map = new Map<string, WordDiffPair>();
    for (const wd of props.wordDiffs) {
      if (wd.old_lineno != null && wd.new_lineno != null) {
        map.set(`${wd.old_lineno}:${wd.new_lineno}`, wd);
      }
    }
    return map;
  });

  const getWordDiff = (row: SideBySideRow): WordDiffPair | undefined => {
    if (!row.left?.old_lineno || !row.right?.new_lineno) return undefined;
    return wordDiffMap().get(`${row.left.old_lineno}:${row.right.new_lineno}`);
  };

  // Comment threading
  const threads = createMemo(() => buildThreads(props.comments, props.threadResolutions));
  const threadMap = createMemo(() => threadsByLine(threads()));

  const getThreadsForRow = (row: SideBySideRow) => {
    const leftThreads = row.left?.old_lineno
      ? (threadMap().get(`${row.left.old_lineno}:LEFT`) ?? [])
      : [];
    const rightThreads = row.right?.new_lineno
      ? (threadMap().get(`${row.right.new_lineno}:RIGHT`) ?? [])
      : [];
    const all = [...leftThreads, ...rightThreads];
    return all.length > 0 ? all : null;
  };

  const isNewCommentRow = (row: SideBySideRow) => {
    const target = props.newCommentTarget;
    if (!target) return false;
    if (target.side === "LEFT" && row.left?.old_lineno === target.line) return true;
    if (target.side === "RIGHT" && row.right?.new_lineno === target.line) return true;
    return false;
  };

  return (
    <table class="w-full border-collapse font-mono text-xs leading-5" style={{ "table-layout": "fixed" }}>
      <colgroup>
        <col style={{ width: "50px" }} />
        <col style={{ width: "calc(50% - 50px)" }} />
        <col style={{ width: "50px" }} />
        <col style={{ width: "calc(50% - 50px)" }} />
      </colgroup>
      <tbody>
        <For each={props.rows}>
          {(row) => (
            <Show
              when={!row.is_hunk_header}
              fallback={
                <tr>
                  <td colspan={4} class="px-3 py-1 text-xs text-(--color-text-muted) bg-(--color-diff-hunk-bg) border-y border-(--color-border-muted)">
                    {row.hunk_header}
                  </td>
                </tr>
              }
            >
              {/* Diff line row */}
              <tr class="group">
                {/* Left side (old) */}
                <td class={`text-right px-2 select-none relative ${lineGutterClass(row.left?.kind)}`}>
                  <span class="text-(--color-diff-gutter)">{row.left?.old_lineno ?? ""}</span>
                  <Show when={row.left?.old_lineno}>
                    <button
                      class="absolute left-0.5 top-1/2 -translate-y-1/2 w-4 h-4 rounded bg-(--color-accent-emphasis) text-white text-[10px] leading-none flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity cursor-pointer z-10"
                      onClick={() => props.commentActions.onAddComment(row.left!.old_lineno!, "LEFT")}
                    >
                      +
                    </button>
                  </Show>
                </td>
                <td class={`px-3 whitespace-pre-wrap break-all ${lineBgClass(row.left?.kind)}`}>
                  <Show when={row.left} fallback={<span>&nbsp;</span>}>
                    {(line) => {
                      const wd = getWordDiff(row);
                      const useWordDiff = () => wd && line().kind === "removed";
                      const searchRanges = () => {
                        if (!props.searchMatchesByKey || !line().old_lineno) return undefined;
                        return props.searchMatchesByKey.get(`old:${line().old_lineno}`);
                      };
                      return (
                        <DiffLineContent
                          content={line().content || " "}
                          wordDiffSpans={useWordDiff() ? wd!.old_spans : undefined}
                          searchRanges={searchRanges()}
                        />
                      );
                    }}
                  </Show>
                </td>

                {/* Right side (new) */}
                <td class={`text-right px-2 select-none relative ${lineGutterClass(row.right?.kind)}`}>
                  <span class="text-(--color-diff-gutter)">{row.right?.new_lineno ?? ""}</span>
                  <Show when={row.right?.new_lineno}>
                    <button
                      class="absolute left-0.5 top-1/2 -translate-y-1/2 w-4 h-4 rounded bg-(--color-accent-emphasis) text-white text-[10px] leading-none flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity cursor-pointer z-10"
                      onClick={() => props.commentActions.onAddComment(row.right!.new_lineno!, "RIGHT")}
                    >
                      +
                    </button>
                  </Show>
                </td>
                <td class={`px-3 whitespace-pre-wrap break-all ${lineBgClass(row.right?.kind)}`}>
                  <Show when={row.right} fallback={<span>&nbsp;</span>}>
                    {(line) => {
                      const wd = getWordDiff(row);
                      const useWordDiff = () => wd && line().kind === "added";
                      const searchRanges = () => {
                        if (!props.searchMatchesByKey || !line().new_lineno) return undefined;
                        return props.searchMatchesByKey.get(`new:${line().new_lineno}`);
                      };
                      return (
                        <DiffLineContent
                          content={line().content || " "}
                          wordDiffSpans={useWordDiff() ? wd!.new_spans : undefined}
                          searchRanges={searchRanges()}
                        />
                      );
                    }}
                  </Show>
                </td>
              </tr>

              {/* Existing comment threads for this row */}
              <Show when={getThreadsForRow(row)}>
                {(rowThreads) => (
                  <tr>
                    <td colspan={4} class="p-0">
                      <div class="mx-4 my-2 flex flex-col gap-2 min-w-0 overflow-hidden">
                        <For each={rowThreads()}>
                          {(thread) => (
                            <div data-comment-thread={thread.root.id}>
                              <CommentThreadView
                                thread={thread}
                                currentUser={props.currentUser}
                                onReply={props.commentActions.onReply}
                                onUpdate={props.commentActions.onUpdate}
                                onDelete={props.commentActions.onDelete}
                                onResolve={props.commentActions.onResolve}
                                onUnresolve={props.commentActions.onUnresolve}
                              />
                            </div>
                          )}
                        </For>
                      </div>
                    </td>
                  </tr>
                )}
              </Show>

              {/* New comment form for this row */}
              <Show when={isNewCommentRow(row)}>
                <tr>
                  <td colspan={4} class="p-0">
                    <div class="mx-4 my-2 min-w-0 overflow-hidden">
                      <div class="rounded border border-(--color-border-default) bg-(--color-bg-secondary) p-3">
                        <CommentForm
                          onSubmit={props.commentActions.onSubmitNewComment}
                          onCancel={props.commentActions.onCancelNewComment}
                          placeholder="Write a comment..."
                        />
                      </div>
                    </div>
                  </td>
                </tr>
              </Show>
            </Show>
          )}
        </For>
      </tbody>
    </table>
  );
}

function lineBgClass(kind?: string): string {
  switch (kind) {
    case "added": return "bg-(--color-diff-add-line)";
    case "removed": return "bg-(--color-diff-del-line)";
    default: return "";
  }
}

function lineGutterClass(kind?: string): string {
  switch (kind) {
    case "added": return "bg-(--color-diff-add-bg)";
    case "removed": return "bg-(--color-diff-del-bg)";
    default: return "";
  }
}
