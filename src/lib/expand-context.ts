import type { DiffHunk, DiffLine, SideBySideRow } from "~/api/files";

export const EXPAND_LINES = 20;

export interface ExpandableGap {
  /** "before" = above first hunk, "between" = between two hunks, "after" = below last hunk */
  position: "before" | "between" | "after";
  /** Index of the hunk AFTER the gap (for "before"/"between"), or last hunk index (for "after") */
  hunkIndex: number;
  /** Number of hidden lines in this gap */
  lineCount: number;
}

/** Compute all expandable gaps between hunks */
export function computeGaps(hunks: DiffHunk[], totalLines: number | null): ExpandableGap[] {
  if (hunks.length === 0) return [];

  const gaps: ExpandableGap[] = [];

  // Gap before first hunk
  const firstHunk = hunks[0];
  if (firstHunk.old_start > 1) {
    gaps.push({
      position: "before",
      hunkIndex: 0,
      lineCount: firstHunk.old_start - 1,
    });
  }

  // Gaps between hunks
  for (let i = 0; i < hunks.length - 1; i++) {
    const curr = hunks[i];
    const next = hunks[i + 1];
    const gapStart = curr.old_start + curr.old_count;
    const gapEnd = next.old_start - 1;
    if (gapEnd >= gapStart) {
      gaps.push({
        position: "between",
        hunkIndex: i + 1,
        lineCount: gapEnd - gapStart + 1,
      });
    }
  }

  // Gap after last hunk
  const lastHunk = hunks[hunks.length - 1];
  const lastLine = lastHunk.old_start + lastHunk.old_count - 1;
  if (totalLines !== null && lastLine < totalLines) {
    gaps.push({
      position: "after",
      hunkIndex: hunks.length - 1,
      lineCount: totalLines - lastLine,
    });
  } else if (totalLines === null) {
    // We don't know total lines yet — show expand-down anyway
    gaps.push({
      position: "after",
      hunkIndex: hunks.length - 1,
      lineCount: -1, // unknown
    });
  }

  return gaps;
}

/** Build context DiffLines from file lines array (0-indexed) for a given 1-based line range */
function buildContextLines(fileLines: string[], fromLine: number, toLine: number): DiffLine[] {
  const result: DiffLine[] = [];
  for (let i = fromLine; i <= toLine && i <= fileLines.length; i++) {
    result.push({
      kind: "context",
      content: fileLines[i - 1] ?? "",
      old_lineno: i,
      new_lineno: null, // will be computed below
    });
  }
  return result;
}

/** Compute new_lineno offset for context lines based on surrounding hunks */
function computeNewLineOffset(hunks: DiffHunk[], hunkIndex: number, position: "before" | "after"): number {
  if (position === "before") {
    const hunk = hunks[hunkIndex];
    // new_start - old_start gives the offset between old and new line numbers
    return hunk.new_start - hunk.old_start;
  } else {
    const hunk = hunks[hunkIndex];
    // After a hunk, offset = (new_start + new_count) - (old_start + old_count)
    return (hunk.new_start + hunk.new_count) - (hunk.old_start + hunk.old_count);
  }
}

/** Expand N lines downward after a hunk */
export function expandDown(
  hunks: DiffHunk[],
  hunkIndex: number,
  fileLines: string[],
  count: number = EXPAND_LINES,
): DiffHunk[] {
  const result = hunks.map((h) => ({ ...h, lines: [...h.lines] }));
  const hunk = result[hunkIndex];
  const nextHunk = result[hunkIndex + 1];

  const gapStart = hunk.old_start + hunk.old_count;
  const gapEnd = nextHunk
    ? nextHunk.old_start - 1
    : fileLines.length;
  const expandEnd = Math.min(gapStart + count - 1, gapEnd);

  if (gapStart > expandEnd) return result;

  const offset = computeNewLineOffset(result, hunkIndex, "after");
  const newLines = buildContextLines(fileLines, gapStart, expandEnd);
  newLines.forEach((line) => {
    line.new_lineno = line.old_lineno! + offset;
  });

  hunk.lines.push(...newLines);
  hunk.old_count += newLines.length;
  hunk.new_count += newLines.length;

  return result;
}

/** Expand N lines upward before a hunk */
export function expandUp(
  hunks: DiffHunk[],
  hunkIndex: number,
  fileLines: string[],
  count: number = EXPAND_LINES,
): DiffHunk[] {
  const result = hunks.map((h) => ({ ...h, lines: [...h.lines] }));
  const hunk = result[hunkIndex];
  const prevHunk = result[hunkIndex - 1];

  const gapEnd = hunk.old_start - 1;
  const gapStart = prevHunk
    ? prevHunk.old_start + prevHunk.old_count
    : 1;
  const expandStart = Math.max(gapEnd - count + 1, gapStart);

  if (expandStart > gapEnd) return result;

  const offset = computeNewLineOffset(result, hunkIndex, "before");
  const newLines = buildContextLines(fileLines, expandStart, gapEnd);
  newLines.forEach((line) => {
    line.new_lineno = line.old_lineno! + offset;
  });

  hunk.lines.unshift(...newLines);
  hunk.old_start = expandStart;
  hunk.new_start = expandStart + offset;
  hunk.old_count += newLines.length;
  hunk.new_count += newLines.length;

  return result;
}

/** Fill the entire gap between two hunks and merge them into one */
export function expandBetween(
  hunks: DiffHunk[],
  hunkIndex: number,
  fileLines: string[],
): DiffHunk[] {
  // hunkIndex = index of the hunk AFTER the gap
  const prevIdx = hunkIndex - 1;
  if (prevIdx < 0) return hunks;

  const result = hunks.map((h) => ({ ...h, lines: [...h.lines] }));
  const prevHunk = result[prevIdx];
  const nextHunk = result[hunkIndex];

  const gapStart = prevHunk.old_start + prevHunk.old_count;
  const gapEnd = nextHunk.old_start - 1;

  if (gapStart > gapEnd) {
    // No actual gap — just merge
  } else {
    const offset = computeNewLineOffset(result, prevIdx, "after");
    const newLines = buildContextLines(fileLines, gapStart, gapEnd);
    newLines.forEach((line) => {
      line.new_lineno = line.old_lineno! + offset;
    });
    prevHunk.lines.push(...newLines);
  }

  // Merge nextHunk lines into prevHunk
  prevHunk.lines.push(...nextHunk.lines);
  prevHunk.old_count = (nextHunk.old_start + nextHunk.old_count) - prevHunk.old_start;
  prevHunk.new_count = (nextHunk.new_start + nextHunk.new_count) - prevHunk.new_start;

  // Remove nextHunk
  result.splice(hunkIndex, 1);

  return result;
}

/** Expand all lines before the first hunk */
export function expandAllBefore(
  hunks: DiffHunk[],
  fileLines: string[],
): DiffHunk[] {
  if (hunks.length === 0 || hunks[0].old_start <= 1) return hunks;
  return expandUp(hunks, 0, fileLines, hunks[0].old_start - 1);
}

/** Expand all lines after the last hunk */
export function expandAllAfter(
  hunks: DiffHunk[],
  fileLines: string[],
): DiffHunk[] {
  if (hunks.length === 0) return hunks;
  const lastHunk = hunks[hunks.length - 1];
  const remaining = fileLines.length - (lastHunk.old_start + lastHunk.old_count - 1);
  if (remaining <= 0) return hunks;
  return expandDown(hunks, hunks.length - 1, fileLines, remaining);
}

/** TypeScript port of Rust to_side_by_side() */
export function toSideBySide(hunks: DiffHunk[]): SideBySideRow[] {
  const rows: SideBySideRow[] = [];

  for (const hunk of hunks) {
    // Hunk header row
    rows.push({
      left: null,
      right: null,
      is_hunk_header: true,
      hunk_header: hunk.header,
    });

    let i = 0;
    const lines = hunk.lines;

    while (i < lines.length) {
      if (lines[i].kind === "context") {
        rows.push({
          left: lines[i],
          right: lines[i],
          is_hunk_header: false,
          hunk_header: null,
        });
        i++;
      } else {
        // Collect consecutive removed and added lines
        const removed: DiffLine[] = [];
        const added: DiffLine[] = [];

        while (i < lines.length && lines[i].kind !== "context") {
          if (lines[i].kind === "removed") removed.push(lines[i]);
          else if (lines[i].kind === "added") added.push(lines[i]);
          i++;
        }

        const maxLen = Math.max(removed.length, added.length);
        for (let j = 0; j < maxLen; j++) {
          rows.push({
            left: removed[j] ?? null,
            right: added[j] ?? null,
            is_hunk_header: false,
            hunk_header: null,
          });
        }
      }
    }
  }

  return rows;
}
