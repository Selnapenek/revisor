import type { ProcessedFile } from "../api/files";
import type { CommitSelection } from "../stores/pr-detail.store";

type PrVersion = { number: number; base_sha: string; head_sha: string };

export interface FocusFileInput {
  filename: string;
  status: string;
  hunks: { header: string; lines: { kind: "context" | "added" | "removed"; content: string }[] }[];
  isBinary: boolean;
  isTooLarge: boolean;
}

export interface FocusSnapshot {
  owner: string;
  repo: string;
  number: number;
  baseSha: string;
  headSha: string;
  files: FocusFileInput[];
}

export interface FocusSuggestion {
  filename: string;
  topic: string;
  score: number;
  confidence: number;
}

export interface FocusSkippedFile {
  filename: string;
  reason: string;
}

export interface FocusResult {
  suggestions: FocusSuggestion[];
  skipped: FocusSkippedFile[];
  analyzedCount: number;
  model: string;
  inputTokens: number;
  outputTokens: number;
  error: string | null;
}

export function prepareFocusSnapshot(
  owner: string,
  repo: string,
  pr: PrVersion,
  files: ProcessedFile[],
): FocusSnapshot {
  return {
    owner,
    repo,
    number: pr.number,
    baseSha: pr.base_sha,
    headSha: pr.head_sha,
    files: files.map((file) => ({
      filename: file.filename,
      status: file.status,
      isBinary: file.diff?.is_binary ?? false,
      isTooLarge: file.diff?.is_too_large ?? false,
      hunks: file.diff?.hunks.map((hunk) => ({
        header: hunk.header,
        lines: hunk.lines.map((line) => ({ kind: line.kind, content: line.content })),
      })) ?? [],
    })),
  };
}

export function isFocusSnapshotCurrent(
  snapshot: FocusSnapshot,
  owner: string,
  repo: string,
  pr: PrVersion | null,
  selection: CommitSelection,
): boolean {
  return selection.type === "all"
    && pr !== null
    && snapshot.owner === owner
    && snapshot.repo === repo
    && snapshot.number === pr.number
    && snapshot.baseSha === pr.base_sha
    && snapshot.headSha === pr.head_sha;
}

export function canApplyFocusResponse(
  requestId: number,
  currentRequestId: number,
  snapshot: FocusSnapshot,
  owner: string,
  repo: string,
  pr: PrVersion | null,
  selection: CommitSelection,
): boolean {
  return requestId === currentRequestId
    && isFocusSnapshotCurrent(snapshot, owner, repo, pr, selection);
}

export function isLatestFocusRequest(requestId: number, currentRequestId: number): boolean {
  return requestId === currentRequestId;
}

export function canAnalyzeFocusFiles(
  snapshot: FocusSnapshot,
  owner: string,
  repo: string,
  pr: PrVersion | null,
  selection: CommitSelection,
  filesLoading: boolean,
): boolean {
  return !filesLoading && isFocusSnapshotCurrent(snapshot, owner, repo, pr, selection);
}
