import { createSignal, createMemo } from "solid-js";
import type { PullRequest, ReviewComment, Commit } from "~/types/pr";
import type { ProcessedFile } from "~/api/files";

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
const [commits, setCommits] = createSignal<Commit[]>([]);
const [commitSelection, setCommitSelection] = createSignal<CommitSelection>({ type: "all" });

export const prDetailStore = {
  get pr() { return pr(); },
  get files() { return files(); },
  get comments() { return comments(); },
  get reviewedFiles() { return reviewedFiles(); },
  get selectedFilePath() { return selectedFilePath(); },
  get diffMode() { return diffMode(); },
  get isLoadingFiles() { return isLoadingFiles(); },
  get isLoadingComments() { return isLoadingComments(); },
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

  reset() {
    setPr(null);
    setFiles([]);
    setComments([]);
    setReviewedFiles(new Set<string>());
    setSelectedFilePath(null);
    setIsLoadingFiles(false);
    setIsLoadingComments(false);
    setCommits([]);
    setCommitSelection({ type: "all" });
  },
};
