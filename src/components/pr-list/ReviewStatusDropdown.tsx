import { For, Show } from "solid-js";
import { Popover } from "~/components/ui/Popover";
import { REVIEW_STATUS_OPTIONS } from "~/stores/pr-filters.store";

interface ReviewStatusDropdownProps {
  selected: string | null;
  onSelect: (value: string | null) => void;
}

export function ReviewStatusDropdown(props: ReviewStatusDropdownProps) {
  const hasSelection = () => props.selected !== null;

  return (
    <Popover
      trigger={({ ref, onClick, isOpen }) => (
        <button
          ref={ref}
          onClick={onClick}
          class={`inline-flex items-center gap-1.5 px-2.5 py-1 text-xs font-medium rounded-md border transition-colors cursor-pointer ${
            isOpen || hasSelection()
              ? "border-(--color-accent-emphasis) text-(--color-accent-fg) bg-(--color-accent-emphasis)/10"
              : "border-(--color-border-default) text-(--color-text-secondary) hover:text-(--color-text-primary) hover:border-(--color-border-muted)"
          }`}
        >
          Reviews
          <Show when={hasSelection()}>
            <span class="inline-flex items-center justify-center w-1.5 h-1.5 rounded-full bg-(--color-accent-emphasis)" />
          </Show>
          <svg class="w-3 h-3 opacity-50" viewBox="0 0 12 12" fill="none">
            <path d="M3 5l3 3 3-3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        </button>
      )}
    >
      <div class="py-1">
        <div class="px-3 py-1.5 text-xs font-semibold text-(--color-text-primary) border-b border-(--color-border-muted) mb-1">
          Filter by reviews
        </div>
        <For each={REVIEW_STATUS_OPTIONS}>
          {(option) => {
            const isSelected = () => props.selected === option.value;
            return (
              <button
                class="flex items-center gap-2 w-full px-3 py-1.5 text-xs text-left hover:bg-(--color-bg-tertiary) transition-colors cursor-pointer"
                onClick={() => props.onSelect(isSelected() ? null : option.value)}
              >
                <span class="w-4 shrink-0 text-center">
                  <Show when={isSelected()}>
                    <svg class="w-3 h-3 text-(--color-accent-fg) inline" viewBox="0 0 12 12" fill="none">
                      <path d="M2.5 6l2.5 2.5 4.5-5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
                    </svg>
                  </Show>
                </span>
                <span class="text-(--color-text-primary)">{option.label}</span>
              </button>
            );
          }}
        </For>
      </div>
    </Popover>
  );
}
