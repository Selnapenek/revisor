import { invoke } from '@tauri-apps/api/core';

export const getPrHeadSourceSnapshot = (owner: string, repo: string, headSha: string) =>
  invoke<{ files: Record<string, string> }>('get_pr_head_source_snapshot', { owner, repo, headSha });
