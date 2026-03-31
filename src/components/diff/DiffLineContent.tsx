import { For } from "solid-js";
import type { WordDiffSpan } from "~/api/files";
import type { SearchRange } from "~/lib/search";
import { mergeSearchHighlights } from "~/lib/search";

interface DiffLineContentProps {
  content: string;
  wordDiffSpans?: WordDiffSpan[];
  searchRanges?: SearchRange[];
}

export function DiffLineContent(props: DiffLineContentProps) {
  const spans = () =>
    mergeSearchHighlights(
      props.content.length,
      props.wordDiffSpans ?? null,
      props.searchRanges ?? [],
    );

  return (
    <span>
      <For each={spans()}>
        {(span) => {
          const text = props.content.substring(span.start, span.end);

          // Build class list
          let cls = "";
          if (span.kind === "delete") cls += "bg-(--color-diff-del-word) rounded-sm ";
          if (span.kind === "insert") cls += "bg-(--color-diff-add-word) rounded-sm ";
          if (span.isActiveMatch) {
            cls += "bg-(--color-search-match-active) outline-1 outline-(--color-search-match-border) rounded-sm ";
          } else if (span.isSearchMatch) {
            cls += "bg-(--color-search-match) rounded-sm ";
          }

          return (
            <span
              class={cls || undefined}
              data-search-match={span.isActiveMatch ? "active" : undefined}
            >
              {text}
            </span>
          );
        }}
      </For>
    </span>
  );
}
