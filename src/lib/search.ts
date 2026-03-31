import type { WordDiffSpan } from "~/api/files";

export interface SearchRange {
  start: number;
  end: number;
  isActive: boolean;
}

export interface HighlightSpan {
  start: number;
  end: number;
  kind: "equal" | "insert" | "delete";
  isSearchMatch: boolean;
  isActiveMatch: boolean;
}

/**
 * Find all non-overlapping, case-insensitive occurrences of `query` in `content`.
 */
export function findMatchesInString(
  content: string,
  query: string,
): { start: number; end: number }[] {
  if (!query || !content) return [];
  const matches: { start: number; end: number }[] = [];
  const lowerContent = content.toLowerCase();
  const lowerQuery = query.toLowerCase();
  let pos = 0;
  while (pos <= lowerContent.length - lowerQuery.length) {
    const idx = lowerContent.indexOf(lowerQuery, pos);
    if (idx === -1) break;
    matches.push({ start: idx, end: idx + lowerQuery.length });
    pos = idx + lowerQuery.length;
  }
  return matches;
}

/**
 * Merge word-diff spans with search highlight ranges into a single flat span list.
 * If `wordDiffSpans` is null, treats the entire content as a single "equal" span.
 */
export function mergeSearchHighlights(
  contentLength: number,
  wordDiffSpans: WordDiffSpan[] | null,
  searchRanges: SearchRange[],
): HighlightSpan[] {
  // Build base spans from word-diff or a single "equal" span
  let base: HighlightSpan[];
  if (wordDiffSpans && wordDiffSpans.length > 0) {
    base = wordDiffSpans.map((s) => ({
      start: s.start,
      end: s.end,
      kind: s.kind,
      isSearchMatch: false,
      isActiveMatch: false,
    }));
  } else {
    if (contentLength <= 0) return [];
    base = [{ start: 0, end: contentLength, kind: "equal", isSearchMatch: false, isActiveMatch: false }];
  }

  if (searchRanges.length === 0) return base;

  // Split base spans at search range boundaries
  const result: HighlightSpan[] = [];

  for (const span of base) {
    // Collect search ranges that overlap this span
    const overlapping = searchRanges.filter(
      (sr) => sr.start < span.end && sr.end > span.start,
    );

    if (overlapping.length === 0) {
      result.push(span);
      continue;
    }

    // Build split points within this span
    const cuts = new Set<number>();
    cuts.add(span.start);
    cuts.add(span.end);
    for (const sr of overlapping) {
      if (sr.start > span.start && sr.start < span.end) cuts.add(sr.start);
      if (sr.end > span.start && sr.end < span.end) cuts.add(sr.end);
    }

    const sorted = Array.from(cuts).sort((a, b) => a - b);
    for (let i = 0; i < sorted.length - 1; i++) {
      const s = sorted[i];
      const e = sorted[i + 1];
      // Check if this sub-span falls within any search range
      const matchingRange = overlapping.find((sr) => sr.start <= s && sr.end >= e);
      result.push({
        start: s,
        end: e,
        kind: span.kind,
        isSearchMatch: !!matchingRange,
        isActiveMatch: matchingRange?.isActive ?? false,
      });
    }
  }

  return result;
}
