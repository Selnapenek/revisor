import { createSignal, createMemo } from "solid-js";
import type { PullRequest, ReviewComment, ThreadResolutionInfo, Commit } from "~/types/pr";
import type { ProcessedFile, DiffHunk, SideBySideRow } from "~/api/files";
import { getFileContent } from "~/api/files";
import { toSideBySide } from "~/lib/expand-context";

type DiffMode = "split" | "unified";

export type CommitSelection =
  | { type: "all" }
  | { type: "single"; index: number }
  | { type: "range"; from: number; to: number };

const [pr, setPr] = createSignal<PullRequest | null>(null);
const [files, setFiles] = createSignal<ProcessedFile[]>([]);
const [comments, setComments] = createSignal<ReviewComment[]>([]);
const [reviewedFiles, setReviewedFiles] = createSignal<Set<string>>(new Set());
const [selectedFilePath, setSelectedFilePath] = createSignal<string | null>(null);
const [diffMode, setDiffMode] = createSignal<DiffMode>("split");
const [isLoadingFiles, setIsLoadingFiles] = createSignal(false);
const [isLoadingComments, setIsLoadingComments] = createSignal(false);
const [threadResolutions, setThreadResolutions] = createSignal<Map<number, ThreadResolutionInfo>>(new Map());
const [commits, setCommits] = createSignal<Commit[]>([]);
const [commitSelection, setCommitSelection] = createSignal<CommitSelection>({ type: "all" });

// In-memory cache for fetched file blobs (keyed by "owner/repo/path@ref")
const fileLinesCache = new Map<string, string[]>();
// Track inflight fetches to avoid duplicate requests
const fileLinesInflight = new Map<string, Promise<string[]>>();

export const prDetailStore = {
  get pr() { return pr(); },
  get files() { return files(); },
  get comments() { return comments(); },
  get reviewedFiles() { return reviewedFiles(); },
  get selectedFilePath() { return selectedFilePath(); },
  get diffMode() { return diffMode(); },
  get isLoadingFiles() { return isLoadingFiles(); },
  get isLoadingComments() { return isLoadingComments(); },
  get threadResolutions() { return threadResolutions(); },
  get commits() { return commits(); },
  get commitSelection() { return commitSelection(); },

  get commitSelectionLabel(): string {
    const sel = commitSelection();
    const list = commits();
    if (sel.type === "all") return `All commits (${list.length})`;
    if (sel.type === "single") {
      const c = list[sel.index];
      return c ? `${c.short_sha} ${c.message}` : "1 commit";
    }
    const count = sel.to - sel.from + 1;
    return `${count} commits`;
  },

  get selectedFile(): ProcessedFile | null {
    const path = selectedFilePath();
    if (!path) return null;
    return files().find((f) => f.filename === path) ?? null;
  },

  get fileComments(): ReviewComment[] {
    const path = selectedFilePath();
    if (!path) return [];
    return comments().filter((c) => c.path === path);
  },

  getCommentsForFile(path: string): ReviewComment[] {
    return comments().filter((c) => c.path === path);
  },

  get reviewProgress(): { reviewed: number; total: number } {
    const total = files().length;
    const reviewed = files().filter((f) => reviewedFiles().has(f.filename)).length;
    return { reviewed, total };
  },

  setPr,
  setFiles,
  setComments,
  setReviewedFiles,
  setSelectedFilePath,
  setDiffMode,
  setIsLoadingFiles,
  setIsLoadingComments,
  setThreadResolutions,
  setCommits,
  setCommitSelection,

  addComment(comment: ReviewComment) {
    setComments((prev) => [...prev, comment]);
  },

  updateComment(commentId: number, body: string) {
    setComments((prev) =>
      prev.map((c) =>
        c.id === commentId
          ? { ...c, body, updated_at: new Date().toISOString() }
          : c
      )
    );
  },

  deleteComment(commentId: number) {
    setComments((prev) => prev.filter((c) => c.id !== commentId));
  },

  toggleFileReviewed(filePath: string) {
    setReviewedFiles((prev) => {
      const next = new Set(prev);
      if (next.has(filePath)) {
        next.delete(filePath);
      } else {
        next.add(filePath);
      }
      return next;
    });
  },

  isFileReviewed(filePath: string): boolean {
    return reviewedFiles().has(filePath);
  },

  navigateFile(direction: 1 | -1, fileList: ProcessedFile[]) {
    if (fileList.length === 0) return;
    const current = selectedFilePath();
    const idx = current ? fileList.findIndex((f) => f.filename === current) : -1;
    const next = (idx + direction + fileList.length) % fileList.length;
    setSelectedFilePath(fileList[next].filename);
  },

  selectedFileIndex(fileList: ProcessedFile[]): number {
    const current = selectedFilePath();
    if (!current) return -1;
    return fileList.findIndex((f) => f.filename === current);
  },

  setThreadResolved(rootCommentId: number, isResolved: boolean) {
    setThreadResolutions((prev) => {
      const next = new Map(prev);
      const existing = next.get(rootCommentId);
      if (existing) {
        next.set(rootCommentId, { ...existing, is_resolved: isResolved });
      }
      return next;
    });
  },

  /** Fetch file lines from GitHub (cached in memory). Returns the lines array. */
  async fetchFileLines(owner: string, repo: string, path: string, ref_: string): Promise<string[]> {
    const key = `${owner}/${repo}/${path}@${ref_}`;

    const cached = fileLinesCache.get(key);
    if (cached) return cached;

    // Deduplicate inflight requests
    const inflight = fileLinesInflight.get(key);
    if (inflight) return inflight;

    const promise = getFileContent(owner, repo, path, ref_).then((lines) => {
      fileLinesCache.set(key, lines);
      fileLinesInflight.delete(key);
      return lines;
    });
    fileLinesInflight.set(key, promise);
    return promise;
  },

  /** Replace hunks + side_by_side for a specific file after context expansion */
  updateFileDiff(filename: string, newHunks: DiffHunk[]) {
    const newSbs = toSideBySide(newHunks);
    setFiles((prev) =>
      prev.map((f) => {
        if (f.filename !== filename || !f.diff) return f;
        return {
          ...f,
          diff: {
            ...f.diff,
            hunks: newHunks,
            side_by_side: newSbs,
          },
        };
      })
    );
  },

  reset() {
    setPr(null);
    setFiles([]);
    setComments([]);
    setReviewedFiles(new Set<string>());
    setSelectedFilePath(null);
    setIsLoadingFiles(false);
    setIsLoadingComments(false);
    setThreadResolutions(new Map());
    setCommits([]);
    setCommitSelection({ type: "all" });
    fileLinesCache.clear();
    fileLinesInflight.clear();
  },
};
