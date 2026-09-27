import { For, Show, createMemo } from "solid-js";
import type { DiffHunk, DiffLine, WordDiffPair } from "~/api/files";
import type { ReviewComment, ThreadResolutionInfo } from "~/types/pr";
import type { CommentActions, NewCommentTarget } from "./DiffViewer";
import type { SearchRange } from "~/lib/search";
import type { SemanticAnchor } from "~/lib/semantic-search";
import type { ExpandableGap } from "~/lib/expand-context";
import { buildThreads, threadsByLine } from "~/lib/comments";
import { CommentThreadView } from "~/components/comments/CommentThread";
import { CommentForm } from "~/components/comments/CommentForm";
import { DiffLineContent } from "./DiffLineContent";
import { ExpandContextRow } from "./ExpandContextRow";

export interface ExpandCallbacks {
  onExpandUp: (hunkIndex: number) => void;
  onExpandDown: (hunkIndex: number) => void;
  onExpandAll: (gap: ExpandableGap) => void;
}

interface UnifiedDiffProps {
  hunks: DiffHunk[];
  wordDiffs: WordDiffPair[];
  comments: ReviewComment[];
  commentActions: CommentActions;
  currentUser: string;
  newCommentTarget: NewCommentTarget | null;
  /** Map<"lineno:side", SearchRange[]> for the current file */
  searchMatchesByKey?: Map<string, SearchRange[]>;
  semanticAnchor?: SemanticAnchor | null;
  threadResolutions?: Map<number, ThreadResolutionInfo>;
  gaps?: ExpandableGap[];
  expandCallbacks?: ExpandCallbacks;
  expandLoading?: boolean;
}

export function UnifiedDiff(props: UnifiedDiffProps) {
  // Index word diffs for O(1) lookup
  const wordDiffByOld = createMemo(() => {
    const map = new Map<number, WordDiffPair>();
    for (const wd of props.wordDiffs) {
      if (wd.old_lineno != null) map.set(wd.old_lineno, wd);
    }
    return map;
  });

  const wordDiffByNew = createMemo(() => {
    const map = new Map<number, WordDiffPair>();
    for (const wd of props.wordDiffs) {
      if (wd.new_lineno != null) map.set(wd.new_lineno, wd);
    }
    return map;
  });

  // Comment threading
  const threads = createMemo(() => buildThreads(props.comments, props.threadResolutions));
  const threadMap = createMemo(() => threadsByLine(threads()));

  const getThreadsForLine = (line: DiffLine) => {
    const lineNo = line.kind === "removed" ? line.old_lineno : line.new_lineno;
    const side = line.kind === "removed" ? "LEFT" : "RIGHT";
    if (lineNo == null) return null;
    const t = threadMap().get(`${lineNo}:${side}`);
    return t && t.length > 0 ? t : null;
  };

  const isNewCommentLine = (line: DiffLine) => {
    const target = props.newCommentTarget;
    if (!target) return false;
    if (target.side === "LEFT" && line.kind === "removed" && line.old_lineno === target.line) return true;
    if (target.side === "RIGHT" && line.kind !== "removed" && line.new_lineno === target.line) return true;
    return false;
  };

  const getGutterLineNo = (line: DiffLine): { lineNo: number; side: "LEFT" | "RIGHT" } | null => {
    if (line.kind === "removed" && line.old_lineno != null) {
      return { lineNo: line.old_lineno, side: "LEFT" };
    }
    if (line.new_lineno != null) {
      return { lineNo: line.new_lineno, side: "RIGHT" };
    }
    return null;
  };

  const getSearchRanges = (line: DiffLine): SearchRange[] | undefined => {
    if (!props.searchMatchesByKey) return undefined;
    const lineno = line.kind === "removed" ? line.old_lineno : line.new_lineno;
    if (lineno == null) return undefined;
    const side = line.kind === "removed" ? "left" : line.kind === "added" ? "right" : "context";
    return props.searchMatchesByKey.get(`${lineno}:${side}`);
  };

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
        <col style={{ width: "50px" }} />
        <col />
      </colgroup>
      <tbody>
        <For each={props.hunks}>
          {(hunk, hunkIdx) => {
            const beforeGap = () => getGap("before", hunkIdx());
            const betweenGap = () => getGap("between", hunkIdx());
            const afterGap = () => hunkIdx() === props.hunks.length - 1 ? getGap("after", hunkIdx()) : undefined;

            return (
            <>
              {/* Expand row before first hunk */}
              <Show when={hunkIdx() === 0 && beforeGap()}>
                {(gap) => (
                  <ExpandContextRow
                    colspan={3}
                    lineCount={gap().lineCount}
                    loading={props.expandLoading}
                    onExpandDown={props.expandCallbacks ? () => props.expandCallbacks!.onExpandUp(0) : undefined}
                    onExpandAll={props.expandCallbacks ? () => props.expandCallbacks!.onExpandAll(gap()) : undefined}
                  />
                )}
              </Show>

              {/* Expand row between hunks (replaces hunk header) */}
              <Show when={hunkIdx() > 0 && betweenGap()}>
                {(gap) => (
                  <ExpandContextRow
                    colspan={3}
                    lineCount={gap().lineCount}
                    loading={props.expandLoading}
                    onExpandUp={props.expandCallbacks ? () => props.expandCallbacks!.onExpandUp(hunkIdx()) : undefined}
                    onExpandDown={props.expandCallbacks ? () => props.expandCallbacks!.onExpandDown(hunkIdx() - 1) : undefined}
                    onExpandAll={props.expandCallbacks ? () => props.expandCallbacks!.onExpandAll(gap()) : undefined}
                  />
                )}
              </Show>

              {/* Hunk header — hide when there's a between-gap expand row replacing it */}
              <Show when={!(hunkIdx() > 0 && betweenGap())}>
                <tr>
                  <td colspan={3} class="px-3 py-1 text-xs text-(--color-text-muted) bg-(--color-diff-hunk-bg) border-y border-(--color-border-muted)">
                    {hunk.header}
                  </td>
                </tr>
              </Show>

              {/* Lines */}
              <For each={hunk.lines}>
                {(line) => {
                  const prefix = () => {
                    switch (line.kind) {
                      case "added": return "+";
                      case "removed": return "-";
                      default: return " ";
                    }
                  };

                  const bgClass = () => {
                    switch (line.kind) {
                      case "added": return "bg-(--color-diff-add-line)";
                      case "removed": return "bg-(--color-diff-del-line)";
                      default: return "";
                    }
                  };

                  const gutterClass = () => {
                    switch (line.kind) {
                      case "added": return "bg-(--color-diff-add-bg)";
                      case "removed": return "bg-(--color-diff-del-bg)";
                      default: return "";
                    }
                  };

                  const prefixColor = () => {
                    switch (line.kind) {
                      case "added": return "text-(--color-success-fg)";
                      case "removed": return "text-(--color-danger-fg)";
                      default: return "text-transparent";
                    }
                  };

                  // Get word diff for this line
                  const wd = () => {
                    if (line.kind === "removed" && line.old_lineno != null) {
                      return wordDiffByOld().get(line.old_lineno);
                    }
                    if (line.kind === "added" && line.new_lineno != null) {
                      return wordDiffByNew().get(line.new_lineno);
                    }
                    return undefined;
                  };

                  const spans = () => {
                    const w = wd();
                    if (!w) return undefined;
                    return line.kind === "removed" ? w.old_spans : w.new_spans;
                  };

                  const gutterInfo = getGutterLineNo(line);
                  const searchRanges = () => getSearchRanges(line);

                  return (
                    <>
                      <tr data-old-line={line.old_lineno ?? undefined} data-new-line={line.new_lineno ?? undefined}
                        class={`group ${props.semanticAnchor && ((props.semanticAnchor.side === "old" && line.old_lineno === props.semanticAnchor.line) || (props.semanticAnchor.side === "new" && line.new_lineno === props.semanticAnchor.line)) ? "ring-1 ring-inset ring-(--color-accent-emphasis)" : ""}`}>
                        <td class={`text-right px-2 select-none relative ${gutterClass()}`}>
                          <span class="text-(--color-diff-gutter)">{line.old_lineno ?? ""}</span>
                          <Show when={gutterInfo}>
                            <button
                              class="absolute left-0.5 top-1/2 -translate-y-1/2 w-4 h-4 rounded bg-(--color-accent-emphasis) text-white text-[10px] leading-none flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity cursor-pointer z-10"
                              onClick={() => props.commentActions.onAddComment(gutterInfo!.lineNo, gutterInfo!.side)}
                            >
                              +
                            </button>
                          </Show>
                        </td>
                        <td class={`text-right px-2 select-none ${gutterClass()}`}>
                          <span class="text-(--color-diff-gutter)">{line.new_lineno ?? ""}</span>
                        </td>
                        <td class={`whitespace-pre-wrap break-all ${bgClass()}`}>
                          <span class={`select-none ${prefixColor()}`}>{prefix()}</span>
                          <span class="pl-1">
                            <DiffLineContent
                              content={line.content || " "}
                              wordDiffSpans={spans()}
                              searchRanges={searchRanges()}
                            />
                          </span>
                        </td>
                      </tr>

                      {/* Comment threads */}
                      <Show when={getThreadsForLine(line)}>
                        {(lineThreads) => (
                          <tr>
                            <td colspan={3} class="p-0">
                              <div class="mx-4 my-2 flex flex-col gap-2 min-w-0 overflow-hidden">
                                <For each={lineThreads()}>
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

                      {/* New comment form */}
                      <Show when={isNewCommentLine(line)}>
                        <tr>
                          <td colspan={3} class="p-0">
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
                    </>
                  );
                }}
              </For>

              {/* Expand row after last hunk */}
              <Show when={afterGap()}>
                {(gap) => (
                  <ExpandContextRow
                    colspan={3}
                    lineCount={gap().lineCount}
                    loading={props.expandLoading}
                    onExpandUp={props.expandCallbacks ? () => props.expandCallbacks!.onExpandDown(hunkIdx()) : undefined}
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
