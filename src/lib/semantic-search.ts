import type { ProcessedFile } from "../api/files";
import type { CommitSelection } from "../stores/pr-detail.store";

type PrVersion = { number: number; base_sha: string; head_sha: string };

export interface SemanticSnapshot {
  owner: string;
  repo: string;
  number: number;
  baseSha: string;
  headSha: string;
  files: {
    filename: string;
    status: string;
    isBinary: boolean;
    isTooLarge: boolean;
    hunks: {
      header: string;
      oldStart: number;
      newStart: number;
      lines: { kind: string; content: string; oldLineno: number | null; newLineno: number | null }[];
    }[];
  }[];
}

export interface SemanticHit {
  filename: string;
  hunkIndex: number;
  oldStart: number;
  newStart: number;
  preview: string;
  score: number;
  confidence: number;
}

export interface SemanticResult {
  results: SemanticHit[];
  skipped: { filename: string; reason: string }[];
  analyzedCount: number;
  model: string;
  inputTokens: number;
  outputTokens: number;
  error: string | null;
}

export type SemanticAnchor = { side: "old" | "new"; line: number };

export function prepareSemanticSnapshot(owner: string, repo: string, pr: PrVersion, files: ProcessedFile[]): SemanticSnapshot {
  return {
    owner, repo, number: pr.number, baseSha: pr.base_sha, headSha: pr.head_sha,
    files: files.map((file) => ({
      filename: file.filename,
      status: file.status,
      isBinary: file.diff?.is_binary ?? false,
      isTooLarge: file.diff?.is_too_large ?? false,
      hunks: file.diff?.hunks.map((hunk) => ({
        header: hunk.header,
        oldStart: hunk.old_start,
        newStart: hunk.new_start,
        lines: hunk.lines.map((line) => ({
          kind: line.kind, content: line.content, oldLineno: line.old_lineno, newLineno: line.new_lineno,
        })),
      })) ?? [],
    })),
  };
}

export function semanticAnchor(file: ProcessedFile, hunkIndex: number): SemanticAnchor | null {
  const lines = file.diff?.hunks[hunkIndex]?.lines;
  if (!lines) return null;
  const added = lines.find((line) => line.kind === "added" && line.new_lineno !== null);
  if (added?.new_lineno != null) return { side: "new", line: added.new_lineno };
  const removed = lines.find((line) => line.kind === "removed" && line.old_lineno !== null);
  return removed?.old_lineno != null ? { side: "old", line: removed.old_lineno } : null;
}

export function canApplySemanticResult(
  requestId: number, currentRequestId: number,
  submittedQuery: string, currentQuery: string,
  snapshot: SemanticSnapshot, owner: string, repo: string,
  pr: PrVersion | null, selection: CommitSelection, filesLoading: boolean,
): boolean {
  return requestId === currentRequestId
    && submittedQuery === currentQuery.trim()
    && !filesLoading && selection.type === "all"
    && pr !== null
    && snapshot.owner === owner && snapshot.repo === repo && snapshot.number === pr.number
    && snapshot.baseSha === pr.base_sha && snapshot.headSha === pr.head_sha;
}

export function semanticLineRange(start: number, count: number): string {
  if (count === 0) return "none";
  return count === 1 ? `${start}` : `${start}–${start + count - 1}`;
}
