import { createSignal, createMemo, For, Show } from "solid-js";
import { prDetailStore } from "~/stores";
import { buildThreads } from "~/lib/comments";
import { ConversationItem } from "./ConversationItem";

type ResolutionFilter = "all" | "unresolved" | "resolved";

interface ConversationListProps {
  onSelectThread: (filePath: string, threadId: number) => void;
  activeThreadId: number | null;
}

export function ConversationList(props: ConversationListProps) {
  const [filterQuery, setFilterQuery] = createSignal("");
  const [resolutionFilter, setResolutionFilter] = createSignal<ResolutionFilter>("all");

  const allThreads = createMemo(() => {
    const threads = buildThreads(prDetailStore.comments, prDetailStore.threadResolutions);
    return threads.sort((a, b) => {
      const aLatest =
        a.replies.length > 0
          ? a.replies[a.replies.length - 1].created_at
          : a.root.created_at;
      const bLatest =
        b.replies.length > 0
          ? b.replies[b.replies.length - 1].created_at
          : b.root.created_at;
      return bLatest.localeCompare(aLatest);
    });
  });

  const filteredThreads = createMemo(() => {
    let threads = allThreads();

    // Resolution filter
    const rf = resolutionFilter();
    if (rf === "resolved") threads = threads.filter((t) => t.isResolved);
    if (rf === "unresolved") threads = threads.filter((t) => !t.isResolved);

    // Text filter
    const q = filterQuery().toLowerCase();
    if (!q) return threads;
    return threads.filter((thread) => {
      const all = [thread.root, ...thread.replies];
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
        </span>
      </div>

      {/* Thread list */}
      <div class="flex-1 overflow-auto">
        <Show
          when={filteredThreads().length > 0}
          fallback={
            <Show when={filterQuery() || resolutionFilter() !== "all"}>
              <div class="px-3 py-4 text-xs text-(--color-text-muted) text-center">
                No conversations match your filters
              </div>
            </Show>
          }
        >
          <For each={filteredThreads()}>
            {(thread) => (
              <ConversationItem
                thread={thread}
                isActive={props.activeThreadId === thread.root.id}
                onClick={() =>
                  props.onSelectThread(thread.root.path, thread.root.id)
                }
              />
            )}
          </For>
        </Show>
      </div>
    </div>
  );
}
