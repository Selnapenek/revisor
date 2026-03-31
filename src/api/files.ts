import { invoke } from "@tauri-apps/api/core";
import type { ReviewComment } from "~/types/pr";

export interface ProcessedDiff {
  hunks: DiffHunk[];
  side_by_side: SideBySideRow[];
  word_diffs: WordDiffPair[];
  is_binary: boolean;
  is_too_large: boolean;
}

export interface DiffHunk {
  header: string;
  old_start: number;
  old_count: number;
  new_start: number;
  new_count: number;
  lines: DiffLine[];
}

export interface DiffLine {
  kind: "context" | "added" | "removed";
  content: string;
  old_lineno: number | null;
  new_lineno: number | null;
}

export interface SideBySideRow {
  left: DiffLine | null;
  right: DiffLine | null;
  is_hunk_header: boolean;
  hunk_header: string | null;
}

export interface WordDiffPair {
  old_lineno: number | null;
  new_lineno: number | null;
  old_spans: WordDiffSpan[];
  new_spans: WordDiffSpan[];
}

export interface WordDiffSpan {
  start: number;
  end: number;
  kind: "equal" | "insert" | "delete";
}

export interface ProcessedFile {
  sha: string;
  filename: string;
  status: string;
  additions: number;
  deletions: number;
  diff: ProcessedDiff | null;
}

export async function getPrFiles(
  owner: string,
  repo: string,
  number: number,
): Promise<ProcessedFile[]> {
  return invoke<ProcessedFile[]>("get_pr_files", { owner, repo, number });
}

export async function getPrComments(
  owner: string,
  repo: string,
  number: number,
): Promise<ReviewComment[]> {
  return invoke<ReviewComment[]>("get_pr_comments", { owner, repo, number });
}

export async function addComment(
  owner: string,
  repo: string,
  number: number,
  path: string,
  line: number,
  side: string,
  body: string,
  commitId: string,
): Promise<ReviewComment> {
  return invoke<ReviewComment>("add_comment", {
    owner, repo, number, path, line, side, body, commitId,
  });
}

export async function replyToComment(
  owner: string,
  repo: string,
  number: number,
  commentId: number,
  body: string,
): Promise<ReviewComment> {
  return invoke<ReviewComment>("reply_to_comment", {
    owner, repo, number, commentId, body,
  });
}

export async function updateComment(
  owner: string,
  repo: string,
  commentId: number,
  body: string,
): Promise<ReviewComment> {
  return invoke<ReviewComment>("update_comment", {
    owner, repo, commentId, body,
  });
}

export async function deleteComment(
  owner: string,
  repo: string,
  commentId: number,
): Promise<void> {
  return invoke("delete_comment", { owner, repo, commentId });
}
