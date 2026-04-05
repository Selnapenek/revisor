import { For, Show, createSignal } from "solid-js";
import { Popover } from "~/components/ui/Popover";
import { relativeTime } from "~/lib/time";
import type { Commit } from "~/types/pr";
import type { CommitSelection } from "~/stores/pr-detail.store";

interface CommitSelectorProps {
  commits: Commit[];
  selection: CommitSelection;
  label: string;
  onSelect: (selection: CommitSelection) => void;
}

export function CommitSelector(props: CommitSelectorProps) {
  const [lastClickedIndex, setLastClickedIndex] = createSignal<number | null>(null);

  const isSelected = (index: number): boolean => {
    const sel = props.selection;
    if (sel.type === "all") return false;
    if (sel.type === "single") return sel.index === index;
    return index >= sel.from && index <= sel.to;
  };

  const handleClick = (index: number, e: MouseEvent) => {
    const last = lastClickedIndex();
    if (e.shiftKey && last !== null && last !== index) {
      const from = Math.min(last, index);
      const to = Math.max(last, index);
      props.onSelect({ type: "range", from, to });
    } else {
      // If already the only selected commit, deselect (go back to all)
      const sel = props.selection;
      if (sel.type === "single" && sel.index === index) {
        props.onSelect({ type: "all" });
        setLastClickedIndex(null);
        return;
      }
      props.onSelect({ type: "single", index });
      setLastClickedIndex(index);
    }
  };

  const handleSelectAll = () => {
    props.onSelect({ type: "all" });
    setLastClickedIndex(null);
  };

  return (
    <Popover
      trigger={({ ref, onClick, isOpen }) => (
        <button
          ref={ref}
          onClick={onClick}
          class={`flex items-center gap-1.5 w-full px-3 py-1.5 text-xs font-medium cursor-pointer transition-colors border-b border-(--color-border-muted) hover:bg-(--color-bg-tertiary) ${
            isOpen ? "bg-(--color-bg-tertiary)" : ""
          }`}
        >
          <svg class="w-3.5 h-3.5 shrink-0 text-(--color-text-muted)" viewBox="0 0 16 16" fill="currentColor">
            <path fill-rule="evenodd" d="M10.5 7.75a2.5 2.5 0 11-5 0 2.5 2.5 0 015 0zm1.43.75a4.002 4.002 0 01-7.86 0H.75a.75.75 0 110-1.5h3.32a4.001 4.001 0 017.86 0h3.32a.75.75 0 110 1.5h-3.32z" />
          </svg>
          <span class="truncate text-(--color-text-primary)">{props.label}</span>
          <svg class="w-3 h-3 ml-auto shrink-0 opacity-50" viewBox="0 0 12 12" fill="none">
            <path d="M3 5l3 3 3-3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        </button>
      )}
    >
      <div class="flex flex-col max-h-[400px]">
        {/* All commits option */}
        <button
          class={`flex items-center gap-2 px-3 py-2 text-xs text-left cursor-pointer transition-colors border-b border-(--color-border-muted) ${
            props.selection.type === "all"
              ? "bg-(--color-accent-emphasis)/10 text-(--color-accent-fg)"
              : "hover:bg-(--color-bg-tertiary) text-(--color-text-primary)"
          }`}
          onClick={handleSelectAll}
        >
          <span class="font-medium">All commits</span>
          <span class="text-(--color-text-muted) ml-auto">{props.commits.length}</span>
        </button>

        {/* Shift-click hint */}
        <div class="px-3 py-1 text-[10px] text-(--color-text-muted) border-b border-(--color-border-muted)">
          Click to select · Shift+click for range
        </div>

        {/* Commit list */}
        <div class="overflow-y-auto">
          <For each={props.commits}>
            {(commit, index) => (
              <button
                class={`flex items-start gap-2 w-full px-3 py-1.5 text-left cursor-pointer transition-colors ${
                  isSelected(index())
                    ? "bg-(--color-accent-emphasis)/10"
                    : "hover:bg-(--color-bg-tertiary)"
                }`}
                onClick={(e) => handleClick(index(), e)}
              >
                {/* Selection indicator */}
                <span
                  class={`mt-0.5 flex items-center justify-center w-3.5 h-3.5 rounded-full border shrink-0 ${
                    isSelected(index())
                      ? "bg-(--color-accent-emphasis) border-(--color-accent-emphasis)"
                      : "border-(--color-border-default)"
                  }`}
                >
                  <Show when={isSelected(index())}>
                    <svg class="w-2.5 h-2.5 text-white" viewBox="0 0 12 12" fill="none">
                      <path d="M2.5 6l2.5 2.5 4.5-5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
                    </svg>
                  </Show>
                </span>

                <div class="flex-1 min-w-0">
                  <div class="flex items-center gap-1.5">
                    <code class="text-[11px] font-mono text-(--color-accent-fg)">{commit.short_sha}</code>
                    <span class="text-xs text-(--color-text-primary) truncate">{commit.message}</span>
                  </div>
                  <div class="flex items-center gap-1.5 mt-0.5">
                    <Show when={commit.author_avatar}>
                      <img
                        src={commit.author_avatar}
                        alt=""
                        class="w-3 h-3 rounded-full"
                        loading="lazy"
                      />
                    </Show>
                    <span class="text-[10px] text-(--color-text-muted)">{commit.author_login}</span>
                    <span class="text-[10px] text-(--color-text-muted)">{relativeTime(commit.authored_date)}</span>
                  </div>
                </div>
              </button>
            )}
          </For>
        </div>
      </div>
    </Popover>
  );
}
