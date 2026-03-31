import { invoke } from "@tauri-apps/api/core";

export async function getReviewedFiles(
  owner: string,
  repo: string,
  prNumber: number,
): Promise<string[]> {
  return invoke<string[]>("get_reviewed_files", { owner, repo, prNumber });
}

export async function markFileReviewed(
  owner: string,
  repo: string,
  prNumber: number,
  filePath: string,
): Promise<void> {
  return invoke("mark_file_reviewed", { owner, repo, prNumber, filePath });
}

export async function unmarkFileReviewed(
  owner: string,
  repo: string,
  prNumber: number,
  filePath: string,
): Promise<void> {
  return invoke("unmark_file_reviewed", { owner, repo, prNumber, filePath });
}
