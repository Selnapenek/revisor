import { invoke } from "@tauri-apps/api/core";
import type { Commit } from "~/types/pr";
import type { ProcessedFile } from "./files";

export async function getPrCommits(
  owner: string,
  repo: string,
  number: number,
): Promise<Commit[]> {
  return invoke<Commit[]>("get_pr_commits", { owner, repo, number });
}

export async function getCompareFiles(
  owner: string,
  repo: string,
  base: string,
  head: string,
): Promise<ProcessedFile[]> {
  return invoke<ProcessedFile[]>("get_compare_files", { owner, repo, base, head });
}
