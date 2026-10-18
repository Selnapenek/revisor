import { buildThreads } from './comments.ts';
import type { ReviewComment, ThreadResolutionInfo } from '../types/pr';
import type { ProcessedFile } from '../api/files';

export interface Finding {
  id: number;
  path: string;
  line: number;
  body: string;
  replies: string[];
  htmlUrl: string;
  author: string;
}
export interface RankedFinding {
  id: number;
  impact: number;
  support: 'supported' | 'contradicted' | 'insufficient_context';
  confidence: number;
  effectiveSupport?: 'supported' | 'contradicted' | 'insufficient_context';
}
export interface RankingResult {
  ranked: RankedFinding[];
  failed: { id: number; reason: string }[];
  error: string | null;
  inputTokens: number;
  outputTokens: number;
}
export interface RankingFile { path: string; status: string; content: string; diff: string; ref: string }
export interface RankingSnapshot { owner: string; repo: string; number: number; baseSha: string; headSha: string; files: RankingFile[]; findings: Finding[]; contextKind?: 'references' }

export function selectFindings(comments: ReviewComment[], resolutions: Map<number, ThreadResolutionInfo>, authors: string[]): Finding[] {
  const selected = new Set(authors);
  return buildThreads(comments, resolutions)
    .filter((thread) => selected.has(thread.root.author_login) && !thread.isResolved && !thread.root.outdated && thread.root.body.trim())
    .map((thread) => ({
      id: thread.root.id, path: thread.root.path, line: thread.line,
      body: thread.root.body, replies: thread.replies.map((reply) => reply.body),
      htmlUrl: thread.root.html_url, author: thread.root.author_login,
    }));
}

export function countExcludedFindings(comments: ReviewComment[], resolutions: Map<number, ThreadResolutionInfo>, authors: string[]): number {
  const selected = new Set(authors);
  const ids = new Set(comments.map((comment) => comment.id));
  const rootCount = comments.filter((comment) => selected.has(comment.author_login)
    && (comment.in_reply_to_id === null || !ids.has(comment.in_reply_to_id))).length;
  return rootCount - selectFindings(comments, resolutions, authors).length;
}

export function rankFindings(items: RankedFinding[]): RankedFinding[] {
  const weight = (support: RankedFinding['support']) => support === 'supported' ? 1 : support === 'insufficient_context' ? 0.5 : 0;
  return items.map((item) => ({ ...item, effectiveSupport: item.confidence < 0.6 ? 'insufficient_context' as const : item.support }))
    .sort((a, b) => b.impact * weight(b.effectiveSupport) - a.impact * weight(a.effectiveSupport)
      || b.impact - a.impact || a.id - b.id);
}

export function canApplyRanking(id: number, currentId: number, snapshotKey: string, currentKey: string): boolean {
  return id === currentId && snapshotKey === currentKey;
}

export function rankingRevision(file: ProcessedFile, pr: { base_sha: string; head_sha: string }): string {
  return file.status === 'removed' ? pr.base_sha : pr.head_sha;
}

export function rankingFilesError(files: ProcessedFile[]): string | null {
  if (!files.length || files.length > 50) return 'Ranking needs 1 to 50 changed files.';
  if (files.some((file) => !file.diff || file.diff.is_binary || file.diff.is_too_large)) {
    return 'A changed file has no available text diff. Complete context is required.';
  }
  return null;
}
