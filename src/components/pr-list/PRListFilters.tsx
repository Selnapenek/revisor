import type { PrFilter } from "~/stores/pr-list.store";

interface PRListFiltersProps {
  current: PrFilter;
  onChange: (filter: PrFilter) => void;
}

const FILTERS: { value: PrFilter; label: string }[] = [
  { value: "open", label: "Open" },
  { value: "closed", label: "Closed" },
  { value: "all", label: "All" },
];

export function PRListFilters(props: PRListFiltersProps) {
  return (
    <div class="flex gap-1 bg-(--color-bg-tertiary) rounded-md p-0.5">
      {FILTERS.map((f) => (
        <button
          class={`px-3 py-1 text-xs font-medium rounded transition-colors cursor-pointer ${
            props.current === f.value
              ? "bg-(--color-bg-secondary) text-(--color-text-primary) shadow-sm"
              : "text-(--color-text-secondary) hover:text-(--color-text-primary)"
          }`}
          onClick={() => props.onChange(f.value)}
        >
          {f.label}
        </button>
      ))}
    </div>
  );
}
