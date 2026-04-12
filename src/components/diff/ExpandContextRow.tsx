import { Show } from "solid-js";
import { EXPAND_LINES } from "~/lib/expand-context";

interface ExpandContextRowProps {
  onExpandUp?: () => void;
  onExpandDown?: () => void;
  onExpandAll?: () => void;
  lineCount: number;
  colspan: number;
  loading?: boolean;
}

export function ExpandContextRow(props: ExpandContextRowProps) {
  const showIncremental = () => props.lineCount > EXPAND_LINES * 2;

  return (
    <tr class="select-none">
      <td
        colspan={props.colspan}
        class="py-0.5 text-center bg-(--color-diff-hunk-bg) border-y border-(--color-border-muted)"
      >
        <Show
          when={!props.loading}
          fallback={
            <span class="text-xs text-(--color-text-muted)">Loading...</span>
          }
        >
          <div class="flex items-center justify-center gap-1">
            <Show when={props.onExpandUp && showIncremental()}>
              <button
                class="inline-flex items-center gap-0.5 px-1.5 py-0 text-xs text-(--color-accent-fg) hover:bg-(--color-bg-tertiary) rounded cursor-pointer"
                onClick={props.onExpandUp}
                title={`Show ${EXPAND_LINES} more lines above`}
              >
                <svg class="w-3 h-3" viewBox="0 0 16 16" fill="currentColor">
                  <path d="M3.22 9.78a.75.75 0 0 1 0-1.06l4.25-4.25a.75.75 0 0 1 1.06 0l4.25 4.25a.75.75 0 0 1-1.06 1.06L8 6.06 4.28 9.78a.75.75 0 0 1-1.06 0Z" />
                </svg>
                <span>{EXPAND_LINES}</span>
              </button>
            </Show>

            <Show when={props.onExpandAll}>
              <button
                class="inline-flex items-center gap-0.5 px-1.5 py-0 text-xs text-(--color-accent-fg) hover:bg-(--color-bg-tertiary) rounded cursor-pointer"
                onClick={props.onExpandAll}
                title={
                  props.lineCount > 0
                    ? `Show all ${props.lineCount} hidden lines`
                    : "Show all hidden lines"
                }
              >
                <svg class="w-3 h-3" viewBox="0 0 16 16" fill="currentColor">
                  <path d="M3.22 5.22a.75.75 0 0 1 1.06 0L8 8.94l3.72-3.72a.75.75 0 1 1 1.06 1.06l-4.25 4.25a.75.75 0 0 1-1.06 0L3.22 6.28a.75.75 0 0 1 0-1.06Zm0-4a.75.75 0 0 1 1.06 0L8 4.94l3.72-3.72a.75.75 0 1 1 1.06 1.06L8.53 6.53a.75.75 0 0 1-1.06 0L3.22 2.28a.75.75 0 0 1 0-1.06Z" />
                </svg>
                <Show when={props.lineCount > 0}>
                  <span>{props.lineCount}</span>
                </Show>
              </button>
            </Show>

            <Show when={props.onExpandDown && showIncremental()}>
              <button
                class="inline-flex items-center gap-0.5 px-1.5 py-0 text-xs text-(--color-accent-fg) hover:bg-(--color-bg-tertiary) rounded cursor-pointer"
                onClick={props.onExpandDown}
                title={`Show ${EXPAND_LINES} more lines below`}
              >
                <svg class="w-3 h-3" viewBox="0 0 16 16" fill="currentColor">
                  <path d="M12.78 6.22a.75.75 0 0 1 0 1.06l-4.25 4.25a.75.75 0 0 1-1.06 0L3.22 7.28a.75.75 0 0 1 1.06-1.06L8 9.94l3.72-3.72a.75.75 0 0 1 1.06 0Z" />
                </svg>
                <span>{EXPAND_LINES}</span>
              </button>
            </Show>
          </div>
        </Show>
      </td>
    </tr>
  );
}
