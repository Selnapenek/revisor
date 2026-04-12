import { For, Show, createMemo } from "solid-js";
import type { DiffHunk, SideBySideRow, WordDiffPair } from "~/api/files";
import type { ReviewComment, ThreadResolutionInfo } from "~/types/pr";
import type { CommentActions, NewCommentTarget } from "./DiffViewer";
import type { SearchRange } from "~/lib/search";
import type { ExpandableGap } from "~/lib/expand-context";
import type { ExpandCallbacks } from "./UnifiedDiff";
import { buildThreads, threadsByLine } from "~/lib/comments";
import { CommentThreadView } from "~/components/comments/CommentThread";
import { CommentForm } from "~/components/comments/CommentForm";
import { DiffLineContent } from "./DiffLineContent";
import { ExpandContextRow } from "./ExpandContextRow";

interface SplitDiffProps {
  rows: SideBySideRow[];
  hunks: DiffHunk[];
  wordDiffs: WordDiffPair[];
  comments: ReviewComment[];
  commentActions: CommentActions;
  currentUser: string;
  newCommentTarget: NewCommentTarget | null;
  /** Map<"old:N" | "new:N", SearchRange[]> for the current file */
  searchMatchesByKey?: Map<string, SearchRange[]>;
  threadResolutions?: Map<number, ThreadResolutionInfo>;
  gaps?: ExpandableGap[];
  expandCallbacks?: ExpandCallbacks;
  expandLoading?: boolean;
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

  // Map hunk headers in rows to their hunk index for gap matching
  const hunkHeaderIndices = createMemo(() => {
    const map = new Map<number, number>(); // rowIndex -> hunkIndex
    let hunkIdx = 0;
    for (let i = 0; i < props.rows.length; i++) {
      if (props.rows[i].is_hunk_header) {
        map.set(i, hunkIdx);
        hunkIdx++;
      }
    }
    return map;
  });

  // Index gaps by hunkIndex for quick lookup
  const gapsByHunk = createMemo(() => {
    const map = new Map<string, ExpandableGap>();
    for (const gap of props.gaps ?? []) {
      map.set(`${gap.position}:${gap.hunkIndex}`, gap);
    }
    return map;
  });

  const getGap = (position: string, hunkIndex: number) =>
    gapsByHunk().get(`${position}:${hunkIndex}`);

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
          {(row, rowIdx) => {
            const hunkIdx = () => hunkHeaderIndices().get(rowIdx());

            return (
            <>
            <Show
              when={!row.is_hunk_header}
              fallback={(() => {
                const hi = hunkIdx()!;
                const beforeGap = hi === 0 ? getGap("before", 0) : undefined;
                const betweenGap = hi > 0 ? getGap("between", hi) : undefined;

                return (
                  <>
                    {/* Expand row before first hunk */}
                    <Show when={hi === 0 && beforeGap}>
                      {(gap) => (
                        <ExpandContextRow
                          colspan={4}
                          lineCount={gap().lineCount}
                          loading={props.expandLoading}
                          onExpandDown={props.expandCallbacks ? () => props.expandCallbacks!.onExpandUp(0) : undefined}
                          onExpandAll={props.expandCallbacks ? () => props.expandCallbacks!.onExpandAll(gap()) : undefined}
                        />
                      )}
                    </Show>

                    {/* Expand row between hunks (replaces hunk header) */}
                    <Show
                      when={betweenGap}
                      fallback={
                        <tr>
                          <td colspan={4} class="px-3 py-1 text-xs text-(--color-text-muted) bg-(--color-diff-hunk-bg) border-y border-(--color-border-muted)">
                            {row.hunk_header}
                          </td>
                        </tr>
                      }
                    >
                      {(gap) => (
                        <ExpandContextRow
                          colspan={4}
                          lineCount={gap().lineCount}
                          loading={props.expandLoading}
                          onExpandUp={props.expandCallbacks ? () => props.expandCallbacks!.onExpandUp(hi) : undefined}
                          onExpandDown={props.expandCallbacks ? () => props.expandCallbacks!.onExpandDown(hi - 1) : undefined}
                          onExpandAll={props.expandCallbacks ? () => props.expandCallbacks!.onExpandAll(gap()) : undefined}
                        />
                      )}
                    </Show>
                  </>
                );
              })()}
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

              {/* Expand row after last hunk */}
              <Show when={rowIdx() === props.rows.length - 1 && getGap("after", props.hunks.length - 1)}>
                {(gap) => (
                  <ExpandContextRow
                    colspan={4}
                    lineCount={gap().lineCount}
                    loading={props.expandLoading}
                    onExpandUp={props.expandCallbacks ? () => props.expandCallbacks!.onExpandDown(props.hunks.length - 1) : undefined}
                    onExpandAll={props.expandCallbacks ? () => props.expandCallbacks!.onExpandAll(gap()) : undefined}
                  />
                )}
              </Show>
            </>
            );
          }}
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
