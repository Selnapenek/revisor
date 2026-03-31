import { onMount } from "solid-js";

interface CodeSearchBarProps {
  query: string;
  onQueryChange: (q: string) => void;
  totalMatches: number;
  activeIndex: number;
  onNext: () => void;
  onPrev: () => void;
  onClose: () => void;
}

export function CodeSearchBar(props: CodeSearchBarProps) {
  let inputRef!: HTMLInputElement;

  onMount(() => inputRef?.focus());

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      if (e.shiftKey) props.onPrev();
      else props.onNext();
    }
    if (e.key === "Escape") {
      e.preventDefault();
      props.onClose();
    }
  };

  const displayIndex = () => {
    if (props.totalMatches === 0) return "";
    return `${props.activeIndex + 1} of ${props.totalMatches}`;
  };

  return (
    <div class="flex items-center gap-2 px-3 py-1.5 border-b border-(--color-border-muted) bg-(--color-bg-secondary)">
      {/* Search icon */}
      <svg class="w-3.5 h-3.5 text-(--color-text-muted) shrink-0" viewBox="0 0 16 16" fill="currentColor">
        <path d="M11.5 7a4.5 4.5 0 1 1-9 0 4.5 4.5 0 0 1 9 0Zm-.82 4.74a6 6 0 1 1 1.06-1.06l3.04 3.04a.75.75 0 1 1-1.06 1.06l-3.04-3.04Z" />
      </svg>

      <input
        ref={inputRef}
        type="text"
        class="flex-1 bg-transparent text-xs text-(--color-text-primary) placeholder:text-(--color-text-muted) outline-none min-w-0"
        placeholder="Search in diff..."
        value={props.query}
        onInput={(e) => props.onQueryChange(e.currentTarget.value)}
        onKeyDown={handleKeyDown}
      />

      {/* Match count */}
      <span class="text-[10px] text-(--color-text-muted) shrink-0 tabular-nums">
        {props.query
          ? props.totalMatches === 0
            ? "No results"
            : displayIndex()
          : ""}
      </span>

      {/* Prev */}
      <button
        class="w-5 h-5 flex items-center justify-center rounded text-(--color-text-muted) hover:text-(--color-text-primary) hover:bg-(--color-bg-tertiary) transition-colors cursor-pointer disabled:opacity-30 disabled:cursor-default"
        onClick={props.onPrev}
        disabled={props.totalMatches === 0}
        title="Previous match (Shift+Enter)"
      >
        <svg class="w-3 h-3" viewBox="0 0 16 16" fill="currentColor">
          <path d="M3.22 9.78a.75.75 0 0 1 0-1.06l4.25-4.25a.75.75 0 0 1 1.06 0l4.25 4.25a.749.749 0 0 1-.326 1.275.749.749 0 0 1-.734-.215L8 6.06 4.28 9.78a.75.75 0 0 1-1.06 0Z" />
        </svg>
      </button>

      {/* Next */}
      <button
        class="w-5 h-5 flex items-center justify-center rounded text-(--color-text-muted) hover:text-(--color-text-primary) hover:bg-(--color-bg-tertiary) transition-colors cursor-pointer disabled:opacity-30 disabled:cursor-default"
        onClick={props.onNext}
        disabled={props.totalMatches === 0}
        title="Next match (Enter)"
      >
        <svg class="w-3 h-3" viewBox="0 0 16 16" fill="currentColor">
          <path d="M12.78 6.22a.75.75 0 0 1 0 1.06l-4.25 4.25a.75.75 0 0 1-1.06 0L3.22 7.28a.749.749 0 0 1 .326-1.275.749.749 0 0 1 .734.215L8 9.94l3.72-3.72a.75.75 0 0 1 1.06 0Z" />
        </svg>
      </button>

      {/* Close */}
      <button
        class="w-5 h-5 flex items-center justify-center rounded text-(--color-text-muted) hover:text-(--color-text-primary) hover:bg-(--color-bg-tertiary) transition-colors cursor-pointer"
        onClick={props.onClose}
        title="Close (Escape)"
      >
        <svg class="w-3 h-3" viewBox="0 0 16 16" fill="currentColor">
          <path d="M3.72 3.72a.75.75 0 0 1 1.06 0L8 6.94l3.22-3.22a.749.749 0 0 1 1.275.326.749.749 0 0 1-.215.734L9.06 8l3.22 3.22a.749.749 0 0 1-.326 1.275.749.749 0 0 1-.734-.215L8 9.06l-3.22 3.22a.751.751 0 0 1-1.042-.018.751.751 0 0 1-.018-1.042L6.94 8 3.72 4.78a.75.75 0 0 1 0-1.06Z" />
        </svg>
      </button>
    </div>
  );
}
