import { invoke } from "@tauri-apps/api/core";
import type { PaginatedResult } from "~/types/pr";

export interface RepoInfo {
  owner: string;
  name: string;
  full_name: string;
  description: string | null;
  private: boolean;
  fork: boolean;
  stargazers_count: number;
  updated_at: string;
  html_url: string;
}

export async function getSavedRepos(): Promise<[string, string][]> {
  return invoke<[string, string][]>("get_saved_repos");
}

export async function saveRepo(owner: string, repo: string): Promise<void> {
  return invoke("save_repo", { owner, repo });
}

export async function removeRepo(owner: string, repo: string): Promise<void> {
  return invoke("remove_repo", { owner, repo });
}

export async function listUserRepos(
  page: number,
  perPage: number = 30,
): Promise<PaginatedResult<RepoInfo>> {
  return invoke<PaginatedResult<RepoInfo>>("list_user_repos", { page, perPage });
}

export async function searchRepos(
  query: string,
  page: number,
  perPage: number = 20,
): Promise<PaginatedResult<RepoInfo>> {
  return invoke<PaginatedResult<RepoInfo>>("search_repos", { query, page, perPage });
}
