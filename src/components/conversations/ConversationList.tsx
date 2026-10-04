import { createSignal, createMemo, For, Show, Switch, Match } from "solid-js";
import { prDetailStore } from "~/stores";
import { buildThreads } from "~/lib/comments";
import type { CommentThread } from "~/lib/comments";
import type { IssueComment, PrReview } from "~/types/pr";
import { ConversationItem } from "./ConversationItem";
import { IssueCommentItem } from "./IssueCommentItem";
import { ReviewItem } from "./ReviewItem";

type ResolutionFilter = "all" | "unresolved" | "resolved";

type TimelineEntry =
  | { kind: "thread"; thread: CommentThread; timestamp: string }
  | { kind: "issue_comment"; comment: IssueComment; timestamp: string }
  | { kind: "review"; review: PrReview; timestamp: string };

interface ConversationListProps {
  onSelectThread: (filePath: string, threadId: number) => void;
  activeThreadId: number | null;
  onValidateThread?: (thread: CommentThread) => void;
}

export function ConversationList(props: ConversationListProps) {
  const [filterQuery, setFilterQuery] = createSignal("");
  const [resolutionFilter, setResolutionFilter] = createSignal<ResolutionFilter>("all");

  const allThreads = createMemo(() =>
    buildThreads(prDetailStore.comments, prDetailStore.threadResolutions)
  );

  const timeline = createMemo(() => {
    const entries: TimelineEntry[] = [];

    for (const thread of allThreads()) {
      const latestTs =
        thread.replies.length > 0
          ? thread.replies[thread.replies.length - 1].created_at
          : thread.root.created_at;
      entries.push({ kind: "thread", thread, timestamp: latestTs });
    }

    for (const comment of prDetailStore.issueComments) {
      entries.push({ kind: "issue_comment", comment, timestamp: comment.created_at });
    }

    for (const review of prDetailStore.reviews) {
      // Only show reviews that have a body (skip empty approvals etc.)
      if (!review.body.trim()) continue;
      entries.push({ kind: "review", review, timestamp: review.submitted_at ?? "" });
    }

    entries.sort((a, b) => b.timestamp.localeCompare(a.timestamp));
    return entries;
  });

  const filteredTimeline = createMemo(() => {
    let entries = timeline();

    // Resolution filter — only affects threads; non-thread entries always pass
    const rf = resolutionFilter();
    if (rf === "resolved") {
      entries = entries.filter((e) => e.kind !== "thread" || e.thread.isResolved);
    }
    if (rf === "unresolved") {
      entries = entries.filter((e) => e.kind !== "thread" || !e.thread.isResolved);
    }

    // Text filter — applies to all types
    const q = filterQuery().toLowerCase();
    if (!q) return entries;
    return entries.filter((entry) => {
      if (entry.kind === "issue_comment") {
        return (
          entry.comment.author_login.toLowerCase().includes(q) ||
          entry.comment.body.toLowerCase().includes(q)
        );
      }
      if (entry.kind === "review") {
        return (
          entry.review.author_login.toLowerCase().includes(q) ||
          entry.review.body.toLowerCase().includes(q)
        );
      }
      const all = [entry.thread.root, ...entry.thread.replies];
      return all.some(
        (c) =>
          c.author_login.toLowerCase().includes(q) ||
          c.body.toLowerCase().includes(q)
      );
    });
  });

  const resolvedCount = createMemo(() => allThreads().filter((t) => t.isResolved).length);
  const unresolvedCount = createMemo(() => allThreads().length - resolvedCount());

  return (
    <div class="h-full flex flex-col">
      {/* Filter input */}
      <div class="px-3 py-2 border-b border-(--color-border-muted) shrink-0">
        <div class="flex items-center gap-1.5 px-2 py-1 rounded bg-(--color-bg-tertiary) border border-(--color-border-muted) focus-within:border-(--color-accent-emphasis) transition-colors">
          <svg
            class="w-3 h-3 text-(--color-text-muted) shrink-0"
            viewBox="0 0 16 16"
            fill="currentColor"
          >
            <path d="M11.5 7a4.5 4.5 0 1 1-9 0 4.5 4.5 0 0 1 9 0Zm-.82 4.74a6 6 0 1 1 1.06-1.06l3.04 3.04a.75.75 0 1 1-1.06 1.06l-3.04-3.04Z" />
          </svg>
          <input
            type="text"
            class="flex-1 bg-transparent text-xs text-(--color-text-primary) placeholder:text-(--color-text-muted) outline-none min-w-0"
            placeholder="Filter conversations..."
            value={filterQuery()}
            onInput={(e) => setFilterQuery(e.currentTarget.value)}
          />
          <Show when={filterQuery()}>
            <button
              class="w-3.5 h-3.5 flex items-center justify-center rounded text-(--color-text-muted) hover:text-(--color-text-primary) cursor-pointer"
              onClick={() => setFilterQuery("")}
            >
              <svg class="w-3 h-3" viewBox="0 0 16 16" fill="currentColor">
                <path d="M3.72 3.72a.75.75 0 0 1 1.06 0L8 6.94l3.22-3.22a.749.749 0 0 1 1.275.326.749.749 0 0 1-.215.734L9.06 8l3.22 3.22a.749.749 0 0 1-.326 1.275.749.749 0 0 1-.734-.215L8 9.06l-3.22 3.22a.751.751 0 0 1-1.042-.018.751.751 0 0 1-.018-1.042L6.94 8 3.72 4.78a.75.75 0 0 1 0-1.06Z" />
              </svg>
            </button>
          </Show>
        </div>
      </div>

      {/* Resolution filter toggle */}
      <div class="px-3 py-1.5 border-b border-(--color-border-muted) shrink-0">
        <div class="flex gap-0.5 bg-(--color-bg-tertiary) rounded p-0.5">
          {(["all", "unresolved", "resolved"] as const).map((value) => (
            <button
              class={`flex-1 px-2 py-0.5 text-[11px] rounded cursor-pointer transition-colors ${
                resolutionFilter() === value
                  ? "bg-(--color-bg-secondary) text-(--color-text-primary) shadow-sm"
                  : "text-(--color-text-secondary) hover:text-(--color-text-primary)"
              }`}
              onClick={() => setResolutionFilter(value)}
            >
              {value === "all" ? "All" : value === "unresolved" ? "Open" : "Resolved"}
            </button>
          ))}
        </div>
      </div>

      {/* Count header */}
      <div class="px-3 py-2 border-b border-(--color-border-muted) shrink-0">
        <span class="text-xs text-(--color-text-secondary)">
          {unresolvedCount()} open
          <Show when={resolvedCount() > 0}>
            <span class="text-(--color-text-muted)"> · {resolvedCount()} resolved</span>
          </Show>
          <Show when={prDetailStore.issueComments.length > 0}>
            <span class="text-(--color-text-muted)"> · {prDetailStore.issueComments.length} comment{prDetailStore.issueComments.length !== 1 ? "s" : ""}</span>
          </Show>
        </span>
      </div>

      {/* Timeline list */}
      <div class="flex-1 overflow-auto">
        <Show
          when={filteredTimeline().length > 0}
          fallback={
            <Show when={filterQuery() || resolutionFilter() !== "all"}>
              <div class="px-3 py-4 text-xs text-(--color-text-muted) text-center">
                No conversations match your filters
              </div>
            </Show>
          }
        >
          <For each={filteredTimeline()}>
            {(entry) => (
              <Switch>
                <Match when={entry.kind === "thread" && entry}>
                  {(e) => {
                    const threadEntry = e() as TimelineEntry & { kind: "thread" };
                    return (
                      <ConversationItem
                        thread={threadEntry.thread}
                        onValidate={props.onValidateThread ? () => props.onValidateThread?.(threadEntry.thread) : undefined}
                        isActive={props.activeThreadId === threadEntry.thread.root.id}
                        onClick={() =>
                          props.onSelectThread(threadEntry.thread.root.path, threadEntry.thread.root.id)
                        }
                      />
                    );
                  }}
                </Match>
                <Match when={entry.kind === "issue_comment" && entry}>
                  {(e) => {
                    const commentEntry = e() as TimelineEntry & { kind: "issue_comment" };
                    return <IssueCommentItem comment={commentEntry.comment} />;
                  }}
                </Match>
                <Match when={entry.kind === "review" && entry}>
                  {(e) => {
                    const reviewEntry = e() as TimelineEntry & { kind: "review" };
                    return <ReviewItem review={reviewEntry.review} />;
                  }}
                </Match>
              </Switch>
            )}
          </For>
        </Show>
      </div>
    </div>
  );
}
