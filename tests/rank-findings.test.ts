import { test } from 'node:test';
import assert from 'node:assert/strict';
import { rankFindings, selectFindings, countExcludedFindings, canApplyRanking, rankingFilesError, rankingRevision } from '../src/lib/rank-findings.ts';
import type { ReviewComment } from '../src/types/pr.ts';
import type { ProcessedFile } from '../src/api/files.ts';

const comment = (id: number, author = 'bot'): ReviewComment => ({ id, path: 'a.ts', line: 3, original_line: null, side: 'RIGHT', diff_hunk: '@@', body: 'bug', author_login: author, author_avatar: '', created_at: '', updated_at: '', in_reply_to_id: null, review_id: null, html_url: '', outdated: false });

test('selects open current roots from selected authors and keeps replies as context', () => {
  const reply = { ...comment(2), in_reply_to_id: 1, body: 'reply' };
  const resolved = new Map([[3, { root_comment_id: 3, thread_node_id: 'x', is_resolved: true }]]);
  const selected = selectFindings([comment(1), reply, comment(3), { ...comment(4), outdated: true }, comment(5, 'human')], resolved, ['bot']);
  assert.deepEqual(selected.map((item) => item.id), [1]);
  assert.deepEqual(selected[0].replies, ['reply']);
  assert.equal(countExcludedFindings([comment(1), reply, comment(3), { ...comment(4), outdated: true }, { ...comment(6), line: null, original_line: null }], resolved, ['bot']), 3);
});

test('ranks by impact and support, then stable id', () => {
  const ranked = rankFindings([
    { id: 3, impact: 3, support: 'contradicted' as const, confidence: .9 },
    { id: 2, impact: 2, support: 'supported' as const, confidence: .8 },
    { id: 1, impact: 3, support: 'supported' as const, confidence: .3 },
  ]);
  assert.deepEqual(ranked.map((x) => x.id), [2, 1, 3]);
  assert.equal(ranked[1].effectiveSupport, 'insufficient_context');
});

test('rejects results from stale PR or request', () => {
  assert.equal(canApplyRanking(1, 1, 'a', 'a'), true);
  assert.equal(canApplyRanking(1, 2, 'a', 'a'), false);
  assert.equal(canApplyRanking(1, 1, 'a', 'b'), false);
});

test('full-file context uses the correct revision and refuses missing text diff', () => {
  const file = { filename: 'a.ts', status: 'removed', diff: null } as ProcessedFile;
  assert.equal(rankingRevision(file, { base_sha: 'base', head_sha: 'head' }), 'base');
  assert.equal(rankingRevision({ ...file, status: 'renamed' }, { base_sha: 'base', head_sha: 'head' }), 'head');
  assert.match(rankingFilesError([file]) ?? '', /no available text diff/);
  assert.match(rankingFilesError(Array.from({ length: 51 }, () => file)) ?? '', /50 changed files/);
});
