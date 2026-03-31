import { createSignal, For, Show, createMemo } from "solid-js";
import { Popover } from "~/components/ui/Popover";

export interface FilterOption {
  value: string;
  label: string;
  color?: string;
  avatar?: string;
}

interface FilterDropdownProps {
  label: string;
  options: FilterOption[];
  selected: string[];
  onToggle: (value: string) => void;
  loading?: boolean;
}

export function FilterDropdown(props: FilterDropdownProps) {
  const [search, setSearch] = createSignal("");

  const filtered = createMemo(() => {
    const q = search().toLowerCase();
    if (!q) return props.options;
    return props.options.filter((o) => o.label.toLowerCase().includes(q));
  });

  const count = () => props.selected.length;

  return (
    <Popover
      trigger={({ ref, onClick, isOpen }) => (
        <button
          ref={ref}
          onClick={onClick}
          class={`inline-flex items-center gap-1.5 px-2.5 py-1 text-xs font-medium rounded-md border transition-colors cursor-pointer ${
            isOpen || count() > 0
              ? "border-(--color-accent-emphasis) text-(--color-accent-fg) bg-(--color-accent-emphasis)/10"
              : "border-(--color-border-default) text-(--color-text-secondary) hover:text-(--color-text-primary) hover:border-(--color-border-muted)"
          }`}
        >
          {props.label}
          <Show when={count() > 0}>
            <span class="inline-flex items-center justify-center w-4 h-4 text-[10px] font-semibold rounded-full bg-(--color-accent-emphasis) text-white">
              {count()}
            </span>
          </Show>
          <svg class="w-3 h-3 opacity-50" viewBox="0 0 12 12" fill="none">
            <path d="M3 5l3 3 3-3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        </button>
      )}
    >
      <div class="flex flex-col max-h-[300px]">
        {/* Search input */}
        <div class="p-2 border-b border-(--color-border-muted)">
          <input
            type="text"
            placeholder={`Filter ${props.label.toLowerCase()}...`}
            value={search()}
            onInput={(e) => setSearch(e.currentTarget.value)}
            class="w-full px-2 py-1 text-xs bg-(--color-bg-primary) border border-(--color-border-default) rounded text-(--color-text-primary) placeholder:text-(--color-text-muted) focus:outline-none focus:border-(--color-accent-emphasis)"
          />
        </div>

        {/* Options list */}
        <div class="overflow-y-auto">
          <Show
            when={!props.loading}
            fallback={
              <div class="px-3 py-4 text-xs text-(--color-text-muted) text-center">Loading...</div>
            }
          >
            <Show
              when={filtered().length > 0}
              fallback={
                <div class="px-3 py-4 text-xs text-(--color-text-muted) text-center">No matches</div>
              }
            >
              <For each={filtered()}>
                {(option) => {
                  const isSelected = () => props.selected.includes(option.value);
                  return (
                    <button
                      class="flex items-center gap-2 w-full px-3 py-1.5 text-xs text-left hover:bg-(--color-bg-tertiary) transition-colors cursor-pointer"
                      onClick={() => props.onToggle(option.value)}
                    >
                      {/* Checkbox */}
                      <span
                        class={`flex items-center justify-center w-3.5 h-3.5 rounded border shrink-0 ${
                          isSelected()
                            ? "bg-(--color-accent-emphasis) border-(--color-accent-emphasis)"
                            : "border-(--color-border-default)"
                        }`}
                      >
                        <Show when={isSelected()}>
                          <svg class="w-2.5 h-2.5 text-white" viewBox="0 0 12 12" fill="none">
                            <path d="M2.5 6l2.5 2.5 4.5-5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
                          </svg>
                        </Show>
                      </span>

                      {/* Color dot for labels */}
                      <Show when={option.color}>
                        <span
                          class="w-3 h-3 rounded-full shrink-0 border border-black/10"
                          style={{ "background-color": `#${option.color}` }}
                        />
                      </Show>

                      {/* Avatar for users */}
                      <Show when={option.avatar}>
                        <img
                          src={option.avatar}
                          alt=""
                          class="w-4 h-4 rounded-full shrink-0"
                          loading="lazy"
                        />
                      </Show>

                      <span class="text-(--color-text-primary) truncate">{option.label}</span>
                    </button>
                  );
                }}
              </For>
            </Show>
          </Show>
        </div>
      </div>
    </Popover>
  );
}
